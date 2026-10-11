//! A split's tab header (tinkery-forge spec F11): a blank row, one pill per
//! open buffer, and a blank row under them.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use crate::theme::Theme;
use crate::ui::kit::{self, Look};

/// Rows a split's header takes above its editor: blank, pills, blank.
pub const HEADER_HEIGHT: u16 = 3;

/// What one tab shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabLabel {
    pub name: String,
    pub dirty: bool,
}

impl TabLabel {
    /// The text between the pill's ends.
    fn text(&self) -> String {
        if self.dirty {
            format!(" {} • ", self.name)
        } else {
            format!(" {} ", self.name)
        }
    }

    /// A pill's cells: the label plus an end either side.
    fn width(&self) -> usize {
        self.text().width() + 2
    }
}

/// The row of a header that holds the pills.
fn pill_row(header: Rect) -> Rect {
    Rect {
        y: header.y + 1,
        height: 1.min(header.height.saturating_sub(1)),
        ..header
    }
}

/// Where each pill lands in a row `width` cells wide, as (column offset of its
/// left end, width); `None` for pills that don't fit. Pills start one cell in and
/// have one blank cell between them (step `len(label) + 3`). When they
/// don't all fit, pills drop off the left until the active one does, so the tab
/// being edited is always on screen.
pub fn layout(labels: &[TabLabel], active: usize, width: u16) -> Vec<Option<(u16, u16)>> {
    let widths: Vec<usize> = labels.iter().map(TabLabel::width).collect();
    let width = usize::from(width);
    // Where the pills from `first` through the active one end: one cell in, each
    // pill plus its gap, less the gap after the last.
    let end = |first: usize| widths[first..=active].iter().map(|w| w + 1).sum::<usize>();
    let mut first = 0;
    while first < active && end(first) > width {
        first += 1;
    }
    let mut x = 1;
    widths
        .iter()
        .enumerate()
        .map(|(index, &w)| {
            if index < first || x >= width {
                return None;
            }
            let at = x;
            x += w + 1;
            // Both fit in u16: `at` is below `width`, which came from a u16, and the
            // width shown is cut to what's left of it.
            let shown = w.min(width - at);
            Some((
                u16::try_from(at).unwrap_or(u16::MAX),
                u16::try_from(shown).unwrap_or(u16::MAX),
            ))
        })
        .collect()
}

/// The tab under screen cell (`col`, `row`) of a header drawn in `header`.
pub fn tab_at(
    labels: &[TabLabel],
    active: usize,
    header: Rect,
    col: u16,
    row: u16,
) -> Option<usize> {
    let pills = pill_row(header);
    if row != pills.y || pills.height == 0 {
        return None;
    }
    let offset = col.checked_sub(pills.x)?;
    layout(labels, active, pills.width)
        .iter()
        .position(|place| place.is_some_and(|(x, w)| (x..x + w).contains(&offset)))
}

/// Draws the header into `header`: the pills on row 1, rows 0 and 2 left as
/// the editor ground. The current tab is the one pill in `accent`; in the
/// split without focus it drops to `btn`, so the accent always names where
/// keys go. The tab at `hovered`, the one under the pointer, is lit unless it
/// is the current one.
pub fn render_tabs(
    look: Look,
    theme: &Theme,
    labels: &[TabLabel],
    (active, hovered): (usize, Option<usize>),
    focused: bool,
    header: Rect,
    frame: &mut Frame,
) {
    let out = frame.buffer_mut();
    out.set_style(header, Style::new().bg(theme.bg).fg(theme.muted));
    let pills = pill_row(header);
    if pills.height == 0 {
        return;
    }
    for (index, place) in layout(labels, active, pills.width).into_iter().enumerate() {
        let Some((x, w)) = place else {
            continue;
        };
        let state = match (index == active, focused) {
            (true, true) => Tab::Current,
            (true, false) => Tab::Unfocused,
            (false, _) => Tab::Other,
        };
        let x = pills.x + x;
        let lit = hovered == Some(index);
        // A pill cut short by the split's edge stops there.
        kit::put(
            out,
            x,
            pills.y,
            &pill(look, theme, &labels[index], state, lit),
            x + w,
        );
    }
}

/// Which of a split's tabs a pill is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    /// The current tab of the split with focus.
    Current,
    /// The current tab of a split without focus.
    Unfocused,
    Other,
}

