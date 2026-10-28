//! Main window: the welcome page (sign-in, then the notes folder), the notes sidebar and the
//! editor.

use crate::i18n::{gettext, gettext_noop, ngettext};
use crate::{Event, core, request_sync};
use adw::prelude::*;
use gtk::{gio, glib};
use notav_core::Note;
use sourceview5::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

/// The title above the text is an entry that looks like a heading.
/// While a note is dragged, the folder it would go to is tinted (`.drop-target`; the tree
/// itself for the top level) instead of GTK's frame around the row under the pointer; the
/// dragged note's row fades. The dense tree (`.dense`) has shorter rows without gaps, as
/// Nautilus' list view.
const CSS: &str = "
.note-title { font-size: 20pt; font-weight: 800; }
.note-text { line-height: 1.4; }
.navigation-sidebar > row:drop(active), scrolledwindow:drop(active) { box-shadow: none; }
.navigation-sidebar > row.drop-target { background-color: alpha(var(--accent-bg-color), 0.2); }
.navigation-sidebar.drop-target { background-color: alpha(var(--accent-bg-color), 0.12); }
.navigation-sidebar > row.dragged { opacity: 0.5; }
.navigation-sidebar.dense > row { min-height: 28px; margin-bottom: 0; border-radius: 6px; }
.drag-card { padding: 6px 12px; border-radius: 6px; background-color: var(--popover-bg-color);
  box-shadow: 0 1px 4px alpha(black, 0.3); }
";

/// How far each folder level is indented in the tree, and the width of a folder's arrow.
const INDENT: i32 = 16;

/// What is being dragged in the tree.
#[derive(Debug, Clone, PartialEq)]
enum Dragged {
    Note(Note),
    Folder(String),
}

/// A row of the sidebar's tree.
#[derive(Debug, Clone, PartialEq)]
enum Item {
    Folder(String),
    Note(Note),
}

/// How long typing pauses before the text is saved (and the sync asked for).
const SAVE_AFTER: std::time::Duration = std::time::Duration::from_millis(800);

/// Where to support development, listed in About (and in the README), as in Todav.
const DONATIONS: [(&str, &str); 3] = [
    (
        gettext_noop("Support on Ko-fi"),
        "https://ko-fi.com/N5E229GJPZ",
    ),
    (
        gettext_noop("Sponsor on GitHub"),
        "https://github.com/sponsors/SelfRef",
    ),
    (
        gettext_noop("Donate on Liberapay"),
        "https://liberapay.com/SelfRef/donate",
    ),
];

struct Ui {
    window: adw::ApplicationWindow,
    toasts: adw::ToastOverlay,
    stack: gtk::Stack,
    split: adw::NavigationSplitView,
    /// Welcome page: how to connect, the GOA account or a server, then the notes folder.
    welcome: adw::NavigationView,
    conn: Rc<crate::connection::Connection>,
    connect_goa: gtk::Button,
    login: adw::SplitButton,
    cancel_login: gtk::Button,
    folder: adw::EntryRow,
    folder_done: gtk::Button,
    sync_state: gtk::Stack,
    search_button: gtk::ToggleButton,
    search: gtk::SearchEntry,
    /// The notes matching the search, in the editor's place.
    results: gtk::ListBox,
    results_stack: gtk::Stack,
    found: RefCell<Vec<Note>>,
    tree: gtk::ListBox,
    tree_stack: gtk::Stack,
    tree_scroll: gtk::ScrolledWindow,
    /// The rows of `tree`, as `items`.
    rows: RefCell<Vec<gtk::ListBoxRow>>,
    /// The note or folder being dragged in the tree.
    dragging: RefCell<Option<Dragged>>,

    /// The items of `tree`'s rows, in order.
    items: RefCell<Vec<Item>>,
    /// Folders shown open in the tree (the device-only `expanded_folders` setting).
    expanded: RefCell<HashSet<String>>,
    /// Where a new note goes: the folder last opened in the tree, or the open note's.
    target_folder: RefCell<String>,
    note_title: adw::WindowTitle,
    note_stack: gtk::Stack,
    title: gtk::Entry,
    note_scroll: gtk::ScrolledWindow,
    note_clamp: adw::Clamp,
    note_box: gtk::Box,
    note_mode: adw::ToggleGroup,
    zoom_reset: gtk::Button,
    /// The note's font, size and width (`apply_text_look`).
    text_css: gtk::CssProvider,
    buffer: sourceview5::Buffer,
    editor: sourceview5::View,
    current: RefCell<Option<Note>>,
    /// The editor holds text not given to the core yet.
    unsaved: Cell<bool>,
    /// The note is styled again before it is drawn next (`restyle`).
    restyle_pending: Cell<bool>,
    save_timer: RefCell<Option<glib::SourceId>>,
    /// Set while code replaces the editor's text or the list's rows.
    loading: Cell<bool>,
    last_error: RefCell<String>,
    /// The locks last told about, so each sync (every 30 s) does not repeat them.
    last_locked: RefCell<Vec<notav_core::Locked>>,
}

thread_local! {
    static UI: RefCell<Option<Rc<Ui>>> = const { RefCell::new(None) };
}

fn ui() -> Option<Rc<Ui>> {
    UI.with(|u| u.borrow().clone())
}

pub fn is_active() -> bool {
    ui().is_some_and(|u| u.window.is_visible() && u.window.is_active())
}

pub fn is_visible() -> bool {
    ui().is_some_and(|u| u.window.is_visible())
}

pub fn setup_actions(app: &adw::Application) {
    let sync = gio::SimpleAction::new("sync", None);
    sync.connect_activate(|_, _| crate::sync_now());
    // As closing the window: quits once what waits for the server is up.
    let quit = gio::SimpleAction::new("quit", None);
    quit.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| match ui() {
            Some(ui) => ui.window.close(),
            None => app.quit(),
        }
    ));
    let shortcuts = gio::SimpleAction::new("shortcuts", None);
    shortcuts.connect_activate(|_, _| {
        if let Some(ui) = ui() {
            get::<adw::Dialog>(&view!("shortcuts"), "dialog").present(Some(&ui.window));
        }
    });
    // Filled from the metainfo file (bundled, see icons/icons.gresource.xml).
    let about = gio::SimpleAction::new("about", None);
    about.connect_activate(|_, _| {
        let Some(ui) = ui() else { return };
        let d = adw::AboutDialog::from_appdata(
            "/dev/aperte/Notav/metainfo.xml",
            Some(env!("CARGO_PKG_VERSION")),
        );
        d.set_copyright("© 2026 SelfRef");
        about_support(&d);
        d.present(Some(&ui.window));
    });
    for a in [&sync, &quit, &shortcuts, &about] {
        app.add_action(a);
    }
    // Listed in ui/shortcuts.blp.
    app.set_accels_for_action("app.sync", &["F5"]);
    app.set_accels_for_action("app.quit", &["<Control>q"]);
    app.set_accels_for_action("app.shortcuts", &["<Control>question"]);
    app.set_accels_for_action("window.close", &["<Control>w"]);
    app.set_accels_for_action("win.preferences", &["<Control>comma"]);
    app.set_accels_for_action("win.new-note", &["<Control>n"]);
    app.set_accels_for_action("win.search", &["<Control>f"]);
    app.set_accels_for_action("win.undo", &["<Control>z"]);
    app.set_accels_for_action(
        "win.zoom-in",
        &["<Control>plus", "<Control>equal", "<Control>KP_Add"],
    );
    app.set_accels_for_action("win.zoom-out", &["<Control>minus", "<Control>KP_Subtract"]);
    app.set_accels_for_action("win.zoom-reset", &["<Control>0", "<Control>KP_0"]);
}

/// Turns About's Details page into Support: a note and the donation links instead of the
/// description, with the website left on the main page. AdwAboutDialog has no API for that, so
/// this finds its rows (the same in libadwaita 1.8 to 1.10); if they move, About keeps a Details
/// page with the note and the links.
fn about_support(d: &adw::AboutDialog) {
    d.set_comments(&glib::markup_escape_text(&gettext(
        "Notav is developed in my free time. If it keeps your notes in order, you can support its \
         development.",
    )));
    let titles: Vec<String> = DONATIONS.iter().map(|(t, _)| gettext(t)).collect();
    for ((_, uri), title) in DONATIONS.iter().zip(&titles) {
        d.add_link(title, uri);
    }
    // Its content: the dialog itself has no children until presented.
    let Some(content) = d.child() else { return };
    let widgets = descendants(&content);
    // The row that opens the page, followed by the website's own (hidden with a Details page).
    let Some(row) = widgets
        .iter()
        .filter_map(|w| w.downcast_ref::<adw::ActionRow>())
        .find(|r| {
            r.action_name().as_deref() == Some("navigation.push")
                && r.action_target_value()
                    .is_some_and(|v| v.str() == Some("details"))
        })
    else {
        return;
    };
    let Some(page) = widgets
        .iter()
        .find_map(|w| w.downcast_ref::<adw::NavigationView>())
        .and_then(|v| v.find_page("details"))
    else {
        return;
    };
    // Translators: the About row that opens the page with the donation links.
    row.set_title(&gettext("_Support"));
    // Translators: title of About's page with the donation links.
    page.set_title(&gettext("Support"));
    if let Some(website) = row.next_sibling() {
        website.set_visible(true);
    }
    for r in descendants(page.upcast_ref()) {
        if let Some(r) = r.downcast_ref::<adw::PreferencesRow>()
            && !titles.iter().any(|t| *t == r.title())
        {
            r.set_visible(false);
        }
    }
}

/// `w` and every widget inside it.
fn descendants(w: &gtk::Widget) -> Vec<gtk::Widget> {
    let mut all = vec![w.clone()];
    let mut i = 0;
    while let Some(w) = all.get(i).cloned() {
        let mut child = w.first_child();
        while let Some(c) = child {
            child = c.next_sibling();
            all.push(c);
        }
        i += 1;
    }
    all
}

