//! SQLite schema, migrations by `PRAGMA user_version`, and the key-value table.

use crate::{Error, Result};
use rusqlite::{Connection, OptionalExtension};

/// The schema, then each change to it (none yet), applied in order from the database's
/// `user_version`. Released steps are never edited: a change is a new step (see `AGENTS.md`).
const MIGRATIONS: &[&str] = &[r#"
CREATE TABLE kv (key TEXT PRIMARY KEY, value TEXT);
-- Where notes are kept (`account.rs`).
CREATE TABLE accounts (
  id INTEGER PRIMARY KEY,
  kind TEXT NOT NULL,                   -- 'nextcloud' or 'local'
  name TEXT NOT NULL DEFAULT '',        -- given by the user; '': the front-end names it
  url TEXT NOT NULL DEFAULT '',
  user TEXT NOT NULL DEFAULT '',
  source TEXT NOT NULL DEFAULT '',      -- the front-end's: where the password comes from
  enabled INTEGER NOT NULL DEFAULT 1,
  position INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE notes (
  -- Never reused after a delete: undo (`undo.rs`) finds notes by them.
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  account INTEGER NOT NULL,
  path TEXT NOT NULL,                   -- relative to the notes folder: "Folder/Title.md"
  fileid TEXT,                          -- oc:id; NULL until uploaded
  etag TEXT,
  server_path TEXT,                     -- the server's path while a local move waits for the push
  text TEXT NOT NULL,
  base TEXT,                            -- the text as of the last sync (for a merge, v2)
  mtime INTEGER,                        -- the server's modification time of that version
  modified INTEGER NOT NULL,            -- unix time of the last change, here or on the server
  dirty INTEGER NOT NULL DEFAULT 0,     -- 0 clean, 1 modified, 2 deleted locally
  favorite INTEGER NOT NULL DEFAULT 0,  -- Nextcloud's favorites (`oc:favorite`)
  favorite_dirty INTEGER NOT NULL DEFAULT 0, -- starred or unstarred here, not on the server yet
  UNIQUE (account, path),
  UNIQUE (account, fileid)              -- a file shared with two accounts has one oc:id
);
-- Search over each note's path (its title and folders) and text, case and diacritics ignored;
-- "ł" has no base letter in Unicode, so it is folded here (and in the query, `notes::search`).
CREATE VIRTUAL TABLE notes_fts USING fts5(
  path, text, content = '', contentless_delete = 1, tokenize = 'unicode61 remove_diacritics 2'
);
CREATE TRIGGER notes_fts_insert AFTER INSERT ON notes BEGIN
  INSERT INTO notes_fts (rowid, path, text) VALUES (new.id,
    replace(replace(new.path, 'ł', 'l'), 'Ł', 'L'), replace(replace(new.text, 'ł', 'l'), 'Ł', 'L'));
END;
CREATE TRIGGER notes_fts_delete AFTER DELETE ON notes BEGIN
  DELETE FROM notes_fts WHERE rowid = old.id;
END;
CREATE TRIGGER notes_fts_update AFTER UPDATE OF path, text ON notes BEGIN
  DELETE FROM notes_fts WHERE rowid = old.id;
  INSERT INTO notes_fts (rowid, path, text) VALUES (new.id,
    replace(replace(new.path, 'ł', 'l'), 'Ł', 'L'), replace(replace(new.text, 'ł', 'l'), 'Ł', 'L'));
END;
CREATE TABLE folders (
  account INTEGER NOT NULL,
  path TEXT NOT NULL,                   -- relative, no trailing slash; '' is the notes folder
  etag TEXT,                            -- as last pulled; NULL: not pulled yet
  dirty INTEGER NOT NULL DEFAULT 0,     -- 1: created here, not on the server yet; 2: deleted here
  PRIMARY KEY (account, path)
);
-- Folders moved here, in order, until the push moves them on the server (paths relative to
-- the notes folder).
CREATE TABLE moves (
  id INTEGER PRIMARY KEY,
  account INTEGER NOT NULL,
  src TEXT NOT NULL,
  dst TEXT NOT NULL
);
"#];

pub const CLEAN: i64 = 0;
pub const MODIFIED: i64 = 1;
pub const DELETED: i64 = 2;
/// `folders.dirty`: deleted here, still on the server (1: made here, not there yet).
pub const FOLDER_DELETED: i64 = 2;

/// A note as stored, with its sync state.
#[derive(Debug, Clone)]
pub struct Row {
    pub id: i64,
    pub account: i64,
    pub path: String,
    pub fileid: Option<String>,
    pub etag: Option<String>,
    pub server_path: Option<String>,
    pub text: String,
    pub base: Option<String>,
    pub mtime: Option<i64>,
    pub modified: i64,
    pub dirty: i64,
    pub favorite: bool,
}

const ROW: &str = "SELECT id, path, fileid, etag, server_path, text, base, mtime, modified, dirty,
    favorite, account FROM notes";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Row> {
    Ok(Row {
        id: r.get(0)?,
        path: r.get(1)?,
        fileid: r.get(2)?,
        etag: r.get(3)?,
        server_path: r.get(4)?,
        text: r.get(5)?,
        base: r.get(6)?,
        mtime: r.get(7)?,
        modified: r.get(8)?,
        dirty: r.get(9)?,
        favorite: r.get(10)?,
        account: r.get(11)?,
    })
}

/// Notes matching `filter` (an SQL condition on `notes`, with `args`).
pub fn rows(db: &Connection, filter: &str, args: impl rusqlite::Params) -> Result<Vec<Row>> {
    let mut st = db.prepare(&format!("{ROW} WHERE {filter} ORDER BY path"))?;
    let rows = st.query_map(args, row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn by_id(db: &Connection, id: i64) -> Result<Option<Row>> {
    Ok(rows(db, "id = ?", [id])?.pop())
}

pub fn by_path(db: &Connection, account: i64, path: &str) -> Result<Option<Row>> {
    Ok(rows(
        db,
        "account = ? AND path = ?",
        rusqlite::params![account, path],
    )?
    .pop())
}

pub fn by_fileid(db: &Connection, account: i64, fileid: &str) -> Result<Option<Row>> {
    Ok(rows(
        db,
        "account = ? AND fileid = ?",
        rusqlite::params![account, fileid],
    )?
    .pop())
}

pub fn open(path: &str) -> Result<Connection> {
    let db = Connection::open(path)?;
    db.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
    let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    // A newer Notav's: its schema may not be what this one reads and writes.
    if version > MIGRATIONS.len() as i64 {
        return Err(Error::Db(format!(
            "{path} was made by a newer version of Notav (schema {version}, this one knows up to \
             {}); update Notav to open it",
            MIGRATIONS.len()
        )));
    }
    for (i, m) in MIGRATIONS.iter().enumerate().skip(version as usize) {
        let tx = db.unchecked_transaction()?;
        tx.execute_batch(m)?;
        tx.pragma_update(None, "user_version", i as i64 + 1)?;
        tx.commit()?;
    }
    Ok(db)
}

pub fn kv_get(db: &Connection, key: &str) -> Result<Option<String>> {
    Ok(db
        .query_row("SELECT value FROM kv WHERE key = ?", [key], |r| r.get(0))
        .optional()?)
}

pub fn kv_set(db: &Connection, key: &str, value: &str) -> Result<()> {
    db.execute(
        "INSERT OR REPLACE INTO kv (key, value) VALUES (?, ?)",
        [key, value],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newer_notavs_database_is_left_alone() {
        let dir = std::env::temp_dir().join(format!("notav-test-schema-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("notav.sqlite").to_string_lossy().to_string();
        drop(open(&path).unwrap());
        // Opened again: nothing to do.
        let db = open(&path).unwrap();
        let version: i64 = db
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
        db.pragma_update(None, "user_version", version + 1).unwrap();
        drop(db);
        assert!(matches!(open(&path), Err(Error::Db(e)) if e.contains("newer version")));
    }
}
