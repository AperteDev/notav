# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Text copied from a web page, an email or a document is pasted in Editor mode as Markdown:
  headings, bold and italic, links, lists, quotes, code, images and tables kept, the page's
  layout left out (not in a table or a code block). Ctrl+Shift+V pastes the text as it is.
- Preview mode, the third of the note's modes beside Editor and Source: the note rendered as
  GitHub renders Markdown (alerts, tables, tasks, footnotes, bare URLs linked), read only, in
  the light or dark style and the note's fonts and size; its links open notes or the browser,
  the outline goes to its headings. Notav now needs WebKitGTK 6.0.

## [0.5.1] - 2026-11-07

### Added

- Android: an account of the Nextcloud app on the device fills in the server and login name
  of a sign-in.

## [0.5.0] - 2026-11-07

### Added

- An Android app (an APK with each release), in Material 3 Expressive: the same notes, accounts
  and synced settings. Sign in with the browser or an app password, or keep notes on this
  device; the tree of folders and notes, the note and its outline (with the notes linking to
  it) swiped between, one at a time on a phone, two side by side on wide screens (an unfolded
  foldable, a tablet); Editor mode's styles, ticking tasks, lists and tables
  (tables and code blocks unwrapped, each scrolled sideways on its own, shaded at the edge
  where more is hidden); find and replace;
  search, Today's note, text shared from other apps as a note; a note locked on the server
  told about, with Unlock…; the Trash and a note's versions. It syncs while open, when the
  tree is pulled down and once more when left; Settings can add a sync in the background,
  from every 15 minutes to once a day. Settings has the Linux app's pages: the style, the
  tree's density and icons, restoring the note left open, Editor mode's font, table
  formatting, the journal's daily notes, the accounts' order and each one's page (on or off,
  name, sign in again, notes folder, server status, remove), settings sync, and About with the
  Support links
- Tabs, as in Obsidian: a click on a note opens it in the selected tab, Ctrl+click or a middle
  click (or "Open in New Tab" in its menu) in a new one; the tabs show above the note once
  there are two. Each keeps where its cursor was; Ctrl+Page Down and Ctrl+Page Up go through
  them, Ctrl+W closes one (the window once none is left)
- Restore Session, on by default (Preferences, General): the tabs open when Notav closed, and
  the one selected, open again as it starts; kept for each device
- GitHub's alerts in Editor mode: a quote starting with `[!NOTE]`, `[!TIP]`, `[!IMPORTANT]`,
  `[!WARNING]` or `[!CAUTION]` is a box tinted in its colour (blue, green, purple, yellow, red),
  its `[!NOTE]` and `>` bold in that colour; light and dark
- Enter in a quote (an alert too) starts the next line with its `>`; on an empty quote line it
  leaves the quote, with a blank line after it
- Find and replace in the open note: Ctrl+F (or the Find in Note button) opens a bar above it
  that highlights the matches and counts them ("2 of 11"), Enter and Shift+Enter (or Ctrl+G
  and Ctrl+Shift+G) go through them; Ctrl+H adds a row to replace one or all. Escape goes back
  to the note
- Back from the Notes Folder step of adding an account (on the welcome page too): the account
  just connected goes, to connect another
- The outline shows by itself for a note with two headings or more, and hides for others,
  while it is not switched on with its button; at once, without sliding. Preferences, Editor →
  Outline turns it off
- The welcome page waits for the first sync, with how many notes it has downloaded, and
  shows the notes once they are all here
- A grip beside the list item under the pointer in Editor mode, as on GitHub: dragging it moves
  the item, its sub-items with it, among the items of its list (an ordered list's numbers stay
  in order)

### Changed

- Tables and code blocks wider than the note scroll sideways each on its own, the note keeping
  its width (it widened for them, and the whole text scrolled sideways): edited in place, they
  follow the cursor; a shade at an edge shows that more of one is hidden that way; their
  scrollbar stays at the bottom of the window while they go on below
  it, and a touchpad's sideways move or Shift and the wheel over them scrolls them too. Without
  Wrap Lines the whole text still scrolls
- The sidebar, as in Todav: the Toggle Sidebar button (F9) hides and shows it; on a narrow
  window it hides by itself and shows over the note, going once a note is picked
- Searching all notes is in the sidebar, with its button at the top, and Ctrl+Shift+F (Ctrl+F
  finds in the open note; with no note open it still searches all notes). The notes found show
  in the tree's place, in their folders, with how many each folder holds; a click or Enter
  opens one with the search left open for the next. Closed, the tree is as it was left, its
  folders and scroll unchanged
