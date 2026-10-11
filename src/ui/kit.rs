//! The drawing kit every screen's chrome is built from: pills, chips, strips,
//! keycaps and hint lines (tinkery-forge spec F4, F6).
//!
//! A pill is a filled run of cells whose two ends are rounded by a Nerd Font
//! half-circle drawn in the fill colour on the ground around the pill. A font
//! without those glyphs shows boxes, so `[ui] pill_caps = false` squares the
//! ends off, and `mono`, which must work in any terminal, never rounds them:
//! the end is then a blank of the fill colour, so a pill is the same width
//! either way and nothing around it moves.
//!
//! Nothing here reads a global. Each function takes the `Look` and the `Theme`
//! it draws with, and the `ground` (the colour behind the pill) where an end
//! needs it, so drawing stays a pure function of `App`.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use crate::config::UiConfig;
use crate::theme::Theme;

/// The half-circle that rounds a pill's left end (Nerd Font, U+E0B6).
pub const CAP_L: &str = "\u{e0b6}";
/// The half-circle that rounds a pill's right end (Nerd Font, U+E0B4).
pub const CAP_R: &str = "\u{e0b4}";

/// How the chrome is drawn, from `[ui]` in `config.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Look {
    /// Round pill ends with `CAP_L` and `CAP_R`; off squares them.
    pub pill_caps: bool,
}

impl Default for Look {
    fn default() -> Self {
        Look::of(&UiConfig::default())
    }
}

impl Look {
    /// The look `[ui]` asks for.
    pub fn of(ui: &UiConfig) -> Look {
        Look {
            pill_caps: ui.pill_caps,
        }
    }

    /// Whether pill ends are the cap glyphs in `theme`: the setting is on and
    /// the theme isn't `mono`, which never draws a Nerd Font glyph.
    pub fn round(self, theme: &Theme) -> bool {
        self.pill_caps && !theme.flat()
    }
}

/// `color` for text on `fill`, or `text` when the two are the same colour and
/// the words would vanish. Only terminal colours collide: `mono`'s `muted`,
/// `guide`, `btn` and `raised2` are all dark grey.
pub fn ink(theme: &Theme, color: Color, fill: Color) -> Color {
    if color == fill { theme.text } else { color }
}

/// The display width of `spans` in cells.
pub fn width(spans: &[Span]) -> u16 {
    let cells: usize = spans.iter().map(Span::width).sum();
    u16::try_from(cells).unwrap_or(u16::MAX)
}

/// Draws `spans` on row `y` from `x`, stopping before `max_x`, and returns the
/// column after the last cell drawn.
pub fn put(buf: &mut Buffer, x: u16, y: u16, spans: &[Span], max_x: u16) -> u16 {
    let mut x = x;
    for span in spans {
        if x >= max_x {
            break;
        }
        x = buf.set_span(x, y, span, max_x - x).0;
    }
    x
}

/// The two ends of a pill of `fill` sitting on `ground`: `CAP_L` and `CAP_R`
/// in `fill` on `ground`, or, squared off, a blank of `fill` each. One cell
/// wide either way.
pub fn ends(
    look: Look,
    theme: &Theme,
    fill: Color,
    ground: Color,
) -> (Span<'static>, Span<'static>) {
    if look.round(theme) {
        let style = Style::new().fg(fill).bg(ground);
        (Span::styled(CAP_L, style), Span::styled(CAP_R, style))
    } else {
        let style = Style::new().bg(fill);
        (Span::styled(" ", style), Span::styled(" ", style))
    }
}

/// `spans` on a pill of `fill` on `ground`: an end, the spans with their
/// background set to `fill`, an end. Two cells wider than the spans.
pub fn pill(
    look: Look,
    theme: &Theme,
    spans: Vec<Span<'static>>,
    fill: Color,
    ground: Color,
) -> Vec<Span<'static>> {
    let (left, right) = ends(look, theme, fill, ground);
    let mut out = Vec::with_capacity(spans.len() + 2);
    out.push(left);
    out.extend(
        spans
            .into_iter()
            .map(|span| span.patch_style(Style::new().bg(fill))),
    );
    out.push(right);
    out
}

