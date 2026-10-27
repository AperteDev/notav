# Notav — Roadmap

Planned work, not yet done; the order is not fixed.

## Planned features

- **GitHub Flavored Markdown** as the target syntax: CommonMark plus tables, task lists,
  strikethrough and autolinks, in the editor's highlighting (`notav-markdown`) and in whatever
  reads the text (links, tags, the preview). Maybe more extensions later (footnotes, math,
  callouts, as Obsidian has them), each only where the files stay readable elsewhere.
- **Journal**: today's daily note, one per day in a dated folder (see
  [Open questions](#open-questions-and-risks)).
- **Links, backlinks and tags**: `[[wiki links]]`, backlinks and tags, computed locally from the
  files.
- **Links to Todav's tasks**: a note links a task as `todav://task/<uid>`. Todav opens it (it
  handles the `todav` URI scheme), and Notav shows the task's title and state through Todav's
  D-Bus API (`GetTask`, to be added in Todav). Without Todav the link stays a plain link.
- **Client Push**: Nextcloud's Client Push app (`notify_push`), which the desktop sync client
  uses. Its WebSocket (from the `notify_push` capability) sends `notify_file` when the user's
  files change; the app then pulls. Optional on the server; polling stays as the safety net
  (`TODO(v2)` in `linux/app/src/main.rs`). Todav's `dav_push` covers CalDAV only.

Later (v2):

- Attachments (see [Open questions](#open-questions-and-risks)).
- A rendered Markdown preview next to the editor.
- A three-way merge of a note changed here and on the server, from the text as of the last
  sync (`notes.base`), instead of a conflict copy.
- A shared crate, in its own repository, for the code copied from Todav (listed in
  `AGENTS.md`), once copying costs more than sharing: when multiple accounts or other services
  rework the account layer, when a third app or a front-end of its own needs it, or when the
  same fix keeps being made twice.

## Distribution

- **Screenshots** in the README and the metainfo (Flathub requires them); `scripts/release.py`
  points them at each release's tag once they are in `docs/screenshots/`.
- **Flathub**: a manifest building offline from the release tag, with a generated
  `cargo-sources.json`, written and opened by hand: Flathub accepts no AI-generated manifests or
  AI-opened submissions, and AI-assisted code in the app must be disclosed (as for Todav).

## Open questions and risks

- **Journal**: the folder layout and name format of the daily notes, and whether a new one
  starts from a template.
- **Client Push**: its WebSocket needs a client: a minimal hand-written one over the TLS `ureq`
  already brings, or a crate (justified as `AGENTS.md` asks).
- **Attachments**: Nextcloud Notes keeps them in `.attachments.<fileid>/` next to the note,
  Obsidian in a folder of its own.
- **ETags are times**: Nextcloud derives a file's ETag from its modification time in whole
  seconds, so a same-size edit made in the same second by another client (the Nextcloud Notes
  web app, say) can go unnoticed until the next change (see `Dav::put`).

## Ruled out

- **A background service** (autostart, `HOLD`) in v1: notes have no reminders, so the app syncs
  only while it is open (decided 2026-10-24).
- **The first line as the title**: the title is the file name, as in Obsidian; Nextcloud Notes
  renames files after their first line, Notav does not.
- **Editor styles** not taken, for now: a monospace font (Apostrophe's Fira Mono, Iotas's
  default), a font size that grows with the window (Apostrophe), the `#` marks hanging in the
  margin (Iotas), font and width preferences.
