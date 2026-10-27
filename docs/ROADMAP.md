# Notav — Roadmap

Planned work, not yet done; the order is not fixed.

## Editor mode

Two modes, as other editors name them: **Editor mode**, the default, and **Source mode**, the
editor of now. The file is plain Markdown in both. Source mode keeps its highlighting (the
GtkSourceView language `notav-markdown`) and nothing more; all else below is Editor mode's.
Useful features come before looks: the Markdown marks stay visible for now.

1. **Parsing in the core**: GitHub Flavored Markdown (CommonMark plus tables, task lists,
   strikethrough, autolinks; footnotes too) parsed in `notav-core` with `pulldown-cmark` into
   style ranges, with nothing of GTK in it, so it is unit-tested and serves links, tags and the
   preview later. The whole note is parsed again shortly after each change (notes are small;
   above some size, live styling stops). A new dependency: justify it in `AGENTS.md`. Maybe more
   extensions later (math, callouts, as Obsidian has them), each only where the files stay
   readable elsewhere.
2. **All of the syntax shown well**, from the parse: headings (`#` and underlined), emphasis,
   strong, strikethrough, inline code and code blocks (fenced and indented, on a background),
   quotes (nested), lists (nested, numbered, tasks), tables (monospace), links, autolinks and
   images (as text for now), rules, HTML (dimmed), footnotes, line breaks and escapes; the marks
   dimmed, as now.
3. **Features**:
   - Tables that look like Markdown tables, kept aligned: cells padded so the pipes line up
     and the `|---|` row rewritten with its alignment colons, when the cursor leaves a row or on
     request. From the right-click menu: insert a table, add, delete and move rows and columns,
     align a column, sort by one. Text pasted from a spreadsheet (tab-separated) becomes a
     table. Each change is one undo step.
   - Task lists: a click on `[ ]` or `[x]` ticks or unticks it, though it still looks like text.
   - Lists: Enter continues a bullet, a number (renumbering what follows) or a task; Enter on
     an empty item ends the list.
   - Links: Ctrl+click opens one.
4. **Keyboard shortcuts**, later: Tab and Shift+Tab between table cells, Enter to the next row
   (a new one at the end); Tab and Shift+Tab to indent list items; Ctrl+B, Ctrl+I, Ctrl+K around
   the selection; Ctrl+E between Editor and Source mode.
5. **Hiding the marks**, later, as Obsidian's Live Preview: `**`, `_`, backticks, `#`, `>` and
   link targets hidden except in the element (or the heading's line) the cursor is in; drawn
   bullets, quote bars, rules and table grids; images below their line. Blocked for now (see
   [Open questions](#open-questions-and-risks)).

## Planned features

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

- **Hiding text in the editor**: GtkTextView's `invisible` tag can abort the app
  ([gtk#8346](https://gitlab.gnome.org/GNOME/gtk/-/issues/8346)); the fix (!10228) was not
  merged nor released as of 2026-10-27, and Notav supports GNOME 49. Either wait for it in the
  supported runtimes, or shrink the marks to almost nothing instead, as
  [Scribe](https://github.com/gnacho/scribe) does (the cursor then steps through them). Scribe
  (Rust, GTK 4, GtkSourceView 5, `pulldown-cmark`) is the closest prior art; it is AGPL, so its
  ideas only, not its code.
- **Table alignment**: CJK characters and emoji take two columns; padding needs their display
  width (the `unicode-width` crate, or measuring with Pango). Tables wider than the text column
  either wrap (and lose the alignment) or scroll sideways. Padding changes the file's text when
  a table is tidied, which other devices and apps see.

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
