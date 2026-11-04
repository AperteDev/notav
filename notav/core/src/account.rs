//! Accounts: where notes are kept. A Nextcloud account syncs its notes folder over WebDAV; the
//! local account (one at most) keeps its notes on this device only. Each note and folder belongs
//! to one (`notes.account`); a switched-off account's notes are hidden and not synced, nothing
//! is deleted.

use crate::dav::Dav;
use crate::{Client, Error, Result, store};
use rusqlite::{OptionalExtension, params};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AccountKind {
    Nextcloud,
    /// Notes on this device only, never synced.
    Local,
}

impl AccountKind {
    fn as_str(self) -> &'static str {
        match self {
            AccountKind::Nextcloud => "nextcloud",
            AccountKind::Local => "local",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    pub id: i64,
    pub kind: AccountKind,
    /// Given by the user; empty: the front-end names it ("user@host", "On This Device").
    pub name: String,
    /// The server, as signed in to.
    pub url: String,
    pub user: String,
    /// The front-end's note on where the password comes from: on Linux the GNOME Online
    /// Accounts id; empty when signed in in Notav, the password in its keyring.
    pub source: String,
    /// Off: its notes are hidden and not synced.
    pub enabled: bool,
}

const COLS: &str = "id, kind, name, url, user, source, enabled";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Account> {
    Ok(Account {
        id: r.get(0)?,
        kind: match r.get::<_, String>(1)?.as_str() {
            "local" => AccountKind::Local,
            _ => AccountKind::Nextcloud,
        },
        name: r.get(2)?,
        url: r.get(3)?,
        user: r.get(4)?,
        source: r.get(5)?,
        enabled: r.get(6)?,
    })
}

/// A per-account `kv` key.
pub(crate) fn key(account: i64, name: &str) -> String {
    format!("a{account}_{name}")
}

impl Client {
    /// Every account, in the user's order (`reorder_accounts`; the order added until then).
    pub fn accounts(&self) -> Vec<Account> {
        let db = self.db.lock().unwrap();
        let st = db.prepare(&format!(
            "SELECT {COLS} FROM accounts ORDER BY position, id"
        ));
        st.and_then(|mut st| st.query_map([], row)?.collect())
            .unwrap_or_default()
    }

    pub fn account(&self, id: i64) -> Option<Account> {
        let db = self.db.lock().unwrap();
        db.query_row(
            &format!("SELECT {COLS} FROM accounts WHERE id = ?"),
            [id],
            row,
        )
        .optional()
        .ok()?
    }

    /// A new account; returns its id. Refuses (`Error::Exists`) a Nextcloud account already
    /// here (same server and user) and a second local one.
    pub fn add_account(
        &self,
        kind: AccountKind,
        url: String,
        user: String,
        source: String,
    ) -> Result<i64> {
        let url = url.trim_end_matches('/').to_string();
        let db = self.db.lock().unwrap();
        let same: bool = db.query_row(
            "SELECT EXISTS (SELECT 1 FROM accounts WHERE kind = ?1
               AND (kind = 'local' OR (url = ?2 AND user = ?3)))",
            params![kind.as_str(), url, user],
            |r| r.get(0),
        )?;
        if same {
            return Err(Error::Exists(format!("{user}@{url}")));
        }
        db.execute(
            "INSERT INTO accounts (kind, url, user, source, position)
             VALUES (?1, ?2, ?3, ?4, (SELECT COALESCE(MAX(position), 0) + 1 FROM accounts))",
            params![kind.as_str(), url, user, source],
        )?;
        Ok(db.last_insert_rowid())
    }

    /// Put the accounts in this order (ids; ones left out keep their place after them).
    pub fn reorder_accounts(&self, ids: Vec<i64>) -> Result<()> {
        let db = self.db.lock().unwrap();
        let tx = db.unchecked_transaction()?;
        tx.execute(
            "UPDATE accounts SET position = position + ?1",
            [ids.len() as i64],
        )?;
        for (i, id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE accounts SET position = ?2 WHERE id = ?1",
                params![id, i as i64],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Name it; empty gives back the front-end's name.
    pub fn rename_account(&self, id: i64, name: String) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "UPDATE accounts SET name = ?2 WHERE id = ?1",
            params![id, name.trim()],
        )?;
        Ok(())
    }

    /// Switch an account on or off: off hides its notes and stops syncing them.
    pub fn set_account_enabled(&self, id: i64, enabled: bool) -> Result<()> {
        self.db.lock().unwrap().execute(
            "UPDATE accounts SET enabled = ?2 WHERE id = ?1",
            params![id, enabled],
        )?;
        Ok(())
    }

    /// The password is kept in memory only; front-ends keep it in the platform keyring (or
    /// get it from where `Account::source` says).
    pub fn connect(&self, id: i64, password: String) -> Result<()> {
        let a = self.account(id).ok_or(Error::NoAccount)?;
        let dav = Dav::new(&a.url, &a.user, &password);
        self.dav.lock().unwrap().insert(id, Arc::new(dav));
        Ok(())
    }

    /// Connected for this run: `connect` gave the password. An account stays without one,
    /// e.g. when the keyring no longer has it; its notes are there, nothing syncs.
    pub fn connected(&self, id: i64) -> bool {
        self.dav.lock().unwrap().contains_key(&id)
    }

    /// The server moved: same user and password at `url`. Its notes are fetched again from
    /// there (edits waiting for the old server are lost: sync first).
    pub fn move_account(&self, id: i64, url: String, password: String) -> Result<()> {
        self.drop_notes(id)?;
        self.db.lock().unwrap().execute(
            "UPDATE accounts SET url = ?2 WHERE id = ?1",
            params![id, url.trim_end_matches('/')],
        )?;
        self.connect(id, password)
    }

