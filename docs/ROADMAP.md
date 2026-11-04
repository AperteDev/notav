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
   - Enter after a code fence's opening ` ``` ` adds the closing one (as Typora and Obsidian
     do), next to the lists', quotes' and tables' Enter.
   - "Insert Alert" in the right-click menu, next to Insert Table, with GitHub's five kinds.
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
   bullets, quote bars, rules and table grids; images below their line; an alert's `[!NOTE]`
   shown as its icon and title, as GitHub draws it. Blocked for now (see
   [Open questions](#open-questions-and-risks)).
4. **More Markdown**, maybe: math, and callouts as Obsidian has them beyond GitHub's five alerts
   (more kinds, a title of their own, folding), each only where the files stay readable
   elsewhere.
5. **Faster styling** if long notes need it: each change parses and styles the whole note again
   (about 45 ms per keystroke for 120 KB in a debug build, a few for a normal note; above
   300,000 characters it stops). Restyling only the blocks around a change would cut that.

## Planned features

- **Accounts**, more: notes and folders moved between accounts (dragged in the tree; now a
  drag stays within its account), as a copy there and a deletion here; the journal's account
  chosen in Preferences (now the first account switched on, `Client::journal_account`).
- **GNOME Shell search**: notes found from the Activities overview by title and text, as Iotas
  and Folio have it; Todav's search provider (`todav/linux/app/src/search.rs`, its `.ini`) over the
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
- **Tabs**: each tab's undo history kept while another is shown (the one editor loads the
  selected tab's note, so switching starts it anew); a restored tab at its cursor and scroll.
- **Notes' tree**: sorting by name (now), modified or created time, either way, from a menu
  above the tree (device-only); buttons to fold and unfold every folder.
- **Search**: filters as Obsidian's (`path:`, `file:`, `tag:`, `"exact phrase"`, `-word`) over
  today's word-prefix match on FTS5, and each result in the sidebar's tree with its matched
  line under it; folders there closing (all open now, and not kept); a result opens at
  its first match, highlighted (as Obsidian and Joplin), with the find bar's text filled in.
  Find and replace in the note with case, whole words and regular expressions, as GNOME Text
  Editor's options.
- **Trash and versions**, more: "Delete All" in the Trash (each note, not the whole trash bin,
  which holds other files too); a version's changes against the note's text rather than the
  whole text.
- **Templates**: notes in a folder chosen in Preferences (a synced setting); "New Note from
  Template" copies one, filling `{{title}}`, `{{date}}` and `{{time}}` (dates from the locale).
  The Journal's daily notes start from one template note already (`{{title}}`, `{{date}}`;
  `notav/core/src/journal.rs`): both should fill in the same way.
- **Links to Todav's tasks**: a note links a task as `todav://task/<uid>`. Todav opens it (it
  handles the `todav` URI scheme), and Notav shows the task's title and state through Todav's
  D-Bus API (`GetTask`, to be added in Todav). Without Todav the link stays a plain link.
