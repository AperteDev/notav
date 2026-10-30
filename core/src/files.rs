//! What Nextcloud Files keeps besides the notes themselves: favorites, the trash bin and file
//! versions. The last two are apps an admin can turn off (`files_trashbin`, `files_versions`)
//! and set up (how long they keep things, whether a user may delete from the trash): the
//! server's capabilities say what there is (`Client::features`).

use crate::dav::{DAV, NC, http_date};
use crate::json::Json;
use crate::notes::{is_note, split};
use crate::store::{self, DELETED};
use crate::{Client, Error, Result};
use rusqlite::params;
use std::collections::HashSet;
use std::sync::atomic::Ordering;

/// What the server offers besides files, as its capabilities say.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Features {
    /// The trash bin (`files.undelete`): deleted notes can be restored.
    pub trash: bool,
    /// Deleting from it for good (`files.delete_from_trash`).
    pub purge: bool,
    /// File versions (`files.versioning`).
    pub versions: bool,
}

/// A note or folder in Nextcloud's trash bin, deleted from the notes folder.
#[derive(Debug, Clone, PartialEq)]
pub struct Deleted {
    /// Its href in the trash bin, for `restore` and `purge`.
    pub id: String,
    /// Where it was, relative to the notes folder.
    pub path: String,
    pub folder: bool,
    /// Unix time.
    pub deleted: i64,
}

/// A version of a note kept by Nextcloud.
#[derive(Debug, Clone, PartialEq)]
pub struct Version {
    /// Its href, for `version_text`.
    pub id: String,
    /// Unix time it was saved.
    pub modified: i64,
    pub size: Option<u64>,
    /// Who saved it, and the name given to it in Files.
    pub author: Option<String>,
    pub label: Option<String>,
    /// The file as it is on the server now (its text is the note's; it cannot be downloaded).
    pub current: bool,
}

impl Client {
    /// What the server offers, as last asked (`refresh_features`, by the sync once a run).
    pub fn features(&self) -> Features {
        let db = self.db.lock().unwrap();
        let has = |k: &str| store::kv_get(&db, k).ok().flatten().as_deref() == Some("1");
        Features {
            trash: has("config_trash"),
            purge: has("config_trash_purge"),
            versions: has("config_versions"),
        }
    }

    /// Ask the server what it offers; kept for `features`.
    pub fn refresh_features(&self) -> Result<Features> {
        let caps = self.capabilities()?;
        let on = |k: &str| {
            matches!(
                caps.get("files").and_then(|f| f.get(k)),
                Some(Json::Bool(true))
            )
        };
        let f = Features {
            trash: on("undelete"),
            purge: on("undelete") && on("delete_from_trash"),
            versions: on("versioning"),
        };
        let db = self.db.lock().unwrap();
        for (k, v) in [
            ("config_trash", f.trash),
            ("config_trash_purge", f.purge),
            ("config_versions", f.versions),
        ] {
            store::kv_set(&db, k, if v { "1" } else { "0" })?;
        }
        self.features_asked.store(true, Ordering::SeqCst);
        Ok(f)
    }

    /// The server's capabilities (`ocs/v2.php/cloud/capabilities`).
    pub(crate) fn capabilities(&self) -> Result<Json> {
        let url = self.account().ok_or(Error::NoAccount)?.url;
        let r = self.dav()?.request(
            "GET",
            &format!(
                "{}/ocs/v2.php/cloud/capabilities?format=json",
                url.trim_end_matches('/')
            ),
            &[("OCS-APIRequest", "true")],
            "",
        )?;
        let json = Json::parse(&r.body).ok_or_else(|| Error::Parse("capabilities".into()))?;
        json.get("ocs")
            .and_then(|o| o.get("data"))
            .and_then(|d| d.get("capabilities"))
            .cloned()
            .ok_or(Error::Http(r.status, r.body))
    }

