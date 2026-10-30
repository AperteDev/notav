# Notav — Features

What Notav can do, in short. The details of each release are in the
[changelog](CHANGELOG.md), the plans in the [roadmap](ROADMAP.md).

## Now

**Notes and sync**

- Notes are Markdown files in a folder of your Nextcloud Files (`Notes/` by default, as the
  Nextcloud Notes app); subfolders are folders in Notav too.
- Sign in with GNOME Online Accounts, the browser, or a login name and app password.
- Works from a local copy: offline too; changes go up when the server is reachable.
- A note changed on two devices at once keeps both versions (the other as a conflict copy).
- A note locked on the server (open in Nextcloud Text) waits with its change; Notav says who
  locked it and can unlock it.
- Settings synced between your devices.

**Organizing**

- Folders and notes as a tree, as in Obsidian; each folder shows how many notes it holds.
- Favorites, shared with Nextcloud Files and Notes: starred notes come first in their folder.
- Drag notes and folders into and out of folders.
- New notes and folders, renaming folders and deleting both from the tree's menu; a note is
  renamed through its title.
- Undo deleting, moving and renaming (the toast's Undo, or Ctrl+Z).
- Trash: restore deleted notes from Nextcloud's trash bin, or delete them for good.
- Versions of a note, from Nextcloud's file versions, to look at and restore.
- Search through titles and text, ignoring case and accents.
- Journal: Today's Note (Ctrl+T), made from a template note, in a folder and name format of your
  choice.

**Editing**

- Saves as you type; the title is the file name.
- Editor mode: Markdown styled as it reads (headings, emphasis, code, quotes, lists, tasks,
  tables, links, footnotes, front matter), the marks dimmed. Source mode: plain Markdown.
- Click a task's box to tick it; Enter continues a list.
- Ctrl+click opens a link (Alt+Enter the one at the cursor).
- Links between notes: Markdown links and `[[wiki links]]`, to a heading too; a missing note is
  made when its link is followed.
- Backlinks under each note.
- Outline of the note's headings in a side panel; a click goes to one.

**Tables**

- Kept aligned as you type and move around.
- Enter after a header line (`| Name | Size |`) starts a table.
- Tab, Shift+Tab and Enter move between cells, adding rows past the last.
- Rows and columns added, moved, duplicated, deleted, aligned and sorted from the right-click
  menu or with handles, as in Obsidian.
- Rows copied from a spreadsheet are pasted as a table.

**Look and app**

- Text size (Ctrl+plus, Ctrl+minus, Ctrl+0), Wrap Lines and Full Width in the view menu.
- Fonts for Editor and Source mode; light, dark or the system's style.
- A dense tree, and icons on notes, folders or neither.
- English and Polish.
- Restart into a new version from a banner, when Notav is updated while it runs.

## Planned

- Use Notav without a server: notes on this device only.
- Notes in GNOME Shell's search.
- Emptying the Trash of notes at once; a version's changes shown against the note.
- Spell checking.
- Find and replace in a note (Ctrl+F); search filters, results opening at the match.
- `[[` suggesting notes as you type; links updated when a note is renamed or moved.
- Paste a URL over selected text to link it; wrap a selection in `*`, `_`, `` ` `` and the like
  by typing it.
- Ctrl+Enter ticks a task; Enter continues quotes and closes code fences.
- Tags, outgoing links and unlinked mentions in the side panel.
- Tabs, templates, sorting the tree, a journal calendar.
- Focus mode and word count.
- An editor toolbar; Markdown marks hidden outside the line being edited.
- Links to Todav's tasks.
- Changes from other devices within seconds (Nextcloud's Client Push).
