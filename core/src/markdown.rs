//! How a note looks in Editor mode: GitHub Flavored Markdown (with footnotes and Obsidian's
//! front matter) parsed into styled ranges. Nothing here knows GTK: a front-end turns each
//! style into its own text attributes.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd};
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

/// A change to a note's text: characters `start` to `end` replaced by `text`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

pub(crate) fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_GFM
        | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_WIKILINKS
}

/// The styled ranges of `text`, in the order found. Ranges of different styles overlap
/// (strong text in a heading, marks in a link).
pub fn spans(text: &str) -> Vec<Span> {
    let parser = Parser::new_ext(text, options()).into_offset_iter();
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

/// What a link points to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    /// An address with a scheme, opened by another app: a web page, `mailto:`.
    Address(String),
    /// A note: from a Markdown link, a path relative to the linking note's folder (decoded, `.md`
    /// optional; "" for that note itself, `/` first for the notes folder); from a wiki link
    /// (`wiki`), a title or a path from the notes folder, without `.md`. With the heading after
    /// a `#`.
    Note {
        target: String,
        heading: Option<String>,
        wiki: bool,
    },
}

/// What a link's target is, if it is one to follow: not an image's, nor a file other than a
/// note's.
fn link(kind: LinkType, dest: &str) -> Option<Link> {
    let (target, heading) = match dest.split_once('#') {
        Some((t, h)) => (t, Some(h.to_string())),
        None => (dest, None),
    };
    let note = |target: String, wiki| {
        Some(Link::Note {
            target,
            heading: heading.clone(),
            wiki,
        })
    };
    match kind {
        LinkType::Email => Some(Link::Address(format!("mailto:{dest}"))),
        LinkType::WikiLink { .. } => note(target.trim().to_string(), true),
        _ if has_scheme(dest) => Some(Link::Address(dest.to_string())),
        _ if dest.is_empty() => None,
        _ => {
            let target = crate::dav::percent_decode(target);
            let name = target.rsplit('/').next().unwrap_or_default();
            let note_file = !name.contains('.') || name.ends_with(".md") || name.ends_with(".txt");
            note_file.then(|| note(target, false)).flatten()
        }
    }
}

/// Whether `url` starts with a scheme ("https:", "mailto:").
fn has_scheme(url: &str) -> bool {
    url.split_once(':').is_some_and(|(scheme, _)| {
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
    })
}

/// The link at character `at`: a link's or a wiki link's target, an autolink (an e-mail one
/// as `mailto:`), a bare URL (a `www.` one as `https://`), an image's address (not its file).
pub fn link_at(text: &str, at: usize) -> Option<Link> {
    let byte = text.char_indices().nth(at)?.0;
    let parser = Parser::new_ext(text, options()).into_offset_iter();
    for (event, _) in parser.filter(|(_, r)| r.contains(&byte)) {
        match event {
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                ..
            }) => return link(link_type, &dest_url),
            Event::Start(Tag::Image { dest_url, .. }) => {
                return has_scheme(&dest_url).then(|| Link::Address(dest_url.to_string()));
            }
            Event::Text(_) => {
                let start = text[..byte].rfind('\n').map_or(0, |n| n + 1);
                let line = &text[start..line_end(text, byte)];
                let url = bare_urls(line)
                    .into_iter()
                    .find(|u| u.contains(&(byte - start)))?;
                let url = &line[url];
                return if url.starts_with("www.") {
                    Some(Link::Address(format!("https://{url}")))
                } else {
                    has_scheme(url).then(|| Link::Address(url.to_string()))
                };
            }
            Event::Code(_) | Event::Start(Tag::CodeBlock(_)) => return None,
            _ => {}
        }
    }
    None
}

/// The links to notes in `text` (`Link::Note`), in order.
pub fn links(text: &str) -> Vec<Link> {
    Parser::new_ext(text, options())
        .filter_map(|event| match event {
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                ..
            }) => link(link_type, &dest_url),
            _ => None,
        })
        .filter(|l| matches!(l, Link::Note { .. }))
        .collect()
}

/// Where the heading titled `heading` starts in `text` (a character), its case and spaces as a
/// link writes them aside.
pub fn heading_at(text: &str, heading: &str) -> Option<usize> {
    let wanted = heading.trim().to_lowercase();
    let mut current: Option<(usize, String)> = None;
    for (event, range) in Parser::new_ext(text, options()).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { .. }) => current = Some((range.start, String::new())),
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, title)) = &mut current {
                    title.push_str(&t);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((start, title)) = current.take()
                    && title.trim().to_lowercase() == wanted
                {
                    return Some(text[..start].chars().count());
                }
            }
            _ => {}
        }
    }
    None
}

