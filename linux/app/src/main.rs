//! Notav for Linux: the GTK app, with a sync worker while it runs. There is no background
//! service: closing the window quits, once what is waiting for the server has gone up.

mod add_account;
mod editor;
mod files;
mod goa;
mod i18n;
mod settings;
mod update;
mod window;

use adw::prelude::*;
use gtk::{gio, glib};
use notav_core::{Account, AccountKind, ChangeListener, Client};
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

pub const APP_ID: &str = "dev.aperte.Notav";

static CORE: OnceLock<Arc<Client>> = OnceLock::new();
static SYNC: OnceLock<Sender<()>> = OnceLock::new();
static SYNC_STATE: Mutex<String> = Mutex::new(String::new());
/// Per account: when its last sync went through (unix time) and why its last one failed, for
/// Preferences; this run's only.
static LAST_SYNC: Mutex<Option<HashMap<i64, i64>>> = Mutex::new(None);
static SYNC_ERRORS: Mutex<Option<HashMap<i64, String>>> = Mutex::new(None);

thread_local! {
    /// The window was closed with changes waiting for the server: quit after the next sync.
    static QUIT_PENDING: Cell<bool> = const { Cell::new(false) };
}

pub fn core() -> &'static Arc<Client> {
    CORE.get().expect("core opened at startup")
}

/// After an edit: sync if it left something for the server.
pub fn request_sync() {
    if core().pending(None) {
        sync_now();
    }
}

/// Ask the worker for a full sync; requests arriving while one runs are coalesced.
pub fn sync_now() {
    if let Some(tx) = SYNC.get() {
        let _ = tx.send(());
    }
}

/// The main window has something to show: an account whose notes are here (the local one, or
/// a Nextcloud one with its notes folder chosen; one not connected too: a banner offers to sign
/// in again).
pub fn ready() -> bool {
    core()
        .accounts()
        .iter()
        .any(|a| a.kind == AccountKind::Local || core().chosen_notes_folder(a.id).is_some())
}

/// Some account syncs.
pub fn can_sync() -> bool {
    core().accounts().iter().any(|a| core().syncing(a.id))
}

pub fn sync_state() -> String {
    SYNC_STATE.lock().unwrap().clone()
}

/// When account `id` last synced in this run (unix time).
pub fn last_sync(id: i64) -> Option<i64> {
    LAST_SYNC.lock().unwrap().as_ref()?.get(&id).copied()
}

/// Why account `id`'s last sync failed.
pub fn sync_error(id: i64) -> Option<String> {
    SYNC_ERRORS.lock().unwrap().as_ref()?.get(&id).cloned()
}

/// Events from background threads, handled on the main thread.
#[derive(Debug, Clone)]
pub enum Event {
    /// Notes changed, by path.
    Changed(Vec<String>),
    /// Settings from another device arrived.
    SettingsChanged,
    /// "idle", "syncing" or "error:" with what failed, naming the accounts.
    SyncState(String),
    /// Conflict copies a sync made, by path.
    Conflicts(Vec<String>),
    /// Changes that wait for a lock on the server (empty: none).
    Locked(Vec<notav_core::Locked>),
}

fn post(ev: Event) {
    glib::MainContext::default().invoke(move || {
        if let Event::SyncState(s) = &ev {
            *SYNC_STATE.lock().unwrap() = s.clone();
            if s != "syncing"
                && QUIT_PENDING.take()
                && !window::is_visible()
                && let Some(app) = gio::Application::default()
            {
                app.quit();
            }
        }
        window::on_event(&ev);
        if let Event::SyncState(_) = &ev {
            settings::status_changed();
        }
    });
}

/// Settings, then every account that syncs; what failed is told by account.
fn sync_all() {
    let (mut conflicts, mut locked, mut failed) = (Vec::new(), Vec::new(), Vec::new());
    let mut errors = HashMap::new();
    if let Err(e) = core().sync_settings() {
        failed.push(e.to_string());
    }
    for a in core().accounts() {
        if a.kind != AccountKind::Nextcloud || !core().syncing(a.id) {
            continue;
        }
        match core().sync_account(a.id) {
            Ok(r) => {
                conflicts.extend(r.conflicts);
                locked.extend(r.locked);
                LAST_SYNC
                    .lock()
                    .unwrap()
                    .get_or_insert_default()
                    .insert(a.id, notav_core::now());
            }
            Err(e) => {
                failed.push(format!("{}: {e}", account_name(&a)));
                errors.insert(a.id, e.to_string());
            }
        }
    }
    *SYNC_ERRORS.lock().unwrap() = Some(errors);
    if !conflicts.is_empty() {
        post(Event::Conflicts(conflicts));
    }
    post(Event::Locked(locked));
    post(Event::SyncState(if failed.is_empty() {
        "idle".into()
    } else {
        format!("error:{}", failed.join("; "))
    }));
}