/// One tab's pill on the editor ground: the name and a dirty `•` between
/// blanks, between the kit's ends. `hovered` turns the fill of a tab that
/// isn't a split's current one to `hov`; `mono` has no fills to turn.
fn pill(
    look: Look,
    theme: &Theme,
    label: &TabLabel,
    state: Tab,
    hovered: bool,
) -> Vec<Span<'static>> {
    let flat = theme.flat();
    if flat && state == Tab::Current {
        // `mono` can't count on `accent` filling a cell, so its current tab is
        // reverse video, ends and dot included: a `warn` dot would cut a
        // coloured block into the fill. The kit's ends take a fill colour, not
        // a modifier, so these two blanks are drawn here.
        let fill =
            Theme::highlight(theme.tab_active_bg, theme.tab_active_fg).add_modifier(Modifier::BOLD);
        return vec![Span::styled(format!(" {} ", label.text()), fill)];
    }
    let (fill, name, dot) = match state {
        Tab::Current => (
            theme.accent,
            Style::new().fg(theme.acc_ink).add_modifier(Modifier::BOLD),
            theme.acc_ink,
        ),
        // Not bold: bold marks only the focused split's name. `mono`'s `btn`
        // is a terminal grey that may not show against the ground, so there
        // the name is underlined as well.
        Tab::Unfocused if flat => (
            theme.btn,
            Style::new()
                .fg(theme.strong)
                .add_modifier(Modifier::UNDERLINED),
            theme.warn,
        ),
        Tab::Unfocused => (theme.btn, Style::new().fg(theme.strong), theme.warn),
        Tab::Other if hovered && !flat => (theme.hov, Style::new().fg(theme.text), theme.warn),
        Tab::Other => (theme.raised, Style::new().fg(theme.text), theme.warn),
    };
    let mut spans = vec![Span::raw(" "), Span::styled(label.name.clone(), name)];
    if label.dirty {
        spans.push(Span::raw(" "));
        spans.push(Span::styled("•", Style::new().fg(dot)));
    }
    spans.push(Span::raw(" "));
    kit::pill(look, theme, spans, fill, theme.bg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::style::Color;

    use crate::ui::kit::{CAP_L, CAP_R};

    const ROUND: Look = Look {
        pill_caps: true,
        corners: crate::config::Corners::Flush,
    };
    const SQUARE: Look = Look {
        pill_caps: false,
        corners: crate::config::Corners::Flush,
    };

    fn label(name: &str, dirty: bool) -> TabLabel {
        TabLabel {
            name: name.into(),
            dirty,
        }
    }

    #[test]
    fn pills_step_by_their_label_plus_three() {
        let labels = [label("a.txt", false), label("b.txt", true)];
        // " a.txt " is 7 wide, 9 with ends; the next starts 10 on.
        assert_eq!(layout(&labels, 0, 40), vec![Some((1, 9)), Some((11, 11))]);
        let header = Rect::new(29, 0, 40, 3);
        assert_eq!(tab_at(&labels, 0, header, 30, 1), Some(0));
        assert_eq!(tab_at(&labels, 0, header, 38, 1), Some(0));
        assert_eq!(tab_at(&labels, 0, header, 39, 1), None);
        assert_eq!(tab_at(&labels, 0, header, 40, 1), Some(1));
        assert_eq!(tab_at(&labels, 0, header, 50, 1), Some(1));
        assert_eq!(tab_at(&labels, 0, header, 51, 1), None);
        assert_eq!(tab_at(&labels, 0, header, 5, 1), None);
        // Only the pill row is clickable.
        assert_eq!(tab_at(&labels, 0, header, 32, 0), None);
        assert_eq!(tab_at(&labels, 0, header, 32, 2), None);
    }

    #[test]
    fn pills_drop_off_the_left_so_the_active_one_shows() {
        let labels: Vec<TabLabel> = (0..5).map(|n| label(&format!("f{n}"), false)).collect();
        // Each pill is 6 cells and a gap; 16 fit two and a bit.
        assert_eq!(
            layout(&labels, 0, 16),
            vec![Some((1, 6)), Some((8, 6)), Some((15, 1)), None, None]
        );
        assert_eq!(
            layout(&labels, 4, 16),
            vec![None, None, None, Some((1, 6)), Some((8, 6))]
        );
    }

    fn draw_in(
        look: Look,
        theme: &Theme,
        labels: &[TabLabel],
        (active, focused): (usize, bool),
        width: u16,
    ) -> Buffer {
        draw_under(look, theme, labels, (active, None), focused, width)
    }

    /// The header with the pointer on the tab at `hovered`.
    fn draw_under(
        look: Look,
        theme: &Theme,
        labels: &[TabLabel],
        (active, hovered): (usize, Option<usize>),
        focused: bool,
        width: u16,
    ) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, 3)).expect("test terminal");
        terminal
            .draw(|frame| {
                render_tabs(
                    look,
                    theme,
                    labels,
                    (active, hovered),
                    focused,
                    Rect::new(0, 0, width, 3),
                    frame,
                )
            })
            .expect("draw");
        terminal.backend().buffer().clone()
    }

    fn draw(theme: &Theme, labels: &[TabLabel], active: usize, focused: bool) -> Buffer {
        draw_in(ROUND, theme, labels, (active, focused), 40)
    }

    #[test]
    fn the_pointer_lights_another_tab_and_never_a_current_one() {
        let labels = [label("a.txt", false), label("b.txt", true)];
        let theme = Theme::default();
        assert_ne!(theme.hov, theme.raised);
        let rest = draw(&theme, &labels, 0, true);
        // `b.txt •`, the other tab, fills columns 11..=21.
        let lit = draw_under(ROUND, &theme, &labels, (0, Some(1)), true, 40);
        assert_pill(&lit, &theme, 11, 21, theme.hov);
        assert_eq!(row(&lit, 1), row(&rest, 1));
        // Its name and dot keep their colours, and the current tab its fill.
        assert_eq!(lit[(13, 1)].fg, theme.text);
        assert_eq!(lit[(19, 1)].fg, theme.warn);
        assert_pill(&lit, &theme, 1, 9, theme.accent);
        // On the current tab, in a split with focus or without, nothing
        // changes.
        for focused in [true, false] {
            let rest = draw(&theme, &labels, 0, focused);
            let over = draw_under(ROUND, &theme, &labels, (0, Some(0)), focused, 40);
            assert_eq!(over, rest, "{focused}");
        }
        // `mono` has no fills to turn.
        let mono = Theme::named("mono").expect("mono exists");
        let rest = draw(&mono, &labels, 0, true);
        let over = draw_under(ROUND, &mono, &labels, (0, Some(1)), true, 40);
        assert_eq!(over, rest);
    }

    fn row(buffer: &Buffer, y: u16) -> String {
        (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect()
    }

    /// Asserts the pill filling columns `from..=to` of the pill row: round ends
    /// in `fill` on the ground, `fill` behind everything between them.
    fn assert_pill(buffer: &Buffer, theme: &Theme, from: u16, to: u16, fill: Color) {
        for (x, cap) in [(from, CAP_L), (to, CAP_R)] {
            let end = &buffer[(x, 1)];
            assert_eq!(end.symbol(), cap, "{x}");
            assert_eq!((end.fg, end.bg), (fill, theme.bg), "{x}");
        }
        for x in from + 1..to {
            assert_eq!(buffer[(x, 1)].bg, fill, "{x}");
        }
    }

    #[test]
    fn the_current_tab_is_an_accent_pill_and_the_others_sit_on_raised() {
        for name in ["tinkery", "tinkery-light"] {
            let theme = Theme::named(name).expect("the theme exists");
            let labels = [label("a.txt", true), label("b.txt", true)];
            let buffer = draw(&theme, &labels, 1, true);
            assert_eq!(row(&buffer, 0).trim(), "");
            assert_eq!(
                row(&buffer, 1).trim_end(),
                format!(" {CAP_L} a.txt • {CAP_R} {CAP_L} b.txt • {CAP_R}")
            );
            // Another tab: `text` on `raised`, not bold, its dot in `warn`.
            assert_pill(&buffer, &theme, 1, 11, theme.raised);
            assert_eq!(buffer[(3, 1)].fg, theme.text);
            assert!(!buffer[(3, 1)].modifier.contains(Modifier::BOLD));
            assert_eq!(buffer[(9, 1)].symbol(), "•");
            assert_eq!(buffer[(9, 1)].fg, theme.warn);
            // The gap between two pills is the ground.
            assert_eq!(buffer[(12, 1)].bg, theme.bg);
            // The current one: `acc_ink` bold on `accent`, its dot in `acc_ink`.
            assert_pill(&buffer, &theme, 13, 23, theme.accent);
            for x in 15..20 {
                assert_eq!(buffer[(x, 1)].fg, theme.acc_ink, "{x}");
                assert!(buffer[(x, 1)].modifier.contains(Modifier::BOLD), "{x}");
            }
            assert_eq!(buffer[(21, 1)].symbol(), "•");
            assert_eq!(buffer[(21, 1)].fg, theme.acc_ink);
        }
    }

    #[test]
    fn the_rows_above_and_below_the_pills_are_blank_ground() {
        for name in ["tinkery", "tinkery-light", "mono"] {
            let theme = Theme::named(name).expect("the theme exists");
            let buffer = draw(&theme, &[label("a.txt", false)], 0, true);
            for y in [0, 2] {
                for x in 0..40 {
                    let cell = &buffer[(x, y)];
                    assert_eq!((cell.symbol(), cell.bg), (" ", theme.bg), "{name} {x},{y}");
                }
            }
        }
    }

    #[test]
    fn the_unfocused_splits_current_tab_is_strong_on_btn() {
        let theme = Theme::default();
        let labels = [label("a.txt", true), label("b.txt", false)];
        let buffer = draw(&theme, &labels, 0, false);
        assert_pill(&buffer, &theme, 1, 11, theme.btn);
        assert_eq!(buffer[(3, 1)].fg, theme.strong);
        assert!(buffer[(3, 1)].modifier.is_empty());
        assert_eq!(buffer[(9, 1)].fg, theme.warn);
        // The other tabs are as they are in the focused split.
        assert_pill(&buffer, &theme, 13, 21, theme.raised);
        let focused = draw(&theme, &labels, 0, true);
        for x in 12..40 {
            assert_eq!(buffer[(x, 1)], focused[(x, 1)], "{x}");
        }
    }

    #[test]
    fn pill_caps_off_squares_every_pill_without_moving_it() {
        let theme = Theme::default();
        let labels = [label("a.txt", false), label("b.txt", true)];
        let round = draw_in(ROUND, &theme, &labels, (1, true), 40);
        let square = draw_in(SQUARE, &theme, &labels, (1, true), 40);
        assert_eq!(row(&square, 1).trim_end(), "   a.txt     b.txt •");
        for (x, fill) in [
            (1, theme.raised),
            (9, theme.raised),
            (11, theme.accent),
            (21, theme.accent),
        ] {
            let end = &square[(x, 1)];
            assert_eq!((end.symbol(), end.bg), (" ", fill), "{x}");
        }
        // Between the ends the two looks are the same cells.
        for x in (2..9).chain(12..21) {
            assert_eq!(square[(x, 1)], round[(x, 1)], "{x}");
        }
    }

    #[test]
    fn a_pill_cut_by_the_edge_stops_there() {
        let theme = Theme::default();
        let labels = [label("a.txt", false), label("b.txt", false)];
        // The second pill starts in column 11 of 14: its end, a blank and `b`.
        let buffer = draw_in(ROUND, &theme, &labels, (0, true), 14);
        assert_eq!(row(&buffer, 1), format!(" {CAP_L} a.txt {CAP_R} {CAP_L} b"));
        assert_eq!(buffer[(13, 1)].bg, theme.raised);
    }

    #[test]
    fn mono_reverses_the_current_pill_bold_and_draws_no_cap() {
        let theme = Theme::named("mono").expect("mono exists");
        let labels = [label("a.txt", false), label("b.txt", false)];
        let buffer = draw(&theme, &labels, 1, true);
        assert_eq!(row(&buffer, 1).trim_end(), "   a.txt     b.txt");
        let reversed: String = (0..40)
            .filter(|&x| buffer[(x, 1)].modifier.contains(Modifier::REVERSED))
            .map(|x| buffer[(x, 1)].symbol())
            .collect();
        assert_eq!(reversed, "  b.txt  ");
        for x in 11..20 {
            assert!(buffer[(x, 1)].modifier.contains(Modifier::BOLD), "{x}");
        }
        // The other tab is terminal colours: `text` on `raised`, squared.
        for x in 1..10 {
            assert_eq!(buffer[(x, 1)].bg, theme.raised, "{x}");
            assert!(buffer[(x, 1)].modifier.is_empty(), "{x}");
        }
        assert_eq!(buffer[(3, 1)].fg, theme.text);
    }

    #[test]
    fn mono_draws_a_dirty_current_pills_dot_in_the_fill() {
        let theme = Theme::named("mono").expect("mono exists");
        let labels = [label("a.txt", false), label("b.txt", true)];
        let buffer = draw(&theme, &labels, 1, true);
        assert_eq!(buffer[(19, 1)].symbol(), "•");
        assert_eq!(buffer[(19, 1)].style(), buffer[(13, 1)].style());
        assert!(buffer[(19, 1)].modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn mono_underlines_the_unfocused_current_name_without_bold() {
        let theme = Theme::named("mono").expect("mono exists");
        let labels = [label("a.txt", true)];
        let buffer = draw(&theme, &labels, 0, false);
        assert_eq!(row(&buffer, 1).trim_end(), "   a.txt •");
        let underlined: String = (0..40)
            .filter(|&x| buffer[(x, 1)].modifier.contains(Modifier::UNDERLINED))
            .map(|x| buffer[(x, 1)].symbol())
            .collect();
        assert_eq!(underlined, "a.txt");
        let name = &buffer[(3, 1)];
        assert_eq!((name.fg, name.bg), (theme.strong, theme.btn));
        assert!(!name.modifier.contains(Modifier::BOLD));
        assert!(!name.modifier.contains(Modifier::REVERSED));
        assert_eq!(buffer[(9, 1)].fg, theme.warn);
    }
}
