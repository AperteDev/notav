//! Editor mode: the note styled from the core's Markdown parse (`notav_core::markdown`), its
//! marks still there, dimmed. Source mode is GtkSourceView's own highlighting
//! (`sourceview/notav-markdown.lang`).

use crate::i18n::gettext;
use gtk::prelude::*;
use gtk::{gio, glib};
use notav_core::markdown::{self, Style};
use notav_core::table::{self, Align, Op};
use sourceview5::prelude::*;
use std::rc::Rc;

/// Above this many characters a note is not styled: each change parses it whole.
const MAX_CHARS: i32 = 300_000;

/// The mark where text was last changed: its table is tidied when the cursor leaves its line.
const EDITED: &str = "notav-edited";

/// The table menu's actions (`table.<name>`).
const TABLE_OPS: &[(&str, Op)] = &[
    ("row-above", Op::RowAbove),
    ("row-below", Op::RowBelow),
    ("move-row-up", Op::MoveRowUp),
    ("move-row-down", Op::MoveRowDown),
    ("delete-row", Op::DeleteRow),
    ("column-left", Op::ColumnLeft),
    ("column-right", Op::ColumnRight),
    ("move-column-left", Op::MoveColumnLeft),
    ("move-column-right", Op::MoveColumnRight),
    ("delete-column", Op::DeleteColumn),
    ("align-left", Op::Align(Align::Left)),
    ("align-center", Op::Align(Align::Center)),
    ("align-right", Op::Align(Align::Right)),
    ("sort-ascending", Op::Sort { descending: false }),
    ("sort-descending", Op::Sort { descending: true }),
    ("format", Op::Tidy),
];

/// Editor mode's tags and the style scheme's styles they look like (`sourceview/notav.xml`),
/// lowest priority first: a later tag wins where they overlap (the dimmed marks over all).
const TAGS: &[(&str, &str)] = &[
    ("md-heading1", "notav-markdown:heading1"),
    ("md-heading2", "notav-markdown:heading2"),
    ("md-heading3", "notav-markdown:heading3"),
    ("md-heading4", "notav-markdown:heading4"),
    ("md-heading5", "notav-markdown:heading5"),
    ("md-heading6", "notav-markdown:heading6"),
    ("md-table", ""),
    ("md-table-header", "notav-editor:table-header"),
    ("md-quote", "notav-editor:quote"),
    ("md-code-block", "notav-editor:code-block"),
    ("md-strong", "notav-editor:strong"),
    ("md-emphasis", "notav-editor:emphasis"),
    ("md-strikethrough", "notav-editor:strikethrough"),
    ("md-code", "notav-editor:code"),
    ("md-link", "notav-editor:link"),
    ("md-list-mark", "notav-editor:list-mark"),
    ("md-task", "notav-editor:task"),
    ("md-mark", "notav-editor:mark"),
];

fn tag(style: Style) -> &'static str {
    match style {
        Style::Heading(1) => "md-heading1",
        Style::Heading(2) => "md-heading2",
        Style::Heading(3) => "md-heading3",
        Style::Heading(4) => "md-heading4",
        Style::Heading(5) => "md-heading5",
        Style::Heading(_) => "md-heading6",
        Style::Strong => "md-strong",
        Style::Emphasis => "md-emphasis",
        Style::Strikethrough => "md-strikethrough",
        Style::Code => "md-code",
        Style::CodeBlock => "md-code-block",
        Style::Quote => "md-quote",
        Style::Link => "md-link",
        Style::ListMark => "md-list-mark",
        Style::Task(_) => "md-task",
        Style::Table => "md-table",
        Style::TableHeader => "md-table-header",
        Style::Mark => "md-mark",
    }
}

/// Add Editor mode's tags to `buffer`; `look` gives them their colours.
pub fn setup(buffer: &sourceview5::Buffer) {
    let table = buffer.tag_table();
    for (name, _) in TAGS {
        table.add(&gtk::TextTag::new(Some(name)));
    }
    // What a style scheme cannot say. Tables in monospace, so their columns line up.
    for name in ["md-code", "md-code-block", "md-table"] {
        if let Some(t) = table.lookup(name) {
            t.set_family(Some("monospace"));
        }
    }
    if let Some(t) = table.lookup("md-quote") {
        t.set_left_margin(16);
    }
}

