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
     link was followed from. A note dragged from the tree into the text is linked there, as in
     Obsidian.
   - Paste: a URL pasted over selected text links it (`[text](url)`), as VS Code's Markdown
     extension and Obsidian's paste plugins do; Ctrl+Shift+V pastes plain text (no link, no
     table from spreadsheet rows).
   - `*`, `_`, `` ` ``, `~`, `=` or `[` typed with text selected wraps it (as Typora and
     Obsidian do), which covers most of Ctrl+B and Ctrl+I without learning them.
   - Ctrl+Enter ticks or unticks the task on the cursor's line, and makes a list item a task
     (as Obsidian and Logseq do).
   - Enter in a quote continues its `> `; Enter after a code fence's opening ` ``` ` adds the
     closing one (as Typora and Obsidian do), next to the lists' and tables' Enter.
   - Spell checking with libspelling (as Iotas, Apostrophe and GNOME Text Editor have it):
     underlined words, suggestions in the right-click menu (see
     [Open questions](#open-questions-and-risks)).
2. **Keyboard shortcuts**, later: Tab and Shift+Tab to indent list items; Ctrl+B, Ctrl+I,
   Ctrl+K around the selection; Ctrl+E between Editor and Source mode.
   **Editor toolbar**, later: a row above the note with what Editor mode can insert or apply
   (headings, emphasis, lists, tasks, links, Insert Table, code), each also on its key; hidden
   in Source mode.
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

- **On This Device Only**, as in Todav: use Notav without a server, chosen on the welcome page;
  the notes stay on this computer, synced nowhere. Later, with an account too, notes kept on
  this device next to the synced ones, as Todav's local lists (see
  [Open questions](#open-questions-and-risks)).
- **Versions** of a note from Nextcloud's file versions (`remote.php/dav/versions/<user>/`):
  "Versions" in a note's menu lists them, shows one and restores it (moved into `restore`), as
  Joplin's and Standard Notes' note history; also a way back after a conflict copy.
- **GNOME Shell search**: notes found from the Activities overview by title and text, as Iotas
  and Folio have it; Todav's search provider (`linux/app/src/search.rs`, its `.ini`) over the
  core's search, under the app's own bus name.
- **View menu**, more: a focus mode keeping the cursor's line in the middle of the view (as iA
  Writer, Apostrophe and Iotas); the note's word count in the header's subtitle (as Bear's info
  panel and Obsidian's status bar).
- **Journal**, more: a calendar of the days with a note (the previous and next day's note from
  one), weekday and month names in the name format (from the locale), `{{time}}` and
  `{{yesterday}}`-style placeholders.
- **Tags**: `#tag` and front matter's `tags`, computed locally from the files, to list the notes
  with one. Backlinks read every note each time a note opens; an index of links (and tags) when
  that gets slow.
- **Side panel**: the outline at the right of the note (the Outline button) gets more views, as
  Obsidian's: buttons at its top switch between them.
  - **Outline**: each level folding; the heading the cursor is under marked.
  - **Backlinks**: the notes linking to the open one (linked mentions) and those naming its
    title without a link (unlinked mentions), each with its line.
  - **Outgoing links**: the notes the open one links to, missing ones marked, and the other
    notes' titles it names without a link. Both link views need links between notes (Editor
    mode) and read the notes as Tags does.
  - **Tags**: every tag with its count of notes, nested tags (`#a/b`) as a tree; a click lists
    the notes with it (a `tag:` search).
- **Tabs**: several notes open at once, in tabs above the editor (`AdwTabBar`); a middle click
  opens a note in a new tab; the open tabs are kept for the next launch (device-only).
- **Notes' tree**: sorting by name (now), modified or created time, either way, from a menu
  above the tree (device-only); buttons to fold and unfold every folder.
- **Search**: filters as Obsidian's (`path:`, `file:`, `tag:`, `"exact phrase"`, `-word`) over
  today's word-prefix match on FTS5, and the results with the matched line; a result opens at
  its first match, highlighted (as Obsidian and Joplin). Ctrl+F finds (and replaces) in the open
  note, GtkSourceView's search, and Ctrl+Shift+F searches all notes, as Obsidian and VS Code
  (Iotas searches both ways too).
- **Trash** through Nextcloud's own trash bin, where deleted notes already go: its WebDAV API
  (`remote.php/dav/trashbin/<user>/trash`) lists them with where they were and when they were
  deleted, restores one in place (moved into `restore`), deletes one for good and empties it. A
  view of the deleted notes in the notes folder restores them; no trash folder of Notav's own.
  Folio has a trash too.
- **Templates**: notes in a folder chosen in Preferences (a synced setting); "New Note from
  Template" copies one, filling `{{title}}`, `{{date}}` and `{{time}}` (dates from the locale).
  The Journal's daily notes start from one template note already (`{{title}}`, `{{date}}`;
  `core/src/journal.rs`): both should fill in the same way.
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
  Tables wider than the text column wrap and lose the alignment (and their handles, placed from
  the header's pipes, land off the columns); scrolling them sideways would need them out of the
  text view. Tidying changes the file's text, which other devices and apps
  see.

- **On This Device Only**: where the notes live, the local database only (as Todav's local
  lists) or Markdown files in a folder on disk that other apps (Obsidian) open too; and how a
  local note moves to an account later.
- **Trash and versions**: Nextcloud's trash bin keeps files for a time of its own (the
  server's `trashbin_retention_obligation`), and its versions API documents listing and
  restoring but not reading an old version's text (a `GET` of the version is to be tried).
- **Spell checking**: libspelling is a new dependency (as `AGENTS.md` asks, with its reason);
  whether the GNOME runtime the Flatpak uses ships it, or the manifest builds it.
- **GNOME Shell search**: the provider is reached through the app's bus name and D-Bus
  activation; Notav runs no service of its own (see [Ruled out](#ruled-out)), so a search wakes
  the app the way opening it does.
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
- **Export to PDF, ODT or HTML** (Iotas, Typora): Notav leaves out `pulldown-cmark`'s HTML
  writer, and the notes are Markdown files any app opens already.
- **Colours in fenced code blocks** by their language (Obsidian, Bear): GtkSourceView
  highlights one language per buffer, the note's.
- **Folding headings** (Obsidian, Logseq), for now: it hides text, which the `invisible` tag
  bug blocks (see [Open questions](#open-questions-and-risks)).
