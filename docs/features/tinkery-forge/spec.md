# Spec: Tinkery Forge

Intent: [Tinkery Forge](intent.md) · Status: accepted · Date: 2026-10-11

Design source: seshi's code, `CydoEntis/seshi` at commit `e3e4e09` (v0.21.1). Paths under
"seshi" below are in that repo. Clone it read-only to look; copy the idea, not the file.

## Concerns

- **Seshi's guide and code disagree.** Popups have an accent border with the title set into
  it, not a filled title bar; toasts sit at the top right of the editor area. Decided: follow
  the code.
- **Nerd Font glyphs.** Pill ends are U+E0B6 and U+E0B4. A font without them shows boxes.
  Decided: `pill_caps` (on by default) swaps them for a space of the pill's colour; `mono`
  never uses them.
- **Light theme.** No light theme has been drawn in this style in Tinkery. Decided: both
  new themes pass a contrast test (F2) before anything else is built on them.
- **Tests pin the old look.** About 450 checks in 23 test files read colours, glyphs and
  positions; about 80 assume `hydra` is the default. Decided: each ticket updates the tests
  for the screen it changes; tests that read `hydra`'s colours keep `hydra` by naming it in
  their config rather than being rewritten to the new palette.
- **A timer.** Toasts and motion need one; Tinkery has none for drawing. Decided: motion is
  off in tests and off in `mono`, and nothing may depend on it.
- **Aurora's design tables.** A unit test reads the theme tables in
  `tinkery-aurora/design/README.md`, which stays as delivered. Decided: the two new themes
  are pinned by the table in this spec instead.

## Requirements

Theme
- **F1.** Themes `tinkery` and `tinkery-light` exist with the roles and syntax colours in
  "Palette" below. A value may move by the smallest step that makes F2 pass; the PR says
  which.
- **F2.** A contrast test, with seshi's pairs and minimums (seshi `src/theme.rs:434-523`),
  passes for `tinkery` and `tinkery-light`: text on bg 4.5, strong on bg 7.0, muted on bg
  and on surface 3.0, text on raised 4.5, text on raised2 4.0, strong on btn 4.5, accent on
  btn 3.0, acc_ink on accent 4.5, strong on hov 4.5, err on raised 3.0, ok and warn on
  surface 3.0.
- **F3.** `tinkery` is the theme when config names none, and the fallback when a theme or
  an override is bad; the message ends ", using tinkery". The `>settings` template lists
  both new themes and shows `# theme = "tinkery"`.

Kit (`src/ui/kit.rs`)
- **F4.** `ends`, `chip`, `pill`, `row_pill`, `strip`: filled runs whose ends are the cap
  glyphs in the fill colour on the ground, or a space of the fill colour when `pill_caps`
  is off or the theme is `mono` (seshi `src/client/hydra/card.rs:7-113`).
- **F5.** `button` in four kinds (seshi `src/client/design.rs:116-146`): primary (accent,
  acc_ink), danger (err darkened until its text reads at 4.5), normal (btn, strong, key in
  accent), ghost (surface, text). Label and key are bold. Hover turns normal and ghost to
  hov. In `mono`: primary and danger are reverse video, the others bold.
- **F6.** `keycap`, `keycaps`, `hints` (seshi `design.rs:149-185`): a key is a chip on btn
  in strong bold; a hint is a keycap, a space and the word in muted, three spaces apart.
- **F7.** `card` and `panel` (seshi `card.rs:116-298`, `popups.rs:38-54`): edges are flush
  (`▁ ▔ ▕ ▏`, blank corners) or rounded (`─ │ ╭ ╮ ╰ ╯`) by the `corners` setting; the
  title sits in the top edge; a panel is a centred card with an accent edge and title on
  raised. `dim` darkens the screen behind a panel: text 0.6 toward its ground, then
  everything 0.45 toward black, cap glyphs keeping their shape (seshi `popups.rs:8-26`).
- **F8.** `sel_row`: the selected row is a row pill in hov with `›` in accent bold and its
  label in strong bold; a hovered row gets the pill without `›`. Accent is never a row fill.
  In `mono` the selected row is reverse video.
- **F9.** Settings `[ui] pill_caps = true` and `[ui] corners = "flush" | "rounded"`, read
  from `config.toml`, applied on save, listed in the `>settings` template.

Screens. What each shows and where it sits does not change; only how it is drawn.
- **F10. Tree.** The panel is a card. The selected or active row is F8. The brand row keeps
  its orange-to-gold letters. No glow.
- **F11. Tabs.** Each tab is a pill: the current one accent with acc_ink bold text, the
  others on raised with text, a dirty `•` in warn. The aurora thread is gone. An unfocused
  split's current tab is btn with strong text.
- **F12. Status bar.** The brand block keeps its ramp. The rest is flat on surface; key
  hints on the splash variant are keycaps (F6).
- **F13. Editor.** The cursor line is flat `cur_line` with its number in accent bold; the
  paused line is flat too, mixed 0.12 toward accent2. Selection, find matches, diagnostics
  and the lens do not change.
- **F14. Bars.** Find, replace and prompt bars are a strip (F4) on raised2; the `Aa` and
  `.*` toggles are chips, on = accent with acc_ink, off = btn with muted; hints are keycaps.
