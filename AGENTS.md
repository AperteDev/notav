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
  | `Error`, `b64encode`, `Client::open`/`set_account`/`logout`, store migrations, `kv` | `core/src/lib.rs`, `store.rs` | Without tasks and calendars |
  | Undo of local changes | `core/src/undo.rs` | Steps of notes and folders, no grouping |
  | CLI `.env` loading | `cli/src/main.rs` | `NOTAV_*` |
  | GNOME Online Accounts | `linux/app/src/goa.rs` | As is |
  | gettext over glibc, first weekday | `linux/app/src/i18n.rs` | Domain `notav` |
  | Secret Service, account handling, sync worker | `linux/app/src/main.rs` | No push, reminders, D-Bus or local mode |
  | Welcome page, login dialogs | `connection.rs`, `ui/connection.blp`, `ui/password-login.blp`, parts of `window.rs` | GOA accounts need Files on |
  | Preferences: style, account | `linux/app/src/settings.rs`, `ui/settings.blp` | The account group as is |
  | Undo toast, Ctrl+Z | `linux/app/src/window.rs` (`undo_toast`, `undo`) | As is |
  | About with Support and donation links | `window.rs` (`about_support`, `DONATIONS`) | As is |
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
  change. It follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/): `Added`,
  `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`.
- Don't bump versions by hand: releases are cut by the manual Release workflow
  (`.github/workflows/release.yml`), as in Todav: `scripts/release.py` sets the version in
  `Cargo.toml`, `Cargo.lock`, the metainfo and the RPM spec and moves the `[Unreleased]` changes
  under it; the metainfo gets them as the release's notes (untranslated). The metainfo must
  pass `appstreamcli validate` without warnings (Flathub treats them as errors).
- CI (`.github/workflows/ci.yml`) runs on every push: `cargo fmt --check`, clippy with warnings
  as errors, the unit tests, and the packages (Flatpak, tarball, `.deb`). Run the checks before
  committing.

## Translations

- All text a user sees is translatable; one gettext domain, `notav`, with catalogs in `po/`.
  The rules are Todav's (see its `AGENTS.md`): named placeholders, plurals with `ngettext`,
  whole sentences, Polish translated for review, dates from the locale.

## Linux UI

- Static views are Blueprint files in `linux/app/ui/`, compiled by `build.rs`; dynamic UI (note
  rows) stays in Rust code.

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
- With GTK 4.24, Broadway aborts as soon as a note opens (`append_node_ops` assertion, with or
  without Editor mode). To see the editor, run the sandbox copy in a headless nested GNOME Shell
  as Todav's notes describe (`gnome-shell --headless --virtual-monitor 1400x1000 --unsafe-mode`
  on its own bus, Todav's `secrets-forward.py` for the keyring), with clicks and screenshots
  through `org.gnome.Shell.Eval` and `org.gnome.Shell.Screenshot`. Send a button's press and
  release in separate calls: in one, the click is lost.
- Editor mode's reference note, `core/tests/reference.md`, has every piece of Markdown it styles;
  the core's tests parse it, and it can be put on the Test account (`notav new`) to look at in the
  app. Delete it from there afterwards.

