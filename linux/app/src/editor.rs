//! Editor mode: the note styled from the core's Markdown parse (`notav_core::markdown`), its
//! marks still there, dimmed. Source mode is GtkSourceView's own highlighting
//! (`sourceview/notav-markdown.lang`).

use crate::i18n::gettext;
use gtk::prelude::*;
use gtk::{gio, glib};
use notav_core::markdown::{self, Style};
use notav_core::table::{self, Align, Op};
use sourceview5::prelude::*;
use std::cmp::Ordering;
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
    ("duplicate-row", Op::DuplicateRow),
    ("delete-row", Op::DeleteRow),
    ("column-left", Op::ColumnLeft),
    ("column-right", Op::ColumnRight),
    ("move-column-left", Op::MoveColumnLeft),
    ("move-column-right", Op::MoveColumnRight),
    ("duplicate-column", Op::DuplicateColumn),
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

/// What a click and keys do in Editor mode: a click ticks a task, Ctrl+click opens a link (Alt+
/// Enter the one at the cursor, as in Obsidian), Enter continues a list; Tab, Shift+Tab and
/// Enter go through a table's cells, and Enter after a header line starts one.
pub fn connect(view: &sourceview5::View) {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    keys.connect_key_pressed(|keys, key, _, state| {
        use gtk::gdk::Key;
        let Some(view) = keys.widget().and_downcast::<sourceview5::View>() else {
            return glib::Propagation::Proceed;
        };
        let buffer = view.buffer().downcast::<sourceview5::Buffer>().unwrap();
        let mods = state & gtk::accelerator_get_default_mod_mask();
        let enter = matches!(key, Key::Return | Key::KP_Enter);
        // The link the cursor is in, or ends.
        let cursor = buffer.cursor_position() as usize;
        if enter
            && mods == gtk::gdk::ModifierType::ALT_MASK
            && follow(&view, &[cursor, cursor.saturating_sub(1)])
        {
            return glib::Propagation::Stop;
        }
        let plain = mods.is_empty();
        let step = match key {
            Key::Tab | Key::KP_Tab if plain => table::Step::Next,
            Key::ISO_Left_Tab if mods == gtk::gdk::ModifierType::SHIFT_MASK => {
                table::Step::Previous
            }
            _ if enter && plain => table::Step::Down,
            _ => return glib::Propagation::Proceed,
        };
        if buffer.has_selection() || !editor_mode(&buffer) {
            return glib::Propagation::Proceed;
        }
        let (start, end) = buffer.bounds();
        let text = buffer.text(&start, &end, true);
        // In a table Tab and Shift+Tab go from cell to cell (a tab would break it), Enter to the
        // next row; a header line typed out of one starts it.
        let c = match step {
            table::Step::Down => {
                table::step(&text, cursor, step).or_else(|| table::start(&text, cursor))
            }
            _ => table::step(&text, cursor, step),
        };
        if let Some(c) = c {
            change(&buffer, &c);
            return glib::Propagation::Stop;
        }
        if step != table::Step::Down {
            return if table::is_at(&text, cursor) {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            };
        }
        let edits = markdown::enter(&text, cursor);
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
    handles(view, update.clone());
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

/// How far past a table the pointer can go with its handles still shown, how thick the bars
/// that add a row or column are and how far from the table (pixels).
const HANDLES_REACH: i32 = 28;
const BAR: i32 = 16;
const BAR_GAP: i32 = 4;

/// The handles of the table under the pointer, as Obsidian has them: a grip on the delimiter
/// row in the column and on the left edge of the row the pointer is on (a click opens its
/// menu, a drag moves it), and bars past the last column and under the last row that add one.
struct Handles {
    view: glib::WeakRef<sourceview5::View>,
    row: gtk::MenuButton,
    column: gtk::MenuButton,
    add_row: gtk::Button,
    add_column: gtk::Button,
    /// Where a dragged row or column would go.
    drop: gtk::Box,
    shown: std::cell::RefCell<Option<Shown>>,
    /// While a grip is pressed: its row or column, then where a drag would take it.
    target: std::cell::Cell<Option<usize>>,
    /// Whether the pressed grip has gone far enough to be dragged, not clicked.
    dragging: std::cell::Cell<bool>,
}

/// The table the handles are on: where its rows and columns are, in the text and in buffer
/// coordinates, and the row and column of the grips.
struct Shown {
    layout: table::Layout,
    /// Each row's top and height, and the delimiter row's (the column grip's place: on the
    /// header, it would hide the column's name).
    rows: Vec<(i32, i32)>,
    delimiter: (i32, i32),
    /// Each column edge's x (its pipe's middle).
    edges: Vec<i32>,
    row: usize,
    column: usize,
}

impl Shown {
    fn new(view: &sourceview5::View, layout: table::Layout) -> Self {
        let mut s = Self {
            layout,
            rows: Vec::new(),
            delimiter: (0, 0),
            edges: Vec::new(),
            row: 0,
            column: 0,
        };
        s.measure(view);
        s
    }

    /// Where the rows and columns are now (after a change of font or size).
    fn measure(&mut self, view: &sourceview5::View) {
        let buffer = view.buffer();
        let at = |offset: usize| buffer.iter_at_offset(offset as i32);
        self.rows = self
            .layout
            .rows
            .iter()
            .map(|&r| view.line_yrange(&at(r)))
            .collect();
        let mut delimiter = at(self.layout.rows[0]);
        delimiter.forward_line();
        self.delimiter = view.line_yrange(&delimiter);
        self.edges = self
            .layout
            .edges
            .iter()
            .map(|&e| {
                let r = view.iter_location(&at(e));
                r.x() + r.width() / 2
            })
            .collect();
    }

    fn top(&self) -> i32 {
        self.rows[0].0
    }

    fn bottom(&self) -> i32 {
        self.rows.last().map_or(0, |(y, h)| y + h)
    }

    fn left(&self) -> i32 {
        self.edges[0]
    }

    fn right(&self) -> i32 {
        self.edges.last().copied().unwrap_or_default()
    }

    /// The row and the column at buffer coordinates `y` and `x`, the nearest past the edges.
    fn row_at(&self, y: i32) -> usize {
        let last = self.rows.len() - 1;
        self.rows
            .iter()
            .position(|(top, h)| y < top + h)
            .unwrap_or(last)
    }

    fn column_at(&self, x: i32) -> usize {
        let last = self.edges.len() - 2;
        self.edges[1..].iter().position(|&e| x < e).unwrap_or(last)
    }

    /// A character in the grips' row or column (the header's cell), past its pipe and space.
    fn cell(&self, rows: bool) -> usize {
        let (start, end) = if rows {
            (self.layout.rows[self.row], usize::MAX)
        } else {
            (
                self.layout.edges[self.column],
                self.layout.edges[self.column + 1],
            )
        };
        (start + 2).min(end)
    }
}

/// Show `view`'s table handles as the pointer goes over a table in Editor mode; `update` sets
/// the table actions for the cursor's cell before a grip's menu opens.
fn handles(view: &sourceview5::View, update: impl Fn(&sourceview5::View) + 'static) {
    let grip = |icon: &str, tooltip: String, menu: gio::Menu| {
        let b = gtk::MenuButton::builder()
            .icon_name(icon)
            .tooltip_text(tooltip)
            .menu_model(&menu)
            .can_focus(false)
            .focus_on_click(false)
            .visible(false)
            .css_classes(["table-grip"])
            .build();
        b.set_cursor_from_name(Some("grab"));
        b
    };
    let bar = |tooltip: String| {
        let b = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text(tooltip)
            .can_focus(false)
            .focus_on_click(false)
            .visible(false)
            .css_classes(["table-add"])
            .build();
        b.set_cursor_from_name(Some("default"));
        b
    };
    let h = Rc::new(Handles {
        view: view.downgrade(),
        row: grip("list-drag-handle-symbolic", gettext("Row"), row_menu()),
        column: grip(
            "notav-drag-handle-horizontal-symbolic",
            gettext("Column"),
            column_menu(),
        ),
        add_row: bar(gettext("Add Row")),
        add_column: bar(gettext("Add Column")),
        drop: gtk::Box::builder()
            .visible(false)
            .can_target(false)
            .css_classes(["table-drop"])
            .build(),
        shown: Default::default(),
        target: Default::default(),
        dragging: Default::default(),
    });
    h.row.set_direction(gtk::ArrowType::Right);
    // Drawn in this order: the drop line under the grip dragged on it.
    for w in [
        h.drop.upcast_ref::<gtk::Widget>(),
        h.add_row.upcast_ref(),
        h.add_column.upcast_ref(),
        h.row.upcast_ref(),
        h.column.upcast_ref(),
    ] {
        view.add_overlay(w, 0, 0);
    }

    // The closures hold the handles: they live as long as the view, which is never dropped.
    // A grip's menu is for its row or column: the cursor goes there first.
    let update = Rc::new(update);
    for (grip, rows) in [(&h.row, true), (&h.column, false)] {
        let (hh, update) = (h.clone(), update.clone());
        grip.set_create_popup_func(move |_| {
            let Some(view) = hh.view.upgrade() else {
                return;
            };
            let Some(at) = hh.shown.borrow().as_ref().map(|s| s.cell(rows)) else {
                return;
            };
            let buffer = view.buffer();
            // Not past a row of one character.
            let mut iter = buffer.iter_at_offset(at as i32);
            if iter.line() != buffer.iter_at_offset(at as i32 - 2).line() {
                iter.backward_char();
            }
            buffer.place_cursor(&iter);
            update(&view);
        });
        let hh = h.clone();
        grip.connect_active_notify(move |grip| {
            if !grip.is_active() {
                hh.hide();
            }
        });
        drag(&h, grip, rows);
    }

    for (bar, op) in [(&h.add_row, Op::RowBelow), (&h.add_column, Op::ColumnRight)] {
        let h = h.clone();
        bar.connect_clicked(move |_| {
            let Some(view) = h.view.upgrade() else { return };
            // The last row or column's.
            let at = h.shown.borrow().as_ref().map(|s| match op {
                Op::RowBelow => s.layout.rows[s.layout.rows.len() - 1],
                _ => s.layout.edges[s.layout.edges.len() - 2] + 1,
            });
            let (buffer, text, _) = state(&view);
            if let Some(c) = at.and_then(|at| table::apply(&text, at, at, op)) {
                change(&buffer, &c);
                view.grab_focus();
            }
        });
    }

    let motion = gtk::EventControllerMotion::new();
    let hh = h.clone();
    motion.connect_motion(move |m, x, y| {
        if let Some(view) = m.widget().and_downcast() {
            hh.hover(&view, x, y);
        }
    });
    let hh = h.clone();
    motion.connect_leave(move |_| {
        if !hh.busy() {
            hh.hide();
        }
    });
    view.add_controller(motion);
    // Typing moves the table: its handles go until the pointer moves.
    view.buffer().connect_changed(move |_| {
        if !h.busy() {
            h.hide();
        }
    });
}

impl Handles {
    /// Whether a grip's menu is open or a grip is being dragged.
    fn busy(&self) -> bool {
        self.row.is_active() || self.column.is_active() || self.target.get().is_some()
    }

    fn hide(&self) {
        self.shown.replace(None);
        for w in [
            self.row.upcast_ref::<gtk::Widget>(),
            self.column.upcast_ref(),
            self.add_row.upcast_ref(),
            self.add_column.upcast_ref(),
            self.drop.upcast_ref(),
        ] {
            w.set_visible(false);
        }
    }

    /// The pointer at the view's `x`, `y`: the handles on the table under it, or near the one
    /// they are on, at the row and column it is in.
    fn hover(&self, view: &sourceview5::View, x: f64, y: f64) {
        if self.busy() {
            return;
        }
        let (x, y) = view.window_to_buffer_coords(gtk::TextWindowType::Widget, x as i32, y as i32);
        let mut shown = self.shown.borrow_mut();
        if let Some(s) = shown.as_mut() {
            s.measure(view);
        }
        let near = |s: &Shown| {
            (s.left() - HANDLES_REACH..s.right() + HANDLES_REACH).contains(&x)
                && (s.top() - HANDLES_REACH..s.bottom() + HANDLES_REACH).contains(&y)
        };
        if !shown.as_ref().is_some_and(near) {
            *shown = table_under(view, x, y).map(|layout| Shown::new(view, layout));
        }
        let Some(s) = shown.as_mut() else {
            drop(shown);
            self.hide();
            return;
        };
        if (s.left()..s.right()).contains(&x) && (s.top()..s.bottom()).contains(&y) {
            s.row = s.row_at(y);
            s.column = s.column_at(x);
        }
        // Shown first: a hidden widget keeps the size it was measured at. No row grip on the
        // header, which stays first (its row items are in the context menu).
        for w in [
            self.column.upcast_ref::<gtk::Widget>(),
            self.add_row.upcast_ref(),
            self.add_column.upcast_ref(),
        ] {
            w.set_visible(true);
        }
        self.row.set_visible(s.row != 0);
        let (top, height) = s.delimiter;
        let middle = (s.edges[s.column] + s.edges[s.column + 1]) / 2;
        center(view, &self.column, middle, top + height / 2);
        let (top, height) = s.rows[s.row];
        center(view, &self.row, s.left(), top + height / 2);
        self.add_column.set_size_request(BAR, s.bottom() - s.top());
        view.move_overlay(&self.add_column, s.right() + BAR_GAP, s.top());
        self.add_row.set_size_request(s.right() - s.left(), BAR);
        view.move_overlay(&self.add_row, s.left(), s.bottom() + BAR_GAP);
    }
}

/// Put `w`, a child of `view`, with its middle at buffer coordinates `x`, `y` (not past the
/// view's left edge).
fn center(view: &sourceview5::View, w: &impl IsA<gtk::Widget>, x: i32, y: i32) {
    let width = w.measure(gtk::Orientation::Horizontal, -1).1;
    let height = w.measure(gtk::Orientation::Vertical, -1).1;
    view.move_overlay(w, (x - width / 2).max(0), y - height / 2);
}

/// How far the pointer goes from where a grip was pressed before it is a drag, not a click.
const DRAG_THRESHOLD: f64 = 8.0;

/// Dragging `grip` moves its row (`rows`) or column: a line shows where it would go, the grip
/// on it, the other handles hidden (rows not above the header). A click opens its menu.
fn drag(h: &Rc<Handles>, grip: &gtk::MenuButton, rows: bool) {
    let gesture = gtk::GestureDrag::new();
    // The press is the grip's alone: a button does not claim it, so it would go on to the
    // text, which selects from the first move.
    gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
    let hh = h.clone();
    gesture.connect_drag_begin(move |g, _, _| {
        g.set_state(gtk::EventSequenceState::Claimed);
        let source = hh
            .shown
            .borrow()
            .as_ref()
            .map(|s| if rows { s.row } else { s.column });
        hh.target.set(source);
    });
    let hh = h.clone();
    gesture.connect_drag_update(move |g, dx, dy| {
        let (h, Some(view), Some(grip)) = (&hh, hh.view.upgrade(), g.widget()) else {
            return;
        };
        let shown = h.shown.borrow();
        let Some(s) = shown.as_ref() else { return };
        let source = if rows { s.row } else { s.column };
        if !h.dragging.get() {
            // The offsets are the grip's, which moves once dragged.
            if dx.hypot(dy) < DRAG_THRESHOLD {
                return;
            }
            h.dragging.set(true);
            let others = [
                h.row.upcast_ref::<gtk::Widget>(),
                h.column.upcast_ref(),
                h.add_row.upcast_ref(),
                h.add_column.upcast_ref(),
            ];
            for w in others.into_iter().filter(|w| **w != grip) {
                w.set_visible(false);
            }
        }
        // The pointer, from the event: not moving with the grip.
        let (Some(native), Some((px, py))) =
            (view.native(), g.current_event().and_then(|e| e.position()))
        else {
            return;
        };
        let (tx, ty) = native.surface_transform();
        let point = gtk::graphene::Point::new((px - tx) as f32, (py - ty) as f32);
        let Some(p) = native.compute_point(&view, &point) else {
            return;
        };
        let (x, y) =
            view.window_to_buffer_coords(gtk::TextWindowType::Widget, p.x() as i32, p.y() as i32);
        let target = if rows {
            s.row_at(y).max(1)
        } else {
            s.column_at(x)
        };
        h.target.set(Some(target));
        h.drop.set_visible(target != source);
        // The line at the edge it would go past, the grip on it; where it was if it stays.
        if rows {
            let (top, height) = s.rows[target];
            let y = match target.cmp(&source) {
                Ordering::Less => top,
                Ordering::Equal => top + height / 2,
                Ordering::Greater => top + height,
            };
            if target != source {
                h.drop.set_size_request(s.right() - s.left(), 2);
                view.move_overlay(&h.drop, s.left(), y - 1);
            }
            center(&view, &grip, s.left(), y);
        } else {
            let x = match target.cmp(&source) {
                Ordering::Less => s.edges[target],
                Ordering::Equal => (s.edges[target] + s.edges[target + 1]) / 2,
                Ordering::Greater => s.edges[target + 1],
            };
            if target != source {
                h.drop.set_size_request(2, s.bottom() - s.top());
                view.move_overlay(&h.drop, x - 1, s.top());
            }
            let (top, height) = s.delimiter;
            center(&view, &grip, x, top + height / 2);
        }
    });
    // Stopped by GTK while pressed (a grab elsewhere, a screenshot): no drag-end follows. Not
    // after it, when the menu it opened takes the grab.
    let hh = h.clone();
    gesture.connect_cancel(move |_, _| {
        if hh.target.take().is_some() {
            hh.dragging.set(false);
            hh.hide();
        }
    });
    let h = h.clone();
    gesture.connect_drag_end(move |g, _, _| {
        let Some(target) = h.target.take() else {
            return;
        };
        h.drop.set_visible(false);
        if !h.dragging.replace(false) {
            if let Some(grip) = g.widget().and_downcast::<gtk::MenuButton>() {
                grip.popup();
            }
            return;
        }
        let at = h.shown.borrow().as_ref().map(|s| {
            if rows {
                (s.layout.rows[s.row], Op::MoveRow { to: target })
            } else {
                (s.layout.edges[s.column] + 1, Op::MoveColumn { to: target })
            }
        });
        let Some(view) = h.view.upgrade() else { return };
        let (buffer, text, _) = state(&view);
        match at.and_then(|(at, op)| table::apply(&text, at, at, op)) {
            Some(c) => change(&buffer, &c),
            None => h.hide(),
        }
    });
    grip.add_controller(gesture);
}

/// The layout of the table at buffer coordinates `x`, `y`, in Editor mode.
fn table_under(view: &sourceview5::View, x: i32, y: i32) -> Option<table::Layout> {
    let buffer = view.buffer().downcast::<sourceview5::Buffer>().ok()?;
    let at = view.iter_at_location(x, y)?;
    // Parsing the note on each move only where it may be.
    let mut end = at;
    if !end.ends_line() {
        end.forward_to_line_end();
    }
    let start = buffer.iter_at_line(at.line())?;
    if !editor_mode(&buffer) || !buffer.text(&start, &end, false).contains('|') {
        return None;
    }
    let (start, end) = buffer.bounds();
    table::layout(&buffer.text(&start, &end, true), at.offset() as usize)
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
    if c.edit.start != c.edit.end || !c.edit.text.is_empty() {
        apply(buffer, std::slice::from_ref(&c.edit));
    }
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
    let menu = gio::Menu::new();
    menu.append_submenu(Some(&gettext("Row")), &row_menu());
    menu.append_submenu(Some(&gettext("Column")), &column_menu());
    menu.append(Some(&gettext("Format Table")), Some("table.format"));
    let outer = gio::Menu::new();
    outer.append_section(None, &menu);
    outer
}

/// A menu section of `table.<action>` items.
fn section(items: &[(String, &str)]) -> gio::Menu {
    let s = gio::Menu::new();
    for (label, action) in items {
        s.append(Some(label), Some(&format!("table.{action}")));
    }
    s
}

/// What can be done to the cursor's row (the context menu's Row, a row grip's menu).
fn row_menu() -> gio::Menu {
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
    row.append_section(
        None,
        &section(&[
            (gettext("Duplicate Row"), "duplicate-row"),
            (gettext("Delete Row"), "delete-row"),
        ]),
    );
    row
}

/// What can be done to the cursor's column (the context menu's Column, a column grip's menu).
fn column_menu() -> gio::Menu {
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
        &section(&[
            (gettext("Duplicate Column"), "duplicate-column"),
            (gettext("Delete Column"), "delete-column"),
        ]),
    );
    column
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

/// Open the link under the point in Editor mode; whether there was one.
fn open_link(view: &sourceview5::View, x: f64, y: f64) -> bool {
    iter_at(view, x, y).is_some_and(|at| follow(view, &[at.offset() as usize]))
}

/// Open the link at the first of the characters `at` that has one, in Editor mode: a note here,
/// an address in the app for it (the browser); whether there was one.
fn follow(view: &sourceview5::View, at: &[usize]) -> bool {
    let buffer = view.buffer().downcast::<sourceview5::Buffer>().unwrap();
    if !editor_mode(&buffer) {
        return false;
    }
    let (start, end) = buffer.bounds();
    let text = buffer.text(&start, &end, true);
    match at.iter().find_map(|&a| markdown::link_at(&text, a)) {
        Some(markdown::Link::Address(url)) => {
            let window = view.root().and_downcast::<gtk::Window>();
            gtk::UriLauncher::new(&url).launch(window.as_ref(), gio::Cancellable::NONE, |_| {});
        }
        // After the click or key: it replaces the text under it.
        Some(markdown::Link::Note {
            target,
            heading,
            wiki,
        }) => {
            glib::idle_add_local_once(move || crate::window::follow_link(target, heading, wiki));
        }
        None => return false,
    }
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
