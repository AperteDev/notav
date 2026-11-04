//! Notav's core for the apps that are not in Rust (Android, in Kotlin), through UniFFI.
//!
//! One object, `Notav`, holds the database (`notav_core::Client`); pure rules are plain
//! functions. Everything is synchronous and may block on the network: call it off the UI
//! thread. `Listener` is called from Notav's own threads. Text positions are UTF-16 code units,
//! as Kotlin counts them (the core counts characters).

use notav_core::dav::ServerInfo;
use notav_core::{
    Account, AccountKind, Client, Credentials, Deleted, Features, LoginFlow, Note, SyncReport,
    Version,
};
use notav_core::{accounts, markdown};
use std::sync::Arc;

mod error;
mod i18n;
mod types;

pub use error::{NotavError, Result};
pub use types::*;

uniffi::setup_scaffolding!();

// --- callbacks -------------------------------------------------------------------------

/// Told about changes to the data, from Notav's threads.
#[uniffi::export(callback_interface)]
pub trait Listener: Send + Sync {
    /// Notes changed, by path relative to the notes folder.
    fn changed(&self, paths: Vec<String>);
    /// Settings changed by another device arrived.
    fn settings_changed(&self);
    /// A sync of `account` went through `done` of the `total` notes it downloads.
    fn progress(&self, account: i64, done: u32, total: u32);
}

/// Where the front-end keeps the app passwords of accounts signed in in Notav, by server and
/// user.
#[uniffi::export(callback_interface)]
pub trait Keyring: Send + Sync {
    /// None when stored, else why not.
    fn store(&self, url: String, user: String, password: String) -> Option<String>;
    fn clear(&self, url: String, user: String);
}

struct KeyringAdapter(Box<dyn Keyring>);

impl notav_core::accounts::Keyring for KeyringAdapter {
    fn store(&self, url: &str, user: &str, password: &str) -> std::result::Result<(), String> {
        match self.0.store(url.into(), user.into(), password.into()) {
            None => Ok(()),
            Some(e) => Err(e),
        }
    }

    fn clear(&self, url: &str, user: &str) {
        self.0.clear(url.into(), user.into());
    }
}

struct ListenerAdapter(Box<dyn Listener>);

impl notav_core::ChangeListener for ListenerAdapter {
    fn changed(&self, paths: Vec<String>) {
        self.0.changed(paths);
    }

    fn settings_changed(&self) {
        self.0.settings_changed();
    }

    fn progress(&self, account: i64, done: u32, total: u32) {
        self.0.progress(account, done, total);
    }
}

// --- the object ------------------------------------------------------------------------

/// The local database and the accounts: `notav_core::Client`, as is.
#[derive(uniffi::Object)]
pub struct Notav(Client);

#[uniffi::export]
impl Notav {
    /// Open (or create) the database in `data_dir`.
    #[uniffi::constructor]
    pub fn open(data_dir: String) -> Result<Arc<Self>> {
        Ok(Arc::new(Notav(Client::open(data_dir)?)))
    }

    pub fn subscribe(&self, listener: Box<dyn Listener>) {
        self.0.subscribe(Box::new(ListenerAdapter(listener)))
    }

    // --- accounts

    pub fn accounts(&self) -> Vec<Account> {
        self.0.accounts()
    }

    pub fn account(&self, id: i64) -> Option<Account> {
        self.0.account(id)
    }

    /// A new account, not connected yet; refuses one here already (`Exists`).
    pub fn add_account(
        &self,
        kind: AccountKind,
        url: String,
        user: String,
        source: String,
    ) -> Result<i64> {
        Ok(self.0.add_account(kind, url, user, source)?)
    }

    pub fn reorder_accounts(&self, ids: Vec<i64>) -> Result<()> {
        Ok(self.0.reorder_accounts(ids)?)
    }

    pub fn rename_account(&self, id: i64, name: String) -> Result<()> {
        Ok(self.0.rename_account(id, name)?)
    }

    pub fn set_account_enabled(&self, id: i64, enabled: bool) -> Result<()> {
        Ok(self.0.set_account_enabled(id, enabled)?)
    }

    /// Connect for this run with the account's password (kept in memory only).
    pub fn connect(&self, id: i64, password: String) -> Result<()> {
        Ok(self.0.connect(id, password)?)
    }

    pub fn connected(&self, id: i64) -> bool {
        self.0.connected(id)
    }

