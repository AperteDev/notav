//! The sync engine: push local changes, then pull the notes folder by folder ETags.
//!
//! A folder's ETag changes when anything below it does, so the pull only lists folders whose
//! ETag changed and only downloads files whose ETag changed. Notes are matched by `oc:id`, which
//! stays with a file across a rename or move.

use crate::dav::{Dav, Entry};
use crate::notes::{free_path, is_note, split};
use crate::store::{self, CLEAN, DELETED, MODIFIED, Row};
use crate::{Client, Error, Result, now};
use rusqlite::{Connection, params};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct SyncReport {
    pub uploaded: u32,
    pub downloaded: u32,
    pub removed: u32,
    /// Conflict copies made: notes changed here and on the server (paths of the copies).
    pub conflicts: Vec<String>,
}

/// The notes folder unless set: Nextcloud Notes' default.
pub const DEFAULT_FOLDER: &str = "Notes";

impl Client {
    /// Settings, then notes: push local changes, pull the server's.
    pub fn sync(&self) -> Result<SyncReport> {
        let _guard = self.sync_lock.lock().unwrap();
        self.sync_settings()?;
        let dav = self.dav()?;
        let root = self.notes_root()?;
        {
            // Another notes folder (here or from another device): list it all again. Notes keep
            // their rows; those moved along are found by oc:id.
            let db = self.db.lock().unwrap();
            if store::kv_get(&db, "config_notes_root")?.as_deref() != Some(&root) {
                db.execute("DELETE FROM folders WHERE dirty = 0", [])?;
            }
        }
        let mut report = SyncReport::default();
        let mut changed = Vec::new();
        let result = self
            .push(&dav, &root, &mut report, &mut changed)
            .and_then(|_| self.pull(&dav, &root, &mut report, &mut changed));
        self.emit(changed);
        result?;
        store::kv_set(&self.db.lock().unwrap(), "config_notes_root", &root)?;
        Ok(report)
    }

    /// The notes folder, relative to the Files root ("Notes", "Documents/Notes").
    pub fn notes_folder(&self) -> String {
        self.setting("notes_folder".into())
            .map(|f| f.trim_matches('/').to_string())
            .filter(|f| !f.is_empty())
            .unwrap_or_else(|| DEFAULT_FOLDER.into())
    }

    /// The notes folder to offer when signing in: the one set on another device (the settings
    /// are synced first), else Nextcloud Notes' own, else the default.
    pub fn suggested_notes_folder(&self) -> String {
        let _ = self.sync_settings();
        if let Some(f) = self.setting("notes_folder".into()) {
            return f;
        }
        let from_notes_app = || {
            let dav = self.dav().ok()?;
            let url = self.account()?.url;
            let r = dav
                .request(
                    "GET",
                    &format!(
                        "{}/ocs/v2.php/cloud/capabilities?format=json",
                        url.trim_end_matches('/')
                    ),
                    &[("OCS-APIRequest", "true")],
                    "",
                )
                .ok()?;
            let json = crate::json::Json::parse(&r.body)?;
            let caps = json.get("ocs")?.get("data")?.get("capabilities")?;
            caps.get("notes")?
                .get("notes_path")?
                .str()
                .map(|p| p.trim_matches('/').to_string())
                .filter(|p| !p.is_empty())
        };
        from_notes_app().unwrap_or_else(|| DEFAULT_FOLDER.into())
    }

    /// Use another notes folder; `move_notes`: move what is in the current one there first (on
    /// the server). The new folder is created when missing.
    pub fn set_notes_folder(&self, folder: String, move_notes: bool) -> Result<()> {
        let folder = folder.trim_matches('/').to_string();
        if folder.is_empty() || folder.split('/').any(|s| s.starts_with('.')) {
            return Err(Error::Invalid(folder));
        }
        let old = self.notes_root()?;
        let home = self.files_home()?;
        let new = format!("{home}{folder}/");
        if new == old {
            return Ok(());
        }
        if new.starts_with(&old) || old.starts_with(&new) {
            return Err(Error::Invalid(folder));
        }
        if move_notes {
            // Changes made here go to the old folder first, so they move along.
            self.sync()?;
            let _guard = self.sync_lock.lock().unwrap();
            let dav = self.dav()?;
            let parent = &new[..new[..new.len() - 1].rfind('/').unwrap() + 1];
            dav.mkcol_all(&home, parent)?;
            match dav.mv(&old, &new) {
                Ok(()) => {}
                // It exists: move what is in the old folder into it, one by one; a name taken
                // there stays behind.
                Err(Error::Conflict) => {
                    for e in dav.list(&old, "1")?.iter().skip(1) {
                        let name = &e.href[old.len()..];
                        match dav.mv(&e.href, &format!("{new}{name}")) {
                            Ok(()) | Err(Error::Conflict) => {}
                            Err(e) => return Err(e),
                        }
                    }
                }
                Err(e) => return Err(e),
            }
        }
        self.set_setting("notes_folder".into(), folder)
    }

