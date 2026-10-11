# Intent: Completion opens as you type

Type: change
Author: Cody · Status: accepted
Date: 2026-10-09 · Plan: [Phase 17](../../PLANNING.md)

## Problem

"how come i have the typescript language server, but not all options show up? like
console.log() should be a thing." The popup shows the first ten matches and can't
scroll, so anything after them can't be reached.

"if i do l it doesnt even give me autocomplete i thought it would?" After `console.`
the popup had closed, and typing `l` showed nothing. Completion only opens after a
character the server names (`.` in TypeScript) or Alt+/, as v1's R33 says; a letter
only filters a popup that is already open. This change overrides that clause of
[R33](../tinkery-v1/spec.md).

## Proposed outcome

Typing a word opens the completion popup by itself, from its first letter, as VS Code
does, and it can be turned off. The list scrolls to every match.

## Affected users and systems

- Cody, in every language with a language server.
- `src/app.rs` (typed text), `src/lsp/` (what the server offers), `src/ui/completion.rs`
  (the popup), `src/config.rs` (the setting), `tests/lsp.rs`.

## Constraints

- Any letter or `_` typed in the editor with the popup closed asks the server; that
  includes the next letter after Esc (decided 2026-10-09).
- Enter and Tab both accept, whichever way the popup opened (decided 2026-10-09).
- `[editor] auto_complete`, on by default; off is the old behaviour (decided 2026-10-09).
- The popup shows ten rows at most; ↑↓ scroll through every match (decided 2026-10-09).
- Trigger characters, Alt+/, the arrows, Esc and the popup's look don't change.
- A missing or slow server never blocks typing (R29).

## Out of scope

- Tinkery deciding by syntax where completion makes sense (strings, comments): the server
  answers or doesn't.
- Asking again when a server says its list is incomplete; a minimum number of letters;
  fuzzy matching.

## Open questions

## Tickets

#201, #202, #203