pub fn present(app: &adw::Application) {
    let ui = ui().unwrap_or_else(|| {
        let ui = build(app);
        UI.with(|u| *u.borrow_mut() = Some(ui.clone()));
        ui
    });
    crate::stay();
    if crate::ready() {
        ui.stack.set_visible_child_name("main");
        refresh(&ui);
    } else {
        ui.stack.set_visible_child_name("welcome");
        // Connected before, the folder not chosen yet (the app quit on that step).
        if crate::connected() && ui.welcome.visible_page_tag().as_deref() != Some("folder") {
            show_folder_step(&ui);
        }
    }
    ui.window.present();
}

/// Disconnected: back to the welcome page, at its first step.
pub fn show_welcome() {
    if let Some(ui) = ui() {
        close_note(&ui);
        ui.welcome.pop_to_tag("choose");
        ui.stack.set_visible_child_name("welcome");
        refresh(&ui); // empties the tree
    }
}

/// The account was replaced (from Preferences): show its notes once they arrive.
pub fn account_changed() {
    if let Some(ui) = ui() {
        close_note(&ui);
        ui.stack.set_visible_child_name("main");
        refresh(&ui);
    }
}

/// Save what the editor holds now (before the notes folder changes, say).
pub fn save() {
    if let Some(ui) = ui() {
        save_now(&ui);
    }
}

pub fn toast(msg: &str) {
    if let Some(ui) = ui() {
        ui.toasts.add_toast(note(msg));
    }
}

/// Tell what was done, with a button undoing just that (the latest change).
fn undo_toast(ui: &Ui, title: &str) {
    let t = note(title);
    t.set_priority(adw::ToastPriority::High);
    t.set_button_label(Some(&gettext("Undo")));
    let change = core().last_change();
    t.connect_button_clicked(move |_| undo(change));
    ui.toasts.add_toast(t);
}

/// Undo `change`, else the latest one.
fn undo(change: Option<u64>) {
    let Some(ui) = ui() else { return };
    match core().undo(change) {
        Ok(true) => {
            refresh(&ui);
            reload_current(&ui);
            request_sync();
        }
        Ok(false) => ui.window.error_bell(),
        Err(e) => toast(&e.to_string()),
    }
}

/// A toast showing `text` as it is: a toast title is markup otherwise, and errors carry `<`.
pub fn note(text: &str) -> adw::Toast {
    let t = adw::Toast::new(text);
    t.set_use_markup(false);
    t
}

pub fn on_event(ev: &Event) {
    let Some(ui) = ui() else { return };
    match ev {
        Event::Changed(_) | Event::SettingsChanged => {
            // Rebuild outside the signal handler that caused the change.
            glib::idle_add_local_once(move || {
                refresh(&ui);
                reload_current(&ui);
            });
        }
        Event::SyncState(s) => {
            ui.sync_state
                .set_visible_child_name(if s == "syncing" { "syncing" } else { "idle" });
            if let Some(err) = s.strip_prefix("error:") {
                if *ui.last_error.borrow() != err {
                    eprintln!("Sync failed: {err}");
                    ui.toasts.add_toast(note(
                        &gettext("Sync failed: {error}").replace("{error}", err),
                    ));
                    *ui.last_error.borrow_mut() = err.to_string();
                }
            } else if s == "idle" {
                ui.last_error.borrow_mut().clear();
            }
        }
        Event::Locked(locks) => {
            let told = ui.last_locked.replace(locks.clone());
            for l in locks.iter().filter(|l| !told.contains(l)) {
                locked_toast(&ui, l);
            }
        }
        Event::Conflicts(copies) => {
            let msg = match copies.as_slice() {
                [one] => gettext(
                    "“{title}” was changed elsewhere too: your version is kept next to it as a copy",
                )
                .replace("{title}", &title_of(one)),
                _ => ngettext(
                    "{n} note was changed elsewhere too: your version is kept next to it as a copy",
                    "{n} notes were changed elsewhere too: your versions are kept next to them as copies",
                    copies.len() as u64,
                )
                .replace("{n}", &copies.len().to_string()),
            };
            let t = note(&msg);
            t.set_priority(adw::ToastPriority::High);
            t.set_timeout(0);
            ui.toasts.add_toast(t);
        }
    }
}

/// Tell that a change waits for a lock on the server: who holds it and since when, with an
/// Unlock button for a note's.
fn locked_toast(ui: &Rc<Ui>, l: &notav_core::Locked) {
    let since = l.since.and_then(|t| {
        let d = glib::DateTime::from_unix_local(t).ok()?;
        Some(format!(
            "{} {}",
            d.format("%x").ok()?,
            d.format("%H:%M").ok()?
        ))
    });
    let msg = match (&l.file, &l.owner, &since) {
        (None, _, _) => gettext("Something in “{title}” is locked on the server"),
        (Some(_), Some(_), Some(_)) => {
            gettext("“{title}” is locked on the server by {owner} since {time}")
        }
        (Some(_), Some(_), None) => gettext("“{title}” is locked on the server by {owner}"),
        (Some(_), None, _) => gettext("“{title}” is locked on the server"),
    }
    .replace("{title}", &title_of(&l.path))
    .replace("{owner}", l.owner.as_deref().unwrap_or_default())
    .replace("{time}", since.as_deref().unwrap_or_default());
    let t = note(&msg);
    t.set_timeout(0);
    if let Some(file) = l.file.clone() {
        t.set_button_label(Some(&gettext("Unlock…")));
        let (ui, title) = (ui.clone(), title_of(&l.path));
        t.connect_button_clicked(move |_| unlock(&ui, file.clone(), title.clone()));
    }
    ui.toasts.add_toast(t);
}

/// Remove a lock on the server, after asking: an editor that still has the note open may lose
/// what it saves afterwards.
fn unlock(ui: &Rc<Ui>, file: String, title: String) {
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let alert = adw::AlertDialog::new(
            Some(&gettext("Unlock “{title}”?").replace("{title}", &title)),
            Some(&gettext(
                "Your change to it waits for the lock. If it is still open in an editor, what is saved there afterwards may be lost: unlock it only if nobody is editing it.",
            )),
        );
        alert.add_responses(&[
            ("cancel", &gettext("Cancel")),
            ("unlock", &gettext("Unlock")),
        ]);
        alert.set_response_appearance("unlock", adw::ResponseAppearance::Destructive);
        alert.set_default_response(Some("cancel"));
        alert.set_close_response("cancel");
        if alert.choose_future(Some(&ui.window)).await != "unlock" {
            return;
        }
        match gio::spawn_blocking(move || core().unlock(file)).await {
            Ok(Ok(())) => {
                ui.last_locked.borrow_mut().clear();
                crate::sync_now();
            }
            Ok(Err(e)) => toast(
                &gettext("Cannot unlock “{title}”: {error}")
                    .replace("{title}", &title)
                    .replace("{error}", &e.to_string()),
            ),
            Err(_) => {}
        }
    });
}

/// A note's title from its path: the file name without its extension.
fn title_of(path: &str) -> String {
    let file = path.rsplit('/').next().unwrap_or(path);
    file.rsplit_once('.').map_or(file, |(t, _)| t).to_string()
}

/// Loads a view compiled from `ui/<name>.blp` by build.rs.
macro_rules! view {
    ($name:literal) => {
        gtk::Builder::from_string(include_str!(concat!(env!("OUT_DIR"), "/", $name, ".ui")))
    };
}
pub(crate) use view;

pub fn get<T: IsA<glib::Object>>(b: &gtk::Builder, id: &str) -> T {
    b.object(id).unwrap_or_else(|| panic!("no `{id}` in view"))
}

/// Restores the last window size; saves it, and what the editor holds, when the window closes.
/// Closing hides the window and quits once what waits for the server is up. GTK keeps the
/// unmaximized size in default-width/height.
fn restore_size(window: &adw::ApplicationWindow, app: &adw::Application) {
    let size = core().setting("window_size".into()).unwrap_or_default();
    if let Some((w, h)) = size.split_once('x')
        && let (Ok(w), Ok(h)) = (w.parse(), h.parse())
    {
        window.set_default_size(w, h);
    }
    window.set_maximized(core().setting("window_maximized".into()).as_deref() == Some("true"));
    window.connect_close_request(glib::clone!(
        #[weak]
        app,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |w| {
            let (width, height) = w.default_size();
            let _ = core().set_setting("window_size".into(), format!("{width}x{height}"));
            let _ = core().set_setting("window_maximized".into(), w.is_maximized().to_string());
            save();
            crate::quit_after_sync(&app);
            glib::Propagation::Proceed
        }
    ));
}

