//! How a note looks in Editor mode: GitHub Flavored Markdown (with footnotes and Obsidian's
//! front matter) parsed into styled ranges. Nothing here knows GTK: a front-end turns each
//! style into its own text attributes.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::ops::Range;

/// What a range of a note is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Style {
    /// A heading's line, level 1 to 6.
    Heading(u8),
    Strong,
    Emphasis,
    Strikethrough,
    /// Inline code, its backticks included.
    Code,
    /// A code block's lines, its fences included.
    CodeBlock,
    /// A quote's lines.
    Quote,
    /// A link's or image's text, an autolink, a bare URL, a footnote reference.
    Link,
    /// A list item's bullet or number.
    ListMark,
    /// A task's `[ ]` (false) or `[x]` (true).
    Task(bool),
    /// A table's lines.
    Table,
    /// A table's header row.
    TableHeader,
    /// Markdown's own syntax, shown dimmed: `**`, `#`, `>`, a link's target, fences, a table's
    /// pipes, HTML, rules, front matter.
    Mark,
}

/// `style` from character `start` to `end` (character offsets, as GTK's text buffer counts).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub style: Style,
}

/// An element being parsed: where it is and what its children cover, so that the rest of it
/// is its Markdown marks.
struct Open {
    range: Range<usize>,
    marks: bool,
    children: Vec<Range<usize>>,
}

/// The styled ranges of `text`, in the order found. Ranges of different styles overlap
/// (strong text in a heading, marks in a link).
pub fn spans(text: &str) -> Vec<Span> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_GFM
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS;
    let parser = Parser::new_ext(text, options).into_offset_iter();
    let mut out: Vec<(Range<usize>, Style)> = Vec::new();
    // Link reference definitions ("[id]: url") make no events.
    for (_, def) in parser.reference_definitions().iter() {
        out.push((def.span.clone(), Style::Mark));
    }
    let mut stack: Vec<Open> = Vec::new();
    let mut in_code_block = false;
    let mut in_link = 0;
    // Text the parser hands over in pieces (it splits at `_`, `*`…), joined to find URLs in.
    let mut run: Option<Range<usize>> = None;
    let urls = |run: Option<Range<usize>>, out: &mut Vec<(Range<usize>, Style)>| {
        if let Some(r) = run {
            for url in bare_urls(&text[r.clone()]) {
                out.push((r.start + url.start..r.start + url.end, Style::Link));
            }
        }
    };
    for (event, range) in parser {
        match (&event, &mut run) {
            (Event::Text(_), Some(r)) if r.end == range.start => {
                r.end = range.end;
            }
            (Event::Text(_), _) if !in_code_block && in_link == 0 => {
                urls(run.replace(range.clone()), &mut out);
            }
            _ => urls(run.take(), &mut out),
        }
        // Where what came before in the same parent ends: an escape's backslash is in no event.
        let before = stack
            .last()
            .map_or(0, |p| p.children.last().map_or(p.range.start, |c| c.end));
        if let Some(parent) = stack.last_mut()
            && !matches!(event, Event::End(_))
        {
            parent.children.push(range.clone());
        }
        match event {
            Event::Start(tag) => {
                let marks = match &tag {
                    Tag::Heading { level, .. } => {
                        out.push((range.clone(), Style::Heading(heading(*level))));
                        true
                    }
                    Tag::Strong => style(&mut out, &range, Style::Strong),
                    Tag::Emphasis => style(&mut out, &range, Style::Emphasis),
                    Tag::Strikethrough => style(&mut out, &range, Style::Strikethrough),
                    Tag::Link { .. } | Tag::Image { .. } => {
                        in_link += 1;
                        true
                    }
                    Tag::CodeBlock(kind) => {
                        in_code_block = true;
                        out.push((whole_lines(text, &range), Style::CodeBlock));
                        matches!(kind, CodeBlockKind::Fenced(_))
                    }
                    Tag::BlockQuote(_) => {
                        out.push((whole_lines(text, &range), Style::Quote));
                        quote_marks(text, &range, &mut out);
                        false
                    }
                    Tag::Table(_) => {
                        out.push((whole_lines(text, &range), Style::Table));
                        true
                    }
                    Tag::TableHead => style(&mut out, &range, Style::TableHeader),
                    Tag::TableRow | Tag::FootnoteDefinition(_) => true,
                    Tag::HtmlBlock | Tag::MetadataBlock(_) => {
                        out.push((range.clone(), Style::Mark));
                        false
                    }
                    _ => false,
                };
                stack.push(Open {
                    range,
                    marks,
                    children: Vec::new(),
                });
            }
            Event::End(end) => {
                let Some(open) = stack.pop() else { continue };
                match end {
                    TagEnd::Link | TagEnd::Image => {
                        in_link -= 1;
                        for c in &open.children {
                            out.push((c.clone(), Style::Link));
                        }
                    }
                    TagEnd::CodeBlock => in_code_block = false,
                    // The bullet or number: from the item's start to its first content.
                    TagEnd::Item => {
                        let line_end = line_end(text, open.range.start);
                        let first = open.children.first().map_or(line_end, |c| c.start);
                        out.push((open.range.start..first.min(line_end), Style::ListMark));
                    }
                    _ => {}
                }
                if open.marks {
                    for gap in gaps(&open.range, open.children) {
                        out.push((gap, Style::Mark));
                    }
                }
            }
            Event::Code(_) => {
                out.push((range.clone(), Style::Code));
                let ticks = text[range.clone()]
                    .bytes()
                    .take_while(|&b| b == b'`')
                    .count();
                out.push((range.start..range.start + ticks, Style::Mark));
                out.push((range.end - ticks..range.end, Style::Mark));
            }
            // An escaped mark ("\*"): its backslash, just before the text, is Markdown's.
            Event::Text(_) if range.start > before && text.as_bytes()[range.start - 1] == b'\\' => {
                out.push((range.start - 1..range.start, Style::Mark))
            }
            Event::TaskListMarker(done) => out.push((range, Style::Task(done))),
            Event::FootnoteReference(_) => {
                out.push((range.clone(), Style::Link));
                out.push((range.start..range.start + 2, Style::Mark));
                out.push((range.end - 1..range.end, Style::Mark));
            }
            Event::Html(_) | Event::InlineHtml(_) | Event::Rule | Event::HardBreak => {
                out.push((range, Style::Mark))
            }
            _ => {}
        }
    }
    urls(run, &mut out);
    to_chars(text, out)
}

