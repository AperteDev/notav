//! SQLite schema, migrations by `PRAGMA user_version`, and the key-value table.

use crate::Result;
use rusqlite::{Connection, OptionalExtension};

const MIGRATIONS: &[&str] = &[
    r#"
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
  path TEXT PRIMARY KEY,                -- relative, no trailing slash; '' is the notes folder
  etag TEXT,                            -- as last pulled; NULL: not pulled yet
  dirty INTEGER NOT NULL DEFAULT 0      -- 1: created here, not on the server yet; 2: deleted here
);
"#,
    // Folders moved here, in order, until the push moves them on the server (paths relative to
    // the notes folder).
    r#"
CREATE TABLE moves (id INTEGER PRIMARY KEY, src TEXT NOT NULL, dst TEXT NOT NULL);
"#,
    // Note ids never come back after a delete: undo (`undo.rs`) finds notes by them.
    r#"
CREATE TABLE notes_new (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  path TEXT NOT NULL UNIQUE,
  fileid TEXT UNIQUE,
  etag TEXT,
  server_path TEXT,
  text TEXT NOT NULL,
  base TEXT,
  mtime INTEGER,
  modified INTEGER NOT NULL,
  dirty INTEGER NOT NULL DEFAULT 0
);
INSERT INTO notes_new SELECT id, path, fileid, etag, server_path, text, base, mtime, modified, dirty
  FROM notes;
DROP TABLE notes;
ALTER TABLE notes_new RENAME TO notes;
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
"#,
    // Nextcloud's favorites (`oc:favorite`): starred here and not on the server yet while
    // `favorite_dirty`.
    r#"
ALTER TABLE notes ADD COLUMN favorite INTEGER NOT NULL DEFAULT 0;
ALTER TABLE notes ADD COLUMN favorite_dirty INTEGER NOT NULL DEFAULT 0;
"#,
    // Accounts (`account.rs`): the one account so far (kept in `kv`) becomes the first; notes,
    // folders and moves belong to one each. Its `config_` keys become per-account keys, the
    // notes folder (a synced setting) its own, and settings sync through it unless turned off.
    r#"
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
INSERT INTO accounts (id, kind, url, user, source, position)
  SELECT 1, 'nextcloud', u.value, n.value,
    coalesce((SELECT value FROM kv WHERE key = 'setting_goa_account'), ''), 1
  FROM kv u JOIN kv n ON n.key = 'account_user' WHERE u.key = 'account_url';
UPDATE kv SET key = 'a1_' || substr(key, 8)
  WHERE substr(key, 1, 7) = 'config_' AND EXISTS (SELECT 1 FROM accounts);
INSERT INTO kv (key, value)
  SELECT 'a1_notes_folder', value FROM kv
  WHERE key = 'setting_notes_folder' AND EXISTS (SELECT 1 FROM accounts);
INSERT INTO kv (key, value)
  SELECT 'setting_settings_account', '1' FROM accounts
  WHERE NOT EXISTS (SELECT 1 FROM kv WHERE key = 'setting_sync_settings' AND value = 'false');
DELETE FROM kv WHERE substr(key, 1, 7) = 'config_' OR key IN ('account_url', 'account_user',
  'setting_goa_account', 'settime_goa_account', 'setting_sync_settings', 'settime_sync_settings');

CREATE TABLE notes_new (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  account INTEGER NOT NULL,
  path TEXT NOT NULL,
  fileid TEXT,
  etag TEXT,
  server_path TEXT,
  text TEXT NOT NULL,
  base TEXT,
  mtime INTEGER,
  modified INTEGER NOT NULL,
  dirty INTEGER NOT NULL DEFAULT 0,
  favorite INTEGER NOT NULL DEFAULT 0,
  favorite_dirty INTEGER NOT NULL DEFAULT 0,
  UNIQUE (account, path),
  UNIQUE (account, fileid)              -- a file shared with two accounts has one oc:id
);
INSERT INTO notes_new SELECT id, 1, path, fileid, etag, server_path, text, base, mtime, modified,
  dirty, favorite, favorite_dirty FROM notes;
DROP TABLE notes;
ALTER TABLE notes_new RENAME TO notes;
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

CREATE TABLE folders_new (
  account INTEGER NOT NULL,
  path TEXT NOT NULL,
  etag TEXT,
  dirty INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (account, path)
);
INSERT INTO folders_new SELECT 1, path, etag, dirty FROM folders;
DROP TABLE folders;
ALTER TABLE folders_new RENAME TO folders;
ALTER TABLE moves ADD COLUMN account INTEGER NOT NULL DEFAULT 1;
"#,
];

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
    fn the_one_account_becomes_the_first() {
        let dir = std::env::temp_dir().join(format!("notav-test-migrate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("core.sqlite").to_string_lossy().to_string();
        {
            // Notav 0.1's database, signed in through GNOME Online Accounts.
            let db = Connection::open(&path).unwrap();
            for m in &MIGRATIONS[..4] {
                db.execute_batch(m).unwrap();
            }
            db.pragma_update(None, "user_version", 4).unwrap();
            db.execute_batch(
                "INSERT INTO kv VALUES ('account_url', 'https://cloud.example'),
                   ('account_user', 'kris'), ('setting_goa_account', 'account_1'),
                   ('setting_notes_folder', 'Docs/Notes'), ('config_files_home', '/h/'),
                   ('setting_tree_icons', 'all');
                 INSERT INTO notes (path, text, modified, fileid) VALUES ('a.md', 'x', 1, 'f1');
                 INSERT INTO folders (path, etag) VALUES ('Work', 'e');
                 INSERT INTO moves (src, dst) VALUES ('Work', 'Old');",
            )
            .unwrap();
        }
        let db = open(&path).unwrap();
        let account: (i64, String, String, String) = db
            .query_row("SELECT id, kind, user, source FROM accounts", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })
            .unwrap();
        assert_eq!(
            account,
            (1, "nextcloud".into(), "kris".into(), "account_1".into())
        );
        let kv = |k: &str| kv_get(&db, k).unwrap();
        assert_eq!(kv("a1_files_home").as_deref(), Some("/h/"));
        assert_eq!(kv("a1_notes_folder").as_deref(), Some("Docs/Notes"));
        assert_eq!(kv("setting_settings_account").as_deref(), Some("1"));
        assert_eq!(kv("setting_tree_icons").as_deref(), Some("all"));
        assert_eq!(kv("account_url"), None);
        let note = by_path(&db, 1, "a.md").unwrap().unwrap();
        assert_eq!((note.account, note.fileid.as_deref()), (1, Some("f1")));
        let owned = |t: &str| -> i64 {
            db.query_row(&format!("SELECT account FROM {t}"), [], |r| r.get(0))
                .unwrap()
        };
        assert_eq!((owned("folders"), owned("moves")), (1, 1));
    }
}
