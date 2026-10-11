//! A note from an action (saved, not found, failed) as a toast (tinkery-forge
//! spec F22): a pill on `raised2` at the top right of the editor area, its
//! tone's glyph and then the text in `strong` bold. `App` takes it down when
//! `hold` is up; nothing here knows the time.

use std::time::Duration;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::theme::Theme;
use crate::ui::kit::{self, Look};
use crate::ui::status::Tone;

/// Blank cells between the toast and the editor area's right edge, the same
/// margin the status bar keeps on its right.
const MARGIN: u16 = 2;
/// Rows the editor area needs: the one above the toast, the toast, one below.
const MIN_HEIGHT: u16 = 3;
/// The least text worth showing; an area too narrow for it gets no toast.
const MIN_TEXT: u16 = 8;
/// Cells of the pill that aren't text: an end, a blank, the glyph, a blank,
/// then after the text a blank and an end.
const CHROME: u16 = 6;

/// How long a toast stays up: an error longer, since it takes more reading and
/// matters more if missed.
pub fn hold(tone: Tone) -> Duration {
    Duration::from_millis(match tone {
        Tone::Err => 4500,
        Tone::Ok | Tone::Warn => 2500,
    })
}

/// Draws `text` as a toast one row below the top of `area`, right-aligned. A
/// text too long for the area is cut with `…`; an area too small gets nothing.
pub fn render_toast(
    look: Look,
    theme: &Theme,
    tone: Tone,
    text: &str,
    area: Rect,
    buf: &mut Buffer,
) {
    let area = area.intersection(buf.area);
    if area.height < MIN_HEIGHT || area.width < CHROME + MIN_TEXT + 2 * MARGIN {
        return;
    }
    let text = cut(text, usize::from(area.width - CHROME - 2 * MARGIN));
    let y = area.y + 1;
    let x = area.right() - MARGIN - (CHROME + width(&text));
    // Whatever the toast lands on (editor text, the splash, a dialog) is the
    // ground its round ends are cut out of.
    let ground = buf[(x, y)].bg;
    let bold = Style::new().add_modifier(Modifier::BOLD);
    let spans = if theme.flat() {
        // `mono` has no fill to lift a pill off the page and its `raised2` is a
        // grey that would swallow the words: the whole toast is reverse video
        // in the terminal's own colours, the glyph telling the tone.
        let inside = vec![Span::raw(format!(" {} {text} ", tone.glyph()))];
        let reversed = bold.fg(theme.fg).add_modifier(Modifier::REVERSED);
        kit::pill(look, theme, inside, theme.bg, theme.bg)
            .into_iter()
            .map(|span| span.patch_style(reversed))
            .collect()
    } else {
        let inside = vec![
            Span::styled(format!(" {} ", tone.glyph()), bold.fg(tone.color(theme))),
            Span::styled(format!("{text} "), bold.fg(theme.strong)),
        ];
        kit::pill(look, theme, inside, theme.raised2, ground)
    };
    kit::put(buf, x, y, &spans, area.right());
}

/// `text` in at most `max` cells, ending in `…` when some of it had to go. A
/// note is one line; a line break in it would tear the pill.
fn cut(text: &str, max: usize) -> String {
    let text: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if text.width() <= max {
        return text;
    }
    let mut out = String::new();
    let mut used = 1;
    for c in text.chars() {
        let w = c.width().unwrap_or(0);
        if used + w > max {
            break;
        }
        used += w;
        out.push(c);
    }
    out.push('…');
    out
}

