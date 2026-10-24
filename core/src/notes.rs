//! Notes and folders in the local copy. Every change is local and marks the note dirty; `sync`
//! uploads it later.

use crate::store::{self, CLEAN, DELETED, MODIFIED, Row};
use crate::{Client, Error, Result, now};
use rusqlite::{Connection, params};

/// A note: one Markdown file in the notes folder.
#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub id: i64,
    /// Relative to the notes folder: "Folder/Title.md".
    pub path: String,
    /// The file name without its extension.
    pub title: String,
    /// The folder it is in, relative to the notes folder; "" at the top.
    pub folder: String,
    /// Unix time of the last change, here or on the server.
    pub modified: i64,
    /// Changed here and not on the server yet.
    pub pending: bool,
}

impl From<&Row> for Note {
    fn from(r: &Row) -> Note {
        let (folder, title) = split(&r.path);
        Note {
            id: r.id,
            path: r.path.clone(),
            title: stem(title).to_string(),
            folder: folder.to_string(),
            modified: r.modified,
            pending: r.dirty != CLEAN,
        }
    }
}

/// (folder, file name) of a relative path.
pub(crate) fn split(path: &str) -> (&str, &str) {
    path.rsplit_once('/').unwrap_or(("", path))
}

/// A file name without its extension.
fn stem(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(s, _)| s)
}

/// Files that are notes: Markdown, and plain text as Nextcloud Notes may write them.
pub(crate) fn is_note(name: &str) -> bool {
    !name.starts_with('.') && (name.ends_with(".md") || name.ends_with(".txt"))
}

/// A title as a file name: characters other systems refuse in names become "-".
fn file_title(title: &str) -> Result<String> {
    let t: String = title
        .trim()
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let t = t.trim_start_matches('.').trim();
    if t.is_empty() {
        return Err(Error::Invalid(title.into()));
    }
    Ok(t.to_string())
}

/// A folder path: "a/b", no empty or dot segments.
fn folder_path(folder: &str) -> Result<String> {
    let parts: Vec<&str> = folder
        .split('/')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if parts.iter().any(|p| p.starts_with('.')) {
        return Err(Error::Invalid(folder.into()));
    }
    Ok(parts.join("/"))
}

fn join(folder: &str, name: &str) -> String {
    if folder.is_empty() {
        name.to_string()
    } else {
        format!("{folder}/{name}")
    }
}

/// `folder/title.ext`, or with " 2", " 3"... added to the title until no note has that path.
pub(crate) fn free_path(db: &Connection, folder: &str, title: &str, ext: &str) -> Result<String> {
    for n in 1.. {
        let t = if n == 1 {
            title.to_string()
        } else {
            format!("{title} {n}")
        };
        let path = join(folder, &format!("{t}.{ext}"));
        if store::by_path(db, &path)?.is_none() {
            return Ok(path);
        }
    }
    unreachable!()
}

impl Client {
    /// Every note, by path.
    pub fn notes(&self) -> Vec<Note> {
        let db = self.db.lock().unwrap();
        let rows = store::rows(&db, "dirty != ?", [DELETED]).unwrap_or_default();
        rows.iter().map(Note::from).collect()
    }

    pub fn note(&self, id: i64) -> Option<Note> {
        let db = self.db.lock().unwrap();
        let r = store::by_id(&db, id).ok()??;
        (r.dirty != DELETED).then(|| Note::from(&r))
    }

    pub fn note_text(&self, id: i64) -> Option<String> {
        let db = self.db.lock().unwrap();
        let r = store::by_id(&db, id).ok()??;
        (r.dirty != DELETED).then_some(r.text)
    }

