//! The one-row prompt bar above the status line: a strip holding a label and a
//! single line of text being typed, e.g. ` New file  notes.md`, with what Enter
//! and Esc do at the right, each key a keycap.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::buffer::movement::Motion;
use crate::keymap::{Action, Input};
use crate::theme::Theme;
use crate::ui::kit::{self, Look};

/// What a key did to the bar, when it did more than edit the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Enter: the text is the answer.
    Submit,
    /// Esc: close the bar and do nothing.
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptBar {
    pub label: &'static str,
    text: String,
    /// Char index into `text`.
    cursor: usize,
}

impl PromptBar {
    /// A bar holding `text` with the cursor at its end.
    pub fn new(label: &'static str, text: &str) -> Self {
        PromptBar {
            label,
            text: text.to_string(),
            cursor: text.chars().count(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    /// Edits the text for one key; Enter and Esc are left to the caller.
    pub fn handle(&mut self, input: Input) -> Option<Outcome> {
        match input {
            Input::Text(c) => self.insert(c),
            Input::Action(Action::Newline) => return Some(Outcome::Submit),
            Input::Action(Action::Cancel) => return Some(Outcome::Cancel),
            Input::Action(Action::Backspace) if self.cursor > 0 => {
                self.cursor -= 1;
                self.remove_at(self.cursor);
            }
            Input::Action(Action::Delete) => self.remove_at(self.cursor),
            Input::Action(Action::Move(motion)) => {
                let len = self.text.chars().count();
                self.cursor = match motion {
                    Motion::Left => self.cursor.saturating_sub(1),
                    Motion::Right => (self.cursor + 1).min(len),
                    Motion::LineStart | Motion::DocStart => 0,
                    Motion::LineEnd | Motion::DocEnd => len,
                    _ => self.cursor,
                };
            }
            _ => {}
        }
        None
    }

    /// Pasted text goes in as if typed. A line break submits, since the bar holds
    /// one line and on Windows typing followed quickly by Enter arrives as a paste.
    pub fn paste(&mut self, text: &str) -> Option<Outcome> {
        for c in text.chars() {
            match c {
                '\n' => return Some(Outcome::Submit),
                c if c.is_control() => {}
                c => self.insert(c),
            }
        }
        None
    }

    fn insert(&mut self, c: char) {
        let at = self.byte_at(self.cursor);
        self.text.insert(at, c);
        self.cursor += 1;
    }

    fn remove_at(&mut self, index: usize) {
        if index < self.text.chars().count() {
            let at = self.byte_at(index);
            self.text.remove(at);
        }
    }

    fn byte_at(&self, index: usize) -> usize {
        self.text
            .char_indices()
            .nth(index)
            .map_or(self.text.len(), |(at, _)| at)
    }

    /// Draws the prompt bar: a strip of `raised2` on the editor's ground holding
    /// ` Label  text`, with `hint` (each key and what it does: Enter, then Esc)
    /// as keycaps and words at the right, ending one cell short of the strip's
    /// end. Returns where the cursor goes.
    pub fn render(
        &self,
        look: Look,
        theme: &Theme,
        frame: &mut Frame,
        area: Rect,
        hint: &[(&str, &str)],
    ) -> (u16, u16) {
        let buf = frame.buffer_mut();
        let inner = kit::strip(look, theme, buf, area, theme.raised2, theme.bg);
        let hint = kit::hints(look, theme, hint, theme.raised2);
        let hint_width = kit::width(&hint);
        let label_width = u16::try_from(self.label.width()).unwrap_or(u16::MAX);
        // The hint goes before the field is squeezed below its label and a few
        // letters of text.
        let field_width = inner.width.saturating_sub(hint_width.saturating_add(2));
        if field_width < label_width.saturating_add(8) {
            return self.render_field(theme, buf, inner, true, Style::new());
        }
        let hint_x = inner.right() - hint_width - 1;
        kit::put(buf, hint_x, inner.y, &hint, inner.right());
        let field = Rect {
            width: field_width,
            ..inner
        };
        self.render_field(theme, buf, field, true, Style::new())
    }

    /// Draws one field of a bar across `area` on `raised2`, the fill of the strip
    /// it sits in: ` Label  text`, the label in `text` while the field has focus
    /// and `muted` while it doesn't, the text in `strong` patched with `value`.
    /// Returns where the cursor goes when the field has focus.
    pub fn render_field(
        &self,
        theme: &Theme,
        buf: &mut Buffer,
        area: Rect,
        focused: bool,
        value: Style,
    ) -> (u16, u16) {
        let base = Style::new().bg(theme.raised2).fg(theme.strong);
        buf.set_style(area, base);
        if area.width == 0 {
            return (area.x, area.y);
        }
        let quiet = kit::ink(theme, theme.muted, theme.raised2);
        let label = base.fg(if focused { theme.text } else { quiet });
        buf.set_stringn(
            area.x + 1,
            area.y,
            self.label,
            usize::from(area.width - 1),
            label,
        );
        let head = u16::try_from(self.label.width() + 3).unwrap_or(u16::MAX);
        if head >= area.width {
            return (area.right() - 1, area.y);
        }
        let text = Rect {
            x: area.x + head,
            width: area.width - head,
            ..area
        };
        self.render_value(buf, text, base.patch(value))
    }

    /// Draws just the text in `style` from the left of `area`, without the label,
    /// for a dialog's input row. When the text before the cursor doesn't fit,
    /// its start scrolls off so the cursor stays in view. Returns where the
    /// cursor goes.
    pub fn render_value(&self, buf: &mut Buffer, area: Rect, style: Style) -> (u16, u16) {
        if area.width == 0 {
            return (area.x, area.y);
        }
        let room = usize::from(area.width) - 1;
        let mut skip = 0;
        let mut before: usize = self
            .text
            .chars()
            .take(self.cursor)
            .map(|c| c.width().unwrap_or(0))
            .sum();
        for c in self.text.chars().take(self.cursor) {
            if before <= room {
                break;
            }
            before -= c.width().unwrap_or(0);
            skip += 1;
        }
        let shown: String = self.text.chars().skip(skip).collect();
        buf.set_stringn(area.x, area.y, shown, usize::from(area.width), style);
        let col = u16::try_from(before)
            .unwrap_or(u16::MAX)
            .min(area.width - 1);
        (area.x + col, area.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Corners;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Modifier;

    fn act(action: Action) -> Input {
        Input::Action(action)
    }

    #[test]
    fn typing_and_editing_keys_change_the_text_at_the_cursor() {
        let mut bar = PromptBar::new("Rename", "notes.txt");
        for _ in 0..3 {
            assert_eq!(bar.handle(act(Action::Backspace)), None);
        }
        for c in "md".chars() {
            bar.handle(Input::Text(c));
        }
        assert_eq!(bar.text(), "notes.md");
        bar.handle(act(Action::Move(Motion::LineStart)));
        bar.handle(act(Action::Delete));
        bar.handle(Input::Text('N'));
        bar.handle(act(Action::Move(Motion::Right)));
        bar.handle(Input::Text('é'));
        assert_eq!(bar.text(), "Noétes.md");
        bar.handle(act(Action::Move(Motion::LineEnd)));
        bar.handle(act(Action::Delete));
        assert_eq!(bar.text(), "Noétes.md");
        assert_eq!(bar.handle(act(Action::Newline)), Some(Outcome::Submit));
        assert_eq!(bar.handle(act(Action::Cancel)), Some(Outcome::Cancel));
    }

    #[test]
    fn a_paste_with_a_line_break_submits_what_came_before() {
        let mut bar = PromptBar::new("New file", "");
        assert_eq!(bar.paste("notes.md\nrest"), Some(Outcome::Submit));
        assert_eq!(bar.text(), "notes.md");
        let mut bar = PromptBar::new("New file", "");
        assert_eq!(bar.paste("a\tb"), None);
        assert_eq!(bar.text(), "ab");
    }

    const ROUND: Look = Look {
        pill_caps: true,
        corners: Corners::Flush,
        motion: true,
        glide: None,
    };
    const SQUARE: Look = Look {
        pill_caps: false,
        corners: Corners::Flush,
        motion: true,
        glide: None,
    };
    const HINT: [(&str, &str); 2] = [("⏎", "create"), ("Esc", "cancel")];

    /// Draws `bar` with `HINT` across row 1 of a 50-cell screen filled with the
    /// theme's `bg`, as the app's frame is.
    fn drawn(bar: &PromptBar, look: Look, theme: &Theme) -> anyhow::Result<(Buffer, (u16, u16))> {
        let mut terminal = Terminal::new(TestBackend::new(50, 2))?;
        let mut at = (0, 0);
        terminal.draw(|frame| {
            let screen = frame.area();
            frame
                .buffer_mut()
                .set_style(screen, Style::new().bg(theme.bg));
            at = bar.render(look, theme, frame, Rect::new(0, 1, 50, 1), &HINT);
        })?;
        Ok((terminal.backend().buffer().clone(), at))
    }

    fn row(buffer: &Buffer, y: u16) -> String {
        (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect()
    }

    #[test]
    fn renders_a_strip_with_label_text_keycap_hint_and_cursor() -> anyhow::Result<()> {
        let theme = Theme::default();
        let bar = PromptBar::new("New file", "notes.md");
        let (buffer, at) = drawn(&bar, ROUND, &theme)?;
        let (l, r) = (kit::CAP_L, kit::CAP_R);
        // The strip's ends are the first and last cells; the hint ends a cell
        // short of the right one.
        assert_eq!(
            row(&buffer, 1),
            format!(
                "{l}{:<22}{l}⏎{r} create   {l}Esc{r} cancel {r}",
                " New file  notes.md"
            )
        );
        assert_eq!(at, (20, 1));
        // The ends are `raised2` on the ground; everything between is on
        // `raised2`, but for the keycaps, which are `btn`.
        for x in [0, 49] {
            let end = &buffer[(x, 1)];
            assert_eq!((end.fg, end.bg), (theme.raised2, theme.bg), "{x}");
        }
        for x in (1..49).filter(|x| ![24, 37, 38, 39].contains(x)) {
            assert_eq!(buffer[(x, 1)].bg, theme.raised2, "{x}");
        }
        assert_eq!(buffer[(2, 1)].fg, theme.text);
        assert_eq!(buffer[(12, 1)].fg, theme.strong);
        // A keycap: its ends `btn` on the strip, the key `strong` on `btn`, bold.
        for x in [23, 25, 36, 40] {
            let end = &buffer[(x, 1)];
            assert_eq!((end.fg, end.bg), (theme.btn, theme.raised2), "{x}");
        }
        for x in [24, 37, 38, 39] {
            let key = &buffer[(x, 1)];
            assert_eq!((key.fg, key.bg), (theme.strong, theme.btn), "{x}");
            assert!(key.modifier.contains(Modifier::BOLD), "{x}");
        }
        // The words are `muted`.
        assert_eq!(buffer[(27, 1)].fg, theme.muted);
        assert_eq!(buffer[(42, 1)].fg, theme.muted);
        // The row above is untouched.
        assert_eq!(buffer[(0, 0)].symbol(), " ");
        Ok(())
    }

    #[test]
    fn without_pill_caps_or_in_mono_the_bar_has_no_cap_glyphs() -> anyhow::Result<()> {
        let bar = PromptBar::new("New file", "notes.md");
        let mono = Theme::named("mono").expect("mono exists");
        for (look, theme) in [(SQUARE, Theme::default()), (ROUND, mono)] {
            let (buffer, at) = drawn(&bar, look, &theme)?;
            // Every end is a blank, so nothing moved.
            assert_eq!(
                row(&buffer, 1),
                format!("{:<23} ⏎  create    Esc  cancel  ", "  New file  notes.md")
            );
            assert_eq!(at, (20, 1));
            for x in [0, 49] {
                assert_eq!(buffer[(x, 1)].bg, theme.raised2, "{x}");
            }
            for x in [23, 25, 36, 40] {
                assert_eq!(buffer[(x, 1)].bg, theme.btn, "{x}");
            }
            assert_eq!(buffer[(24, 1)].fg, theme.strong);
            // `mono`'s `muted` is its `raised2`, so the words fall back to `text`.
            let word = kit::ink(&theme, theme.muted, theme.raised2);
            assert_eq!(buffer[(27, 1)].fg, word);
            assert_ne!(buffer[(27, 1)].fg, buffer[(27, 1)].bg);
        }
        Ok(())
    }

    #[test]
    fn a_narrow_bar_drops_the_hint_and_an_unfocused_label_is_muted() -> anyhow::Result<()> {
        let theme = Theme::default();
        let bar = PromptBar::new("Rename", "a.md");
        let mut terminal = Terminal::new(TestBackend::new(20, 1))?;
        terminal.draw(|frame| {
            let hint = [("⏎", "rename"), ("Esc", "cancel")];
            bar.render(ROUND, &theme, frame, frame.area(), &hint);
        })?;
        let buffer = terminal.backend().buffer();
        assert_eq!(
            row(buffer, 0),
            format!("{}{:<18}{}", kit::CAP_L, " Rename  a.md", kit::CAP_R)
        );
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        bar.render_field(
            &theme,
            &mut buf,
            Rect::new(0, 0, 20, 1),
            false,
            Style::new(),
        );
        assert_eq!(buf[(1, 0)].fg, theme.muted);
        Ok(())
    }

    #[test]
    fn a_value_too_long_for_its_row_scrolls_to_keep_the_cursor_in_view() {
        let bar = PromptBar::new("Search", "abcdefgh");
        let mut buf = Buffer::empty(Rect::new(0, 0, 5, 1));
        let at = bar.render_value(&mut buf, Rect::new(0, 0, 5, 1), Style::new());
        let row: String = (0..5).map(|x| buf[(x, 0)].symbol()).collect();
        // Four letters and the cursor after them.
        assert_eq!(row, "efgh ");
        assert_eq!(at, (4, 0));
    }
}