struct Listener;
impl ChangeListener for Listener {
    fn changed(&self, paths: Vec<String>) {
        post(Event::Changed(paths));
    }

    fn settings_changed(&self) {
        post(Event::SettingsChanged);
    }
}

/// The window closed (it only hides): quit, once the changes waiting for the server are up (or
/// the sync failed).
pub fn quit_after_sync(app: &adw::Application) {
    if SYNC.get().is_some() && can_sync() && core().pending(None) {
        QUIT_PENDING.set(true);
        sync_now();
    } else {
        app.quit();
    }
}

/// The window is shown again: no quitting after the sync.
pub fn stay() {
    QUIT_PENDING.set(false);
}

// --- keyring -----------------------------------------------------------------

/// `NOTAV_SANDBOX` set: a separate copy for testing other connections (`make -C linux sandbox`).
/// It runs next to the normal app, keeps its data where XDG_DATA_HOME says and its passwords
/// apart, so signing out there leaves the normal app signed in.
pub fn sandbox() -> bool {
    std::env::var_os("NOTAV_SANDBOX").is_some()
}

fn schema() -> libsecret::Schema {
    let attrs = HashMap::from([
        ("url", libsecret::SchemaAttributeType::String),
        ("user", libsecret::SchemaAttributeType::String),
    ]);
    let name = if sandbox() {
        format!("{APP_ID}.Sandbox")
    } else {
        APP_ID.into()
    };
    libsecret::Schema::new(&name, libsecret::SchemaFlags::NONE, attrs)
}

pub fn password_load(url: &str, user: &str) -> Option<String> {
    let attrs = HashMap::from([("url", url), ("user", user)]);
    libsecret::password_lookup_sync(Some(&schema()), attrs, gio::Cancellable::NONE)
        .ok()?
        .map(|s| s.to_string())
}

pub fn password_store(url: &str, user: &str, password: &str) -> Result<(), glib::Error> {
    let attrs = HashMap::from([("url", url), ("user", user)]);
    let label = format!("Notav ({user} @ {url})");
    libsecret::password_store_sync(
        Some(&schema()),
        attrs,
        Some(libsecret::COLLECTION_DEFAULT),
        &label,
        password,
        gio::Cancellable::NONE,
    )
}

pub fn password_clear(url: &str, user: &str) {
    let attrs = HashMap::from([("url", url), ("user", user)]);
    let _ = libsecret::password_clear_sync(Some(&schema()), attrs, gio::Cancellable::NONE);
}

// --- accounts ----------------------------------------------------------------

/// What an account is called: the name given to it, else "user@host", or "On This Device".
pub fn account_name(a: &Account) -> String {
    if !a.name.is_empty() {
        return a.name.clone();
    }
    match a.kind {
        AccountKind::Local => i18n::gettext("On This Device"),
        AccountKind::Nextcloud => format!("{}@{}", a.user, host(a)),
    }
}

/// The host of a Nextcloud account's server, as its default name shows it ("user@host").
pub fn host(a: &Account) -> &str {
    let host = a.url.split_once("://").map_or(a.url.as_str(), |(_, h)| h);
    host.trim_end_matches('/')
}

/// Connect a Nextcloud account with its saved password: from GNOME Online Accounts (its
/// `source`) or Notav's keyring entry. False without one (keyring cleared, GOA account gone or
/// with files turned off).
pub fn connect_saved(a: &Account) -> bool {
    if a.kind == AccountKind::Local {
        return false;
    }
    let pw = if a.source.is_empty() {
        password_load(&a.url, &a.user)
    } else if goa::accounts().iter().any(|g| g.id == a.source) {
        goa::password(&a.source)
    } else {
        None
    };
    pw.is_some_and(|pw| core().connect(a.id, pw).is_ok())
}

/// Connect every account with its saved password; false when there is none (the welcome page
/// shows).
pub fn restore_accounts() -> bool {
    let all = core().accounts();
    for a in &all {
        connect_saved(a);
    }
    !all.is_empty()
}

