# Notav — Roadmap

Planned work, not yet done; the order is not fixed.

## Editor mode

Editor mode (the default) styles the note from the core's parse (`notav_core::markdown`, GitHub
Flavored Markdown); Source mode keeps GtkSourceView's highlighting and nothing more. All below is
Editor mode's. Useful features come before looks: the Markdown marks stay visible for now.

1. **Features**:
   - Tables in quotes (`> | a |`): kept aligned and changed like the others, which leave them
     as they are now.
   - Links between notes: `[[` offers the notes' titles as you type; renaming or moving a note
     updates the links to it (asked first, as Obsidian does); Alt+Left back to the note a
     link was followed from.
2. **Keyboard shortcuts**, later: Tab and Shift+Tab between table cells, Enter to the next row
   (a new one at the end); Tab and Shift+Tab to indent list items; Ctrl+B, Ctrl+I, Ctrl+K around
   the selection; Ctrl+E between Editor and Source mode.
3. **Hiding the marks**, later, as Obsidian's Live Preview: `**`, `_`, backticks, `#`, `>` and
   link targets hidden except in the element (or the heading's line) the cursor is in; drawn
   bullets, quote bars, rules and table grids; images below their line. Blocked for now (see
   [Open questions](#open-questions-and-risks)).
4. **More Markdown**, maybe: math and callouts as Obsidian has them (GitHub's `> [!NOTE]` alerts
   are only quotes now), each only where the files stay readable elsewhere.
5. **Faster styling** if long notes need it: each change parses and styles the whole note again
   (about 45 ms per keystroke for 120 KB in a debug build, a few for a normal note; above
   300,000 characters it stops). Restyling only the blocks around a change would cut that.

## Planned features

- **Journal**: today's daily note, one per day in a dated folder (see
  [Open questions](#open-questions-and-risks)).
- **Tags**: `#tag` and front matter's `tags`, computed locally from the files, to list the notes
  with one. Backlinks read every note each time a note opens; an index of links (and tags) when
  that gets slow.
- **Side panel** at the right of the note, as Obsidian's: buttons at its top switch between its
  views, and it hides like the notes' sidebar.
  - **Outline**: the open note's headings as a tree (from the core's parse), each level folding;
    a click scrolls to the heading.
  - **Backlinks**: the notes linking to the open one (linked mentions) and those naming its
    title without a link (unlinked mentions), each with its line.
  - **Outgoing links**: the notes the open one links to, missing ones marked, and the other
    notes' titles it names without a link. Both link views need links between notes (Editor
    mode) and read the notes as Tags does.
- **Tabs**: several notes open at once, in tabs above the editor (`AdwTabBar`); a middle click
  opens a note in a new tab; the open tabs are kept for the next launch (device-only).
- **Notes' tree**: sorting by name (now), modified or created time, either way, from a menu
  above the tree (device-only); buttons to fold and unfold every folder.
- **Search**: filters as Obsidian's (`path:`, `file:`, `tag:`, `"exact phrase"`, `-word`) over
  today's word-prefix match on FTS5, and the results with the matched line.
- **Trash**: deleting a note or folder moves it into a folder of its own in the notes folder;
  a view of it restores or deletes for good, and empties it (see
  [Open questions](#open-questions-and-risks)).
- **Templates**: notes in a folder chosen in Preferences (a synced setting); "New Note from
  Template" copies one, filling `{{title}}`, `{{date}}` and `{{time}}` (dates from the locale).
  The Journal's daily notes can start from one.
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
- **Table alignment**: tables are padded by each character's width in a monospace font, taken
  from the common wide ranges (CJK, Hangul, emoji; `width` in `core/src/table.rs`). A script
  that still lines up wrong needs Unicode's whole East Asian Width table (the `unicode-width`
  crate) or measuring with Pango, and fallback fonts may not be exactly two columns wide anyway.
  Tables wider than the text column wrap and lose the alignment; scrolling them sideways would
  need them out of the text view. Tidying changes the file's text, which other devices and apps
  see.

- **Journal**: the folder layout and name format of the daily notes, and whether a new one
  starts from a template.
- **Trash**: the folder's name (`.trash` as Obsidian's, if Nextcloud Notes hides it; a visible
  one shows there as a category), whether it is left out of search and links, and whether it
  empties itself after some days. Nextcloud's own trash bin already keeps deleted files, but
  only through the web interface.
- **Created time** for sorting: from the server (`nc:creation_time`, not set by every storage)
  or the time first seen here.
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
- **Editor styles** not taken, for now: a monospace font by default (Apostrophe's Fira Mono,
  Iotas's; Preferences can choose one), a font size that grows with the window (Apostrophe), the
  `#` marks hanging in the margin (Iotas).