    /// The server moved: same user and password at `url` (sync first).
    pub fn move_account(&self, id: i64, url: String, password: String) -> Result<()> {
        Ok(self.0.move_account(id, url, password)?)
    }

    /// Add a Nextcloud account signed in here and connect it, its app password in `keyring`;
    /// with `settings`, settings sync through it. Its notes sync once its folder is chosen.
    pub fn add_nextcloud(
        &self,
        keyring: Box<dyn Keyring>,
        url: String,
        user: String,
        password: String,
        settings: bool,
    ) -> Result<i64> {
        let k = KeyringAdapter(keyring);
        Ok(accounts::add(
            &self.0,
            &k,
            &url,
            &user,
            password,
            String::new(),
            settings,
        )?)
    }

    /// Account `id` at `url` with `password`: a new app password, or the same user on a moved
    /// server.
    pub fn reconnect(
        &self,
        keyring: Box<dyn Keyring>,
        id: i64,
        url: String,
        password: String,
    ) -> Result<()> {
        Ok(accounts::reconnect(
            &self.0,
            &KeyringAdapter(keyring),
            id,
            &url,
            password,
        )?)
    }

    /// Remove an account from this device, with its saved password; edits waiting for its
    /// server go up first (`Upload`, `NotUploaded` when they cannot), unless `force`.
    pub fn remove_account(&self, keyring: Box<dyn Keyring>, id: i64, force: bool) -> Result<()> {
        Ok(accounts::remove(
            &self.0,
            &KeyringAdapter(keyring),
            id,
            force,
        )?)
    }

    pub fn settings_account(&self) -> Option<i64> {
        self.0.settings_account()
    }

    pub fn set_settings_account(&self, id: Option<i64>) -> Result<()> {
        Ok(self.0.set_settings_account(id)?)
    }

    /// Switched on and connected.
    pub fn syncing(&self, id: i64) -> bool {
        self.0.syncing(id)
    }

    pub fn server_info(&self, id: i64) -> Result<ServerInfo> {
        Ok(self.0.server_info(id)?)
    }

    // --- sync

    /// Settings, then every account that syncs; the first failure is returned.
    pub fn sync(&self) -> Result<SyncReport> {
        Ok(self.0.sync()?)
    }

    pub fn sync_account(&self, id: i64) -> Result<SyncReport> {
        Ok(self.0.sync_account(id)?)
    }

    pub fn sync_settings(&self) -> Result<()> {
        Ok(self.0.sync_settings()?)
    }

    /// Something waits for the server: of `account`, or of any switched on.
    pub fn pending(&self, account: Option<i64>) -> bool {
        self.0.pending(account)
    }

    pub fn chosen_notes_folder(&self, id: i64) -> Option<String> {
        self.0.chosen_notes_folder(id)
    }

    pub fn notes_folder(&self, id: i64) -> String {
        self.0.notes_folder(id)
    }

    pub fn suggested_notes_folder(&self, id: i64) -> String {
        self.0.suggested_notes_folder(id)
    }

    pub fn set_notes_folder(&self, id: i64, folder: String, move_notes: bool) -> Result<()> {
        Ok(self.0.set_notes_folder(id, folder, move_notes)?)
    }

    pub fn unlock(&self, id: i64, file: String) -> Result<()> {
        Ok(self.0.unlock(id, file)?)
    }

    // --- notes and folders

    pub fn notes(&self) -> Vec<Note> {
        self.0.notes()
    }

    pub fn note(&self, id: i64) -> Option<Note> {
        self.0.note(id)
    }

    pub fn note_text(&self, id: i64) -> Option<String> {
        self.0.note_text(id)
    }

    pub fn search(&self, query: String) -> Vec<Note> {
        self.0.search(query)
    }

    pub fn folders(&self, account: i64) -> Vec<String> {
        self.0.folders(account)
    }

    pub fn follow_link(&self, from: i64, target: String, wiki: bool) -> Result<Note> {
        Ok(self.0.follow_link(from, target, wiki)?)
    }

    pub fn backlinks(&self, id: i64) -> Vec<Note> {
        self.0.backlinks(id)
    }

    pub fn add_note(
        &self,
        account: i64,
        folder: String,
        title: String,
        text: String,
    ) -> Result<Note> {
        Ok(self.0.add_note(account, folder, title, text)?)
    }

