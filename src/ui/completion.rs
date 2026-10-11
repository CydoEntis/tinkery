//! The completion popup: what the language server offered for the word at the
//! cursor, filtered by what has been typed of it since, in a small card anchored
//! at the cursor like the hover's.

use std::ops::Range;

use ratatui::Frame;
use ratatui::layout::Rect;
use ropey::Rope;
use unicode_width::UnicodeWidthStr;

use super::hover::{anchor, popup_card};
use super::kit::{self, Look};
use crate::lsp::CompletionItem;
use crate::theme::Theme;

/// Most items the popup shows at once.
pub const MAX_ITEMS: usize = 10;
/// Widest a row's text gets inside the card.
const MAX_TEXT_WIDTH: usize = 60;
/// How far into a row its text starts: the row pill's end, the mark of the
/// selected row and a blank.
const MARK: u16 = 3;
/// The card's edge and `MARK` before a row's text.
const LEFT: u16 = 1 + MARK;
/// The row pill's other end and the card's edge after it.
const RIGHT: u16 = 2;

/// How wide the kinds' column is for `shown`, and the blank after it: none
/// when no item has a kind.
fn kind_column(shown: &[&CompletionItem]) -> (usize, usize) {
    let width = shown.iter().map(|i| i.kind.width()).max().unwrap_or(0);
    (width, usize::from(width > 0))
}

/// Whether `c` belongs to the word being completed.
pub fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Where the word ending at char index `cursor` starts: back over word chars.
pub fn word_start(rope: &Rope, cursor: usize) -> usize {
    let mut start = cursor.min(rope.len_chars());
    while start > 0 && is_word_char(rope.char(start - 1)) {
        start -= 1;
    }
    start
}

/// The text typed of the word since `start`, when the cursor at `cursor` is
/// still on it: at or after `start` with only word chars between. `None` once it
/// has moved off.
pub fn typed_word(rope: &Rope, start: usize, cursor: usize) -> Option<String> {
    if cursor < start || cursor > rope.len_chars() {
        return None;
    }
    let typed = rope.slice(start..cursor).to_string();
    typed.chars().all(is_word_char).then_some(typed)
}

#[derive(Debug, Clone)]
pub struct Completion {
    /// The buffer it completes in.
    pub doc: u64,
    /// Where the word being completed starts, as a char index.
    pub start: usize,
    /// The cursor when the request went out, which the items' edit ranges are
    /// relative to.
    asked_at: usize,
    items: Vec<CompletionItem>,
    /// Index into the shown items.
    selected: usize,
}

impl Completion {
    pub fn new(doc: u64, start: usize, asked_at: usize, items: Vec<CompletionItem>) -> Self {
        Self {
            doc,
            start,
            asked_at,
            items,
            selected: 0,
        }
    }

    /// The items whose filter text starts with `typed`, ignoring case, at most
    /// `MAX_ITEMS`.
    pub fn shown(&self, typed: &str) -> Vec<&CompletionItem> {
        let typed = typed.to_lowercase();
        self.items
            .iter()
            .filter(|item| item.filter.to_lowercase().starts_with(&typed))
            .take(MAX_ITEMS)
            .collect()
    }

    pub fn selected(&self, typed: &str) -> Option<&CompletionItem> {
        let shown = self.shown(typed);
        shown
            .get(self.selected.min(shown.len().saturating_sub(1)))
            .copied()
    }

    /// Moves the selection one row, stopping at either end.
    pub fn step(&mut self, typed: &str, down: bool) {
        let last = self.shown(typed).len().saturating_sub(1);
        self.selected = if down {
            (self.selected + 1).min(last)
        } else {
            self.selected.min(last).saturating_sub(1)
        };
    }

    /// Typing changed what's shown, so the best match is selected again.
    pub fn reset(&mut self) {
        self.selected = 0;
    }

    /// The range accepting `item` replaces with the cursor at `cursor`: its
    /// `textEdit` range stretched over whatever was typed since the request, or
    /// else the word typed so far.
    pub fn replaced(&self, item: &CompletionItem, cursor: usize) -> Range<usize> {
        match &item.edit {
            Some(edit) => {
                // Text after the request's cursor that the edit covers is still
                // just after the cursor; typing only ever added before it.
                let after = edit.end.saturating_sub(self.asked_at);
                edit.start.min(cursor)..cursor + after
            }
            None => self.start.min(cursor)..cursor,
        }
    }

