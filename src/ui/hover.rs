//! The hover popup: a small card anchored at the cursor showing what the language
//! server said about the symbol under it, as plain text wrapped to the card, its
//! signature ruled off from its docs.
//! `anchor` places it and is kept apart so the completion popup can share it.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::kit::{self, Card, Look};
use crate::config::Corners;
use crate::lsp::HoverText;
use crate::theme::Theme;

/// Widest and tallest the text gets inside the card.
const MAX_TEXT_WIDTH: u16 = 60;
const MAX_TEXT_HEIGHT: u16 = 12;
/// The card's edge plus one cell of padding on each side.
const SIDE: u16 = 2;

/// Where a card of `width` by `height` goes for the cursor at screen cell
/// `cursor`, inside `bounds`: on the row below the cursor, starting at its column.
/// When it doesn't fit below it flips above, and when neither side has room it
/// takes the roomier one, cut to fit. Near the right edge it shifts left. The
/// cursor's own cell is never covered.
pub fn anchor(cursor: (u16, u16), width: u16, height: u16, bounds: Rect) -> Rect {
    let (col, row) = cursor;
    let width = width.min(bounds.width);
    let x = col.min(bounds.right().saturating_sub(width)).max(bounds.x);
    let below = bounds.bottom().saturating_sub(row.saturating_add(1));
    let above = row.saturating_sub(bounds.y);
    let (y, height) = if height <= below {
        (row + 1, height)
    } else if height <= above {
        (row - height, height)
    } else if below >= above {
        (row + 1, below)
    } else {
        (bounds.y, above)
    };
    Rect {
        x,
        y,
        width,
        height,
    }
}

/// `text` broken into lines no wider than `width` cells: at the last space that
/// fits, or mid-word when a word is wider than the line. Line breaks in `text`
/// are kept and tabs become four spaces.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.replace('\t', "    ");
        let mut current = String::new();
        let mut current_width = 0;
        // Byte index just past the last space in `current`.
        let mut last_space = None;
        for ch in line.chars() {
            let w = ch.width().unwrap_or(0);
            if current_width + w > width && !current.is_empty() {
                if ch == ' ' {
                    out.push(current.trim_end().to_string());
                    current.clear();
                    current_width = 0;
                    last_space = None;
                    continue;
                }
                match last_space {
                    Some(at) if !current[..at].trim().is_empty() => {
                        let rest = current.split_off(at);
                        out.push(current.trim_end().to_string());
                        current_width = rest.width();
                        current = rest;
                    }
                    _ => {
                        out.push(std::mem::take(&mut current));
                        current_width = 0;
                    }
                }
                last_space = None;
            }
            current.push(ch);
            current_width += w;
            if ch == ' ' {
                last_space = Some(current.len());
            }
        }
        out.push(current.trim_end().to_string());
    }
    out
}

/// Draws the card a popup anchored at the cursor sits in, over `area`: a kit
/// card with a `line2` edge around `raised`, its edge following `look.corners`.
/// Returns the inside. The screen around it is left as it was: a popup at the
/// cursor is read alongside the code, so nothing is dimmed.
///
/// A kit card's edge cells keep the background under them, and a popup lands
/// on the editor's text. Selected text is reverse video, so the background its
/// cell carries is the text's colour, and a find match or a paused line has a
/// fill of its own: left there, each would show as a bright block in the edge.
/// So the cells under the card are emptied onto the editor's own `bg` first.
/// A wide character whose second half the card covers is blanked as well: the
/// terminal draws both halves from the first cell, which would otherwise paint
/// over the card's left edge.
pub fn popup_card(look: Look, theme: &Theme, buf: &mut Buffer, area: Rect) -> Rect {
    let area = area.intersection(buf.area);
    if area.width < 4 || area.height < 2 {
        return Rect {
            width: 0,
            height: 0,
            ..area
        };
    }
    for y in area.top()..area.bottom() {
        if area.x > buf.area.x {
            let before = &mut buf[(area.x - 1, y)];
            if before.symbol().width() > 1 {
                before.set_symbol(" ");
            }
        }
        for x in area.left()..area.right() {
            buf[(x, y)].reset();
        }
    }
    buf.set_style(area, Style::new().bg(theme.bg));
    kit::card(look, theme, buf, area, &Card::new(theme, ""))
}

