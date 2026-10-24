//! Runs against the real server from the workspace `.env`: `cargo test -p notav-core --features integration`.
//! Each test owns a notes folder `it-notav/<name>/`, deleted first, and two "devices" on it.
//! Settings sync is off on them, so tests running at once don't change each other's folder.
#![cfg(feature = "integration")]

use notav_core::dav::Dav;
use notav_core::{Client, Note};

fn get(k: &str) -> Option<String> {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../.env"))
        .unwrap_or_default();
    std::env::var(k).ok().or_else(|| {
        text.lines()
            .find_map(|l| l.strip_prefix(&format!("{k}=")).map(str::to_string))
    })
}

fn env() -> (String, String, String) {
    (
        get("NOTAV_URL").expect("NOTAV_URL"),
        get("NOTAV_USER").expect("NOTAV_USER"),
        get("NOTAV_PASS").expect("NOTAV_PASS"),
    )
}

struct Fixture {
    /// "Another client", straight to the server.
    dav: Dav,
    /// The Files root and the notes folder's href, ending in "/".
    home: String,
    root: String,
    a: Client,
    b: Client,
}

fn client(test: &str, dev: &str, folder: &str) -> Client {
    let (url, user, pass) = env();
    let dir = std::env::temp_dir().join(format!("notav-it-{test}-{dev}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let c = Client::open(dir.to_string_lossy().into()).unwrap();
    c.set_account(url, user, pass).unwrap();
    c.set_setting("sync_settings".into(), "false".into())
        .unwrap();
    c.set_setting("notes_folder".into(), folder.into()).unwrap();
    c
}

impl Fixture {
    fn new(name: &str) -> Fixture {
        let (url, user, pass) = env();
        let dav = Dav::new(&url, &user, &pass);
        let folder = format!("it-notav/{name}");
        let a = client(name, "a", &folder);
        let home = a.files_home().unwrap();
        // Tests making it at once trip the server up (503, 409).
        static PARENT: std::sync::Once = std::sync::Once::new();
        PARENT.call_once(|| dav.mkcol_all(&home, &format!("{home}it-notav/")).unwrap());
        let root = format!("{home}{folder}/");
        dav.delete(&root, None).unwrap();
        let b = client(name, "b", &folder);
        Fixture {
            dav,
            home,
            root,
            a,
            b,
        }
    }

    fn put(&self, path: &str, text: &str) {
        let href = format!("{}{path}", self.root);
        let parent = &href[..href.rfind('/').unwrap() + 1];
        self.dav.mkcol_all(&self.home, parent).unwrap();
        let etag = self.dav.list(&href, "0").unwrap().pop().map(|e| e.etag);
        let mtime = notav_core::now();
        self.dav.put(&href, text, etag.as_deref(), mtime).unwrap();
    }

    fn server_text(&self, path: &str) -> Option<String> {
        let href = format!("{}{path}", self.root);
        self.dav.get(&href).unwrap().map(|(_, body)| body)
    }
}

fn find(c: &Client, path: &str) -> Note {
    c.notes()
        .into_iter()
        .find(|n| n.path == path)
        .unwrap_or_else(|| panic!("no note {path} in {:?}", c.notes()))
}

fn paths(c: &Client) -> Vec<String> {
    c.notes().into_iter().map(|n| n.path).collect()
}

#[test]
fn two_devices_round_trip() {
    let f = Fixture::new("round-trip");
    let n =
        f.a.add_note("Work".into(), "Plan".into(), "# Plan\n\nŻółw ✓".into())
            .unwrap();
    let r = f.a.sync().unwrap();
    assert_eq!(r.uploaded, 1);
    assert!(!f.a.pending());
    assert_eq!(f.server_text("Work/Plan.md").unwrap(), "# Plan\n\nŻółw ✓");

    f.b.sync().unwrap();
    let on_b = find(&f.b, "Work/Plan.md");
    assert_eq!(f.b.note_text(on_b.id).unwrap(), "# Plan\n\nŻółw ✓");
    assert_eq!(f.b.folders(), ["Work"]);

    f.b.set_text(on_b.id, "changed on b".into()).unwrap();
    f.b.sync().unwrap();
    // Written in the same second as a's upload, often: Nextcloud keeps the ETag then.
    let r = f.a.sync().unwrap();
    assert_eq!(r.downloaded, 1);
    assert_eq!(f.a.note_text(n.id).unwrap(), "changed on b");

    // A rename on a is a move on b: the same note, by oc:id.
    f.a.move_note(n.id, "Done".into(), "Plan 2026".into())
        .unwrap();
    f.a.sync().unwrap();
    assert!(f.server_text("Work/Plan.md").is_none());
    let r = f.b.sync().unwrap();
    assert_eq!(r.downloaded, 0, "a move downloads nothing");
    assert_eq!(paths(&f.b), ["Done/Plan 2026.md"]);
    assert_eq!(f.b.note(on_b.id).unwrap().title, "Plan 2026");

    f.b.delete_note(on_b.id).unwrap();
    f.b.sync().unwrap();
    assert!(f.server_text("Done/Plan 2026.md").is_none());
    let r = f.a.sync().unwrap();
    assert_eq!(r.removed, 1);
    assert!(f.a.notes().is_empty());
}

#[test]
fn changes_made_on_the_server() {
    let f = Fixture::new("server");
    f.put("Top.md", "top");
    f.put("Old.txt", "plain text");
    f.put("A/B/Deep.md", "deep");
    f.put("A/photo.png", "not a note");
    f.put(".obsidian/app.md", "settings");
    let r = f.a.sync().unwrap();
    assert_eq!(r.downloaded, 3);
    assert_eq!(paths(&f.a), ["A/B/Deep.md", "Old.txt", "Top.md"]);
    assert_eq!(f.a.folders(), ["A", "A/B"]);
    assert_eq!(f.a.note(find(&f.a, "Old.txt").id).unwrap().title, "Old");

    // Nothing changed: one PROPFIND, nothing downloaded.
    assert_eq!(f.a.sync().unwrap(), Default::default());

    // Edited, moved, a folder removed with its notes.
    let deep = find(&f.a, "A/B/Deep.md");
    f.put("Top.md", "top, edited");
    let from = format!("{}A/B/Deep.md", f.root);
    f.dav.mv(&from, &format!("{}Deep.md", f.root)).unwrap();
    f.put("Gone/Note.md", "soon gone");
    f.a.sync().unwrap();
    assert!(paths(&f.a).contains(&"Gone/Note.md".to_string()));
    f.dav.delete(&format!("{}Gone/", f.root), None).unwrap();
    f.a.sync().unwrap();
    assert_eq!(paths(&f.a), ["Deep.md", "Old.txt", "Top.md"]);
    assert_eq!(f.a.note(deep.id).unwrap().path, "Deep.md");
    assert_eq!(
        f.a.note_text(find(&f.a, "Top.md").id).unwrap(),
        "top, edited"
    );
    assert_eq!(f.a.folders(), ["A", "A/B"]);
}

#[test]
fn offline_edits_on_both_make_a_conflict_copy() {
    let f = Fixture::new("conflict");
    let n =
        f.a.add_note("".into(), "Shared".into(), "v1".into())
            .unwrap();
    f.a.sync().unwrap();
    f.b.sync().unwrap();
    let on_b = find(&f.b, "Shared.md");

    f.a.set_text(n.id, "edited on a".into()).unwrap();
    f.b.set_text(on_b.id, "edited on b".into()).unwrap();
    f.a.sync().unwrap();
    let r = f.b.sync().unwrap();
    assert_eq!(r.conflicts.len(), 1);
    let copy = &r.conflicts[0];
    assert!(copy.starts_with("Shared (conflict "), "{copy}");
    // The server's version keeps the name, b's text is in the copy; nothing is lost.
    assert_eq!(f.b.note_text(on_b.id).unwrap(), "edited on a");
    assert_eq!(f.b.note_text(find(&f.b, copy).id).unwrap(), "edited on b");
    assert_eq!(f.server_text(copy).unwrap(), "edited on b");
    assert!(!f.b.pending());

    f.a.sync().unwrap();
    assert_eq!(f.a.notes().len(), 2);

    // A note made on both with the same name: the same, as a conflict.
    f.a.add_note("".into(), "Idea".into(), "from a".into())
        .unwrap();
    f.b.add_note("".into(), "Idea".into(), "from b".into())
        .unwrap();
    f.a.sync().unwrap();
    let r = f.b.sync().unwrap();
    assert_eq!(r.conflicts.len(), 1);
    assert_eq!(f.b.note_text(find(&f.b, "Idea.md").id).unwrap(), "from a");
    assert_eq!(f.b.notes().len(), 4);
}

#[test]
fn edited_here_deleted_there_comes_back() {
    let f = Fixture::new("delete-edit");
    let n = f.a.add_note("".into(), "Keep".into(), "v1".into()).unwrap();
    f.a.sync().unwrap();
    f.b.sync().unwrap();
    f.b.delete_note(find(&f.b, "Keep.md").id).unwrap();
    f.b.sync().unwrap();
    f.a.set_text(n.id, "still needed".into()).unwrap();
    f.a.sync().unwrap();
    assert_eq!(f.server_text("Keep.md").unwrap(), "still needed");
    f.b.sync().unwrap();
    assert_eq!(paths(&f.b), ["Keep.md"]);
}

#[test]
fn notes_folder_moves_with_its_notes() {
    let f = Fixture::new("folder-from");
    let to = "it-notav/folder-to";
    f.dav
        .delete(&format!("{}{to}/", f.a.files_home().unwrap()), None)
        .unwrap();
    let n =
        f.a.add_note("Sub".into(), "Moved".into(), "text".into())
            .unwrap();
    f.a.sync().unwrap();
    f.a.set_notes_folder(to.into(), true).unwrap();
    assert_eq!(f.a.notes_folder(), to);
    let r = f.a.sync().unwrap();
    assert_eq!((r.downloaded, r.removed), (0, 0), "{r:?}");
    assert_eq!(f.a.note(n.id).unwrap().path, "Sub/Moved.md");
    assert!(f.server_text("Sub/Moved.md").is_none());

    // b had the old folder: it follows (as the synced setting would make it).
    f.b.sync().unwrap();
    f.b.set_setting("notes_folder".into(), to.into()).unwrap();
    f.b.sync().unwrap();
    assert_eq!(paths(&f.b), ["Sub/Moved.md"]);

    // Without moving: the other folder's notes only.
    f.a.set_notes_folder("it-notav/folder-from".into(), false)
        .unwrap();
    f.a.sync().unwrap();
    assert!(f.a.notes().is_empty());
}

#[test]
fn a_note_locked_on_the_server_waits_and_the_rest_syncs() {
    let f = Fixture::new("locked");
    let locked = f.a.add_note("".into(), "Open".into(), "v1".into()).unwrap();
    let other =
        f.a.add_note("".into(), "Other".into(), "v1".into())
            .unwrap();
    f.a.sync().unwrap();
    // As the Nextcloud Text editor does while it has the note open.
    let href = format!("{}Open.md", f.root);
    let body = r#"<?xml version="1.0"?><d:lockinfo xmlns:d="DAV:"><d:lockscope><d:exclusive/></d:lockscope><d:locktype><d:write/></d:locktype><d:owner>it</d:owner></d:lockinfo>"#;
    let r = f
        .dav
        .request("LOCK", &href, &[("Timeout", "Second-120")], body)
        .unwrap();
    assert_eq!(r.status, 200, "{}", r.body);
    let token = r
        .body
        .split_once("<d:href>opaquelocktoken:")
        .and_then(|(_, t)| t.split_once('<'))
        .map(|(t, _)| format!("<opaquelocktoken:{t}>"))
        .expect("lock token");

    f.a.move_note(locked.id, "Work".into(), "Open".into())
        .unwrap();
    f.a.set_text(other.id, "v2".into()).unwrap();
    let r = f.a.sync().unwrap();
    assert_eq!(r.locked, ["Work/Open.md"]);
    assert_eq!(r.uploaded, 1);
    assert_eq!(f.server_text("Other.md").unwrap(), "v2");
    assert!(f.a.pending());
    assert!(f.server_text("Open.md").is_some());

    // The server now and then answers 500 while other tests run: try again.
    let unlocked = (0..5).any(|_| {
        let r = f
            .dav
            .request("UNLOCK", &href, &[("Lock-Token", &token)], "");
        let ok = r.is_ok_and(|r| r.status < 300);
        if !ok {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        ok
    });
    assert!(unlocked, "UNLOCK failed");
    let r = f.a.sync().unwrap();
    assert!(r.locked.is_empty());
    assert!(!f.a.pending());
    assert_eq!(f.server_text("Work/Open.md").unwrap(), "v1");
}

#[test]
fn a_folder_moves_whole_on_the_server() {
    let f = Fixture::new("folder-move");
    let plan =
        f.a.add_note("Work".into(), "Plan".into(), "p".into())
            .unwrap();
    f.a.add_note("Work/Old".into(), "Deep".into(), "d".into())
        .unwrap();
    f.a.add_folder("Work/Empty".into()).unwrap();
    f.a.add_folder("Archive".into()).unwrap();
    f.a.sync().unwrap();
    f.b.sync().unwrap();

    // Edited, then moved, before the next sync: the edit goes up at the new place.
    f.a.set_text(plan.id, "p2".into()).unwrap();
    let to = f.a.move_folder("Work".into(), "Archive".into()).unwrap();
    assert_eq!(to, "Archive/Work");
    assert_eq!(f.a.note(plan.id).unwrap().path, "Archive/Work/Plan.md");
    let r = f.a.sync().unwrap();
    assert!(r.conflicts.is_empty(), "{r:?}");
    assert!(!f.a.pending());
    assert_eq!(f.server_text("Archive/Work/Plan.md").unwrap(), "p2");
    assert_eq!(f.server_text("Archive/Work/Old/Deep.md").unwrap(), "d");
    assert!(f.server_text("Work/Plan.md").is_none());
    let empty = format!("{}Archive/Work/Empty/", f.root);
    assert!(
        !f.dav.list(&empty, "0").unwrap().is_empty(),
        "empty folder moved too"
    );

    // The other device: moves, one changed text.
    let r = f.b.sync().unwrap();
    assert_eq!((r.downloaded, r.removed), (1, 0), "{r:?}");
    assert_eq!(
        paths(&f.b),
        ["Archive/Work/Old/Deep.md", "Archive/Work/Plan.md"]
    );
    assert!(f.b.folders().contains(&"Archive/Work/Empty".to_string()));
}

#[test]
fn a_folder_moved_onto_a_name_taken_on_the_server_moves_note_by_note() {
    let f = Fixture::new("folder-merge");
    f.a.add_note("Work".into(), "Plan".into(), "p".into())
        .unwrap();
    f.a.add_folder("Archive".into()).unwrap();
    f.a.sync().unwrap();
    // Made elsewhere, not pulled here yet.
    f.put("Archive/Work/Other.md", "o");

    f.a.move_folder("Work".into(), "Archive".into()).unwrap();
    let r = f.a.sync().unwrap();
    assert!(r.conflicts.is_empty(), "{r:?}");
    assert!(!f.a.pending());
    assert_eq!(f.server_text("Archive/Work/Plan.md").unwrap(), "p");
    assert!(f.server_text("Work/Plan.md").is_none());
    assert_eq!(
        paths(&f.a),
        ["Archive/Work/Other.md", "Archive/Work/Plan.md"]
    );
}