fn build(app: &adw::Application) -> Rc<Ui> {
    let b = view!("window");
    let window: adw::ApplicationWindow = get(&b, "window");
    window.set_application(Some(app));
    if crate::sandbox() {
        window.set_title(Some("Notav (sandbox)"));
    }
    restore_size(&window, app);

    let css = gtk::CssProvider::new();
    css.load_from_string(CSS);
    gtk::style_context_add_provider_for_display(
        &WidgetExt::display(&window),
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let editor: sourceview5::View = get(&b, "editor");
    let buffer = editor.buffer().downcast::<sourceview5::Buffer>().unwrap();
    buffer.set_language(
        sourceview5::LanguageManager::default()
            .language("notav-markdown")
            .as_ref(),
    );
    crate::editor::setup(&buffer);
    crate::editor::connect(&editor);
    follow_style(&buffer);

    let ui = Rc::new(Ui {
        window,
        toasts: get(&b, "toasts"),
        stack: get(&b, "stack"),
        split: get(&b, "split"),
        welcome: get(&b, "welcome"),
        conn: crate::connection::Connection::new(true, None),
        connect_goa: get(&b, "connect_goa"),
        login: get(&b, "login"),
        cancel_login: get(&b, "cancel_login"),
        folder: get(&b, "folder"),
        folder_done: get(&b, "folder_done"),
        sync_state: get(&b, "sync_state"),
        search_button: get(&b, "search_button"),
        search: get(&b, "search"),
        results: get(&b, "results"),
        results_stack: get(&b, "results_stack"),
        found: RefCell::new(Vec::new()),
        tree: get(&b, "tree"),
        tree_stack: get(&b, "tree_stack"),
        tree_scroll: get(&b, "tree_scroll"),
        rows: RefCell::new(Vec::new()),
        dragging: RefCell::new(None),
        items: RefCell::new(Vec::new()),
        expanded: RefCell::new(
            core()
                .setting("expanded_folders".into())
                .unwrap_or_default()
                .lines()
                .map(str::to_string)
                .collect(),
        ),
        target_folder: RefCell::new(String::new()),
        note_title: get(&b, "note_title"),
        note_stack: get(&b, "note_stack"),
        title: get(&b, "title"),
        note_scroll: get(&b, "note_scroll"),
        note_clamp: get(&b, "note_clamp"),
        note_box: get(&b, "note_box"),
        note_mode: get(&b, "note_mode"),
        zoom_reset: get(&b, "zoom_reset"),
        text_css: gtk::CssProvider::new(),
        buffer,
        editor,
        current: RefCell::new(None),
        unsaved: Cell::new(false),
        restyle_pending: Cell::new(false),
        save_timer: RefCell::new(None),
        loading: Cell::new(false),
        last_error: RefCell::new(String::new()),
        last_locked: RefCell::new(Vec::new()),
    });
    ui.conn.attach(&get(&b, "login_group"));
    let zoom: gtk::Box = get(&b, "zoom_box");
    if zoom.parent().is_none() {
        get::<gtk::PopoverMenu>(&b, "view_popover").add_child(&zoom, "zoom");
    }
    // Over `CSS`, which it overrides.
    gtk::style_context_add_provider_for_display(
        &WidgetExt::display(&ui.window),
        &ui.text_css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
    );
    apply_text_look(&ui);
    close_note(&ui);
    connect(&ui);
    connect_welcome(&ui, &b);
    ui
}

/// The device-only `text_zoom` setting: the note's text size in percent, 50 to 300.
fn text_zoom() -> i32 {
    core()
        .setting("text_zoom".into())
        .and_then(|z| z.parse().ok())
        .unwrap_or(100)
        .clamp(50, 300)
}

/// The device-only `full_width` setting: the text across the window, without margins.
fn full_width() -> bool {
    core().setting("full_width".into()).as_deref() == Some("true")
}

/// The note's text again after a setting of its look changed (Preferences).
pub fn text_look_changed() {
    if let Some(ui) = ui() {
        apply_text_look(&ui);
    }
}

/// The note's text in the font from Preferences (the device-only `editor_font`; the
/// interface's at 12 pt without one), at the size `text_zoom` scales it to, as wide as
/// `full_width` says.
fn apply_text_look(ui: &Ui) {
    let zoom = text_zoom();
    let mut css = String::from(".note-text {");
    let mut size = 12.0;
    if let Some(font) = crate::settings::editor_font() {
        let set = font.set_fields();
        if let Some(family) = font
            .family()
            .filter(|_| set.contains(gtk::pango::FontMask::FAMILY))
        {
            css.push_str(&format!(" font-family: \"{}\";", family.replace('"', "")));
        }
        if font.size() > 0 {
            size = font.size() as f64 / gtk::pango::SCALE as f64;
            if font.is_size_absolute() {
                size *= 0.75; // pixels to points
            }
        }
        if set.contains(gtk::pango::FontMask::WEIGHT) {
            use glib::translate::IntoGlib;
            css.push_str(&format!(" font-weight: {};", font.weight().into_glib()));
        }
        if font.style() == gtk::pango::Style::Italic {
            css.push_str(" font-style: italic;");
        }
    }
    css.push_str(&format!(
        " font-size: {:.1}pt; }}",
        size * zoom as f64 / 100.0
    ));
    ui.text_css.load_from_string(&css);
    // Translators: the note's text size in the view menu, as "100%".
    let percent = gettext("{percent}%");
    ui.zoom_reset
        .set_label(&percent.replace("{percent}", &zoom.to_string()));
    let full = full_width();
    // No limit: wider than any window.
    ui.note_clamp
        .set_maximum_size(if full { 100_000 } else { 720 });
    ui.note_clamp
        .set_tightening_threshold(if full { 100_000 } else { 600 });
    let margin = if full { 0 } else { 12 };
    ui.note_box.set_margin_start(margin);
    ui.note_box.set_margin_end(margin);
}

/// The device-only `source_mode` setting: Source mode instead of Editor mode.
fn source_mode() -> bool {
    core().setting("source_mode".into()).as_deref() == Some("true")
}

/// Style the note again for its mode, once the changes made now are done and before it is
/// drawn: typed text does not take the styles around it.
fn restyle(ui: &Rc<Ui>) {
    if ui.restyle_pending.replace(true) {
        return;
    }
    let weak = Rc::downgrade(ui);
    glib::idle_add_local_full(glib::Priority::HIGH_IDLE, move || {
        if let Some(ui) = weak.upgrade() {
            ui.restyle_pending.set(false);
            crate::editor::restyle(&ui.buffer, source_mode());
        }
        glib::ControlFlow::Break
    });
}

/// Scroll the note to its cursor, once the text is laid out: what scrolls is the box with the
/// title and the text, so the text view cannot do it itself.
fn follow_cursor(ui: &Rc<Ui>) {
    let weak = Rc::downgrade(ui);
    glib::idle_add_local_once(move || {
        let Some(ui) = weak.upgrade() else { return };
        let (view, adj) = (&ui.editor, ui.note_scroll.vadjustment());
        let cursor = ui.buffer.iter_at_mark(&ui.buffer.get_insert());
        // On the first line, the title above it too.
        if cursor.line() == 0 {
            adj.set_value(0.0);
            return;
        }
        let at = view.iter_location(&cursor);
        let (x, y) = view.buffer_to_window_coords(gtk::TextWindowType::Widget, at.x(), at.y());
        // Where it is in what scrolls, as that is not moved until it is laid out again.
        let point = gtk::graphene::Point::new(x as f32, y as f32);
        let Some(p) = view
            .ancestor(adw::Clamp::static_type())
            .and_then(|content| view.compute_point(&content, &point))
        else {
            return;
        };
        let (top, bottom, margin) = (p.y() as f64, (p.y() + at.height() as f32) as f64, 24.0);
        if top - margin < adj.value() {
            adj.set_value(top - margin);
        } else if bottom + margin > adj.value() + adj.page_size() {
            adj.set_value(bottom + margin - adj.page_size());
        }
    });
}

/// The editor's colours follow the light or dark style: Adwaita's, for prose (`sourceview/`).
fn follow_style(buffer: &sourceview5::Buffer) {
    let apply = {
        let buffer = buffer.clone();
        move |sm: &adw::StyleManager| {
            let name = if sm.is_dark() { "notav-dark" } else { "notav" };
            buffer.set_style_scheme(
                sourceview5::StyleSchemeManager::default()
                    .scheme(name)
                    .as_ref(),
            );
            crate::editor::look(&buffer);
        }
    };
    let sm = adw::StyleManager::default();
    apply(&sm);
    sm.connect_dark_notify(apply);
}

fn connect(ui: &Rc<Ui>) {
    let add = |name: &str, f: fn(&Rc<Ui>)| {
        let a = gio::SimpleAction::new(name, None);
        let weak = Rc::downgrade(ui);
        a.connect_activate(move |_, _| {
            if let Some(ui) = weak.upgrade() {
                f(&ui);
            }
        });
        ui.window.add_action(&a);
        a
    };
    add("preferences", |ui| crate::settings::present(&ui.window));
    let new = add("new-note", new_note);
    let search = add("search", |ui| {
        ui.search_button.set_active(true);
        ui.search.grab_focus();
    });
    // From a note's menu in the tree, with the note's id.
    let delete = gio::SimpleAction::new("delete-note", Some(glib::VariantTy::INT64));
    let weak = Rc::downgrade(ui);
    delete.connect_activate(move |_, id| {
        if let (Some(ui), Some(id)) = (weak.upgrade(), id.and_then(|i| i.get::<i64>())) {
            delete_note(&ui, id);
        }
    });
    ui.window.add_action(&delete);
    // From the tree's menus, with the folder ("": the top level).
    for (name, f) in [
        ("new-note-in", new_note_in as fn(&Rc<Ui>, String)),
        ("new-folder-in", new_folder),
        ("rename-folder", rename_folder),
        ("delete-folder", delete_folder),
    ] {
        let a = gio::SimpleAction::new(name, Some(glib::VariantTy::STRING));
        let weak = Rc::downgrade(ui);
        a.connect_activate(move |_, folder| {
            if let (Some(ui), Some(folder)) = (weak.upgrade(), folder.and_then(|f| f.str())) {
                f(&ui, folder.to_string());
            }
        });
        ui.window.add_action(&a);
    }
    // Source mode or Editor mode, from the header's toggles.
    let source = gio::SimpleAction::new_stateful("source-mode", None, &source_mode().to_variant());
    let weak = Rc::downgrade(ui);
    source.connect_change_state(move |a, state| {
        let (Some(ui), Some(on)) = (weak.upgrade(), state.and_then(|s| s.get::<bool>())) else {
            return;
        };
        a.set_state(&on.to_variant());
        let _ = core().set_setting("source_mode".into(), on.to_string());
        ui.note_mode
            .set_active_name(Some(if on { "source" } else { "editor" }));
        restyle(&ui);
    });
    ui.window.add_action(&source);
    ui.note_mode
        .set_active_name(Some(if source_mode() { "source" } else { "editor" }));
    let window = ui.window.downgrade();
    ui.note_mode.connect_active_name_notify(move |g| {
        let on = g.active_name().as_deref() == Some("source");
        if let Some(w) = window.upgrade()
            && on != source_mode()
        {
            w.change_action_state("source-mode", &on.to_variant());
        }
    });
    // The text's size and width, from the view menu.
    for (name, step) in [("zoom-in", 10), ("zoom-out", -10), ("zoom-reset", 0)] {
        let a = gio::SimpleAction::new(name, None);
        let weak = Rc::downgrade(ui);
        a.connect_activate(move |_, _| {
            let Some(ui) = weak.upgrade() else { return };
            let zoom = if step == 0 { 100 } else { text_zoom() + step };
            let _ = core().set_setting("text_zoom".into(), zoom.clamp(50, 300).to_string());
            apply_text_look(&ui);
        });
        ui.window.add_action(&a);
    }
    let full = gio::SimpleAction::new_stateful("full-width", None, &full_width().to_variant());
    let weak = Rc::downgrade(ui);
    full.connect_change_state(move |a, state| {
        let (Some(ui), Some(on)) = (weak.upgrade(), state.and_then(|s| s.get::<bool>())) else {
            return;
        };
        a.set_state(&on.to_variant());
        let _ = core().set_setting("full_width".into(), on.to_string());
        apply_text_look(&ui);
    });
    ui.window.add_action(&full);
    // Undo the latest change; in a text field, the typing there (as in Todav).
    let window = ui.window.downgrade();
    let undo_last = gio::SimpleAction::new("undo", None);
    undo_last.connect_activate(move |_, _| {
        let Some(w) = window.upgrade() else { return };
        match gtk::prelude::RootExt::focus(&w)
            .filter(|f| f.is::<gtk::Text>() || f.is::<gtk::TextView>())
        {
            Some(text) => {
                let _ = text.activate_action("text.undo", None);
            }
            None if w.visible_dialog().is_none() => undo(None),
            None => {}
        }
    });
    ui.window.add_action(&undo_last);
    // Only while the notes are shown.
    let enable = {
        let ui = Rc::downgrade(ui);
        move || {
            let Some(ui) = ui.upgrade() else { return };
            let main = ui.stack.visible_child_name().as_deref() == Some("main");
            new.set_enabled(main);
            search.set_enabled(main);
            undo_last.set_enabled(main);
        }
    };
    enable();
    ui.stack
        .connect_visible_child_name_notify(move |_| enable());
    context_menu(ui);
    connect_drop(ui);

    // Shrinking keeps the open note.
    ui.split.connect_collapsed_notify(|s| {
        if s.is_collapsed() {
            s.set_show_content(true);
        }
    });

    // A folder opens or closes, a note opens; after the click, as that rebuilds the tree.
    let weak = Rc::downgrade(ui);
    ui.tree.connect_row_activated(move |_, row| {
        let Some(ui) = weak.upgrade() else { return };
        let item = ui.items.borrow().get(row.index() as usize).cloned();
        glib::idle_add_local_once(move || match item {
            // Folder rows are not activatable: they have their own keys and clicks.
            Some(Item::Folder(_)) => {}
            Some(Item::Note(n)) => {
                ui.search_button.set_active(false);
                open_note(&ui, &n);
                ui.split.set_show_content(true);
            }
            None => {}
        });
    });

    // Enter or Space on a folder opens or closes it.
    let keys = gtk::EventControllerKey::new();
    let weak = Rc::downgrade(ui);
    keys.connect_key_pressed(move |_, key, _, _| {
        use gtk::gdk::Key;
        let Some(ui) = weak.upgrade() else {
            return glib::Propagation::Proceed;
        };
        if !matches!(key, Key::Return | Key::KP_Enter | Key::space) {
            return glib::Propagation::Proceed;
        }
        let row = ui.tree.focus_child().and_downcast::<gtk::ListBoxRow>();
        let item = row.and_then(|r| ui.items.borrow().get(r.index() as usize).cloned());
        let Some(Item::Folder(f)) = item else {
            return glib::Propagation::Proceed;
        };
        glib::idle_add_local_once(move || toggle_folder(&ui, f));
        glib::Propagation::Stop
    });
    ui.tree.add_controller(keys);

    let weak = Rc::downgrade(ui);
    ui.search.connect_search_changed(move |_| {
        if let Some(ui) = weak.upgrade() {
            refresh_results(&ui);
        }
    });
    let weak = Rc::downgrade(ui);
    ui.search_button.connect_active_notify(move |_| {
        if let Some(ui) = weak.upgrade() {
            refresh_results(&ui);
        }
    });
    let weak = Rc::downgrade(ui);
    ui.results.connect_row_activated(move |_, row| {
        let Some(ui) = weak.upgrade() else { return };
        let n = ui.found.borrow().get(row.index() as usize).cloned();
        if let Some(n) = n {
            ui.search_button.set_active(false);
            open_note(&ui, &n);
        }
    });

    // Page Up and Down by what is shown: the text view's own would page by all of the note.
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = Rc::downgrade(ui);
    keys.connect_key_pressed(move |_, key, _, state| {
        use gtk::gdk::{Key, ModifierType};
        let (Some(ui), Some(down)) = (
            weak.upgrade(),
            match key {
                Key::Page_Down | Key::KP_Page_Down => Some(true),
                Key::Page_Up | Key::KP_Page_Up => Some(false),
                _ => None,
            },
        ) else {
            return glib::Propagation::Proceed;
        };
        if state.intersects(!ModifierType::SHIFT_MASK & gtk::accelerator_get_default_mod_mask()) {
            return glib::Propagation::Proceed;
        }
        let (view, adj) = (&ui.editor, ui.note_scroll.vadjustment());
        let page = if down {
            adj.page_size()
        } else {
            -adj.page_size()
        } as i32;
        let at = view.iter_location(&ui.buffer.iter_at_mark(&ui.buffer.get_insert()));
        // Past the end of a shorter line: the line's start.
        let y = at.y() + page;
        let to = view
            .iter_at_location(at.x(), y)
            .unwrap_or_else(|| view.line_at_y(y).0);
        adj.set_value(adj.value() + page as f64);
        if state.contains(ModifierType::SHIFT_MASK) {
            ui.buffer.move_mark(&ui.buffer.get_insert(), &to);
        } else {
            ui.buffer.place_cursor(&to);
        }
        glib::Propagation::Stop
    });
    ui.editor.add_controller(keys);

    let weak = Rc::downgrade(ui);
    ui.buffer.connect_mark_set(move |b, _, mark| {
        if let Some(ui) = weak.upgrade().filter(|_| *mark == b.get_insert()) {
            follow_cursor(&ui);
        }
    });
    // Typing, at the cursor; not a change elsewhere (a task ticked with a click).
    let at_cursor = |b: &sourceview5::Buffer, at: &gtk::TextIter| {
        at.line() == b.iter_at_mark(&b.get_insert()).line()
    };
    let weak = Rc::downgrade(ui);
    ui.buffer.connect_insert_text(move |b, at, _| {
        if let Some(ui) = weak.upgrade().filter(|_| at_cursor(b, at)) {
            follow_cursor(&ui);
        }
    });
    let weak = Rc::downgrade(ui);
    ui.buffer.connect_delete_range(move |b, start, end| {
        if let Some(ui) = weak
            .upgrade()
            .filter(|_| at_cursor(b, start) || at_cursor(b, end))
        {
            follow_cursor(&ui);
        }
    });

    // Typing saves after a pause.
    let weak = Rc::downgrade(ui);
    ui.buffer.connect_changed(move |_| {
        let Some(ui) = weak.upgrade() else { return };
        restyle(&ui);
        if ui.loading.get() {
            return;
        }
        ui.unsaved.set(true);
        if let Some(t) = ui.save_timer.borrow_mut().take() {
            t.remove();
        }
        let weak = Rc::downgrade(&ui);
        let t = glib::timeout_add_local_once(SAVE_AFTER, move || {
            if let Some(ui) = weak.upgrade() {
                ui.save_timer.borrow_mut().take();
                save_now(&ui);
            }
        });
        *ui.save_timer.borrow_mut() = Some(t);
    });

    // The title renames the note: on Enter, or when the focus leaves it.
    let weak = Rc::downgrade(ui);
    ui.title.connect_activate(move |_| {
        if let Some(ui) = weak.upgrade() {
            rename(&ui);
            ui.editor.grab_focus();
        }
    });
    let focus = gtk::EventControllerFocus::new();
    let weak = Rc::downgrade(ui);
    focus.connect_leave(move |_| {
        if let Some(ui) = weak.upgrade() {
            rename(&ui);
        }
    });
    ui.title.add_controller(focus);

    // Back to the window: what changed elsewhere meanwhile.
    ui.window.connect_is_active_notify(|w| {
        if w.is_active() && crate::ready() {
            crate::sync_now();
        }
    });
}

/// Rebuild the tree and the search results after the notes changed.
fn refresh(ui: &Rc<Ui>) {
    refresh_tree(ui);
    refresh_results(ui);
}

/// Rebuild the sidebar's tree, the open note's row selected.
fn refresh_tree(ui: &Rc<Ui>) {
    let (folders, notes) = if crate::ready() {
        (core().folders(), core().notes())
    } else {
        Default::default()
    };
    let items = tree(&folders, &notes, &ui.expanded.borrow());
    ui.tree.remove_all();
    ui.rows.borrow_mut().clear();
    let current = ui.current.borrow().as_ref().map(|n| n.id);
    if dense_tree() {
        ui.tree.add_css_class("dense");
    } else {
        ui.tree.remove_css_class("dense");
    }
    let icons = tree_icons();
    for (depth, item) in &items {
        let open = matches!(item, Item::Folder(f) if ui.expanded.borrow().contains(f));
        let row = tree_row(*depth, item, open, icons);
        match item {
            Item::Folder(f) => {
                // Every click opens or closes it: the list's own activation skips the second
                // and third press of a double or triple click.
                let click = gtk::GestureClick::new();
                // Not claimed on press, so the row can still be dragged (a drag cancels the
                // click); not activatable, so the list does not toggle it too.
                click.set_button(gtk::gdk::BUTTON_PRIMARY);
                row.set_activatable(false);
                let (weak, path) = (Rc::downgrade(ui), f.clone());
                click.connect_released(move |_, _, _, _| {
                    if let Some(ui) = weak.upgrade() {
                        let path = path.clone();
                        // After the click, as that rebuilds the tree.
                        glib::idle_add_local_once(move || toggle_folder(&ui, path));
                    }
                });
                row.add_controller(click);
                let name = f.rsplit('/').next().unwrap_or(f.as_str());
                connect_drag(ui, &row, Dragged::Folder(f.clone()), name);
            }
            Item::Note(n) => connect_drag(ui, &row, Dragged::Note(n.clone()), &n.title),
        }
        ui.tree.append(&row);
        if matches!(item, Item::Note(n) if Some(n.id) == current) {
            ui.tree.select_row(Some(&row));
        }
        ui.rows.borrow_mut().push(row);
    }
    ui.tree_stack
        .set_visible_child_name(if items.is_empty() { "empty" } else { "tree" });
    *ui.items.borrow_mut() = items.into_iter().map(|(_, i)| i).collect();
}

/// The tree's rows: each folder's folders, then its notes, by name; what is in a folder only
/// while it is open. With each row its depth.
fn tree(folders: &[String], notes: &[Note], expanded: &HashSet<String>) -> Vec<(usize, Item)> {
    fn add(
        out: &mut Vec<(usize, Item)>,
        folder: &str,
        depth: usize,
        folders: &[String],
        notes: &[Note],
        expanded: &HashSet<String>,
    ) {
        let parent = |f: &str| f.rsplit_once('/').map_or("", |(p, _)| p).to_string();
        let mut subs: Vec<&String> = folders.iter().filter(|f| parent(f) == folder).collect();
        subs.sort_by(|a, b| by_name(a.rsplit('/').next().unwrap(), b.rsplit('/').next().unwrap()));
        for f in subs {
            out.push((depth, Item::Folder(f.clone())));
            if expanded.contains(f) {
                add(out, f, depth + 1, folders, notes, expanded);
            }
        }
        let mut here: Vec<&Note> = notes.iter().filter(|n| n.folder == folder).collect();
        here.sort_by(|a, b| by_name(&a.title, &b.title));
        out.extend(here.into_iter().map(|n| (depth, Item::Note(n.clone()))));
    }
    let mut out = Vec::new();
    add(&mut out, "", 0, folders, notes, expanded);
    out
}

/// Names in the locale's order, case ignored.
fn by_name(a: &str, b: &str) -> std::cmp::Ordering {
    glib::casefold(a).as_gstr().collate(glib::casefold(b))
}

/// The device-only `dense_tree` setting: shorter rows.
pub fn dense_tree() -> bool {
    core().setting("dense_tree".into()).as_deref() == Some("true")
}

/// Which rows of the tree have an icon, (folders, notes), as the synced `tree_icons` setting
/// says: "all", "notes" (the default) or "none".
fn tree_icons() -> (bool, bool) {
    match core().setting("tree_icons".into()).as_deref() {
        Some("all") => (true, true),
        Some("none") => (false, false),
        _ => (false, true),
    }
}

/// The tree in the density and with the icons set in Preferences.
pub fn apply_tree_look() {
    if let Some(ui) = ui() {
        refresh_tree(&ui);
    }
}

/// A folder (its arrow showing whether it is open) or a note, indented by its depth, with its
/// icon if `icons` (folders, notes) says so.
fn tree_row(depth: usize, item: &Item, open: bool, icons: (bool, bool)) -> gtk::ListBoxRow {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    line.set_margin_start(INDENT * depth as i32);
    let (name, folder) = match item {
        Item::Folder(f) => (f.rsplit('/').next().unwrap_or(f), true),
        Item::Note(n) => (n.title.as_str(), false),
    };
    let arrow = gtk::Image::new();
    arrow.set_pixel_size(INDENT);
    if folder {
        arrow.set_icon_name(Some(if open {
            "pan-down-symbolic"
        } else {
            "pan-end-symbolic"
        }));
    }
    line.append(&arrow);
    if (folder && icons.0) || (!folder && icons.1) {
        let icon = gtk::Image::from_icon_name(if folder {
            "folder-symbolic"
        } else {
            "text-x-generic-symbolic"
        });
        icon.set_pixel_size(16);
        line.append(&icon);
    }
    let label = gtk::Label::new(Some(name));
    label.set_xalign(0.0);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    line.append(&label);
    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&line));
    row.set_selectable(!folder);
    // For screen readers and UI tests: a note's row has no tooltip to be named after.
    row.update_property(&[gtk::accessible::Property::Label(name)]);
    if let Item::Folder(f) = item {
        row.set_tooltip_text(Some(f));
    }
    row
}

