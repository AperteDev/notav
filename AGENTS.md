# Agent notes

## Architecture

- `docs/ARCHITECTURE.md` holds the architecture, the design decisions and the plan (progress,
  open questions). Read it before working on a feature, and update it in the same change when
  the work changes what it says: a decision made, a step done, a plan dropped.

## Todav

- Notav is the sibling of Todav (`SelfRef/todav`, cloned next to this repository) and follows
  its conventions; when something here is not decided yet, look at how Todav does it (its
  `AGENTS.md` and `docs/ARCHITECTURE.md`).
- Some code here is copied from Todav (listed in "Code from Todav" in `docs/ARCHITECTURE.md`).
  A fix to one of those probably applies there too: port it or say so.
- The two apps keep separate configuration and credentials: never read or write Todav's
  files, keyring entries, settings or database. Talk to Todav only through its D-Bus API and
  `todav://` links.

## Releases

- Add user-visible changes to `CHANGELOG.md` under `## [Unreleased]`, in the same commit as the
  change. It follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/): `Added`,
  `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security`.
- Don't bump versions by hand: releases are cut by the manual Release workflow
  (`.github/workflows/release.yml`), as in Todav: `scripts/release.py` sets the version in
  `Cargo.toml`, `Cargo.lock` and the metainfo and moves the `[Unreleased]` changes under it.
- CI (`.github/workflows/ci.yml`) runs on every push: `cargo fmt --check`, clippy with warnings
  as errors, the unit tests, and the packages (Flatpak, tarball, `.deb`). Run the checks before
  committing.

## Translations

- All text a user sees is translatable; one gettext domain, `notav`, with catalogs in `po/`.
  The rules are Todav's (see its `AGENTS.md`): named placeholders, plurals with `ngettext`,
  whole sentences, Polish translated for review, dates from the locale.

## Linux UI

- Static views are Blueprint files in `linux/app/ui/`, compiled by `build.rs`; dynamic UI (note
  rows) stays in Rust code.

## Testing

- Unit tests: `cargo test --workspace`. Against the server: `cargo test -p notav-core --features
  integration`, with the Test account from the gitignored `.env` (`NOTAV_URL`, `NOTAV_USER`,
  `NOTAV_PASS`, as in Todav); each test works in its own `it-notav/<name>/` folder with settings
  sync off. Leave the Test account's synced `notes_folder` at `Notes` when done.
- Nextcloud derives a file's ETag from its modification time in whole seconds: two writes in one
  second keep it. Keep `X-OC-MTime` on uploads (see "ETags are times" in
  `docs/ARCHITECTURE.md`).
- The app, as in Todav: `make -C linux sandbox` runs a separate copy (`NOTAV_SANDBOX`, data in
  `target/sandbox`); `make -C linux broadway` runs it in a browser on display `:6`
  (`http://127.0.0.1:8086`, Todav uses `:5`). Todav's notes on Broadway's limits apply. Closing
  the window quits the app (after the sync), so a new launch starts fresh.