- **F15. Lists in a panel.** The cast palette, run picker, folder browser and project
  search are panels (F7) with a query strip, F8 rows, and a hints line ending `Esc close`.
- **F16. Confirm cards.** A panel; buttons are F5 with the action first (danger when it
  loses work, else primary) and `Cancel Esc` second.
- **F17. Popups.** Hover and completion are cards on raised, not dimmed; the selected
  completion row is F8.
- **F18. Panels.** Run and debug panels: the title row's hints are keycaps; selected stack
  and variable rows are F8.
- **F19. Cards of rows.** The language-server catalog and the keybindings card are panels
  with F8 rows; every key shown is a keycap.
- **F20. Splash and no file open.** The three splash actions are buttons (F5) with their
  keys as keycaps; the wordmark and rule keep the ramp; the glow behind the wordmark is
  gone. The no-file key list uses keycaps.

Behaviour
- **F21. Hover.** Moving the mouse lights the row, button, tab or chip under it (hov), on
  every screen above. Nothing else changes on hover.
- **F22. Toasts.** A note from an action (saved, failed, not found) is a pill on raised2 at
  the top right of the editor area: `✓` in ok or `✕` in err, the text in strong bold. It
  holds 2.5 s, or 4.5 s for an error, and a new one replaces it. The diagnostic under the
  cursor stays in the status bar.
- **F23. Motion.** The selection pill glides between rows in 90 ms; a toast slides in over
  120 ms and fades over 350 ms; movement is in whole cells, eased out. `[ui] motion = false`
  turns all of it off. It is off in `mono` and in tests: the environment variable
  `TINKERY_MOTION=0`, which the PTY harness sets, turns it off whatever the setting says.
- **F24. Cleanup.** No renderer calls `glow_row`, `dialog_card` or `draw_thread`; they and
  their tests are removed. The Aurora spec is marked superseded by this one.

## Palette

Roles in `Theme::signature` order, then `hov` and `btn`, which these two themes set apart
from `sel` and `raised2`.

| role | tinkery | tinkery-light |
|---|---|---|
| bg | #110f0e | #f5f0e9 |
| deep | #0a0908 | #e8e0d5 |
| surface | #171413 | #ede6dc |
| raised | #1f1b19 | #e4dcd0 |
| raised2 | #292320 | #d9cfc2 |
| line2 | #40362f | #c0b3a3 |
| guide | #251f1c | #ddd4c7 |
| muted | #7d7067 | #8b7e71 |
| text | #b5a89d | #5b5047 |
| fg | #dcd2c7 | #3a322c |
| strong | #f8f0e6 | #1c1714 |
| accent | #ff8a3d | #b84a08 |
| accent2 | #ffd27a | #b07d0c |
| acc_ink | #1f0d02 | #fff6ec |
| err | #ff5d4d | #c0322b |
| warn | #f2c14e | #9a6a00 |
| ok | #8fc7a2 | #3f7d5a |
| info | #8fb4d6 | #3c6a94 |
| err_soft | #a8483c | #d99a93 |
| warn_soft | #a08544 | #d3bb85 |
| info_soft | #5f7d99 | #9db7cf |
| sel | #3b2514 | #f2d3b5 |
| cur_line | #1a1614 | #ece4d8 |
| gutter | #4d423b | #a99c8d |
| scrim | #000000 | #1c1714 |
| hov | #3a302a | #d3c7b8 |
| btn | #332a25 | #dcd2c5 |
| syn.keyword | #ff8a3d | #b84a08 |
| syn.string | #e6c98a | #8a6a12 |
| syn.comment | #6e625a | #9b8e81 |
| syn.function | #f8ead6 | #1c1714 |
| syn.type | #8fb4d6 | #3c6a94 |
| syn.number | #ffb86b | #a85a12 |
| syn.constant | #ffb86b | #a85a12 |
| syn.operator | #9c8e83 | #7b6f63 |
| syn.punctuation | #776a60 | #958879 |
| syn.variable | #dcd2c7 | #3a322c |
| syn.property | #c7b6a5 | #5f5249 |
| syn.tag | #ff8a3d | #b84a08 |
| syn.attribute | #9c8e83 | #7b6f63 |

## Checks

- Every screen keeps a PTY test that runs the real `tk`; a ticket that changes a screen
  updates that screen's tests in the same change.
- Person check, at 160×45 in Windows Terminal and inside seshi, in `tinkery`,
  `tinkery-light` and `mono`: open a project; walk the tree, tabs, find and replace,
  Ctrl+P and its modes, project search, a confirm card, hover and completion, the run and
  debug panels, the catalog, the keybindings card and the splash. Each matches this spec,
  nothing is unreadable, and with `pill_caps = false` no box glyphs appear.

## Coverage

| Requirement | Ticket |
|---|---|
| F1, F2 | #211 |
| F3 | #212 |
| F4, F6, F9 (pill_caps), F14 | #213 |
| F5, F7, F9 (corners), F16 | #214 |
| F8, F15 | #215, #216 |
| F19 | #217 |
| F17 | #218 |
| F10 | #219 |
| F11 | #220 |
| F12 | #221 |
| F13 | #222 |
| F18 | #223 |
| F20 | #224 |
| F21 | #225 |
| F22 | #226 |
| F23 | #227 |
| F24 | #228 |