    /// Star or unstar note `id`; the server gets it with the next sync.
    pub fn set_favorite(&self, id: i64, on: bool) -> Result<()> {
        let path = {
            let db = self.db.lock().unwrap();
            let r = store::by_id(&db, id)?
                .filter(|r| r.dirty != DELETED)
                .ok_or_else(|| Error::NotFound(id.to_string()))?;
            if r.favorite == on {
                return Ok(());
            }
            db.execute(
                "UPDATE notes SET favorite = ?, favorite_dirty = 1 WHERE id = ?",
                params![on, id],
            )?;
            r.path
        };
        self.emit(vec![path]);
        Ok(())
    }

    /// Star or unstar on the server the notes starred or unstarred here, once they are there.
    pub(crate) fn push_favorites(&self, root: &str) -> Result<()> {
        let dav = self.dav()?;
        let rows = {
            let db = self.db.lock().unwrap();
            store::rows(
                &db,
                "favorite_dirty = 1 AND fileid IS NOT NULL AND server_path IS NULL AND dirty != ?",
                [DELETED],
            )?
        };
        for r in rows {
            match dav.set_favorite(&format!("{root}{}", r.path), r.favorite) {
                // Gone there: the pull sorts the note out.
                Ok(()) | Err(Error::NotFound(_)) => {
                    self.db.lock().unwrap().execute(
                        "UPDATE notes SET favorite_dirty = 0 WHERE id = ? AND favorite = ?",
                        params![r.id, r.favorite],
                    )?;
                }
                Err(Error::Locked(_)) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Take the server's favorites, for the notes not starred or unstarred here meanwhile: paths
    /// of the notes that changed.
    pub(crate) fn pull_favorites(&self) -> Result<Vec<String>> {
        let starred: HashSet<String> = self
            .dav()?
            .favorites(&self.files_home()?)?
            .into_iter()
            .collect();
        let db = self.db.lock().unwrap();
        let mut changed = Vec::new();
        for r in store::rows(&db, "fileid IS NOT NULL AND favorite_dirty = 0", [])? {
            let on = starred.contains(r.fileid.as_deref().unwrap_or_default());
            if on != r.favorite {
                db.execute(
                    "UPDATE notes SET favorite = ? WHERE id = ? AND favorite_dirty = 0",
                    params![on, r.id],
                )?;
                changed.push(r.path);
            }
        }
        Ok(changed)
    }

    /// The user's DAV home of `kind` ("trashbin", "versions"), as `files_home` is "files"'s.
    fn dav_home(&self, kind: &str) -> Result<String> {
        Ok(self
            .files_home()?
            .replacen("/files/", &format!("/{kind}/"), 1))
    }

    /// Notes and folders deleted from the notes folder that Nextcloud's trash bin keeps, the
    /// latest first. Nextcloud empties it by itself after a time its admin sets.
    pub fn trash(&self) -> Result<Vec<Deleted>> {
        let base = format!("{}trash/", self.dav_home("trashbin")?);
        let root = format!("{}/", self.notes_folder());
        let props = "<nc:trashbin-original-location/><nc:trashbin-deletion-time/><d:resourcetype/>";
        let mut out: Vec<Deleted> = self
            .dav()?
            .propfind(&base, "1", props)?
            .into_iter()
            .filter(|r| {
                r.status == 200 && r.href.trim_end_matches('/') != base.trim_end_matches('/')
            })
            .filter_map(|r| {
                let location = r.props.find_text(NC, "trashbin-original-location")?;
                let path = location
                    .strip_prefix(&root)?
                    .trim_end_matches('/')
                    .to_string();
                let folder = r
                    .props
                    .child(DAV, "resourcetype")
                    .is_some_and(|t| t.child(DAV, "collection").is_some());
                let hidden = path.split('/').any(|s| s.starts_with('.'));
                (!hidden && (folder || is_note(split(&path).1))).then(|| Deleted {
                    id: r.href.clone(),
                    path,
                    folder,
                    deleted: r
                        .props
                        .find_text(NC, "trashbin-deletion-time")
                        .and_then(|t| t.parse().ok())
                        .unwrap_or(0),
                })
            })
            .collect();
        out.sort_by(|a, b| b.deleted.cmp(&a.deleted).then(a.path.cmp(&b.path)));
        Ok(out)
    }

    /// Put a deleted note or folder back where it was (Nextcloud names it "… (restored)" when
    /// that name is taken); the next sync brings it here.
    pub fn restore_deleted(&self, id: String) -> Result<()> {
        let name = id
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or_default();
        let dav = self.dav()?;
        let to = dav.url(&format!("{}restore/{name}", self.dav_home("trashbin")?));
        // Not `Dav::mv`: the restore folder answers its `Overwrite: F` with 412.
        let r = dav.request("MOVE", &id, &[("Destination", &to)], "")?;
        match r.status {
            200..=299 => Ok(()),
            404 => Err(Error::NotFound(id)),
            s => Err(Error::Http(s, r.body)),
        }
    }

    /// Delete a note or folder from the trash bin for good.
    pub fn purge(&self, id: String) -> Result<()> {
        self.dav()?.delete(&id, None)
    }

    /// The versions of note `id` Nextcloud keeps, the latest (the current one) first; none for
    /// a note not on the server yet.
    pub fn versions(&self, id: i64) -> Result<Vec<Version>> {
        let fileid = {
            let db = self.db.lock().unwrap();
            store::by_id(&db, id)?
                .ok_or_else(|| Error::NotFound(id.to_string()))?
                .fileid
        };
        // `oc:id` is the numeric file id, zero-padded to 8 digits, and the instance's id.
        let Some(number) = fileid
            .as_deref()
            .map(|f| f.trim_start_matches('0'))
            .map(|f| &f[..f.find(|c: char| !c.is_ascii_digit()).unwrap_or(f.len())])
            .filter(|n| !n.is_empty())
        else {
            return Ok(Vec::new());
        };
        let base = format!("{}versions/{number}/", self.dav_home("versions")?);
        let props =
            "<d:getlastmodified/><d:getcontentlength/><nc:version-author/><nc:version-label/>";
        let mut out: Vec<Version> = match self.dav()?.propfind(&base, "1", props) {
            Err(Error::Http(404, _)) => return Ok(Vec::new()),
            r => r?,
        }
        .into_iter()
        .filter(|r| r.status == 200 && r.href.trim_end_matches('/') != base.trim_end_matches('/'))
        .map(|r| Version {
            // Named after the time it was saved.
            modified: r
                .href
                .rsplit('/')
                .next()
                .and_then(|t| t.parse().ok())
                .or_else(|| {
                    r.props
                        .find_text(DAV, "getlastmodified")
                        .and_then(|d| http_date(&d))
                })
                .unwrap_or(0),
            size: r
                .props
                .find_text(DAV, "getcontentlength")
                .and_then(|s| s.parse().ok()),
            author: r.props.find_text(NC, "version-author"),
            label: r.props.find_text(NC, "version-label"),
            current: false,
            id: r.href,
        })
        .collect();
        out.sort_by_key(|v| std::cmp::Reverse(v.modified));
        if let Some(v) = out.first_mut() {
            v.current = true;
        }
        Ok(out)
    }

    /// The text of a version (not the current one's: that is the note's).
    pub fn version_text(&self, id: String) -> Result<String> {
        self.dav()?
            .get(&id)?
            .map(|(_, body)| body)
            .ok_or(Error::NotFound(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_star_waits_for_the_server() {
        let dir = std::env::temp_dir().join(format!("notav-test-fav-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let c = Client::open(dir.to_string_lossy().into()).unwrap();
        let n = c
            .add_note(String::new(), "Plan".into(), String::new())
            .unwrap();
        c.db.lock()
            .unwrap()
            .execute(
                "UPDATE notes SET fileid = '00000042ocabc', dirty = 0 WHERE id = ?",
                [n.id],
            )
            .unwrap();
        assert!(!c.pending());
        c.set_favorite(n.id, true).unwrap();
        assert!(c.note(n.id).unwrap().favorite);
        assert!(c.pending(), "the star waits for the server");
        let db = c.db.lock().unwrap();
        let waiting: i64 = db
            .query_row(
                "SELECT favorite_dirty FROM notes WHERE id = ?",
                [n.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(waiting, 1);
    }
}
