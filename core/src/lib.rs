pub mod dav;
pub mod json;
mod login;
mod notes;
mod settings;
pub mod store;
mod sync;

use dav::Dav;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub use login::{Credentials, LoginFlow, login_flow_poll, login_flow_start, password_login};
pub use notes::Note;
pub use settings::DEVICE_ONLY;
pub use sync::{DEFAULT_FOLDER, SyncReport};

#[derive(Debug, Clone)]
pub struct Account {
    pub url: String,
    pub user: String,
}

pub trait ChangeListener: Send + Sync {
    /// Notes changed, by path relative to the notes folder.
    fn changed(&self, paths: Vec<String>);
    /// Settings changed by another device arrived.
    fn settings_changed(&self) {}
}

pub struct Client {
    db: Mutex<Connection>,
    dav: Mutex<Option<Arc<Dav>>>,
    listeners: Mutex<Vec<Box<dyn ChangeListener>>>,
    sync_lock: Mutex<()>,
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
        let db = store::open(&format!("{data_dir}/core.sqlite"))?;
        Ok(Client {
            db: Mutex::new(db),
            dav: Mutex::new(None),
            listeners: Mutex::new(Vec::new()),
            sync_lock: Mutex::new(()),
        })
    }

    /// The password is kept in memory only; front-ends keep it in the platform keyring.
    pub fn set_account(&self, url: String, user: String, app_password: String) -> Result<()> {
        let db = self.db.lock().unwrap();
        if store::kv_get(&db, "account_url")?.as_deref() != Some(&url)
            || store::kv_get(&db, "account_user")?.as_deref() != Some(&user)
        {
            // Different account: its copy belongs to the old one.
            db.execute_batch(
                "DELETE FROM notes; DELETE FROM folders; DELETE FROM kv WHERE key LIKE 'config_%';",
            )?;
        }
        store::kv_set(&db, "account_url", &url)?;
        store::kv_set(&db, "account_user", &user)?;
        *self.dav.lock().unwrap() = Some(Arc::new(Dav::new(&url, &user, &app_password)));
        Ok(())
    }

    pub fn account(&self) -> Option<Account> {
        let db = self.db.lock().unwrap();
        Some(Account {
            url: store::kv_get(&db, "account_url").ok()??,
            user: store::kv_get(&db, "account_user").ok()??,
        })
    }

    /// Forget the account and its local copy.
    pub fn logout(&self) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute_batch(
            "DELETE FROM notes; DELETE FROM folders;
             DELETE FROM kv WHERE key LIKE 'config_%' OR key LIKE 'account_%';",
        )?;
        *self.dav.lock().unwrap() = None;
        Ok(())
    }

    fn dav(&self) -> Result<Arc<Dav>> {
        self.dav.lock().unwrap().clone().ok_or(Error::NoAccount)
    }

    /// The user's Nextcloud Files root, `/remote.php/dav/files/<user id>/`. The user id can
    /// differ from the login name (an email address, for one), so it comes from the principal.
    pub fn files_home(&self) -> Result<String> {
        if let Some(h) = store::kv_get(&self.db.lock().unwrap(), "config_files_home")? {
            return Ok(h);
        }
        let principal = self.dav()?.principal()?;
        let user = principal
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or_default();
        let home = format!("/remote.php/dav/files/{user}/");
        store::kv_set(&self.db.lock().unwrap(), "config_files_home", &home)?;
        Ok(home)
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