- **Notebooks**: categories above folders, as Joplin's and OneNote's notebooks (see
  [Open questions](#open-questions-and-risks)).
- **Client Push**: Nextcloud's Client Push app (`notify_push`), which the desktop sync client
  uses. Its WebSocket (from the `notify_push` capability) sends `notify_file` when the user's
  files change; the app then pulls. Optional on the server; polling stays as the safety net
  (`TODO(v2)` in `notav/linux/app/src/main.rs`). Todav's `dav_push` covers CalDAV only.

Later (v2):

- Attachments (see [Open questions](#open-questions-and-risks)).
- Handwritten notes and drawings: a drawing drawn with a stylus (its pressure, through
  `GtkGestureStylus`), a mouse or a finger, kept as an attachment and shown in the note where it
  is linked (as Xournal++, OneNote and Obsidian's Excalidraw plugin); see
  [Open questions](#open-questions-and-risks).
- Audio notes: a recording made in the app, kept as an attachment and played from the note,
  with its transcription as the note's text, editable (as Joplin's and OneNote's recordings,
  Nextcloud Talk's transcripts); see [Open questions](#open-questions-and-risks).
- A rendered Markdown preview next to the editor.
- A three-way merge of a note changed here and on the server, from the text as of the last
  sync (`notes.base`), instead of a conflict copy.
- A shared crate, in its own repository, for the code copied from Todav (listed in
  `AGENTS.md`), once copying costs more than sharing: when other services rework the account
  layer (accounts are copied already), when a third app or a front-end of its own needs it, or
  when the same fix keeps being made twice.

## Android

The Android app (`notav/android`) still lacks, next:

- Trash, versions, unlocking a note locked on the server (its banner), the tree's density and
  "Sync settings with" in Preferences, About with the Support links.
- Release on F-Droid (after three months of history, from about 2027-01-24; a recipe as
  Todav's, its build matching our signed APK) and Google Play, as Todav; screenshots in the
  fastlane metadata.

Later: signing in
through the Nextcloud app's accounts (as Todav), dragging notes and folders in the tree (a
long press opens their menu now, with Move…), table handles and menus, a table's cells keeping
their padding as they are typed in (`table::padding_taken`), tables wider than the note
scrolling sideways (they wrap now), the outline of headings, find and replace, widgets, a Quick
Settings tile.

## Distribution

- **Screenshots** in the README and the metainfo (Flathub requires them); `scripts/release.py`
  points them at each release's tag once they are in `docs/screenshots/`.
- **Flathub**: a manifest building offline from the release tag, with a generated
  `cargo-sources.json`, written and opened by hand: Flathub accepts no AI-generated manifests or
  AI-opened submissions, and AI-assisted code in the app must be disclosed (as for Todav).

## Open questions and risks

- **Long notes on Android**: Android adds each text style in time growing with the styles already
  there, so a note's thousands of them took seconds a keystroke (a 120 KB note: about 12,000).
  Notes over 10,000 characters are styled in full only around the screen, and everywhere in
  what changes line heights (headings, code, tables); typing there takes about 30 ms a frame in
  a debug build, with a slow frame now and then (about 1 s at the 90th percentile in a debug
  build). Building fewer, merged styles, or styling only the paragraphs that changed, if it
  shows in release builds.
- **A Compose crash in the text selection toolbar** (`TextFieldCoreModifier.kt:759`, "Required
  value was null"), seen once on the emulator: a selection's toolbar asked for the editor's
  bounds as a home gesture's inset animation started. Compose's own; watch its releases, and
  whether it happens on phones.
- **Hiding text in the editor**: GtkTextView's `invisible` tag can abort the app
  ([gtk#8346](https://gitlab.gnome.org/GNOME/gtk/-/issues/8346)); the fix (!10228) was not
  merged nor released as of 2026-10-27, and Notav supports GNOME 49. Either wait for it in the
  supported runtimes, or shrink the marks to almost nothing instead, as
  [Scribe](https://github.com/gnacho/scribe) does (the cursor then steps through them). Scribe
  (Rust, GTK 4, GtkSourceView 5, `pulldown-cmark`) is the closest prior art; it is AGPL, so its
  ideas only, not its code.
- **Table alignment**: tables are padded by each character's width in a monospace font, taken
  from the common wide ranges (CJK, Hangul, emoji; `width` in `notav/core/src/table.rs`). A script
  that still lines up wrong needs Unicode's whole East Asian Width table (the `unicode-width`
  crate) or measuring with Pango, and fallback fonts may not be exactly two columns wide anyway.
  Tables (and code blocks) wider than the text column do not wrap: the note widens for them,
  and past the window's width the whole text scrolls sideways with them; scrolling only the
  table, as Obsidian does, would need it out of the text view. Tidying
  changes the file's text, which other devices and apps see.

- **Notes on this device as files**: the local account keeps its notes in the database only,
  as Todav's local lists; Markdown files in a folder on disk would let other apps (Obsidian)
  open them too, at the cost of watching that folder for their changes.
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
- **Drawings' format**: SVG (strokes as paths) opens in any app and in Nextcloud's previews, and
  Markdown links it as an image; Xournal++'s `.xopp` or Excalidraw's JSON keep more (layers,
  shapes to edit again) but only their apps open them. Drawn into the text view, or in a
  canvas of its own opened from the note.
- **Audio notes**: recording needs GStreamer (`gstreamer-rs`, a new dependency, as `AGENTS.md`
  asks), Opus in Ogg for size. Transcription either on the server, through Nextcloud
  Assistant's speech-to-text (the TaskProcessing API, `core:audio2text`; only where an admin
  installed a provider app), or on the device with whisper.cpp (models of hundreds of MB,
  downloaded on first use; slow without a GPU).
- **Notebooks**: what they add over folders, which Notav already has: Nextcloud Notes'
  categories are its folders (its `category` is the folder's path). Either top-level folders
  shown one at a time (a notebook picked above the tree, as Joplin), several notes folders per
  account, each a notebook, or categories as tags (see Tags). Decide before building any.
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
