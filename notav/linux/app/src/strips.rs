//! Tables and code blocks wider than the note, each scrolled sideways on its own, as on Android,
//! the rest of the note wrapped as before. A block's strip is a second view of the note's
//! buffer, laid out as the note's text is, seen through a window over the block's lines: typing,
//! the cursor, selections, the table handles and the styles are the text view's own. The focus
//! follows the cursor into and out of a strip, and the strip scrolls to keep the cursor in sight.
//! Its scrollbar stays at the bottom of what is on screen while the block goes on below it; a
//! touchpad's or a tilted wheel's sideways move (or Shift and the wheel) over it scrolls it too.

use crate::editor::Block;
use gtk::prelude::*;
use gtk::{glib, graphene};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// How wide the shade at a strip's edge is (pixels).
const EDGE: i32 = 16;

/// The shades at a strip's edges (`[left, right]`): each while there is more of the block that
/// way, so a block not shown whole is seen to be one at a glance; none while the strip is not
/// `used`.
fn show_edges(adj: &gtk::Adjustment, edges: &[gtk::Box; 2], used: bool) {
    // Not for the room past a block's end alone (a table's for the bar that adds a column).
    let more = adj.upper() - adj.page_size() - adj.value();
    edges[0].set_visible(used && adj.value() > 0.5);
    edges[1].set_visible(used && more > (2 * EDGE) as f64);
}

/// One block's strip.
struct Strip {
    /// The window on the block's lines (scrolled down to them), over the note's text.
    window: gtk::ScrolledWindow,
    /// The block's own view of the buffer, inside it.
    view: sourceview5::View,
    /// Its scrollbar, also over the note's text, and its range: the block's width.
    bar: gtk::Scrollbar,
    adj: gtk::Adjustment,
    /// A shade at its left and right edge while there is more of it that way.
    edges: [gtk::Box; 2],
    /// The block, and where its lines are in the note's text (its top, its bottom).
    block: Cell<Block>,
    top: Rc<Cell<i32>>,
    bottom: Cell<i32>,
}

pub struct Strips {
    main: sourceview5::View,
    /// The note's scrolled window: what is on screen, and where vertical scrolling goes.
    scroll: gtk::ScrolledWindow,
    strips: RefCell<Vec<Strip>>,
    /// How many of them are in use (the first ones); the rest are hidden.
    used: Cell<usize>,
    /// Wide blocks get strips (Editor mode with Wrap Lines); else the whole text scrolls.
    on: Cell<bool>,
    pending: Cell<bool>,
}

impl Strips {
    pub fn new(main: &sourceview5::View, scroll: &gtk::ScrolledWindow) -> Rc<Self> {
        let s = Rc::new(Self {
            main: main.clone(),
            scroll: scroll.clone(),
            strips: RefCell::default(),
            used: Cell::new(0),
            on: Cell::new(false),
            pending: Cell::new(false),
        });
        if let Some(side) = main.hadjustment() {
            // The note's text itself never scrolls sideways while strips do it.
            let weak = Rc::downgrade(&s);
            side.connect_value_changed(move |a| {
                if weak.upgrade().is_some_and(|s| s.on.get()) && a.value() != 0.0 {
                    a.set_value(0.0);
                }
            });
            // Another width (the window's, the font's): the blocks again.
            let weak = Rc::downgrade(&s);
            side.connect_changed(move |_| {
                if let Some(s) = weak.upgrade() {
                    s.schedule();
                }
            });
        }
        // The text laid out to another height (its styles or lines changed above a block): the
        // blocks where they are now.
        if let Some(height) = main.vadjustment() {
            let weak = Rc::downgrade(&s);
            height.connect_changed(move |_| {
                if let Some(s) = weak.upgrade() {
                    s.schedule();
                }
            });
        }
        // Scrolled, or another height: the bars where they go now.
        let place = {
            let weak = Rc::downgrade(&s);
            move |_: &gtk::Adjustment| {
                if let Some(s) = weak.upgrade() {
                    s.place_bars();
                }
            }
        };
        scroll.vadjustment().connect_value_changed(place.clone());
        scroll.vadjustment().connect_changed(place);
        let weak = Rc::downgrade(&s);
        main.buffer().connect_mark_set(move |b, _, mark| {
            if *mark == b.get_insert()
                && let Some(s) = weak.upgrade()
            {
                let weak = Rc::downgrade(&s);
                glib::idle_add_local_once(move || {
                    if let Some(s) = weak.upgrade() {
                        s.follow();
                    }
                });
            }
        });
        // Focused with the cursor in a block (after opening the note, say): the block's strip.
        let weak = Rc::downgrade(&s);
        main.connect_has_focus_notify(move |v| {
            if v.has_focus()
                && let Some(s) = weak.upgrade()
            {
                s.follow();
            }
        });
        s
    }

    /// Strips for the wide blocks (`on`), or none; after the text's layout, which a change of
    /// its styles or width redoes.
    pub fn update(self: &Rc<Self>, on: bool) {
        self.on.set(on);
        self.schedule();
    }