/// What a click and keys do in Editor mode: a click ticks a task, Ctrl+click opens a link,
/// Enter continues a list.
pub fn connect(view: &sourceview5::View) {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(|keys, key, _, state| {
        use gtk::gdk::Key;
        let Some(view) = keys.widget().and_downcast::<sourceview5::View>() else {
            return glib::Propagation::Proceed;
        };
        let buffer = view.buffer().downcast::<sourceview5::Buffer>().unwrap();
        let plain = !state.intersects(gtk::accelerator_get_default_mod_mask());
        if !matches!(key, Key::Return | Key::KP_Enter)
            || !plain
            || buffer.has_selection()
            || !editor_mode(&buffer)
        {
            return glib::Propagation::Proceed;
        }
        let (start, end) = buffer.bounds();
        let edits = markdown::enter(
            &buffer.text(&start, &end, true),
            buffer.cursor_position() as usize,
        );
        if edits.is_empty() {
            return glib::Propagation::Proceed;
        }
        apply(&buffer, &edits);
        glib::Propagation::Stop
    });
    view.add_controller(keys);
    // Before the text view's own, which would move the cursor there.
    let click = gtk::GestureClick::new();
    click.set_button(gtk::gdk::BUTTON_PRIMARY);
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    click.connect_pressed(|gesture, n, x, y| {
        let Some(view) = gesture.widget().and_downcast::<sourceview5::View>() else {
            return;
        };
        let mods = gesture.current_event_state() & gtk::accelerator_get_default_mod_mask();
        let done = n == 1
            && match mods {
                m if m.is_empty() => toggle_task(&view, x, y),
                gtk::gdk::ModifierType::CONTROL_MASK => open_link(&view, x, y),
                _ => false,
            };
        if done {
            gesture.set_state(gtk::EventSequenceState::Claimed);
        }
    });
    view.add_controller(click);
    // A hand over what a click acts on: a task, a link with Ctrl.
    let motion = gtk::EventControllerMotion::new();
    motion.connect_motion(|motion, x, y| {
        let Some(view) = motion.widget().and_downcast::<sourceview5::View>() else {
            return;
        };
        let ctrl = motion
            .current_event_state()
            .contains(gtk::gdk::ModifierType::CONTROL_MASK);
        let hand =
            task_at(&view, x, y).is_some() || (ctrl && tagged_at(&view, "md-link", x, y).is_some());
        view.set_cursor_from_name(Some(if hand { "pointer" } else { "text" }));
    });
    view.add_controller(motion);
    tables(view);
}

/// Forget where text was changed: the buffer holds another note (or the same, reloaded).
pub fn forget(buffer: &sourceview5::Buffer) {
    if let Some(m) = buffer.mark(EDITED) {
        buffer.delete_mark(&m);
    }
}

