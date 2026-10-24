# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Notes kept as Markdown files in a folder of your Nextcloud Files (`Notes/` by default, as the
  Nextcloud Notes app), synced over WebDAV; Notav works from a local copy, also offline
- Sign-in through GNOME Online Accounts, the browser, or a login name and app password, then
  the notes folder to use
- The notes folder as a tree of folders and notes in the sidebar, as in Obsidian
- Search through titles and text, ignoring case and accents
- Markdown editor that saves as you type; the title renames the note; move to a folder, delete
- A note changed on two devices at once keeps both versions: yours as a conflict copy next to it
- Preferences: style, notes folder (moving the notes there or not), account, settings sync
- Settings synced between devices through `.config/notav/settings.json`
- Polish translation

[Unreleased]: https://github.com/SelfRef/notav/commits/HEAD