fn width(s: &str) -> u16 {
    u16::try_from(s.width()).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Corners;
    use ratatui::style::Color;

    const ROUND: Look = Look {
        pill_caps: true,
        corners: Corners::Flush,
    };
    const SQUARE: Look = Look {
        pill_caps: false,
        corners: Corners::Flush,
    };

    fn theme(name: &str) -> Theme {
        Theme::named(name).unwrap_or_else(|| panic!("no theme {name}"))
    }

    /// A 40×6 screen on `theme`'s ground with the toast drawn over all of it.
    fn drawn(look: Look, theme: &Theme, tone: Tone, text: &str) -> Buffer {
        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        buf.set_style(area, Style::new().bg(theme.bg).fg(theme.fg));
        render_toast(look, theme, tone, text, area, &mut buf);
        buf
    }

    fn row(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width)
            .map(|x| buf[(x, y)].symbol().to_string())
            .collect()
    }

    #[test]
    fn a_toast_is_a_pill_one_row_down_ending_two_cells_from_the_right() {
        let theme = theme("tinkery");
        let buf = drawn(ROUND, &theme, Tone::Ok, "saved a.txt");
        let expected = format!("{} ✓ saved a.txt {}", kit::CAP_L, kit::CAP_R);
        let line = row(&buf, 1);
        assert_eq!(line.trim_start(), format!("{expected}  "));
        // Only that row is touched.
        for y in [0, 2, 3, 4, 5] {
            assert_eq!(row(&buf, y).trim(), "", "row {y}");
        }
        // The ends at 21 and 37, with the last two cells left alone.
        assert_eq!(buf[(21, 1)].symbol(), kit::CAP_L);
        assert_eq!(buf[(37, 1)].symbol(), kit::CAP_R);
        assert_eq!(buf[(38, 1)].bg, theme.bg);
    }

    #[test]
    fn the_pill_is_raised2_with_the_glyph_in_its_tone_and_the_text_strong_bold() {
        for name in ["tinkery", "tinkery-light"] {
            let theme = theme(name);
            for (tone, glyph, color) in [
                (Tone::Ok, "✓", theme.ok),
                (Tone::Warn, "⚠", theme.warn),
                (Tone::Err, "✕", theme.err),
            ] {
                let buf = drawn(ROUND, &theme, tone, "note");
                // 40 − 2 − (6 + 4) = 28: the left end, then a blank, the glyph.
                let cap = &buf[(28, 1)];
                assert_eq!((cap.fg, cap.bg), (theme.raised2, theme.bg), "{name}");
                let mark = &buf[(30, 1)];
                assert_eq!(mark.symbol(), glyph, "{name}");
                assert_eq!((mark.fg, mark.bg), (color, theme.raised2), "{name}");
                for x in 32..36 {
                    let cell = &buf[(x, 1)];
                    assert_eq!((cell.fg, cell.bg), (theme.strong, theme.raised2));
                    assert!(cell.modifier.contains(Modifier::BOLD), "{name} {x}");
                    assert!(!cell.modifier.contains(Modifier::REVERSED));
                }
            }
        }
    }

    #[test]
    fn without_pill_caps_the_ends_are_blanks_of_the_fill() {
        let theme = theme("tinkery");
        let buf = drawn(SQUARE, &theme, Tone::Ok, "note");
        assert_eq!(&row(&buf, 1)[28..], "  ✓ note    ");
        assert_eq!(buf[(28, 1)].bg, theme.raised2);
        assert_eq!(buf[(37, 1)].bg, theme.raised2);
    }

    #[test]
    fn in_mono_the_toast_is_reverse_video_without_caps_or_colours() {
        let theme = theme("mono");
        for tone in [Tone::Ok, Tone::Warn, Tone::Err] {
            let buf = drawn(ROUND, &theme, tone, "note");
            let line = row(&buf, 1);
            assert!(!line.contains(kit::CAP_L) && !line.contains(kit::CAP_R));
            assert!(line.ends_with(&format!("  {} note    ", tone.glyph())));
            for x in 28..38 {
                let cell = &buf[(x, 1)];
                assert!(cell.modifier.contains(Modifier::REVERSED), "{x}");
                assert!(cell.modifier.contains(Modifier::BOLD), "{x}");
                assert_eq!((cell.fg, cell.bg), (Color::Reset, Color::Reset), "{x}");
            }
            assert!(!buf[(27, 1)].modifier.contains(Modifier::REVERSED));
            assert!(!buf[(38, 1)].modifier.contains(Modifier::REVERSED));
        }
    }

    #[test]
    fn the_round_ends_take_the_ground_under_them() {
        let theme = theme("tinkery");
        let area = Rect::new(0, 0, 40, 6);
        let mut buf = Buffer::empty(area);
        buf.set_style(area, Style::new().bg(theme.raised));
        render_toast(ROUND, &theme, Tone::Ok, "note", area, &mut buf);
        assert_eq!(buf[(28, 1)].bg, theme.raised);
        assert_eq!(buf[(37, 1)].bg, theme.raised);
    }

    #[test]
    fn a_long_note_is_cut_to_the_area() {
        let theme = theme("tinkery");
        let buf = drawn(ROUND, &theme, Tone::Err, &"x".repeat(80));
        let line = row(&buf, 1);
        // Two cells clear on each side.
        assert_eq!(buf[(2, 1)].symbol(), kit::CAP_L);
        assert_eq!(buf[(37, 1)].symbol(), kit::CAP_R);
        assert!(line.contains(&format!("{}… ", "x".repeat(29))), "{line:?}");
        assert!(!line.contains(&"x".repeat(30)));
    }

    #[test]
    fn a_line_break_in_a_note_is_a_blank() {
        let theme = theme("tinkery");
        let buf = drawn(ROUND, &theme, Tone::Err, "one\ntwo");
        assert!(row(&buf, 1).contains("✕ one two "));
    }

    #[test]
    fn an_area_too_small_gets_no_toast() {
        let theme = theme("tinkery");
        for area in [Rect::new(0, 0, 40, 2), Rect::new(0, 0, 17, 6)] {
            let mut buf = Buffer::empty(Rect::new(0, 0, 40, 6));
            render_toast(ROUND, &theme, Tone::Ok, "saved a.txt", area, &mut buf);
            for y in 0..6 {
                assert_eq!(row(&buf, y).trim(), "", "{area:?} row {y}");
            }
        }
        // One column more and it fits, cut.
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 6));
        let area = Rect::new(0, 0, 18, 6);
        render_toast(ROUND, &theme, Tone::Ok, "saved a.txt", area, &mut buf);
        assert!(row(&buf, 1).contains("✓ saved a… "), "{:?}", row(&buf, 1));
    }

    #[test]
    fn an_error_holds_longer() {
        assert_eq!(hold(Tone::Ok), Duration::from_millis(2500));
        assert_eq!(hold(Tone::Warn), Duration::from_millis(2500));
        assert_eq!(hold(Tone::Err), Duration::from_millis(4500));
    }
}
