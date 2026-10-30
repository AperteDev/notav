//! Preferences dialog: General (style, the tree's density and icons), Editor (each mode's font,
//! what Editor mode changes by itself), Journal and Accounts (a page per account: switched on,
//! name, connection, notes folder, server status; settings sync).

use crate::core;
use crate::i18n::{gettext, pgettext};
use crate::window::{get, note, url_text, view};
use adw::prelude::*;
use gtk::{gio, glib};
use notav_core::{Account, AccountKind};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// A scheme picker and the rest of a URL.
type UrlRow = (gtk::DropDown, adw::EntryRow);

/// Light or dark as the device-only `color_scheme` says: "light", "dark", else the system's.
pub fn apply_color_scheme() {
    let scheme = match core().setting("color_scheme".into()).as_deref() {
        Some("light") => adw::ColorScheme::ForceLight,
        Some("dark") => adw::ColorScheme::ForceDark,
        _ => adw::ColorScheme::Default,
    };
    adw::StyleManager::default().set_color_scheme(scheme);
}

pub fn present(parent: &impl IsA<gtk::Widget>) {
    let b = view!("settings");
    let dialog: adw::PreferencesDialog = get(&b, "dialog");
    connect_accounts(&b, &dialog);
    let style: adw::ToggleGroup = get(&b, "color_scheme");
    let current = core().setting("color_scheme".into());
    style.set_active_name(Some(match current.as_deref() {
        Some("light") => "light",
        Some("dark") => "dark",
        _ => "system",
    }));
    style.connect_active_name_notify(|g| {
        let name = g.active_name().map(|n| n.to_string()).unwrap_or_default();
        let _ = core().set_setting("color_scheme".into(), name);
        apply_color_scheme();
    });
    let dense: adw::SwitchRow = get(&b, "dense_tree");
    dense.set_active(crate::window::dense_tree());
    dense.connect_active_notify(|s| {
        let _ = core().set_setting("dense_tree".into(), s.is_active().to_string());
        crate::window::apply_tree_look();
    });
    let icons: adw::ToggleGroup = get(&b, "tree_icons");
    let current = core().setting("tree_icons".into());
    icons.set_active_name(Some(match current.as_deref() {
        Some("all") => "all",
        Some("none") => "none",
        _ => "notes",
    }));
    icons.connect_active_name_notify(|g| {
        let name = g.active_name().map(|n| n.to_string()).unwrap_or_default();
        let _ = core().set_setting("tree_icons".into(), name);
        crate::window::apply_tree_look();
    });
    connect_journal(&b, &dialog);
    connect_font(&b, "editor_font");
    connect_font(&b, "source_font");
    let tables: adw::SwitchRow = get(&b, "format_tables");
    tables.set_active(crate::editor::format_tables());
    tables.connect_active_notify(|s| {
        let _ = core().set_setting("format_tables".into(), s.is_active().to_string());
    });
    dialog.present(Some(parent));
}

/// The Journal page: the daily notes' folder (made when new), template note and name format,
/// with today's note's path as they make it.
fn connect_journal(b: &gtk::Builder, dialog: &adw::PreferencesDialog) {
    let folder: adw::EntryRow = get(b, "journal_folder");
    let template: adw::EntryRow = get(b, "journal_template");
    let format: adw::EntryRow = get(b, "journal_format");
    let preview: adw::ActionRow = get(b, "journal_preview");
    let show = {
        let (folder, template, format) = (folder.clone(), template.clone(), format.clone());
        move || {
            // Only where it differs: setting a row's text shows its apply button again.
            let template_name = core().setting("journal_template".into());
            for (row, text) in [
                (&folder, core().journal_folder()),
                (&template, template_name.unwrap_or_default()),
                (&format, core().journal_format()),
            ] {
                // Text set here is no edit to apply: the button is off meanwhile.
                if row.text() != text {
                    row.set_show_apply_button(false);
                    row.set_text(&text);
                    row.set_show_apply_button(true);
                }
            }
            let (y, m, d) = crate::window::today();
            preview.set_subtitle(&glib::markup_escape_text(&format!(
                "{}.md",
                core().daily_path(y, m, d)
            )));
        }
    };
    show();
    // In the journal's account (the first switched on).
    let journal_notes = || {
        let account = core().journal_account();
        core()
            .notes()
            .into_iter()
            .filter(move |n| Some(n.account) == account)
    };
    suggest(&folder, || {
        core()
            .journal_account()
            .map(|a| core().folders(a))
            .unwrap_or_default()
    });
    suggest(&template, move || {
        journal_notes()
            .map(|n| n.path.strip_suffix(".md").unwrap_or(&n.path).to_string())
            .collect()
    });
    let (d, shown) = (dialog.clone(), show.clone());
    folder.connect_apply(move |row| {
        let f = row.text().trim().trim_matches('/').to_string();
        if !f.is_empty()
            && let Some(account) = core().journal_account()
            && let Err(e) = core().add_folder(account, f.clone())
        {
            d.add_toast(crate::window::note(&match e {
                notav_core::Error::Invalid(_) => gettext("Not a folder name"),
                e => e.to_string(),
            }));
            return shown();
        }
        let _ = core().set_setting("journal_folder".into(), f);
        crate::window::apply_tree_look(); // the folder, if new
        shown();
    });
    let (d, shown) = (dialog.clone(), show.clone());
    template.connect_apply(move |row| {
        let t = row.text().trim().trim_matches('/').to_string();
        let path = if t.ends_with(".md") {
            t.clone()
        } else {
            format!("{t}.md")
        };
        if !t.is_empty() && !journal_notes().any(|n| n.path == path) {
            d.add_toast(crate::window::note(
                &gettext("No note “{name}”").replace("{name}", &t),
            ));
            return shown();
        }
        let _ = core().set_setting("journal_template".into(), t);
        shown();
    });
    format.connect_apply(move |row| {
        let _ = core().set_setting("journal_format".into(), row.text().trim().to_string());
        show();
    });
}

