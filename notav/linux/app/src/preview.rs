//! Preview mode: the note read only, rendered as GitHub renders Markdown (the core's
//! `markdown::html`), in a web view. The note's own scripts never run; links open notes here
//! and addresses in their apps.

use gtk::prelude::*;
use gtk::{gio, glib};
use notav_core::markdown;
use std::cell::Cell;
use std::rc::Rc;
use webkit6::prelude::*;

/// The page's style: GitHub's look for Markdown, in its light and dark colours.
const CSS: &str = include_str!("preview.css");
/// Wide tables and code blocks scrolled each on its own, as Editor mode's strips.
const STRIPS: &str = include_str!("preview.js");

pub struct Preview {
    pub view: webkit6::WebView,
    /// The heading (an index of `markdown::headings`) to show once the page is loaded.
    heading: Cell<Option<usize>>,
}

/// What the page looks like besides the note: the app's choices.
pub struct Look {
    pub dark: bool,
    /// CSS rules for the text's and the code's fonts and sizes.
    pub fonts: String,
    pub full_width: bool,
    /// Wrap Lines: the text wrapped, wide blocks scrolled each on its own; else nothing wraps
    /// and the page scrolls sideways.
    pub wrap: bool,
    /// GitHub's alerts' titles, translated: Note, Tip, Important, Warning, Caution.
    pub alerts: [String; 5],
}

impl Preview {
    pub fn new() -> Rc<Self> {
        let view = webkit6::WebView::new();
        if let Some(settings) = WebViewExt::settings(&view) {
            // The note's scripts (in its HTML) never run; the app's own (scrolling) do.
            settings.set_enable_javascript_markup(false);
            settings.set_enable_developer_extras(false);
        }
        if let Some(content) = view.user_content_manager() {
            content.add_script(&webkit6::UserScript::new(
                STRIPS,
                webkit6::UserContentInjectedFrames::TopFrame,
                webkit6::UserScriptInjectionTime::End,
                &[],
                &[],
            ));
        }
        view.set_background_color(&gtk::gdk::RGBA::TRANSPARENT);
        view.set_vexpand(true);
        let preview = Rc::new(Self {
            view,
            heading: Cell::new(None),
        });
        preview.view.connect_decide_policy(|view, decision, kind| {
            use webkit6::PolicyDecisionType as T;
            let Some(nav) = decision.downcast_ref::<webkit6::NavigationPolicyDecision>() else {
                return false;
            };
            let Some(action) = nav.navigation_action() else {
                return false;
            };
            let uri = action.request().and_then(|r| r.uri()).unwrap_or_default();
            // The page itself, loaded by `show`, and its own anchors (footnotes).
            if kind == T::NavigationAction
                && (action.navigation_type() != webkit6::NavigationType::LinkClicked
                    || uri.starts_with("about:blank#"))
            {
                return false;
            }
            decision.ignore();
            follow(view, &uri);
            true
        });
        // Only what copies: no Reload, Back or Open in New Window.
        preview.view.connect_context_menu(|_, menu, _| {
            use webkit6::ContextMenuAction as A;
            for item in menu.items() {
                if !matches!(
                    item.stock_action(),
                    A::Copy
                        | A::CopyLinkToClipboard
                        | A::CopyImageToClipboard
                        | A::CopyImageUrlToClipboard
                        | A::SelectAll
                ) {
                    menu.remove(&item);
                }
            }
            menu.n_items() == 0
        });
        let weak = Rc::downgrade(&preview);
        preview.view.connect_load_changed(move |_, event| {
            if let (Some(p), webkit6::LoadEvent::Finished) = (weak.upgrade(), event)
                && let Some(i) = p.heading.take()
            {
                p.scroll_to(i);
            }
        });
        preview
    }

    /// Show the note `text` titled `title`.
    // ponytail: the whole page is loaded again (back at its top) on each change of the note or
    // its look; keep the scroll through a reload if that gets in the way.
    pub fn show(&self, title: &str, text: &str, look: &Look) {
        let mut css = String::from(CSS);
        css.push_str(&look.fonts);
        if look.full_width {
            css.push_str(".markdown-body { max-width: none; }\n");
        }
        for (kind, title) in ["note", "tip", "important", "warning", "caution"]
            .iter()
            .zip(&look.alerts)
        {
            let title = title.replace('\\', "\\\\").replace('"', "\\\"");
            css.push_str(&format!(
                ".markdown-alert-{kind}::before {{ content: \"{title}\"; }}\n"
            ));
        }
        let page = format!(
            "<!DOCTYPE html><html class=\"{}{}\"><head><meta charset=\"utf-8\"><style>{css}</style>\
             </head><body><article class=\"markdown-body\"><h1 class=\"note-title\">{}</h1>{}\
             </article></body></html>",
            if look.dark { "dark" } else { "light" },
            if look.wrap { "" } else { " nowrap" },
            escape(title),
            markdown::html(text),
        );
        self.view.load_html(&page, Some("about:blank"));
    }

    /// Scroll to the heading `index` (of `markdown::headings`), once the page is loaded (after
    /// the next `show` with `reloading`).
    pub fn show_heading(&self, index: usize, reloading: bool) {
        if reloading || self.view.is_loading() {
            self.heading.set(Some(index));
        } else {
            self.scroll_to(index);
        }
    }

    fn scroll_to(&self, index: usize) {
        let script = format!("document.getElementById('h{index}')?.scrollIntoView()");
        self.view
            .evaluate_javascript(&script, None, None, gio::Cancellable::NONE, |_| {});
    }
}

/// Open a link clicked in the page: a note here (as Editor mode's Ctrl+click does), an
/// address in the app for it.
fn follow(view: &webkit6::WebView, uri: &str) {
    match markdown::preview_link(uri) {
        Some(markdown::Link::Address(url)) => {
            let window = view.root().and_downcast::<gtk::Window>();
            gtk::UriLauncher::new(&url).launch(window.as_ref(), gio::Cancellable::NONE, |_| {});
        }
        Some(markdown::Link::Note {
            target,
            heading,
            wiki,
        }) => {
            glib::idle_add_local_once(move || crate::window::follow_link(target, heading, wiki));
        }
        None => {}
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
