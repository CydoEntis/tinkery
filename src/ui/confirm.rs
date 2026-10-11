//! A confirm card: a panel over the dimmed screen (tinkery-forge spec F16)
//! with a question, a line saying what answering means, and a row of buttons,
//! e.g. `notes.txt has unsaved changes` / ` Save S   Discard D   Cancel Esc `.
//! The focused button (the first, until ← → or Tab move it) is the one Enter
//! presses, and the only one filled: accent, or red when pressing it loses
//! work. The others wait on `btn`.

use std::borrow::Cow;

use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use unicode_width::UnicodeWidthStr;

use crate::buffer::movement::Motion;
use crate::keymap::{Action, Input};
use crate::theme::Theme;
use crate::ui::kit::{self, ButtonKind, Look};

/// What pressing a button does to the user's work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Goes ahead, losing nothing.
    Action,
    /// Goes ahead and loses work: unsaved edits, a file, a backup.
    Danger,
    /// Backs out, as Esc does.
    Cancel,
}

/// One button: pressing `key` (either case) picks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choice {
    pub key: char,
    pub label: &'static str,
    pub role: Role,
}

impl Choice {
    pub const fn action(key: char, label: &'static str) -> Choice {
        let role = Role::Action;
        Choice { key, label, role }
    }

    pub const fn danger(key: char, label: &'static str) -> Choice {
        let role = Role::Danger;
        Choice { key, label, role }
    }

    pub const fn cancel(key: char, label: &'static str) -> Choice {
        let role = Role::Cancel;
        Choice { key, label, role }
    }

    /// The key the button shows after its label. The way out shows `Esc`,
    /// which every card takes; its letter still works.
    fn key_label(&self) -> String {
        match self.role {
            Role::Cancel => "Esc".to_string(),
            Role::Action | Role::Danger => self.key.to_ascii_uppercase().to_string(),
        }
    }

    /// How the button is drawn: filled only with the focus on it, and then
    /// red if pressing it loses work.
    fn kind(&self, focused: bool) -> ButtonKind {
        match (focused, self.role) {
            (false, _) => ButtonKind::Normal,
            (true, Role::Danger) => ButtonKind::Danger,
            (true, Role::Action | Role::Cancel) => ButtonKind::Primary,
        }
    }

    /// The button's width: `kit::button`'s ` label key `.
    fn width(&self) -> u16 {
        u16::try_from(self.label.width() + self.key_label().width() + 3).unwrap_or(u16::MAX)
    }
}

/// What the card was answered with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Picked(char),
    /// Esc or a click outside the card: close it and do nothing.
    Dismissed,
}

