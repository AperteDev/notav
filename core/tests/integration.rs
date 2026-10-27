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

/// A setup step, tried again when the server fails under the tests' parallel load (503 with a
/// database error, now and then).
fn retry<T>(f: impl Fn() -> notav_core::Result<T>) -> T {
    for _ in 0..4 {
        match f() {
            Ok(v) => return v,
            Err(notav_core::Error::Http(503, _)) => {
                std::thread::sleep(std::time::Duration::from_millis(500))
            }
            Err(e) => panic!("{e}"),
        }
    }
    f().unwrap()
}

impl Fixture {
    fn new(name: &str) -> Fixture {
        let (url, user, pass) = env();
        let dav = Dav::new(&url, &user, &pass);
        let folder = format!("it-notav/{name}");
        let a = client(name, "a", &folder);
        let home = retry(|| a.files_home());
        // Tests making it at once trip the server up (503, 409).
        static PARENT: std::sync::Once = std::sync::Once::new();
        PARENT.call_once(|| retry(|| dav.mkcol_all(&home, &format!("{home}it-notav/"))));
        let root = format!("{home}{folder}/");
        retry(|| dav.delete(&root, None));
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
    // An app's lock, as the Nextcloud Text editor takes while it has the note open.
    let href = format!("{}Open.md", f.root);
    let r = f
        .dav
        .request(
            "LOCK",
            &href,
            &[("X-User-Lock", "1"), ("X-User-Lock-Type", "1")],
            "",
        )
        .unwrap();
    assert_eq!(r.status, 200, "{}", r.body);

    f.a.move_note(locked.id, "Work".into(), "Open".into())
        .unwrap();
    f.a.set_text(other.id, "v2".into()).unwrap();
    let r = f.a.sync().unwrap();
    assert_eq!(r.locked.len(), 1, "{r:?}");
    let l = &r.locked[0];
    assert_eq!(
        (l.path.as_str(), l.file.as_deref()),
        ("Work/Open.md", Some("Open.md"))
    );
    assert!(l.owner.is_some() && l.since.is_some_and(|t| t > 0), "{l:?}");
    assert_eq!(r.uploaded, 1);
    assert_eq!(f.server_text("Other.md").unwrap(), "v2");
    assert!(f.a.pending());
    assert!(f.server_text("Open.md").is_some());

    // Unlocked from here: the move goes up.
    f.a.unlock(l.file.clone().unwrap()).unwrap();
    let r = f.a.sync().unwrap();
    assert!(r.locked.is_empty(), "{r:?}");
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

#[test]
fn a_new_note_takes_the_name_of_one_moved_away() {
    let f = Fixture::new("name-freed");
    // As the app adds them: "Untitled", empty.
    let first =
        f.a.add_note("".into(), "Untitled".into(), "".into())
            .unwrap();
    f.a.sync().unwrap();
    f.b.sync().unwrap();

    // Moved into a folder sorted after its old name, then a new note takes that name: the
    // move goes up first, or the new note finds the old one's file there.
    f.a.move_note(first.id, "work".into(), "Moved".into())
        .unwrap();
    let second =
        f.a.add_note("".into(), "Untitled".into(), "".into())
            .unwrap();
    assert_eq!(second.path, "Untitled.md");
    let r = f.a.sync().unwrap();
    assert!(r.conflicts.is_empty(), "{r:?}");
    assert!(!f.a.pending());
    assert_eq!(paths(&f.a), ["Untitled.md", "work/Moved.md"]);
    assert_eq!(f.server_text("work/Moved.md").unwrap(), "");
    assert_eq!(f.server_text("Untitled.md").unwrap(), "");

    // The other device sees the move (the same note) and the new one.
    let r = f.b.sync().unwrap();
    assert_eq!((r.downloaded, r.removed), (1, 0), "{r:?} {:?}", f.b.notes());
    assert_eq!(paths(&f.b), ["Untitled.md", "work/Moved.md"]);
}

#[test]
fn a_new_note_on_the_name_of_one_changed_here_and_moved_there_waits() {
    let f = Fixture::new("name-held");
    let old = f.a.add_note("".into(), "Old".into(), "v1".into()).unwrap();
    f.a.sync().unwrap();
    // Renamed by another client, not pulled here yet; edited here meanwhile, and a new note
    // takes the name it has there now.
    f.dav
        .mv(&format!("{}Old.md", f.root), &format!("{}New.md", f.root))
        .unwrap();
    f.a.set_text(old.id, "v2".into()).unwrap();
    f.a.add_note("".into(), "New".into(), "new".into()).unwrap();

    // The file there is the edited note's, not the new note's to take: no text is lost.
    for _ in 0..3 {
        f.a.sync().unwrap();
    }
    assert!(!f.a.pending(), "{:?}", f.a.notes());
    let texts: Vec<String> =
        f.a.notes()
            .iter()
            .map(|n| f.a.note_text(n.id).unwrap())
            .collect();
    for t in ["v2", "new"] {
        assert!(texts.iter().any(|x| x == t), "{t} missing: {texts:?}");
    }
}

#[test]
fn a_folder_renamed_then_deleted_on_the_other_device() {
    let f = Fixture::new("folder-rename-delete");
    f.a.add_note("Work".into(), "Plan".into(), "p".into())
        .unwrap();
    f.a.add_note("Work/Sub".into(), "Deep".into(), "d".into())
        .unwrap();
    f.a.sync().unwrap();
    f.b.sync().unwrap();

    assert_eq!(
        f.a.rename_folder("Work".into(), "Projects".into()).unwrap(),
        "Projects"
    );
    f.a.sync().unwrap();
    assert!(!f.a.pending());
    assert_eq!(f.server_text("Projects/Sub/Deep.md").unwrap(), "d");
    assert!(f.server_text("Work/Plan.md").is_none());
    let r = f.b.sync().unwrap();
    assert_eq!(r.downloaded, 0, "a rename downloads nothing: {r:?}");
    assert_eq!(paths(&f.b), ["Projects/Plan.md", "Projects/Sub/Deep.md"]);

    f.b.delete_folder("Projects".into()).unwrap();
    assert!(f.b.notes().is_empty() && f.b.folders().is_empty());
    f.b.sync().unwrap();
    assert!(!f.b.pending());
    let gone = format!("{}Projects/", f.root);
    assert!(f.dav.list(&gone, "0").unwrap().is_empty(), "folder deleted");
    let r = f.a.sync().unwrap();
    assert_eq!(r.removed, 2, "{r:?}");
    assert!(f.a.notes().is_empty() && f.a.folders().is_empty());
}

#[test]
fn a_deleted_folder_keeps_what_changed_on_the_server() {
    let f = Fixture::new("folder-delete-keep");
    f.a.add_note("Work".into(), "Plan".into(), "p".into())
        .unwrap();
    f.a.add_note("Work".into(), "Other".into(), "o".into())
        .unwrap();
    f.a.add_note("Inbox".into(), "Old".into(), "o".into())
        .unwrap();
    f.a.sync().unwrap();
    // Changed and added by another client, not pulled here yet; a second later, as Nextcloud's
    // ETags are times in whole seconds.
    std::thread::sleep(std::time::Duration::from_secs(1));
    f.put("Work/Plan.md", "p2");
    f.put("Inbox/New.md", "n");

    f.a.delete_folder("Work".into()).unwrap();
    f.a.delete_folder("Inbox".into()).unwrap();
    f.a.sync().unwrap();
    assert!(!f.a.pending());
    assert!(f.server_text("Work/Other.md").is_none());
    assert!(f.server_text("Inbox/Old.md").is_none());
    assert_eq!(paths(&f.a), ["Inbox/New.md", "Work/Plan.md"]);
    assert_eq!(f.a.note_text(find(&f.a, "Work/Plan.md").id).unwrap(), "p2");
    assert_eq!(f.a.folders(), ["Inbox", "Work"]);
}

#[test]
fn undo_before_and_after_the_server_has_the_change() {
    let f = Fixture::new("undo");
    let plan =
        f.a.add_note("Work".into(), "Plan".into(), "p".into())
            .unwrap();
    let idea = f.a.add_note("".into(), "Idea".into(), "i".into()).unwrap();
    f.a.sync().unwrap();

    // Undone before a sync: nothing to do on the server.
    f.a.move_note(idea.id, "Work".into(), "Idea".into())
        .unwrap();
    f.a.delete_folder("Work".into()).unwrap();
    assert!(f.a.undo(None).unwrap() && f.a.undo(None).unwrap());
    let r = f.a.sync().unwrap();
    assert_eq!((r.uploaded, r.removed), (0, 0), "{r:?}");
    assert!(!f.a.pending());
    assert_eq!(f.server_text("Idea.md").unwrap(), "i");

    // Undone after: the deleted folder and note come back as new ones there.
    f.a.delete_folder("Work".into()).unwrap();
    f.a.sync().unwrap();
    assert!(f.server_text("Work/Plan.md").is_none());
    assert!(f.a.undo(None).unwrap());
    f.a.sync().unwrap();
    assert!(!f.a.pending());
    assert_eq!(f.server_text("Work/Plan.md").unwrap(), "p");
    assert_eq!(f.a.note(plan.id).unwrap().path, "Work/Plan.md");
    f.b.sync().unwrap();
    assert_eq!(paths(&f.b), ["Idea.md", "Work/Plan.md"]);
}
