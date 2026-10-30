# Agent notes

## Roadmap

- `docs/ROADMAP.md` holds the planned work only (features, open questions, what was ruled out);
  what is built is described by the code. Read it before working on a planned feature, and
  update it in the same change: remove what is done, add a plan or an open question, move a
  dropped plan under "Ruled out" with why.

## Todav

- Notav is the sibling of Todav (`AperteDev/todav`, cloned next to this repository) and follows
  its conventions; when something here is not decided yet, look at how Todav does it (its
  `AGENTS.md`, `docs/ROADMAP.md` and code).
- The two apps keep separate configuration and credentials: never read or write Todav's
  files, keyring entries, settings or database. Talk to Todav only through its D-Bus API and
  `todav://` links.
- Code copied from Todav (as of Todav 1.2.0; the copies may diverge). A fix to one of these
  probably applies there too: port it or say so.

  | Piece | From Todav | Here |
  | --- | --- | --- |
  | Login Flow v2, app password sign-in | `core/src/login.rs` | As is |
  | Minimal JSON | `core/src/json.rs` | As is |
  | WebDAV XML, multistatus, percent-encoding, requests with retries | `core/src/caldav.rs` (first part) | `core/src/dav.rs` |
  | Synced settings | `core/src/settings.rs` | Its own `DEVICE_ONLY`, no ntfy keys |
  | `Error`, `b64encode`, `Client::open`, store migrations, `kv` | `core/src/lib.rs`, `store.rs` | Without tasks and calendars |
  | Accounts (Todav 2.3): the `accounts` table, per-account `kv` keys, settings account, `server_info` | `core/src/account.rs`, `caldav.rs` (`server_info`) | Nextcloud and local only (no Google or system calendars); notes and folders per account instead of lists; no push |
  | Undo of local changes | `core/src/undo.rs` | Steps of notes and folders, no grouping |
  | CLI `.env` loading | `cli/src/main.rs` | `NOTAV_*` |
  | GNOME Online Accounts | `linux/app/src/goa.rs` | As is |
  | gettext over glibc, first weekday | `linux/app/src/i18n.rs` | Domain `notav` |
  | Secret Service, accounts (add, local, reconnect, switch off, remove), sync worker | `linux/app/src/main.rs`, `todav/app/src/accounts.rs` | No push, reminders, D-Bus or Google; the worker syncs each account itself and names the failed ones |
  | Adding an account (welcome page, Preferences), login dialogs | `add_account.rs`, `ui/add-account.blp`, `ui/password-login.blp`, parts of `window.rs` | A notes folder step after connecting; no ntfy or Google; GOA accounts need Files on |
  | Preferences: style, accounts (a page each, settings sync, reordering) | `linux/app/src/settings.rs`, `ui/settings.blp`, `ui/account.blp` | A notes folder row; no DAV Push row or ntfy |
  | Undo toast, Ctrl+Z | `linux/app/src/window.rs` (`undo_toast`, `undo`) | As is |
  | About with Support and donation links | `window.rs` (`about_support`, `DONATIONS`) | As is |
  | Update while running, Restart banner | `linux/app/src/update.rs`, `update_banners` in `window.rs` | No restart in the background (no service): only the banner, quitting as closing the window does |
  | Build, packaging, release | `build.rs`, `linux/Makefile`, Flatpak manifest, `scripts/`, `.github/workflows/`, `po/its/`, RPM spec, `.copr/Makefile` | No GNOME extension, D-Bus service or autostart; GtkSourceView |
  | Polish translations | `po/pl.po` | Shared strings merged |

## Dependencies

- Standard library and platform SDKs first; a third-party crate only where it replaces real
  work, with the reason in its commit. Now: `ureq` (rustls), `rusqlite`, `quick-xml`,
  `pulldown-cmark` (Markdown for Editor mode, without its HTML writer), `gtk4`, `libadwaita`,
  `sourceview5`, `libsecret`, `libc`.
- `rusqlite` stays within 0.38 to 0.40 on the system SQLite, not bundled: distributions
  (Fedora) require that, and Fedora packages 0.38. Search needs FTS5, which every SQLite Notav
  runs on has.

## Releases

- Add user-visible changes to `docs/CHANGELOG.md` under `## [Unreleased]`, in the same commit as the
  change. Keep `docs/FEATURES.md` (what Notav does, one short line each, and what is planned)
  in step in the same commit: a new feature goes under "Now" and leaves "Planned"; a new plan
  in `docs/ROADMAP.md` gets its line under "Planned". It follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/): `Added`,
  `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`.
- Don't bump versions by hand: releases are cut by the manual Release workflow
  (`.github/workflows/release.yml`), as in Todav: `scripts/release.py` sets the version in
  `Cargo.toml`, `Cargo.lock`, the metainfo and the RPM spec and moves the `[Unreleased]` changes
  under it; the metainfo gets them as the release's notes (untranslated). After publishing,
  `scripts/aur.sh` moves the AUR packages `notav` and `notav-bin` to the release, pushed by the
  AURomatic account (a co-maintainer; AperteDev secret `AUR_SSH_KEY`, shared with Todav) as
  commits by SelfRef; a packaging change to them still goes in by hand. The metainfo must
  pass `appstreamcli validate` without warnings (Flathub treats them as errors).
- CI (`.github/workflows/ci.yml`) runs on every push: `cargo fmt --check`, clippy with warnings
  as errors, the unit tests, and the packages (Flatpak, tarball, `.deb`). Run the checks before
  committing.