/// What a key press or click means while the card is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reply {
    Answer(Answer),
    /// Move the focus to the button at this index.
    Focus(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirm {
    /// Built at runtime when it names a file or a count.
    pub question: Cow<'static, str>,
    pub explanation: Cow<'static, str>,
    pub choices: &'static [Choice],
}

/// The card's height: the edge, a blank row, the question, the explanation,
/// two blank rows, the buttons, a blank row and the edge.
const HEIGHT: u16 = 9;
/// The narrowest card.
const MIN_WIDTH: u16 = 44;
/// Where text starts inside the card, from its edge.
const INSET: u16 = 3;
/// The rows of the question, the explanation and the buttons, from the card's
/// top.
const QUESTION_ROW: u16 = 2;
const EXPLANATION_ROW: u16 = 3;
const BUTTON_ROW: u16 = 6;
/// Blank cells between two buttons.
const BUTTON_GAP: u16 = 2;

impl Confirm {
    /// What `input` does with `focused` the focused button. Letters arrive as
    /// typed text and the rest as actions, so the card never looks at key codes.
    pub fn handle(&self, input: Input, focused: usize) -> Option<Reply> {
        let last = self.choices.len().saturating_sub(1);
        match input {
            Input::Text(c) => {
                let c = c.to_ascii_lowercase();
                self.choices
                    .iter()
                    .find(|choice| choice.key.to_ascii_lowercase() == c)
                    .map(|choice| Reply::Answer(Answer::Picked(choice.key)))
            }
            Input::Action(Action::Cancel) => Some(Reply::Answer(Answer::Dismissed)),
            Input::Action(Action::Newline) => self
                .choices
                .get(focused)
                .map(|choice| Reply::Answer(Answer::Picked(choice.key))),
            Input::Action(Action::Move(Motion::Left)) => {
                Some(Reply::Focus(focused.saturating_sub(1)))
            }
            Input::Action(Action::Move(Motion::Right)) => {
                Some(Reply::Focus((focused + 1).min(last)))
            }
            // There's no Shift+Tab binding to go back, so Tab wraps instead of
            // getting stuck on the last button.
            Input::Action(Action::Tab) => {
                Some(Reply::Focus(if focused >= last { 0 } else { focused + 1 }))
            }
            _ => None,
        }
    }

    /// What a left click at `at` does when the card is drawn in `area`: a button
    /// presses it, anywhere outside the card is Esc (SPEC_V1_LAYOUT §7).
    pub fn click(&self, area: Rect, at: Position) -> Option<Reply> {
        let card = self.card(area);
        if !card.contains(at) {
            return Some(Reply::Answer(Answer::Dismissed));
        }
        self.buttons(card)
            .into_iter()
            .zip(self.choices)
            .find(|(button, _)| button.contains(at))
            .map(|(_, choice)| Reply::Answer(Answer::Picked(choice.key)))
    }

    /// Where the card goes in `area`: centred, 9 tall, and as wide as its
    /// longest line or its row of buttons with four cells either side, but at
    /// least 44; kept a cell inside the screen on every side.
    pub fn card(&self, area: Rect) -> Rect {
        let gaps = u16::try_from(self.choices.len().saturating_sub(1)).unwrap_or(0);
        let buttons = self
            .choices
            .iter()
            .map(Choice::width)
            .fold(gaps * BUTTON_GAP, u16::saturating_add);
        let longest = self.question.width().max(self.explanation.width());
        let width = u16::try_from(longest)
            .unwrap_or(u16::MAX)
            .max(buttons)
            .saturating_add(8)
            .max(MIN_WIDTH);
        kit::centred(area, width, HEIGHT)
    }

    /// Each choice's button in `card`, two cells apart, cut at the card's
    /// inset. None when the card is too short to have a button row.
    fn buttons(&self, card: Rect) -> Vec<Rect> {
        let y = card.y + BUTTON_ROW;
        if y + 1 >= card.bottom() {
            return Vec::new();
        }
        let right = card.right().saturating_sub(INSET);
        let mut x = card.x + INSET;
        let mut buttons = Vec::with_capacity(self.choices.len());
        for choice in self.choices {
            let width = choice.width().min(right.saturating_sub(x));
            buttons.push(Rect::new(x, y, width, 1));
            x = x.saturating_add(width + BUTTON_GAP).min(right);
        }
        buttons
    }

    /// Dims `area` and draws the card centred in it: a panel with the question
    /// in `strong` bold, the explanation in `text`, and the buttons, the
    /// `focused` one filled.
    pub fn render(&self, look: Look, theme: &Theme, frame: &mut Frame, area: Rect, focused: usize) {
        let buf = frame.buffer_mut();
        kit::dim(theme, buf, area);
        let card = self.card(area);
        let inside = kit::panel(look, theme, buf, area, (card.width, card.height), "");
        let (x, right) = (card.x + INSET, card.right().saturating_sub(INSET));
        if inside.is_empty() || x >= right {
            return;
        }
        let ground = theme.raised;
        let question = Style::new().fg(theme.strong).add_modifier(Modifier::BOLD);
        let explanation = Style::new().fg(theme.text);
        for (row, text, style) in [
            (QUESTION_ROW, &self.question, question),
            (EXPLANATION_ROW, &self.explanation, explanation),
        ] {
            let y = card.y + row;
            if y < inside.bottom() {
                buf.set_stringn(x, y, text, usize::from(right - x), style.bg(ground));
            }
        }
        for (i, (button, choice)) in self.buttons(card).into_iter().zip(self.choices).enumerate() {
            let spans = kit::button(
                look,
                theme,
                choice.label,
                &choice.key_label(),
                choice.kind(i == focused),
                false,
                ground,
            );
            kit::put(buf, button.x, button.y, &spans, button.right());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Corners;
    use crate::theme::mix;
    use crate::ui::kit::{CAP_L, CAP_R};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::style::Color;

    const CARD: Confirm = Confirm {
        question: Cow::Borrowed("a.txt has unsaved changes"),
        explanation: Cow::Borrowed("Closing discards them unless you save."),
        choices: &[
            Choice::action('s', "Save"),
            Choice::danger('d', "Discard"),
            Choice::cancel('c', "Cancel"),
        ],
    };

    const FLUSH: Look = Look {
        pill_caps: true,
        corners: Corners::Flush,
    };
    const SQUARE: Look = Look {
        pill_caps: false,
        corners: Corners::Flush,
    };
    const ROUNDED: Look = Look {
        pill_caps: true,
        corners: Corners::Rounded,
    };

    /// At 100x30 the card is 46 wide and 9 tall at (27, 10): its text starts
    /// at column 30, and ` Save S `, ` Discard D ` and ` Cancel Esc ` on row 16
    /// start at 30, 40 and 53.
    const X: u16 = 27;
    const Y: u16 = 10;
    const TEXT_X: u16 = X + 3;
    const BUTTONS_Y: u16 = Y + 6;
    const SAVE_X: u16 = 30;
    const DISCARD_X: u16 = 40;
    const CANCEL_X: u16 = 53;

    fn mono() -> Theme {
        Theme::named("mono").expect("mono exists")
    }

    /// The card over a screen of `x`s in the theme's editor colours.
    fn draw(look: Look, theme: &Theme, focused: usize) -> anyhow::Result<Buffer> {
        let mut terminal = Terminal::new(TestBackend::new(100, 30))?;
        terminal.draw(|frame| {
            let area = frame.area();
            let behind = Style::new().fg(theme.fg).bg(theme.bg);
            for y in 0..area.height {
                frame
                    .buffer_mut()
                    .set_string(0, y, "x".repeat(usize::from(area.width)), behind);
            }
            CARD.render(look, theme, frame, area, focused);
        })?;
        Ok(terminal.backend().buffer().clone())
    }

    fn row(buffer: &Buffer, y: u16) -> String {
        (0..100).map(|x| buffer[(x, y)].symbol()).collect()
    }

    /// The cells of row `y` from `x`, `count` of them.
    fn cells(buffer: &Buffer, x: u16, y: u16, count: u16) -> String {
        (x..x + count).map(|x| buffer[(x, y)].symbol()).collect()
    }

    #[test]
    fn letters_pick_in_either_case_and_esc_dismisses() {
        let picked = |c| Some(Reply::Answer(Answer::Picked(c)));
        assert_eq!(CARD.handle(Input::Text('s'), 0), picked('s'));
        assert_eq!(CARD.handle(Input::Text('D'), 0), picked('d'));
        // The way out shows `Esc`, and its letter still presses it.
        assert_eq!(CARD.handle(Input::Text('c'), 0), picked('c'));
        assert_eq!(CARD.handle(Input::Text('x'), 0), None);
        assert_eq!(
            CARD.handle(Input::Action(Action::Cancel), 1),
            Some(Reply::Answer(Answer::Dismissed))
        );
    }

    #[test]
    fn arrows_and_tab_move_the_focus_and_enter_presses_it() {
        let left = Input::Action(Action::Move(Motion::Left));
        let right = Input::Action(Action::Move(Motion::Right));
        let tab = Input::Action(Action::Tab);
        assert_eq!(CARD.handle(right, 0), Some(Reply::Focus(1)));
        assert_eq!(CARD.handle(right, 2), Some(Reply::Focus(2)));
        assert_eq!(CARD.handle(left, 0), Some(Reply::Focus(0)));
        assert_eq!(CARD.handle(left, 2), Some(Reply::Focus(1)));
        assert_eq!(CARD.handle(tab, 1), Some(Reply::Focus(2)));
        assert_eq!(CARD.handle(tab, 2), Some(Reply::Focus(0)));
        assert_eq!(
            CARD.handle(Input::Action(Action::Newline), 1),
            Some(Reply::Answer(Answer::Picked('d')))
        );
    }

    #[test]
    fn the_card_is_at_least_44_wide_and_grows_with_its_longest_line_or_its_buttons() {
        let area = Rect::new(0, 0, 100, 30);
        // The explanation, 38 cells, is the longest: 38 + 8 = 46.
        assert_eq!(CARD.card(area), Rect::new(X, Y, 46, 9));
        let short = Confirm {
            question: Cow::Borrowed("Short?"),
            explanation: Cow::Borrowed(""),
            ..CARD
        };
        // Its buttons are 35 cells: 35 + 8 is under the least width.
        assert_eq!(short.card(area), Rect::new(28, 10, 44, 9));
        // ` Save everything S `, ` Discard D ` and ` Cancel Esc ` are 46.
        const WIDE: &[Choice] = &[
            Choice::action('s', "Save everything"),
            Choice::danger('d', "Discard"),
            Choice::cancel('c', "Cancel"),
        ];
        let wide = Confirm {
            choices: WIDE,
            ..short.clone()
        };
        assert_eq!(wide.card(area), Rect::new(23, 10, 54, 9));
        let last = wide.buttons(wide.card(area))[2];
        assert_eq!(last.right(), wide.card(area).right() - 5);
        // Clamped a cell inside a small screen.
        assert_eq!(CARD.card(Rect::new(0, 0, 40, 6)), Rect::new(1, 1, 38, 4));
    }

    #[test]
    fn a_button_is_as_wide_as_the_kit_draws_it_and_shows_its_key() {
        let theme = Theme::default();
        let buttons = CARD.buttons(CARD.card(Rect::new(0, 0, 100, 30)));
        let starts: Vec<u16> = buttons.iter().map(|button| button.x).collect();
        assert_eq!(starts, [SAVE_X, DISCARD_X, CANCEL_X]);
        let keys: Vec<String> = CARD.choices.iter().map(Choice::key_label).collect();
        assert_eq!(keys, ["S", "D", "Esc"]);
        for (button, choice) in buttons.iter().zip(CARD.choices) {
            assert_eq!(button.y, BUTTONS_Y);
            for look in [FLUSH, SQUARE] {
                let drawn = kit::button(
                    look,
                    &theme,
                    choice.label,
                    &choice.key_label(),
                    choice.kind(false),
                    false,
                    theme.raised,
                );
                assert_eq!(button.width, kit::width(&drawn), "{}", choice.label);
                assert_eq!(button.width, choice.width(), "{}", choice.label);
            }
        }
    }

    #[test]
    fn only_the_focused_button_is_filled_and_red_when_it_loses_work() {
        let [save, discard, cancel] = [CARD.choices[0], CARD.choices[1], CARD.choices[2]];
        assert_eq!(save.kind(true), ButtonKind::Primary);
        assert_eq!(discard.kind(true), ButtonKind::Danger);
        assert_eq!(cancel.kind(true), ButtonKind::Primary);
        for choice in CARD.choices {
            assert_eq!(choice.kind(false), ButtonKind::Normal, "{}", choice.label);
        }
    }

    #[test]
    fn the_card_is_a_panel_with_its_copy_and_buttons_at_their_offsets() -> anyhow::Result<()> {
        let theme = Theme::default();
        let buffer = draw(SQUARE, &theme, 0)?;
        let black = Color::Rgb(0, 0, 0);
        let behind = mix(theme.bg, black, 0.45);

        // The flush edge: eighth blocks in accent on the dimmed screen, the
        // corners blank.
        assert_eq!(cells(&buffer, X, Y, 46), format!(" {} ", "▁".repeat(44)));
        assert_eq!(
            cells(&buffer, X, Y + 8, 46),
            format!(" {} ", "▔".repeat(44))
        );
        for y in Y + 1..Y + 8 {
            assert_eq!(buffer[(X, y)].symbol(), "▕", "{y}");
            assert_eq!(buffer[(X + 45, y)].symbol(), "▏", "{y}");
            assert_eq!(buffer[(X, y)].fg, theme.accent, "{y}");
            assert_eq!(buffer[(X, y)].bg, behind, "{y}");
        }
        assert_eq!(
            (buffer[(X + 1, Y)].fg, buffer[(X + 1, Y)].bg),
            (theme.accent, behind)
        );
        assert_eq!(buffer[(X, Y)].bg, behind);
        // Inside is `raised`, but for the buttons; a blank row, then the
        // question, then the explanation.
        for y in (Y + 1..Y + 8).filter(|y| *y != BUTTONS_Y) {
            for x in X + 1..X + 45 {
                assert_eq!(buffer[(x, y)].bg, theme.raised, "{x},{y}");
            }
        }
        assert_eq!(cells(&buffer, X + 1, Y + 1, 44), " ".repeat(44));
        assert_eq!(
            cells(&buffer, TEXT_X, Y + 2, 26),
            "a.txt has unsaved changes "
        );
        let question = &buffer[(TEXT_X, Y + 2)];
        assert_eq!(question.fg, theme.strong);
        assert!(question.modifier.contains(Modifier::BOLD));
        assert_eq!(
            cells(&buffer, TEXT_X, Y + 3, 39),
            "Closing discards them unless you save. "
        );
        let explanation = &buffer[(TEXT_X, Y + 3)];
        assert_eq!(explanation.fg, theme.text);
        assert!(!explanation.modifier.contains(Modifier::BOLD));
        for y in [Y + 4, Y + 5, Y + 7] {
            assert_eq!(cells(&buffer, X + 1, y, 44), " ".repeat(44), "{y}");
        }
        // The buttons, the action first and the way out last, two cells apart.
        assert_eq!(
            cells(&buffer, TEXT_X, BUTTONS_Y, 40),
            " Save S    Discard D    Cancel Esc      "
        );
        // No `esc` hint beside them any more: the Cancel button says it.
        assert!(!row(&buffer, BUTTONS_Y).contains("esc"));

        // The screen behind is dimmed: text faded into its ground, then all
        // of it darkened.
        let far = &buffer[(2, 2)];
        assert_eq!(far.symbol(), "x");
        assert_eq!(far.bg, behind);
        assert_eq!(far.fg, mix(mix(theme.fg, theme.bg, 0.6), black, 0.45));
        Ok(())
    }

    #[test]
    fn the_focused_button_is_filled_and_the_others_wait_on_btn() -> anyhow::Result<()> {
        let theme = Theme::default();
        let y = BUTTONS_Y;
        let buffer = draw(FLUSH, &theme, 0)?;
        // ` Save S ` is a pill: caps in accent on the card, acc_ink bold between.
        for (x, cap) in [(SAVE_X, CAP_L), (SAVE_X + 7, CAP_R)] {
            let end = &buffer[(x, y)];
            assert_eq!(end.symbol(), cap);
            assert_eq!((end.fg, end.bg), (theme.accent, theme.raised));
        }
        for x in SAVE_X + 1..SAVE_X + 7 {
            let cell = &buffer[(x, y)];
            assert_eq!((cell.fg, cell.bg), (theme.acc_ink, theme.accent), "{x}");
            assert!(cell.modifier.contains(Modifier::BOLD), "{x}");
            assert!(!cell.modifier.contains(Modifier::UNDERLINED), "{x}");
        }
        assert_eq!(cells(&buffer, SAVE_X + 1, y, 6), "Save S");
        // ` Discard D ` and ` Cancel Esc ` wait: strong on btn, the key in accent.
        for (x, label, key) in [(DISCARD_X, 7, 1), (CANCEL_X, 6, 3)] {
            assert_eq!(buffer[(x, y)].symbol(), CAP_L);
            assert_eq!(
                (buffer[(x, y)].fg, buffer[(x, y)].bg),
                (theme.btn, theme.raised)
            );
            for at in x + 1..x + 1 + label {
                let cell = &buffer[(at, y)];
                assert_eq!((cell.fg, cell.bg), (theme.strong, theme.btn), "{at}");
                assert!(cell.modifier.contains(Modifier::BOLD), "{at}");
            }
            for at in x + 2 + label..x + 2 + label + key {
                let cell = &buffer[(at, y)];
                assert_eq!((cell.fg, cell.bg), (theme.accent, theme.btn), "{at}");
                assert!(cell.modifier.contains(Modifier::BOLD), "{at}");
            }
            assert_eq!(buffer[(x + 2 + label + key, y)].symbol(), CAP_R);
        }
        // The gap between two buttons is the card.
        for x in [SAVE_X + 8, SAVE_X + 9, DISCARD_X + 11, DISCARD_X + 12] {
            assert_eq!(
                (buffer[(x, y)].symbol(), buffer[(x, y)].bg),
                (" ", theme.raised)
            );
        }

        // Moving the focus moves the fill. Discard loses work, so it is red,
        // with text that reads on it; Save goes back to btn.
        let buffer = draw(FLUSH, &theme, 1)?;
        assert_eq!(buffer[(SAVE_X + 1, y)].bg, theme.btn);
        assert_eq!(buffer[(SAVE_X + 1, y)].fg, theme.strong);
        assert_eq!(buffer[(SAVE_X + 6, y)].fg, theme.accent);
        let discard = &buffer[(DISCARD_X + 1, y)];
        assert_eq!((discard.fg, discard.bg), (theme.bg, theme.err));
        assert_eq!(buffer[(DISCARD_X, y)].fg, theme.err);
        assert_eq!(buffer[(DISCARD_X + 9, y)].fg, theme.bg);
        assert_eq!(buffer[(CANCEL_X + 1, y)].bg, theme.btn);
        // Cancel loses nothing: focused, it is the accent.
        let buffer = draw(FLUSH, &theme, 2)?;
        assert_eq!(buffer[(DISCARD_X + 1, y)].bg, theme.btn);
        let cancel = &buffer[(CANCEL_X + 1, y)];
        assert_eq!((cancel.fg, cancel.bg), (theme.acc_ink, theme.accent));
        assert_eq!(buffer[(CANCEL_X + 8, y)].fg, theme.acc_ink);
        Ok(())
    }

    #[test]
    fn rounded_corners_draw_the_edge_as_a_line_and_move_nothing() -> anyhow::Result<()> {
        let theme = Theme::default();
        let (flush, rounded) = (draw(FLUSH, &theme, 0)?, draw(ROUNDED, &theme, 0)?);
        assert_eq!(cells(&rounded, X, Y, 46), format!("╭{}╮", "─".repeat(44)));
        assert_eq!(
            cells(&rounded, X, Y + 8, 46),
            format!("╰{}╯", "─".repeat(44))
        );
        for y in Y + 1..Y + 8 {
            assert_eq!(rounded[(X, y)].symbol(), "│", "{y}");
            assert_eq!(rounded[(X + 45, y)].symbol(), "│", "{y}");
            assert_eq!(rounded[(X, y)].fg, theme.accent, "{y}");
            // Everything inside the edge is the same cell for cell.
            for x in X + 1..X + 45 {
                assert_eq!(rounded[(x, y)], flush[(x, y)], "{x},{y}");
            }
        }
        Ok(())
    }

    #[test]
    fn square_pill_ends_leave_no_cap_glyphs_on_the_card() -> anyhow::Result<()> {
        let theme = Theme::default();
        let buffer = draw(SQUARE, &theme, 0)?;
        for y in 0..30 {
            let row = row(&buffer, y);
            assert!(!row.contains(CAP_L) && !row.contains(CAP_R), "{row:?}");
        }
        // The ends are blanks of the fill instead, so nothing moves.
        let end = &buffer[(SAVE_X, BUTTONS_Y)];
        assert_eq!((end.symbol(), end.bg), (" ", theme.accent));
        let end = &buffer[(DISCARD_X, BUTTONS_Y)];
        assert_eq!((end.symbol(), end.bg), (" ", theme.btn));
        Ok(())
    }

    #[test]
    fn mono_reverses_the_focused_button_and_dims_the_screen_behind() -> anyhow::Result<()> {
        let mono = mono();
        let reversed = |buffer: &Buffer, y: u16| -> String {
            (0..100)
                .filter(|x| buffer[(*x, y)].modifier.contains(Modifier::REVERSED))
                .map(|x| buffer[(x, y)].symbol())
                .collect()
        };
        for look in [FLUSH, SQUARE, ROUNDED] {
            for (focused, button) in [" Save S ", " Discard D ", " Cancel Esc "]
                .iter()
                .enumerate()
            {
                let buffer = draw(look, &mono, focused)?;
                // The focused button is the only reverse video on the card;
                // every button is bold.
                assert_eq!(reversed(&buffer, BUTTONS_Y), *button);
                assert_eq!(
                    cells(&buffer, TEXT_X, BUTTONS_Y, 35),
                    " Save S    Discard D    Cancel Esc "
                );
                for x in [SAVE_X + 1, DISCARD_X + 1, CANCEL_X + 1] {
                    assert!(buffer[(x, BUTTONS_Y)].modifier.contains(Modifier::BOLD));
                }
                for y in 0..30 {
                    let text = row(&buffer, y);
                    assert!(!text.contains(CAP_L) && !text.contains(CAP_R), "{text:?}");
                    for x in 0..100 {
                        let cell = &buffer[(x, y)];
                        // Terminal colours only: nothing is blended.
                        assert!(
                            !matches!(cell.fg, Color::Rgb(..))
                                && !matches!(cell.bg, Color::Rgb(..)),
                            "{x},{y}"
                        );
                        // Everything outside the card is dimmed with the DIM
                        // modifier; the card itself isn't.
                        let on_card = (X..X + 46).contains(&x) && (Y..Y + 9).contains(&y);
                        assert_eq!(cell.modifier.contains(Modifier::DIM), !on_card, "{x},{y}");
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn a_screen_too_small_for_the_card_draws_what_fits() -> anyhow::Result<()> {
        let theme = Theme::default();
        for (width, height) in [(40, 6), (20, 9), (5, 3), (2, 2), (1, 1)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height))?;
            terminal.draw(|frame| CARD.render(FLUSH, &theme, frame, frame.area(), 0))?;
            // No button is offered where none is drawn.
            let area = Rect::new(0, 0, width, height);
            let card = CARD.card(area);
            if card.height < 8 {
                assert!(CARD.buttons(card).is_empty(), "{width}x{height}");
            }
        }
        // At 40x6 the card is 38x4: the question and explanation, cut at the
        // inset, and no buttons.
        let mut terminal = Terminal::new(TestBackend::new(40, 6))?;
        terminal.draw(|frame| CARD.render(SQUARE, &theme, frame, frame.area(), 0))?;
        let buffer = terminal.backend().buffer();
        let line = |y: u16| -> String { (0..40).map(|x| buffer[(x, y)].symbol()).collect() };
        assert_eq!(line(1), format!("  {}  ", "▁".repeat(36)));
        assert!(
            line(3)[6..].starts_with("a.txt has unsaved changes"),
            "{:?}",
            line(3)
        );
        assert_eq!(line(4), format!("  {}  ", "▔".repeat(36)));
        Ok(())
    }

    #[test]
    fn clicks_press_buttons_and_outside_dismisses() {
        let area = Rect::new(0, 0, 100, 30);
        let picked = |c| Some(Reply::Answer(Answer::Picked(c)));
        let y = BUTTONS_Y;
        // A button is pressed anywhere on its pill, ends included.
        assert_eq!(CARD.click(area, Position::new(SAVE_X, y)), picked('s'));
        assert_eq!(CARD.click(area, Position::new(SAVE_X + 7, y)), picked('s'));
        assert_eq!(
            CARD.click(area, Position::new(DISCARD_X + 3, y)),
            picked('d')
        );
        assert_eq!(CARD.click(area, Position::new(CANCEL_X, y)), picked('c'));
        assert_eq!(
            CARD.click(area, Position::new(CANCEL_X + 11, y)),
            picked('c')
        );
        // The gaps, the rest of the card and its edge do nothing.
        assert_eq!(CARD.click(area, Position::new(SAVE_X + 8, y)), None);
        assert_eq!(CARD.click(area, Position::new(CANCEL_X + 12, y)), None);
        assert_eq!(CARD.click(area, Position::new(SAVE_X + 1, y - 1)), None);
        assert_eq!(CARD.click(area, Position::new(SAVE_X + 1, Y + 2)), None);
        assert_eq!(CARD.click(area, Position::new(X, Y)), None);
        assert_eq!(CARD.click(area, Position::new(X + 45, Y + 8)), None);
        // Anywhere off the card is Esc.
        let dismissed = Some(Reply::Answer(Answer::Dismissed));
        assert_eq!(CARD.click(area, Position::new(0, 0)), dismissed);
        assert_eq!(CARD.click(area, Position::new(X - 1, y)), dismissed);
        assert_eq!(CARD.click(area, Position::new(X + 46, y)), dismissed);
        assert_eq!(CARD.click(area, Position::new(SAVE_X, Y + 9)), dismissed);
    }
}