    /// The notes folder's href, ending in "/".
    fn notes_root(&self) -> Result<String> {
        Ok(format!("{}{}/", self.files_home()?, self.notes_folder()))
    }

    fn push(
        &self,
        dav: &Dav,
        root: &str,
        report: &mut SyncReport,
        changed: &mut Vec<String>,
    ) -> Result<()> {
        let folders: Vec<String> = {
            let db = self.db.lock().unwrap();
            let mut st = db.prepare("SELECT path FROM folders WHERE dirty = 1")?;
            st.query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        for f in folders {
            dav.mkcol_all(&self.files_home()?, &format!("{root}{f}/"))?;
            let db = self.db.lock().unwrap();
            db.execute("UPDATE folders SET dirty = 0 WHERE path = ?", [&f])?;
        }
        let rows = {
            let db = self.db.lock().unwrap();
            store::rows(&db, "dirty != ?", [CLEAN])?
        };
        for r in rows {
            if r.dirty == DELETED {
                let href = format!("{root}{}", r.server_path.as_ref().unwrap_or(&r.path));
                match dav.delete(&href, r.etag.as_deref()) {
                    Ok(()) => {
                        let db = self.db.lock().unwrap();
                        db.execute(
                            "DELETE FROM notes WHERE id = ? AND dirty = ?",
                            params![r.id, DELETED],
                        )?;
                    }
                    // Changed on the server since: that version stays, and comes back here.
                    Err(Error::Conflict) => {
                        let db = self.db.lock().unwrap();
                        db.execute(
                            "UPDATE notes SET dirty = ?, etag = NULL, server_path = NULL,
                               path = coalesce(server_path, path) WHERE id = ?",
                            params![CLEAN, r.id],
                        )?;
                    }
                    Err(e) => return Err(e),
                }
                continue;
            }
            let mut r = r;
            if let Some(from) = r.server_path.clone() {
                r = self.push_move(dav, root, r, &from, changed)?;
            }
            if r.base.as_deref() != Some(&r.text) || r.fileid.is_none() {
                self.upload(dav, root, &r, report, changed)?;
            } else {
                let db = self.db.lock().unwrap();
                db.execute(
                    "UPDATE notes SET dirty = ? WHERE id = ? AND text = ? AND server_path IS NULL",
                    params![CLEAN, r.id, r.text],
                )?;
            }
        }
        Ok(())
    }

    /// Move the server copy of a note moved here: the row as it is now.
    fn push_move(
        &self,
        dav: &Dav,
        root: &str,
        r: Row,
        from: &str,
        changed: &mut Vec<String>,
    ) -> Result<Row> {
        let to = format!("{root}{}", r.path);
        dav.mkcol_all(&self.files_home()?, &to[..to.rfind('/').unwrap() + 1])?;
        let db_row = |db: &Connection| store::by_id(db, r.id).map(|o| o.unwrap_or(r.clone()));
        match dav.mv(&format!("{root}{from}"), &to) {
            Ok(()) => {
                let db = self.db.lock().unwrap();
                db.execute("UPDATE notes SET server_path = NULL WHERE id = ?", [r.id])?;
                db_row(&db)
            }
            // Gone from the server: upload it as a new note.
            Err(Error::NotFound(_)) => {
                let db = self.db.lock().unwrap();
                db.execute(
                    "UPDATE notes SET server_path = NULL, fileid = NULL, etag = NULL WHERE id = ?",
                    [r.id],
                )?;
                db_row(&db)
            }
            // A file has that name on the server: this note takes another one.
            Err(Error::Conflict) => {
                let db = self.db.lock().unwrap();
                let (folder, name) = split(&r.path);
                let (stem, ext) = name.rsplit_once('.').unwrap_or((name, "md"));
                let path = conflict_path(&db, folder, stem, ext, 1)?;
                db.execute(
                    "UPDATE notes SET path = ? WHERE id = ?",
                    params![path, r.id],
                )?;
                changed.extend([r.path.clone(), path]);
                drop(db);
                let r = store::by_id(&self.db.lock().unwrap(), r.id)?.unwrap();
                self.push_move(dav, root, r, from, changed)
            }
            Err(e) => Err(e),
        }
    }

    /// PUT a note; on a conflict the server's version stays and this one is uploaded next to it.
    fn upload(
        &self,
        dav: &Dav,
        root: &str,
        r: &Row,
        report: &mut SyncReport,
        changed: &mut Vec<String>,
    ) -> Result<()> {
        let href = format!("{root}{}", r.path);
        let etag = r.etag.as_deref().filter(|_| r.fileid.is_some());
        // Later than the version it replaces, so its ETag differs (see `Dav::put`).
        let mtime = now().max(r.mtime.map_or(0, |m| m + 1));
        let result = match dav.put(&href, &r.text, etag, mtime) {
            Err(Error::NotFound(_)) => {
                dav.mkcol_all(&self.files_home()?, &href[..href.rfind('/').unwrap() + 1])?;
                dav.put(&href, &r.text, etag, mtime)
            }
            other => other,
        };
        match result {
            Ok((etag, fileid)) => {
                let fileid = match fileid {
                    Some(f) => Some(f),
                    None => dav.list(&href, "0")?.pop().map(|e| e.fileid),
                };
                let db = self.db.lock().unwrap();
                db.execute(
                    "UPDATE notes SET etag = ?1, fileid = ?2, base = ?3, mtime = ?4,
                       dirty = CASE WHEN text = ?3 AND server_path IS NULL THEN 0 ELSE dirty END
                     WHERE id = ?5",
                    params![etag, fileid, r.text, mtime, r.id],
                )?;
                report.uploaded += 1;
                Ok(())
            }
            Err(Error::Conflict) => self.conflict(dav, root, r, report, changed),
            Err(e) => Err(e),
        }
    }

    /// The note changed on the server too (or a new one has its name there): keep the server's
    /// version under the note's name and upload this one as a conflict copy.
    fn conflict(
        &self,
        dav: &Dav,
        root: &str,
        r: &Row,
        report: &mut SyncReport,
        changed: &mut Vec<String>,
    ) -> Result<()> {
        let href = format!("{root}{}", r.path);
        let entry = dav.list(&href, "0")?.pop();
        let server = dav.get(&href)?;
        let (Some(entry), Some((_, body))) = (entry, server) else {
            // Gone on the server: upload it as new.
            let db = self.db.lock().unwrap();
            db.execute(
                "UPDATE notes SET fileid = NULL, etag = NULL WHERE id = ?",
                [r.id],
            )?;
            drop(db);
            let r = Row {
                fileid: None,
                etag: None,
                ..r.clone()
            };
            return self.upload(dav, root, &r, report, changed);
        };
        let take_server = |db: &Connection| -> Result<()> {
            // Another note here may hold this oc:id (moved on the server): it is this one now.
            db.execute(
                "DELETE FROM notes WHERE fileid = ? AND id != ? AND dirty = 0",
                params![entry.fileid, r.id],
            )?;
            db.execute(
                "UPDATE notes SET text = ?, base = ?1, etag = ?, fileid = ?, mtime = ?,
                   modified = ?4, dirty = 0
                 WHERE id = ?",
                params![body, entry.etag, entry.fileid, entry.modified, r.id],
            )?;
            Ok(())
        };
        if body == r.text {
            take_server(&self.db.lock().unwrap())?;
            return Ok(());
        }
        // The text as it is now (it may have changed since the push read it) goes to the copy.
        let (folder, name) = split(&r.path);
        let (stem, ext) = name.rsplit_once('.').unwrap_or((name, "md"));
        for attempt in 1..=5 {
            let (copy, text) = {
                let db = self.db.lock().unwrap();
                let text = store::by_id(&db, r.id)?.map_or(r.text.clone(), |n| n.text);
                (conflict_path(&db, folder, stem, ext, attempt)?, text)
            };
            let mtime = now();
            match dav.put(&format!("{root}{copy}"), &text, None, mtime) {
                Ok((etag, fileid)) => {
                    let db = self.db.lock().unwrap();
                    db.execute(
                        "INSERT INTO notes (path, fileid, etag, text, base, mtime, modified, dirty)
                         VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?5, ?6)",
                        params![copy, fileid, etag, text, mtime, CLEAN],
                    )?;
                    take_server(&db)?;
                    report.conflicts.push(copy.clone());
                    changed.extend([r.path.clone(), copy]);
                    return Ok(());
                }
                // That name is taken on the server: try the next.
                Err(Error::Conflict) => {}
                Err(e) => return Err(e),
            }
        }
        Err(Error::Conflict)
    }