/// Push `style` over `range`; its marks are what its children leave.
fn style(out: &mut Vec<(Range<usize>, Style)>, range: &Range<usize>, style: Style) -> bool {
    out.push((range.clone(), style));
    true
}

fn heading(level: HeadingLevel) -> u8 {
    level as u8
}

/// What of `range` its children do not cover.
fn gaps(range: &Range<usize>, mut children: Vec<Range<usize>>) -> Vec<Range<usize>> {
    children.sort_by_key(|c| c.start);
    let mut out = Vec::new();
    let mut at = range.start;
    for c in children {
        if c.start > at {
            out.push(at..c.start.min(range.end));
        }
        at = at.max(c.end);
    }
    if at < range.end {
        out.push(at..range.end);
    }
    out
}

/// The `>` at the start of each line of a quote, with the space after each.
fn quote_marks(text: &str, range: &Range<usize>, out: &mut Vec<(Range<usize>, Style)>) {
    let bytes = text.as_bytes();
    let mut line = range.start;
    while line < range.end {
        let mut i = line;
        loop {
            while i < range.end && matches!(bytes[i], b' ' | b'\t') {
                i += 1;
            }
            if i >= range.end || bytes[i] != b'>' {
                break;
            }
            let end = if bytes.get(i + 1) == Some(&b' ') {
                i + 2
            } else {
                i + 1
            };
            out.push((i..end.min(range.end), Style::Mark));
            i = end;
        }
        line = line_end(text, line) + 1;
    }
}

/// `range` from the start of its first line: a block's line background and margin apply only
/// to lines the style starts on (an indented code block's range starts after the indent).
fn whole_lines(text: &str, range: &Range<usize>) -> Range<usize> {
    text[..range.start].rfind('\n').map_or(0, |n| n + 1)..range.end
}

/// The byte where the line holding `at` ends (its `\n`, or the text's end).
fn line_end(text: &str, at: usize) -> usize {
    text[at..].find('\n').map_or(text.len(), |n| at + n)
}

/// Bare URLs in `text` (GitHub's extended autolinks): `http://`, `https://` or `www.` up to a
/// space, less the punctuation that ends a sentence.
fn bare_urls(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(at) = ["https://", "http://", "www."]
        .iter()
        .filter_map(|p| text[from..].find(p).map(|i| (from + i, p.len())))
        .min()
    {
        let (start, prefix) = at;
        let starts_word = text[..start]
            .chars()
            .next_back()
            .is_none_or(|c| c.is_whitespace() || "(*_~".contains(c));
        let mut end = text[start..]
            .find(|c: char| c.is_whitespace() || c == '<')
            .map_or(text.len(), |n| start + n);
        while end > start + prefix
            && text[..end].ends_with(['.', ',', ':', ';', '!', '?', '*', '_', '~', '\'', '"'])
        {
            end -= 1;
        }
        // A closing parenthesis without its opening one ends the sentence, not the URL.
        while text[start..end].ends_with(')')
            && text[start..end].matches(')').count() > text[start..end].matches('(').count()
        {
            end -= 1;
        }
        if starts_word && end > start + prefix {
            out.push(start..end);
        }
        from = end.max(start + prefix);
    }
    out
}

