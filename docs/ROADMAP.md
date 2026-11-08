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
     extension and Obsidian's paste plugins do (Ctrl+Shift+V, which pastes the text as it is,
     not).
   - `*`, `_`, `` ` ``, `~`, `=` or `[` typed with text selected wraps it (as Typora and
     Obsidian do), which covers most of Ctrl+B and Ctrl+I without learning them.
   - Ctrl+Enter ticks or unticks the task on the cursor's line, and makes a list item a task
     (as Obsidian and Logseq do).
   - Enter after a code fence's opening ` ``` ` adds the closing one (as Typora and Obsidian
     do), next to the lists', quotes' and tables' Enter.
   - "Insert Alert" in the right-click menu, next to Insert Table, with GitHub's five kinds.
   - A slash menu: `/` at the start of a line offers table, alert, task, code block, quote, rule
     and the date, filtered as you type (as Obsidian's Slash commands and Notion's); it shares
     its popover with `[[`, and replaces a menu or toolbar for what is inserted.
   - Alt+Up and Alt+Down move the line (or the list item with its sub-items) up and down, as VS
     Code and Obsidian do; dragging the item exists already.
   - Ordered lists renumbered as items are added, moved or removed (Typora's, Obsidian's smart
     lists).
   - Typing `(`, `[`, `"` or `` ` `` adds the closing one (as Obsidian and Typora do), with a
     switch in Preferences, Editor → Markdown, as it changes the text by itself.
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
- **Quick switcher** (Ctrl+O): a list of notes' titles filtered as you type (fuzzy), Enter opens
  one, and a title that matches none offers to create it (as Obsidian's; nextcloud/notes #1200);
  Ctrl+Shift+F stays the full-text search. Recent notes first, with Alt+Left and Alt+Right
  going back and forward through every note opened, not only a followed link.
- **Notes' menu**, more: Copy Link (`[[title]]`), Duplicate, and Open in Nextcloud (the
  browser, through the file's id); **Share** creates a Nextcloud public link (OCS share API,
  with a password or expiry if asked) and copies it.
- **Import**: Markdown files dropped from the file manager onto the tree are added to the folder
  they land on (as Obsidian's Importer, for files exported from Joplin, Bear or others).
- **Desktop entry actions** (New Note, Today's Note, from the app icon's menu) and a `notav://`
  link (`new?title=&text=`) so a bookmarklet or browser extension can clip a page into a note,
  without an HTML converter of our own to fetch pages (see Android's Later, its shortcuts).
- **Link hover preview**: Ctrl+hover on a link to a note shows its first lines (Obsidian's Page
  preview); needs links between notes.
- **Notes' tree**, more: an excerpt and the modified date under each title, as a density option
  (Bear, Joplin, Nextcloud Notes' list); folders hidden from the tree and search (an archive,
  nextcloud/notes #295; Obsidian's excluded files).
- **Extract and merge** (Obsidian's Note composer): the selection moved into a new note, a
  link left in its place; one note merged into another.
- **Split view**: a second note beside the open one (Obsidian's panes, Joplin's horizontal
  split), to read one while writing the other; the tabs' notes, one in each side.
- **Preview mode**, more: find in the page (Ctrl+F, over WebKit's `FindController`, the find
  bar without Replace), the backlinks under the note as in Editor mode, the scroll kept when the
  note or its look changes (it loads again at its top now), images from the note's folder (with
  attachments).
- **View menu**, more: a focus mode keeping the cursor's line in the middle of the view (as iA
  Writer, Apostrophe and Iotas); the note's word count in the header's subtitle (as Bear's info
  panel and Obsidian's status bar).
- **Journal**, more: a calendar of the days with a note (the previous and next day's note from
  one), weekday and month names in the name format (from the locale), `{{time}}` and
  `{{yesterday}}`-style placeholders.
- **Tags**: `#tag` and front matter's `tags`, computed locally from the files, to list the notes
  with one. Backlinks read every note each time a note opens; an index of links (and tags) when
  that gets slow. Front matter's `aliases` too: `[[` offers them and links to one open the
  note. A table of the notes of a folder by their front matter (Obsidian's Bases) only after
  this, if at all.
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
  Editor's options. Searches saved by name and listed above the tree (Obsidian's Bookmarks,
  Bear), and a filter for open tasks (`- [ ]`) over every note.
- **Trash and versions**, more: "Delete All" in the Trash (each note, not the whole trash bin,
  which holds other files too); a version's changes against the note's text rather than the
  whole text.
- **Templates**: notes in a folder chosen in Preferences (a synced setting); "New Note from
  Template" copies one, filling `{{title}}`, `{{date}}` and `{{time}}` (dates from the locale).
  The Journal's daily notes start from one template note already (`{{title}}`, `{{date}}`;
  `notav/core/src/journal.rs`): both should fill in the same way.
- **Links to Todav's tasks**: a note links a task as `todav://task/<uid>`. Todav opens it (it
  handles the `todav` URI scheme), and Notav shows the task's title and state through Todav's
  D-Bus API (`GetTask`, to be added in Todav). Without Todav the link stays a plain link. The
  other way too: "Send to Todav" on a task's line makes a task there through its D-Bus API
  (and leaves the link), which wants a `CreateTask` call in Todav.
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
- A three-way merge of a note changed here and on the server, from the text as of the last
  sync (`notes.base`), instead of a conflict copy.
- A shared crate, in its own repository, for the code copied from Todav (listed in
  `AGENTS.md`), once copying costs more than sharing: when other services rework the account
  layer (accounts are copied already), when a third app or a front-end of its own needs it, or
  when the same fix keeps being made twice.

## Android

The Android app (`notav/android`) still lacks, next:

- Release on F-Droid (after three months of history, from about 2027-01-24; a recipe as
  Todav's, its build matching our signed APK) and Google Play, as Todav; screenshots in the
  fastlane metadata.
- A wide table or code block edited unwrapped, scrolled to the cursor, as on Linux: with the
  cursor in one it wraps now, the strip over it showing it only. Its strip would be a text field
  of its own, its changes put into the note's.
- List items moved by dragging, as on Linux: a long press on an item's marker picks it up, the
  core's `markdown::list_at` and `move_item` (exported through the bindings) placing it.
- Preview mode as on Linux: the core's `markdown::html` (exported through the bindings) in a
  `WebView`, with the Linux app's stylesheet (`preview.css`), links opening notes, the third of
  the mode buttons.
- Paste as on Linux: HTML (`ClipData`'s HTML text) as Markdown with the core's
  `html::to_markdown`, spreadsheet rows as a table (`table::from_tsv`), both exported through
  the bindings; Compose's text field pastes plain text now, so this needs its own paste.

- What Linux gets above, in the same words, once built: the quick switcher, the slash menu, the
  notes' menu's items (Share through the share sheet too), link preview (a sheet), excerpts in
  the tree, extract and merge, saved searches; and the editor's keys that apply to a keyboard.
- App lock: the app asks for the device's biometrics or PIN to open (Joplin #289), a setting on
  this device only.

Later: dragging notes and folders in the tree (a long press opens their menu now, with Move…),
table handles and menus, a table's cells keeping their padding as they are typed in
(`table::padding_taken`), the text's size, widgets, a Quick Settings tile, launcher shortcuts
(New Note, Today's Note; the Linux app's desktop entry actions), split view (a wide screen's
two panes already show the tree and the note).

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
  Tables (and code blocks) wider than the text column do not wrap: each scrolls sideways on
  its own, in a second view of the note's text over its lines (`strips.rs`), which costs a
  layout of the whole note per wide block on screen. Without Wrap Lines the whole text scrolls
  instead. Tidying changes the file's text, which other devices and apps see.

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
- **Embeds**: `![[note]]` showing another note's text in place (Obsidian) needs the text
  drawn from elsewhere and not editable there; maybe with Preview mode only, as with images
  (see Attachments).
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

- **On Android, "Show outline automatically"**: the outline is a swipe away, never hidden beside
  the note; and **a font for Source mode**: Android has one monospace font.
- **A background service** (autostart, `HOLD`) in v1: notes have no reminders, so the app syncs
  only while it is open (decided 2026-10-24).
- **The first line as the title**: the title is the file name, as in Obsidian; Nextcloud Notes
  renames files after their first line, Notav does not.
- **Editor styles** not taken, for now: a monospace font by default (Apostrophe's Fira Mono,
  Iotas's; Preferences can choose one), a font size that grows with the window (Apostrophe), the
  `#` marks hanging in the margin (Iotas).
- **Export to PDF, ODT or HTML** (Iotas, Typora): the notes are Markdown files any app opens
  already (Preview mode shows them rendered).
- **Colours in fenced code blocks** by their language (Obsidian, Bear): GtkSourceView
  highlights one language per buffer, the note's.
- **Folding headings** (Obsidian, Logseq), for now: it hides text, which the `invisible` tag
  bug blocks (see [Open questions](#open-questions-and-risks)).
- **Encrypting notes** (a password per note, as Joplin's and Standard Notes'; Nextcloud's
  end-to-end encrypted folders, nextcloud/notes #476): the files stop being Markdown other apps
  open, and Nextcloud's encrypted folders cannot be reached over WebDAV.
- **Block references** (`^id`, Obsidian and Logseq): they write ids into the notes' files.
- **Diagrams** (Mermaid) and **fetching a link's title when a URL is pasted**: the first needs
  a renderer beyond Preview mode's page, the second a network request on every paste.
- **A graph view** (Obsidian): heavy in GTK for what the side panel's Backlinks and Outgoing
  links already answer; a local graph could follow them if asked for.