    /// Notes whose title, folder or text has every word of `query` (as a word or its start),
    /// case and diacritics ignored; by path.
    pub fn search(&self, query: String) -> Vec<Note> {
        // Each word quoted (FTS5 syntax stays literal) and matched as a prefix.
        let fts: Vec<String> = query
            .split_whitespace()
            .map(|w| w.replace('ł', "l").replace('Ł', "L").replace('"', "\"\""))
            .map(|w| format!("\"{w}\"*"))
            .collect();
        if fts.is_empty() {
            return self.notes();
        }
        let db = self.db.lock().unwrap();
        let filter = "dirty != ? AND id IN (SELECT rowid FROM notes_fts WHERE notes_fts MATCH ?)";
        let rows = store::rows(&db, filter, params![DELETED, fts.join(" ")]).unwrap_or_default();
        rows.iter().map(Note::from).collect()
    }

    /// Folders, relative to the notes folder, by path; empty ones too.
    pub fn folders(&self) -> Vec<String> {
        let db = self.db.lock().unwrap();
        let mut out: Vec<String> = db
            .prepare("SELECT path FROM folders WHERE path != ''")
            .and_then(|mut st| st.query_map([], |r| r.get(0))?.collect())
            .unwrap_or_default();
        // And the folders of notes made here, in folders not pulled yet.
        for n in store::rows(&db, "dirty != ?", [DELETED]).unwrap_or_default() {
            let mut f = split(&n.path).0;
            while !f.is_empty() {
                out.push(f.to_string());
                f = split(f).0;
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// A new note in `folder` ("" at the top); a number is added to the title if one exists.
    pub fn add_note(&self, folder: String, title: String, text: String) -> Result<Note> {
        let folder = folder_path(&folder)?;
        let title = file_title(&title)?;
        let row = {
            let db = self.db.lock().unwrap();
            let path = free_path(&db, &folder, &title, "md")?;
            db.execute(
                "INSERT INTO notes (path, text, modified, dirty) VALUES (?, ?, ?, ?)",
                params![path, text, now(), MODIFIED],
            )?;
            store::by_id(&db, db.last_insert_rowid())?.unwrap()
        };
        self.emit(vec![row.path.clone()]);
        Ok(Note::from(&row))
    }

    /// Replace a note's text.
    pub fn set_text(&self, id: i64, text: String) -> Result<()> {
        let path = {
            let db = self.db.lock().unwrap();
            let r = store::by_id(&db, id)?
                .filter(|r| r.dirty != DELETED)
                .ok_or_else(|| Error::NotFound(id.to_string()))?;
            if r.text == text {
                return Ok(());
            }
            db.execute(
                "UPDATE notes SET text = ?, modified = ?, dirty = ? WHERE id = ?",
                params![text, now(), MODIFIED, id],
            )?;
            r.path
        };
        self.emit(vec![path]);
        Ok(())
    }

    /// Rename a note and/or move it to another folder. `Error::Exists` when a note has that
    /// name there already.
    pub fn move_note(&self, id: i64, folder: String, title: String) -> Result<Note> {
        let folder = folder_path(&folder)?;
        let title = file_title(&title)?;
        let (old, row) = {
            let db = self.db.lock().unwrap();
            let r = store::by_id(&db, id)?
                .filter(|r| r.dirty != DELETED)
                .ok_or_else(|| Error::NotFound(id.to_string()))?;
            let ext = split(&r.path).1.rsplit_once('.').map_or("md", |(_, e)| e);
            let path = join(&folder, &format!("{title}.{ext}"));
            if path == r.path {
                return Ok(Note::from(&r));
            }
            if store::by_path(&db, &path)?.is_some() {
                return Err(Error::Exists(path));
            }
            // The server copy stays where it is until the push moves it.
            let server_path = r
                .server_path
                .clone()
                .or_else(|| r.fileid.as_ref().map(|_| r.path.clone()));
            db.execute(
                "UPDATE notes SET path = ?, server_path = ?, dirty = ? WHERE id = ?",
                params![path, server_path, MODIFIED, id],
            )?;
            (r.path, store::by_id(&db, id)?.unwrap())
        };
        self.emit(vec![old, row.path.clone()]);
        Ok(Note::from(&row))
    }

    pub fn delete_note(&self, id: i64) -> Result<()> {
        let path = {
            let db = self.db.lock().unwrap();
            let r = store::by_id(&db, id)?.ok_or_else(|| Error::NotFound(id.to_string()))?;
            if r.fileid.is_some() {
                db.execute(
                    "UPDATE notes SET dirty = ? WHERE id = ?",
                    params![DELETED, id],
                )?;
            } else {
                db.execute("DELETE FROM notes WHERE id = ?", [id])?;
            }
            r.path
        };
        self.emit(vec![path]);
        Ok(())
    }

    /// A new, empty folder ("a/b"), created on the server with the next push.
    pub fn add_folder(&self, folder: String) -> Result<()> {
        let folder = folder_path(&folder)?;
        if folder.is_empty() {
            return Err(Error::Invalid(folder));
        }
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT OR IGNORE INTO folders (path, dirty) VALUES (?, 1)",
            [&folder],
        )?;
        Ok(())
    }

    /// Something waits for the server.
    pub fn pending(&self) -> bool {
        let db = self.db.lock().unwrap();
        db.query_row(
            "SELECT EXISTS (SELECT 1 FROM notes WHERE dirty != 0)
               OR EXISTS (SELECT 1 FROM folders WHERE dirty != 0)",
            [],
            |r| r.get(0),
        )
        .unwrap_or(false)
    }

    pub(crate) fn emit(&self, paths: Vec<String>) {
        if paths.is_empty() {
            return;
        }
        for l in self.listeners.lock().unwrap().iter() {
            l.changed(paths.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(name: &str) -> Client {
        let dir = std::env::temp_dir().join(format!("notav-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Client::open(dir.to_string_lossy().into()).unwrap()
    }

    #[test]
    fn names_and_paths() {
        assert_eq!(file_title("  a/b: c?  ").unwrap(), "a-b- c-");
        assert!(file_title(" .. ").is_err());
        assert_eq!(folder_path("/Work//Ideas/").unwrap(), "Work/Ideas");
        assert!(folder_path("a/.obsidian").is_err());
        assert!(is_note("Ideas.md") && is_note("Old.txt"));
        assert!(!is_note(".hidden.md") && !is_note("photo.png"));
    }

    #[test]
    fn add_rename_delete() {
        let c = client("notes");
        let a = c
            .add_note("Work".into(), "Plan".into(), "# Plan".into())
            .unwrap();
        let b = c.add_note("Work".into(), "Plan".into(), "".into()).unwrap();
        assert_eq!(
            (a.path.as_str(), b.path.as_str()),
            ("Work/Plan.md", "Work/Plan 2.md")
        );
        assert_eq!((b.title.as_str(), b.folder.as_str()), ("Plan 2", "Work"));
        assert!(matches!(
            c.move_note(b.id, "Work".into(), "Plan".into()),
            Err(Error::Exists(_))
        ));
        let b = c.move_note(b.id, "".into(), "Later".into()).unwrap();
        assert_eq!(b.path, "Later.md");
        c.set_text(b.id, "text".into()).unwrap();
        assert_eq!(c.note_text(b.id).as_deref(), Some("text"));
        assert_eq!(c.folders(), ["Work"]);
        let found =
            |q: &str| -> Vec<String> { c.search(q.into()).into_iter().map(|n| n.path).collect() };
        c.set_text(a.id, "Zażółć gęślą jaźń, \"quoted\" * NEAR".into())
            .unwrap();
        assert_eq!(found("zazolc GESL"), ["Work/Plan.md"]);
        assert_eq!(found("work later"), Vec::<String>::new());
        assert_eq!(found("\"quoted\" * NEAR("), ["Work/Plan.md"]);
        assert_eq!(found("later text"), ["Later.md"]);
        assert_eq!(found(" ").len(), 2);
        // Never uploaded: gone at once.
        c.delete_note(a.id).unwrap();
        assert_eq!(c.notes().len(), 1);
        assert!(c.pending());
    }
}
