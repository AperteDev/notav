//! The sync engine: push local changes, then pull the notes folder by folder ETags.
//!
//! A folder's ETag changes when anything below it does, so the pull only lists folders whose
//! ETag changed and only downloads files whose ETag changed. Notes are matched by `oc:id`, which
//! stays with a file across a rename or move.

use crate::account::{AccountKind, key};
use crate::dav::{Dav, Entry};
use crate::notes::{free_path, is_note, split, under};
use crate::store::{self, CLEAN, DELETED, FOLDER_DELETED, MODIFIED, Row};
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
    /// Changes that could not go up: something locked on the server, by an editor that has it
    /// open (or had: such locks can outlive it). They stay changed here, for the next sync.
    pub locked: Vec<Locked>,
}

impl SyncReport {
    fn add(&mut self, other: SyncReport) {
        self.uploaded += other.uploaded;
        self.downloaded += other.downloaded;
        self.removed += other.removed;
        self.conflicts.extend(other.conflicts);
        self.locked.extend(other.locked);
    }
}

/// A note (or folder) whose change waits for a lock on the server.
#[derive(Debug, Clone, PartialEq)]
pub struct Locked {
    pub account: i64,
    /// Here, relative to the notes folder.
    pub path: String,
    /// The locked file on the server, relative to the notes folder, for `Client::unlock`; None
    /// when a folder's move waits for something locked in it.
    pub file: Option<String>,
    /// Who holds the lock, as Nextcloud names it ("Text").
    pub owner: Option<String>,
    /// Since when (unix time).
    pub since: Option<i64>,
}

/// The notes folder unless set: Nextcloud Notes' default.
pub const DEFAULT_FOLDER: &str = "Notes";

impl Client {
    /// Settings, then every account that syncs; all are tried, the first failure is returned.
    pub fn sync(&self) -> Result<SyncReport> {
        let mut result = self.sync_settings().map(|()| SyncReport::default());
        for a in self.accounts() {
            if a.kind != AccountKind::Nextcloud || !self.syncing(a.id) {
                continue;
            }
            match (self.sync_account(a.id), &mut result) {
                (Ok(r), Ok(report)) => report.add(r),
                (Err(e), Ok(_)) => result = Err(e),
                _ => {}
            }
        }
        result
    }

    /// Account `id`'s notes: push local changes, pull the server's. Nothing until its notes
    /// folder is chosen.
    pub fn sync_account(&self, id: i64) -> Result<SyncReport> {
        let _guard = self.sync_lock.lock().unwrap();
        if self.chosen_notes_folder(id).is_none() {
            return Ok(SyncReport::default());
        }
        if !self.features_asked.lock().unwrap().contains(&id) {
            // Not worth failing the sync over: the trash and versions wait for the next run.
            let _ = self.refresh_features(id);
        }
        let dav = self.dav(id)?;
        let root = self.notes_root(id)?;
        let root_key = key(id, "notes_root");
        {
            // Another notes folder: list it all again. Notes keep their rows; those moved
            // along are found by oc:id. Folders deleted here were in the other one.
            let db = self.db.lock().unwrap();
            if store::kv_get(&db, &root_key)?.as_deref() != Some(&root) {
                db.execute("DELETE FROM folders WHERE account = ? AND dirty != 1", [id])?;
            }
        }
        let mut report = SyncReport::default();
        let mut changed = Vec::new();
        // Folders first: until they are moved, the server does not have what this copy shows
        // (pushing notes into them or pulling would undo the move), so the rest waits.
        let result = self
            .push_folder_moves(id, &dav, &root, &mut report)
            .and_then(|moved| match moved {
                true => self
                    .push(id, &dav, &root, &mut report, &mut changed)
                    .and_then(|_| self.push_favorites(id, &root))
                    .and_then(|_| self.pull(id, &dav, &root, &mut report, &mut changed))
                    // Starring changes no ETag: the favorites are asked for each time.
                    .and_then(|_| self.pull_favorites(id))
                    .map(|starred| changed.extend(starred)),
                false => Ok(()),
            });
        self.emit(changed);
        result?;
        store::kv_set(&self.db.lock().unwrap(), &root_key, &root)?;
        Ok(report)
    }

    /// Account `id`'s notes folder, relative to its Files root ("Notes", "Documents/Notes"),
    /// once chosen.
    pub fn chosen_notes_folder(&self, id: i64) -> Option<String> {
        store::kv_get(&self.db.lock().unwrap(), &key(id, "notes_folder"))
            .ok()?
            .map(|f| f.trim_matches('/').to_string())
            .filter(|f| !f.is_empty())
    }

