//! Notav for Linux: the GTK app, with a sync worker while it runs. There is no background
//! service: closing the window quits, once what is waiting for the server has gone up.

mod connection;
mod editor;
mod goa;
mod i18n;
mod settings;
mod window;

use adw::prelude::*;
use gtk::{gio, glib};
use notav_core::{ChangeListener, Client};
use std::cell::Cell;
use std::collections::HashMap;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

pub const APP_ID: &str = "dev.aperte.Notav";

static CORE: OnceLock<Arc<Client>> = OnceLock::new();
static SYNC: OnceLock<Sender<()>> = OnceLock::new();
static SYNC_STATE: Mutex<String> = Mutex::new(String::new());

thread_local! {
    /// The window was closed with changes waiting for the server: quit after the next sync.
    static QUIT_PENDING: Cell<bool> = const { Cell::new(false) };
}

pub fn core() -> &'static Arc<Client> {
    CORE.get().expect("core opened at startup")
}

/// After an edit: sync if it left something for the server.
pub fn request_sync() {
    if core().pending() {
        sync_now();
    }
}

/// Ask the worker for a full sync; requests arriving while one runs are coalesced.
pub fn sync_now() {
    if let Some(tx) = SYNC.get() {
        let _ = tx.send(());
    }
}

/// Connected to the account (restored at start or signed in to), with its password.
static CONNECTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn connected() -> bool {
    CONNECTED.load(std::sync::atomic::Ordering::SeqCst) && core().account().is_some()
}

/// The main window has something to show: an account, and the notes folder chosen for it.
pub fn ready() -> bool {
    connected() && core().setting("notes_folder".into()).is_some()
}

pub fn sync_state() -> String {
    SYNC_STATE.lock().unwrap().clone()
}

/// Events from background threads, handled on the main thread.
#[derive(Debug, Clone)]
pub enum Event {
    /// Notes changed, by path.
    Changed(Vec<String>),
    /// Settings from another device arrived.
    SettingsChanged,
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
    });
}