    fn pull(
        &self,
        dav: &Dav,
        root: &str,
        report: &mut SyncReport,
        changed: &mut Vec<String>,
    ) -> Result<()> {
        let mut top = dav.list(root, "0")?;
        if top.is_empty() {
            dav.mkcol_all(&self.files_home()?, root)?;
            top = dav.list(root, "0")?;
        }
        let top = top.pop().ok_or_else(|| Error::NotFound(root.into()))?;
        let stored: HashMap<String, Option<String>> = {
            let db = self.db.lock().unwrap();
            let mut st = db.prepare("SELECT path, etag FROM folders")?;
            st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?
        };
        if stored.get("").cloned().flatten().as_deref() == Some(&top.etag) {
            return Ok(());
        }

        // List the folders that changed, from the top down.
        let mut walked = HashSet::new();
        let mut listed: HashMap<String, String> = HashMap::new(); // folder → etag
        let mut files: Vec<(String, Entry)> = Vec::new();
        let mut queue = vec![String::new()];
        while let Some(folder) = queue.pop() {
            let href = format!("{root}{}", slash(&folder));
            let entries = dav.list(&href, "1")?;
            let mut entries = entries.into_iter();
            match entries.next() {
                Some(own) => {
                    listed.insert(folder.clone(), own.etag);
                }
                None => continue, // gone since its parent was listed
            }
            walked.insert(folder.clone());
            for e in entries {
                let Some(rel) = e.href.strip_prefix(root) else {
                    continue;
                };
                let rel = rel.trim_end_matches('/').to_string();
                let name = split(&rel).1;
                if name.starts_with('.') {
                    continue;
                }
                if e.folder {
                    if stored.get(&rel).cloned().flatten().as_deref() != Some(&e.etag) {
                        queue.push(rel.clone());
                    }
                    listed.entry(rel).or_insert(e.etag);
                } else if is_note(name) {
                    files.push((rel, e));
                }
            }
        }

        // Folders now: the stored ones, less those gone from a listed folder (and what was in
        // them), plus the listed ones.
        let gone: Vec<&String> = stored
            .keys()
            .filter(|f| !f.is_empty() && walked.contains(split(f).0) && !listed.contains_key(*f))
            .collect();
        let under_gone = |f: &str| {
            gone.iter()
                .any(|g| f == *g || f.starts_with(&format!("{g}/")))
        };
        let folders: HashSet<&str> = stored
            .keys()
            .filter(|f| !under_gone(f))
            .chain(listed.keys())
            .map(String::as_str)
            .collect();
        let seen: HashSet<&str> = files.iter().map(|(_, e)| e.fileid.as_str()).collect();

        let db = self.db.lock().unwrap();
        // Notes no longer on the server: in a listed folder or one that is not there (any more),
        // and not seen anywhere.
        for n in store::rows(&db, "fileid IS NOT NULL", [])? {
            let folder = split(n.server_path.as_ref().unwrap_or(&n.path)).0;
            if !(walked.contains(folder) || !folders.contains(folder))
                || seen.contains(n.fileid.as_deref().unwrap())
            {
                continue;
            }
            if n.dirty == MODIFIED {
                // Changed here: upload it again, as a new note.
                db.execute(
                    "UPDATE notes SET fileid = NULL, etag = NULL, server_path = NULL WHERE id = ?",
                    [n.id],
                )?;
            } else {
                db.execute("DELETE FROM notes WHERE id = ?", [n.id])?;
                report.removed += 1;
                changed.push(n.path);
            }
        }
        drop(db);

        for (path, e) in files {
            let local = {
                let db = self.db.lock().unwrap();
                store::by_fileid(&db, &e.fileid)?
            };
            match local {
                Some(n) if n.dirty != CLEAN => {
                    // Changed here; the push sorts it out (a conflict copy if the text changed
                    // there too). A move there is taken unless it was moved here as well.
                    if n.server_path.is_none() && n.path != path {
                        self.relocate(&n, &path, changed)?;
                    }
                }
                Some(n) => {
                    if n.path != path {
                        self.relocate(&n, &path, changed)?;
                    }
                    // Nextcloud can give a file written twice in one second the same ETag: a
                    // size other than the last synced text's is a change too.
                    let size = n.base.as_ref().map(|b| b.len() as u64);
                    if n.etag.as_deref() != Some(&e.etag) || e.size.is_some_and(|s| Some(s) != size)
                    {
                        let Some((_, body)) = dav.get(&e.href)? else {
                            continue;
                        };
                        let db = self.db.lock().unwrap();
                        db.execute(
                            "UPDATE notes SET text = ?, base = ?1, etag = ?, mtime = ?, modified = ?3
                             WHERE id = ? AND dirty = 0",
                            params![body, e.etag, e.modified, n.id],
                        )?;
                        report.downloaded += 1;
                        changed.push(path);
                    }
                }
                None => {
                    {
                        // A note made here waits for this name: the push sorts it out.
                        let db = self.db.lock().unwrap();
                        if store::by_path(&db, &path)?.is_some() {
                            continue;
                        }
                    }
                    let Some((_, body)) = dav.get(&e.href)? else {
                        continue;
                    };
                    let db = self.db.lock().unwrap();
                    db.execute(
                        "INSERT OR IGNORE INTO notes
                           (path, fileid, etag, text, base, mtime, modified, dirty)
                         VALUES (?, ?, ?, ?, ?4, ?, ?5, ?)",
                        params![path, e.fileid, e.etag, body, e.modified, CLEAN],
                    )?;
                    report.downloaded += 1;
                    changed.push(path);
                }
            }
        }

        let db = self.db.lock().unwrap();
        let tx = db.unchecked_transaction()?;
        for g in &gone {
            tx.execute(
                "DELETE FROM folders WHERE (path = ?1 OR path LIKE ?1 || '/%') AND dirty = 0",
                [g],
            )?;
        }
        for (f, etag) in &listed {
            tx.execute(
                "INSERT INTO folders (path, etag) VALUES (?, ?)
                 ON CONFLICT (path) DO UPDATE SET etag = excluded.etag",
                params![f, etag],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// A note moved on the server: give it the new path, unless a note made here has it (that
    /// one is pushed later and finds the name taken).
    fn relocate(&self, n: &Row, path: &str, changed: &mut Vec<String>) -> Result<()> {
        let db = self.db.lock().unwrap();
        if store::by_path(&db, path)?.is_some() {
            return Ok(());
        }
        db.execute(
            "UPDATE notes SET path = ? WHERE id = ?",
            params![path, n.id],
        )?;
        changed.extend([n.path.clone(), path.to_string()]);
        Ok(())
    }
}

/// "a/b" → "a/b/", "" → "".
fn slash(folder: &str) -> String {
    if folder.is_empty() {
        String::new()
    } else {
        format!("{folder}/")
    }
}

/// A free name for a conflict copy: "folder/stem (conflict 2026-10-24 19-30).ext" in local time;
/// "... 19-30) 2.ext" for the second `attempt` (the first was taken on the server), and so on.
fn conflict_path(
    db: &Connection,
    folder: &str,
    stem: &str,
    ext: &str,
    attempt: u32,
) -> Result<String> {
    let at: String = db.query_row(
        "SELECT strftime('%Y-%m-%d %H-%M', 'now', 'localtime')",
        [],
        |r| r.get(0),
    )?;
    let title = match attempt {
        1 => format!("{stem} (conflict {at})"),
        n => format!("{stem} (conflict {at}) {n}"),
    };
    free_path(db, folder, &title, ext)
}
