//! Main window: the welcome page (sign-in, then the notes folder), the notes sidebar and the
//! editor.

use crate::i18n::{gettext, ngettext};
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
const CSS: &str = "
.note-title { font-size: 20pt; font-weight: 800; }
.note-text { font-size: 12pt; }
";

/// How far each folder level is indented in the tree, and the width of a folder's arrow.
const INDENT: i32 = 16;

/// A row of the sidebar's tree.
#[derive(Debug, Clone, PartialEq)]
enum Item {
    Folder(String),
    Note(Note),
}

/// How long typing pauses before the text is saved (and the sync asked for).
const SAVE_AFTER: std::time::Duration = std::time::Duration::from_millis(800);

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
    /// The items of `tree`'s rows, in order.
    items: RefCell<Vec<Item>>,
    /// Folders shown open in the tree (the device-only `expanded_folders` setting).
    expanded: RefCell<HashSet<String>>,
    /// Where a new note goes: the folder last opened in the tree, or the open note's.
    target_folder: RefCell<String>,
    note_title: adw::WindowTitle,
    note_menu: gtk::MenuButton,
    note_stack: gtk::Stack,
    title: gtk::Entry,
    buffer: sourceview5::Buffer,
    editor: sourceview5::View,
    current: RefCell<Option<Note>>,
    /// The editor holds text not given to the core yet.
    unsaved: Cell<bool>,
    save_timer: RefCell<Option<glib::SourceId>>,
    /// Set while code replaces the editor's text or the list's rows.
    loading: Cell<bool>,
    last_error: RefCell<String>,
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
            "/io/github/selfref/Notav/metainfo.xml",
            Some(env!("CARGO_PKG_VERSION")),
        );
        d.set_copyright("© 2026 SelfRef");
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
                    ui.toasts.add_toast(note(
                        &gettext("Sync failed: {error}").replace("{error}", err),
                    ));
                    *ui.last_error.borrow_mut() = err.to_string();
                }
            } else if s == "idle" {
                ui.last_error.borrow_mut().clear();
            }
        }
        Event::Conflicts(copies) => {
            let name = |p: &str| {
                let file = p.rsplit('/').next().unwrap_or(p);
                file.rsplit_once('.').map_or(file, |(t, _)| t).to_string()
            };
            let msg = match copies.as_slice() {
                [one] => gettext(
                    "“{title}” was changed elsewhere too: your version is kept next to it as a copy",
                )
                .replace("{title}", &name(one)),
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
            .language("markdown")
            .as_ref(),
    );
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
        note_menu: get(&b, "note_menu"),
        note_stack: get(&b, "note_stack"),
        title: get(&b, "title"),
        buffer,
        editor,
        current: RefCell::new(None),
        unsaved: Cell::new(false),
        save_timer: RefCell::new(None),
        loading: Cell::new(false),
        last_error: RefCell::new(String::new()),
    });
    ui.conn.attach(&get(&b, "login_group"));
    close_note(&ui);
    connect(&ui);
    connect_welcome(&ui, &b);
    ui
}