/// Open or close a folder in the tree; new notes go there.
fn toggle_folder(ui: &Rc<Ui>, f: String) {
    if !ui.expanded.borrow_mut().remove(&f) {
        ui.expanded.borrow_mut().insert(f.clone());
    }
    save_expanded(ui);
    *ui.target_folder.borrow_mut() = f;
    refresh_tree(ui);
}

/// The open folders, kept on this device.
fn save_expanded(ui: &Ui) {
    let mut open: Vec<String> = ui.expanded.borrow().iter().cloned().collect();
    open.sort();
    let _ = core().set_setting("expanded_folders".into(), open.join("\n"));
}

/// Open the folders above `folder`, so a note in it shows in the tree.
fn reveal(ui: &Ui, folder: &str) {
    let mut f = folder;
    let mut changed = false;
    while !f.is_empty() {
        changed |= ui.expanded.borrow_mut().insert(f.to_string());
        f = f.rsplit_once('/').map_or("", |(p, _)| p);
    }
    if changed {
        save_expanded(ui);
    }
}

/// The notes matching the search in the editor's place, while something is typed.
fn refresh_results(ui: &Rc<Ui>) {
    let query = ui.search.text().trim().to_string();
    let searching = ui.search_button.is_active() && !query.is_empty() && crate::ready();
    let mut found = if searching {
        core().search(query)
    } else {
        Vec::new()
    };
    found.sort_by(|a, b| b.modified.cmp(&a.modified).then(a.path.cmp(&b.path)));
    ui.results.remove_all();
    for n in &found {
        let row = adw::ActionRow::builder()
            .title(&n.title)
            .subtitle(subtitle(n))
            .use_markup(false)
            .activatable(true)
            .build();
        ui.results.append(&row);
    }
    ui.results_stack
        .set_visible_child_name(if found.is_empty() { "none" } else { "list" });
    *ui.found.borrow_mut() = found;
    show_page(ui);
}

