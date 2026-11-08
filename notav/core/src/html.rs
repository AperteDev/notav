//! Editor mode's paste of HTML (copied from a web page, an email, a document) as Markdown:
//! headings, emphasis, links, lists, quotes, code, rules, images and tables kept, the page's
//! layout left out.

use crate::table;

/// The Markdown for `html`, or `None` when it keeps no formatting (only text and layout, as a
/// code editor's copy has), so that its plain text is pasted instead.
pub fn to_markdown(html: &str) -> Option<String> {
    let mut w = Writer::default();
    let mut blocks = w.blocks(&parse(html).children);
    // Part of a heading's line pasted into a line is its text.
    if let [(text, Kind::Heading)] = &mut blocks[..] {
        *text = text.trim_start_matches('#').trim_start().to_string();
        w.marks -= 1;
    }
    (w.marks > 0).then(|| join(&blocks, false))
}

enum Node {
    Element(Element),
    Text(String),
}

#[derive(Default)]
struct Element {
    /// Lower case.
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<Node>,
}

impl Element {
    fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];
/// Elements whose text is not markup, all left out.
const RAW: &[&str] = &["script", "style", "title", "textarea", "xmp"];
const BLOCK: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "body",
    "caption",
    "center",
    "dd",
    "details",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hr",
    "html",
    "li",
    "main",
    "menu",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "summary",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "tr",
    "ul",
];

/// The tree of `html`, as forgiving as a browser for what clipboards hold: unclosed elements
/// end with their parent, a stray end tag is skipped.
fn parse(html: &str) -> Element {
    let mut stack = vec![Element::default()];
    let mut rest = html;
    while !rest.is_empty() {
        let lt = rest.find('<').unwrap_or(rest.len());
        if lt > 0 {
            text(&mut stack, &rest[..lt]);
            rest = &rest[lt..];
            continue;
        }
        if let Some(r) = rest.strip_prefix("<!--") {
            rest = r.find("-->").map_or("", |e| &r[e + 3..]);
        } else if rest.starts_with("<!") || rest.starts_with("<?") {
            rest = rest.find('>').map_or("", |e| &rest[e + 1..]);
        } else if let Some(r) = rest.strip_prefix("</") {
            close(&mut stack, &name(r));
            rest = r.find('>').map_or("", |e| &r[e + 1..]);
        } else if rest[1..].starts_with(|c: char| c.is_ascii_alphabetic()) {
            let (element, r) = start_tag(&rest[1..]);
            rest = r;
            let name = element.name.clone();
            open(&mut stack, element);
            if RAW.contains(&name.as_str()) {
                let end = rest.to_ascii_lowercase().find(&format!("</{name}"));
                rest = end.map_or("", |e| &rest[e..]);
            } else if VOID.contains(&name.as_str()) {
                close(&mut stack, &name);
            }
        } else {
            text(&mut stack, "<");
            rest = &rest[1..];
        }
    }
    while stack.len() > 1 {
        pop(&mut stack);
    }
    stack.pop().unwrap()
}

fn name(tag: &str) -> String {
    tag.chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '-'))
        .collect::<String>()
        .to_ascii_lowercase()
}

/// The element of a start tag (after its `<`), and what follows the tag.
fn start_tag(tag: &str) -> (Element, &str) {
    let name = name(tag);
    let mut element = Element {
        name,
        ..Element::default()
    };
    let mut rest = &tag[element.name.len()..];
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '/');
        if rest.is_empty() || rest.starts_with('>') {
            return (element, rest.get(1..).unwrap_or(""));
        }
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, '=' | '>' | '/'))
            .unwrap_or(rest.len());
        let attr = rest[..end].to_ascii_lowercase();
        rest = rest[end..].trim_start();
        let mut value = String::new();
        if let Some(r) = rest.strip_prefix('=') {
            let r = r.trim_start();
            let (v, after) = match r.chars().next() {
                Some(q @ ('"' | '\'')) => match r[1..].find(q) {
                    Some(e) => (&r[1..e + 1], &r[e + 2..]),
                    None => (&r[1..], ""),
                },
                _ => {
                    let e = r
                        .find(|c: char| c.is_whitespace() || c == '>')
                        .unwrap_or(r.len());
                    (&r[..e], &r[e..])
                }
            };
            value = decode(v);
            rest = after;
        }
        element.attrs.push((attr, value));
    }
}