/// The context menu's table items (on a row or column, or a new table), tables tidied when
/// the cursor leaves a changed row, and spreadsheet rows pasted as a table.
fn tables(view: &sourceview5::View) {
    let group = gio::SimpleActionGroup::new();
    let weak = view.downgrade();
    let action = gio::SimpleAction::new("insert", None);
    action.connect_activate(move |_, _| {
        if let Some(view) = weak.upgrade() {
            let (buffer, text, cursor) = state(&view);
            if let Some(c) = table::insert(&text, cursor, &table::empty()) {
                change(&buffer, &c);
            }
        }
    });
    group.add_action(&action);
    for (name, op) in TABLE_OPS {
        let action = gio::SimpleAction::new(name, None);
        let weak = view.downgrade();
        action.connect_activate(move |_, _| {
            if let Some(view) = weak.upgrade() {
                let (buffer, text, cursor) = state(&view);
                if let Some(c) = table::apply(&text, cursor, cursor, *op) {
                    change(&buffer, &c);
                }
            }
        });
        group.add_action(&action);
    }
    view.insert_action_group("table", Some(&group));
    let menus = Rc::new((table_menu(), insert_menu()));

    // The menu for where it opens: the cursor, put at the right click first.
    let update = {
        let menus = menus.clone();
        move |view: &sourceview5::View| {
            let (buffer, text, cursor) = state(view);
            let editor = editor_mode(&buffer);
            let inside = editor && table::is_at(&text, cursor);
            view.set_extra_menu(match (editor, inside) {
                (false, _) => None,
                (true, true) => Some(&menus.0),
                (true, false) => Some(&menus.1),
            });
            for (name, op) in TABLE_OPS {
                let enabled = inside && table::apply(&text, cursor, cursor, *op).is_some();
                enable(&group, name, enabled);
            }
            enable(&group, "insert", editor && !inside);
        }
    };
    let click = gtk::GestureClick::new();
    click.set_button(gtk::gdk::BUTTON_SECONDARY);
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let on_click = update.clone();
    click.connect_pressed(move |gesture, _, x, y| {
        let Some(view) = gesture.widget().and_downcast::<sourceview5::View>() else {
            return;
        };
        let buffer = view.buffer();
        if let Some(at) = iter_at(&view, x, y)
            && !buffer
                .selection_bounds()
                .is_some_and(|(s, e)| at.in_range(&s, &e))
        {
            buffer.place_cursor(&at);
        }
        on_click(&view);
    });
    view.add_controller(click);
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(move |keys, key, _, state| {
        use gtk::gdk::{Key, ModifierType};
        let menu_key =
            key == Key::Menu || (key == Key::F10 && state.contains(ModifierType::SHIFT_MASK));
        if let (true, Some(view)) = (menu_key, keys.widget().and_downcast()) {
            update(&view);
        }
        glib::Propagation::Proceed
    });
    view.add_controller(keys);

    let buffer = view.buffer();
    // A change that took the cursor off the changed row (Enter) does not move it.
    buffer.connect_end_user_action(|b| {
        let at = b.iter_at_mark(&b.get_insert());
        match b.mark(EDITED) {
            Some(m) if b.iter_at_mark(&m).line() != at.line() => {
                let b = b.downcast_ref::<sourceview5::Buffer>().unwrap().clone();
                glib::idle_add_local_once(move || tidy_left(&b));
            }
            Some(m) => b.move_mark(&m, &at),
            None => {
                b.create_mark(Some(EDITED), &at, true);
            }
        }
    });
    buffer.connect_mark_set(|b, _, mark| {
        if *mark == b.get_insert() {
            let b = b.downcast_ref::<sourceview5::Buffer>().unwrap().clone();
            glib::idle_add_local_once(move || tidy_left(&b));
        }
    });

    view.connect_paste_clipboard(|view| {
        if !editor_mode(&view.buffer().downcast().unwrap()) {
            return;
        }
        view.stop_signal_emission_by_name("paste-clipboard");
        let view = view.clone();
        view.clipboard()
            .read_text_async(gio::Cancellable::NONE, move |pasted| {
                let (buffer, text, cursor) = state(&view);
                let editable = view.is_editable();
                let rows = pasted.ok().flatten().and_then(|p| table::from_tsv(&p));
                let Some(rows) = rows.filter(|_| !table::is_at(&text, cursor)) else {
                    buffer.paste_clipboard(&view.clipboard(), None, editable);
                    return;
                };
                buffer.begin_user_action();
                buffer.delete_selection(true, editable);
                let (_, text, cursor) = state(&view);
                if let Some(c) = table::insert(&text, cursor, &rows) {
                    change(&buffer, &c);
                }
                buffer.end_user_action();
            });
    });
}

/// The synced `format_tables` setting (on unless "false"): tables tidied by themselves.
pub fn format_tables() -> bool {
    crate::core().setting("format_tables".into()).as_deref() != Some("false")
}

