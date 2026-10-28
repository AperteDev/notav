//! The journal: one daily note, in the folder and under the name the synced settings say
//! (`journal_folder`, `journal_format`), made from a template note (`journal_template`).

use crate::notes::{file_title, folder_path, join, split};
use crate::{Client, Note, Result};

pub const DEFAULT_FOLDER: &str = "Journal";
pub const DEFAULT_FORMAT: &str = "YYYY-MM-DD";

/// A daily note's name for a day, from `format`: `YYYY` the year, `YY` its last two digits,
/// `MM` and `DD` the month and day (`M` and `D` without a leading zero); `/` starts a folder;
/// text in `[brackets]` is kept as written, the rest too.
// ponytail: numbers only; weekday and month names need the locale's.
pub fn name(format: &str, year: i32, month: u32, day: u32) -> String {
    let mut out = String::new();
    let mut rest = format;
    while let Some(c) = rest.chars().next() {
        if c == '['
            && let Some(end) = rest.find(']')
        {
            out.push_str(&rest[1..end]);
            rest = &rest[end + 1..];
            continue;
        }
        let (text, len) = match rest {
            r if r.starts_with("YYYY") => (format!("{year:04}"), 4),
            r if r.starts_with("YY") => (format!("{:02}", year.rem_euclid(100)), 2),
            r if r.starts_with("MM") => (format!("{month:02}"), 2),
            r if r.starts_with('M') => (month.to_string(), 1),
            r if r.starts_with("DD") => (format!("{day:02}"), 2),
            r if r.starts_with('D') => (day.to_string(), 1),
            _ => (c.to_string(), c.len_utf8()),
        };
        out.push_str(&text);
        rest = &rest[len..];
    }
    out
}

impl Client {
    /// The journal's folder (the setting, or `DEFAULT_FOLDER`).
    pub fn journal_folder(&self) -> String {
        self.setting("journal_folder".into())
            .map(|f| f.trim().trim_matches('/').to_string())
            .filter(|f| !f.is_empty())
            .unwrap_or_else(|| DEFAULT_FOLDER.into())
    }

    /// The journal's name format (the setting, or `DEFAULT_FORMAT`).
    pub fn journal_format(&self) -> String {
        self.setting("journal_format".into())
            .filter(|f| !f.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_FORMAT.into())
    }

    /// The daily note's path for a day, from the notes folder, without `.md`.
    pub fn daily_path(&self, year: i32, month: u32, day: u32) -> String {
        join(
            &self.journal_folder(),
            &name(&self.journal_format(), year, month, day),
        )
    }

    /// The daily note of a day: the one there is, or a new one from the template note (its
    /// `{{date}}` and `{{title}}` filled in), its folders made if need be.
    pub fn daily_note(&self, year: i32, month: u32, day: u32) -> Result<Note> {
        let path = self.daily_path(year, month, day);
        let (folder, title) = split(&path);
        let (folder, title) = (folder_path(folder)?, file_title(title)?);
        let file = format!("{}.md", join(&folder, &title));
        if let Some(n) = self.notes().into_iter().find(|n| n.path == file) {
            return Ok(n);
        }
        let template = self
            .setting("journal_template".into())
            .map(|t| t.trim().trim_matches('/').to_string())
            .filter(|t| !t.is_empty())
            .and_then(|t| {
                let with_md = if t.ends_with(".md") {
                    t
                } else {
                    format!("{t}.md")
                };
                self.notes().into_iter().find(|n| n.path == with_md)
            })
            .and_then(|n| self.note_text(n.id))
            .unwrap_or_default();
        let text = template
            .replace("{{date}}", &format!("{year:04}-{month:02}-{day:02}"))
            .replace("{{title}}", &title);
        self.add_note(folder, title, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_from_the_format() {
        assert_eq!(name("YYYY-MM-DD", 2026, 3, 7), "2026-03-07");
        assert_eq!(name("D.M.YY", 2026, 3, 7), "7.3.26");
        assert_eq!(name("YYYY/MM/[Day] DD", 2026, 11, 28), "2026/11/Day 28");
        assert_eq!(name("[YYYY] Zażółć", 2026, 1, 1), "YYYY Zażółć");
        assert_eq!(name("[unclosed", 2026, 1, 1), "[unclosed");
    }

    #[test]
    fn the_daily_note_is_made_once_from_the_template() {
        let dir = std::env::temp_dir().join(format!("notav-test-journal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let c = Client::open(dir.to_string_lossy().into()).unwrap();
        c.add_note(
            "Templates".into(),
            "Day".into(),
            "# {{title}}\n\nOn {{date}}.".into(),
        )
        .unwrap();
        c.set_setting("journal_template".into(), "Templates/Day".into())
            .unwrap();
        c.set_setting("journal_format".into(), "YYYY/MM-DD".into())
            .unwrap();
        let n = c.daily_note(2026, 3, 7).unwrap();
        assert_eq!(n.path, "Journal/2026/03-07.md");
        assert_eq!(c.note_text(n.id).unwrap(), "# 03-07\n\nOn 2026-03-07.");
        assert_eq!(c.daily_note(2026, 3, 7).unwrap().id, n.id);
        c.set_setting("journal_folder".into(), "/Days/".into())
            .unwrap();
        assert_eq!(c.daily_path(2026, 3, 8), "Days/2026/03-08");
        // A folder made deep down shows with the folders above it.
        c.add_folder("New/Deep".into()).unwrap();
        assert!(
            c.folders()
                .starts_with(&["Journal".into(), "Journal/2026".into(), "New".into()])
        );
    }
}