fn open(stack: &mut Vec<Element>, element: Element) {
    // The end tags HTML lets out: an item, a cell, a row or a paragraph ends at the next.
    let (same, within): (&[&str], &[&str]) = match element.name.as_str() {
        "li" => (&["li"], &["ul", "ol", "menu"]),
        "dt" | "dd" => (&["dt", "dd"], &["dl"]),
        "td" | "th" => (&["td", "th"], &["tr", "table"]),
        "tr" => (&["tr"], &["table"]),
        "p" => (&["p"], BLOCK),
        _ => (&[], &[]),
    };
    let open = stack
        .iter()
        .rposition(|e| same.contains(&e.name.as_str()) || within.contains(&e.name.as_str()));
    if let Some(i) = open.filter(|&i| i > 0 && same.contains(&stack[i].name.as_str())) {
        while stack.len() > i {
            pop(stack);
        }
    }
    stack.push(element);
}

fn close(stack: &mut Vec<Element>, name: &str) {
    if let Some(i) = stack
        .iter()
        .rposition(|e| e.name == name)
        .filter(|&i| i > 0)
    {
        while stack.len() > i {
            pop(stack);
        }
    }
}

fn pop(stack: &mut Vec<Element>) {
    let element = stack.pop().unwrap();
    stack
        .last_mut()
        .unwrap()
        .children
        .push(Node::Element(element));
}

fn text(stack: &mut [Element], text: &str) {
    let children = &mut stack.last_mut().unwrap().children;
    match children.last_mut() {
        Some(Node::Text(t)) => t.push_str(&decode(text)),
        _ => children.push(Node::Text(decode(text))),
    }
}