/// Tidy the table of the row last changed, once the cursor has left that row (if
/// `format_tables`).
fn tidy_left(buffer: &sourceview5::Buffer) {
    let Some(mark) = buffer.mark(EDITED) else {
        return;
    };
    let (edited, cursor) = (
        buffer.iter_at_mark(&mark),
        buffer.iter_at_mark(&buffer.get_insert()),
    );
    if buffer.has_selection() || edited.line() == cursor.line() {
        return;
    }
    buffer.delete_mark(&mark);
    if !editor_mode(buffer) || !format_tables() {
        return;
    }
    let (start, end) = buffer.bounds();
    let text = buffer.text(&start, &end, true);
    let (at, cursor) = (edited.offset() as usize, cursor.offset() as usize);
    if let Some(c) = table::apply(&text, at, cursor, Op::Tidy) {
        change(buffer, &c);
    }
}

/// The view's buffer, its text and the cursor's offset.
fn state(view: &sourceview5::View) -> (sourceview5::Buffer, glib::GString, usize) {
    let buffer = view.buffer().downcast::<sourceview5::Buffer>().unwrap();
    let (start, end) = buffer.bounds();
    let text = buffer.text(&start, &end, true);
    let cursor = buffer.cursor_position() as usize;
    (buffer, text, cursor)
}

/// Make a table change, as one step to undo, and put the cursor where it says.
fn change(buffer: &sourceview5::Buffer, c: &table::Change) {
    apply(buffer, std::slice::from_ref(&c.edit));
    buffer.place_cursor(&buffer.iter_at_offset(c.cursor as i32));
}

fn enable(group: &gio::SimpleActionGroup, name: &str, enabled: bool) {
    if let Some(a) = group
        .lookup_action(name)
        .and_downcast::<gio::SimpleAction>()
    {
        a.set_enabled(enabled);
    }
}

/// The context menu's items in a table.
fn table_menu() -> gio::Menu {
    let section = |items: &[(String, &str)]| {
        let s = gio::Menu::new();
        for (label, action) in items {
            s.append(Some(label), Some(&format!("table.{action}")));
        }
        s
    };
    let row = gio::Menu::new();
    row.append_section(
        None,
        &section(&[
            (gettext("Insert Above"), "row-above"),
            (gettext("Insert Below"), "row-below"),
        ]),
    );
    row.append_section(
        None,
        &section(&[
            (gettext("Move Up"), "move-row-up"),
            (gettext("Move Down"), "move-row-down"),
        ]),
    );
    row.append_section(None, &section(&[(gettext("Delete Row"), "delete-row")]));
    let column = gio::Menu::new();
    column.append_section(
        None,
        &section(&[
            (gettext("Insert Left"), "column-left"),
            (gettext("Insert Right"), "column-right"),
        ]),
    );
    column.append_section(
        None,
        &section(&[
            (gettext("Move Left"), "move-column-left"),
            (gettext("Move Right"), "move-column-right"),
        ]),
    );
    column.append_section(
        None,
        &section(&[
            (gettext("Align Left"), "align-left"),
            (gettext("Align Center"), "align-center"),
            (gettext("Align Right"), "align-right"),
        ]),
    );
    column.append_section(
        None,
        &section(&[
            (gettext("Sort Ascending"), "sort-ascending"),
            (gettext("Sort Descending"), "sort-descending"),
        ]),
    );
    column.append_section(
        None,
        &section(&[(gettext("Delete Column"), "delete-column")]),
    );
    let menu = gio::Menu::new();
    menu.append_submenu(Some(&gettext("Row")), &row);
    menu.append_submenu(Some(&gettext("Column")), &column);
    menu.append(Some(&gettext("Format Table")), Some("table.format"));
    let outer = gio::Menu::new();
    outer.append_section(None, &menu);
    outer
}

/// The context menu's item out of tables.
fn insert_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(Some(&gettext("Insert Table")), Some("table.insert"));
    let outer = gio::Menu::new();
    outer.append_section(None, &menu);
    outer
}

/// Whether `buffer` is in Editor mode (and not too long for it).
fn editor_mode(buffer: &sourceview5::Buffer) -> bool {
    !buffer.is_highlight_syntax() && buffer.char_count() <= MAX_CHARS
}