/// The search results while searching, else the open note (or that none is).
fn show_page(ui: &Ui) {
    let searching = ui.search_button.is_active() && !ui.search.text().trim().is_empty();
    ui.note_stack.set_visible_child_name(match () {
        _ if searching => "results",
        _ if ui.current.borrow().is_some() => "note",
        _ => "none",
    });
}

/// "Folder · date" under a note's title (the date in the locale's format).
fn subtitle(n: &Note) -> String {
    let date = glib::DateTime::from_unix_local(n.modified)
        .and_then(|d| d.format("%x"))
        .map(|d| d.to_string())
        .unwrap_or_default();
    if n.folder.is_empty() {
        date
    } else {
        format!("{} · {date}", n.folder)
    }
}

fn open_note(ui: &Rc<Ui>, n: &Note) {
    if ui.current.borrow().as_ref().map(|c| c.id) == Some(n.id) {
        return;
    }
    save_now(ui);
    let text = core().note_text(n.id).unwrap_or_default();
    *ui.current.borrow_mut() = Some(n.clone());
    ui.title.set_text(&n.title);
    set_text(ui, &text, false);
    ui.buffer.place_cursor(&ui.buffer.start_iter());
    show_heading(ui, n);
    show_page(ui);
    *ui.target_folder.borrow_mut() = n.folder.clone();
    reveal(ui, &n.folder);
    refresh_tree(ui);
    let _ = core().set_setting("last_note".into(), n.path.clone());
}

fn show_heading(ui: &Ui, n: &Note) {
    ui.note_title.set_title(&n.title);
    ui.note_title.set_subtitle(&n.folder);
}

fn close_note(ui: &Rc<Ui>) {
    save_now(ui);
    ui.current.take();
    set_text(ui, "", false);
    ui.title.set_text("");
    ui.note_title.set_title("");
    ui.note_title.set_subtitle("");
    show_page(ui);
    ui.tree.unselect_all();
}

/// Replace the editor's text without saving it back; `keep_cursor`: where it was, as far as
/// the new text goes. Not something to undo.
fn set_text(ui: &Ui, text: &str, keep_cursor: bool) {
    let offset = ui.buffer.cursor_position();
    ui.loading.set(true);
    ui.buffer.begin_irreversible_action();
    ui.buffer.set_text(text);
    ui.buffer.end_irreversible_action();
    crate::editor::forget(&ui.buffer);
    ui.loading.set(false);
    ui.unsaved.set(false);
    if keep_cursor {
        ui.buffer
            .place_cursor(&ui.buffer.iter_at_offset(offset.min(ui.buffer.char_count())));
    }
}

/// Give the editor's text to the core, if it changed; the sync takes it to the server.
fn save_now(ui: &Ui) {
    if let Some(t) = ui.save_timer.borrow_mut().take() {
        t.remove();
    }
    if !ui.unsaved.replace(false) {
        return;
    }
    let Some(id) = ui.current.borrow().as_ref().map(|n| n.id) else {
        return;
    };
    let (start, end) = ui.buffer.bounds();
    let text = ui.buffer.text(&start, &end, true).to_string();
    match core().set_text(id, text) {
        Ok(()) => request_sync(),
        Err(e) => {
            toast(&gettext("Cannot save the note: {error}").replace("{error}", &e.to_string()))
        }
    }
}

/// After a sync or another change: the open note as it is now. Its text is replaced only
/// while the editor has nothing unsaved (a sync never overwrites a note changed here).
fn reload_current(ui: &Rc<Ui>) {
    let Some(id) = ui.current.borrow().as_ref().map(|n| n.id) else {
        return;
    };
    let Some(n) = core().note(id) else {
        // Deleted, here or elsewhere.
        close_note(ui);
        return;
    };
    if !ui.title.has_focus() {
        ui.title.set_text(&n.title);
    }
    show_heading(ui, &n);
    *ui.current.borrow_mut() = Some(n);
    if ui.unsaved.get() {
        return;
    }
    let text = core().note_text(id).unwrap_or_default();
    let (start, end) = ui.buffer.bounds();
    if ui.buffer.text(&start, &end, true) != text {
        set_text(ui, &text, true);
    }
}