/// Byte ranges as character ranges; empty ones dropped.
fn to_chars(text: &str, ranges: Vec<(Range<usize>, Style)>) -> Vec<Span> {
    let mut chars = vec![0u32; text.len() + 1];
    let mut n = 0;
    for (i, c) in text.char_indices() {
        for b in 0..c.len_utf8() {
            chars[i + b] = n;
        }
        n += 1;
    }
    chars[text.len()] = n;
    ranges
        .into_iter()
        .filter(|(r, _)| r.start < r.end && r.end <= text.len())
        .map(|(r, style)| Span {
            start: chars[r.start] as usize,
            end: chars[r.end] as usize,
            style,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pieces of `text` with `style`.
    fn of(text: &str, style: Style) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        let mut found: Vec<(usize, String)> = spans(text)
            .into_iter()
            .filter(|s| s.style == style)
            .map(|s| (s.start, chars[s.start..s.end].iter().collect()))
            .collect();
        found.sort();
        found.dedup();
        found.into_iter().map(|(_, s)| s).collect()
    }

    #[test]
    fn inline_styles_and_their_marks() {
        let t = "Zażółć **bold _and_ more** ~~gone~~ `a ``b`` c` ***both***\n";
        assert_eq!(of(t, Style::Strong), ["**bold _and_ more**", "**both**"]);
        assert_eq!(of(t, Style::Emphasis), ["_and_", "***both***"]);
        assert_eq!(of(t, Style::Strikethrough), ["~~gone~~"]);
        assert_eq!(of(t, Style::Code), ["`a ``b`` c`"]);
        assert_eq!(
            of(t, Style::Mark),
            [
                "**", "_", "_", "**", "~~", "~~", "`", "`", "*", "**", "**", "*"
            ]
        );
    }

    #[test]
    fn headings_quotes_and_lists() {
        let t =
            "# One #\n\nTwo\n---\n\n> quoted\n> > nested\n\n- a\n  1. b\n- [ ] todo\n- [x] done\n";
        assert_eq!(of(t, Style::Heading(1)), ["# One #\n"]);
        assert_eq!(of(t, Style::Heading(2)), ["Two\n---\n"]);
        assert_eq!(
            of(t, Style::Quote),
            ["> quoted\n> > nested\n", "> > nested\n"]
        );
        assert_eq!(of(t, Style::ListMark), ["- ", "1. ", "- ", "- "]);
        assert_eq!(of(t, Style::Task(false)), ["[ ]"]);
        assert_eq!(of(t, Style::Task(true)), ["[x]"]);
        let marks = of(t, Style::Mark);
        for m in ["# ", " #\n", "\n---\n", "> "] {
            assert!(marks.contains(&m.to_string()), "{m:?} in {marks:?}");
        }
    }

    #[test]
    fn links_code_blocks_and_tables() {
        let t = "[text](http://a.b \"t\") <http://c.d> see https://e.f/g_(h). ![alt](i.png) [^1]\n\n\
                 ```rust\nlet a = 1;\n```\n\n| A | B |\n|:--|--:|\n| 1 | 2 |\n\n[^1]: Note.\n";
        assert_eq!(
            of(t, Style::Link),
            ["text", "http://c.d", "https://e.f/g_(h)", "alt", "[^1]"]
        );
        let marks = of(t, Style::Mark);
        for m in [
            "](http://a.b \"t\")",
            "<",
            ">",
            "![",
            "```rust\n",
            "```",
            "|:--|--:|\n",
        ] {
            assert!(marks.contains(&m.to_string()), "{m:?} in {marks:?}");
        }
        assert_eq!(of(t, Style::CodeBlock), ["```rust\nlet a = 1;\n```"]);
        assert_eq!(of(t, Style::Table), ["| A | B |\n|:--|--:|\n| 1 | 2 |\n"]);
        assert_eq!(of(t, Style::TableHeader), ["| A | B |\n"]);
    }

    #[test]
    fn blocks_take_whole_lines_and_escapes_dim_their_backslash() {
        let t = "Text \\*plain\\*.\n\n    indented code\n\n- > quoted in a list\n";
        assert_eq!(of(t, Style::CodeBlock), ["    indented code\n"]);
        assert_eq!(of(t, Style::Quote), ["- > quoted in a list\n"]);
        assert_eq!(of(t, Style::Emphasis), Vec::<String>::new());
        assert_eq!(of(t, Style::Mark), ["\\", "\\", "> "]);
    }

    #[test]
    fn bare_urls_stop_at_punctuation() {
        fn urls(t: &str) -> Vec<&str> {
            bare_urls(t).into_iter().map(|r| &t[r]).collect()
        }
        assert_eq!(
            urls("(see www.a.b/c), https://d.e?f=1. no:http://x"),
            ["www.a.b/c", "https://d.e?f=1"]
        );
        assert!(urls("https:// www.").is_empty());
    }

    #[test]
    fn the_reference_note_parses_whole() {
        let t = include_str!("../tests/reference.md");
        let n = t.chars().count();
        let s = spans(t);
        assert!(s.iter().all(|s| s.start < s.end && s.end <= n));
        for style in [
            Style::Heading(1),
            Style::Heading(6),
            Style::Strong,
            Style::Emphasis,
            Style::Strikethrough,
            Style::Code,
            Style::CodeBlock,
            Style::Quote,
            Style::Link,
            Style::ListMark,
            Style::Task(true),
            Style::Task(false),
            Style::Table,
            Style::TableHeader,
            Style::Mark,
        ] {
            assert!(s.iter().any(|s| s.style == style), "no {style:?}");
        }
    }
}