    fn schedule(self: &Rc<Self>) {
        if self.pending.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        // After the text view's own validation of the lines, which runs at a higher priority.
        glib::idle_add_local_once(move || {
            if let Some(s) = weak.upgrade() {
                s.pending.set(false);
                s.lay_out();
            }
        });
    }

    fn lay_out(self: &Rc<Self>) {
        let width = self.main.width();
        let wide: Vec<Block> = if self.on.get() && width > 0 {
            let room = width - self.main.right_margin();
            crate::editor::blocks(&self.main)
                .into_iter()
                .filter(|b| b.width > room)
                .collect()
        } else {
            Vec::new()
        };
        let mut strips = self.strips.borrow_mut();
        while strips.len() < wide.len() {
            strips.push(self.strip());
        }
        // Those left over are hidden, to be used again: a text view's overlays cannot be taken
        // off it. Without the buffer they lay nothing out meanwhile.
        let buffer = self.main.buffer();
        for (i, s) in strips.iter().enumerate() {
            let used = i < wide.len();
            if used == s.window.is_visible() {
                continue;
            }
            if used {
                s.view.set_buffer(Some(&buffer));
            } else {
                if s.view.has_focus() {
                    self.main.grab_focus();
                }
                s.view.set_buffer(None::<&gtk::TextBuffer>);
            }
            s.window.set_visible(used);
            s.bar.set_visible(used);
            show_edges(&s.adj, &s.edges, used);
        }
        self.used.set(wide.len());
        for (s, b) in strips.iter().zip(&wide) {
            let (Some(first), Some(last)) =
                (buffer.iter_at_line(b.first), buffer.iter_at_line(b.last))
            else {
                continue;
            };
            let top = self.main.line_yrange(&first).0;
            let (y, h) = self.main.line_yrange(&last);
            s.block.set(*b);
            s.top.set(top);
            s.bottom.set(y + h);
            s.window.set_size_request(width, y + h - top);
            self.main.move_overlay(&s.window, 0, top);
            // In the strip's view the text starts below its top margin.
            s.window
                .vadjustment()
                .set_value((top + s.view.top_margin()) as f64);
            s.adj.configure(
                s.adj.value(),
                0.0,
                (b.width + self.main.right_margin()) as f64,
                24.0,
                width as f64 * 0.9,
                width as f64,
            );
            s.bar.set_size_request(width, -1);
            let [left, right] = &s.edges;
            for e in [left, right] {
                e.set_size_request(EDGE, y + h - top);
            }
            self.main.move_overlay(left, 0, top);
            self.main.move_overlay(right, width - EDGE, top);
            show_edges(&s.adj, &s.edges, true);
        }
        drop(strips);
        self.place_bars();
        self.follow();
    }

    /// A new strip, its view a copy of the note's.
    fn strip(self: &Rc<Self>) -> Strip {
        let main = &self.main;
        let view = sourceview5::View::with_buffer(
            &main.buffer().downcast::<sourceview5::Buffer>().unwrap(),
        );
        view.set_wrap_mode(main.wrap_mode());
        view.set_top_margin(main.top_margin());
        view.set_bottom_margin(main.bottom_margin());
        view.set_left_margin(main.left_margin());
        view.set_right_margin(main.right_margin());
        for class in main.css_classes() {
            view.add_css_class(&class);
        }
        crate::editor::connect(&view);
        // Its sideways scrolling ours (the bar), its height all of the text's, as the note's.
        let inner = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::External)
            .vscrollbar_policy(gtk::PolicyType::Never)
            .child(&view)
            .build();
        let viewport = gtk::Viewport::builder()
            .scroll_to_focus(false)
            .child(&inner)
            .build();
        let window = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vscrollbar_policy(gtk::PolicyType::External)
            .propagate_natural_height(false)
            .child(&viewport)
            .css_classes(["block-strip"])
            .build();
        // Down to the block's lines, and kept there: again once the view's height is known, and
        // whatever else moves it (the window's first allocation does).
        let top = Rc::new(Cell::new(0));
        let hold = {
            let (t, v, vp) = (top.clone(), view.clone(), viewport.downgrade());
            move |a: &gtk::Adjustment| {
                let want = (t.get() + v.top_margin()) as f64;
                if a.value() != want {
                    a.set_value(want);
                    // Moved during an allocation, the viewport would keep the old place.
                    let vp = vp.clone();
                    glib::idle_add_local_once(move || {
                        if let Some(vp) = vp.upgrade() {
                            vp.queue_allocate();
                        }
                    });
                }
            }
        };
        window.vadjustment().connect_changed(hold.clone());
        window.vadjustment().connect_value_changed(hold);