    /// The selected row, as a number that changes when the selection moves.
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// Where the card listing the items that match `typed` goes, anchored at
    /// the cursor cell `cursor` inside `bounds`: as wide as its widest kind,
    /// label and detail and a row per item. None when nothing matches or
    /// there's no room for a row, and so nothing is drawn.
    pub fn card(&self, typed: &str, cursor: (u16, u16), bounds: Rect) -> Option<Rect> {
        let shown = self.shown(typed);
        if shown.is_empty() {
            return None;
        }
        let (kind_width, gap) = kind_column(&shown);
        let label_width = shown.iter().map(|i| i.label.width()).max().unwrap_or(0);
        let detail_width = shown.iter().map(|i| i.detail.width()).max().unwrap_or(0);
        // Two blanks keep the detail apart from the longest label.
        let detail_room = if detail_width > 0 {
            detail_width + 2
        } else {
            0
        };
        let text_width = (kind_width + gap + label_width + detail_room).clamp(1, MAX_TEXT_WIDTH);
        let rows = u16::try_from(shown.len()).unwrap_or(u16::MAX);
        let width = u16::try_from(text_width).unwrap_or(u16::MAX);
        let card = anchor(cursor, width + LEFT + RIGHT, rows + 2, bounds);
        (card.width > LEFT + RIGHT && card.height > 2).then_some(card)
    }