/// Rename the open note after its title entry.
fn rename(ui: &Rc<Ui>) {
    let Some(n) = ui.current.borrow().clone() else {
        return;
    };
    let title = ui.title.text().trim().to_string();
    if title == n.title {
        return;
    }
    if title.is_empty() {
        ui.title.set_text(&n.title);
        return;
    }
    match core().move_note(n.id, n.folder.clone(), title) {
        Ok(moved) => {
            ui.title.set_text(&moved.title);
            request_sync();
        }
        Err(notav_core::Error::Exists(_)) => {
            toast(&gettext("A note with this name exists already"));
            ui.title.set_text(&n.title);
        }
        Err(e) => {
            toast(&e.to_string());
            ui.title.set_text(&n.title);
        }
    }
}

/// A new note in the folder last opened in the tree or the open note's (at the top if that
/// is gone), its title selected for typing over.
fn new_note(ui: &Rc<Ui>) {
    let folder = Some(ui.target_folder.borrow().clone())
        .filter(|f| core().folders().contains(f))
        .unwrap_or_default();
    new_note_in(ui, folder);
}

/// A new note in `folder` ("": the top level), its title selected for typing over.
fn new_note_in(ui: &Rc<Ui>, folder: String) {
    save_now(ui);
    match core().add_note(folder, gettext("Untitled"), String::new()) {
        Ok(n) => {
            ui.search_button.set_active(false);
            open_note(ui, &n);
            ui.split.set_show_content(true);
            // Once its page is shown: selecting earlier does not stick.
            let title = ui.title.clone();
            glib::idle_add_local_once(move || {
                title.grab_focus();
                title.select_region(0, -1);
            });
            request_sync();
        }
        Err(e) => toast(&e.to_string()),
    }
}

/// A new folder in `parent` ("": the top level), its name asked for; shown open.
fn new_folder(ui: &Rc<Ui>, parent: String) {
    let b = view!("new-folder");
    let dialog: adw::AlertDialog = get(&b, "dialog");
    let name: adw::EntryRow = get(&b, "name");
    let d = dialog.clone();
    name.connect_changed(move |e| d.set_response_enabled("create", !e.text().trim().is_empty()));
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        if dialog.choose_future(Some(&ui.window)).await != "create" {
            return;
        }
        let name = name.text().trim().trim_matches('/').to_string();
        let path = if parent.is_empty() {
            name
        } else {
            format!("{parent}/{name}")
        };
        if core().folders().contains(&path) {
            toast(&gettext("A folder with this name exists already"));
            return;
        }
        match core().add_folder(path.clone()) {
            Ok(()) => {
                reveal(&ui, &path);
                *ui.target_folder.borrow_mut() = path;
                refresh_tree(&ui);
                request_sync();
            }
            Err(notav_core::Error::Invalid(_)) => toast(&gettext("Not a folder name")),
            Err(e) => toast(&e.to_string()),
        }
    });
}

/// Rename folder `folder`, its new name asked for in the New Folder dialog.
fn rename_folder(ui: &Rc<Ui>, folder: String) {
    let b = view!("new-folder");
    let dialog: adw::AlertDialog = get(&b, "dialog");
    let name: adw::EntryRow = get(&b, "name");
    dialog.set_heading(Some(&gettext("Rename Folder")));
    dialog.set_response_label("create", &gettext("Rename"));
    name.set_text(folder.rsplit('/').next().unwrap_or(&folder));
    let d = dialog.clone();
    name.connect_changed(move |e| d.set_response_enabled("create", !e.text().trim().is_empty()));
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        if dialog.choose_future(Some(&ui.window)).await != "create" {
            return;
        }
        save_now(&ui);
        match core().rename_folder(folder.clone(), name.text().to_string()) {
            Ok(to) => {
                follow_folder(&ui, &folder, &to);
                refresh(&ui);
                reload_current(&ui);
                request_sync();
            }
            Err(notav_core::Error::Exists(_)) => {
                toast(&gettext("A folder with this name exists already"))
            }
            Err(notav_core::Error::Invalid(_)) => toast(&gettext("Not a folder name")),
            Err(e) => toast(&e.to_string()),
        }
    });
}

/// Delete folder `folder` with everything in it; the toast can undo it.
fn delete_folder(ui: &Rc<Ui>, folder: String) {
    // What is typed in a note in it comes back with an undo.
    save_now(ui);
    match core().delete_folder(folder.clone()) {
        Ok(()) => {
            refresh(ui);
            reload_current(ui);
            request_sync();
            let name = folder.rsplit('/').next().unwrap_or(&folder);
            undo_toast(ui, &gettext("“{title}” deleted").replace("{title}", name));
        }
        Err(e) => toast(&e.to_string()),
    }
}

/// Delete a note; the toast can undo it.
fn delete_note(ui: &Rc<Ui>, id: i64) {
    let Some(n) = core().note(id) else {
        return;
    };
    save_now(ui);
    match core().delete_note(id) {
        Ok(()) => {
            refresh(ui);
            reload_current(ui);
            request_sync();
            undo_toast(
                ui,
                &gettext("“{title}” deleted").replace("{title}", &n.title),
            );
        }
        Err(e) => toast(&e.to_string()),
    }
}

/// The tree's menu, on a right click or a long press: on a folder, new notes and folders go
/// inside it; on a note, next to it (in its folder), and it can be deleted; elsewhere, to the
/// top level.
fn context_menu(ui: &Rc<Ui>) {
    let weak = Rc::downgrade(ui);
    let show = Rc::new(move |x: f64, y: f64| {
        let Some(ui) = weak.upgrade() else {
            return false;
        };
        if !crate::ready() {
            return false;
        }
        // The stack holds the tree, or the "No Notes" page while there are none.
        let row = ui
            .tree_stack
            .compute_point(&ui.tree, &gtk::graphene::Point::new(x as f32, y as f32))
            .filter(|_| ui.tree.is_mapped())
            .and_then(|at| ui.tree.row_at_y(at.y() as i32));
        let item = row
            .as_ref()
            .and_then(|r| ui.items.borrow().get(r.index() as usize).cloned());
        let folder = match &item {
            Some(Item::Folder(f)) => f.clone(),
            Some(Item::Note(n)) => n.folder.clone(),
            None => String::new(),
        };
        let m = gio::Menu::new();
        let add = gio::Menu::new();
        let target = Some(folder.to_variant());
        let new_note = gio::Action::print_detailed_name("win.new-note-in", target.as_ref());
        add.append(Some(&gettext("New Note")), Some(&new_note));
        let new_folder = gio::Action::print_detailed_name("win.new-folder-in", target.as_ref());
        add.append(Some(&gettext("New Folder")), Some(&new_folder));
        m.append_section(None, &add);
        match &item {
            Some(Item::Note(n)) => {
                let delete =
                    gio::Action::print_detailed_name("win.delete-note", Some(&n.id.to_variant()));
                let note = gio::Menu::new();
                note.append(Some(&gettext("Delete")), Some(&delete));
                m.append_section(None, &note);
            }
            Some(Item::Folder(_)) => {
                let rename = gio::Action::print_detailed_name("win.rename-folder", target.as_ref());
                let delete = gio::Action::print_detailed_name("win.delete-folder", target.as_ref());
                let f = gio::Menu::new();
                f.append(Some(&gettext("Rename…")), Some(&rename));
                f.append(Some(&gettext("Delete")), Some(&delete));
                m.append_section(None, &f);
            }
            None => {}
        }
        // Gone once closed.
        let menu = gtk::PopoverMenu::from_model(Some(&m));
        // On the row it is for, else the tree's stack (a list box takes no other children).
        let parent: gtk::Widget = row
            .as_ref()
            .map_or(ui.tree_stack.clone().upcast(), |r| r.clone().upcast());
        let at = ui
            .tree_stack
            .compute_point(&parent, &gtk::graphene::Point::new(x as f32, y as f32))
            .unwrap_or_else(|| gtk::graphene::Point::new(x as f32, y as f32));
        menu.set_parent(&parent);
        menu.set_has_arrow(false);
        menu.set_halign(gtk::Align::Start);
        menu.set_pointing_to(Some(&gtk::gdk::Rectangle::new(
            at.x() as i32,
            at.y() as i32,
            1,
            1,
        )));
        menu.connect_closed(|m| {
            let m = m.clone();
            glib::idle_add_local_once(move || m.unparent());
        });
        menu.popup();
        true
    });
    let click = gtk::GestureClick::builder()
        .button(gtk::gdk::BUTTON_SECONDARY)
        .build();
    let s = show.clone();
    click.connect_pressed(move |g, _, x, y| {
        if s(x, y) {
            g.set_state(gtk::EventSequenceState::Claimed);
        }
    });
    ui.tree_stack.add_controller(click);
    let hold = gtk::GestureLongPress::builder().touch_only(true).build();
    hold.connect_pressed(move |g, x, y| {
        if show(x, y) {
            g.set_state(gtk::EventSequenceState::Claimed);
        }
    });
    ui.tree_stack.add_controller(hold);
}

/// A row drags its note or folder, shown as a card with its name.
fn connect_drag(ui: &Rc<Ui>, row: &gtk::ListBoxRow, what: Dragged, name: &str) {
    let drag = gtk::DragSource::new();
    drag.set_actions(gtk::gdk::DragAction::MOVE);
    let id = match &what {
        Dragged::Note(n) => format!("note:{}", n.id),
        Dragged::Folder(f) => format!("folder:{f}"),
    };
    drag.connect_prepare(move |_, _, _| Some(gtk::gdk::ContentProvider::for_value(&id.to_value())));
    let (weak, r, name) = (Rc::downgrade(ui), row.clone(), name.to_string());
    drag.connect_drag_begin(move |_, drag| {
        if let Some(ui) = weak.upgrade() {
            *ui.dragging.borrow_mut() = Some(what.clone());
        }
        let card = gtk::Label::new(Some(&name));
        card.add_css_class("drag-card");
        if let Ok(icon) = gtk::DragIcon::for_drag(drag).downcast::<gtk::DragIcon>() {
            icon.set_child(Some(&card));
        }
        r.add_css_class("dragged");
    });
    let (weak, r) = (Rc::downgrade(ui), row.clone());
    drag.connect_drag_end(move |_, _, _| {
        r.remove_css_class("dragged");
        if let Some(ui) = weak.upgrade() {
            ui.dragging.take();
            mark_drop(&ui, None);
        }
    });
    row.add_controller(drag);
}