/// Suggestions under `row` as it is typed in: those of `items()` with its text in them, a
/// click (or Down, then Enter) putting one in; what is typed can be anything else too.
fn suggest(row: &adw::EntryRow, items: impl Fn() -> Vec<String> + 'static) {
    let list = gtk::ListBox::new();
    list.add_css_class("navigation-sidebar");
    let scroll = gtk::ScrolledWindow::builder()
        .child(&list)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .max_content_height(240)
        .propagate_natural_height(true)
        .build();
    let popover = gtk::Popover::builder()
        .child(&scroll)
        .has_arrow(false)
        .autohide(false)
        .position(gtk::PositionType::Bottom)
        .halign(gtk::Align::Start)
        .build();
    popover.set_parent(row);
    row.connect_destroy({
        let popover = popover.clone();
        move |_| popover.unparent()
    });
    let (p, l) = (popover.clone(), list.clone());
    row.connect_changed(move |row| {
        let typed = row.text().to_lowercase();
        let found: Vec<String> = items()
            .into_iter()
            .filter(|_| !typed.is_empty())
            .filter(|i| i.to_lowercase().contains(&typed) && i.to_lowercase() != typed)
            .take(50)
            .collect();
        l.remove_all();
        for f in &found {
            let label = gtk::Label::builder().label(f).xalign(0.0).build();
            l.append(&label);
        }
        scroll.set_size_request(row.width(), -1);
        if found.is_empty() || !row.has_focus() && row.focus_child().is_none() {
            p.popdown();
        } else {
            p.popup();
        }
    });
    let (p, r) = (popover.clone(), row.clone());
    list.connect_row_activated(move |_, item| {
        if let Some(label) = item.child().and_downcast::<gtk::Label>() {
            r.set_text(&label.label());
            r.set_position(-1);
        }
        p.popdown();
        r.grab_focus();
    });
    // Down into the list; Escape or leaving closes it.
    let keys = gtk::EventControllerKey::new();
    let (p, l) = (popover.clone(), list.clone());
    keys.connect_key_pressed(move |_, key, _, _| match key {
        gtk::gdk::Key::Down if p.is_visible() => {
            if let Some(first) = l.row_at_index(0) {
                first.grab_focus();
            }
            glib::Propagation::Stop
        }
        gtk::gdk::Key::Escape if p.is_visible() => {
            p.popdown();
            glib::Propagation::Stop
        }
        _ => glib::Propagation::Proceed,
    });
    row.add_controller(keys);
    let focus = gtk::EventControllerFocus::new();
    let p = popover.clone();
    focus.connect_leave(move |_| {
        let p = p.clone();
        glib::idle_add_local_once(move || {
            if p.focus_child().is_none() {
                p.popdown();
            }
        });
    });
    row.add_controller(focus);
}

/// A device-only font setting, `editor_font` or `source_font` (a Pango font description), if
/// one is chosen.
pub fn font(key: &str) -> Option<gtk::pango::FontDescription> {
    core()
        .setting(key.into())
        .filter(|f| !f.is_empty())
        .map(|f| gtk::pango::FontDescription::from_string(&f))
}