/// One row of the hover card's text.
enum Row<'a> {
    Code(&'a str),
    /// The rule between the code and the docs.
    Rule,
    Docs(&'a str),
}

/// Draws `hover` in a card anchored at the cursor cell `cursor`, kept inside
/// `bounds`: the code in `fg`, a rule, then the docs in `text`. Leaves the
/// terminal cursor where it was.
pub fn render_hover(
    look: Look,
    theme: &Theme,
    hover: &HoverText,
    cursor: (u16, u16),
    bounds: Rect,
    frame: &mut Frame,
) {
    let room = bounds.width.saturating_sub(SIDE * 2).min(MAX_TEXT_WIDTH);
    let longest = hover
        .code
        .lines()
        .chain(hover.docs.lines())
        .map(UnicodeWidthStr::width)
        .max()
        .unwrap_or(0);
    let text_width = u16::try_from(longest)
        .unwrap_or(u16::MAX)
        .clamp(1, room.max(1));
    let wrapped = |text: &str| {
        if text.is_empty() {
            Vec::new()
        } else {
            wrap(text, usize::from(text_width))
        }
    };
    let (code, docs) = (wrapped(&hover.code), wrapped(&hover.docs));
    let rule = !code.is_empty() && !docs.is_empty();
    let lines: Vec<Row> = code
        .iter()
        .map(|line| Row::Code(line))
        .chain(rule.then_some(Row::Rule))
        .chain(docs.iter().map(|line| Row::Docs(line)))
        .take(usize::from(MAX_TEXT_HEIGHT))
        .collect();
    let rows = u16::try_from(lines.len()).unwrap_or(MAX_TEXT_HEIGHT);
    let card = anchor(cursor, text_width + SIDE * 2, rows + 2, bounds);
    if card.width <= SIDE * 2 || card.height <= 2 {
        return;
    }
    let out = frame.buffer_mut();
    let inner = popup_card(look, theme, out, card);
    if inner.is_empty() {
        return;
    }
    let width = usize::from(inner.width.saturating_sub(2));
    let line = Style::new().fg(theme.line2);
    for (y, row) in (inner.y..inner.bottom()).zip(&lines) {
        match row {
            Row::Code(text) => {
                out.set_stringn(inner.x + 1, y, text, width, Style::new().fg(theme.fg));
            }
            // Docs keep the card's own `text`.
            Row::Docs(text) => {
                out.set_stringn(inner.x + 1, y, text, width, Style::new());
            }
            Row::Rule => {
                out.set_string(inner.x, y, "─".repeat(usize::from(inner.width)), line);
                // A rounded edge is a line through the middle of its cell, so
                // the rule joins it with a tee. A flush edge is drawn on the
                // side of its cell that faces the card, where the rule already
                // meets it; a tee there would stick out past the card.
                if look.corners == Corners::Rounded {
                    out[(inner.x - 1, y)].set_symbol("├");
                    out[(inner.right(), y)].set_symbol("┤");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::{Color, Modifier};

    use super::*;

    const SCREEN: Rect = Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 29,
    };

    const FLUSH: Look = Look {
        pill_caps: true,
        corners: Corners::Flush,
        motion: true,
        glide: None,
    };
    const ROUNDED: Look = Look {
        pill_caps: true,
        corners: Corners::Rounded,
        motion: true,
        glide: None,
    };

    fn themes() -> [Theme; 3] {
        ["tinkery", "tinkery-light", "mono"].map(|name| Theme::named(name).expect("it exists"))
    }

    /// A hover of one line of code and one of docs for the cursor at (2, 1) of
    /// a 20 by 9 screen already full of selected text, which is reverse video
    /// on a background of its own colour: the card is at (2, 2), 14 by 5.
    fn hover(look: Look, theme: &Theme) -> anyhow::Result<Buffer> {
        let text = HoverText {
            code: "fn f()".into(),
            docs: "Docs here.".into(),
        };
        let mut terminal = Terminal::new(TestBackend::new(20, 9))?;
        terminal.draw(|frame| {
            let area = frame.area();
            let out = frame.buffer_mut();
            for y in area.top()..area.bottom() {
                out.set_string(
                    0,
                    y,
                    "x".repeat(20),
                    Theme::highlight(theme.sel, Color::Red),
                );
            }
            render_hover(look, theme, &text, (2, 1), area, frame);
        })?;
        Ok(terminal.backend().buffer().clone())
    }

    /// Columns 2 to 15 of row `y`: the card's own cells.
    fn card_row(buf: &Buffer, y: u16) -> String {
        (2..16).map(|x| buf[(x, y)].symbol()).collect()
    }

    #[test]
    fn the_hover_is_a_flush_card_with_its_rule_run_between_the_edges() -> anyhow::Result<()> {
        for theme in themes() {
            let buf = hover(FLUSH, &theme)?;
            let rows: Vec<String> = (2..7).map(|y| card_row(&buf, y)).collect();
            assert_eq!(
                rows,
                [
                    " ▁▁▁▁▁▁▁▁▁▁▁▁ ",
                    "▕ fn f()     ▏",
                    "▕────────────▏",
                    "▕ Docs here. ▏",
                    " ▔▔▔▔▔▔▔▔▔▔▔▔ ",
                ]
            );
            // The code in `fg`, the docs in `text`, the rule in `line2`, all
            // on `raised`.
            for (at, fg) in [
                ((4, 3), theme.fg),
                ((4, 5), theme.text),
                ((8, 4), theme.line2),
            ] {
                assert_eq!((buf[at].fg, buf[at].bg), (fg, theme.raised), "{at:?}");
            }
        }
        Ok(())
    }

    #[test]
    fn a_rounded_hover_joins_its_rule_to_the_edges_with_tees() -> anyhow::Result<()> {
        for theme in themes() {
            let buf = hover(ROUNDED, &theme)?;
            let rows: Vec<String> = (2..7).map(|y| card_row(&buf, y)).collect();
            assert_eq!(
                rows,
                [
                    "╭────────────╮",
                    "│ fn f()     │",
                    "├────────────┤",
                    "│ Docs here. │",
                    "╰────────────╯",
                ]
            );
            for x in [2, 15] {
                assert_eq!((buf[(x, 4)].fg, buf[(x, 4)].bg), (theme.line2, theme.bg));
            }
        }
        Ok(())
    }

    #[test]
    fn the_popup_dims_nothing_and_no_text_or_selection_shows_in_its_edge() -> anyhow::Result<()> {
        for theme in themes() {
            for look in [FLUSH, ROUNDED] {
                let buf = hover(look, &theme)?;
                for y in 0..9 {
                    for x in 0..20 {
                        let cell = &buf[(x, y)];
                        let at = format!("{x},{y} {look:?}");
                        let (in_x, in_y) = ((2..16).contains(&x), (2..7).contains(&y));
                        if !(in_x && in_y) {
                            // Around the card the screen is as it was.
                            assert_eq!(cell.symbol(), "x", "{at}");
                            assert_eq!((cell.fg, cell.bg), (theme.sel, Color::Red), "{at}");
                            assert_eq!(cell.modifier, Modifier::REVERSED, "{at}");
                            continue;
                        }
                        assert_ne!(cell.symbol(), "x", "{at}");
                        assert!(!cell.modifier.contains(Modifier::REVERSED), "{at}");
                        if x == 2 || x == 15 || y == 2 || y == 6 {
                            // The edge: `line2` on the editor's ground, not on
                            // the colour of the selected text it covers.
                            assert_eq!((cell.fg, cell.bg), (theme.line2, theme.bg), "{at}");
                            assert!(cell.modifier.is_empty(), "{at}");
                        } else {
                            assert_eq!(cell.bg, theme.raised, "{at}");
                        }
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn a_wide_character_cut_by_the_cards_edge_is_blanked() {
        let theme = Theme::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 12, 5));
        let red = Style::new().fg(Color::Red);
        for y in 0..5 {
            // Each `日` takes two cells: columns 1-2, 3-4 and so on.
            buf.set_string(1, y, "日日日日日", red);
        }
        let inside = popup_card(FLUSH, &theme, &mut buf, Rect::new(2, 1, 8, 3));
        assert_eq!(inside, Rect::new(3, 2, 6, 1));
        // Left of the card, the half a card row covers is gone with its other
        // half, which the terminal would have drawn across the edge.
        for y in 1..4 {
            assert_eq!(buf[(1, y)].symbol(), " ", "{y}");
        }
        assert_eq!(buf[(2, 2)].symbol(), "▕");
        assert_eq!(buf[(9, 2)].symbol(), "▏");
        // Rows the card doesn't reach keep theirs.
        assert_eq!(buf[(1, 0)].symbol(), "日");
        assert_eq!(buf[(1, 4)].symbol(), "日");
        // Too small to be a card, it draws and clears nothing.
        let before = buf.clone();
        let none = popup_card(FLUSH, &theme, &mut buf, Rect::new(0, 0, 3, 5));
        assert!(none.is_empty());
        assert_eq!(buf, before);
    }

    #[test]
    fn the_card_sits_below_the_cursor_at_its_column() {
        assert_eq!(anchor((10, 5), 20, 4, SCREEN), Rect::new(10, 6, 20, 4));
    }

    #[test]
    fn near_the_bottom_it_flips_above() {
        assert_eq!(anchor((10, 28), 20, 4, SCREEN), Rect::new(10, 24, 20, 4));
        // Exactly enough room below still goes below.
        assert_eq!(anchor((10, 24), 20, 4, SCREEN), Rect::new(10, 25, 20, 4));
    }

    #[test]
    fn near_the_right_edge_it_shifts_left() {
        assert_eq!(anchor((95, 5), 20, 4, SCREEN), Rect::new(80, 6, 20, 4));
        assert_eq!(anchor((95, 5), 200, 4, SCREEN), Rect::new(0, 6, 100, 4));
    }

    #[test]
    fn without_room_either_side_it_takes_the_roomier_cut_to_fit() {
        let short = Rect::new(0, 0, 100, 10);
        assert_eq!(anchor((0, 2), 20, 12, short), Rect::new(0, 3, 20, 7));
        assert_eq!(anchor((0, 7), 20, 12, short), Rect::new(0, 0, 20, 7));
    }

    #[test]
    fn wrapping_breaks_at_spaces_and_keeps_line_breaks() {
        assert_eq!(
            wrap("the quick brown fox\n\njumps", 10),
            ["the quick", "brown fox", "", "jumps"]
        );
        assert_eq!(wrap("abcdefghij", 4), ["abcd", "efgh", "ij"]);
        assert_eq!(wrap("ab  cd", 3), ["ab", "cd"]);
        assert_eq!(wrap("\tx", 10), ["    x"]);
        // Wide characters count two cells.
        assert_eq!(wrap("日本語", 4), ["日本", "語"]);
    }
}
