# Notav: Architecture and Plan

Oct 24, 2026, @Kris

## Overview

Notav (*notatka* + DAV) is a personal, local-first notes app with an Obsidian-style journal, kept as plain Markdown files in Nextcloud Files over WebDAV. It is the sibling of [Todav](https://github.com/SelfRef/todav) (tasks and calendar over CalDAV) and follows its design: one Rust core with all storage, sync and parsing logic, thin front-ends, the same Nextcloud and GNOME Online Accounts integration.

Not started: nothing in the code yet, this document is the plan.

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

- Name Notav, app ID `io.github.selfref.Notav`, gettext domain `notav`, its own repository (decided 2026-10-24).
- **Separate from Todav in configuration and credentials**: its own sign-in (its own app password, a GNOME Online Accounts account picked on its own), keyring entries under its own app ID, its own synced settings file (`.config/notav/settings.json`) and its own database. Neither app reads or writes the other's.
- Storage: Markdown files in Nextcloud Files over WebDAV.
- Journal: one daily note per day in a dated folder; `[[wiki links]]`, backlinks and tags are computed locally from the files.
- Cross-app, with no shared database: a note links a task as `todav://task/<uid>`; Todav opens it (it handles the `todav` URI scheme), and Notav shows the task's title and state through Todav's D-Bus API (`GetTask`, to be added in Todav). Without Todav the link stays a plain link.
- Tooling and releases as in Todav: CI on every push (fmt, clippy with warnings as errors, unit tests, packages), the manual Release workflow, Keep a Changelog, the lowest supported GNOME 49. Set up with the first code, from Todav's workflows and scripts.

## Code from Todav

Copied, not shared: Notav starts from copies of Todav's code that is not about tasks or calendars (as of Todav 1.2.0), and the two copies may diverge.

| Piece | Todav source | Notes |
| --- | --- | --- |
| Login Flow v2, app password sign-in | `todav/core/src/login.rs` | As is |
| Minimal JSON | `todav/core/src/json.rs` | As is |
| WebDAV XML tree, multistatus, percent-encoding | `todav/core/src/caldav.rs` (first part) | The CalDAV requests stay in Todav |
| Push keys, Web Push decryption, ntfy stream | `todav/core/src/push.rs` | Only if file changes can be pushed (see [Open questions](#open-questions)); the `dav_push` registration is CalDAV-only |
| Synced settings merge | `todav/core/src/settings.rs` | The merge, parse and serialize; the rest is tied to Todav's store |
| GNOME Online Accounts | `todav/linux/app/src/goa.rs` | As is |
| gettext over glibc, first weekday | `todav/linux/app/src/i18n.rs` | Domain `notav` |
| Secret Service | `todav/linux/app/src/main.rs` (`schema` and helpers) | Schema under Notav's app ID |
| Welcome page, login dialogs | `todav/linux/app/src/connection.rs`, `ui/connection.blp`, `ui/password-login.blp`, parts of `window.rs` | Adapted |

A fix to one of these probably applies to Todav too: port it, or note it there. They move into a shared crate in its own repository once copying costs more than sharing: when multiple accounts or other services rework the account layer, when a third app or a front-end of its own needs them, or when the same fix keeps being made twice.

## Repository layout (planned)

Same shape as Todav, without the app's directory level:

```text
<repo>/
  Cargo.toml                 # workspace: core, cli, linux/app
  core/                      # notav-core (library): storage, WebDAV sync, Markdown links and tags
  cli/                       # notav-cli: debugging/scripting front-end
  linux/
    app/                     # GTK4 + libadwaita app (Rust); Blueprint views in ui/
    data/                    # .desktop, metainfo, icons, D-Bus service
    flatpak/                 # io.github.selfref.Notav.json
  po/                        # gettext catalogs, domain notav
  scripts/                   # i18n.sh, release.py, deb.sh (from Todav)
```

**Dependency policy**: Todav's table is the starting point: `ureq` (HTTP + TLS), `rusqlite` (SQLite), `quick-xml` (WebDAV bodies), `ring` + `p256` (Web Push decryption, only if needed), `gtk4` and `libadwaita`, `libsecret`, `libc`. Anything beyond it, a Markdown parser or GtkSourceView for instance, needs a one-line justification here.

## Open questions

- [ ] Change detection for files: Nextcloud Files has no `sync-collection`; compare folder ETags (they propagate upwards) and only descend into changed folders.
- [ ] Push for files: `dav_push` covers CalDAV/CardDAV only; check whether Nextcloud's `notify_push` (or a small app posting to ntfy) can signal file changes. Polling as the fallback, as in Todav.
- [ ] Conflicts: two devices editing the same note offline. Newer-wins for the whole file loses text; keep both (a conflict copy, as the Nextcloud desktop client does) or merge.
- [ ] Folder layout and name of the notes root (reuse the Nextcloud Notes folder or a separate one), daily note format, attachments.
- [ ] Editor: GtkSourceView with Markdown highlighting vs. a rendered/WYSIWYG view.

## Plan

Order to be confirmed.

- [ ] Answer the storage questions above: notes root and layout, change detection, conflicts
- [ ] Skeleton: Cargo workspace, CI, release workflow and scripts from Todav
- [ ] Core: the code from Todav; WebDAV Files sync into SQLite (listing, `GET`/`PUT` with ETags, folder-ETag change detection); tests against the Test account
- [ ] CLI round-trip against the server
- [ ] Linux app: sign-in, notes list, editor, today's daily note
- [ ] Links, backlinks and tags; `todav://` links (with `GetTask` in Todav)
