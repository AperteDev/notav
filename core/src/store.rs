//! SQLite schema, migrations by `PRAGMA user_version`, and the key-value table.

use crate::Result;
use rusqlite::{Connection, OptionalExtension};

const MIGRATIONS: &[&str] = &[r#"
CREATE TABLE kv (key TEXT PRIMARY KEY, value TEXT);
CREATE TABLE notes (
  id INTEGER PRIMARY KEY,
  path TEXT NOT NULL UNIQUE,            -- relative to the notes folder: "Folder/Title.md"
  fileid TEXT UNIQUE,                   -- oc:id; NULL until uploaded
  etag TEXT,
  server_path TEXT,                     -- the server's path while a local move waits for the push
  text TEXT NOT NULL,
  base TEXT,                            -- the text as of the last sync (for a merge, v2)
  mtime INTEGER,                        -- the server's modification time of that version
  modified INTEGER NOT NULL,            -- unix time of the last change, here or on the server
  dirty INTEGER NOT NULL DEFAULT 0      -- 0 clean, 1 modified, 2 deleted locally
);
CREATE TABLE folders (
  path TEXT PRIMARY KEY,                -- relative, no trailing slash; '' is the notes folder
  etag TEXT,                            -- as last pulled; NULL: not pulled yet
  dirty INTEGER NOT NULL DEFAULT 0      -- 1: created here, not on the server yet
);
"#];

pub const CLEAN: i64 = 0;
pub const MODIFIED: i64 = 1;
pub const DELETED: i64 = 2;

/// A note as stored, with its sync state.
#[derive(Debug, Clone)]
pub struct Row {
    pub id: i64,
    pub path: String,
    pub fileid: Option<String>,
    pub etag: Option<String>,
    pub server_path: Option<String>,
    pub text: String,
    pub base: Option<String>,
    pub mtime: Option<i64>,
    pub modified: i64,
    pub dirty: i64,
}

const ROW: &str =
    "SELECT id, path, fileid, etag, server_path, text, base, mtime, modified, dirty FROM notes";

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

pub fn by_path(db: &Connection, path: &str) -> Result<Option<Row>> {
    Ok(rows(db, "path = ?", [path])?.pop())
}

pub fn by_fileid(db: &Connection, fileid: &str) -> Result<Option<Row>> {
    Ok(rows(db, "fileid = ?", [fileid])?.pop())
}

pub fn open(path: &str) -> Result<Connection> {
    let db = Connection::open(path)?;
    db.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
    let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
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
