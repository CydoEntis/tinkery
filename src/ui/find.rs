//! The find bar: the one-row prompt strip with case and regex chips, the match
//! count and, for a bad regex, why it found nothing. Opened with Ctrl+R it has a
//! Replace field beside the Find field, and Tab moves between them.

use std::ops::Range;

use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::Span;
use ropey::Rope;
use unicode_width::UnicodeWidthStr;

use crate::keymap::{Action, Input};
use crate::search::{self, Query};
use crate::theme::Theme;
use crate::ui::kit::{self, Look};
use crate::ui::prompt::{Outcome, PromptBar};

/// A chip's cells: its two letters between two ends.
const CHIP_WIDTH: u16 = 4;

/// What a key did to the find bar, for the editor to follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Nothing the editor needs to react to.
    Stay,
    /// The current match may have changed: move the cursor to it.
    Jump,
    /// Esc: close the bar, leaving the cursor where it is.
    Close,
    /// Enter in the Replace field: replace the current match, then go to the next.
    Replace,
    /// Replace every match as one undo step.
    ReplaceAll,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindBar {
    bar: PromptBar,
    /// The Replace field, once Ctrl+R has asked for it.
    replace: Option<PromptBar>,
    /// Keys go to the Replace field instead of the Find field.
    in_replace: bool,
    case_sensitive: bool,
    regex: bool,
    /// Every match of the bar's text in the buffer, in order.
    matches: Vec<Range<usize>>,
    /// Index into `matches` of the one the cursor is on.
    current: Option<usize>,
    /// Why the pattern found nothing, when it's an invalid regex.
    error: Option<String>,
    /// Where the cursor was when the bar opened: typing looks for the first match
    /// from here, so refining the text doesn't walk the cursor down the file.
    origin: usize,
}

impl FindBar {
    /// A bar holding `text`, already searched in `rope` from `origin`.
    pub fn new(text: &str, origin: usize, rope: &Rope) -> Self {
        let mut find = FindBar {
            bar: PromptBar::new("Find", text),
            replace: None,
            in_replace: false,
            case_sensitive: false,
            regex: false,
            matches: Vec::new(),
            current: None,
            error: None,
            origin,
        };
        find.search(rope);
        find
    }

    /// The match the cursor should be on, if any.
    pub fn current_match(&self) -> Option<Range<usize>> {
        self.matches.get(self.current?).cloned()
    }

    pub fn matches(&self) -> &[Range<usize>] {
        &self.matches
    }

    /// Adds the Replace field if it isn't there yet; `focus` moves typing into it.
    pub fn show_replace(&mut self, focus: bool) {
        self.replace
            .get_or_insert_with(|| PromptBar::new("Replace", ""));
        if focus {
            self.in_replace = true;
        }
    }

    /// Every match with the text that replaces it, `$1` groups expanded in regex
    /// mode. Empty while the bar has no Replace field.
    pub fn replacements(&self, rope: &Rope) -> Vec<(Range<usize>, String)> {
        let Some(replace) = &self.replace else {
            return Vec::new();
        };
        search::replacements(rope, &self.query(), replace.text()).unwrap_or_default()
    }

    /// The current match with the text that replaces it.
    pub fn current_replacement(&self, rope: &Rope) -> Option<(Range<usize>, String)> {
        let current = self.current_match()?;
        self.replacements(rope)
            .into_iter()
            .find(|(range, _)| *range == current)
    }

    /// Searches again after the buffer changed, taking the first match from
    /// `origin` as the current one.
    pub fn research_from(&mut self, rope: &Rope, origin: usize) {
        self.origin = origin;
        self.search(rope);
    }

    fn query(&self) -> Query {
        Query {
            pattern: self.bar.text().to_string(),
            case_sensitive: self.case_sensitive,
            regex: self.regex,
        }
    }