- Syncing is faster when many notes changed on the server: they download six at a time, and
  the first sync lists the notes folder in one request. A first sync of 277 notes in 61
  folders takes 5 s instead of 27

### Fixed

- A table's row with "fi" (or another ligature of the font) in a cell no longer goes out of
  line from there on in Editor mode: tables and code are shown without ligatures
- A note changed while it is open in Nextcloud Text waits for its lock and goes up once
  unlocked, instead of becoming a conflict copy: Nextcloud gives a file a new ETag as it is
  locked and unlocked
- A narrow window gives the note all its width: the sidebar never went away there, the outline
  folding instead
- A table's handles show as the pointer comes near the table from outside it too: beside a
  row's first pipe (its row grip there), past the last column or under the last row (the bars
  that add one), not only once it has been over the table's text
- The bar that adds a row no longer covers the line under a table: with text right after the
  table, Editor mode leaves room under it
- A table wider than the note no longer wraps out of its columns in Editor mode, nor does a
  code block's line: the note widens to show them whole, as far as the window goes, and only
  past that the text scrolls sideways (its scrollbar under the note, as without Wrap Lines); a
  table's handles stay on it
- The note's text no longer moves sideways a little as a table's handles show
- Deleting in a table's cell (Backspace, Delete) no longer pulls the pipes after it to the
  left: what goes is given back as the cell's padding, as typing takes it
- A task's empty box `[ ]` is as wide as a ticked one `[x]`, in monospace in Editor mode
- Opening or closing a folder in the tree no longer scrolls it to the top; the folder keeps
  the keyboard focus
- The outline says "No Headings" for a note without headings, instead of staying blank once
  a note was open