    /// Remove an account and its notes from this device; a local account's are gone for
    /// good (edits waiting for the server are lost: sync first).
    pub fn remove_account(&self, id: i64) -> Result<()> {
        self.drop_notes(id)?;
        self.db.lock().unwrap().execute(
            "DELETE FROM kv WHERE substr(key, 1, ?2) = ?1",
            params![key(id, ""), key(id, "").len() as i64],
        )?;
        self.db
            .lock()
            .unwrap()
            .execute("DELETE FROM accounts WHERE id = ?", [id])?;
        if self.settings_account() == Some(id) {
            self.set_settings_account(None)?;
        }
        self.dav.lock().unwrap().remove(&id);
        Ok(())
    }

    /// Forget the notes and folders of an account and what it learnt of its server; the notes
    /// folder chosen for it stays.
    fn drop_notes(&self, id: i64) -> Result<()> {
        {
            let db = self.db.lock().unwrap();
            let tx = db.unchecked_transaction()?;
            for table in ["notes", "folders", "moves"] {
                tx.execute(&format!("DELETE FROM {table} WHERE account = ?"), [id])?;
            }
            tx.execute(
                "DELETE FROM kv WHERE substr(key, 1, ?2) = ?1 AND key != ?3",
                params![
                    key(id, ""),
                    key(id, "").len() as i64,
                    key(id, "notes_folder")
                ],
            )?;
            tx.commit()?;
        }
        self.journal.lock().unwrap().clear();
        self.features_asked.lock().unwrap().remove(&id);
        Ok(())
    }

    /// The account whose server synced settings go through (the device-only
    /// `settings_account` setting); None: settings stay on this device.
    pub fn settings_account(&self) -> Option<i64> {
        self.setting("settings_account".into())?.parse().ok()
    }

    pub fn set_settings_account(&self, id: Option<i64>) -> Result<()> {
        let v = id.map(|i| i.to_string()).unwrap_or_default();
        self.set_setting("settings_account".into(), v)
    }

    /// Switched on and connected: an account that syncs.
    pub fn syncing(&self, id: i64) -> bool {
        self.account(id).is_some_and(|a| a.enabled) && self.connected(id)
    }

    /// The connection of account `id`.
    pub(crate) fn dav(&self, id: i64) -> Result<Arc<Dav>> {
        self.dav
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or(Error::NoAccount)
    }

    /// Ask account `id`'s server what it is (no sign-in needed): its version, maintenance mode.
    pub fn server_info(&self, id: i64) -> Result<crate::dav::ServerInfo> {
        self.dav(id)?.server_info()
    }

    /// The user's Nextcloud Files root on account `id`, `/remote.php/dav/files/<user id>/`. The
    /// user id can differ from the login name (an email address, for one), so it comes from the
    /// principal.
    pub fn files_home(&self, id: i64) -> Result<String> {
        let k = key(id, "files_home");
        if let Some(h) = store::kv_get(&self.db.lock().unwrap(), &k)? {
            return Ok(h);
        }
        let principal = self.dav(id)?.principal()?;
        let user = principal
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or_default();
        let home = format!("/remote.php/dav/files/{user}/");
        store::kv_set(&self.db.lock().unwrap(), &k, &home)?;
        Ok(home)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accounts_keep_their_own_notes() {
        let dir = std::env::temp_dir().join(format!("notav-test-accounts-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let c = Client::open(dir.to_string_lossy().into()).unwrap();
        let add =
            |url: &str| c.add_account(AccountKind::Nextcloud, url.into(), "kris".into(), "".into());
        let (a, b) = (
            add("https://one.example").unwrap(),
            add("https://two.example/").unwrap(),
        );
        assert!(matches!(add("https://one.example/"), Err(Error::Exists(_))));
        let here = c
            .add_account(AccountKind::Local, "".into(), "".into(), "".into())
            .unwrap();
        let second = c.add_account(AccountKind::Local, "".into(), "".into(), "".into());
        assert!(matches!(second, Err(Error::Exists(_))), "one local account");

        // The same path in two accounts: two notes.
        for id in [a, b, here] {
            c.add_folder(id, "Work".into()).unwrap();
            c.add_note(id, "Work".into(), "Plan".into(), id.to_string())
                .unwrap();
        }
        let mut notes: Vec<(i64, String)> =
            c.notes().into_iter().map(|n| (n.account, n.path)).collect();
        notes.sort();
        let plan = "Work/Plan.md".to_string();
        assert_eq!(
            notes,
            [(a, plan.clone()), (b, plan.clone()), (here, plan.clone())]
        );
        assert!(c.pending(Some(a)) && c.pending(Some(b)));
        assert!(!c.pending(Some(here)), "nothing for a server");

        // Switched off: hidden and not pending, nothing deleted.
        c.set_account_enabled(a, false).unwrap();
        assert_eq!(c.notes().len(), 2);
        assert!(c.pending(Some(a)), "still waits, for when it is on again");
        c.set_account_enabled(a, true).unwrap();
        assert_eq!(c.notes().len(), 3);

        // Removed: its notes go, the others stay.
        c.set_settings_account(Some(a)).unwrap();
        c.remove_account(a).unwrap();
        let left: Vec<i64> = c.notes().iter().map(|n| n.account).collect();
        assert_eq!(left, [b, here]);
        assert_eq!(c.folders(a), Vec::<String>::new());
        assert_eq!(c.settings_account(), None);

        // The user's order, new ones last.
        c.reorder_accounts(vec![here]).unwrap();
        let three = add("https://three.example").unwrap();
        let order: Vec<i64> = c.accounts().iter().map(|a| a.id).collect();
        assert_eq!(order, [here, b, three]);
    }
}
