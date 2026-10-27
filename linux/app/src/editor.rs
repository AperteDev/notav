//! Editor mode: the note styled from the core's Markdown parse (`notav_core::markdown`), its
//! marks still there, dimmed. Source mode is GtkSourceView's own highlighting
//! (`sourceview/notav-markdown.lang`).

use gtk::prelude::*;
use notav_core::markdown::{self, Style};
use sourceview5::prelude::*;

/// Above this many characters a note is not styled: each change parses it whole.
const MAX_CHARS: i32 = 300_000;

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