/// A mode's font: the one chosen in `key`'s setting, or the default (shown, not chosen): the
/// interface's font in Editor mode, the system's monospace one in Source mode, at 12 pt. Source
/// mode offers monospace fonts only.
fn connect_font(b: &gtk::Builder, key: &'static str) {
    let source = key == "source_font";
    let button: gtk::FontDialogButton = get(b, key);
    let default: gtk::Button = get(b, &format!("{key}_default"));
    if source && let Some(dialog) = button.dialog() {
        // The dialog lists faces (at this level) or families.
        let monospace = gtk::CustomFilter::new(|o| {
            let family = match o.downcast_ref::<gtk::pango::FontFace>() {
                Some(face) => Some(face.family()),
                None => o.downcast_ref::<gtk::pango::FontFamily>().cloned(),
            };
            family.is_some_and(|f| f.is_monospace())
        });
        dialog.set_filter(Some(&monospace));
    }
    // Set while the button is given the font to show, which is no choice.
    let showing = Rc::new(std::cell::Cell::new(false));
    let show = {
        let (button, default, showing) = (button.clone(), default.clone(), showing.clone());
        move || {
            let chosen = font(key);
            default.set_visible(chosen.is_some());
            let shown = chosen.unwrap_or_else(|| {
                let name = if source {
                    "Monospace".into()
                } else {
                    gtk::Settings::default()
                        .and_then(|s| s.gtk_font_name())
                        .unwrap_or_default()
                };
                let mut f = gtk::pango::FontDescription::from_string(&name);
                f.set_size(12 * gtk::pango::SCALE);
                f
            });
            showing.set(true);
            button.set_font_desc(&shown);
            showing.set(false);
        }
    };
    show();
    let default_ = default.clone();
    button.connect_font_desc_notify(move |b| {
        let Some(font) = b.font_desc().filter(|_| !showing.get()) else {
            return;
        };
        let _ = core().set_setting(key.into(), font.to_string());
        crate::window::text_look_changed();
        default_.set_visible(true);
    });
    default.connect_clicked(move |_| {
        let _ = core().set_setting(key.into(), String::new());
        crate::window::text_look_changed();
        show();
    });
}

/// Account `id`'s notes folder: another one asks whether the notes move along (when there are
/// some), then the notes there are shown. One not chosen yet (the account added, its last step
/// left) is chosen here.
fn connect_folder(b: &gtk::Builder, dialog: &adw::PreferencesDialog, id: i64) {
    let row: adw::EntryRow = get(b, "notes_folder");
    row.set_text(&core().notes_folder(id));
    let d = dialog.clone();
    row.connect_apply(move |row| {
        let folder = row.text().trim().trim_matches('/').to_string();
        let old = core().notes_folder(id);
        let chosen = core().chosen_notes_folder(id).is_some();
        if folder.is_empty() || folder == old && chosen {
            row.set_text(&old);
            return;
        }
        let (d, row) = (d.clone(), row.clone());
        glib::spawn_future_local(async move {
            let has_notes = core().notes().iter().any(|n| n.account == id);
            let move_notes = if !chosen || !has_notes {
                false
            } else {
                let alert = adw::AlertDialog::new(
                    Some(&gettext("Move the Notes?")),
                    Some(
                        &gettext(
                            "Move your notes from “{old}” to “{new}”, or leave them there and show the notes in “{new}”?",
                        )
                        .replace("{old}", &old)
                        .replace("{new}", &folder),
                    ),
                );
                alert.add_responses(&[
                    ("cancel", &gettext("Cancel")),
                    ("keep", &gettext("Leave Them")),
                    ("move", &gettext("Move")),
                ]);
                alert.set_response_appearance("move", adw::ResponseAppearance::Suggested);
                alert.set_default_response(Some("move"));
                alert.set_close_response("cancel");
                match alert.choose_future(Some(&d)).await.as_str() {
                    "move" => true,
                    "keep" => false,
                    _ => {
                        row.set_text(&old);
                        return;
                    }
                }
            };
            crate::window::save();
            row.set_sensitive(false);
            let f = folder.clone();
            let r = gio::spawn_blocking(move || core().set_notes_folder(id, f, move_notes)).await;
            row.set_sensitive(true);
            let msg = match r {
                Ok(Ok(())) => {
                    crate::sync_now();
                    crate::window::account_changed();
                    gettext("Notes folder changed to “{folder}”").replace("{folder}", &folder)
                }
                Ok(Err(notav_core::Error::Invalid(_))) => gettext("Not a folder name"),
                Ok(Err(e)) => gettext("Notes folder not changed: {error}")
                    .replace("{error}", &e.to_string()),
                Err(_) => gettext("Notes folder not changed"),
            };
            row.set_text(&core().notes_folder(id));
            d.add_toast(note(&msg));
        });
    });
}

