mod account;
pub mod dav;
mod files;
pub mod journal;
pub mod json;
mod login;
pub mod markdown;
mod notes;
mod settings;
pub mod store;
mod sync;
pub mod table;
mod undo;

use dav::Dav;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

pub use account::{Account, AccountKind};
pub use files::{Deleted, Features, Version};
pub use login::{Credentials, LoginFlow, login_flow_poll, login_flow_start, password_login};
pub use notes::Note;
pub use settings::DEVICE_ONLY;
pub use sync::{DEFAULT_FOLDER, Locked, SyncReport};

pub trait ChangeListener: Send + Sync {
    /// Notes changed, by path relative to the notes folder.
    fn changed(&self, paths: Vec<String>);
    /// Settings changed by another device arrived.
    fn settings_changed(&self) {}
}

pub struct Client {
    db: Mutex<Connection>,
    /// The connections of the accounts connected in this run, by account id.
    dav: Mutex<HashMap<i64, Arc<Dav>>>,
    listeners: Mutex<Vec<Box<dyn ChangeListener>>>,
    sync_lock: Mutex<()>,
    journal: Mutex<undo::Journal>,
    /// The accounts whose server's features were asked for in this run (`refresh_features`).
    features_asked: Mutex<HashSet<i64>>,
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Client {
    pub fn open(data_dir: String) -> Result<Client> {
        std::fs::create_dir_all(&data_dir).map_err(|e| Error::Db(e.to_string()))?;
        // Not `core.sqlite`, Notav 0.1's, whose schema versions this one's would be taken for.
        let db = store::open(&format!("{data_dir}/notav.sqlite"))?;
        Ok(Client {
            db: Mutex::new(db),
            dav: Mutex::default(),
            listeners: Mutex::new(Vec::new()),
            sync_lock: Mutex::new(()),
            journal: Mutex::default(),
            features_asked: Default::default(),
        })
    }

    pub fn subscribe(&self, listener: Box<dyn ChangeListener>) {
        self.listeners.lock().unwrap().push(listener);
    }
}

#[derive(Debug)]
pub enum Error {
    Auth,
    Conflict,
    NoAccount,
    NotFound(String),
    /// A note has that name already.
    Exists(String),
    /// Not a name a note or folder can have.
    Invalid(String),
    /// Locked on the server (HTTP 423), by an editor that has it open.
    Locked(String),
    Http(u16, String),
    Net(String),
    Parse(String),
    Db(String),
    /// The data was left by a newer Notav, whose schema this one may not read or write.
    Newer(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::Auth => write!(f, "authentication failed"),
            Error::Conflict => write!(f, "precondition failed (changed on server)"),
            Error::NoAccount => write!(f, "no account configured"),
            Error::NotFound(s) => write!(f, "not found: {s}"),
            Error::Exists(s) => write!(f, "exists already: {s}"),
            Error::Invalid(s) => write!(f, "invalid name: {s}"),
            Error::Locked(s) => write!(f, "locked on the server: {s}"),
            // A WebDAV error body is XML: its <s:message> is the part worth reading, else (it
            // can be empty) the exception's name.
            Error::Http(code, body) => {
                let tag = |t: &str| {
                    body.split_once(&format!("<s:{t}>"))
                        .and_then(|(_, m)| m.split_once(&format!("</s:{t}>")))
                        .map(|(m, _)| m.trim())
                        .filter(|m| !m.is_empty())
                };
                let message = tag("message")
                    .or_else(|| tag("exception").map(|e| e.rsplit('\\').next().unwrap_or(e)))
                    .unwrap_or(body.as_str());
                let short: String = message.chars().take(300).collect();
                write!(f, "HTTP {code}: {}", short.trim())
            }
            Error::Net(s) => write!(f, "network: {s}"),
            Error::Parse(s) => write!(f, "parse: {s}"),
            Error::Db(s) => write!(f, "database: {s}"),
            Error::Newer(s) => write!(f, "made by a newer version of Notav: {s}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Db(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Base64; `url` = URL-safe alphabet without padding.
pub fn b64encode(data: &[u8], url: bool) -> String {
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = c
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..=c.len() {
            let ch = B64[(n >> (18 - 6 * i) & 63) as usize] as char;
            out.push(match (url, ch) {
                (true, '+') => '-',
                (true, '/') => '_',
                _ => ch,
            });
        }
        if !url {
            out.extend(std::iter::repeat_n('=', 3 - c.len()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_errors_show_the_server_message_or_exception() {
        let body = |inner: &str| {
            format!(
                "<?xml version=\"1.0\"?>\n<d:error xmlns:d=\"DAV:\" xmlns:s=\"http://sabredav.org/ns\">{inner}</d:error>"
            )
        };
        let shown = |inner: &str| Error::Http(423, body(inner)).to_string();
        assert_eq!(
            shown("<s:exception>Sabre\\DAV\\Exception\\Locked</s:exception><s:message/>"),
            "HTTP 423: Locked"
        );
        assert_eq!(
            shown("<s:exception>X</s:exception><s:message>Plan.md is locked</s:message>"),
            "HTTP 423: Plan.md is locked"
        );
        assert_eq!(
            Error::Http(500, "oops".into()).to_string(),
            "HTTP 500: oops"
        );
    }
}