    /// Account `id`'s notes folder, `DEFAULT_FOLDER` until chosen.
    pub fn notes_folder(&self, id: i64) -> String {
        self.chosen_notes_folder(id)
            .unwrap_or_else(|| DEFAULT_FOLDER.into())
    }

    /// The notes folder to offer for account `id`: for the settings account the one set on
    /// another device (the settings are synced first), else Nextcloud Notes' own, else the
    /// default.
    pub fn suggested_notes_folder(&self, id: i64) -> String {
        if self.settings_account() == Some(id) {
            let _ = self.sync_settings();
            if let Some(f) = self.setting("notes_folder".into()) {
                return f;
            }
        }
        let from_notes_app = || {
            let caps = self.capabilities(id).ok()?;
            caps.get("notes")?
                .get("notes_path")?
                .str()
                .map(|p| p.trim_matches('/').to_string())
                .filter(|p| !p.is_empty())
        };
        from_notes_app().unwrap_or_else(|| DEFAULT_FOLDER.into())
    }

    /// Use another notes folder for account `id`; `move_notes`: move what is in the current one
    /// there first (on the server). The new folder is created when missing. The settings
    /// account's is offered to other devices (`suggested_notes_folder`).
    pub fn set_notes_folder(&self, id: i64, folder: String, move_notes: bool) -> Result<()> {
        let folder = folder.trim_matches('/').to_string();
        if folder.is_empty() || folder.split('/').any(|s| s.starts_with('.')) {
            return Err(Error::Invalid(folder));
        }
        let old = self.notes_root(id)?;
        let home = self.files_home(id)?;
        let new = format!("{home}{folder}/");
        if move_notes && new != old {
            if new.starts_with(&old) || old.starts_with(&new) {
                return Err(Error::Invalid(folder));
            }
            // Changes made here go to the old folder first, so they move along.
            self.sync_account(id)?;
            let _guard = self.sync_lock.lock().unwrap();
            let dav = self.dav(id)?;
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
        store::kv_set(&self.db.lock().unwrap(), &key(id, "notes_folder"), &folder)?;
        if self.settings_account() == Some(id) {
            self.set_setting("notes_folder".into(), folder)?;
        }
        Ok(())
    }

    /// Remove the lock on `file` (a `Locked::file`) on account `id`'s server, whoever holds it:
    /// what the editor holding it saves afterwards may be lost. The next sync uploads what
    /// waited.
    pub fn unlock(&self, id: i64, file: String) -> Result<()> {
        let dav = self.dav(id)?;
        let href = format!("{}{file}", self.notes_root(id)?);
        match dav.lock_info(&href)? {
            Some(_) => dav.unlock(&href),
            None => Ok(()),
        }
    }

    /// Account `id`'s notes folder's href, ending in "/".
    pub(crate) fn notes_root(&self, id: i64) -> Result<String> {
        Ok(format!(
            "{}{}/",
            self.files_home(id)?,
            self.notes_folder(id)
        ))
    }

    /// MOVE the folders moved here on the server, in order: false while one cannot go yet
    /// (something in it is locked; reported in `report.locked`).
    fn push_folder_moves(
        &self,
        account: i64,
        dav: &Dav,
        root: &str,
        report: &mut SyncReport,
    ) -> Result<bool> {
        let moves: Vec<(i64, String, String)> = {
            let db = self.db.lock().unwrap();
            let mut st =
                db.prepare("SELECT id, src, dst FROM moves WHERE account = ? ORDER BY id")?;
            st.query_map([account], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<rusqlite::Result<_>>()?
        };
        for (id, src, dst) in moves {
            let to = format!("{root}{dst}/");
            dav.mkcol_all(
                &self.files_home(account)?,
                &to[..to[..to.len() - 1].rfind('/').unwrap() + 1],
            )?;
            match dav.mv(&format!("{root}{src}/"), &to) {
                // Gone from the server (moved or deleted elsewhere): the pull finds its notes by
                // oc:id, or uploads the ones changed here again.
                Ok(()) | Err(Error::NotFound(_)) => {}
                // A folder has that name there on the server: its notes move one by one, each
                // from where the server has it (a name taken there gets a conflict name).
                Err(Error::Conflict) => {
                    let db = self.db.lock().unwrap();
                    let under = |c: &str| format!("substr({c}, 1, length(?1) + 1) = ?1 || '/'");
                    db.execute(
                        &format!(
                            "UPDATE notes SET server_path = ?2 || substr(server_path, length(?1) + 1),
                               dirty = 1 WHERE account = ?3 AND {}",
                            under("server_path")
                        ),
                        params![dst, src, account],
                    )?;
                    db.execute(
                        &format!(
                            "UPDATE notes SET server_path = ?2 || substr(path, length(?1) + 1),
                               dirty = 1
                             WHERE account = ?3 AND fileid IS NOT NULL AND server_path IS NULL
                               AND {}",
                            under("path")
                        ),
                        params![dst, src, account],
                    )?;
                }
                Err(Error::Locked(_)) => {
                    report.locked.push(Locked {
                        account,
                        path: dst,
                        file: None,
                        owner: None,
                        since: None,
                    });
                    return Ok(false);
                }
                Err(e) => return Err(e),
            }
            self.db
                .lock()
                .unwrap()
                .execute("DELETE FROM moves WHERE id = ?", [id])?;
        }
        Ok(true)
    }

    fn push(
        &self,
        account: i64,
        dav: &Dav,
        root: &str,
        report: &mut SyncReport,
        changed: &mut Vec<String>,
    ) -> Result<()> {
        let folders: Vec<String> = {
            let db = self.db.lock().unwrap();
            let mut st = db.prepare("SELECT path FROM folders WHERE account = ? AND dirty = 1")?;
            st.query_map([account], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        for f in folders {
            dav.mkcol_all(&self.files_home(account)?, &format!("{root}{f}/"))?;
            let db = self.db.lock().unwrap();
            db.execute(
                "UPDATE folders SET dirty = 0 WHERE account = ? AND path = ?",
                params![account, f],
            )?;
        }
        let mut rows = {
            let db = self.db.lock().unwrap();
            store::rows(&db, "dirty != ? AND account = ?", [CLEAN, account])?
        };
        // Moves first: they free names that new notes here may have taken.
        rows.sort_by_key(|r| r.server_path.is_none());
        for r in rows {
            let path = r.path.clone();
            match self.push_note(dav, root, r, report, changed) {
                // Open in an editor that locks it (Nextcloud Text, a WebDAV LOCK): it stays
                // changed here, and the next sync tries again.
                Err(Error::Locked(href)) => {
                    let lock = dav.lock_info(&href).ok().flatten();
                    report.locked.push(Locked {
                        account,
                        path,
                        file: href.strip_prefix(root).map(str::to_string),
                        owner: lock.as_ref().and_then(|l| l.owner.clone()),
                        since: lock.and_then(|l| l.since),
                    })
                }
                r => r?,
            }
        }
        self.push_folder_deletions(account, dav, root, report)
    }

    /// DELETE the folders deleted here, after their notes: unless a note in one stays (changed
    /// on the server, or new there), which keeps its folder; one locked waits.
    fn push_folder_deletions(
        &self,
        account: i64,
        dav: &Dav,
        root: &str,
        report: &mut SyncReport,
    ) -> Result<()> {
        let folders: Vec<String> = {
            let db = self.db.lock().unwrap();
            let mut st = db.prepare("SELECT path FROM folders WHERE account = ? AND dirty = ?")?;
            st.query_map([account, FOLDER_DELETED], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        for f in folders {
            let keep = |db: &Connection| {
                db.execute(
                    "UPDATE folders SET dirty = 0 WHERE account = ? AND path = ?",
                    params![account, f],
                )
                .map(|_| ())
            };
            let notes: Vec<i64> = {
                let db = self.db.lock().unwrap();
                let mut st = db.prepare(&format!(
                    "SELECT dirty FROM notes WHERE account = ?2 AND ({} OR {})",
                    under("path"),
                    under("server_path")
                ))?;
                st.query_map(params![f, account], |r| r.get(0))?
                    .collect::<rusqlite::Result<_>>()?
            };
            if notes.iter().any(|&d| d != DELETED) {
                keep(&self.db.lock().unwrap())?;
                continue;
            }
            if !notes.is_empty() {
                continue; // locked: reported with the note
            }
            let href = format!("{root}{f}/");
            if has_notes(dav, &href)? {
                // New there since the last pull: the pull brings them.
                keep(&self.db.lock().unwrap())?;
                continue;
            }
            match dav.delete(&href, None) {
                Ok(()) => {
                    let db = self.db.lock().unwrap();
                    db.execute(
                        "DELETE FROM folders WHERE account = ? AND path = ? AND dirty = ?",
                        params![account, f, FOLDER_DELETED],
                    )?;
                }
                Err(Error::Locked(_)) => report.locked.push(Locked {
                    account,
                    path: f,
                    file: None,
                    owner: None,
                    since: None,
                }),
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Upload one note's change: its deletion, move or text.
    fn push_note(
        &self,
        dav: &Dav,
        root: &str,
        r: Row,
        report: &mut SyncReport,
        changed: &mut Vec<String>,
    ) -> Result<()> {
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
            return Ok(());
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
        dav.mkcol_all(
            &self.files_home(r.account)?,
            &to[..to.rfind('/').unwrap() + 1],
        )?;
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
                let path = conflict_path(&db, r.account, folder, stem, ext, 1)?;
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
                dav.mkcol_all(
                    &self.files_home(r.account)?,
                    &href[..href.rfind('/').unwrap() + 1],
                )?;
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
        // The file there is another note changed here (moving away from this name, or moved to
        // it on the server): this one waits until that note's push lets go of it.
        if store::by_fileid(&self.db.lock().unwrap(), r.account, &entry.fileid)?
            .is_some_and(|n| n.id != r.id && n.dirty != CLEAN)
        {
            return Ok(());
        }
        let take_server = |db: &Connection| -> Result<()> {
            // Another note here may hold this oc:id (moved on the server): it is this one now.
            db.execute(
                "DELETE FROM notes WHERE account = ? AND fileid = ? AND id != ? AND dirty = 0",
                params![r.account, entry.fileid, r.id],
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
                (
                    conflict_path(&db, r.account, folder, stem, ext, attempt)?,
                    text,
                )
            };
            let mtime = now();
            match dav.put(&format!("{root}{copy}"), &text, None, mtime) {
                Ok((etag, fileid)) => {
                    let db = self.db.lock().unwrap();
                    db.execute(
                        "INSERT INTO notes
                           (account, path, fileid, etag, text, base, mtime, modified, dirty)
                         VALUES (?7, ?1, ?2, ?3, ?4, ?4, ?5, ?5, ?6)",
                        params![copy, fileid, etag, text, mtime, CLEAN, r.account],
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
        account: i64,
        dav: &Dav,
        root: &str,
        report: &mut SyncReport,
        changed: &mut Vec<String>,
    ) -> Result<()> {
        let mut top = dav.list(root, "0")?;
        if top.is_empty() {
            dav.mkcol_all(&self.files_home(account)?, root)?;
            top = dav.list(root, "0")?;
        }
        let top = top.pop().ok_or_else(|| Error::NotFound(root.into()))?;
        let stored: HashMap<String, Option<String>> = {
            let db = self.db.lock().unwrap();
            let mut st = db.prepare("SELECT path, etag FROM folders WHERE account = ?")?;
            st.query_map([account], |r| Ok((r.get(0)?, r.get(1)?)))?
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
        for n in store::rows(&db, "account = ? AND fileid IS NOT NULL", [account])? {
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
        // Notes known here first: their moves free names that new ones take.
        let known: HashSet<String> = {
            let mut st =
                db.prepare("SELECT fileid FROM notes WHERE account = ? AND fileid IS NOT NULL")?;
            st.query_map([account], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?
        };
        drop(db);
        files.sort_by_key(|(_, e)| !known.contains(&e.fileid));

        for (path, e) in files {
            let local = {
                let db = self.db.lock().unwrap();
                store::by_fileid(&db, account, &e.fileid)?
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
                        if store::by_path(&db, account, &path)?.is_some() {
                            continue;
                        }
                    }
                    let Some((_, body)) = dav.get(&e.href)? else {
                        continue;
                    };
                    let db = self.db.lock().unwrap();
                    db.execute(
                        "INSERT OR IGNORE INTO notes
                           (path, fileid, etag, text, base, mtime, modified, dirty, account)
                         VALUES (?, ?, ?, ?, ?4, ?, ?5, ?, ?)",
                        params![path, e.fileid, e.etag, body, e.modified, CLEAN, account],
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
                "DELETE FROM folders WHERE account = ?2
                   AND (path = ?1 OR path LIKE ?1 || '/%') AND dirty = 0",
                params![g, account],
            )?;
        }
        for (f, etag) in &listed {
            tx.execute(
                "INSERT INTO folders (account, path, etag) VALUES (?, ?, ?)
                 ON CONFLICT (account, path) DO UPDATE SET etag = excluded.etag",
                params![account, f, etag],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// A note moved on the server: give it the new path, unless a note made here has it (that
    /// one is pushed later and finds the name taken).
    fn relocate(&self, n: &Row, path: &str, changed: &mut Vec<String>) -> Result<()> {
        let db = self.db.lock().unwrap();
        if store::by_path(&db, n.account, path)?.is_some() {
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

/// Folder `href` has notes, at any depth (dot-folders skipped, as the pull skips them).
fn has_notes(dav: &Dav, href: &str) -> Result<bool> {
    let mut queue = vec![href.to_string()];
    while let Some(h) = queue.pop() {
        for e in dav.list(&h, "1")?.into_iter().skip(1) {
            let name = e
                .href
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or("");
            if e.folder && !name.starts_with('.') {
                queue.push(e.href);
            } else if !e.folder && is_note(name) {
                return Ok(true);
            }
        }
    }
    Ok(false)
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
    account: i64,
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
    free_path(db, account, folder, &title, ext)
}