    /// One key: edits re-run the search, the find keys move or toggle. The buffer
    /// only changes while the bar is open through the replace steps it returns.
    pub fn handle(&mut self, input: Input, rope: &Rope) -> Step {
        match input {
            Input::Action(Action::FindNext) if self.in_replace => Step::Replace,
            Input::Action(Action::FindNext) => self.step(1),
            Input::Action(Action::FindPrev) => self.step(-1),
            Input::Action(Action::FindCase) => {
                self.case_sensitive = !self.case_sensitive;
                self.search(rope)
            }
            Input::Action(Action::FindRegex) => {
                self.regex = !self.regex;
                self.search(rope)
            }
            Input::Action(Action::ReplaceAll) if self.replace.is_some() => Step::ReplaceAll,
            Input::Action(Action::Replace) => {
                self.show_replace(true);
                Step::Stay
            }
            Input::Action(Action::Tab) => {
                self.in_replace = self.replace.is_some() && !self.in_replace;
                Step::Stay
            }
            input if self.in_replace => match self.replace.as_mut().map(|bar| bar.handle(input)) {
                Some(Some(Outcome::Cancel)) => Step::Close,
                // Enter only gets here when `find_next` was moved off it.
                Some(Some(Outcome::Submit)) => Step::Replace,
                _ => Step::Stay,
            },
            input => {
                let before = self.bar.text().to_string();
                match self.bar.handle(input) {
                    Some(Outcome::Cancel) => Step::Close,
                    // Enter only gets here when `find_next` was moved off it.
                    Some(Outcome::Submit) => self.step(1),
                    None if self.bar.text() != before => self.search(rope),
                    None => Step::Stay,
                }
            }
        }
    }

    /// Pasted text goes in as typed; a line break in it is dropped, since the bar
    /// holds one line. A tab moves between the fields as the key does: on Windows
    /// Tab followed quickly by typing arrives as one paste.
    pub fn paste(&mut self, text: &str, rope: &Rope) -> Step {
        let line = text.lines().next().unwrap_or_default();
        let mut step = Step::Stay;
        for (i, part) in line.split('\t').enumerate() {
            if i > 0 {
                self.in_replace = self.replace.is_some() && !self.in_replace;
            }
            if self.paste_into_field(part, rope) == Step::Jump {
                step = Step::Jump;
            }
        }
        step
    }

    fn paste_into_field(&mut self, text: &str, rope: &Rope) -> Step {
        if self.in_replace {
            if let Some(replace) = &mut self.replace {
                let _ = replace.paste(text);
            }
            return Step::Stay;
        }
        let before = self.bar.text().to_string();
        let _ = self.bar.paste(text);
        if self.bar.text() == before {
            Step::Stay
        } else {
            self.search(rope)
        }
    }

    fn search(&mut self, rope: &Rope) -> Step {
        (self.matches, self.error) = match search::find_all(rope, &self.query()) {
            Ok(matches) => (matches, None),
            Err(err) => (Vec::new(), Some(err)),
        };
        self.current = search::next_from(&self.matches, self.origin);
        Step::Jump
    }

    /// Moves `delta` matches along, wrapping at either end.
    fn step(&mut self, delta: isize) -> Step {
        let len = self.matches.len();
        let Some(current) = self.current else {
            return Step::Stay;
        };
        let next = (current.cast_signed() + delta).rem_euclid(len.cast_signed());
        self.current = Some(next.cast_unsigned());
        // Retyping should now search on from here, not from where the bar opened.
        self.origin = self.matches[next.cast_unsigned()].start;
        Step::Jump
    }

    /// What the right end of the bar says about the matches: `n/m`, or the error.
    fn status(&self) -> String {
        if let Some(err) = &self.error {
            return err.clone();
        }
        if self.bar.text().is_empty() {
            return String::new();
        }
        match self.current {
            Some(i) => format!("{}/{}", i + 1, self.matches.len()),
            None => "0/0".to_string(),
        }
    }

    /// How many cells the chips and the count take at the right of the strip:
    /// ` Aa   .*   1/3 `.
    fn right_width(&self) -> u16 {
        let count = u16::try_from(self.status().width()).unwrap_or(u16::MAX);
        (2 * CHIP_WIDTH + 4).saturating_add(count)
    }

    /// Where the `Aa` chip starts on a strip whose inside is `inside`.
    fn chips_x(&self, inside: Rect) -> u16 {
        inside
            .right()
            .saturating_sub(self.right_width())
            .max(inside.x)
    }

    /// The chip under `at` when the bar is drawn in `area`: 0 for `Aa`, 1 for
    /// `.*`. The one the pointer there lights.
    pub fn chip_at(&self, area: Rect, at: Position) -> Option<usize> {
        let inside = kit::strip_inside(area);
        if !inside.contains(at) {
            return None;
        }
        let offset = at.x.checked_sub(self.chips_x(inside))?;
        let (chip, within) = (offset / (CHIP_WIDTH + 1), offset % (CHIP_WIDTH + 1));
        (chip < 2 && within < CHIP_WIDTH).then_some(usize::from(chip))
    }