    /// Draws the items matching `typed` in a card anchored at the cursor cell
    /// `cursor`, inside `bounds`: each row its kind in `muted`, its label in `fg`
    /// with the typed prefix in `accent` bold, and its detail right-aligned in
    /// `muted`. The selected row is the kit's: a pill with its mark, the label
    /// in `strong` bold. `hovered` is the screen row the pointer is on, inside
    /// the card (`kit::row_under`); the row drawn there is the pill alone.
    /// Nothing when nothing matches.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        look: Look,
        theme: &Theme,
        typed: &str,
        cursor: (u16, u16),
        bounds: Rect,
        hovered: Option<u16>,
        frame: &mut Frame,
    ) {
        let Some(card) = self.card(typed, cursor, bounds) else {
            return;
        };
        let shown = self.shown(typed);
        let selected = self.selected.min(shown.len().saturating_sub(1));
        let (kind_width, gap) = kind_column(&shown);
        let out = frame.buffer_mut();
        let inner = popup_card(look, theme, out, card);
        if inner.is_empty() {
            return;
        }
        // The text ends where the row pill's right end begins.
        let right = inner.right().saturating_sub(1);
        let x = inner.x + MARK;
        let room = usize::from(right.saturating_sub(x));
        let typed_chars = typed.chars().count();
        for (index, (y, item)) in (inner.y..inner.bottom()).zip(&shown).enumerate() {
            let row = Rect::new(inner.x, y, inner.width, 1);
            let chosen = index == selected;
            let lit = hovered == Some(y);
            let ink = kit::sel_row(look, theme, out, row, chosen, lit, theme.raised);
            out.set_stringn(x, y, item.kind, room, ink.faint);
            let offset = kind_width + gap;
            if offset >= room {
                continue;
            }
            let label_x = x + u16::try_from(offset).unwrap_or(u16::MAX);
            let label_room = room - offset;
            let (end, _) = out.set_stringn(label_x, y, &item.label, label_room, ink.label);
            // The typed prefix: the label's first chars, when they are what was
            // typed (the match is on the filter text, which can differ).
            let prefix_len = item
                .label
                .char_indices()
                .nth(typed_chars)
                .map_or(item.label.len(), |(at, _)| at);
            let prefix = &item.label[..prefix_len];
            if typed_chars > 0
                && prefix.chars().count() == typed_chars
                && prefix.to_lowercase() == typed.to_lowercase()
            {
                out.set_stringn(label_x, y, prefix, label_room, ink.hit);
            }
            let detail = item.detail.width();
            let right = usize::from(right);
            let free = right.saturating_sub(usize::from(end) + 2);
            if detail > 0 && free > 0 {
                let shown_width = detail.min(free);
                let detail_x = u16::try_from(right - shown_width).unwrap_or(u16::MAX);
                out.set_stringn(detail_x, y, &item.detail, shown_width, ink.faint);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::style::{Color, Modifier, Style};

    use super::*;
    use crate::config::Corners;
    use crate::ui::kit::{CAP_L, CAP_R, SEL_MARK};

    fn item(label: &str, edit: Option<Range<usize>>) -> CompletionItem {
        CompletionItem {
            label: label.into(),
            kind: "fn",
            detail: String::new(),
            filter: label.into(),
            text: label.into(),
            edit,
        }
    }

    #[test]
    fn typing_filters_by_prefix_ignoring_case_and_at_most_ten_show() {
        let names = [
            "Push", "push_str", "pop", "len", "a", "b", "c", "d", "e", "f", "g", "h",
        ];
        let completion = Completion::new(1, 0, 0, names.iter().map(|n| item(n, None)).collect());
        assert_eq!(completion.shown("").len(), MAX_ITEMS);
        let labels = |typed| -> Vec<String> {
            completion
                .shown(typed)
                .iter()
                .map(|i| i.label.clone())
                .collect()
        };
        assert_eq!(labels("pu"), ["Push", "push_str"]);
        assert_eq!(labels("PUSH_"), ["push_str"]);
        assert!(labels("x").is_empty());
    }

    #[test]
    fn the_selection_stops_at_either_end() {
        let items = ["a1", "a2", "a3"].iter().map(|n| item(n, None)).collect();
        let mut completion = Completion::new(1, 0, 0, items);
        completion.step("", false);
        assert_eq!(
            completion.selected("").map(|i| i.label.as_str()),
            Some("a1")
        );
        for _ in 0..5 {
            completion.step("", true);
        }
        assert_eq!(
            completion.selected("").map(|i| i.label.as_str()),
            Some("a3")
        );
        completion.step("", false);
        assert_eq!(
            completion.selected("").map(|i| i.label.as_str()),
            Some("a2")
        );
        completion.reset();
        assert_eq!(
            completion.selected("").map(|i| i.label.as_str()),
            Some("a1")
        );
    }

    #[test]
    fn accepting_replaces_the_edit_range_or_the_typed_word() {
        // Asked at 6 with the word starting at 6; "le" typed since.
        let completion = Completion::new(1, 6, 6, Vec::new());
        assert_eq!(completion.replaced(&item("len", Some(6..6)), 8), 6..8);
        // An edit reaching two chars past the request's cursor still covers them.
        assert_eq!(completion.replaced(&item("len", Some(4..8)), 8), 4..10);
        assert_eq!(completion.replaced(&item("len", None), 8), 6..8);
    }

    #[test]
    fn the_word_is_found_and_left() {
        let rope = Rope::from_str("    s.le x\n");
        assert_eq!(word_start(&rope, 8), 6);
        assert_eq!(word_start(&rope, 6), 6);
        assert_eq!(word_start(&rope, 5), 4);
        assert_eq!(typed_word(&rope, 6, 8).as_deref(), Some("le"));
        assert_eq!(typed_word(&rope, 6, 6).as_deref(), Some(""));
        assert_eq!(typed_word(&rope, 6, 5), None);
        assert_eq!(typed_word(&rope, 6, 10), None);
    }

    /// `push` (with a detail) and `push_str` for a typed `pu`, the first one
    /// selected, with the cursor at (2, 1) of a 32 by 7 screen of `x`: the card
    /// is at (2, 2), 27 by 4, and its rows' text starts at column 6.
    fn popup(look: Look, theme: &Theme) -> anyhow::Result<Buffer> {
        popup_under(look, theme, None)
    }

    /// The same popup with the pointer on screen row `hovered`.
    fn popup_under(look: Look, theme: &Theme, hovered: Option<u16>) -> anyhow::Result<Buffer> {
        let mut push = item("push", None);
        push.detail = "fn(char)".into();
        let completion = Completion::new(1, 0, 0, vec![push, item("push_str", None)]);
        let mut terminal = Terminal::new(TestBackend::new(32, 7))?;
        terminal.draw(|frame| {
            let area = frame.area();
            let out = frame.buffer_mut();
            for y in area.top()..area.bottom() {
                out.set_string(0, y, "x".repeat(32), Style::new().fg(Color::Red));
            }
            completion.render(look, theme, "pu", (2, 1), area, hovered, frame);
        })?;
        Ok(terminal.backend().buffer().clone())
    }

    /// Columns `from` up to `to` of row `y`.
    fn cells(buf: &Buffer, y: u16, from: u16, to: u16) -> String {
        (from..to).map(|x| buf[(x, y)].symbol()).collect()
    }

    const LOOKS: [Look; 2] = [
        Look {
            pill_caps: true,
            corners: Corners::Flush,
            motion: true,
            glide: None,
        },
        Look {
            pill_caps: false,
            corners: Corners::Rounded,
            motion: true,
            glide: None,
        },
    ];

    #[test]
    fn the_popup_is_a_card_on_raised_and_its_selected_row_a_pill() -> anyhow::Result<()> {
        let bold = Modifier::BOLD;
        for name in ["tinkery", "tinkery-light"] {
            let theme = Theme::named(name).expect("it exists");
            for look in LOOKS {
                let at = format!("{name} {look:?}");
                let buf = popup(look, &theme)?;
                let (top, side, bottom) = match look.corners {
                    Corners::Flush => (format!(" {} ", "▁".repeat(25)), ("▕", "▏"), "▔"),
                    Corners::Rounded => (format!("╭{}╮", "─".repeat(25)), ("│", "│"), "─"),
                };
                assert_eq!(cells(&buf, 2, 2, 29), top, "{at}");
                assert_eq!(cells(&buf, 5, 3, 28), bottom.repeat(25), "{at}");
                for y in 3..5 {
                    for (x, glyph) in [(2, side.0), (28, side.1)] {
                        let edge = &buf[(x, y)];
                        assert_eq!(edge.symbol(), glyph, "{at}");
                        assert_eq!((edge.fg, edge.bg), (theme.line2, theme.bg), "{at}");
                    }
                }
                // The selected row: a pill of `hov` on `raised` across the
                // inside, `›` a cell in, then the text.
                assert_eq!(
                    cells(&buf, 3, 4, 27),
                    format!("{SEL_MARK} fn push      fn(char)"),
                    "{at}"
                );
                for (x, cap) in [(3, CAP_L), (27, CAP_R)] {
                    let end = &buf[(x, 3)];
                    if look.pill_caps {
                        assert_eq!(end.symbol(), cap, "{at}");
                        assert_eq!((end.fg, end.bg), (theme.hov, theme.raised), "{at}");
                    } else {
                        assert_eq!((end.symbol(), end.bg), (" ", theme.hov), "{at}");
                    }
                }
                for x in 4..27 {
                    assert_eq!(buf[(x, 3)].bg, theme.hov, "{at} {x}");
                }
                let style = |x: u16, y: u16| (buf[(x, y)].fg, buf[(x, y)].modifier);
                assert_eq!(style(4, 3), (theme.accent, bold), "{at}");
                // Kind and detail in `text`, as `muted` is too close to `hov`.
                assert_eq!(style(6, 3), (theme.text, Modifier::empty()), "{at}");
                assert_eq!(style(19, 3), (theme.text, Modifier::empty()), "{at}");
                // The typed `pu` in `accent` bold, the rest in `strong` bold.
                assert_eq!(style(9, 3), (theme.accent, bold), "{at}");
                assert_eq!(style(10, 3), (theme.accent, bold), "{at}");
                assert_eq!(style(11, 3), (theme.strong, bold), "{at}");
                assert_eq!(style(12, 3), (theme.strong, bold), "{at}");

                // Any other row is left on the card: kind `muted`, label `fg`
                // with the typed prefix still `accent` bold.
                assert_eq!(cells(&buf, 4, 3, 28), format!("   {:<22}", "fn push_str"));
                for x in 3..28 {
                    assert_eq!(buf[(x, 4)].bg, theme.raised, "{at} {x}");
                }
                assert_eq!(style(6, 4), (theme.muted, Modifier::empty()), "{at}");
                assert_eq!(style(9, 4), (theme.accent, bold), "{at}");
                assert_eq!(style(10, 4), (theme.accent, bold), "{at}");
                assert_eq!(style(11, 4), (theme.fg, Modifier::empty()), "{at}");

                // Not dimmed: around the card the screen is as it was.
                for (x, y) in [(1, 3), (29, 3), (10, 1), (10, 6), (0, 0), (31, 6)] {
                    let cell = &buf[(x, y)];
                    assert_eq!((cell.symbol(), cell.fg), ("x", Color::Red), "{at}");
                    assert_eq!((cell.bg, cell.modifier), (Color::Reset, Modifier::empty()));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn in_mono_the_selected_row_is_reverse_video_with_nothing_coloured_on_it() -> anyhow::Result<()>
    {
        let mono = Theme::named("mono").expect("mono exists");
        for look in LOOKS {
            let buf = popup(look, &mono)?;
            assert_eq!(
                cells(&buf, 3, 3, 28),
                format!(" {SEL_MARK} fn push      fn(char) "),
                "{look:?}"
            );
            // One pair of colours from edge to edge of the inside, reversed:
            // no kind, label, prefix or detail brings its own.
            let row = Theme::highlight(mono.hov, mono.strong);
            for x in 3..28 {
                let cell = &buf[(x, 3)];
                assert_eq!(
                    (cell.fg, cell.bg),
                    (row.fg.unwrap(), row.bg.unwrap()),
                    "{x}"
                );
                assert!(cell.modifier.contains(Modifier::REVERSED), "{x}");
                // The mark and the label are told apart by weight alone.
                let heavy = x == 4 || (9..13).contains(&x);
                assert_eq!(cell.modifier.contains(Modifier::BOLD), heavy, "{x}");
            }
            // The row under it is plain: kind in `muted`, the prefix bold.
            for x in 3..28 {
                let cell = &buf[(x, 4)];
                assert_eq!(cell.bg, mono.raised, "{x}");
                assert!(!cell.modifier.contains(Modifier::REVERSED), "{x}");
            }
            assert_eq!(buf[(6, 4)].fg, mono.muted);
            assert_eq!(buf[(9, 4)].fg, mono.accent);
            assert!(buf[(9, 4)].modifier.contains(Modifier::BOLD));
            assert_eq!(buf[(11, 4)].fg, mono.fg);
            // Terminal colours only and no Nerd Font glyph, whatever `[ui]` says.
            for y in 0..7 {
                for x in 0..32 {
                    let cell = &buf[(x, y)];
                    assert!(!matches!(cell.fg, Color::Rgb(..)), "{x},{y}");
                    assert!(!matches!(cell.bg, Color::Rgb(..)), "{x},{y}");
                    assert!(!matches!(cell.symbol(), CAP_L | CAP_R), "{x},{y}");
                }
            }
        }
        Ok(())
    }

    #[test]
    fn the_pointer_lights_a_row_without_the_mark_and_leaves_the_selection() -> anyhow::Result<()> {
        let theme = Theme::default();
        let rest = popup(LOOKS[0], &theme)?;
        // The card is at (2, 2), 27 by 4: `push` on row 3, selected, and
        // `push_str` on row 4.
        let mut push = item("push", None);
        push.detail = "fn(char)".into();
        let completion = Completion::new(1, 0, 0, vec![push, item("push_str", None)]);
        let bounds = Rect::new(0, 0, 32, 7);
        let card = completion.card("pu", (2, 1), bounds);
        assert_eq!(card, Some(Rect::new(2, 2, 27, 4)));
        assert_eq!(completion.card("zz", (2, 1), bounds), None);

        let lit = popup_under(LOOKS[0], &theme, Some(4))?;
        for x in 4..27 {
            assert_eq!(rest[(x, 4)].bg, theme.raised, "{x}");
            assert_eq!(lit[(x, 4)].bg, theme.hov, "{x}");
        }
        assert_eq!(lit[(4, 4)].symbol(), " ");
        assert!(!lit[(11, 4)].modifier.contains(Modifier::BOLD));
        // The selected row is as it was, mark and all.
        for x in 0..32 {
            assert_eq!(lit[(x, 3)], rest[(x, 3)], "{x}");
        }
        assert_eq!(lit[(4, 3)].symbol(), SEL_MARK);
        // On the selected row, or on the card's edge, nothing changes.
        for y in [2, 3, 5] {
            assert_eq!(popup_under(LOOKS[0], &theme, Some(y))?, rest, "{y}");
        }
        // `mono` has no fills.
        let mono = Theme::named("mono").expect("mono exists");
        assert_eq!(
            popup_under(LOOKS[0], &mono, Some(4))?,
            popup(LOOKS[0], &mono)?
        );
        Ok(())
    }

    #[test]
    fn a_popup_with_no_room_for_a_row_draws_nothing() -> anyhow::Result<()> {
        let completion = Completion::new(1, 0, 0, vec![item("push", None)]);
        let theme = Theme::default();
        // Six cells is the edges, the mark's three and the pill's end: no text.
        let mut terminal = Terminal::new(TestBackend::new(6, 6))?;
        terminal.draw(|frame| {
            completion.render(LOOKS[0], &theme, "", (0, 0), frame.area(), None, frame)
        })?;
        assert_eq!(
            terminal.backend().buffer(),
            &Buffer::empty(Rect::new(0, 0, 6, 6))
        );
        Ok(())
    }
}