        // Each follows the other; only a value that differs is passed on, as setting the view's
        // stops its own animated scroll (to the cursor) where it is.
        let adj = gtk::Adjustment::new(0.0, 0.0, 1.0, 24.0, 1.0, 1.0);
        let side = inner.hadjustment();
        let s = side.clone();
        adj.connect_value_changed(move |a| {
            if (s.value() - a.value()).abs() > 0.5 {
                s.set_value(a.value());
            }
        });
        let a = adj.clone();
        side.connect_value_changed(move |s| {
            if (a.value() - s.value()).abs() > 0.5 {
                a.set_value(s.value());
            }
        });
        let bar = gtk::Scrollbar::new(gtk::Orientation::Horizontal, Some(&adj));
        bar.add_css_class("block-scrollbar");

        // Sideways moves over it scroll it; the rest scrolls the note, as over its text.
        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::BOTH_AXES);
        scroll.set_propagation_phase(gtk::PropagationPhase::Capture);
        let (a, note) = (adj.clone(), self.scroll.vadjustment());
        scroll.connect_scroll(move |c, dx, dy| {
            let shift = c
                .current_event_state()
                .contains(gtk::gdk::ModifierType::SHIFT_MASK);
            let (dx, dy) = if shift && dx == 0.0 {
                (dy, 0.0)
            } else {
                (dx, dy)
            };
            // A wheel's step as GTK's scrolled windows take it.
            let step = |adj: &gtk::Adjustment| match c.unit() {
                gtk::gdk::ScrollUnit::Wheel => adj.page_size().powf(2.0 / 3.0),
                _ => 1.0,
            };
            if dx != 0.0 {
                a.set_value(a.value() + dx * step(&a));
            }
            if dy != 0.0 {
                note.set_value(note.value() + dy * step(&note));
            }
            glib::Propagation::Stop
        });
        window.add_controller(scroll);
        // The pointer's presses and moves end here, once the strip's view has had them: the text
        // view takes a press without claiming it, so the note's text under the strip would take
        // it as well (its cursor and a selection at the unscrolled place, its table handles).
        let stop = gtk::EventControllerLegacy::new();
        stop.connect_event(|_, e| {
            use gtk::gdk::EventType::*;
            match e.event_type() {
                ButtonPress | ButtonRelease | MotionNotify | TouchBegin | TouchUpdate
                | TouchEnd | TouchCancel => glib::Propagation::Stop,
                _ => glib::Propagation::Proceed,
            }
        });
        window.add_controller(stop);

        main.add_overlay(&window, 0, 0);
        main.add_overlay(&bar, 0, 0);
        // Over the strip, letting the pointer through to it.
        let edges = ["left", "right"].map(|side| {
            let e = gtk::Box::builder()
                .can_target(false)
                .visible(false)
                .css_classes(["block-edge", side])
                .build();
            main.add_overlay(&e, 0, 0);
            e
        });
        let (e, w) = (edges.clone(), window.clone());
        adj.connect_value_changed(move |a| show_edges(a, &e, w.is_visible()));
        let (e, w) = (edges.clone(), window.clone());
        adj.connect_changed(move |a| show_edges(a, &e, w.is_visible()));
        Strip {
            window,
            view,
            bar,
            adj,
            edges,
            block: Cell::new(Block {
                first: 0,
                last: 0,
                width: 0,
            }),
            top,
            bottom: Cell::new(0),
        }
    }

    /// Each bar under its block, or at the bottom of what is on screen while the block goes on
    /// below it.
    fn place_bars(&self) {
        // What is on screen, in the note's text: the scrolled content's coordinates, less where
        // the text starts in it.
        let Some(content) = self.scroll.child().and_then(|v| v.first_child()) else {
            return;
        };
        let Some(origin) = self
            .main
            .compute_point(&content, &graphene::Point::new(0.0, 0.0))
        else {
            return;
        };
        let adj = self.scroll.vadjustment();
        let shown =
            (adj.value() + adj.page_size()) as i32 - origin.y() as i32 - self.main.top_margin();
        for s in self.strips.borrow().iter().take(self.used.get()) {
            let height = s.bar.measure(gtk::Orientation::Vertical, -1).1;
            let (top, bottom) = (s.top.get(), s.bottom.get());
            let y = (shown.min(bottom) - height).max(top);
            self.main.move_overlay(&s.bar, 0, y);
        }
    }

    /// The focus where the cursor is: a block's strip, or the note's text; the strip scrolled
    /// to show it.
    fn follow(&self) {
        let buffer = self.main.buffer();
        let line = buffer.iter_at_mark(&buffer.get_insert()).line();
        let strips = self.strips.borrow();
        let strips = &strips[..self.used.get()];
        let at = strips
            .iter()
            .find(|s| (s.block.get().first..=s.block.get().last).contains(&line));
        let focused = strips.iter().find(|s| s.view.has_focus());
        match (at, focused) {
            (Some(s), f) if self.main.has_focus() || f.is_some_and(|f| f.view != s.view) => {
                s.view.grab_focus();
            }
            (None, Some(_)) => {
                self.main.grab_focus();
            }
            _ => {}
        }
        if let Some(s) = at {
            s.view.scroll_mark_onscreen(&buffer.get_insert());
        }
    }
}
