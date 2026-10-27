# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- A dense tree in Preferences: smaller rows, as in Files
- Note icons in the tree, set in Preferences: on notes only (the default), on folders too, or none
- Rename and delete folders from the sidebar menu; a note changed on another device meanwhile
  is kept, and its folder with it
- Undo for deleted, moved and renamed notes and folders: the Undo button on the toast, or
  Ctrl+Z
- Editor mode, the new default: every part of GitHub Flavored Markdown styled as it is meant
  (headings, emphasis, code, quotes, lists and tasks, tables in columns, links and bare URLs,
  footnotes, front matter), the Markdown marks dimmed. Source mode, in the main menu, keeps the
  editor as it was

### Changed

- Deleting a note no longer asks first: the toast can undo it

### Fixed

- Sync stopping with "UNIQUE constraint failed: notes.fileid" after a note was moved and a
  new one took its old name before the next sync, and the new note not showing on other devices
- A folder moved before signing out or into another account, and not synced yet, no longer
  moves a folder of that name in the next account's files
- Screen readers name the notes in the sidebar, not only the folders

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
