# Notav: Architecture and Plan

Oct 24, 2026, @Kris, updated Oct 25, 2026

## Overview

Notav (*notatka* + DAV) is a personal, local-first notes app with an Obsidian-style journal, kept as plain Markdown files in Nextcloud Files over WebDAV. It is the sibling of [Todav](https://github.com/AperteDev/todav) (tasks and calendar over CalDAV) and follows its design: one Rust core with all storage, sync and parsing logic, thin front-ends, the same Nextcloud and GNOME Online Accounts integration.

In progress: the core syncs notes, the Linux app edits them; the journal and links are next (see [Plan](#plan)).

**Goals**

- Plain Markdown files: the Nextcloud Notes app and Obsidian (through the Nextcloud desktop sync client) open the same folder. No proprietary format, no proprietary storage.
- Local-first: every UI reads and writes a local copy; the network is never on the UI path.
- Same notes and behaviour on every device; changes from other devices visible within seconds where the server can signal them, by polling otherwise.
- Minimal dependencies: standard library and platform SDKs first; a third-party crate or library only where it replaces real work.

**Principles for the agent** (as in Todav)

- The Rust core is the only place with sync, push, parsing and conflict logic. Front-ends are thin: they render state and call commands.
- Every feature lands in the core with tests against a real Nextcloud (the Test account, as in Todav) before any UI uses it.
- Push is best-effort; polling stays as the safety net everywhere.
- When in doubt, do the simpler thing and leave a `TODO(v2)` comment rather than a configuration option.

## Decisions

- Name Notav, gettext domain `notav`, its own repository (decided 2026-10-24). App ID `dev.aperte.Notav`, on the domain of Aperte, the developer, as Todav's `dev.aperte.Todav` (decided 2026-10-25, before the first release; it was `io.github.selfref.Notav`).
- **Separate from Todav in configuration and credentials**: its own sign-in (its own app password, a GNOME Online Accounts account picked on its own), keyring entries under its own app ID, its own synced settings file (`.config/notav/settings.json`) and its own database. Neither app reads or writes the other's.
- Storage: Markdown files in Nextcloud Files over WebDAV; see [Storage](#storage) (decided 2026-10-24).
- Journal: one daily note per day in a dated folder (layout and format not decided yet); `[[wiki links]]`, backlinks and tags are computed locally from the files.
- Cross-app, with no shared database: a note links a task as `todav://task/<uid>`; Todav opens it (it handles the `todav` URI scheme), and Notav shows the task's title and state through Todav's D-Bus API (`GetTask`, to be added in Todav). Without Todav the link stays a plain link.
- No background service in v1: no autostart and no `HOLD`; notes have no reminders, so the app syncs while it is open (decided 2026-10-24).
- Editor: GtkSourceView 5 with its Markdown highlighting; a rendered preview is v2 (decided 2026-10-24). Styled for prose rather than code, as Iotas, Apostrophe and Folio do (decided 2026-10-25): `linux/app/sourceview/` (bundled in the binary's resources) has `notav-markdown`, GtkSourceView's Markdown with a style per ATX heading level and one for the `#` marks, and the `notav`/`notav-dark` schemes over Adwaita's: headings bold and scaled by level (1.5 to 1.0, below the title's 20 pt), list, quote and heading marks dimmed, link text blue instead of red. The text is 12 pt with `line-height: 1.4` (Iotas; GNOME Text Editor uses 1.2) in a column of at most 720 px, about 80 characters (Folio 720 px, Iotas 800 px, Apostrophe 66 characters). Not taken (yet): a monospace font (Apostrophe's Fira Mono, Iotas's default), a font size that grows with the window (Apostrophe), the `#` marks hanging in the margin (Iotas), font and width preferences.
- Tooling and releases as in Todav: CI on every push (fmt, clippy with warnings as errors, unit tests, packages), the manual Release workflow, Keep a Changelog, the lowest supported GNOME 49. Set up with the first code, from Todav's workflows and scripts.

## Code from Todav

Copied, not shared: Notav starts from copies of Todav's code that is not about tasks or calendars (as of Todav 1.2.0), and the two copies may diverge.

| Piece | Todav source | Notes |
| --- | --- | --- |
| Login Flow v2, app password sign-in | `todav/core/src/login.rs` | As is |
| Minimal JSON | `todav/core/src/json.rs` | As is |
| WebDAV XML tree, multistatus, percent-encoding, HTTP requests with retries | `todav/core/src/caldav.rs` (first part) | In `core/src/dav.rs`; the CalDAV requests stay in Todav |
| Synced settings | `todav/core/src/settings.rs` | Its own `DEVICE_ONLY`; no ntfy keys; the file's href from `Client::files_home` |
| `Error`, `b64encode`, `Client::open`/`set_account`/`logout`, store migrations and `kv` | `todav/core/src/lib.rs`, `store.rs` | The task and calendar parts left out |
| CLI `.env` loading | `todav/cli/src/main.rs` | `NOTAV_*` variables |
| GNOME Online Accounts | `todav/linux/app/src/goa.rs` | As is |
| gettext over glibc, first weekday | `todav/linux/app/src/i18n.rs` | Domain `notav` |
| Secret Service | `todav/linux/app/src/main.rs` (`schema` and helpers) | Schema under Notav's app ID |
| Welcome page, login dialogs | `todav/linux/app/src/connection.rs`, `ui/connection.blp`, `ui/password-login.blp`, parts of `window.rs` | Adapted; GOA accounts need Files on, their Files URI gives the server |
| Account handling, sync worker | `todav/linux/app/src/main.rs` | No push, reminders, D-Bus or local mode |
| Preferences: style, account | `todav/linux/app/src/settings.rs`, `ui/settings.blp` | The account group as is |
| Build and packaging | `build.rs`, `todav/linux/Makefile`, Flatpak manifest, `scripts/`, `.github/workflows/`, `po/its/` | No GNOME extension; GtkSourceView added |
| Release notes in the metainfo, screenshots from the release tag, the RPM spec's version | `scripts/release.py` (as of Todav 1.2.2) | The spec's first entry is for the first release |
| COPR spec and source RPM | `todav/linux/rpm/todav.spec`, `.copr/Makefile` | `linux/rpm/notav.spec`: no extension, D-Bus service or autostart; GtkSourceView |
| About with a Support page and donation links | `todav/linux/app/src/window.rs` (`about_support`, `DONATIONS`) | As is |
| Polish translations | `po/pl.po` | Shared strings merged from Todav's catalog |

A fix to one of these probably applies to Todav too: port it, or note it there. They move into a shared crate in its own repository once copying costs more than sharing: when multiple accounts or other services rework the account layer, when a third app or a front-end of its own needs them, or when the same fix keeps being made twice.

## Storage

Decided 2026-10-24.

- **Notes folder**: a folder in Nextcloud Files, by default `Notes/` (Nextcloud Notes' default), so the Nextcloud Notes web and mobile apps and Obsidian (through the desktop sync client) see the same files. It is picked on the welcome page and in Preferences, a synced setting (`notes_folder`), and created when missing. Changing it while there are notes asks whether to move them to the new folder.
- **Files**: one note per `.md` file (`.txt` read too, as Nextcloud Notes writes them; new notes are `.md`). Subfolders are folders, shown as Nextcloud Notes' categories are. Dot-folders and dot-files (`.obsidian/`, `.attachments.*`, `.trash/`) and other files are skipped; attachments are v2.
- **Title**: the file name without its extension, as in Obsidian; renaming a note is a `MOVE`. The first line is not the title (Nextcloud Notes renames files after it; Notav does not).
- **Change detection**: Nextcloud Files has no `sync-collection`. A folder's ETag changes when anything below it changes, so a pull is a `PROPFIND Depth: 1` from the notes folder down, only into folders whose ETag changed, then a `GET` for each file whose ETag changed. Files are keyed by `oc:fileid`, which stays the same across a rename or move, so one made on another device is a move here, not a delete and a new note.
- **Push**: optional, recommended: Nextcloud's Client Push app (`notify_push`), which the desktop sync client uses. Its WebSocket (from the `notify_push` capability) sends `notify_file` when the user's files change; the app then pulls. Polling stays as the safety net: a `PROPFIND Depth: 0` of the notes folder (its ETag) every 30 s while the window is focused, and when it gets focus. Todav's `dav_push` covers CalDAV only.
- **ETags are times**: Nextcloud derives a file's ETag from its modification time in whole seconds, not from its content (checked on the Test account: two PUTs in one second get the same ETag). So every upload sets the file's time (`X-OC-MTime`) one second past the version it replaces at least, which keeps `If-Match` reliable between Notav devices; and a pull also counts a file whose size differs from the last synced text as changed. A same-size edit made in the same second by another client (the Nextcloud Notes web app, say) can still go unnoticed until the next change.
- **Conflicts**: a note edited here and changed on the server since the last sync is not overwritten: the local text is uploaded next to it as `<name> (conflict YYYY-MM-DD HH-MM).md`, and the server's version is taken. No text is lost. The text as of the last sync is kept, for a three-way merge later (v2).
- **Folder moves**: moving a folder here rewrites the paths of what is in it and queues one `MOVE` of the folder (`moves`, in order). Nextcloud keeps every file's ETag and `oc:id` through it and takes empty subfolders along, so notes edited before the move still upload at their new place and other devices see moves. The push sends queued folder moves first; while one cannot go (something in it locked), the rest of that sync waits, as pushing into the new paths or pulling the old ones would undo it. A folder of that name already on the server: its notes move one by one instead. Renaming a folder is the same move, within its parent.
- **Folder deletions**: deleting a folder here deletes its notes (each as a note's deletion: `If-Match`, so one changed on the server since stays) and marks the folder (`folders.dirty` 2). The push deletes it after the notes, in one `DELETE` (with what else is in it: attachments, other files), unless a note in it stays: one changed there, or a note file found in it on the server that this copy does not have (added since the last pull); then the folder stays and the pull brings them. While a note in it waits for a lock, so does the folder.
- **Locks**: a file open in Nextcloud Text (the Notes web app edits with it) or locked over WebDAV answers `423 Locked` to `PUT`, `MOVE` and `DELETE`; such a lock can stay long after the editor is gone. The push skips that note, which stays changed here, says which notes wait and why (`SyncReport::locked`: who holds the lock and since when, from `nc:lock-owner-displayname` and `nc:lock-time`) and goes on with the others; the next sync tries again. Notav never takes a lock, and removes one only when the user asks: the app's toast has Unlock…, which asks first (an editor that still has the note open may lose what it saves afterwards), then sends `UNLOCK` with `X-User-Lock` and `X-User-Lock-Type: 0` (`Client::unlock`): `files_lock` lets the file's owner remove any lock when the request names a user's lock, whatever the lock's own type; naming the app's type works only for the app itself (tried: 423 for Text's). Seen on the Test account: a lock of Nextcloud Text with no timeout, a day after the editor was closed.
- **Local copy**: SQLite as in Todav (`core/src/store.rs`): `notes` (path, `oc:id`, ETag, the server's path while a local move waits, text, the text and the server time as of the last sync, dirty flag: 1 changed, 2 deleted), `folders` (path, ETag as last pulled, dirty: 1 made here, 2 deleted here) and `kv`. Search through SQLite's FTS5 (built into the bundled SQLite).
- **Sync** (`core/src/sync.rs`, `Client::sync`): settings first (the notes folder is one), then the push: new folders (`MKCOL`), deletions (`DELETE` with `If-Match`; a note changed on the server since stays), moves (`MOVE`; the folders above made first; a name taken there gets a conflict name), uploads (`PUT` with `If-Match`, or `If-None-Match: *` for a new note; `412` makes a conflict copy, unless the file there is another note changed here: then the note waits for that one's push). Moves go before uploads, as a new note may have taken a name a moved one leaves. Then the pull: a `PROPFIND Depth: 0` of the notes folder, created when missing; if its ETag changed, the folders whose ETags changed are listed (`Depth: 1`), changed files downloaded, moved ones moved (before new files are taken, which may have a moved one's old name), and notes in a listed folder (or one gone) that were not seen anywhere deleted, unless changed here: those are uploaded again as new. Another notes folder (set here or on another device) lists everything again.

## Linux app

A GTK 4 + libadwaita app (`linux/app`), thin over the core as Todav's is.

- **Welcome page**: GNOME Online Accounts or a Nextcloud server (browser sign-in or app password), then the notes folder, suggested from the server (`Client::suggested_notes_folder`: the synced setting, else the Nextcloud Notes app's `notes_path` capability, else `Notes`); it is created when missing.
- **Main window**: the sidebar is the notes folder as a tree, as Obsidian shows it: folders first, then notes, each by name (the locale's order, case ignored); a click opens a folder (open ones are kept in the device-only `expanded_folders`) or a note. The content's header has New Note at its start, then the note's title and folder, search and sync, as Todav's task page has them. Search (FTS5) shows the matching notes in the editor's place while something is typed; picking one, or a note in the tree, ends it. New notes go into the folder last opened in the tree or the open note's. The open note: its title (the file name: renames on Enter or when the focus leaves) above a GtkSourceView with Markdown highlighting styled for reading (see the Editor decision). Typing saves to the core after 0.8 s; the sync worker waits 1.5 s more for the burst to end, then syncs. A note changed by a sync reloads in the editor unless it holds unsaved text (the core never overwrites a note changed here). Notes and folders are moved by dragging them in the tree: onto a folder, onto a note (into its folder), or below the last row (to the top level); the target folder is tinted with what shows of it (the whole tree for the top level, as Obsidian does for its root). One drop target covers the tree and finds the row under the pointer itself: GTK hands a drag's motion to every drop target under the pointer, so targets on the rows and one behind them fought over the tint. The tree's menu (right click, long press) adds a note or a folder (its name asked for): inside a folder when on one, next to a note (in its folder) when on one, at the top level elsewhere; a note's also deletes it, a folder's also renames it (in the New Folder dialog) or deletes it, after asking (with how many notes are in it). A folder made here is a `folders` row until the push creates it (`MKCOL`).
- **No background service**: closing the window hides it, and the app quits once what waits for the server is up (`quit_after_sync`). While the window is open it syncs every 30 s when focused and when it gets the focus.
- **Preferences**: style, a dense tree (device-only `dense_tree`: shorter rows without gaps, as Nautilus' list view), the tree's icons (synced `tree_icons`: symbolic folder and note icons on all rows, on notes only (the default), or none; in either density), the notes folder (another one asks whether to move the notes there, when there are some), the account (as in Todav), settings sync.

## Repository layout

Same shape as Todav, without the app's directory level:

```text
<repo>/
  Cargo.toml                 # workspace: core, cli, linux/app
  core/                      # notav-core (library): storage, WebDAV sync, Markdown links and tags
  cli/                       # notav-cli: debugging/scripting front-end
  linux/
    app/                     # GTK4 + libadwaita app (Rust); Blueprint views in ui/, the
                             # editor's Markdown language and style schemes in sourceview/
    data/                    # .desktop, metainfo, icons
    flatpak/                 # dev.aperte.Notav.json
    rpm/                     # notav.spec (COPR, through .copr/Makefile)
  po/                        # gettext catalogs, domain notav
  scripts/                   # i18n.sh, release.py, deb.sh (from Todav)
```

**Dependency policy**: Todav's table is the starting point. Anything beyond it needs a one-line justification here.

| Need | Crate / library | Why not std |
| --- | --- | --- |
| HTTP + TLS | `ureq` (sync, rustls) | As in Todav |
| SQLite | `rusqlite` (0.38 to 0.40), the system SQLite | As in Todav: not bundled, as distributions (Fedora) require, and 0.38 is what Fedora packages. FTS5 is in every SQLite Notav runs on (Fedora's, Ubuntu's, Arch's, the Flatpak runtime's) |
| XML | `quick-xml` | PROPFIND bodies and multistatus responses |
| Linux UI | `gtk4`, `libadwaita` | The platform toolkit |
| Editor | `sourceview5` (GtkSourceView 5) | Markdown highlighting, undo, search; part of the GNOME platform |
| Linux secrets | `libsecret` | App passwords in the Secret Service |
| Locale | `libc` + glibc's own gettext | As in Todav (`i18n.rs`) |

## Open questions

- [ ] Journal: folder layout and name format of the daily notes, and whether a new one starts from a template.
- [ ] Client Push: its WebSocket needs a client; a minimal hand-written one over the TLS `ureq` already brings, or a crate (justify it in the table).
- [ ] Attachments: Nextcloud Notes keeps them in `.attachments.<fileid>/` next to the note, Obsidian in a folder of its own.

## Plan

- [x] Answer the storage questions: notes folder and layout, change detection, conflicts (see [Storage](#storage))
- [x] Skeleton: Cargo workspace (`core`, `cli`), the copied core pieces with their unit tests, CLI `whoami`/`sync`/`login`, CI check job
- [x] Core: WebDAV Files sync into SQLite (folder-ETag pull, `GET`/`PUT`/`DELETE`/`MOVE`/`MKCOL` with ETags, conflict copies), the notes folder (suggested from the Nextcloud Notes capability, moving the notes to another one); integration tests against the Test account (`cargo test -p notav-core --features integration`, each test in its own `it-notav/<name>/` folder with settings sync off): two-device round trip with rename and delete, changes made on the server (moves, removed folders, dot-folders and other files skipped), conflict copies (same note, same new name), edited here and deleted there, moving the notes folder, a new note taking a moved one's name
- [x] CLI round-trip against the server: `ls`, `cat`, `new`, `put`, `mv`, `rm`, `mkdir`, `folder`, `sync`, `whoami`, `login`
- [x] Linux app shell: sign-in (welcome page with the notes folder, GOA, Login Flow v2, app password), Secret Service, i18n (Polish), data files with a placeholder icon, packaging; CI packages and the Release workflow
- [x] Linux app: notes list, editor, rename and move, search (FTS5 in the core), notes folder in Preferences (with moving the notes). Verified in Broadway against the Test account: sign-in through GOA, notes from the server, typing and renaming uploaded, a server edit shown in the open note, moving the notes folder there and back
- [x] App icon: Todav's calendar turned into a Markdown note (yellow header, `#` heading, a pen), and its symbolic icon
- [ ] Screenshots in the README and the metainfo (Flathub requires them; `scripts/release.py` points them at each release's tag once they are in `docs/screenshots/`)
- [x] Packaging for stores, as in Todav: the metainfo with release notes from the changelog, brand colours and links; `linux/rpm/notav.spec` for COPR (`.copr/Makefile`); the system SQLite. Flathub's rules (no AI-written manifest or submission, AI-assisted code disclosed) are in Todav's `docs/ARCHITECTURE.md`
- [x] Folders in the tree: drag them into other folders (`Client::move_folder`: one `MOVE` on the server, queued in `moves` and pushed before anything else)
- [x] Folders in the tree: rename and delete them (`Client::rename_folder`, a `MOVE` as `move_folder`'s; `Client::delete_folder`, one `DELETE` after its notes'); integration tests: renamed then deleted on the other device, deleted while a note in it changed and another was added on the server
- [ ] Client Push (`notify_push`) listener; polling as the fallback
- [ ] Journal: today's daily note
- [ ] Links, backlinks and tags; `todav://` links (with `GetTask` in Todav)