/// Why the edits waiting for an account's server could not go up before it changes.
fn upload_error(e: &notav_core::Error) -> String {
    i18n::gettext("Cannot upload pending changes first: {error}").replace("{error}", &e.to_string())
}

/// A Nextcloud account to add.
pub enum Target {
    Goa(goa::Account),
    /// Signed in here (browser or password); the app password goes into Notav's keyring entry.
    Manual(notav_core::Credentials),
}

/// Add a Nextcloud account and connect it (`settings`: settings sync through it); its notes
/// sync once its notes folder is chosen.
pub async fn add_account(target: Target, settings: bool) -> Result<i64, String> {
    let id = gio::spawn_blocking(move || -> Result<i64, String> {
        let (url, user, pw, source) = match target {
            Target::Goa(a) => {
                let pw = goa::password(&a.id).ok_or_else(|| {
                    i18n::gettext("Online Accounts has no password for this account")
                })?;
                (a.url, a.user, pw, a.id)
            }
            Target::Manual(c) => (c.server, c.login_name, c.app_password, String::new()),
        };
        let url = url.trim_end_matches('/').to_string();
        let id = core()
            .add_account(
                AccountKind::Nextcloud,
                url.clone(),
                user.clone(),
                source.clone(),
            )
            .map_err(|e| match e {
                notav_core::Error::Exists(_) => i18n::gettext("This account is already added"),
                e => e.to_string(),
            })?;
        if source.is_empty()
            && let Err(e) = password_store(&url, &user, &pw)
        {
            let _ = core().remove_account(id);
            return Err(
                i18n::gettext("Cannot save password: {error}").replace("{error}", &e.to_string())
            );
        }
        core().connect(id, pw).map_err(|e| e.to_string())?;
        if settings {
            let _ = core().set_settings_account(Some(id));
        }
        Ok(id)
    })
    .await
    .map_err(|_| i18n::gettext("Connecting failed"))??;
    start_background();
    Ok(id)
}

/// The local account, the first time with a note to start from.
pub fn add_local() -> Result<i64, String> {
    let id = core()
        .add_account(
            AccountKind::Local,
            String::new(),
            String::new(),
            String::new(),
        )
        .map_err(|e| e.to_string())?;
    let _ = core().add_note(
        id,
        String::new(),
        i18n::gettext("Welcome to Notav"),
        i18n::gettext(
            "Your notes stay on this device. They are written in Markdown: # starts a \
             heading, - a list, - [ ] a task, and [[Title]] links another note.",
        ),
    );
    start_background();
    Ok(id)
}

/// Account `id` (signed in here) at `url` with `password`: a new app password, or the same
/// user on a moved server (then its notes come again from there, after the edits waiting for
/// the old one went up).
pub async fn reconnect(id: i64, url: String, password: String) -> Result<(), String> {
    gio::spawn_blocking(move || -> Result<(), String> {
        let a = core()
            .account(id)
            .ok_or_else(|| i18n::gettext("Connecting failed"))?;
        let url = url.trim_end_matches('/').to_string();
        password_store(&url, &a.user, &password).map_err(|e| {
            i18n::gettext("Cannot save password: {error}").replace("{error}", &e.to_string())
        })?;
        if url == a.url {
            return core().connect(id, password).map_err(|e| e.to_string());
        }
        if core().connected(id) || core().pending(Some(id)) {
            core().sync_account(id).map_err(|e| upload_error(&e))?;
        }
        core()
            .move_account(id, url, password)
            .map_err(|e| e.to_string())?;
        password_clear(&a.url, &a.user);
        Ok(())
    })
    .await
    .map_err(|_| i18n::gettext("Connecting failed"))??;
    sync_now();
    Ok(())
}

/// Switch account `id` on or off (off hides its notes and stops syncing them).
pub async fn enable_account(id: i64, on: bool) {
    let _ = gio::spawn_blocking(move || core().set_account_enabled(id, on)).await;
    window::account_changed();
    if on {
        sync_now();
    }
}