thread_local! {
    /// Shows the sync state on the open account page; set while Preferences are open.
    static STATUS: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
    /// Shows the accounts in the open Preferences anew.
    static ACCOUNTS: RefCell<Option<Rc<dyn Fn()>>> = const { RefCell::new(None) };
    /// The account row a dragged one would go next to, and the widget showing the line.
    static DROP_MARK: RefCell<Option<(gtk::Widget, gtk::Widget, &'static str)>> =
        const { RefCell::new(None) };
}

/// Accounts came, went or changed: the open Preferences show them.
pub fn accounts_changed() {
    if let Some(show) = ACCOUNTS.with_borrow(Clone::clone) {
        show();
    }
}

/// A sync started or ended: the open Preferences show it.
pub fn status_changed() {
    if let Some(show) = STATUS.with_borrow(Clone::clone) {
        show();
    }
}

/// One row of "Server Status": its icon (none, "success", "warning" or "error") and subtitle.
fn set_status(row: &adw::ActionRow, icon: &gtk::Image, look: Option<&str>, text: &str) {
    row.set_subtitle(text);
    icon.set_visible(look.is_some());
    for class in ["success", "warning", "error"] {
        icon.remove_css_class(class);
    }
    if let Some(look) = look {
        icon.set_icon_name(Some(match look {
            "success" => "object-select-symbolic",
            "error" => "dialog-error-symbolic",
            _ => "dialog-warning-symbolic",
        }));
        icon.add_css_class(look);
    }
}

/// The "Server Status" group of account `id`: the server (asked each time the returned closure
/// runs) and the last sync, the latter kept up to date while the page is open.
fn connect_status(b: &gtk::Builder, id: i64) -> Rc<dyn Fn()> {
    let (server, server_icon): (adw::ActionRow, gtk::Image) =
        (get(b, "server_row"), get(b, "server_icon"));
    let (sync, sync_icon): (adw::ActionRow, gtk::Image) = (get(b, "sync_row"), get(b, "sync_icon"));
    let show = Rc::new(move || {
        let last = crate::last_sync(id).map(|t| {
            let time = glib::DateTime::from_unix_local(t)
                .and_then(|d| d.format("%X"))
                .map(|d| d.to_string())
                .unwrap_or_default();
            gettext("Last synced: {time}").replace("{time}", &time)
        });
        match crate::sync_error(id) {
            _ if crate::sync_state() == "syncing" => {
                set_status(&sync, &sync_icon, None, &gettext("Syncing…"))
            }
            _ if core().chosen_notes_folder(id).is_none() => set_status(
                &sync,
                &sync_icon,
                Some("warning"),
                &gettext("Choose its notes folder below to sync it"),
            ),
            Some(e) => set_status(
                &sync,
                &sync_icon,
                Some("error"),
                &gettext("Failed: {error}").replace("{error}", &e),
            ),
            None => match &last {
                Some(l) => set_status(&sync, &sync_icon, Some("success"), l),
                None => set_status(&sync, &sync_icon, None, &gettext("Not synced yet")),
            },
        }
    });
    STATUS.set(Some(show.clone()));

    Rc::new(move || {
        show();
        set_status(&server, &server_icon, None, &gettext("Checking…"));
        let (server, icon) = (server.downgrade(), server_icon.downgrade());
        glib::spawn_future_local(async move {
            let r = gio::spawn_blocking(move || core().server_info(id)).await;
            let (Some(server), Some(icon)) = (server.upgrade(), icon.upgrade()) else {
                return;
            };
            match r {
                Ok(Ok(info)) => {
                    let name = format!("{} {}", info.product, info.version);
                    if info.maintenance {
                        let text =
                            gettext("{server}, in maintenance mode").replace("{server}", &name);
                        set_status(&server, &icon, Some("warning"), &text);
                    } else {
                        set_status(&server, &icon, Some("success"), &name);
                    }
                }
                Ok(Err(e)) => set_status(
                    &server,
                    &icon,
                    Some("error"),
                    &gettext("Cannot reach the server: {error}").replace("{error}", &e.to_string()),
                ),
                Err(_) => {}
            }
        });
    })
}

/// The "Accounts" group: a row per account opening its page, and Add; with the settings sync
/// choice, shown anew when accounts change.
fn connect_accounts(b: &gtk::Builder, dialog: &adw::PreferencesDialog) {
    let group: adw::PreferencesGroup = get(b, "accounts");
    let rows: RefCell<Vec<adw::ActionRow>> = RefCell::default();
    let settings_sync = connect_settings_sync(b);
    let d = dialog.downgrade();
    let show: Rc<dyn Fn()> = Rc::new(move || {
        for r in rows.borrow_mut().drain(..) {
            group.remove(&r);
        }
        let accounts = core().accounts();
        for a in &accounts {
            let row = adw::ActionRow::builder()
                .title(glib::markup_escape_text(&crate::account_name(a)))
                .subtitle(account_state(a))
                .activatable(true)
                .build();
            if accounts.len() > 1 {
                connect_reorder(&row, a.id, &crate::account_name(a));
            }
            row.add_prefix(&gtk::Image::from_icon_name(account_icon(a)));
            let lost = a.kind == AccountKind::Nextcloud && !core().connected(a.id);
            if a.enabled && (lost || core().chosen_notes_folder(a.id).is_none() && !is_local(a)) {
                let warning = gtk::Image::from_icon_name("dialog-warning-symbolic");
                warning.add_css_class("warning");
                row.add_suffix(&warning);
            }
            row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
            let (d, id) = (d.clone(), a.id);
            row.connect_activated(move |_| {
                if let Some(d) = d.upgrade() {
                    d.push_subpage(&account_page(&d, id));
                }
            });
            group.add(&row);
            rows.borrow_mut().push(row);
        }
        settings_sync();
    });
    show();
    ACCOUNTS.set(Some(show));
    dialog.connect_closed(|_| {
        ACCOUNTS.set(None);
        STATUS.set(None);
        // A sign-in still waiting for the browser stops with the dialog.
        crate::window::cancel_login();
    });
    let d = dialog.clone();
    get::<gtk::Button>(b, "add_account").connect_clicked(move |_| {
        let d2 = d.clone();
        crate::add_account::dialog(&d, move |id| {
            accounts_changed();
            crate::window::account_added(id);
            if let Some(a) = core().account(id) {
                let name = crate::account_name(&a);
                d2.add_toast(note(&gettext("{name} added").replace("{name}", &name)));
            }
        });
    });
}

fn is_local(a: &Account) -> bool {
    a.kind == AccountKind::Local
}

/// Account rows move by their handle: a dragged row goes above or below the one it is dropped
/// on (its upper or lower half), and the tree follows the new order.
fn connect_reorder(row: &adw::ActionRow, id: i64, title: &str) {
    let handle = gtk::Image::from_icon_name("list-drag-handle-symbolic");
    handle.set_tooltip_text(Some(&gettext("Drag to Move")));
    handle.set_cursor_from_name(Some("grab"));
    let drag = gtk::DragSource::new();
    drag.set_actions(gtk::gdk::DragAction::MOVE);
    drag.set_content(Some(&gtk::gdk::ContentProvider::for_value(&id.to_value())));
    let (r, title) = (row.clone(), title.to_string());
    drag.connect_drag_begin(move |_, drag| {
        // The account's name on a card; the row stays in place, faded.
        let card = gtk::Label::new(Some(&title));
        card.add_css_class("drag-card");
        if let Ok(icon) = gtk::DragIcon::for_drag(drag).downcast::<gtk::DragIcon>() {
            icon.set_child(Some(&card));
        }
        r.add_css_class("dragged");
    });
    let r = row.clone();
    drag.connect_drag_end(move |_, _, _| {
        r.remove_css_class("dragged");
        clear_mark();
    });
    handle.add_controller(drag);
    row.add_prefix(&handle);

    let below =
        |dt: &gtk::DropTarget, y: f64| dt.widget().is_some_and(|w| y > f64::from(w.height()) / 2.0);
    let drop = gtk::DropTarget::new(i64::static_type(), gtk::gdk::DragAction::MOVE);
    drop.connect_motion(move |dt, _, y| {
        if let Some(w) = dt.widget() {
            mark(&w, below(dt, y));
        }
        gtk::gdk::DragAction::MOVE
    });
    drop.connect_leave(|dt| {
        if let Some(w) = dt.widget() {
            unmark(&w);
        }
    });
    drop.connect_drop(move |dt, value, _, y| {
        if let Some(w) = dt.widget() {
            unmark(&w);
        }
        let Ok(moved) = value.get::<i64>() else {
            return false;
        };
        if moved == id {
            return false;
        }
        let mut ids: Vec<i64> = core().accounts().iter().map(|a| a.id).collect();
        ids.retain(|i| *i != moved);
        let at = ids.iter().position(|i| *i == id).unwrap_or(ids.len());
        ids.insert(at + usize::from(below(dt, y)), moved);
        let _ = core().reorder_accounts(ids);
        // After the drop is done: the rows are made anew.
        glib::idle_add_local_once(|| {
            accounts_changed();
            crate::window::account_changed();
        });
        true
    });
    row.add_controller(drop);
}

/// A line where a dragged account row would go: above `owner`, or below it (drawn as the next
/// row's top line, so it does not jump between the two).
fn mark(owner: &gtk::Widget, below: bool) {
    let next = owner.next_sibling().filter(|_| below);
    let (w, class) = match next {
        Some(next) => (next, "drop-above"),
        None => (
            owner.clone(),
            if below { "drop-below" } else { "drop-above" },
        ),
    };
    DROP_MARK.with_borrow_mut(|m| {
        if let Some((_, old, c)) = m.take() {
            old.remove_css_class(c);
        }
        w.add_css_class(class);
        *m = Some((owner.clone(), w, class));
    });
}

/// Remove the line `owner` drew; one the row now entered drew stays.
fn unmark(owner: &gtk::Widget) {
    DROP_MARK.with_borrow_mut(|m| {
        if m.as_ref().is_some_and(|(o, _, _)| o == owner) {
            let (_, w, c) = m.take().unwrap();
            w.remove_css_class(c);
        }
    });
}

/// Remove any drop line, at a drag's end: cancelled drags get no leave.
fn clear_mark() {
    if let Some((_, w, c)) = DROP_MARK.take() {
        w.remove_css_class(c);
    }
}

/// An account's icon: this device, a server signed in to here, or GNOME Online Accounts.
pub fn account_icon(a: &Account) -> &'static str {
    match a.kind {
        AccountKind::Local => "computer-symbolic",
        AccountKind::Nextcloud if a.source.is_empty() => "network-server-symbolic",
        AccountKind::Nextcloud => "goa-panel-symbolic",
    }
}