/// The editor's colours follow the light or dark style.
fn follow_style(buffer: &sourceview5::Buffer) {
    let apply = {
        let buffer = buffer.clone();
        move |sm: &adw::StyleManager| {
            let name = if sm.is_dark() {
                "Adwaita-dark"
            } else {
                "Adwaita"
            };
            buffer.set_style_scheme(
                sourceview5::StyleSchemeManager::default()
                    .scheme(name)
                    .as_ref(),
            );
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
    let move_note = add("move-note", move_dialog);
    let delete = add("delete-note", delete_note);
    // Notes only while the notes are shown; a note's actions while one is open.
    let enable = {
        let ui = Rc::downgrade(ui);
        move || {
            let Some(ui) = ui.upgrade() else { return };
            let main = ui.stack.visible_child_name().as_deref() == Some("main");
            let open = main && ui.current.borrow().is_some();
            new.set_enabled(main);
            search.set_enabled(main);
            move_note.set_enabled(open);
            delete.set_enabled(open);
            ui.note_menu.set_sensitive(open);
        }
    };
    enable();
    let e = enable.clone();
    ui.stack.connect_visible_child_name_notify(move |_| e());
    ui.note_stack
        .connect_visible_child_name_notify(move |_| enable());

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
            Some(Item::Folder(f)) => {
                if !ui.expanded.borrow_mut().remove(&f) {
                    ui.expanded.borrow_mut().insert(f.clone());
                }
                save_expanded(&ui);
                *ui.target_folder.borrow_mut() = f;
                refresh_tree(&ui);
            }
            Some(Item::Note(n)) => {
                ui.search_button.set_active(false);
                open_note(&ui, &n);
                ui.split.set_show_content(true);
            }
            None => {}
        });
    });

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

    // Typing saves after a pause.
    let weak = Rc::downgrade(ui);
    ui.buffer.connect_changed(move |_| {
        let Some(ui) = weak.upgrade() else { return };
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
    let current = ui.current.borrow().as_ref().map(|n| n.id);
    for (depth, item) in &items {
        let open = matches!(item, Item::Folder(f) if ui.expanded.borrow().contains(f));
        let row = tree_row(*depth, item, open);
        ui.tree.append(&row);
        if matches!(item, Item::Note(n) if Some(n.id) == current) {
            ui.tree.select_row(Some(&row));
        }
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

/// A folder (its arrow showing whether it is open) or a note, indented by its depth.
fn tree_row(depth: usize, item: &Item, open: bool) -> gtk::ListBoxRow {
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
    let label = gtk::Label::new(Some(name));
    label.set_xalign(0.0);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    line.append(&label);
    let row = gtk::ListBoxRow::new();
    row.set_child(Some(&line));
    row.set_selectable(!folder);
    if let Item::Folder(f) = item {
        row.set_tooltip_text(Some(f));
    }
    row
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
    save_now(ui);
    let folder = Some(ui.target_folder.borrow().clone())
        .filter(|f| core().folders().contains(f))
        .unwrap_or_default();
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

fn delete_note(ui: &Rc<Ui>) {
    let Some(n) = ui.current.borrow().clone() else {
        return;
    };
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        let alert = adw::AlertDialog::new(
            Some(&gettext("Delete Note?")),
            Some(
                &gettext("“{title}” will be deleted on every device.").replace("{title}", &n.title),
            ),
        );
        alert.add_responses(&[
            ("cancel", &gettext("Cancel")),
            ("delete", &gettext("Delete")),
        ]);
        alert.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        alert.set_default_response(Some("cancel"));
        alert.set_close_response("cancel");
        if alert.choose_future(Some(&ui.window)).await != "delete" {
            return;
        }
        ui.unsaved.set(false);
        match core().delete_note(n.id) {
            Ok(()) => {
                close_note(&ui);
                refresh(&ui);
                request_sync();
            }
            Err(e) => toast(&e.to_string()),
        }
    });
}

/// Move the open note to another folder, or a new one.
fn move_dialog(ui: &Rc<Ui>) {
    let Some(n) = ui.current.borrow().clone() else {
        return;
    };
    let b = view!("move-note");
    let dialog: adw::AlertDialog = get(&b, "dialog");
    let row: adw::ComboRow = get(&b, "folder");
    let names: gtk::StringList = get(&b, "folders");
    let new_folder: adw::EntryRow = get(&b, "new_folder");
    let folders = core().folders();
    names.append(&gettext("Top Level"));
    for f in &folders {
        names.append(f);
    }
    let pos = folders.iter().position(|f| *f == n.folder);
    row.set_selected(pos.map_or(0, |p| p as u32 + 1));
    let ui = ui.clone();
    glib::spawn_future_local(async move {
        if dialog.choose_future(Some(&ui.window)).await != "move" {
            return;
        }
        let typed = new_folder.text().trim().to_string();
        let folder = if !typed.is_empty() {
            typed
        } else {
            (row.selected() as usize)
                .checked_sub(1)
                .and_then(|i| folders.get(i).cloned())
                .unwrap_or_default()
        };
        save_now(&ui);
        match core().move_note(n.id, folder, n.title.clone()) {
            Ok(_) => {
                refresh(&ui);
                reload_current(&ui);
                request_sync();
            }
            Err(notav_core::Error::Exists(_)) => toast(&gettext(
                "A note with this name exists in that folder already",
            )),
            Err(e) => toast(&e.to_string()),
        }
    });
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
