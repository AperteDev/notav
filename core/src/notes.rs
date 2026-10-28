//! Notes and folders in the local copy. Every change is local and marks the note dirty; `sync`
//! uploads it later.

use crate::markdown::{self, Link};
use crate::store::{self, CLEAN, DELETED, FOLDER_DELETED, MODIFIED, Row};
use crate::undo::Before;
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

/// The note among `notes` a link from a note in folder `from` names, or the folder and title of
/// the note it would be. A Markdown link's target is a path from `from` (`/` first: from the
/// notes folder), `.md` optional. A wiki link's is a path from the notes folder (a note whose
/// path ends so, the shortest), or a title, case aside: in `from` first, then the shortest path.
fn resolve<'a>(
    notes: &'a [Note],
    from: &str,
    target: &str,
    wiki: bool,
) -> std::result::Result<&'a Note, (String, String)> {
    let with_md = |p: &str| {
        if p.ends_with(".md") || p.ends_with(".txt") {
            p.to_string()
        } else {
            format!("{p}.md")
        }
    };
    let missing = |path: &str| {
        let (folder, name) = split(path);
        (folder.to_string(), stem(name).to_string())
    };
    if !wiki {
        let mut parts: Vec<&str> = if target.starts_with('/') {
            Vec::new()
        } else {
            from.split('/').filter(|p| !p.is_empty()).collect()
        };
        for p in target.split('/') {
            match p {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                p => parts.push(p),
            }
        }
        let path = with_md(&parts.join("/"));
        return notes
            .iter()
            .find(|n| n.path == path)
            .ok_or_else(|| missing(&path));
    }
    let name = target.trim_start_matches('/');
    if name.contains('/') {
        let path = with_md(name);
        let tail = format!("/{path}");
        return notes
            .iter()
            .filter(|n| n.path == path || n.path.ends_with(&tail))
            .min_by_key(|n| (n.path.len(), n.path.clone()))
            .ok_or_else(|| missing(&path));
    }
    let title = name.strip_suffix(".md").unwrap_or(name).to_lowercase();
    notes
        .iter()
        .filter(|n| n.title.to_lowercase() == title)
        .min_by_key(|n| (n.folder != from, n.path.len(), n.path.clone()))
        .ok_or_else(|| {
            (
                from.to_string(),
                name.strip_suffix(".md").unwrap_or(name).to_string(),
            )
        })
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
pub(crate) fn file_title(title: &str) -> Result<String> {
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
pub(crate) fn folder_path(folder: &str) -> Result<String> {
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

pub(crate) fn join(folder: &str, name: &str) -> String {
    if folder.is_empty() {
        name.to_string()
    } else {
        format!("{folder}/{name}")
    }
}

/// Give note `r` path `path`. Its server copy stays where it is until the push moves it, unless
/// it is back where the server has it.
fn place(db: &Connection, r: &Row, path: &str) -> Result<()> {
    let server_path = r
        .server_path
        .clone()
        .or_else(|| r.fileid.as_ref().map(|_| r.path.clone()))
        .filter(|s| s != path);
    db.execute(
        "UPDATE notes SET path = ?, server_path = ?, dirty = ? WHERE id = ?",
        params![path, server_path, MODIFIED, r.id],
    )?;
    Ok(())
}

/// SQL: column `col` is a path below the folder in `?1`.
pub(crate) fn under(col: &str) -> String {
    format!("(substr({col}, 1, length(?1) + 1) = ?1 || '/')")
}

/// Folder `folder` is on the server: pulled from there (not made here), or a note in it is.
fn on_server(db: &Connection, folder: &str) -> Result<bool> {
    Ok(db.query_row(
        &format!(
            "SELECT (EXISTS (SELECT 1 FROM folders WHERE path = ?1 AND dirty = 0)
               OR EXISTS (SELECT 1 FROM notes WHERE fileid IS NOT NULL
                 AND {} AND server_path IS NULL))
               AND NOT EXISTS (SELECT 1 FROM folders WHERE path = ?1 AND dirty = 1)",
            under("path")
        ),
        [folder],
        |r| r.get(0),
    )?)
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
            .prepare("SELECT path FROM folders WHERE path != '' AND dirty != 2")
            .and_then(|mut st| st.query_map([], |r| r.get(0))?.collect())
            .unwrap_or_default();
        // And the folders of notes made here, and above folders made here, not pulled yet.
        let notes = store::rows(&db, "dirty != ?", [DELETED]).unwrap_or_default();
        let mut paths: Vec<String> = notes.iter().map(|n| split(&n.path).0.to_string()).collect();
        paths.extend(out.iter().map(|f| split(f).0.to_string()));
        for p in paths {
            let mut f = p.as_str();
            while !f.is_empty() {
                out.push(f.to_string());
                f = split(f).0;
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// The note a link in note `from` names (`markdown::Link::Note`'s target and kind), made
    /// where the link puts it when there is none, as Obsidian does.
    pub fn follow_link(&self, from: i64, target: String, wiki: bool) -> Result<Note> {
        let from = self
            .note(from)
            .ok_or_else(|| Error::NotFound(from.to_string()))?;
        if target.is_empty() && !wiki {
            return Ok(from);
        }
        match resolve(&self.notes(), &from.folder, &target, wiki) {
            Ok(n) => Ok(n.clone()),
            Err((folder, title)) => self.add_note(folder, title, String::new()),
        }
    }

    /// The notes with a link to note `id`, by path.
    // ponytail: parses every note each time; an index of links when vaults get large.
    pub fn backlinks(&self, id: i64) -> Vec<Note> {
        let rows = {
            let db = self.db.lock().unwrap();
            store::rows(&db, "dirty != ?", [DELETED]).unwrap_or_default()
        };
        let notes: Vec<Note> = rows.iter().map(Note::from).collect();
        let links_here = |n: &Note, text: &str| {
            markdown::links(text).into_iter().any(|l| match l {
                Link::Note { target, wiki, .. } if wiki || !target.is_empty() => {
                    resolve(&notes, &n.folder, &target, wiki).is_ok_and(|t| t.id == id)
                }
                _ => false,
            })
        };
        rows.iter()
            .zip(&notes)
            .filter(|(r, n)| n.id != id && links_here(n, &r.text))
            .map(|(_, n)| n.clone())
            .collect()
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
            place(&db, &r, &path)?;
            (r.path, store::by_id(&db, id)?.unwrap())
        };
        self.remember(vec![Before::Note {
            id,
            path: old.clone(),
            text: None,
        }]);
        self.emit(vec![old, row.path.clone()]);
        Ok(Note::from(&row))
    }

    pub fn delete_note(&self, id: i64) -> Result<()> {
        let (path, text) = {
            let db = self.db.lock().unwrap();
            let r = store::by_id(&db, id)?
                .filter(|r| r.dirty != DELETED)
                .ok_or_else(|| Error::NotFound(id.to_string()))?;
            if r.fileid.is_some() {
                db.execute(
                    "UPDATE notes SET dirty = ? WHERE id = ?",
                    params![DELETED, id],
                )?;
            } else {
                db.execute("DELETE FROM notes WHERE id = ?", [id])?;
            }
            (r.path, r.text)
        };
        self.remember(vec![Before::Note {
            id,
            path: path.clone(),
            text: Some(text),
        }]);
        self.emit(vec![path]);
        Ok(())
    }

    /// Move folder `folder` ("a/b"), with everything in it, into `parent` ("" for the top level):
    /// its new path. `Error::Exists` when a folder has that name there, `Invalid` for a move
    /// into itself. The server's copy moves with the next push, in one `MOVE`.
    pub fn move_folder(&self, folder: String, parent: String) -> Result<String> {
        let folder = folder_path(&folder)?;
        let dst = join(&folder_path(&parent)?, split(&folder).1);
        let to = self.relocate_folder(folder.clone(), dst)?;
        self.remember(vec![Before::FolderMoved {
            from: folder,
            to: to.clone(),
        }]);
        Ok(to)
    }

    /// Rename folder `folder` ("a/b") to `name`, as `move_folder` moves it: its new path.
    pub fn rename_folder(&self, folder: String, name: String) -> Result<String> {
        let folder = folder_path(&folder)?;
        let dst = join(split(&folder).0, &file_title(&name)?);
        let to = self.relocate_folder(folder.clone(), dst)?;
        self.remember(vec![Before::FolderMoved {
            from: folder,
            to: to.clone(),
        }]);
        Ok(to)
    }

    /// Delete folder `folder` ("a/b") with everything in it. Its notes go as `delete_note`'s do
    /// (one changed on the server since stays, and its folder with it); the folder goes with the
    /// next push, in one `DELETE`.
    pub fn delete_folder(&self, folder: String) -> Result<()> {
        let folder = folder_path(&folder)?;
        if folder.is_empty() {
            return Err(Error::Invalid(folder));
        }
        let inside = |p: &str| p == folder || p.starts_with(&format!("{folder}/"));
        let mut before: Vec<Before> = self
            .folders()
            .into_iter()
            .filter(|f| inside(f))
            .map(Before::Folder)
            .collect();
        let changed = {
            let db = self.db.lock().unwrap();
            let tx = db.unchecked_transaction()?;
            let on_server = on_server(&tx, &folder)?;
            let in_it = under("path");
            let notes = store::rows(
                &tx,
                &format!("dirty != ?2 AND {in_it}"),
                params![folder, DELETED],
            )?;
            before.extend(notes.iter().map(|r| Before::Note {
                id: r.id,
                path: r.path.clone(),
                text: Some(r.text.clone()),
            }));
            let changed: Vec<String> = notes.into_iter().map(|r| r.path).collect();
            tx.execute(
                &format!("DELETE FROM notes WHERE fileid IS NULL AND {in_it}"),
                [&folder],
            )?;
            tx.execute(
                &format!("UPDATE notes SET dirty = ?2 WHERE {in_it}"),
                params![folder, DELETED],
            )?;
            tx.execute(
                &format!("DELETE FROM folders WHERE path = ?1 OR {in_it}"),
                [&folder],
            )?;
            if on_server {
                tx.execute(
                    "INSERT INTO folders (path, dirty) VALUES (?, ?)",
                    params![folder, FOLDER_DELETED],
                )?;
            }
            tx.commit()?;
            changed
        };
        self.remember(before);
        self.emit(changed);
        Ok(())
    }

    /// The latest change, to undo just that one later (a toast's Undo button).
    pub fn last_change(&self) -> Option<u64> {
        self.journal.lock().unwrap().last()
    }

    /// Put back what change `id` (else the latest) replaced: deleted notes and folders, moved
    /// ones where they were (a name taken since gets a number); false when there is none. A note
    /// deleted on the server already comes back as a new one.
    pub fn undo(&self, id: Option<u64>) -> Result<bool> {
        let Some(before) = self.journal.lock().unwrap().take(id) else {
            return Ok(false);
        };
        for b in before.into_iter().rev() {
            match b {
                Before::Note { id, path, text } => self.restore(id, &path, text)?,
                Before::Folder(path) => self.add_folder(path)?,
                Before::FolderMoved { from, to } => {
                    self.relocate_folder(to, from)?;
                }
            }
        }
        Ok(true)
    }

    /// Note `id` back at `path`; `text`: deleted, back with that text. A note deleted since
    /// stays deleted when only its move is undone.
    fn restore(&self, id: i64, path: &str, text: Option<String>) -> Result<()> {
        let changed = {
            let db = self.db.lock().unwrap();
            let (folder, name) = split(path);
            let (stem, ext) = name.rsplit_once('.').unwrap_or((name, "md"));
            let Some(r) = store::by_id(&db, id)? else {
                // Gone from here too: a new note, with its old id.
                let Some(text) = text else { return Ok(()) };
                let to = free_path(&db, folder, stem, ext)?;
                db.execute(
                    "INSERT INTO notes (id, path, text, modified, dirty) VALUES (?, ?, ?, ?, ?)",
                    params![id, to, text, now(), MODIFIED],
                )?;
                drop(db);
                self.emit(vec![to]);
                return Ok(());
            };
            if r.dirty == DELETED {
                if text.is_none() {
                    return Ok(());
                }
                db.execute(
                    "UPDATE notes SET dirty = ? WHERE id = ?",
                    params![MODIFIED, id],
                )?;
            }
            if r.path == path {
                vec![r.path]
            } else {
                let to = free_path(&db, folder, stem, ext)?;
                place(&db, &r, &to)?;
                vec![r.path, to]
            }
        };
        self.emit(changed);
        Ok(())
    }

    fn remember(&self, before: Vec<Before>) {
        self.journal.lock().unwrap().record(before);
    }

    /// Move folder `folder` to path `dst`, see `move_folder`.
    fn relocate_folder(&self, folder: String, dst: String) -> Result<String> {
        let inside = |p: &str, f: &str| p == f || p.starts_with(&format!("{f}/"));
        if folder.is_empty() || dst != folder && inside(&dst, &folder) {
            return Err(Error::Invalid(folder));
        }
        if dst == folder {
            return Ok(dst);
        }
        if self.folders().contains(&dst) {
            return Err(Error::Exists(dst));
        }
        let changed = {
            let db = self.db.lock().unwrap();
            let tx = db.unchecked_transaction()?;
            let on_server = on_server(&tx, &folder)?;
            let changed: Vec<String> = {
                let mut st =
                    tx.prepare(&format!("SELECT path FROM notes WHERE {}", under("path")))?;
                st.query_map([&folder], |r| r.get(0))?
                    .collect::<rusqlite::Result<_>>()?
            };
            let to = |c: &str| format!("{c} = ?2 || substr({c}, length(?1) + 1)");
            tx.execute(
                &format!("UPDATE notes SET {} WHERE {}", to("path"), under("path")),
                params![folder, dst],
            )?;
            // One deleted there and not yet on the server: the push finds the name taken and
            // moves the notes one by one.
            tx.execute(
                "DELETE FROM folders WHERE path = ? AND dirty = ?",
                params![dst, FOLDER_DELETED],
            )?;
            if on_server {
                // The server's copies of what is in it move with it.
                tx.execute(
                    &format!(
                        "UPDATE notes SET {} WHERE {}",
                        to("server_path"),
                        under("server_path")
                    ),
                    params![folder, dst],
                )?;
                tx.execute(
                    "INSERT INTO moves (src, dst) VALUES (?, ?)",
                    params![folder, dst],
                )?;
            }
            tx.execute(
                &format!(
                    "UPDATE folders SET {} WHERE path = ?1 OR {}",
                    to("path"),
                    under("path")
                ),
                params![folder, dst],
            )?;
            tx.commit()?;
            changed
        };
        let moved = changed
            .iter()
            .map(|p| format!("{dst}{}", &p[folder.len()..]))
            .collect::<Vec<_>>();
        self.emit(changed.into_iter().chain(moved).collect());
        Ok(dst)
    }

    /// A new, empty folder ("a/b"), created on the server with the next push.
    pub fn add_folder(&self, folder: String) -> Result<()> {
        let folder = folder_path(&folder)?;
        if folder.is_empty() {
            return Err(Error::Invalid(folder));
        }
        let db = self.db.lock().unwrap();
        // One deleted here and not yet on the server stays there.
        db.execute(
            "INSERT INTO folders (path, dirty) VALUES (?1, 1)
             ON CONFLICT (path) DO UPDATE SET dirty = 1 WHERE dirty = ?2",
            params![folder, FOLDER_DELETED],
        )?;
        Ok(())
    }

    /// Something waits for the server.
    pub fn pending(&self) -> bool {
        let db = self.db.lock().unwrap();
        db.query_row(
            "SELECT EXISTS (SELECT 1 FROM notes WHERE dirty != 0)
               OR EXISTS (SELECT 1 FROM folders WHERE dirty != 0)
               OR EXISTS (SELECT 1 FROM moves)",
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
    fn links_find_their_notes() {
        let notes: Vec<Note> = [
            "Plan.md",
            "Work/Plan.md",
            "Work/Ideas/Big one.md",
            "Old.txt",
        ]
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let (folder, name) = split(p);
            Note {
                id: i as i64,
                path: p.to_string(),
                title: stem(name).to_string(),
                folder: folder.to_string(),
                modified: 0,
                pending: false,
            }
        })
        .collect();
        let find = |from: &str, target: &str, wiki| {
            resolve(&notes, from, target, wiki)
                .map(|n| n.path.as_str())
                .map_err(|(f, t)| format!("{f}|{t}"))
        };
        assert_eq!(find("Work", "Plan.md", false), Ok("Work/Plan.md"));
        assert_eq!(find("Work", "../Plan", false), Ok("Plan.md"));
        assert_eq!(find("Work/Ideas", "/Plan.md", false), Ok("Plan.md"));
        assert_eq!(
            find("", "Work/Ideas/Big one.md", false),
            Ok("Work/Ideas/Big one.md")
        );
        assert_eq!(find("", "Old.txt", false), Ok("Old.txt"));
        assert_eq!(find("Work", "New.md", false), Err("Work|New".into()));
        assert_eq!(find("Work", "plan", true), Ok("Work/Plan.md"));
        assert_eq!(find("Elsewhere", "Plan", true), Ok("Plan.md"));
        assert_eq!(find("", "big ONE", true), Ok("Work/Ideas/Big one.md"));
        assert_eq!(find("", "Ideas/Big one", true), Ok("Work/Ideas/Big one.md"));
        assert_eq!(find("Work", "Missing", true), Err("Work|Missing".into()));
        assert_eq!(find("Work", "A/Missing", true), Err("A|Missing".into()));
    }

    #[test]
    fn following_and_backlinks() {
        let c = client("links");
        let a = c
            .add_note("".into(), "A".into(), "See [[B]] and [c](Sub/C.md).".into())
            .unwrap();
        let b = c
            .add_note("".into(), "B".into(), "Back to [A](A.md)".into())
            .unwrap();
        assert_eq!(c.follow_link(a.id, "B".into(), true).unwrap().id, b.id);
        assert_eq!(c.follow_link(a.id, "".into(), false).unwrap().id, a.id);
        // A note the link names and that is not there yet is made.
        let made = c.follow_link(a.id, "Sub/C.md".into(), false).unwrap();
        assert_eq!(made.path, "Sub/C.md");
        assert_eq!(
            c.follow_link(a.id, "Sub/C.md".into(), false).unwrap().id,
            made.id
        );
        let paths = |v: Vec<Note>| v.into_iter().map(|n| n.path).collect::<Vec<_>>();
        assert_eq!(paths(c.backlinks(a.id)), ["B.md"]);
        assert_eq!(paths(c.backlinks(b.id)), ["A.md"]);
        assert_eq!(paths(c.backlinks(made.id)), ["A.md"]);
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
        // A folder moves with what is in it; never on the server, nothing waits to move it.
        assert_eq!(
            c.move_folder("Work".into(), "Old".into()).unwrap(),
            "Old/Work"
        );
        assert_eq!(c.note(a.id).unwrap().path, "Old/Work/Plan.md");
        assert!(matches!(
            c.move_folder("Old".into(), "Old/Work".into()),
            Err(Error::Invalid(_))
        ));
        c.add_folder("Work".into()).unwrap();
        assert!(matches!(
            c.move_folder("Work".into(), "Old".into()),
            Err(Error::Exists(_))
        ));
        assert_eq!(c.folders(), ["Old", "Old/Work", "Work"]);
        assert_eq!(
            c.rename_folder("Old/Work".into(), " Done/ ".into())
                .unwrap(),
            "Old/Done-"
        );
        assert!(matches!(
            c.rename_folder("Old".into(), "Work".into()),
            Err(Error::Exists(_))
        ));
        assert_eq!(c.note(a.id).unwrap().path, "Old/Done-/Plan.md");
        // Never uploaded: gone at once.
        c.delete_note(b.id).unwrap();
        assert_eq!(c.notes().len(), 1);
        assert!(c.pending());
        c.delete_folder("Old".into()).unwrap();
        assert!(c.notes().is_empty());
        assert_eq!(c.folders(), ["Work"]);
        c.delete_folder("Work".into()).unwrap();
        assert!(!c.pending());
    }

    #[test]
    fn undo_puts_back_deletes_and_moves() {
        let c = client("undo");
        let a = c
            .add_note("Work".into(), "Plan".into(), "p".into())
            .unwrap();
        let b = c.add_note("".into(), "Later".into(), "l".into()).unwrap();
        c.add_folder("Work/Empty".into()).unwrap();
        let paths = || -> Vec<String> { c.notes().into_iter().map(|n| n.path).collect() };

        c.move_note(b.id, "Work".into(), "Later".into()).unwrap();
        let moved = c.last_change();
        // Never uploaded: its row goes, and comes back with its id and text.
        c.delete_note(a.id).unwrap();
        assert!(c.undo(c.last_change()).unwrap());
        assert_eq!(c.note(a.id).unwrap().path, "Work/Plan.md");
        assert_eq!(c.note_text(a.id).as_deref(), Some("p"));

        // An older step, by its id; its name taken since: a number is added.
        c.add_note("".into(), "Later".into(), "new".into()).unwrap();
        assert!(c.undo(moved).unwrap());
        assert_eq!(c.note(b.id).unwrap().path, "Later 2.md");
        assert!(!c.undo(moved).unwrap(), "a step is undone once");

        c.delete_folder("Work".into()).unwrap();
        assert_eq!(paths(), ["Later 2.md", "Later.md"]);
        assert!(c.undo(None).unwrap());
        assert_eq!(paths(), ["Later 2.md", "Later.md", "Work/Plan.md"]);
        assert_eq!(c.folders(), ["Work", "Work/Empty"]);

        c.rename_folder("Work".into(), "Done".into()).unwrap();
        assert!(c.undo(None).unwrap());
        assert_eq!(c.note(a.id).unwrap().path, "Work/Plan.md");
        assert!(!c.undo(None).unwrap());
    }
}
