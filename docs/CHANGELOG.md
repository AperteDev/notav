# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/AperteDev/notav/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/AperteDev/notav/releases/tag/v0.1.0