/// `text` on a pill filled with `style`'s background, on `ground`: as wide as
/// `" text "`. A toggle, a tag, a key.
pub fn chip(
    look: Look,
    theme: &Theme,
    text: &str,
    style: Style,
    ground: Color,
) -> Vec<Span<'static>> {
    let fill = style.bg.unwrap_or(Color::Reset);
    let label = Span::styled(text.to_string(), style);
    pill(look, theme, vec![label], fill, ground)
}

/// An empty pill of `fill` on `ground`, `width` cells from `(x, y)`, over
/// whatever was drawn there: the ground of a selected row or an input. Draw
/// its text afterwards, from `x + 1`. Narrower than its two ends, it draws
/// nothing.
pub fn row_pill(
    look: Look,
    theme: &Theme,
    buf: &mut Buffer,
    (x, y): (u16, u16),
    width: u16,
    fill: Color,
    ground: Color,
) {
    if width < 2 {
        return;
    }
    let row = Rect::new(x, y, width, 1).intersection(buf.area);
    // What was there may be bold or underlined, which a blank would keep.
    for cx in row.left()..row.right() {
        buf[(cx, y)].reset();
    }
    if row.is_empty() {
        return;
    }
    let blank = Span::raw(" ".repeat(usize::from(width - 2)));
    put(
        buf,
        x,
        y,
        &pill(look, theme, vec![blank], fill, ground),
        row.right(),
    );
}

/// A one-row bar of `fill` across the top row of `area`, on `ground`: a row
/// pill, or just the fill where there's no room for two ends and something
/// between them. Returns the cells between the ends, where the bar's content
/// goes; they are the same cells whether the ends are round or square.
pub fn strip(
    look: Look,
    theme: &Theme,
    buf: &mut Buffer,
    area: Rect,
    fill: Color,
    ground: Color,
) -> Rect {
    let row = Rect {
        height: area.height.min(1),
        ..area
    };
    if row.width < 3 {
        let shown = row.intersection(buf.area);
        for y in shown.top()..shown.bottom() {
            for x in shown.left()..shown.right() {
                buf[(x, y)].reset();
            }
        }
        buf.set_style(shown, Style::new().bg(fill));
        return row;
    }
    row_pill(look, theme, buf, (row.x, row.y), row.width, fill, ground);
    Rect {
        x: row.x + 1,
        width: row.width - 2,
        ..row
    }
}

/// A key as a keycap: a chip on `btn` with the key in `strong`, bold.
pub fn keycap(look: Look, theme: &Theme, key: &str, ground: Color) -> Vec<Span<'static>> {
    let style = Style::new()
        .bg(theme.btn)
        .fg(theme.strong)
        .add_modifier(Modifier::BOLD);
    chip(look, theme, key, style, ground)
}

/// `keys` as keycaps, one blank of `ground` apart. Keys are separated by a
/// space and sets of keys by two; a set of four or more one-letter keys shares
/// one cap (`h j k l`), every other key gets its own.
pub fn keycaps(look: Look, theme: &Theme, keys: &str, ground: Color) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    for set in keys.split("  ").filter(|set| !set.is_empty()) {
        let each: Vec<&str> = set.split(' ').filter(|key| !key.is_empty()).collect();
        let caps = if each.len() >= 4 && each.iter().all(|key| key.chars().count() == 1) {
            vec![each.join(" ")]
        } else {
            each.iter().map(|key| (*key).to_string()).collect()
        };
        for cap in caps {
            if !out.is_empty() {
                out.push(Span::styled(" ", Style::new().bg(ground)));
            }
            out.extend(keycap(look, theme, &cap, ground));
        }
    }
    out
}