/// `text` with its character references replaced.
fn decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let reference = rest[1..]
            .find(';')
            .filter(|&e| e <= 32)
            .and_then(|e| Some((reference(&rest[1..e + 1])?, e + 2)));
        match reference {
            Some((c, len)) => {
                out.push(c);
                rest = &rest[len..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn reference(name: &str) -> Option<char> {
    if let Some(number) = name.strip_prefix('#') {
        let n = match number.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => number.parse().ok()?,
        };
        return Some(
            char::from_u32(n)
                .filter(|&c| c != '\0')
                .unwrap_or('\u{fffd}'),
        );
    }
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{a0}',
        "shy" => '\u{ad}',
        "zwnj" => '\u{200c}',
        "zwj" => '\u{200d}',
        "ndash" => '–',
        "mdash" => '—',
        "hellip" => '…',
        "bull" => '•',
        "middot" => '·',
        "lsquo" => '‘',
        "rsquo" => '’',
        "sbquo" => '‚',
        "ldquo" => '“',
        "rdquo" => '”',
        "bdquo" => '„',
        "laquo" => '«',
        "raquo" => '»',
        "copy" => '©',
        "reg" => '®',
        "trade" => '™',
        "euro" => '€',
        "pound" => '£',
        "deg" => '°',
        "times" => '×',
        "sect" => '§',
        "rarr" => '→',
        "larr" => '←',
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Text,
    Heading,
    List,
    Other,
}

type Block = (String, Kind);

/// Blocks one after another: a blank line between, or (`tight`, in a list item) a line break
/// before a list.
fn join(blocks: &[Block], tight: bool) -> String {
    let mut out = String::new();
    for (i, (text, kind)) in blocks.iter().enumerate() {
        if i > 0 {
            out.push_str(if tight && *kind == Kind::List {
                "\n"
            } else {
                "\n\n"
            });
        }
        out.push_str(text);
    }
    out
}

/// The formatting the text is in.
#[derive(Default, Clone, Copy)]
struct Inline {
    strong: bool,
    em: bool,
    strike: bool,
    link: bool,
}

#[derive(Default)]
struct Writer {
    /// How much Markdown it wrote: emphasis, links, lists and the like.
    marks: usize,
}

impl Writer {
    fn blocks(&mut self, nodes: &[Node]) -> Vec<Block> {
        let mut out = Vec::new();
        let mut line = String::new();
        for node in nodes {
            match node {
                Node::Element(e) if BLOCK.contains(&e.name.as_str()) || has_block(e) => {
                    paragraph(&mut line, &mut out);
                    self.block(e, &mut out);
                }
                _ => self.inline(node, &mut line, Inline::default()),
            }
        }
        paragraph(&mut line, &mut out);
        out
    }

    fn block(&mut self, e: &Element, out: &mut Vec<Block>) {
        match e.name.as_str() {
            "head" => {}
            h @ ("h1" | "h2" | "h3" | "h4" | "h5" | "h6") => {
                let mut text = String::new();
                let s = Inline {
                    strong: true,
                    ..Inline::default()
                };
                for c in &e.children {
                    self.inline(c, &mut text, s);
                }
                let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
                if !text.is_empty() {
                    self.marks += 1;
                    let level = usize::from(h.as_bytes()[1] - b'0');
                    out.push((format!("{} {text}", "#".repeat(level)), Kind::Heading));
                }
            }
            "ul" | "ol" | "menu" => self.list(e, out),
            "blockquote" => {
                let inner = join(&self.blocks(&e.children), false);
                if !inner.is_empty() {
                    self.marks += 1;
                    let quoted = inner
                        .split('\n')
                        .map(|l| {
                            if l.is_empty() {
                                ">".into()
                            } else {
                                format!("> {l}")
                            }
                        })
                        .collect::<Vec<_>>();
                    out.push((quoted.join("\n"), Kind::Other));
                }
            }
            "pre" => {
                let mut code = String::new();
                raw_text(&e.children, &mut code);
                let code = code.strip_prefix('\n').unwrap_or(&code).trim_end();
                if !code.trim().is_empty() {
                    self.marks += 1;
                    let fence = "`".repeat(longest_run(code, '`').max(2) + 1);
                    let language = language(e).unwrap_or_default();
                    out.push((format!("{fence}{language}\n{code}\n{fence}"), Kind::Other));
                }
            }
            "hr" => {
                self.marks += 1;
                out.push(("---".into(), Kind::Other));
            }
            "table" => self.table(e, out),
            _ => out.extend(self.blocks(&e.children)),
        }
    }

    fn list(&mut self, e: &Element, out: &mut Vec<Block>) {
        let mut items: Vec<Vec<Block>> = Vec::new();
        for c in &e.children {
            match c {
                // A list right in a list (not in an item) belongs to the item before it.
                Node::Element(l) if matches!(l.name.as_str(), "ul" | "ol") => {
                    let mut nested = Vec::new();
                    self.list(l, &mut nested);
                    match items.last_mut() {
                        Some(item) => item.extend(nested),
                        None => items.push(nested),
                    }
                }
                Node::Element(li) if li.name == "li" => items.push(self.blocks(&li.children)),
                _ => items.push(self.blocks(std::slice::from_ref(c))),
            }
        }
        items.retain(|i| !i.is_empty());
        if items.is_empty() {
            return;
        }
        self.marks += 1;
        let first: usize = e.attr("start").and_then(|s| s.parse().ok()).unwrap_or(1);
        let loose = items
            .iter()
            .any(|i| i.iter().filter(|b| b.1 != Kind::List).count() > 1);
        let mut text = String::new();
        for (n, item) in items.iter().enumerate() {
            if n > 0 {
                text.push_str(if loose { "\n\n" } else { "\n" });
            }
            let marker = if e.name == "ol" {
                format!("{}. ", first + n)
            } else {
                "- ".into()
            };
            text.push_str(&marker);
            for (i, line) in join(item, true).split('\n').enumerate() {
                if i > 0 {
                    text.push('\n');
                    if !line.is_empty() {
                        text.push_str(&" ".repeat(marker.len()));
                    }
                }
                text.push_str(line);
            }
        }
        out.push((text, Kind::List));
    }

    /// A table of data as a Markdown table; one for layout (as emails are made of) as its
    /// cells' blocks one after another.
    fn table(&mut self, e: &Element, out: &mut Vec<Block>) {
        let mut rows = Vec::new();
        rows_of(e, &mut rows);
        let marks = self.marks;
        let cells: Vec<Vec<Vec<Block>>> = rows
            .iter()
            .map(|r| {
                r.children
                    .iter()
                    .filter_map(|c| match c {
                        Node::Element(c) if matches!(c.name.as_str(), "td" | "th") => {
                            Some(self.blocks(&c.children))
                        }
                        _ => None,
                    })
                    .collect()
            })
            .collect();
        let data = rows.len() > 1
            && e.attr("role") != Some("presentation")
            && cells.iter().any(|r| r.len() > 1)
            && !rows.iter().any(|r| has(r, "table"))
            && cells.iter().flatten().all(|c| match &c[..] {
                [] => true,
                [(_, kind)] => *kind == Kind::Text,
                _ => false,
            });
        if !data {
            out.extend(cells.into_iter().flatten().flatten());
            return;
        }
        self.marks = marks + 1;
        let rows = cells
            .into_iter()
            .map(|r| {
                r.into_iter()
                    .map(|c| {
                        c.first()
                            .map(|(t, _)| t.replace('\n', " ").replace('|', "\\|"))
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .collect();
        out.push((table::from_rows(rows), Kind::Other));
    }

    fn inline(&mut self, node: &Node, out: &mut String, s: Inline) {
        let e = match node {
            Node::Text(t) => return push_text(out, &escape(t)),
            Node::Element(e) => e,
        };
        match e.name.as_str() {
            "br" => {
                trim_spaces(out);
                out.push('\n');
            }
            "img" => {
                let src = e.attr("src").unwrap_or_default();
                if src.starts_with("http://") || src.starts_with("https://") {
                    self.marks += 1;
                    let alt = escape(e.attr("alt").unwrap_or_default());
                    let alt = alt.split_whitespace().collect::<Vec<_>>().join(" ");
                    out.push_str(&format!("![{alt}]({})", url(src)));
                }
            }
            "input" if e.attr("type") == Some("checkbox") => {
                self.marks += 1;
                let checked = e.attr("checked").is_some();
                out.push_str(if checked { "[x] " } else { "[ ] " });
            }
            "code" | "kbd" | "samp" | "tt" => {
                let mut code = String::new();
                raw_text(&e.children, &mut code);
                let code = code.split_whitespace().collect::<Vec<_>>().join(" ");
                if !code.is_empty() {
                    self.marks += 1;
                    let ticks = "`".repeat(longest_run(&code, '`') + 1);
                    let pad = if code.starts_with('`') || code.ends_with('`') {
                        " "
                    } else {
                        ""
                    };
                    out.push_str(&format!("{ticks}{pad}{code}{pad}{ticks}"));
                }
            }
            "a" if !s.link => {
                let href = e.attr("href").unwrap_or_default().trim();
                let mut text = String::new();
                let inner = Inline { link: true, ..s };
                for c in &e.children {
                    self.inline(c, &mut text, inner);
                }
                let skip =
                    href.is_empty() || href.starts_with('#') || href.starts_with("javascript:");
                let (lead, core, trail) = split_spaces(&text);
                if skip || core.is_empty() {
                    return push_inline(out, &text);
                }
                self.marks += 1;
                push_inline(out, lead);
                let bare = href.strip_prefix("mailto:").unwrap_or(href);
                if core == href || core == bare {
                    out.push_str(&format!("<{href}>"));
                } else {
                    out.push_str(&format!("[{core}]({})", url(href)));
                }
                out.push_str(trail);
            }
            _ => {
                let style = Style::of(e);
                let strong = style
                    .strong
                    .unwrap_or(matches!(e.name.as_str(), "b" | "strong"));
                let em = style.em.unwrap_or(matches!(
                    e.name.as_str(),
                    "i" | "em" | "cite" | "var" | "dfn"
                ));
                let strike = style.strike || matches!(e.name.as_str(), "s" | "del" | "strike");
                let inner = Inline {
                    strong: s.strong || strong,
                    em: s.em || em,
                    strike: s.strike || strike,
                    link: s.link,
                };
                let mut text = String::new();
                for c in &e.children {
                    self.inline(c, &mut text, inner);
                }
                let mut marks = String::new();
                for (on, already, mark) in [
                    (strong, s.strong, "**"),
                    (em, s.em, "*"),
                    (strike, s.strike, "~~"),
                ] {
                    if on && !already {
                        marks.push_str(mark);
                    }
                }
                let (lead, core, trail) = split_spaces(&text);
                if marks.is_empty() || core.is_empty() {
                    return push_inline(out, &text);
                }
                self.marks += 1;
                let closing: String = marks.chars().rev().collect();
                push_inline(out, lead);
                out.push_str(&format!("{marks}{core}{closing}"));
                out.push_str(trail);
            }
        }
    }
}

/// The emphasis an element's `style` gives or takes away (as Google Docs marks it).
#[derive(Default)]
struct Style {
    strong: Option<bool>,
    em: Option<bool>,
    strike: bool,
}

impl Style {
    fn of(e: &Element) -> Self {
        let mut style = Style::default();
        let css = e.attr("style").unwrap_or_default().to_ascii_lowercase();
        for declaration in css.split(';') {
            let Some((property, value)) = declaration.split_once(':') else {
                continue;
            };
            let value = value.trim().trim_end_matches("!important").trim();
            match property.trim() {
                "font-weight" => {
                    style.strong = Some(match value {
                        "bold" | "bolder" => true,
                        v => v.parse::<u32>().is_ok_and(|w| w >= 600),
                    })
                }
                "font-style" => style.em = Some(value != "normal"),
                "text-decoration" | "text-decoration-line" => {
                    style.strike = value.contains("line-through")
                }
                _ => {}
            }
        }
        style
    }
}

/// The line so far as a paragraph of `out`, its lines trimmed.
fn paragraph(line: &mut String, out: &mut Vec<Block>) {
    let text = std::mem::take(line);
    let lines: Vec<&str> = text.split('\n').map(str::trim).collect();
    let first = lines.iter().position(|l| !l.is_empty());
    let last = lines.iter().rposition(|l| !l.is_empty());
    let (Some(first), Some(last)) = (first, last) else {
        return;
    };
    let lines: Vec<String> = lines[first..=last]
        .iter()
        .map(|l| escape_start(l))
        .collect();
    out.push((lines.join("\n"), Kind::Text));
}

fn has_block(e: &Element) -> bool {
    e.children.iter().any(|c| match c {
        Node::Element(c) => BLOCK.contains(&c.name.as_str()) || has_block(c),
        Node::Text(_) => false,
    })
}

fn has(e: &Element, name: &str) -> bool {
    e.children.iter().any(|c| match c {
        Node::Element(c) => c.name == name || has(c, name),
        Node::Text(_) => false,
    })
}

/// A table's rows, through its head, bodies and foot, not in tables within.
fn rows_of<'a>(e: &'a Element, rows: &mut Vec<&'a Element>) {
    for c in &e.children {
        if let Node::Element(c) = c {
            match c.name.as_str() {
                "tr" => rows.push(c),
                "thead" | "tbody" | "tfoot" => rows_of(c, rows),
                _ => {}
            }
        }
    }
}

/// The text as it is, with line breaks (of a code block).
fn raw_text(nodes: &[Node], out: &mut String) {
    for node in nodes {
        match node {
            Node::Text(t) => out.push_str(&t.replace('\u{a0}', " ")),
            Node::Element(e) if e.name == "br" => out.push('\n'),
            Node::Element(e) => raw_text(&e.children, out),
        }
    }
}

/// A code block's language, from its class (`language-rust`, as GitHub and highlighters mark it).
fn language(pre: &Element) -> Option<String> {
    let code = pre.children.iter().find_map(|c| match c {
        Node::Element(c) if c.name == "code" => Some(c),
        _ => None,
    });
    [Some(pre), code].into_iter().flatten().find_map(|e| {
        e.attr("class")?.split_whitespace().find_map(|c| {
            let l = c.strip_prefix("language-").or(c.strip_prefix("lang-"))?;
            Some(l.to_string()).filter(|l| !l.is_empty() && !l.contains('`'))
        })
    })
}

fn longest_run(text: &str, c: char) -> usize {
    text.split(|x| x != c).map(str::len).max().unwrap_or(0)
}

/// An HTML text after `out`, its white space (line breaks and no-break spaces too) collapsed to
/// single spaces as a browser shows it; only `<br>` breaks a line.
fn push_text(out: &mut String, text: &str) {
    for c in text.chars() {
        if !c.is_whitespace() {
            out.push(c);
        } else if !out.ends_with([' ', '\n']) {
            out.push(' ');
        }
    }
}

/// Markdown made of inline elements after `out`, without doubling a space.
fn push_inline(out: &mut String, text: &str) {
    if out.ends_with([' ', '\n']) {
        out.push_str(text.trim_start_matches(' '));
    } else {
        out.push_str(text);
    }
}

fn trim_spaces(out: &mut String) {
    let n = out.trim_end_matches(' ').len();
    out.truncate(n);
}

/// `text`'s white space before and after, and what is between.
fn split_spaces(text: &str) -> (&str, &str, &str) {
    let core = text.trim();
    let lead = &text[..text.len() - text.trim_start().len()];
    let trail = &text[text.trim_end().len()..];
    (lead, core, trail)
}

/// Text with what Markdown would take for its marks escaped.
fn escape(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        let word = |j: Option<usize>| {
            j.and_then(|j| chars.get(j))
                .is_some_and(|c| c.is_alphanumeric())
        };
        let escaped = match c {
            '\\' | '*' | '`' | '[' | ']' => true,
            // snake_case stays as it is.
            '_' => !(word(i.checked_sub(1)) && word(Some(i + 1))),
            '<' => chars
                .get(i + 1)
                .is_some_and(|n| n.is_ascii_alphabetic() || matches!(n, '/' | '!' | '?')),
            _ => false,
        };
        if escaped {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A paragraph's line, escaped where it would start a heading, a list, a quote or a rule.
fn escape_start(line: &str) -> String {
    let digits = line.len() - line.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let after = &line[digits..];
    let marker = match line.chars().next() {
        Some('#') => {
            let rest = line.trim_start_matches('#');
            rest.is_empty() || rest.starts_with(' ')
        }
        Some('-' | '+') => line[1..].starts_with(' ') || line.chars().all(|c| c == '-'),
        Some('=') => line.chars().all(|c| c == '='),
        Some('>') => true,
        _ => false,
    };
    if marker {
        format!("\\{line}")
    } else if digits > 0
        && (after.starts_with(". ") || after.starts_with(") ") || after == "." || after == ")")
    {
        format!("{}\\{after}", &line[..digits])
    } else {
        line.to_string()
    }
}

/// A link's target as Markdown takes it.
fn url(href: &str) -> String {
    href.replace(' ', "%20")
        .replace('(', "%28")
        .replace(')', "%29")
}

#[cfg(test)]
mod tests {
    use super::to_markdown;

    #[test]
    fn email() {
        let html = "<meta charset='utf-8'><p>Dzień dobry,</p>\n\
            <p style=\"background:#2a8\"><b>Co możesz zyskać w&nbsp;promocji</b></p>\n\
            <p>Nagrodę 120 zł otrzymasz:</p>\n<ul>\n  <li>zapewnisz min. <b>1500 zł wpływu </b>na konto,</li>\n\
            <li>zapłacisz łącznie <strong>5 razy</strong> (nie uwzględniamy\n   przelewów),</li>\n</ul>\n\
            <p>Pamiętaj o <a href=\"https://bank.example/r?a=1&amp;b=2\">regulaminie</a>.</p>";
        assert_eq!(
            to_markdown(html).unwrap(),
            "Dzień dobry,\n\n**Co możesz zyskać w promocji**\n\nNagrodę 120 zł otrzymasz:\n\n\
             - zapewnisz min. **1500 zł wpływu** na konto,\n\
             - zapłacisz łącznie **5 razy** (nie uwzględniamy przelewów),\n\n\
             Pamiętaj o [regulaminie](https://bank.example/r?a=1&b=2)."
        );
    }

    #[test]
    fn plain_text_is_left_to_the_plain_paste() {
        // As a code editor copies: lines of spans, no formatting.
        let html = "<div style=\"white-space: pre\"><div><span>fn main() {</span></div>\
            <div><span>    x</span></div></div>";
        assert_eq!(to_markdown(html), None);
        assert_eq!(to_markdown("<p>one</p><p>two</p>"), None);
        // A part of a heading is its text.
        assert_eq!(to_markdown("<h1>Notav</h1>"), None);
        assert_eq!(
            to_markdown("<h2>The <i>best</i></h2>").as_deref(),
            Some("The *best*")
        );
    }

    #[test]
    fn blocks() {
        let html = "<h1>Title</h1><blockquote><p>a</p><p>b</p></blockquote><hr>\
            <pre><code class=\"language-rust\">let a = 1;\n\nlet b = a;</code></pre>\
            <ol start=\"3\"><li><p>three</p><p>more</p></li><li>four<ul><li>sub</li></ul></li></ol>";
        assert_eq!(
            to_markdown(html).unwrap(),
            "# Title\n\n> a\n>\n> b\n\n---\n\n```rust\nlet a = 1;\n\nlet b = a;\n```\n\n\
             3. three\n\n   more\n\n4. four\n   - sub"
        );
    }

    #[test]
    fn nested_lists_and_unclosed_items() {
        assert_eq!(
            to_markdown("<ul><li>a<li>b<ul><li>c</ul></ul>").unwrap(),
            "- a\n- b\n  - c"
        );
        // Google Docs: a list right in a list, bold and italic in styles.
        let html = "<b style=\"font-weight:normal\" id=\"docs-internal-guid-1\"><ul>\
            <li><p><span style=\"font-weight:700\">bold</span> and \
            <span style=\"font-style:italic\">italic</span></p></li>\
            <ul><li><p>sub</p></li></ul></ul></b>";
        assert_eq!(
            to_markdown(html).unwrap(),
            "- **bold** and *italic*\n  - sub"
        );
    }

    #[test]
    fn tables() {
        let data = "<table><thead><tr><th>Name</th><th>Size</th></tr></thead>\
            <tbody><tr><td>a|b</td><td><b>1</b></td></tr><tr><td>c</td></tr></tbody></table>";
        assert_eq!(
            to_markdown(data).unwrap(),
            "| Name | Size  |\n| ---- | ----- |\n| a\\|b | **1** |\n| c    |       |"
        );
        // Layout, as in an email: its cells' text.
        let layout = "<table role=\"presentation\"><tr><td><p>Hi</p></td><td>you</td></tr>\
            <tr><td><table><tr><td><a href=\"https://x.org\">link</a></td></tr></table></td></tr></table>";
        assert_eq!(
            to_markdown(layout).unwrap(),
            "Hi\n\nyou\n\n[link](https://x.org)"
        );
    }

    #[test]
    fn inline() {
        let md = |html| to_markdown(html).unwrap();
        assert_eq!(md("<b>a</b><b> b</b>"), "**a** **b**");
        assert_eq!(md("x<b><strong>y</strong></b>z"), "x**y**z");
        assert_eq!(md("<i>2 * 3_ [a]</i>"), "*2 \\* 3\\_ \\[a\\]*");
        assert_eq!(
            md("<b>snake_case &lt;tag&gt;</b>"),
            "**snake_case \\<tag>**"
        );
        assert_eq!(md("<code>a `b`</code>"), "`` a `b` ``");
        assert_eq!(
            md("<s>old</s> <a href=\"https://x.org\">https://x.org</a>"),
            "~~old~~ <https://x.org>"
        );
        assert_eq!(md("<a href=\"mailto:a@b.c\">a@b.c</a>"), "<mailto:a@b.c>");
        assert_eq!(md("<a href=\"#top\">top</a> <b>x</b>"), "top **x**");
        assert_eq!(
            md(
                "<img src=\"https://x.org/a b.png\" alt=\"A\"><img src=\"data:image/png;base64,AA\">"
            ),
            "![A](https://x.org/a%20b.png)"
        );
        assert_eq!(md("<b>Jan</b><br>Kowalski <br/> 2"), "**Jan**\nKowalski\n2");
        assert_eq!(
            md("<p>1. not a list</p><p># nor a heading <b>x</b></p>"),
            "1\\. not a list\n\n\\# nor a heading **x**"
        );
        assert_eq!(
            md("<ul><li><input type=checkbox checked> done</li></ul>"),
            "- [x] done"
        );
        assert_eq!(
            md("<!-- c --><style>p{}</style><script>x</script><b>&#x41;&#66;&amp;c&foo;</b>"),
            "**AB&c&foo;**"
        );
    }
}
