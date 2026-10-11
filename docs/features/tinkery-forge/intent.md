# Intent: Tinkery Forge

Type: change
Author: Cody · Status: accepted
Date: 2026-10-11 · Plan: [Phase 19](../../PLANNING.md)

## Problem

"We need a tinkery theme for the app that is the default and light a dark mode, dwarvern
steal, regal crafting orange fires from the forge." And: "are you able to look at how
seshi's ui is made and make tinkery have the same UI style?"

Tinkery wears the Aurora look (glowing rows, colour ramps, a lit edge on dialogs) and its
default theme is `hydra`, another app's palette. Seshi, the app Tinkery runs inside,
draws flat pills, keycaps, cards and hint lines. The two don't look like one family, and
Tinkery has no colours of its own. This overrides [Tinkery Aurora](../tinkery-aurora/intent.md),
including its "the default theme stays `hydra`" (decided 2026-10-07).

## Proposed outcome

Every screen Tinkery draws uses seshi's style: pills for anything you click, keycaps and
hint lines for keys, cards with flush or rounded edges, a selected row as a quiet pill
with `›`. Its default theme is `tinkery`, the forge at night, with `tinkery-light` for
daytime. Rows light up under the mouse, short notes arrive as toasts, and the selection
glides.

## Affected users and systems

- Cody, on Windows and Ubuntu, in Windows Terminal and inside seshi.
- `src/theme.rs`, every file in `src/ui/`, the gutter and cursor line in `src/view/`,
  the frame in `src/app.rs`, `src/config.rs`, the PTY tests that pin the look, and the docs.

## Constraints

- Full re-skin: the Aurora look is retired (decided 2026-10-11). The design source is
  seshi's code at `CydoEntis/seshi` commit `e3e4e09` (v0.21.1); where its UI guide and its
  code disagree, the code wins.
- The orange-to-gold ramp stays in three brand spots only: the splash wordmark, the
  status-bar brand block and the tree's `✦ tinkery`. Rows, tabs, dialogs and the cursor
  line are flat (decided 2026-10-11).
- `tinkery` is the default theme; `tinkery-light` is chosen in config. All 13 existing
  themes stay and work in the new chrome.
- `mono` keeps working with terminal colours only: no blends, and no Nerd Font glyphs.
- In with the re-skin (decided 2026-10-11): mouse hover states; settings for round pill
  ends and for flush or rounded card edges; toasts; motion with one off switch.
- Built before Phases 16 (C and C++) and 17 (completion) (decided 2026-10-11).
- No code is shared with seshi; Tinkery gets its own copies.
- Keys reach actions only through the keymap (R5); rendering stays a pure function of `App`.

## Out of scope

- Seshi's three themes (Walnut, Night, Day) in Tinkery's list.
- Right-click menus.
- Switching between `tinkery` and `tinkery-light` by time of day or by the terminal.
- Layout changes: what is on each screen and where it lives stays as it is.

## Open questions

## Tickets

#211, #212, #213, #214, #215, #216, #217, #218, #219, #220, #221, #222, #223, #224, #225, #226,
#227, #228