/// A hint line on `ground`: for each `(keys, word)`, the keys as keycaps, a
/// blank and the word in `muted`; three blanks between one hint and the next.
pub fn hints(
    look: Look,
    theme: &Theme,
    pairs: &[(&str, &str)],
    ground: Color,
) -> Vec<Span<'static>> {
    let plain = Style::new().bg(ground);
    let word = plain.fg(ink(theme, theme.muted, ground));
    let mut out = Vec::new();
    for (index, (keys, what)) in pairs.iter().enumerate() {
        if index > 0 {
            out.push(Span::styled("   ", plain));
        }
        out.extend(keycaps(look, theme, keys, ground));
        out.push(Span::styled(format!(" {what}"), word));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROUND: Look = Look { pill_caps: true };
    const SQUARE: Look = Look { pill_caps: false };
    const FILL: Color = Color::Rgb(1, 2, 3);
    const GROUND: Color = Color::Rgb(9, 9, 9);

    fn mono() -> Theme {
        Theme::named("mono").expect("mono exists")
    }

    /// Every way an end is drawn: round, squared by the setting, squared by
    /// `mono` whatever the setting says.
    fn looks() -> [(Look, Theme, bool); 3] {
        [
            (ROUND, Theme::default(), true),
            (SQUARE, Theme::default(), false),
            (ROUND, mono(), false),
        ]
    }

    fn text(spans: &[Span]) -> String {
        spans.iter().map(|span| span.content.as_ref()).collect()
    }

    fn drawn(spans: &[Span], cells: u16) -> Buffer {
        let mut buf = Buffer::empty(Rect::new(0, 0, cells, 1));
        put(&mut buf, 0, 0, spans, cells);
        buf
    }

    fn row(buf: &Buffer) -> String {
        (0..buf.area.width).map(|x| buf[(x, 0)].symbol()).collect()
    }

    /// Asserts the cell at `x` is a pill end of `fill` on `ground`: the cap
    /// glyph when `round`, else a blank of `fill`.
    fn assert_end(buf: &Buffer, x: u16, cap: &str, round: bool, fill: Color, ground: Color) {
        let cell = &buf[(x, 0)];
        if round {
            assert_eq!(cell.symbol(), cap, "{x}");
            assert_eq!((cell.fg, cell.bg), (fill, ground), "{x}");
        } else {
            assert_eq!(cell.symbol(), " ", "{x}");
            assert_eq!(cell.bg, fill, "{x}");
        }
    }

    fn assert_no_caps(buf: &Buffer) {
        let row = row(buf);
        assert!(!row.contains(CAP_L) && !row.contains(CAP_R), "{row:?}");
    }

    #[test]
    fn the_look_follows_the_setting_and_mono_never_rounds() {
        assert!(Look::default().pill_caps);
        assert!(!Look::of(&UiConfig { pill_caps: false }).pill_caps);
        assert!(ROUND.round(&Theme::default()));
        assert!(!SQUARE.round(&Theme::default()));
        assert!(!ROUND.round(&mono()));
        for name in crate::theme::NAMES.iter().filter(|name| **name != "mono") {
            let theme = Theme::named(name).expect("a listed theme exists");
            assert!(ROUND.round(&theme), "{name}");
        }
    }

    #[test]
    fn ends_are_caps_in_the_fill_on_the_ground_or_blanks_of_the_fill() {
        for (look, theme, round) in looks() {
            let (left, right) = ends(look, &theme, FILL, GROUND);
            assert_eq!((left.width(), right.width()), (1, 1));
            let buf = drawn(&[left, right], 2);
            assert_end(&buf, 0, CAP_L, round, FILL, GROUND);
            assert_end(&buf, 1, CAP_R, round, FILL, GROUND);
            if !round {
                assert_no_caps(&buf);
            }
        }
    }

    #[test]
    fn a_pill_fills_its_spans_and_is_two_cells_wider() {
        for (look, theme, round) in looks() {
            let spans = vec![
                Span::styled("ab", Style::new().fg(Color::Red)),
                Span::styled("c", Style::new().bg(Color::Blue)),
            ];
            let pill = pill(look, &theme, spans, FILL, GROUND);
            assert_eq!(width(&pill), 5);
            let buf = drawn(&pill, 5);
            assert_end(&buf, 0, CAP_L, round, FILL, GROUND);
            assert_end(&buf, 4, CAP_R, round, FILL, GROUND);
            assert_eq!(&row(&buf)[if round { 3 } else { 1 }..][..3], "abc");
            for x in 1..4 {
                assert_eq!(buf[(x, 0)].bg, FILL, "{x}");
            }
            // A span keeps its own colour for the text.
            assert_eq!(buf[(1, 0)].fg, Color::Red);
        }
    }

    #[test]
    fn a_chip_is_its_text_between_ends_of_its_own_background() {
        for (look, theme, round) in looks() {
            let style = Style::new()
                .fg(Color::Red)
                .bg(FILL)
                .add_modifier(Modifier::BOLD);
            let chip = chip(look, &theme, "Aa", style, GROUND);
            // As wide as ` Aa `.
            assert_eq!(width(&chip), 4);
            let buf = drawn(&chip, 4);
            assert_end(&buf, 0, CAP_L, round, FILL, GROUND);
            assert_end(&buf, 3, CAP_R, round, FILL, GROUND);
            assert_eq!(text(&chip[1..2]), "Aa");
            for x in 1..3 {
                let cell = &buf[(x, 0)];
                assert_eq!((cell.fg, cell.bg), (Color::Red, FILL), "{x}");
                assert!(cell.modifier.contains(Modifier::BOLD), "{x}");
            }
        }
    }

    #[test]
    fn a_row_pill_is_as_wide_as_asked_and_clears_what_was_there() {
        for (look, theme, round) in looks() {
            let mut buf = Buffer::empty(Rect::new(0, 0, 10, 1));
            let underlined = Style::new().add_modifier(Modifier::UNDERLINED);
            buf.set_string(0, 0, "xxxxxxxxxx", underlined);
            row_pill(look, &theme, &mut buf, (2, 0), 6, FILL, GROUND);
            assert_end(&buf, 2, CAP_L, round, FILL, GROUND);
            assert_end(&buf, 7, CAP_R, round, FILL, GROUND);
            for x in 3..7 {
                let cell = &buf[(x, 0)];
                assert_eq!((cell.symbol(), cell.bg), (" ", FILL), "{x}");
            }
            for x in 2..8 {
                assert!(buf[(x, 0)].modifier.is_empty(), "{x}");
            }
            // Outside the pill nothing changed.
            assert_eq!(buf[(1, 0)].symbol(), "x");
            assert_eq!(buf[(8, 0)].symbol(), "x");
        }
        // No room for two ends: nothing is drawn. Past the buffer: clipped.
        let theme = Theme::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 1));
        row_pill(ROUND, &theme, &mut buf, (0, 0), 1, FILL, GROUND);
        assert_eq!(buf, Buffer::empty(Rect::new(0, 0, 4, 1)));
        row_pill(ROUND, &theme, &mut buf, (2, 0), 9, FILL, GROUND);
        assert_eq!(buf[(2, 0)].symbol(), CAP_L);
        assert_eq!(buf[(3, 0)].bg, FILL);
    }

    #[test]
    fn a_strip_fills_its_row_between_two_ends_and_returns_the_inside() {
        for (look, theme, round) in looks() {
            let mut buf = Buffer::empty(Rect::new(0, 0, 12, 2));
            let inner = strip(look, &theme, &mut buf, Rect::new(1, 0, 10, 2), FILL, GROUND);
            // The inside is the same cells whatever the ends are.
            assert_eq!(inner, Rect::new(2, 0, 8, 1));
            assert_end(&buf, 1, CAP_L, round, FILL, GROUND);
            assert_end(&buf, 10, CAP_R, round, FILL, GROUND);
            for x in inner.left()..inner.right() {
                let cell = &buf[(x, 0)];
                assert_eq!((cell.symbol(), cell.bg), (" ", FILL), "{x}");
            }
            // One row only.
            assert_eq!(buf[(5, 1)].bg, Color::Reset);
            if !round {
                assert_no_caps(&buf);
            }
        }
        // Too narrow for ends and a cell between them: just the fill.
        let mut buf = Buffer::empty(Rect::new(0, 0, 4, 1));
        let area = Rect::new(0, 0, 2, 1);
        let inner = strip(ROUND, &Theme::default(), &mut buf, area, FILL, GROUND);
        assert_eq!(inner, area);
        assert_eq!(row(&buf), "    ");
        assert_eq!((buf[(0, 0)].bg, buf[(1, 0)].bg), (FILL, FILL));
        assert_eq!(buf[(2, 0)].bg, Color::Reset);
    }

    #[test]
    fn a_keycap_is_the_key_in_strong_bold_on_btn() {
        for (look, theme, round) in looks() {
            let cap = keycap(look, &theme, "esc", GROUND);
            assert_eq!(width(&cap), 5);
            let buf = drawn(&cap, 5);
            assert_end(&buf, 0, CAP_L, round, theme.btn, GROUND);
            assert_end(&buf, 4, CAP_R, round, theme.btn, GROUND);
            assert_eq!(text(&cap[1..2]), "esc");
            for x in 1..4 {
                let cell = &buf[(x, 0)];
                assert_eq!((cell.fg, cell.bg), (theme.strong, theme.btn), "{x}");
                assert!(cell.modifier.contains(Modifier::BOLD), "{x}");
            }
        }
    }

    #[test]
    fn keycaps_give_each_key_a_cap_and_four_letters_one() {
        let theme = Theme::default();
        let caps = |keys: &str| text(&keycaps(SQUARE, &theme, keys, GROUND));
        assert_eq!(caps("esc"), " esc ");
        assert_eq!(caps("↑ ↓"), " ↑   ↓ ");
        assert_eq!(caps("h j k l"), " h j k l ");
        // Three letters, or four keys with a longer one among them, stay apart.
        assert_eq!(caps("a b c"), " a   b   c ");
        assert_eq!(caps("h j k tab"), " h   j   k   tab ");
        // Two blanks part sets of keys.
        assert_eq!(caps("h j k l  ⏎"), " h j k l   ⏎ ");
        assert_eq!(caps(""), "");

        for (look, theme, round) in looks() {
            let caps = keycaps(look, &theme, "↑ ↓", GROUND);
            assert_eq!(width(&caps), 7);
            let buf = drawn(&caps, 7);
            assert_end(&buf, 0, CAP_L, round, theme.btn, GROUND);
            assert_end(&buf, 2, CAP_R, round, theme.btn, GROUND);
            // The gap between two caps is the ground, not the caps' fill.
            assert_eq!((buf[(3, 0)].symbol(), buf[(3, 0)].bg), (" ", GROUND));
            assert_end(&buf, 4, CAP_L, round, theme.btn, GROUND);
            assert_end(&buf, 6, CAP_R, round, theme.btn, GROUND);
            assert_eq!(buf[(5, 0)].fg, theme.strong);
            if !round {
                assert_no_caps(&buf);
            }
        }
    }

    #[test]
    fn hints_are_keycaps_and_muted_words_three_blanks_apart() {
        for (look, theme, round) in looks() {
            let ground = theme.raised2;
            let line = hints(look, &theme, &[("⏎", "create"), ("esc", "cancel")], ground);
            // ` ⏎  create    esc  cancel`, the blanks round a key being its ends.
            assert_eq!(width(&line), 25);
            let buf = drawn(&line, 25);
            let ends = if round { (CAP_L, CAP_R) } else { (" ", " ") };
            let (l, r) = ends;
            assert_eq!(row(&buf), format!("{l}⏎{r} create   {l}esc{r} cancel"));
            assert_end(&buf, 0, CAP_L, round, theme.btn, ground);
            assert_end(&buf, 2, CAP_R, round, theme.btn, ground);
            assert_eq!((buf[(1, 0)].fg, buf[(1, 0)].bg), (theme.strong, theme.btn));
            // The word, and the gap before the next key, sit on the ground.
            let word = &buf[(4, 0)];
            assert_eq!(word.bg, ground);
            assert_eq!(word.fg, ink(&theme, theme.muted, ground));
            assert!(!word.modifier.contains(Modifier::BOLD));
            for x in 10..13 {
                assert_eq!((buf[(x, 0)].symbol(), buf[(x, 0)].bg), (" ", ground));
            }
            assert_end(&buf, 13, CAP_L, round, theme.btn, ground);
            assert_end(&buf, 17, CAP_R, round, theme.btn, ground);
        }
    }

    #[test]
    fn ink_stays_readable_where_terminal_colours_collide() {
        let theme = Theme::default();
        assert_eq!(ink(&theme, theme.muted, theme.raised2), theme.muted);
        // mono's `muted` and `raised2` are the same dark grey.
        let mono = mono();
        assert_eq!(mono.muted, mono.raised2);
        assert_eq!(ink(&mono, mono.muted, mono.raised2), mono.text);
        assert_ne!(mono.text, mono.raised2);
    }

    #[test]
    fn put_stops_at_the_edge_and_says_where_it_ended() {
        let spans = [Span::raw("abc"), Span::raw("def")];
        assert_eq!(width(&spans), 6);
        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 1));
        assert_eq!(put(&mut buf, 1, 0, &spans, 5), 5);
        assert_eq!(row(&buf), " abcd   ");
        assert_eq!(put(&mut buf, 1, 0, &spans, 8), 7);
    }
}