/// What Enter at character `at` does in a list item, in the order of the text (none: a plain
/// new line). In an item's text it starts the next item: the same bullet, the next number (the
/// numbers after it moved up while they count on), an open task after a task. In an empty
/// item it ends the list: the line keeps only a quote's `>`.
pub fn enter(text: &str, at: usize) -> Vec<Edit> {
    let Some(byte) = text
        .char_indices()
        .map(|(i, _)| i)
        .chain([text.len()])
        .nth(at)
    else {
        return Vec::new();
    };
    let start = text[..byte].rfind('\n').map_or(0, |n| n + 1);
    let end = line_end(text, byte);
    let line = &text[start..end];
    let Some(item) = ListItem::parse(line) else {
        return Vec::new();
    };
    if byte < start + item.content {
        return Vec::new();
    }
    // The parser decides what is an item (not in a code block, not "1986\.").
    let Some(after) = items_after(text, start + item.prefix.len()) else {
        return Vec::new();
    };
    let chars = |b: usize| text[..b].chars().count();
    if line[item.content..].trim().is_empty() {
        let quote = item.prefix.trim_end();
        let keep = if quote.is_empty() {
            String::new()
        } else {
            format!("{quote} ")
        };
        return vec![Edit {
            start: chars(start),
            end: chars(end),
            text: keep,
        }];
    }
    let task = if item.task { "[ ] " } else { "" };
    let spacing = if item.spacing.is_empty() {
        " "
    } else {
        item.spacing
    };
    let next = match item.number {
        Some((n, delimiter)) => format!("{}{delimiter}", n + 1),
        None => item.marker.to_string(),
    };
    let mut edits = vec![Edit {
        start: at,
        end: at,
        text: format!("\n{}{next}{spacing}{task}", item.prefix),
    }];
    // Renumber the items after it that counted on from it.
    if let Some((n, _)) = item.number {
        let mut expected = n + 1;
        for s in after {
            let digits = text[s..].bytes().take_while(u8::is_ascii_digit).count();
            if text[s..s + digits].parse() != Ok(expected) {
                break;
            }
            expected += 1;
            edits.push(Edit {
                start: chars(s),
                end: chars(s + digits),
                text: expected.to_string(),
            });
        }
    }
    edits
}

/// A line that starts like a list item.
struct ListItem<'a> {
    /// Indentation and a quote's `>`s before the marker.
    prefix: &'a str,
    marker: &'a str,
    /// An ordered item's number and its `.` or `)`.
    number: Option<(u64, char)>,
    spacing: &'a str,
    task: bool,
    /// Where its text starts in the line.
    content: usize,
}

impl<'a> ListItem<'a> {
    fn parse(line: &'a str) -> Option<Self> {
        let prefix_len = line.len() - line.trim_start_matches([' ', '\t', '>']).len();
        let (prefix, rest) = line.split_at(prefix_len);
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        let (marker, number) = match rest.as_bytes().first()? {
            b'-' | b'*' | b'+' => (&rest[..1], None),
            _ if (1..=9).contains(&digits)
                && matches!(rest.as_bytes()[digits..].first(), Some(b'.' | b')')) =>
            {
                let delimiter = rest.as_bytes()[digits] as char;
                (
                    &rest[..digits + 1],
                    Some((rest[..digits].parse().ok()?, delimiter)),
                )
            }
            _ => return None,
        };
        let after = &rest[marker.len()..];
        let spacing = &after[..after.len() - after.trim_start_matches([' ', '\t']).len()];
        if spacing.is_empty() && !after.is_empty() {
            return None;
        }
        let mut content = prefix.len() + marker.len() + spacing.len();
        let task = matches!(line.get(content..content + 3), Some("[ ]" | "[x]" | "[X]"))
            && matches!(line.as_bytes().get(content + 3), None | Some(b' '));
        if task {
            content = (content + 4).min(line.len());
        }
        Some(Self {
            prefix,
            marker,
            number,
            spacing,
            task,
            content,
        })
    }
}

