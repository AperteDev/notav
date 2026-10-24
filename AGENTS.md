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
- Don't bump versions by hand; releases will be cut by a Release workflow, as in Todav.

## Translations

- All text a user sees is translatable; one gettext domain, `notav`, with catalogs in `po/`.
  The rules are Todav's (see its `AGENTS.md`): named placeholders, plurals with `ngettext`,
  whole sentences, Polish translated for review, dates from the locale.