- A server whose certificate comes from a certificate authority added to the system (a
  self-hosted Nextcloud's own CA) connects, instead of failing with "invalid peer certificate:
  UnknownIssuer": Notav trusts the system's certificate authorities, not only its built-in list
- An error page from the server or a proxy shows as its text in error messages, not as its
  HTML

## [0.2.0] - 2026-10-30

### Added

- Accounts, as in Todav: notes of more than one Nextcloud account, and On This Device Only,
  notes kept on this computer without a server. The welcome page offers a GNOME Online
  Accounts account, a Nextcloud server or this device; Preferences → Accounts adds more, each
  with its page (use it or not, its name, signing in again, its server, notes folder and
  status, Remove). With more than one shown, each account is a row of the tree with its notes
  under it
- A dense tree in Preferences: smaller rows, as in Files
- Favorites, shared with Nextcloud Files and the Nextcloud Notes app: "Add to Favorites" in a
  note's menu stars it; starred notes come first in their folder, with a star
- Trash in the main menu: the notes and folders deleted from the notes folder that Nextcloud's
  trash bin keeps, to restore or delete for good; only where the server has a trash bin, and
  deleting for good only where it allows that
- Versions in a note's menu: the earlier versions Nextcloud keeps of it, each to look at and
  restore (an edit Ctrl+Z undoes); only where the server keeps versions
- When Notav is updated while it runs, a banner offers Restart into the new version (as in
  Todav)
- Folders in the tree show how many notes they hold, their folders' included, dimmed
- An outline of the open note's headings at its right, shown and hidden with the Outline
  button; a click goes to the heading
- Note icons in the tree, set in Preferences: on notes only (the default), on folders too, or none
- Rename and delete folders from the sidebar menu; a note changed on another device meanwhile
  is kept, and its folder with it
- Undo for deleted, moved and renamed notes and folders: the Undo button on the toast, or
  Ctrl+Z
- Editor mode, the new default: every part of GitHub Flavored Markdown styled as it is meant
  (headings, emphasis, code, quotes, lists and tasks, tables in columns, links and bare URLs,
  footnotes, front matter), the Markdown marks dimmed. Source mode keeps the editor as it was;
  the two toggles at the header's right switch between them
- A view menu next to them: the text's size (also Ctrl+plus, Ctrl+minus, Ctrl+0) and Full
  Width, the text across the window but for a small margin, and Wrap Lines (on by default; off,
  long lines scroll sideways); kept on each device
- Each mode's font in Preferences, under Editor, for each device: Source mode's is monospace
  (the system's by default, and only monospace ones are offered)
- In Editor mode, a click on a task's `[ ]` or `[x]` ticks or unticks it
- In Editor mode, Enter in a list starts the next item: the same bullet, the next number (the
  numbers below moved up), an open task after a task; Enter on an empty item ends the list
- In Editor mode, Ctrl+click opens a link, an autolink or a bare URL; Alt+Enter the one at the
  cursor
- Links between notes: a Markdown link to a note (`[text](Other.md)`, `../Folder/Note.md`) or a
  wiki link (`[[Note]]`, `[[Folder/Note|shown]]`, by title or path, as in Obsidian) opens the
  note, at the heading after a `#`; a note a link names that is not there yet is made
- Backlinks: under a note, the notes that link to it, each opened with a click
- Journal: Today's Note in the header (Ctrl+T) opens the day's note, made from a template note
  if there is none (its `{{title}}` and `{{date}}` filled in); its folder, template and name
  format (`YYYY-MM-DD` by default, `/` for folders) are set on a Journal page in Preferences,
  the folder and template fields suggesting what there is
- Tables in Editor mode: kept aligned (the pipes lined up, the alignment in the `|---|` row)
  once the cursor leaves a changed row; from the right-click menu, insert a table, add, move,
  duplicate and delete rows and columns, align a column or sort by it; rows copied from a
  spreadsheet are pasted as a table
- Table handles in Editor mode, as in Obsidian: over a table, a grip in the column (on the
  `|---|` row) and on the row's left edge opens its menu or, dragged, moves it; bars past the last column
  and under the last row add one
- Tables from the keyboard in Editor mode: Enter after a header line (`| Name | Size |`) makes
  it a table with an empty row; in a table, Tab and Shift+Tab go from cell to cell and Enter to
  the next row, past the last one adding a row, and Enter on an empty last row leaves the table;
  text typed in a cell takes the place of its padding, so the pipes stay in line
- An Editor page in Preferences for what Editor mode does by itself: "Format tables", under
  Markdown, turns off the tables' tidying as the cursor leaves a row (Format Table in the menu
  still works)

### Changed

- Deleting a note no longer asks first: the toast can undo it
- The notes folder is set on its account's page in Preferences, on each device; the one of the
  account settings sync through is offered to other devices signing in to it
- Settings sync through the account chosen in "Sync settings with" (Preferences → Accounts), or
  stay on this device; Remove Account takes the place of Sign Out
- An older Notav no longer opens data a newer one left (after a downgrade, say): it shows a
  window saying to update, and leaves the data as it is

### Fixed

- Sync stopping with "UNIQUE constraint failed: notes.fileid" after a note was moved and a
  new one took its old name before the next sync, and the new note not showing on other devices
- A folder moved before signing out or into another account, and not synced yet, no longer
  moves a folder of that name in the next account's files
- Screen readers name the notes in the sidebar, not only the folders
- Clicking into a long note no longer scrolls it to its end; the note follows the cursor, and
  Page Up and Page Down move by a screen, not to the start or the end

## [0.1.0] - 2026-10-25

### Added

- Notes kept as Markdown files in a folder of your Nextcloud Files (`Notes/` by default, as the
  Nextcloud Notes app), synced over WebDAV; Notav works from a local copy, also offline
- Sign-in through GNOME Online Accounts, the browser, or a login name and app password, then
  the notes folder to use
- The notes folder as a tree of folders and notes in the sidebar, as in Obsidian
- Search through titles and text, ignoring case and accents
- Markdown editor that saves as you type, styled for reading: headings sized by level, the
  Markdown marks dimmed, a line of about 80 characters; the title renames the note
- Drag notes and folders into and out of folders in the sidebar
- Sidebar menu (right click): new note or folder inside a folder, next to a note, or at the top
  level; delete a note
- A note changed on two devices at once keeps both versions: yours as a conflict copy next to it
- A note locked on the server (open in Nextcloud Text) waits with its change and the rest syncs;
  the app says who locked it and since when, and can unlock it
- Preferences: style, notes folder (moving the notes there or not), account, settings sync
- Settings synced between devices through `.config/notav/settings.json`
- Polish translation
- About has a Support page with where to support Notav's development: Ko-fi, GitHub Sponsors
  and Liberapay
- An RPM spec for Fedora, built on COPR from `.copr/Makefile`; Notav uses the system's SQLite

[Unreleased]: https://github.com/AperteDev/notav/compare/v0.5.1...HEAD
[0.5.1]: https://github.com/AperteDev/notav/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/AperteDev/notav/compare/v0.2.0...v0.5.0
[0.2.0]: https://github.com/AperteDev/notav/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/AperteDev/notav/releases/tag/v0.1.0