/// The starts of the items after the one starting at byte `item`, in its list; None when no
/// item starts there.
fn items_after(text: &str, item: usize) -> Option<Vec<usize>> {
    let mut lists: Vec<Vec<usize>> = Vec::new();
    for (event, range) in Parser::new_ext(text, options()).into_offset_iter() {
        match event {
            Event::Start(Tag::List(_)) => lists.push(Vec::new()),
            Event::Start(Tag::Item) => {
                if let Some(items) = lists.last_mut() {
                    items.push(range.start);
                }
            }
            Event::End(TagEnd::List(_)) => {
                let items = lists.pop().unwrap_or_default();
                if let Some(i) = items.iter().position(|&s| s == item) {
                    return Some(items[i + 1..].to_vec());
                }
            }
            _ => {}
        }
    }
    None
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
    fn links_open_their_address() {
        let t = "Zażółć [the *text*](https://a.b/c \"t\") ![i](https://d.e/i.png) [rel](x.md)\n\
                 <me@f.g> [ref][r] see www.h.i/j_(k). `https://code` and https://l.m\n\n\
                 [r]: http://n.o\n";
        let at = |s: &str| match link_at(t, t[..t.find(s).unwrap()].chars().count()) {
            Some(Link::Address(a)) => Some(a),
            Some(other) => panic!("{other:?}"),
            None => None,
        };
        assert_eq!(at("text").as_deref(), Some("https://a.b/c"));
        assert_eq!(at("](https://a").as_deref(), Some("https://a.b/c"));
        assert_eq!(at("[i]").as_deref(), Some("https://d.e/i.png"));
        assert_eq!(at("me@").as_deref(), Some("mailto:me@f.g"));
        assert_eq!(at("ref").as_deref(), Some("http://n.o"));
        assert_eq!(at("(k)").as_deref(), Some("https://www.h.i/j_(k)"));
        assert_eq!(at("code"), None);
        assert_eq!(at("l.m").as_deref(), Some("https://l.m"));
        assert_eq!(at("see"), None);
        assert_eq!(at("Zażółć"), None);
    }

    #[test]
    fn links_to_notes() {
        let note = |target: &str, heading: Option<&str>, wiki| Link::Note {
            target: target.into(),
            heading: heading.map(Into::into),
            wiki,
        };
        let t = "[a](Other.md) [b](../Up%20One.md#Part) [c](#Here) [d](Plain) [e](pic.png)\n\
                 [[Wiki]] [[Dir/Name|shown]] [[Wiki#Sub part]] ![[img.png]] ![f](g.md)\n";
        let at = |s: &str| link_at(t, t[..t.find(s).unwrap()].chars().count());
        assert_eq!(at("a]"), Some(note("Other.md", None, false)));
        assert_eq!(at("b]"), Some(note("../Up One.md", Some("Part"), false)));
        assert_eq!(at("c]"), Some(note("", Some("Here"), false)));
        assert_eq!(at("d]"), Some(note("Plain", None, false)));
        assert_eq!(at("e]"), None);
        assert_eq!(at("Wiki]]"), Some(note("Wiki", None, true)));
        assert_eq!(at("shown"), Some(note("Dir/Name", None, true)));
        assert_eq!(at("Sub"), Some(note("Wiki", Some("Sub part"), true)));
        assert_eq!(at("img"), None);
        assert_eq!(at("f]"), None);
        assert_eq!(links(t).len(), 7);
        assert_eq!(
            of(t, Style::Link),
            [
                "a",
                "b",
                "c",
                "d",
                "e",
                "Wiki",
                "shown",
                "Wiki#Sub part",
                "img.png",
                "f"
            ]
        );
    }

    #[test]
    fn headings_are_found_by_their_title() {
        let t = "Intro\n\n## Sub *part*\n\nText\n\nŁódź\n=====\n";
        assert_eq!(heading_at(t, "sub part"), Some(7));
        assert_eq!(
            heading_at(t, "Łódź "),
            Some(t[..t.find("Łódź").unwrap()].chars().count())
        );
        assert_eq!(heading_at(t, "Text"), None);
    }

    /// `text` (with `|` where the cursor is) after Enter, with `|` where the cursor goes.
    fn after_enter(text: &str) -> String {
        let at = text.find('|').unwrap();
        let mut t: Vec<char> = text.replace('|', "").chars().collect();
        let at = text[..at].chars().count();
        let edits = enter(&t.iter().collect::<String>(), at);
        let mut cursor = at;
        for e in edits.iter().rev() {
            t.splice(e.start..e.end, e.text.chars());
        }
        if let Some(first) = edits.first() {
            cursor = first.start + first.text.chars().count();
        } else {
            t.insert(at, '\n');
            cursor += 1;
        }
        t.insert(cursor, '|');
        t.into_iter().collect()
    }

    #[test]
    fn enter_continues_a_list() {
        assert_eq!(after_enter("- a|"), "- a\n- |");
        assert_eq!(after_enter("* a|b"), "* a\n* |b");
        assert_eq!(after_enter("- [x] done|"), "- [x] done\n- [ ] |");
        assert_eq!(after_enter("> - ą|\n"), "> - ą\n> - |\n");
        assert_eq!(after_enter("- a\n  - b|"), "- a\n  - b\n  - |");
        assert_eq!(after_enter("3) a|"), "3) a\n4) |");
        assert_eq!(
            after_enter("1. a|\n2. b\n   1. c\n3. d\n\n5. e\n"),
            "1. a\n2. |\n3. b\n   1. c\n4. d\n\n5. e\n"
        );
        // Items numbered alike stay so.
        assert_eq!(after_enter("1. a|\n1. b\n"), "1. a\n2. |\n1. b\n");
    }

    #[test]
    fn enter_on_an_empty_item_ends_the_list() {
        assert_eq!(after_enter("- a\n- |"), "- a\n|");
        assert_eq!(after_enter("- a\n- [ ] |\n"), "- a\n|\n");
        assert_eq!(after_enter("> - a\n> - |"), "> - a\n> |");
    }

    #[test]
    fn enter_elsewhere_is_a_new_line() {
        for t in [
            "|- a",
            "-| a",
            "a|",
            "```\n- a|\n```",
            "1986\\. a|",
            "* * *|",
            "-a|",
        ] {
            assert_eq!(after_enter(t), t.replace('|', "\n|"), "{t:?}");
        }
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