/// An account row's subtitle: what it is, or why it does not sync.
fn account_state(a: &Account) -> String {
    match a.kind {
        _ if !a.enabled => pgettext("account", "Off"),
        AccountKind::Local => gettext("Notes that stay on this computer"),
        _ if !core().connected(a.id) => gettext("Not connected"),
        _ if core().chosen_notes_folder(a.id).is_none() => gettext("No notes folder chosen"),
        AccountKind::Nextcloud if !a.source.is_empty() => gettext("GNOME Online Accounts"),
        AccountKind::Nextcloud => gettext("Nextcloud server"),
    }
}

/// "Sync settings with": off, or a Nextcloud account; the returned closure fills it anew.
fn connect_settings_sync(b: &gtk::Builder) -> impl Fn() + 'static {
    let row: adw::ComboRow = get(b, "settings_account");
    let names: gtk::StringList = get(b, "settings_accounts");
    let ids: Rc<RefCell<Vec<Option<i64>>>> = Rc::default();
    let filling = Rc::new(Cell::new(false));
    let (i, f) = (ids.clone(), filling.clone());
    row.connect_selected_notify(move |r| {
        if f.get() {
            return;
        }
        let id = i.borrow().get(r.selected() as usize).copied().flatten();
        let _ = core().set_settings_account(id);
        crate::sync_now(); // pulls that account's settings right away
    });
    move || {
        filling.set(true);
        let mut v = vec![None];
        let mut labels = vec![pgettext("settings sync", "Off")];
        for a in core().accounts() {
            if a.kind == AccountKind::Nextcloud {
                v.push(Some(a.id));
                labels.push(crate::account_name(&a));
            }
        }
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        names.splice(0, names.n_items(), &labels);
        let current = core().settings_account();
        row.set_selected(v.iter().position(|i| *i == current).unwrap_or(0) as u32);
        *ids.borrow_mut() = v;
        filling.set(false);
    }
}