/// Make the core's `edits` (in the order of the text) as one step to undo. The cursor stays
/// at the first, after what it inserts.
fn apply(buffer: &sourceview5::Buffer, edits: &[markdown::Edit]) {
    buffer.begin_user_action();
    for e in edits.iter().rev() {
        let (mut start, mut end) = (
            buffer.iter_at_offset(e.start as i32),
            buffer.iter_at_offset(e.end as i32),
        );
        buffer.delete(&mut start, &mut end);
        buffer.insert(&mut start, &e.text);
    }
    buffer.end_user_action();
}

/// The text under the widget's point `x`, `y`.
fn iter_at(view: &sourceview5::View, x: f64, y: f64) -> Option<gtk::TextIter> {
    let (x, y) = view.window_to_buffer_coords(gtk::TextWindowType::Widget, x as i32, y as i32);
    view.iter_at_location(x, y)
}

/// The text under the point, if it has Editor mode's tag `name`.
fn tagged_at(view: &sourceview5::View, name: &str, x: f64, y: f64) -> Option<gtk::TextIter> {
    let at = iter_at(view, x, y)?;
    let tag = view.buffer().tag_table().lookup(name)?;
    at.has_tag(&tag).then_some(at)
}

/// The start of the task box (`[ ]`, `[x]`) under the point.
fn task_at(view: &sourceview5::View, x: f64, y: f64) -> Option<gtk::TextIter> {
    let mut at = tagged_at(view, "md-task", x, y)?;
    let tag = view.buffer().tag_table().lookup("md-task")?;
    if !at.starts_tag(Some(&tag)) {
        at.backward_to_tag_toggle(Some(&tag));
    }
    Some(at)
}

/// Open the link under the point in Editor mode, in the app for its address (the browser);
/// whether there was one.
fn open_link(view: &sourceview5::View, x: f64, y: f64) -> bool {
    let buffer = view.buffer().downcast::<sourceview5::Buffer>().unwrap();
    let (Some(at), true) = (iter_at(view, x, y), editor_mode(&buffer)) else {
        return false;
    };
    let (start, end) = buffer.bounds();
    let Some(url) = markdown::link_at(&buffer.text(&start, &end, true), at.offset() as usize)
    else {
        return false;
    };
    let window = view.root().and_downcast::<gtk::Window>();
    gtk::UriLauncher::new(&url).launch(window.as_ref(), gtk::gio::Cancellable::NONE, |_| {});
    true
}

/// Tick or untick the task box under the point, as one step to undo; whether there was one.
fn toggle_task(view: &sourceview5::View, x: f64, y: f64) -> bool {
    let Some(mut start) = task_at(view, x, y) else {
        return false;
    };
    let buffer = view.buffer();
    start.forward_char();
    let mut end = start;
    end.forward_char();
    let mark = if buffer.text(&start, &end, false) == " " {
        "x"
    } else {
        " "
    };
    buffer.begin_user_action();
    buffer.delete(&mut start, &mut end);
    buffer.insert(&mut start, mark);
    buffer.end_user_action();
    true
}

/// Give the tags the look of `buffer`'s style scheme, light or dark (again when it changes).
pub fn look(buffer: &sourceview5::Buffer) {
    let (Some(scheme), table) = (buffer.style_scheme(), buffer.tag_table()) else {
        return;
    };
    for (name, id) in TAGS {
        if let (Some(t), Some(s)) = (table.lookup(name), scheme.style(id)) {
            s.apply(&t);
        }
    }
}

/// Style `buffer` anew: from the parse in Editor mode, GtkSourceView's highlighting in Source
/// mode (`source`).
pub fn restyle(buffer: &sourceview5::Buffer, source: bool) {
    let (start, end) = buffer.bounds();
    for (name, _) in TAGS {
        buffer.remove_tag_by_name(name, &start, &end);
    }
    buffer.set_highlight_syntax(source);
    if source || buffer.char_count() > MAX_CHARS {
        return;
    }
    let text = buffer.text(&start, &end, true);
    for s in markdown::spans(&text) {
        buffer.apply_tag_by_name(
            tag(s.style),
            &buffer.iter_at_offset(s.start as i32),
            &buffer.iter_at_offset(s.end as i32),
        );
    }
}