/// Notes and folders dropped on the tree move: onto a folder, into it; onto a note, into its
/// folder; below the last row, to the top level. Where it would go is tinted (`mark_drop`);
/// where it is already, or a folder into itself, nothing happens. One target for the whole tree, finding the row itself: GTK gives a
/// drag's motion to every drop target under the pointer, so targets on the rows and one for the
/// top level behind them would all take it.
fn connect_drop(ui: &Rc<Ui>) {
    let drop = gtk::DropTarget::new(glib::Type::STRING, gtk::gdk::DragAction::MOVE);
    let weak = Rc::downgrade(ui);
    drop.connect_accept(move |_, _| {
        weak.upgrade()
            .is_some_and(|ui| ui.dragging.borrow().is_some())
    });
    let hover = {
        let weak = Rc::downgrade(ui);
        move |x: f64, y: f64| {
            let Some(ui) = weak.upgrade() else {
                return gtk::gdk::DragAction::empty();
            };
            let to = drop_folder(&ui, x, y);
            mark_drop(&ui, to.as_deref());
            if to.is_some() {
                gtk::gdk::DragAction::MOVE
            } else {
                gtk::gdk::DragAction::empty()
            }
        }
    };
    let h = hover.clone();
    drop.connect_enter(move |_, x, y| h(x, y));
    drop.connect_motion(move |_, x, y| hover(x, y));
    let weak = Rc::downgrade(ui);
    drop.connect_leave(move |_| {
        if let Some(ui) = weak.upgrade() {
            mark_drop(&ui, None);
        }
    });
    let weak = Rc::downgrade(ui);
    drop.connect_drop(move |_, _, x, y| {
        let Some(ui) = weak.upgrade() else {
            return false;
        };
        mark_drop(&ui, None);
        let Some(folder) = drop_folder(&ui, x, y) else {
            return false;
        };
        let Some(what) = ui.dragging.take() else {
            return false;
        };
        // After the drop: moving rebuilds the tree, the drag's rows with it.
        glib::idle_add_local_once(move || {
            save_now(&ui);
            let moved = match &what {
                Dragged::Note(n) => core()
                    .move_note(n.id, folder.clone(), n.title.clone())
                    .map(|_| ()),
                Dragged::Folder(f) => core()
                    .move_folder(f.clone(), folder.clone())
                    .map(|to| follow_folder(&ui, f, &to)),
            };
            match moved {
                Ok(()) => {
                    reveal(&ui, &folder);
                    refresh(&ui);
                    reload_current(&ui);
                    request_sync();
                }
                Err(notav_core::Error::Exists(_)) => toast(&match what {
                    Dragged::Note(_) => {
                        gettext("A note with this name exists in that folder already")
                    }
                    Dragged::Folder(_) => {
                        gettext("A folder with this name exists in that folder already")
                    }
                }),
                Err(e) => toast(&e.to_string()),
            }
        });
        true
    });
    ui.tree_scroll.add_controller(drop);
}

/// The folder the dragged note or folder would go to with the pointer at (`x`, `y`) on the
/// tree's scrolled window ("": the top level); None when nothing would move.
fn drop_folder(ui: &Ui, x: f64, y: f64) -> Option<String> {
    let at = ui
        .tree_scroll
        .compute_point(&ui.tree, &gtk::graphene::Point::new(x as f32, y as f32))?;
    let folder = match ui.tree.row_at_y(at.y() as i32) {
        Some(row) => match ui.items.borrow().get(row.index() as usize)? {
            Item::Folder(f) => f.clone(),
            Item::Note(n) => n.folder.clone(),
        },
        None => String::new(),
    };
    moves(ui.dragging.borrow().as_ref()?, &folder).then_some(folder)
}

/// Whether dropping `what` into `folder` moves it: not where it is, not a folder into itself.
fn moves(what: &Dragged, folder: &str) -> bool {
    match what {
        Dragged::Note(n) => n.folder != folder,
        Dragged::Folder(f) => {
            let parent = f.rsplit_once('/').map_or("", |(p, _)| p);
            parent != folder && folder != f && !folder.starts_with(&format!("{f}/"))
        }
    }
}

/// A folder moved from `from` to `to`: the tree keeps its folders open, new notes still go
/// where they went.
fn follow_folder(ui: &Ui, from: &str, to: &str) {
    let renamed = |p: &str| {
        (p == from || p.starts_with(&format!("{from}/")))
            .then(|| format!("{to}{}", &p[from.len()..]))
    };
    let open: HashSet<String> = ui
        .expanded
        .borrow()
        .iter()
        .map(|p| renamed(p).unwrap_or_else(|| p.clone()))
        .collect();
    *ui.expanded.borrow_mut() = open;
    save_expanded(ui);
    let target = renamed(&ui.target_folder.borrow());
    if let Some(t) = target {
        *ui.target_folder.borrow_mut() = t;
    }
}

/// Tint where a dragged note would go: a folder's row with what shows of it below, or the
/// whole tree for the top level ("").
fn mark_drop(ui: &Ui, folder: Option<&str>) {
    if folder == Some("") {
        ui.tree.add_css_class("drop-target");
    } else {
        ui.tree.remove_css_class("drop-target");
    }
    for (row, item) in ui.rows.borrow().iter().zip(ui.items.borrow().iter()) {
        if folder.is_some_and(|f| !f.is_empty() && within(item, f)) {
            row.add_css_class("drop-target");
        } else {
            row.remove_css_class("drop-target");
        }
    }
}

/// `item` is the folder `folder` or inside it.
fn within(item: &Item, folder: &str) -> bool {
    let inside = |p: &str| p == folder || p.starts_with(&format!("{folder}/"));
    match item {
        Item::Folder(f) => inside(f),
        Item::Note(n) => inside(&n.folder),
    }
}

// --- welcome -----------------------------------------------------------------

fn connect_welcome(ui: &Rc<Ui>, b: &gtk::Builder) {
    let page: adw::NavigationPage = get(b, "connect_page");
    let status: adw::StatusPage = get(b, "connect_status");
    let use_goa: adw::ActionRow = get(b, "use_goa");
    // The switch is the choice made on the first step.
    ui.conn.goa.set_visible(false);
    match ui.conn.account_names().as_slice() {
        [] => use_goa.set_subtitle(&gettext(
            "No Nextcloud account in Settings → Online Accounts",
        )),
        names => use_goa.set_subtitle(&names.join(", ")),
    }
    use_goa.set_sensitive(ui.conn.has_accounts());

    let open = {
        let ui = ui.clone();
        move |goa: bool| {
            ui.conn.goa.set_active(goa);
            // Shown even for one account, as what Connect connects to.
            ui.conn.account.set_visible(goa);
            ui.connect_goa.set_visible(goa);
            ui.login.set_visible(!goa);
            if goa {
                page.set_title(&gettext("GNOME Online Accounts"));
                status.set_icon_name(Some("goa-panel-symbolic"));
                status.set_description(Some(&gettext(
                    "Connect with a Nextcloud account from GNOME Settings",
                )));
            } else {
                page.set_title(&gettext("Nextcloud Server"));
                status.set_icon_name(Some("network-server-symbolic"));
                status.set_description(Some(&gettext(
                    "Sign in to your Nextcloud; Notav gets an app password of its own",
                )));
            }
            ui.welcome.push_by_tag("connect");
        }
    };
    let o = open.clone();
    use_goa.connect_activated(move |_| o(true));
    get::<adw::ActionRow>(b, "use_server").connect_activated(move |_| open(false));

    let weak = Rc::downgrade(ui);
    ui.login.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            start_login(&ui, false);
        }
    });
    let weak = Rc::downgrade(ui);
    ui.connect_goa.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            connect_goa(&ui);
        }
    });
    let weak = Rc::downgrade(ui);
    let copy = gio::SimpleAction::new("copy-login", None);
    copy.connect_activate(move |_, _| {
        if let Some(ui) = weak.upgrade() {
            start_login(&ui, true);
        }
    });
    ui.window.add_action(&copy);
    let weak = Rc::downgrade(ui);
    let password = gio::SimpleAction::new("password-login", None);
    password.connect_activate(move |_, _| {
        let Some(ui) = weak.upgrade() else { return };
        let server = url_text(&ui.conn.server);
        if server.is_empty() {
            toast(&no_server());
            return;
        }
        let u = ui.clone();
        password_login(&ui.window, server, move |c| {
            let ui = u.clone();
            glib::spawn_future_local(async move {
                connect_with(&ui, crate::Target::Manual(c)).await;
            });
        });
    });
    ui.window.add_action(&password);
    let weak = Rc::downgrade(ui);
    ui.cancel_login.connect_clicked(move |_| {
        cancel_login(); // the running sign-in sees it is stale and stops
        if let Some(ui) = weak.upgrade() {
            reset_login(&ui);
        }
    });

    // Back to the first step: a sign-in waiting for the browser stops.
    let weak = Rc::downgrade(ui);
    ui.welcome.connect_popped(move |_, _| {
        cancel_login();
        if let Some(ui) = weak.upgrade() {
            reset_login(&ui);
        }
    });

    let weak = Rc::downgrade(ui);
    ui.folder_done.connect_clicked(move |_| {
        if let Some(ui) = weak.upgrade() {
            choose_folder(&ui);
        }
    });
    let weak = Rc::downgrade(ui);
    ui.folder.connect_entry_activated(move |_| {
        if let Some(ui) = weak.upgrade() {
            choose_folder(&ui);
        }
    });
}

/// What every way of signing in says when the server field is empty.
pub fn no_server() -> String {
    gettext("Enter the Nextcloud server first")
}

/// Full URL from a scheme picker and host entry; empty stays empty, a typed scheme wins over the picker.
pub fn url_text((scheme, row): &(gtk::DropDown, adw::EntryRow)) -> String {
    let host = row.text().trim().to_string();
    if host.is_empty() || host.contains("://") {
        return host;
    }
    let scheme = if scheme.selected() == 0 {
        "https"
    } else {
        "http"
    };
    format!("{scheme}://{host}")
}