    /// Draws a strip of `raised2` on the editor's ground holding ` Find  text`
    /// (and, when it's there, a `│` rule at the middle and ` Replace  text` after
    /// it) with, at the right, the `Aa` and `.*` chips (`acc_ink` on `accent`
    /// while on, `muted` on `btn` while off) and the match count, and returns
    /// where the cursor goes. The chip at `hovered`, the one under the
    /// pointer, is lit while it's off.
    pub fn render(
        &self,
        look: Look,
        theme: &Theme,
        frame: &mut Frame,
        area: Rect,
        hovered: Option<usize>,
    ) -> (u16, u16) {
        let status = self.status();
        let fill = theme.raised2;
        let base = Style::new().bg(fill);
        let chip = |index: usize, text: &str, on: bool| {
            kit::toggle(look, theme, text, on, hovered == Some(index), fill)
        };
        // A bad pattern is drawn in `err` along with the count saying why.
        let (pattern, status_style) = if self.error.is_some() {
            (Style::new().fg(theme.err), base.fg(theme.err))
        } else {
            (Style::new(), base.fg(theme.strong))
        };
        // Right-aligned, each chip as wide as ` Aa `: ` Aa   .*   1/3 `.
        let mut right = chip(0, "Aa", self.case_sensitive);
        right.push(Span::styled(" ", base));
        right.extend(chip(1, ".*", self.regex));
        right.push(Span::styled("  ", base));
        right.push(Span::styled(status, status_style));
        right.push(Span::styled(" ", base));
        let width = self.right_width();
        let buf = frame.buffer_mut();
        let area = kit::strip(look, theme, buf, area, fill, theme.bg);
        let at = match &self.replace {
            None => {
                let find_area = Rect {
                    width: area.width.saturating_sub(width),
                    ..area
                };
                self.bar.render_field(theme, buf, find_area, true, pattern)
            }
            Some(replace) => {
                // Halves of the whole row, not of what the count leaves, so the
                // Replace field stays put while the count changes width; the
                // Replace field shrinks first as the row narrows.
                let half = area.width / 2;
                let find_area = Rect {
                    width: half,
                    ..area
                };
                let replace_area = Rect {
                    x: area.x + half + 1,
                    width: area.width.saturating_sub(half + 1).saturating_sub(width),
                    ..area
                };
                let find_at =
                    self.bar
                        .render_field(theme, buf, find_area, !self.in_replace, pattern);
                if half < area.width {
                    // `guide` is too close to the strip's fill to show on it.
                    let rule = base.fg(kit::ink(theme, theme.line2, fill));
                    buf.set_string(area.x + half, area.y, "│", rule);
                }
                let replace_at =
                    replace.render_field(theme, buf, replace_area, self.in_replace, Style::new());
                if self.in_replace { replace_at } else { find_at }
            }
        };
        // A row too narrow for them cuts the count, not the chips.
        kit::put(buf, self.chips_x(area), area.y, &right, area.right());
        at
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Corners;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Modifier;

    fn rope() -> Rope {
        Rope::from_str("foo bar\nFoo foo\n")
    }

    fn type_text(find: &mut FindBar, rope: &Rope, text: &str) {
        for c in text.chars() {
            find.handle(Input::Text(c), rope);
        }
    }

    #[test]
    fn typing_finds_the_first_match_from_where_the_bar_opened() {
        let rope = rope();
        let mut find = FindBar::new("", 5, &rope);
        assert_eq!(find.current_match(), None);
        type_text(&mut find, &rope, "foo");
        assert_eq!(find.matches(), [0..3, 8..11, 12..15]);
        assert_eq!(find.current_match(), Some(8..11));
        assert_eq!(find.status(), "2/3");
    }

    #[test]
    fn next_and_previous_wrap() {
        let rope = rope();
        let mut find = FindBar::new("foo", 0, &rope);
        assert_eq!(find.current_match(), Some(0..3));
        assert_eq!(
            find.handle(Input::Action(Action::FindPrev), &rope),
            Step::Jump
        );
        assert_eq!(find.current_match(), Some(12..15));
        find.handle(Input::Action(Action::FindNext), &rope);
        assert_eq!(find.current_match(), Some(0..3));
        find.handle(Input::Action(Action::FindNext), &rope);
        assert_eq!(find.status(), "2/3");
    }

    #[test]
    fn toggles_research_and_a_bad_regex_says_so_with_no_matches() {
        let rope = rope();
        let mut find = FindBar::new("foo", 0, &rope);
        find.handle(Input::Action(Action::FindCase), &rope);
        assert_eq!(find.matches(), [0..3, 12..15]);
        find.handle(Input::Action(Action::FindRegex), &rope);
        type_text(&mut find, &rope, "(");
        assert!(find.matches().is_empty());
        assert_eq!(find.current_match(), None);
        assert_eq!(find.status(), "invalid regex");
        find.handle(Input::Action(Action::FindRegex), &rope);
        assert_eq!(find.status(), "0/0");
        assert_eq!(
            find.handle(Input::Action(Action::Cancel), &rope),
            Step::Close
        );
    }

    #[test]
    fn tab_moves_between_fields_and_enter_in_replace_asks_to_replace() {
        let rope = rope();
        let mut find = FindBar::new("foo", 0, &rope);
        let act = Input::Action;
        // Without the Replace field, Tab and Alt+A do nothing.
        assert_eq!(find.handle(act(Action::Tab), &rope), Step::Stay);
        assert_eq!(find.handle(act(Action::ReplaceAll), &rope), Step::Stay);
        assert!(find.replacements(&rope).is_empty());

        find.show_replace(false);
        find.handle(act(Action::Tab), &rope);
        type_text(&mut find, &rope, "x");
        // Typing in Replace doesn't touch the pattern.
        assert_eq!(find.matches(), [0..3, 8..11, 12..15]);
        assert_eq!(
            find.current_replacement(&rope),
            Some((0..3, "x".to_string()))
        );
        assert_eq!(find.handle(act(Action::FindNext), &rope), Step::Replace);
        assert_eq!(
            find.handle(act(Action::ReplaceAll), &rope),
            Step::ReplaceAll
        );
        // Shift+Enter still steps back through matches from the Replace field.
        assert_eq!(find.handle(act(Action::FindPrev), &rope), Step::Jump);
        assert_eq!(find.current_match(), Some(12..15));
        find.handle(act(Action::Tab), &rope);
        type_text(&mut find, &rope, "x");
        assert!(find.matches().is_empty());
        assert_eq!(find.handle(act(Action::Cancel), &rope), Step::Close);
    }

    #[test]
    fn a_pasted_tab_moves_to_the_other_field() {
        let rope = rope();
        let mut find = FindBar::new("", 0, &rope);
        find.show_replace(false);
        assert_eq!(find.paste("foo\tbar", &rope), Step::Jump);
        assert_eq!(find.matches(), [0..3, 8..11, 12..15]);
        assert_eq!(
            find.current_replacement(&rope),
            Some((0..3, "bar".to_string()))
        );
        // Without the Replace field a tab is just dropped.
        let mut find = FindBar::new("", 0, &rope);
        find.paste("fo\to", &rope);
        assert_eq!(find.matches(), [0..3, 8..11, 12..15]);
    }

    #[test]
    fn research_from_picks_the_next_match_after_an_edit() {
        let mut find = FindBar::new("foo", 0, &rope());
        find.show_replace(true);
        let edited = Rope::from_str("x bar\nFoo foo\n");
        find.research_from(&edited, 1);
        assert_eq!(find.matches(), [6..9, 10..13]);
        assert_eq!(find.current_match(), Some(6..9));
    }

    const ROUND: Look = Look {
        pill_caps: true,
        corners: Corners::Flush,
    };
    const SQUARE: Look = Look {
        pill_caps: false,
        corners: Corners::Flush,
    };

    /// Draws `find` across a one-row screen `width` cells wide.
    fn drawn(
        find: &FindBar,
        look: Look,
        theme: &Theme,
        width: u16,
    ) -> anyhow::Result<(ratatui::buffer::Buffer, (u16, u16))> {
        let mut terminal = Terminal::new(TestBackend::new(width, 1))?;
        let mut at = (0, 0);
        terminal.draw(|frame| at = find.render(look, theme, frame, frame.area(), None))?;
        Ok((terminal.backend().buffer().clone(), at))
    }

    #[test]
    fn the_pointer_lights_an_off_chip_where_it_is_drawn() -> anyhow::Result<()> {
        let theme = Theme::default();
        assert_ne!(theme.hov, theme.btn);
        let rope = rope();
        let mut find = FindBar::new("foo", 0, &rope);
        find.handle(Input::Action(Action::FindCase), &rope);
        let area = Rect::new(0, 0, 60, 1);
        let (rest, _) = drawn(&find, SQUARE, &theme, 60)?;
        // `Aa` is on and `.*` off: each chip is the cells its fill covers.
        let chip = |x: u16| find.chip_at(area, Position::new(x, 0));
        for x in 0..60 {
            let fill = rest[(x, 0)].bg;
            let expected = if fill == theme.accent {
                Some(0)
            } else if fill == theme.btn {
                Some(1)
            } else {
                None
            };
            assert_eq!(chip(x), expected, "{x}");
        }
        assert_eq!((0..60).filter(|x| chip(*x) == Some(0)).count(), 4);
        assert_eq!((0..60).filter(|x| chip(*x) == Some(1)).count(), 4);
        assert_eq!(find.chip_at(area, Position::new(50, 1)), None);

        let draw = |theme: &Theme, hovered: Option<usize>| -> anyhow::Result<_> {
            let mut terminal = Terminal::new(TestBackend::new(60, 1))?;
            terminal.draw(|frame| {
                find.render(SQUARE, theme, frame, frame.area(), hovered);
            })?;
            Ok(terminal.backend().buffer().clone())
        };
        // The off chip turns `hov`, its text `text`; nothing else changes.
        let lit = draw(&theme, Some(1))?;
        for x in 0..60 {
            let (was, now) = (&rest[(x, 0)], &lit[(x, 0)]);
            assert_eq!(was.symbol(), now.symbol(), "{x}");
            if chip(x) == Some(1) {
                assert_eq!(now.bg, theme.hov, "{x}");
            } else {
                assert_eq!(was, now, "{x}");
            }
        }
        let dot = (0..60).find(|x| lit[(*x, 0)].symbol() == ".").expect(".*");
        assert_eq!(lit[(dot, 0)].fg, theme.text);
        // The chip that is on stays the accent.
        assert_eq!(draw(&theme, Some(0))?, rest);
        // `mono` has no fills to turn.
        let mono = Theme::named("mono").expect("mono exists");
        assert_eq!(draw(&mono, Some(1))?, draw(&mono, None)?);
        Ok(())
    }

    fn row(buffer: &ratatui::buffer::Buffer) -> String {
        (0..buffer.area.width)
            .map(|x| buffer[(x, 0)].symbol())
            .collect()
    }

    #[test]
    fn renders_find_and_replace_fields_side_by_side() -> anyhow::Result<()> {
        let theme = Theme::default();
        let rope = rope();
        let mut find = FindBar::new("foo", 0, &rope);
        find.show_replace(true);
        find.paste("bar", &rope);
        let (buffer, at) = drawn(&find, ROUND, &theme, 62)?;
        let (l, r) = (kit::CAP_L, kit::CAP_R);
        // Between the strip's ends, Find takes half the row, the rule sits at
        // the middle, and Replace runs up to the chips.
        assert_eq!(
            row(&buffer),
            format!(
                "{l}{:<30}│{:<14}{l}Aa{r} {l}.*{r}  1/3 {r}",
                " Find  foo", " Replace  bar"
            )
        );
        assert_eq!(at, (45, 0));
        assert_eq!(buffer[(31, 0)].fg, theme.line2);
        // The focused field's label is `text`, the other one's `muted`.
        assert_eq!(buffer[(2, 0)].fg, theme.muted);
        assert_eq!(buffer[(33, 0)].fg, theme.text);
        // The strip is `raised2` on the editor's ground.
        for x in [0, 61] {
            let end = &buffer[(x, 0)];
            assert_eq!((end.fg, end.bg), (theme.raised2, theme.bg), "{x}");
        }
        for x in 1..46 {
            assert_eq!(buffer[(x, 0)].bg, theme.raised2, "{x}");
        }
        Ok(())
    }

    #[test]
    fn renders_text_chips_and_count() -> anyhow::Result<()> {
        let theme = Theme::default();
        let rope = rope();
        let mut find = FindBar::new("foo", 0, &rope);
        find.handle(Input::Action(Action::FindCase), &rope);
        let (buffer, at) = drawn(&find, ROUND, &theme, 40)?;
        let (l, r) = (kit::CAP_L, kit::CAP_R);
        assert_eq!(
            row(&buffer),
            format!("{l}{:<23}{l}Aa{r} {l}.*{r}  1/2 {r}", " Find  foo")
        );
        assert_eq!(at, (11, 0));
        // An `on` chip is `acc_ink` on `accent`, bold, its ends `accent` on the
        // strip.
        for x in [25, 26] {
            let on = &buffer[(x, 0)];
            assert_eq!((on.fg, on.bg), (theme.acc_ink, theme.accent), "{x}");
            assert!(on.modifier.contains(Modifier::BOLD), "{x}");
        }
        for x in [24, 27] {
            let end = &buffer[(x, 0)];
            assert_eq!((end.fg, end.bg), (theme.accent, theme.raised2), "{x}");
        }
        // An `off` one is `muted` on `btn`, its ends `btn` on the strip.
        for x in [30, 31] {
            let off = &buffer[(x, 0)];
            assert_eq!((off.fg, off.bg), (theme.muted, theme.btn), "{x}");
            assert!(!off.modifier.contains(Modifier::BOLD), "{x}");
        }
        for x in [29, 32] {
            let end = &buffer[(x, 0)];
            assert_eq!((end.fg, end.bg), (theme.btn, theme.raised2), "{x}");
        }
        // Between the chips, and round the count, is the strip.
        for x in [28, 33, 34, 38] {
            assert_eq!(buffer[(x, 0)].bg, theme.raised2, "{x}");
        }
        assert_eq!(buffer[(35, 0)].fg, theme.strong);
        assert_eq!(buffer[(8, 0)].fg, theme.strong);
        Ok(())
    }

    #[test]
    fn without_pill_caps_or_in_mono_the_bar_has_no_cap_glyphs() -> anyhow::Result<()> {
        let rope = rope();
        let mut find = FindBar::new("foo", 0, &rope);
        find.show_replace(true);
        find.handle(Input::Action(Action::FindCase), &rope);
        let mono = Theme::named("mono").expect("mono exists");
        for (look, theme) in [(SQUARE, Theme::default()), (ROUND, mono)] {
            let (buffer, at) = drawn(&find, look, &theme, 62)?;
            // Every end is a blank of its fill, so nothing moved.
            assert_eq!(
                row(&buffer),
                format!("{:<31}│{:<14} Aa   .*   1/2  ", "  Find  foo", " Replace  ")
            );
            assert_eq!(at, (42, 0));
            for x in [0, 61] {
                assert_eq!(buffer[(x, 0)].bg, theme.raised2, "{x}");
            }
            for x in 46..50 {
                assert_eq!(buffer[(x, 0)].bg, theme.accent, "{x}");
            }
            for x in 51..55 {
                assert_eq!(buffer[(x, 0)].bg, theme.btn, "{x}");
            }
            // Where a theme's quiet colours are its fills (`mono`), the `off`
            // chip, the unfocused label and the rule still show.
            for x in [2, 31, 52] {
                assert_ne!(buffer[(x, 0)].fg, buffer[(x, 0)].bg, "{x}");
            }
        }
        Ok(())
    }

    #[test]
    fn a_bad_regex_draws_the_pattern_and_the_count_in_err() -> anyhow::Result<()> {
        let theme = Theme::default();
        let rope = rope();
        let mut find = FindBar::new("(", 0, &rope);
        find.handle(Input::Action(Action::FindRegex), &rope);
        let (buffer, _) = drawn(&find, ROUND, &theme, 40)?;
        let row = row(&buffer);
        let end = format!("  invalid regex {}", kit::CAP_R);
        assert!(row.ends_with(&end), "{row:?}");
        assert_eq!(buffer[(8, 0)].fg, theme.err);
        for x in 25..38 {
            assert_eq!(buffer[(x, 0)].fg, theme.err, "{x}");
        }
        Ok(())
    }

    #[test]
    fn a_bar_too_narrow_for_its_chips_stays_inside_its_strip() -> anyhow::Result<()> {
        let theme = Theme::default();
        let rope = rope();
        let find = FindBar::new("foo", 0, &rope);
        let (buffer, _) = drawn(&find, ROUND, &theme, 12)?;
        let (l, r) = (kit::CAP_L, kit::CAP_R);
        // The chips keep their place and the count is what gets cut.
        assert_eq!(row(&buffer), format!("{l}{l}Aa{r} {l}.*{r} {r}"));
        Ok(())
    }
}