    pub fn set_text(&self, id: i64, text: String) -> Result<()> {
        Ok(self.0.set_text(id, text)?)
    }

    pub fn move_note(&self, id: i64, folder: String, title: String) -> Result<Note> {
        Ok(self.0.move_note(id, folder, title)?)
    }

    pub fn delete_note(&self, id: i64) -> Result<()> {
        Ok(self.0.delete_note(id)?)
    }

    pub fn add_folder(&self, account: i64, folder: String) -> Result<()> {
        Ok(self.0.add_folder(account, folder)?)
    }

    pub fn move_folder(&self, account: i64, folder: String, parent: String) -> Result<String> {
        Ok(self.0.move_folder(account, folder, parent)?)
    }

    pub fn rename_folder(&self, account: i64, folder: String, name: String) -> Result<String> {
        Ok(self.0.rename_folder(account, folder, name)?)
    }

    pub fn delete_folder(&self, account: i64, folder: String) -> Result<()> {
        Ok(self.0.delete_folder(account, folder)?)
    }

    pub fn set_favorite(&self, id: i64, on: bool) -> Result<()> {
        Ok(self.0.set_favorite(id, on)?)
    }

    // --- undo

    /// The latest change, to undo just that one later (a snackbar's Undo).
    pub fn last_change(&self) -> Option<u64> {
        self.0.last_change()
    }

    /// Put back what change `id` (else the latest) replaced; false when there is none.
    pub fn undo(&self, id: Option<u64>) -> Result<bool> {
        Ok(self.0.undo(id)?)
    }

    // --- trash and versions

    pub fn features(&self, id: i64) -> Features {
        self.0.features(id)
    }

    pub fn refresh_features(&self, id: i64) -> Result<Features> {
        Ok(self.0.refresh_features(id)?)
    }

    pub fn trash(&self, account: i64) -> Result<Vec<Deleted>> {
        Ok(self.0.trash(account)?)
    }

    pub fn restore_deleted(&self, account: i64, id: String) -> Result<()> {
        Ok(self.0.restore_deleted(account, id)?)
    }

    pub fn purge(&self, account: i64, id: String) -> Result<()> {
        Ok(self.0.purge(account, id)?)
    }

    pub fn versions(&self, id: i64) -> Result<Vec<Version>> {
        Ok(self.0.versions(id)?)
    }

    pub fn version_text(&self, account: i64, id: String) -> Result<String> {
        Ok(self.0.version_text(account, id)?)
    }

    // --- journal

    pub fn journal_folder(&self) -> String {
        self.0.journal_folder()
    }

    pub fn journal_format(&self) -> String {
        self.0.journal_format()
    }

    pub fn journal_account(&self) -> Option<i64> {
        self.0.journal_account()
    }

    /// The daily note of a day: the one there is, or a new one from the template.
    pub fn daily_note(&self, year: i32, month: u32, day: u32) -> Result<Note> {
        Ok(self.0.daily_note(year, month, day)?)
    }

    // --- settings

    pub fn setting(&self, key: String) -> Option<String> {
        self.0.setting(key)
    }

    /// Records the change time too, so the newest value wins when settings sync.
    pub fn set_setting(&self, key: String, value: String) -> Result<()> {
        Ok(self.0.set_setting(key, value)?)
    }
}

// --- translations ---------------------------------------------------------------------

/// One language's messages, from the `notav` gettext catalogs: what the Linux app shows, with
/// the same contexts and plural rules. Placeholders are named (`{n}`); fill them after.
#[derive(uniffi::Object)]
pub struct Translations(i18n::Catalog);

#[uniffi::export]
impl Translations {
    /// A language's catalog: the `.mo` file msgfmt makes of `po/<language>.po`.
    #[uniffi::constructor]
    pub fn new(mo: Vec<u8>) -> Result<Arc<Self>> {
        match i18n::Catalog::parse(&mo) {
            Some(c) => Ok(Arc::new(Translations(c))),
            None => Err(NotavError::Parse {
                reason: "not a .mo file".into(),
            }),
        }
    }

    /// English: the messages as written.
    #[uniffi::constructor]
    pub fn english() -> Arc<Self> {
        Arc::new(Translations(i18n::Catalog::english()))
    }

    pub fn gettext(&self, msgid: String) -> String {
        self.0.gettext(&msgid)
    }