/// Remove account `id` from this device, with its saved password; edits waiting for its server
/// go up first, unless `force` (then they are lost).
pub async fn remove_account(id: i64, force: bool) -> Result<(), String> {
    window::save();
    gio::spawn_blocking(move || -> Result<(), String> {
        let Some(a) = core().account(id) else {
            return Ok(());
        };
        if a.kind == AccountKind::Nextcloud && !force {
            if core().connected(id) {
                core().sync_account(id).map_err(|e| upload_error(&e))?;
            } else if core().pending(Some(id)) {
                return Err(i18n::gettext(
                    "It has changes not uploaded yet: sign in again to upload them",
                ));
            }
        }
        if a.kind == AccountKind::Nextcloud && a.source.is_empty() {
            password_clear(&a.url, &a.user);
        }
        core().remove_account(id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| i18n::gettext("Removing the account failed"))??;
    if core().accounts().is_empty() {
        window::show_welcome();
    } else {
        window::account_changed();
    }
    Ok(())
}

// --- background --------------------------------------------------------------

/// Start the sync worker and the polling timer, or sync if they run. Idempotent.
pub fn start_background() {
    if SYNC.get().is_some() {
        sync_now();
        return;
    }
    let (tx, rx) = channel::<()>();
    let _ = SYNC.set(tx);
    std::thread::spawn(move || {
        while rx.recv().is_ok() {
            // Let a burst of edits settle: typing saves every second or so.
            std::thread::sleep(Duration::from_millis(1500));
            while rx.try_recv().is_ok() {}
            if !core().accounts().iter().any(|a| core().syncing(a.id)) {
                continue; // nothing to sync until an account connects
            }
            post(Event::SyncState("syncing".into()));
            sync_all();
        }
    });
    sync_now();

    // Changes from other devices: every 30 s while the window is focused (each notes folder's
    // ETag, one request when nothing changed), and when it gets the focus.
    // TODO(v2): Nextcloud's Client Push (notify_push) instead, where the server has it.
    glib::timeout_add_seconds_local(30, || {
        if window::is_active() {
            sync_now();
        }
        glib::ControlFlow::Continue
    });
}

/// The data could not be opened (left by a newer Notav, say): a window saying so, and nothing
/// else runs; closing it quits.
fn open_failed(e: &notav_core::Error) -> glib::ExitCode {
    eprintln!("open local database: {e}");
    let description = match e {
        notav_core::Error::Newer(_) => i18n::gettext(
            "It was saved by a newer version of Notav. Update Notav to open it; this version leaves it as it is.",
        ),
        e => e.to_string(),
    };
    let app = adw::Application::new(Some(APP_ID), gio::ApplicationFlags::NON_UNIQUE);
    app.connect_activate(move |app| {
        let page = adw::StatusPage::builder()
            .icon_name("dialog-error-symbolic")
            .title(i18n::gettext("Notav Cannot Open Its Data"))
            .description(description.as_str())
            .build();
        let view = adw::ToolbarView::new();
        view.add_top_bar(&adw::HeaderBar::new());
        view.set_content(Some(&page));
        adw::ApplicationWindow::builder()
            .application(app)
            .title("Notav")
            .default_width(480)
            .default_height(360)
            .content(&view)
            .build()
            .present();
    });
    app.run_with_args::<&str>(&[])
}

fn main() -> glib::ExitCode {
    i18n::init();
    gio::resources_register_include!("icons.gresource").expect("register icons");
    let data = glib::user_data_dir().join("notav");
    let client = match Client::open(data.to_string_lossy().into()) {
        Ok(c) => c,
        Err(e) => return open_failed(&e),
    };
    client.subscribe(Box::new(Listener));
    let _ = CORE.set(Arc::new(client));

    let flags = if sandbox() {
        gio::ApplicationFlags::NON_UNIQUE
    } else {
        // A restart after an update starts the new version before this one has quit (Flatpak).
        gio::ApplicationFlags::ALLOW_REPLACEMENT
    };
    let app = adw::Application::new(Some(APP_ID), flags);
    app.connect_startup(|app| {
        sourceview5::init();
        // The editor's Markdown and style schemes, bundled with the icons.
        sourceview5::LanguageManager::default()
            .append_search_path("resource:///dev/aperte/Notav/sourceview/");
        sourceview5::StyleSchemeManager::default()
            .append_search_path("resource:///dev/aperte/Notav/sourceview/");
        window::setup_actions(app);
        settings::apply_color_scheme();
        if restore_accounts() {
            start_background();
        }
        update::watch(app);
    });
    app.connect_activate(window::present);
    let code = app.run();
    update::exec_if_restarting();
    code
}