fn set_state(r: &Result<notav_core::SyncReport, notav_core::Error>) {
    if let Some(report) = r.as_ref().ok().filter(|r| !r.conflicts.is_empty()) {
        post(Event::Conflicts(report.conflicts.clone()));
    }
    if let Ok(report) = r {
        post(Event::Locked(report.locked.clone()));
    }
    post(Event::SyncState(match r {
        Ok(_) => "idle".into(),
        Err(e) => format!("error:{e}"),
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
    if SYNC.get().is_some() && connected() && core().pending() {
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

// --- account -----------------------------------------------------------------

/// The GNOME Online Accounts account Notav connects through; None means signed in here.
pub fn goa_id() -> Option<String> {
    core()
        .setting("goa_account".into())
        .filter(|id| !id.is_empty())
}

/// Restore the account, its password from GOA or the keyring; false → the UI shows the
/// welcome page (also when the GOA account is gone or has files turned off).
pub fn restore_account() -> bool {
    let Some(acc) = core().account() else {
        return false;
    };
    let (url, user, pw) = match goa_id() {
        Some(id) => match goa::accounts().into_iter().find(|a| a.id == id) {
            Some(a) => (a.url, a.user, goa::password(&id)),
            None => return false,
        },
        None => (
            acc.url.clone(),
            acc.user.clone(),
            password_load(&acc.url, &acc.user),
        ),
    };
    let Some(pw) = pw else {
        return false;
    };
    let ok = core().set_account(url, user, pw).is_ok();
    CONNECTED.store(ok, std::sync::atomic::Ordering::SeqCst);
    ok
}

/// (server, user) of the last browser sign-in.
fn remembered() -> Option<(String, String)> {
    let get = |k: &str| core().setting(k.into()).filter(|v| !v.is_empty());
    Some((get("manual_url")?, get("manual_user")?))
}

/// The last browser sign-in, while its password is still saved: where turning GOA off goes.
pub fn remembered_sign_in() -> Option<notav_core::Credentials> {
    let (server, login_name) = remembered()?;
    let app_password = password_load(&server, &login_name)?;
    Some(notav_core::Credentials {
        server,
        login_name,
        app_password,
    })
}

/// Forget the browser sign-in (Sign Out): its saved password goes too.
pub fn forget_sign_in() {
    if let Some((u, n)) = remembered() {
        password_clear(&u, &n);
    }
    let _ = core().set_setting("manual_url".into(), String::new());
    let _ = core().set_setting("manual_user".into(), String::new());
}

/// Why the account cannot change yet: edits not on the server would be lost with the local copy.
fn pending_error(e: &notav_core::Error) -> String {
    i18n::gettext("Cannot upload pending changes first: {error}").replace("{error}", &e.to_string())
}

/// Stop using the current account and drop the local copy; the window shows the welcome page.
/// Pending edits are uploaded first, unless `force` (then they are lost when that fails).
pub async fn disconnect(force: bool) -> Result<(), String> {
    gio::spawn_blocking(move || -> Result<(), String> {
        if core().account().is_some()
            && let Err(e) = core().sync()
            && !force
        {
            return Err(pending_error(&e));
        }
        let _ = core().set_setting("goa_account".into(), String::new());
        CONNECTED.store(false, std::sync::atomic::Ordering::SeqCst);
        core().logout().map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| i18n::gettext("Disconnecting failed"))??;
    window::show_welcome();
    Ok(())
}

/// An account to connect with.
pub enum Target {
    Goa(goa::Account),
    /// Signed in here (browser login); the app password goes into Notav's keyring entry.
    Manual(notav_core::Credentials),
}

/// Connect with `target`, replacing the current account (its pending edits are uploaded
/// first). The caller starts syncing (`start_background`) once the notes folder is known.
pub async fn switch_account(target: Target) -> Result<(), String> {
    gio::spawn_blocking(move || -> Result<(), String> {
        let (url, user, pw, goa) = match target {
            Target::Goa(a) => {
                let pw = goa::password(&a.id).ok_or_else(|| {
                    i18n::gettext("Online Accounts has no password for this account")
                })?;
                (a.url, a.user, pw, a.id)
            }
            Target::Manual(c) => (c.server, c.login_name, c.app_password, String::new()),
        };
        if let Some(old) = core().account().filter(|_| SYNC.get().is_some())
            && (old.url.as_str(), old.user.as_str()) != (url.as_str(), user.as_str())
        {
            // A different account drops the local copy: pending edits must reach the old one.
            core().sync().map_err(|e| pending_error(&e))?;
        }
        // The browser sign-in is remembered while GOA is used, to go back to it.
        if goa.is_empty() {
            if let Some((u, n)) = remembered().filter(|r| *r != (url.clone(), user.clone())) {
                password_clear(&u, &n);
            }
            password_store(&url, &user, &pw).map_err(|e| {
                i18n::gettext("Cannot save password: {error}").replace("{error}", &e.to_string())
            })?;
            let _ = core().set_setting("manual_url".into(), url.clone());
            let _ = core().set_setting("manual_user".into(), user.clone());
        }
        core()
            .set_account(url, user, pw)
            .map_err(|e| e.to_string())?;
        CONNECTED.store(true, std::sync::atomic::Ordering::SeqCst);
        core()
            .set_setting("goa_account".into(), goa)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| i18n::gettext("Connecting failed"))??;
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
            if core().account().is_none() {
                continue; // disconnected: nothing to sync until the next account
            }
            post(Event::SyncState("syncing".into()));
            set_state(&core().sync());
        }
    });
    sync_now();

    // Changes from other devices: every 30 s while the window is focused (the notes folder's
    // ETag, one request when nothing changed), and when it gets the focus.
    // TODO(v2): Nextcloud's Client Push (notify_push) instead, where the server has it.
    glib::timeout_add_seconds_local(30, || {
        if window::is_active() {
            sync_now();
        }
        glib::ControlFlow::Continue
    });
}

fn main() -> glib::ExitCode {
    i18n::init();
    gio::resources_register_include!("icons.gresource").expect("register icons");
    let data = glib::user_data_dir().join("notav");
    let client = Client::open(data.to_string_lossy().into()).expect("open local database");
    client.subscribe(Box::new(Listener));
    let _ = CORE.set(Arc::new(client));

    let flags = if sandbox() {
        gio::ApplicationFlags::NON_UNIQUE
    } else {
        gio::ApplicationFlags::empty()
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
        if restore_account() && core().setting("notes_folder".into()).is_some() {
            start_background();
        }
    });
    app.connect_activate(window::present);
    app.run()
}