    /// The form for `n` of a counted message.
    pub fn ngettext(&self, one: String, many: String, n: u64) -> String {
        self.0.ngettext(&one, &many, n)
    }

    /// A message with a context, for a short word that could translate two ways.
    pub fn pgettext(&self, context: String, msgid: String) -> String {
        self.0.pgettext(&context, &msgid)
    }
}

// --- sign-in -------------------------------------------------------------------------------

/// Start Nextcloud's Login Flow v2: open `login_url` in a browser, then poll.
#[uniffi::export]
pub fn login_flow_start(server: String) -> Result<LoginFlow> {
    Ok(notav_core::login_flow_start(server)?)
}

/// The credentials once the user granted access, None while waiting.
#[uniffi::export]
pub fn login_flow_poll(flow: LoginFlow) -> Result<Option<Credentials>> {
    Ok(notav_core::login_flow_poll(&flow)?)
}

/// Sign in with a login name and a password (or app password): an app password for Notav.
#[uniffi::export]
pub fn password_login(server: String, login_name: String, password: String) -> Result<Credentials> {
    Ok(notav_core::password_login(server, login_name, password)?)
}

/// Trust these certificate authorities (DER) for HTTPS: on Android its store's, the system's
/// and the user's, as the core cannot read them itself. Call it before the first request.
#[uniffi::export]
pub fn set_trusted_certificates(certs: Vec<Vec<u8>>) {
    notav_core::set_roots(certs);
}

/// The host of a Nextcloud account's server, for its default name ("user@host").
#[uniffi::export]
pub fn account_host(account: Account) -> String {
    accounts::host(&account).into()
}

/// The notes folder of an account until one is chosen ("Notes").
#[uniffi::export]
pub fn default_notes_folder() -> String {
    notav_core::DEFAULT_FOLDER.into()
}

// --- Markdown ----------------------------------------------------------------------------

/// The styled ranges of a note's text, for Editor mode.
#[uniffi::export]
pub fn note_spans(text: String) -> Vec<NoteSpan> {
    let at = Utf16::of(&text);
    markdown::spans(&text)
        .into_iter()
        .map(|s| NoteSpan {
            start: at.unit(s.start),
            end: at.unit(s.end),
            style: s.style.into(),
        })
        .collect()
}

/// What Enter at `at` does in a list item or a quote (none: a plain new line).
#[uniffi::export]
pub fn note_enter(text: String, at: u32) -> Vec<NoteEdit> {
    let p = Utf16::of(&text);
    p.edits(markdown::enter(&text, p.char(at)))
}

/// The link at `at`, if there is one to follow.
#[uniffi::export]
pub fn note_link_at(text: String, at: u32) -> Option<NoteLink> {
    let p = Utf16::of(&text);
    markdown::link_at(&text, p.char(at)).map(Into::into)
}

/// The headings of a note, in order.
#[uniffi::export]
pub fn note_headings(text: String) -> Vec<NoteHeading> {
    let p = Utf16::of(&text);
    markdown::headings(&text)
        .into_iter()
        .map(|h| NoteHeading {
            level: h.level,
            title: h.title,
            at: p.unit(h.at),
        })
        .collect()
}

/// The table holding `at` (where the text was last changed) aligned, the cursor at `cursor`
/// after it; None when there is no table there or it is aligned already.
#[uniffi::export]
pub fn note_table_tidy(text: String, at: u32, cursor: u32) -> Option<NoteChange> {
    let p = Utf16::of(&text);
    let c = notav_core::table::apply(
        &text,
        p.char(at),
        p.char(cursor),
        notav_core::table::Op::Tidy,
    )?;
    let cursor = Utf16::of(&apply(&text, &c.edit)).unit(c.cursor);
    Some(NoteChange {
        start: p.unit(c.edit.start),
        end: p.unit(c.edit.end),
        text: c.edit.text,
        cursor,
    })
}

/// `text` with `edit` made (character positions).
fn apply(text: &str, edit: &markdown::Edit) -> String {
    let mut chars: Vec<char> = text.chars().collect();
    chars.splice(edit.start..edit.end, edit.text.chars());
    chars.into_iter().collect()
}

/// Where the heading titled `heading` starts in a note, as a link names it.
#[uniffi::export]
pub fn note_heading_at(text: String, heading: String) -> Option<u32> {
    let p = Utf16::of(&text);
    markdown::heading_at(&text, &heading).map(|c| p.unit(c))
}
