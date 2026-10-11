# Intent: Tome becomes Tinkery

Type: change
Author: Cody · Status: accepted
Date: 2026-10-11 · Plan: [Phase 18](../../PLANNING.md)

## Problem

"This is a minimal no ai text editor going back to the old day of hand crafting code."
"Can we go with tinkery and tk for the command so its easy to write." The editor is
called Tome everywhere: the command you type, the splash wordmark, the status line,
the config folder, `.tome.toml`, the docs. It has been renamed twice, Griffin to Glyph
(#79) and Glyph to Tome (#195-#197, 2026-10-09). This overrides
[Glyph becomes Tome](../tome-rename/intent.md).

## Proposed outcome

The editor is Tinkery and you start it with `tk`: the splash wordmark reads `tinkery`,
the UI and docs say Tinkery, settings live in `<config dir>/tinkery/config.toml`,
backups in `<data dir>/tinkery`, a project's file is `.tinkery.toml`, and the repo is
`CydoEntis/tinkery`.

## Affected users and systems

- Cody, on Windows and Ubuntu.
- `Cargo.toml`, `Cargo.lock`, `src/` (CLI name, config, backups, clipboard, splash,
  every user-facing string), `tests/` and the harness, `README.md`, `AGENTS.md`, `docs/`.

## Constraints

- One binary, `tk` (decided 2026-10-11). The package is `tinkery`; no `tinkery` command.
- Clean break (decided 2026-10-11): Tinkery never reads or moves `.tome.toml` or the
  `tome` config and data folders.
- Environment variables become `TINKERY_CONFIG`, `TINKERY_DATA_DIR`,
  `TINKERY_CLIPBOARD_FILE`.
- Feature doc folders `docs/features/tome-*` become `tinkery-*` with every link updated,
  except `tome-rename`, which records the earlier rename and keeps its name. Design
  source files under `docs/features/*/design/` keep their names and contents.
- The GitHub repo is renamed to `tinkery` once the tickets land (decided 2026-10-11).
- Lands before Phase 16 (C and C++) and the completion tickets #201-#203 are built
  (decided 2026-10-11).

## Out of scope

- Reading, moving or warning about old Tome files.
- Publishing anywhere: "we dont need to publish anywhere since this is just a public
  repo i made to use myself."
- Renaming local checkout folders (`C:\dev\glyph`, worktrees).
- Removing the installed `tome.exe`.

## Open questions

## Tickets

#204, #205, #206