/// Sign in from the welcome page with the browser (Login Flow v2); `copy` puts the login link on
/// the clipboard instead of opening the browser.
fn start_login(ui: &Rc<Ui>, copy: bool) {
    let server = url_text(&ui.conn.server);
    if server.is_empty() {
        toast(&no_server());
        return;
    }
    ui.login.set_sensitive(false);
    ui.login.set_label(&if copy {
        gettext("Waiting for Sign-In…")
    } else {
        gettext("Waiting for Browser…")
    });
    ui.cancel_login.set_visible(true);
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        match browser_login(server, copy).await {
            Ok(Some(c)) => connect_with(&ui, crate::Target::Manual(c)).await,
            // Cancelled: Cancel already reset the page, which may be waiting for a new sign-in.
            Ok(None) => return,
            Err(e) => ui.toasts.add_toast(note(&e)),
        }
        reset_login(&ui);
    });
}

/// Connect from the welcome page with the chosen GNOME Online Accounts account.
fn connect_goa(ui: &Rc<Ui>) {
    let Some(account) = ui.conn.chosen() else {
        return;
    };
    ui.connect_goa.set_sensitive(false);
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        connect_with(&ui, crate::Target::Goa(account)).await;
        ui.connect_goa.set_sensitive(true);
    });
}

/// Connected: the notes folder is next.
async fn connect_with(ui: &Rc<Ui>, target: crate::Target) {
    match crate::switch_account(target).await {
        Ok(()) => show_folder_step(ui),
        Err(e) => ui.toasts.add_toast(note(&e)),
    }
}

/// The welcome page's last step, its folder suggested by the server: the one other devices
/// use, else Nextcloud Notes' own.
fn show_folder_step(ui: &Rc<Ui>) {
    ui.welcome.push_by_tag("folder");
    ui.folder.set_text("");
    ui.folder.set_sensitive(false);
    ui.folder_done.set_sensitive(false);
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let folder = gio::spawn_blocking(|| core().suggested_notes_folder())
            .await
            .unwrap_or_else(|_| notav_core::DEFAULT_FOLDER.into());
        ui.folder.set_text(&folder);
        ui.folder.set_sensitive(true);
        ui.folder_done.set_sensitive(true);
    });
}

fn choose_folder(ui: &Rc<Ui>) {
    let folder = ui.folder.text().trim().to_string();
    if folder.is_empty() || !ui.folder_done.is_sensitive() {
        return;
    }
    ui.folder_done.set_sensitive(false);
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let r = gio::spawn_blocking(move || core().set_notes_folder(folder, false)).await;
        ui.folder_done.set_sensitive(true);
        match r {
            Ok(Ok(())) => {
                crate::start_background();
                ui.stack.set_visible_child_name("main");
                refresh(&ui);
            }
            Ok(Err(notav_core::Error::Invalid(_))) => toast(&gettext("Not a folder name")),
            Ok(Err(e)) => toast(&e.to_string()),
            Err(_) => {}
        }
    });
}

/// Bumped on every sign-in and on cancel; a sign-in only goes on while its number is current.
static LOGIN_ATTEMPT: AtomicU64 = AtomicU64::new(0);

/// Stop a running `browser_login`; it then returns `Ok(None)`.
pub fn cancel_login() {
    LOGIN_ATTEMPT.fetch_add(1, Ordering::SeqCst);
}

fn reset_login(ui: &Ui) {
    ui.login.set_sensitive(true);
    ui.login.set_label(&gettext("Sign In with Browser"));
    ui.cancel_login.set_visible(false);
}

/// Login Flow v2: open the browser (or copy the link) and wait until the user approves.
/// `Ok(None)`: cancelled, or another sign-in started.
pub async fn browser_login(
    server: String,
    copy: bool,
) -> Result<Option<notav_core::Credentials>, String> {
    let attempt = LOGIN_ATTEMPT.fetch_add(1, Ordering::SeqCst) + 1;
    let current = move || LOGIN_ATTEMPT.load(Ordering::SeqCst) == attempt;
    let flow = gio::spawn_blocking(move || notav_core::login_flow_start(server))
        .await
        .map_err(|_| gettext("Cannot start login"))?
        .map_err(|e| gettext("Cannot start login: {error}").replace("{error}", &e.to_string()))?;
    if !current() {
        return Ok(None);
    }
    if copy {
        if let Some(d) = gtk::gdk::Display::default() {
            d.clipboard().set_text(&flow.login_url);
        }
        toast(&gettext("Login link copied to clipboard"));
    } else {
        let parent = ui().map(|u| u.window.clone());
        gtk::UriLauncher::new(&flow.login_url).launch(
            parent.as_ref(),
            gio::Cancellable::NONE,
            |_| {},
        );
    }
    let flow = std::sync::Arc::new(flow);
    for _ in 0..600 {
        glib::timeout_future_seconds(2).await;
        if !current() {
            return Ok(None);
        }
        let f = flow.clone();
        match gio::spawn_blocking(move || notav_core::login_flow_poll(&f)).await {
            Ok(Ok(None)) => continue,
            Ok(Ok(Some(c))) => return Ok(current().then_some(c)),
            Ok(Err(e)) => {
                return Err(gettext("Login failed: {error}").replace("{error}", &e.to_string()));
            }
            Err(_) => return Err(gettext("Login failed")),
        }
    }
    Err(gettext("Login timed out"))
}

/// Ask for a login name and (app) password for `server` and check them there; `done` gets the
/// credentials once they work.
pub fn password_login(
    parent: &impl IsA<gtk::Widget>,
    server: String,
    done: impl Fn(notav_core::Credentials) + 'static,
) {
    let b = view!("password-login");
    let dialog: adw::Dialog = get(&b, "dialog");
    let user: adw::EntryRow = get(&b, "user");
    let password: adw::PasswordEntryRow = get(&b, "password");
    let sign_in: gtk::Button = get(&b, "sign_in");
    let error: adw::Banner = get(&b, "error");
    // Signing in again usually keeps the login name.
    if let Some(a) = core().account().filter(|a| a.url == server) {
        user.set_text(&a.user);
    }
    let filled = {
        let (user, password, sign_in) = (user.clone(), password.clone(), sign_in.clone());
        let error = error.clone();
        move || {
            error.set_revealed(false);
            sign_in.set_sensitive(!user.text().trim().is_empty() && !password.text().is_empty())
        }
    };
    let f = filled.clone();
    user.connect_changed(move |_| f());
    password.connect_changed(move |_| filled());
    let d = dialog.clone();
    get::<gtk::Button>(&b, "cancel").connect_clicked(move |_| {
        d.close();
    });
    let (d, done) = (dialog.clone(), Rc::new(done));
    sign_in.connect_clicked(move |button| {
        let (login, pw) = (user.text().trim().to_string(), password.text().to_string());
        let server = server.clone();
        button.set_sensitive(false);
        let (d, done, error, button) = (d.clone(), done.clone(), error.clone(), button.clone());
        glib::spawn_future_local(async move {
            let r =
                gio::spawn_blocking(move || notav_core::password_login(server, login, pw)).await;
            let msg = match r {
                Ok(Ok(c)) => {
                    d.close();
                    return done(c);
                }
                Ok(Err(notav_core::Error::Auth)) => gettext("Wrong login name or password"),
                Ok(Err(e)) => gettext("Sign-in failed: {error}").replace("{error}", &e.to_string()),
                Err(_) => gettext("Sign-in failed"),
            };
            error.set_title(&msg);
            error.set_revealed(true);
            button.set_sensitive(true);
        });
    });
    dialog.present(Some(parent));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(path: &str) -> Note {
        let (folder, file) = path.rsplit_once('/').unwrap_or(("", path));
        Note {
            id: 0,
            path: path.into(),
            title: file.trim_end_matches(".md").into(),
            folder: folder.into(),
            modified: 0,
            pending: false,
        }
    }

    #[test]
    fn drops_that_move_nothing_or_into_itself_are_refused() {
        let folder = |f: &str| Dragged::Folder(f.into());
        assert!(moves(&Dragged::Note(note("Work/Plan.md")), ""));
        assert!(!moves(&Dragged::Note(note("Work/Plan.md")), "Work"));
        assert!(moves(&folder("Work/Old"), ""));
        assert!(moves(&folder("Work"), "Archive"));
        assert!(!moves(&folder("Work/Old"), "Work"), "already there");
        assert!(!moves(&folder("Work"), "Work"));
        assert!(!moves(&folder("Work"), "Work/Old"), "into itself");
        assert!(moves(&folder("Work"), "Workshop"));
    }

    #[test]
    fn a_folder_holds_its_notes_and_folders_at_any_depth() {
        let folder = |f: &str| Item::Folder(f.into());
        assert!(within(&folder("Work"), "Work") && within(&folder("Work/Old"), "Work"));
        assert!(within(&Item::Note(note("Work/Old/x.md")), "Work"));
        assert!(!within(&folder("Workshop"), "Work") && !within(&Item::Note(note("x.md")), "Work"));
    }

    #[test]
    fn tree_lists_folders_first_and_only_open_ones_contents() {
        let folders: Vec<String> = ["Work", "Work/Old", "archive"].map(String::from).into();
        let notes = [
            "b.md",
            "A.md",
            "Work/Plan.md",
            "Work/Old/x.md",
            "archive/y.md",
        ]
        .map(note);
        let names = |open: &[&str]| -> Vec<(usize, String)> {
            let open = open.iter().map(|s| s.to_string()).collect();
            tree(&folders, &notes, &open)
                .into_iter()
                .map(|(d, i)| match i {
                    Item::Folder(f) => (d, format!("{f}/")),
                    Item::Note(n) => (d, n.title),
                })
                .collect()
        };
        let s = |d: usize, n: &str| (d, n.to_string());
        // Case does not decide the order: "archive" before "Work", "A" before "b".
        assert_eq!(
            names(&[]),
            [s(0, "archive/"), s(0, "Work/"), s(0, "A"), s(0, "b")]
        );
        assert_eq!(
            names(&["Work", "Work/Old"]),
            [
                s(0, "archive/"),
                s(0, "Work/"),
                s(1, "Work/Old/"),
                s(2, "x"),
                s(1, "Plan"),
                s(0, "A"),
                s(0, "b")
            ]
        );
    }
}