/// Account `id`'s page: switched on, its name; for a Nextcloud account how it is connected
/// (signed in here: sign in again, the server's address), its notes folder and the server
/// status; Remove.
fn account_page(dialog: &adw::PreferencesDialog, id: i64) -> adw::NavigationPage {
    let b = view!("account");
    let page: adw::NavigationPage = get(&b, "page");
    let Some(a) = core().account(id) else {
        return page;
    };
    page.set_title(&crate::account_name(&a));
    let remote = a.kind == AccountKind::Nextcloud;
    let manual = remote && a.source.is_empty();
    get::<gtk::Widget>(&b, "connection").set_visible(remote);
    get::<gtk::Widget>(&b, "notes").set_visible(remote);
    if remote {
        connect_folder(&b, dialog, id);
    }

    let enabled: adw::SwitchRow = get(&b, "enabled");
    enabled.set_active(a.enabled);
    enabled.connect_active_notify(move |r| {
        let on = r.is_active();
        glib::spawn_future_local(async move {
            crate::enable_account(id, on).await;
            accounts_changed();
            status_changed();
        });
    });

    // Shown with the name it has without one; applying that name gives that back.
    let name: adw::EntryRow = get(&b, "name");
    name.set_text(&crate::account_name(&a));
    let p = page.clone();
    name.connect_apply(move |e| {
        let Some(mut a) = core().account(id) else {
            return;
        };
        a.name = String::new();
        let text = e.text().trim().to_string();
        let given = if text == crate::account_name(&a) {
            String::new()
        } else {
            text
        };
        let _ = core().rename_account(id, given);
        if let Some(a) = core().account(id) {
            p.set_title(&crate::account_name(&a));
            e.set_text(&crate::account_name(&a));
        }
        accounts_changed();
        crate::window::account_changed();
    });

    let status: adw::ActionRow = get(&b, "status");
    let icon: gtk::Image = get(&b, "status_icon");
    let sign_in: adw::SplitButton = get(&b, "sign_in");
    let cancel: gtk::Button = get(&b, "cancel_sign_in");
    let server: UrlRow = (get(&b, "server_scheme"), get(&b, "server"));
    let server_status: adw::PreferencesGroup = get(&b, "server_status");
    server_status.set_visible(false); // a synced account's, while connected (`describe`)
    set_url(&server, &a.url);
    server.1.set_visible(manual);
    sign_in.set_visible(manual);
    let check_server: Rc<dyn Fn()> = if remote {
        connect_status(&b, id)
    } else {
        Rc::new(|| {})
    };
    let describe: Rc<dyn Fn()> = {
        let (status, icon) = (status.clone(), icon.clone());
        Rc::new(move || {
            let Some(a) = core().account(id) else {
                return;
            };
            let connected = core().connected(id);
            let host = crate::host(&a);
            if !connected {
                status.set_title(&gettext("Not connected"));
                status.set_subtitle(&if manual {
                    gettext("Sign in again to sync it")
                } else {
                    gettext("Check the account in Settings → Online Accounts")
                });
            } else if manual {
                status.set_title(&gettext("Signed in as {name}").replace("{name}", &a.user));
                status.set_subtitle(host);
            } else {
                status.set_title(&gettext("Connected as {name}").replace("{name}", &a.user));
                status.set_subtitle(&format!("{host} · {}", gettext("GNOME Online Accounts")));
            }
            icon.set_icon_name(Some(if connected {
                "object-select-symbolic"
            } else {
                "dialog-warning-symbolic"
            }));
            for (class, on) in [("success", connected), ("warning", !connected)] {
                if on {
                    icon.add_css_class(class);
                } else {
                    icon.remove_css_class(class);
                }
            }
            server_status.set_visible(connected && a.enabled);
            if connected && a.enabled {
                check_server();
            }
        })
    };
    if remote {
        describe();
    }

    // A new app password for the account (the same user on its server).
    let reconnect = {
        let (d, describe) = (dialog.downgrade(), describe.clone());
        move |c: notav_core::Credentials| {
            let (d, describe) = (d.clone(), describe.clone());
            glib::spawn_future_local(async move {
                let Some(a) = core().account(id) else { return };
                let msg = if c.login_name != a.user {
                    gettext("Signed in as {name}: add that as another account")
                        .replace("{name}", &c.login_name)
                } else {
                    match crate::reconnect(id, a.url.clone(), c.app_password).await {
                        Ok(()) => gettext("Signed in as {name}").replace("{name}", &a.user),
                        Err(e) => e,
                    }
                };
                describe();
                accounts_changed();
                crate::window::account_changed();
                if let Some(d) = d.upgrade() {
                    d.add_toast(note(&msg));
                }
            });
        }
    };
    let reset = {
        let (sign_in, cancel) = (sign_in.clone(), cancel.clone());
        let label = sign_in.label();
        move || {
            sign_in.set_sensitive(true);
            sign_in.set_label(label.as_deref().unwrap_or_default());
            cancel.set_visible(false);
        }
    };
    let r = reset.clone();
    cancel.connect_clicked(move |_| {
        crate::window::cancel_login();
        r();
    });
    let start = {
        let (sign_in, cancel, d, reconnect) = (
            sign_in.clone(),
            cancel.clone(),
            dialog.downgrade(),
            reconnect.clone(),
        );
        move |copy: bool| {
            sign_in.set_sensitive(false);
            sign_in.set_label(&if copy {
                gettext("Waiting for Sign-In…")
            } else {
                gettext("Waiting for Browser…")
            });
            cancel.set_visible(true);
            let (d, reset, reconnect) = (d.clone(), reset.clone(), reconnect.clone());
            let url = core().account(id).map(|a| a.url).unwrap_or_default();
            glib::spawn_future_local(async move {
                match crate::window::browser_login(url, copy).await {
                    Ok(Some(c)) => reconnect(c),
                    Ok(None) => return, // cancelled: Cancel or closing the dialog reset it
                    Err(e) => {
                        if let Some(d) = d.upgrade() {
                            d.add_toast(note(&e));
                        }
                    }
                }
                reset();
            });
        }
    };
    let actions = gio::SimpleActionGroup::new();
    let copy = gio::SimpleAction::new("sign-in-copy", None);
    let s = start.clone();
    copy.connect_activate(move |_, _| s(true));
    actions.add_action(&copy);
    // Or with the app password typed in.
    let password = gio::SimpleAction::new("sign-in-password", None);
    let d = dialog.downgrade();
    password.connect_activate(move |_, _| {
        let (Some(d), Some(a)) = (d.upgrade(), core().account(id)) else {
            return;
        };
        crate::window::password_login(&d, a.url, Some(a.user), reconnect.clone());
    });
    actions.add_action(&password);
    page.insert_action_group("account", Some(&actions));
    sign_in.connect_clicked(move |_| start(false));

    // Another address: the same account on a moved server.
    let (d, describe_, row) = (dialog.downgrade(), describe.clone(), server.1.clone());
    row.connect_apply(move |_| {
        let Some(a) = core().account(id) else { return };
        let url = url_text(&server);
        if url.is_empty() || url == a.url {
            return;
        }
        let (d, describe) = (d.clone(), describe_.clone());
        glib::spawn_future_local(async move {
            let msg = match crate::password_load(&a.url, &a.user) {
                None => gettext("No saved password: sign in again first"),
                Some(pw) => match crate::reconnect(id, url, pw).await {
                    Ok(()) => gettext("Nextcloud server changed"),
                    Err(e) => e,
                },
            };
            describe();
            accounts_changed();
            crate::window::account_changed();
            if let Some(d) = d.upgrade() {
                d.add_toast(note(&msg));
            }
        });
    });

    let d = dialog.downgrade();
    get::<gtk::Button>(&b, "remove").connect_clicked(move |_| {
        if let Some(d) = d.upgrade() {
            glib::spawn_future_local(remove_account(d, id));
        }
    });
    page
}

