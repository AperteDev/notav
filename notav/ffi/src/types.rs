//! The records and enums the bindings carry. Core types are mirrored with `uniffi::remote` (the
//! fields must be theirs exactly; the compiler checks); the rest are this crate's own, where
//! the original does not cross the boundary: enums with unnamed fields, and text positions,
//! which the core counts in characters and Kotlin in UTF-16 code units.

use notav_core::dav::ServerInfo;
use notav_core::markdown::{self, Alert, Style};
use notav_core::{
    Account, AccountKind, Credentials, Deleted, Features, Locked, LoginFlow, Note, SyncReport,
    Version,
};

#[uniffi::remote(Enum)]
pub enum AccountKind {
    Nextcloud,
    Local,
}

#[uniffi::remote(Record)]
pub struct Account {
    pub id: i64,
    pub kind: AccountKind,
    pub name: String,
    pub url: String,
    pub user: String,
    pub source: String,
    pub enabled: bool,
}

#[uniffi::remote(Record)]
pub struct Note {
    pub id: i64,
    pub account: i64,
    pub path: String,
    pub title: String,
    pub folder: String,
    pub modified: i64,
    pub pending: bool,
    pub favorite: bool,
}

#[uniffi::remote(Record)]
pub struct LoginFlow {
    pub login_url: String,
    pub endpoint: String,
    pub token: String,
}

#[uniffi::remote(Record)]
pub struct Credentials {
    pub server: String,
    pub login_name: String,
    pub app_password: String,
}

#[uniffi::remote(Record)]
pub struct ServerInfo {
    pub product: String,
    pub version: String,
    pub maintenance: bool,
}

#[uniffi::remote(Record)]
pub struct Locked {
    pub account: i64,
    pub path: String,
    pub file: Option<String>,
    pub owner: Option<String>,
    pub since: Option<i64>,
}

#[uniffi::remote(Record)]
pub struct SyncReport {
    pub uploaded: u32,
    pub downloaded: u32,
    pub removed: u32,
    pub conflicts: Vec<String>,
    pub locked: Vec<Locked>,
}

#[uniffi::remote(Record)]
pub struct Features {
    pub trash: bool,
    pub purge: bool,
    pub versions: bool,
}

#[uniffi::remote(Record)]
pub struct Deleted {
    pub id: String,
    pub path: String,
    pub folder: bool,
    pub deleted: i64,
}

#[uniffi::remote(Record)]
pub struct Version {
    pub id: String,
    pub modified: i64,
    pub size: Option<u64>,
    pub author: Option<String>,
    pub label: Option<String>,
    pub current: bool,
}

#[uniffi::remote(Enum)]
pub enum Alert {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
}

/// What a range of a note is (`markdown::Style`).
#[derive(uniffi::Enum)]
pub enum NoteStyle {
    Heading { level: u8 },
    Strong,
    Emphasis,
    Strikethrough,
    Code,
    CodeBlock,
    Quote,
    Alert { kind: Alert },
    AlertMark { kind: Alert },
    Link,
    ListMark,
    Task { done: bool },
    Table,
    TableHeader,
    Mark,
}

impl From<Style> for NoteStyle {
    fn from(s: Style) -> Self {
        match s {
            Style::Heading(level) => NoteStyle::Heading { level },
            Style::Strong => NoteStyle::Strong,
            Style::Emphasis => NoteStyle::Emphasis,
            Style::Strikethrough => NoteStyle::Strikethrough,
            Style::Code => NoteStyle::Code,
            Style::CodeBlock => NoteStyle::CodeBlock,
            Style::Quote => NoteStyle::Quote,
            Style::Alert(kind) => NoteStyle::Alert { kind },
            Style::AlertMark(kind) => NoteStyle::AlertMark { kind },
            Style::Link => NoteStyle::Link,
            Style::ListMark => NoteStyle::ListMark,
            Style::Task(done) => NoteStyle::Task { done },
            Style::Table => NoteStyle::Table,
            Style::TableHeader => NoteStyle::TableHeader,
            Style::Mark => NoteStyle::Mark,
        }
    }
}

/// `style` over UTF-16 code units `start` to `end` of a note's text.
#[derive(uniffi::Record)]
pub struct NoteSpan {
    pub start: u32,
    pub end: u32,
    pub style: NoteStyle,
}

/// A change to a note's text: UTF-16 code units `start` to `end` replaced by `text`.
#[derive(uniffi::Record)]
pub struct NoteEdit {
    pub start: u32,
    pub end: u32,
    pub text: String,
}

/// What a link points to (`markdown::Link`).
#[derive(uniffi::Enum)]
pub enum NoteLink {
    /// An address another app opens: a web page, `mailto:`.
    Address { url: String },
    /// A note, for `Notav::follow_link`; with the heading after a `#`.
    Note {
        target: String,
        heading: Option<String>,
        wiki: bool,
    },
}

impl From<markdown::Link> for NoteLink {
    fn from(l: markdown::Link) -> Self {
        match l {
            markdown::Link::Address(url) => NoteLink::Address { url },
            markdown::Link::Note {
                target,
                heading,
                wiki,
            } => NoteLink::Note {
                target,
                heading,
                wiki,
            },
        }
    }
}

/// A heading of a note: its level (1 to 6), its title and where it starts (UTF-16 code units).
#[derive(uniffi::Record)]
pub struct NoteHeading {
    pub level: u8,
    pub title: String,
    pub at: u32,
}

/// Positions in a text both ways: characters, as the core counts, and UTF-16 code units, as
/// Kotlin does.
pub(crate) struct Utf16(Vec<u32>);

impl Utf16 {
    pub(crate) fn of(text: &str) -> Utf16 {
        let mut at = Vec::with_capacity(text.len() + 1);
        let mut n = 0u32;
        for c in text.chars() {
            at.push(n);
            n += c.len_utf16() as u32;
        }
        at.push(n);
        Utf16(at)
    }

    /// Character `c` in code units (the end past the last).
    pub(crate) fn unit(&self, c: usize) -> u32 {
        self.0[c.min(self.0.len() - 1)]
    }

    /// Code unit `u` as a character: the one it starts, or the one it is inside (the second half
    /// of a surrogate pair).
    pub(crate) fn char(&self, u: u32) -> usize {
        self.0.partition_point(|&x| x <= u).saturating_sub(1)
    }

    pub(crate) fn edits(&self, edits: Vec<markdown::Edit>) -> Vec<NoteEdit> {
        edits
            .into_iter()
            .map(|e| NoteEdit {
                start: self.unit(e.start),
                end: self.unit(e.end),
                text: e.text,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::Utf16;

    #[test]
    fn positions_after_an_emoji() {
        // "a😀b": a at 0, the emoji at 1 (two code units), b at 3; four in all.
        let p = Utf16::of("a😀b");
        assert_eq!([p.unit(0), p.unit(1), p.unit(2), p.unit(3)], [0, 1, 3, 4]);
        assert_eq!(
            [p.char(0), p.char(1), p.char(2), p.char(3), p.char(4)],
            [0, 1, 1, 2, 3]
        );
        assert_eq!(p.unit(99), 4, "past the end: the end");
    }
}