## Translations

- All text a user sees is translatable; one gettext domain, `notav`, with catalogs in `po/`.
  The rules are Todav's (see its `AGENTS.md`): named placeholders, plurals with `ngettext`,
  whole sentences, Polish translated for review, dates from the locale.

## Stored data

- An update carries on with what the previous version kept. The database (`notav.sqlite` in
  the data folder) may change in any way, as a new step at the end of `core/src/store.rs`
  `MIGRATIONS` that converts what is there (tables, columns, `kv` keys and values); a released
  step is never edited. Each new step gets a test that opens a database of the step before and
  checks what the new one made of it. A database from a newer Notav is refused (`store::open`),
  never written to.
- Settings change the same way: a renamed key or another value format is converted by a step
  (device-only ones such as `expanded_folders` too). Synced settings
  (`.config/notav/settings.json`) are also read by other devices, which may run an older
  version until they update: a synced key changed in meaning goes under a new name, so the old
  versions keep theirs.

## Accounts

- Notes, folders and folder moves belong to accounts (`accounts` table, as in Todav): Nextcloud
  ones and the local one (on this device only, never synced), at most one. Paths are unique
  within an account; core calls that take a path take its account too. Anything per server
  (connection, notes folder, Files root, features, last notes root, settings' ETag) is per
  account; per-account `kv` keys are `a<id>_…` (`account::key`).
- A Nextcloud account syncs once its notes folder is chosen (`chosen_notes_folder`), device-only.
  Settings sync through the account picked in "Sync settings with" (device-only
  `settings_account`); that account's notes folder is also the synced `notes_folder`, offered
  to other devices signing in to it.
- The tree shows an account's row only while more than one is switched on. Notes and folders
  move within their account only (see `docs/ROADMAP.md`).

## Linux UI

- Features should be easy to use without extra UI: prefer what fits where the user already is
  (keys that do the expected thing in context, as Tab in a table or Enter after a header line,
  a context menu, a click on the text) over new buttons, panels or dialogs. Add visible UI only
  where nothing in place would do, or where it is planned (`docs/ROADMAP.md`).
- Static views are Blueprint files in `linux/app/ui/`, compiled by `build.rs`; dynamic UI (note
  rows) stays in Rust code.
- What Editor mode changes by itself (a table tidied as the cursor leaves it) gets a switch in
  Preferences, Editor → Markdown: a synced setting, on by default, read where the change is made
  (`editor::format_tables`). What the user asks for (a menu item, a click) needs none.

## Testing

- Unit tests: `cargo test --workspace`. Against the server: `cargo test -p notav-core --features
  integration`, with the Test account from the gitignored `.env` (`NOTAV_URL`, `NOTAV_USER`,
  `NOTAV_PASS`, as in Todav); each test works in its own `it-notav/<name>/` folder with settings
  sync off. Leave the Test account's synced `notes_folder` at `Notes` when done.
- Nextcloud derives a file's ETag from its modification time in whole seconds: two writes in one
  second keep it. Keep `X-OC-MTime` on uploads (see `Dav::put`).
- The app, as in Todav: `make -C linux sandbox` runs a separate copy (`NOTAV_SANDBOX`, data in
  `target/sandbox`); `make -C linux broadway` runs it in a browser on display `:6`
  (`http://127.0.0.1:8086`, Todav uses `:5`). Todav's notes on Broadway's limits apply. Closing
  the window quits the app (after the sync), so a new launch starts fresh.
- With GTK 4.24, Broadway aborts as soon as a note opens (`append_node_ops` assertion). To see
  and drive the app, use the `waydriver` MCP server (`waydriver-mcp`, installed with `cargo
  install waydriver-mcp` and registered at user level): `start_session` with `command` the
  absolute path of `scripts/waydriver-app.sh` and `app_name` `notav-waydriver`. It runs the
  sandbox copy in a headless Mutter of its own (its own screen and input: nothing reaches the
  desktop), finds widgets in the accessibility tree by XPath, and records each session (report
  under `/tmp/waydriver`). Build the app first. What works:
  - Find rows, buttons and the like by name (`//ListItem[@name='Plan']`, `//ToggleButton[@name=
    'Main Menu']`; icon buttons are named after their tooltip). Never another `app_name`: the
    tree is the desktop's, and a copy running there under its own name would be driven instead.
  - Click with `hover` then `pointer_click`; the first pointer click of a session is often lost,
    so check the result (`query`, `take_screenshot`) and click again. `click` (the accessibility
    action) does nothing on the tree's rows and labels.
  - Menus open (F10 for the main menu) but their items are not in the tree: point at them by
    their place in a screenshot (`move_pointer_absolute`, `pointer_click`), or use the keyboard.
  - `read_text` on `(//TextBox)[last()]` is the open note's text; `press_key`, `type_text` type.
    A capital letter is typed with Shift: `Ctrl+z` undoes, `Ctrl+Z` is Ctrl+Shift+Z (redo).
    `type_text` drops tabs: `press_key` `Tab`. A right click is `pointer_click` with button 273;
    a click with a key held (Ctrl+click) cannot be made.
  - `read_value` on `(//ScrollBar)[1]` is the open note's scroll position.
- Editor mode's reference note, `core/tests/reference.md`, has every piece of Markdown it styles;
  the core's tests parse it, and it can be put on the Test account (`notav new`) to look at in the
  app. Delete it from there afterwards.