/// Remove account `id` after asking; when its changes cannot be uploaded first, asks again
/// whether to lose them.
async fn remove_account(dialog: adw::PreferencesDialog, id: i64) {
    let Some(a) = core().account(id) else { return };
    let name = crate::account_name(&a);
    let body = match a.kind {
        AccountKind::Local => gettext(
            "Its notes are deleted from this device, with everything in them. This cannot be \
             undone.",
        ),
        AccountKind::Nextcloud => gettext(
            "Its notes leave this device; they stay on the server. Switch it off instead to \
             only hide them.",
        ),
    };
    let ask = |heading: String, body: String, yes: String| {
        let alert = adw::AlertDialog::new(Some(&heading), Some(&body));
        alert.add_responses(&[("cancel", &gettext("Cancel")), ("remove", &yes)]);
        alert.set_response_appearance("remove", adw::ResponseAppearance::Destructive);
        alert.set_default_response(Some("cancel"));
        alert.set_close_response("cancel");
        alert
    };
    let alert = ask(
        gettext("Remove {name}?").replace("{name}", &name),
        body,
        gettext("Remove"),
    );
    if alert.choose_future(Some(&dialog)).await != "remove" {
        return;
    }
    let mut r = crate::remove_account(id, false).await;
    if let Err(e) = &r {
        let alert = ask(
            gettext("Remove Without Uploading?"),
            gettext("{error}. Changes made here that the server does not have yet will be lost.")
                .replace("{error}", e),
            gettext("Remove Anyway"),
        );
        if alert.choose_future(Some(&dialog)).await != "remove" {
            return;
        }
        r = crate::remove_account(id, true).await;
    }
    match r {
        Ok(()) if core().accounts().is_empty() => {
            dialog.close();
        }
        Ok(()) => {
            dialog.pop_subpage();
            accounts_changed();
            dialog.add_toast(note(&gettext("{name} removed").replace("{name}", &name)));
        }
        Err(e) => dialog.add_toast(note(&e)),
    }
}

/// Fill a scheme picker and host entry from a full URL.
fn set_url((scheme, row): &UrlRow, url: &str) {
    match url.strip_prefix("http://") {
        Some(host) => {
            scheme.set_selected(1);
            row.set_text(host);
        }
        None => row.set_text(url.strip_prefix("https://").unwrap_or(url)),
    }
}
