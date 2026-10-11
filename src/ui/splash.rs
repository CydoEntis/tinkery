//! The splash Tinkery opens on when it's started with nothing to edit
//! (tinkery-splash spec S2, as in `design/splash.png`): no card, just one block
//! centred in the editor area: the `tinkery` wordmark in half-block letters, a
//! ramp rule, the project path, and three buttons to pick from (tinkery-forge
//! spec F20).

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use unicode_width::UnicodeWidthStr;

use crate::theme::{Theme, grad, mix};
use crate::ui::kit::{self, ButtonKind, Look};
use crate::ui::tree::{cut, cut_front};

/// The wordmark: `tinkery` drawn in half-block pixels, two pixel rows per text
/// row: the `t`'s top, the `i`'s dot and the `k`'s stem above the x-height,
/// three rows of letter bodies, then a row for the `y`'s descender. Letters
/// are 5 cells wide, bar the 3-cell `t` and the 1-cell `i`, with 2 cells
/// between them. Every row has `WORD_W` characters.
const WORDMARK: [&str; 5] = [
    concat!(
        " ▄ ", "  ", "▀", "  ", "     ", "  ", "█    ", "  ", "     ", "  ", "     ", "  ", "     "
    ),
    concat!(
        "▀█▀",
        "  ",
        "█",
        "  ",
        "█▀▀▀▄",
        "  ",
        "█  ▄▀",
        "  ",
        "▄▀▀▀▄",
        "  ",
        "█▄▀▀▄",
        "  ",
        "█   █"
    ),
    concat!(
        " █ ",
        "  ",
        "█",
        "  ",
        "█   █",
        "  ",
        "█▀▀▄ ",
        "  ",
        "█▀▀▀▀",
        "  ",
        "█    ",
        "  ",
        "█   █"
    ),
    concat!(
        " █▄",
        "  ",
        "█",
        "  ",
        "█   █",
        "  ",
        "█   █",
        "  ",
        "▀▄▄▄▄",
        "  ",
        "█    ",
        "  ",
        "▀▄▄▄█"
    ),
    concat!(
        "   ",
        "  ",
        " ",
        "  ",
        "     ",
        "  ",
        "     ",
        "  ",
        "     ",
        "  ",
        "     ",
        "  ",
        "▄▄▄▄▀"
    ),
];

/// The wordmark's width in cells.
const WORD_W: u16 = 41;

/// The least width the big wordmark is drawn in: it and two cells clear on
/// either side.
const WORD_ROOM: u16 = WORD_W + 4;

/// The rule's width: it runs eleven cells past the wordmark on either side.
const RULE_W: u16 = 63;

/// The action rows' width: a click anywhere in it runs the row's action, and
/// what a row holds is centred in it.
const ROW_W: u16 = 56;

/// The width of the widest button, `Open directory o` between its two ends;
/// the hints start in one column, `HINT_GAP` clear of it.
const BUTTON_W: u16 = 18;
const HINT_GAP: u16 = 2;

/// Where a row's hint starts, from its button's left end.
const HINT_X: u16 = BUTTON_W + HINT_GAP;

/// One of the splash's actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    NewFile,
    NewDirectory,
    OpenDirectory,
}

impl Item {
    /// The rows, top to bottom.
    pub const ALL: [Item; 3] = [Item::NewFile, Item::NewDirectory, Item::OpenDirectory];

    pub fn label(self) -> &'static str {
        match self {
            Item::NewFile => "New file",
            Item::NewDirectory => "New directory",
            Item::OpenDirectory => "Open directory",
        }
    }

    /// The letter shown on the button; the keymap binds the same one.
    pub fn key(self) -> &'static str {
        match self {
            Item::NewFile => "n",
            Item::NewDirectory => "d",
            Item::OpenDirectory => "o",
        }
    }

    /// What the row does, beside its button: where a new file or folder goes.
    fn hint(self, project: &str, room: usize) -> String {
        match self {
            Item::NewFile | Item::NewDirectory => {
                format!("in {}", cut_front(project, room.saturating_sub(3)))
            }
            Item::OpenDirectory => cut("choose a folder", room),
        }
    }
}

/// What one row of the splash holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Line {
    /// One of the wordmark's five rows.
    Word(usize),
    /// `✦ tinkery` on one row, for when the wordmark doesn't fit.
    Brand,
    Rule,
    Blank,
    Path,
    Item(Item),
}

use Line::{Blank, Brand, Path, Rule, Word};

/// One size of the splash: its rows, and whether the action rows carry hints.
struct Step {
    lines: &'static [Line],
    hints: bool,
}

impl Step {
    fn big(&self) -> bool {
        self.lines.contains(&Word(0))
    }

    fn fits(&self, area: Rect) -> bool {
        self.lines.len() <= usize::from(area.height)
            && (!self.big() || area.width >= WORD_ROOM)
            && (!self.hints || area.width >= ROW_W)
    }
}

const FULL: &[Line] = &[
    Word(0),
    Word(1),
    Word(2),
    Word(3),
    Word(4),
    Rule,
    Blank,
    Path,
    Blank,
    Line::Item(Item::NewFile),
    Blank,
    Line::Item(Item::NewDirectory),
    Blank,
    Line::Item(Item::OpenDirectory),
];

const SMALL: &[Line] = &[
    Brand,
    Blank,
    Path,
    Blank,
    Line::Item(Item::NewFile),
    Blank,
    Line::Item(Item::NewDirectory),
    Blank,
    Line::Item(Item::OpenDirectory),
];

const NO_PATH: &[Line] = &[
    Brand,
    Blank,
    Line::Item(Item::NewFile),
    Blank,
    Line::Item(Item::NewDirectory),
    Blank,
    Line::Item(Item::OpenDirectory),
];

const TIGHT: &[Line] = &[
    Brand,
    Line::Item(Item::NewFile),
    Line::Item(Item::NewDirectory),
    Line::Item(Item::OpenDirectory),
];

/// The splash's sizes, biggest first: with less room the wordmark becomes
/// `✦ tinkery`, then the hints go, then the path (spec S2). The last is the least
/// the splash can be; with fewer rows than that it's cut off at the bottom.
const STEPS: &[Step] = &[
    Step {
        lines: FULL,
        hints: true,
    },
    Step {
        lines: SMALL,
        hints: true,
    },
    Step {
        lines: SMALL,
        hints: false,
    },
    Step {
        lines: NO_PATH,
        hints: false,
    },
    Step {
        lines: TIGHT,
        hints: false,
    },
];

/// Where the splash goes in an editor area.
struct Layout {
    step: &'static Step,
    /// The row the first line is on.
    top: u16,
    /// The action rows' span across the area.
    row_x: u16,
    row_w: u16,
}

impl Layout {
    fn new(area: Rect) -> Layout {
        let step = STEPS
            .iter()
            .find(|step| step.fits(area))
            .unwrap_or(&STEPS[STEPS.len() - 1]);
        let height = u16::try_from(step.lines.len()).unwrap_or(u16::MAX);
        let row_w = ROW_W.min(area.width);
        Layout {
            step,
            top: area.y + area.height.saturating_sub(height) / 2,
            row_x: centred(area, row_w),
            row_w,
        }
    }

    /// Each line with the row it's on, as far as the area goes.
    fn rows(&self, area: Rect) -> impl Iterator<Item = (u16, Line)> + '_ {
        (self.top..area.bottom()).zip(self.step.lines.iter().copied())
    }
}

/// What the action rows hold and where: the buttons share a left edge and the
/// hints a column, and the block the two make is centred in the rows.
struct Actions {
    /// The buttons' left edge.
    x: u16,
    /// The column after the rows' last.
    end: u16,
    /// Each action's hint, in `Item::ALL`'s order; empty at a size without.
    hints: [String; 3],
}

impl Actions {
    fn new(layout: &Layout, project: &str) -> Actions {
        let room = usize::from(layout.row_w.saturating_sub(HINT_X));
        let hints = Item::ALL.map(|item| {
            if layout.step.hints {
                item.hint(project, room)
            } else {
                String::new()
            }
        });
        let widest = hints.iter().map(|hint| hint.width()).max().unwrap_or(0);
        let block = match u16::try_from(widest).unwrap_or(u16::MAX) {
            0 => BUTTON_W,
            hint => HINT_X.saturating_add(hint),
        };
        Actions {
            x: layout.row_x + layout.row_w.saturating_sub(block) / 2,
            end: layout.row_x + layout.row_w,
            hints,
        }
    }
}

/// The x that centres something `width` wide in `area`.
fn centred(area: Rect, width: u16) -> u16 {
    area.x + area.width.saturating_sub(width) / 2
}

/// The splash's state: which row Enter runs.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Splash {
    selected: usize,
}

impl Splash {
    pub fn selected(&self) -> Item {
        Item::ALL[self.selected % Item::ALL.len()]
    }

    pub fn select(&mut self, item: Item) {
        self.selected = Item::ALL.iter().position(|&i| i == item).unwrap_or(0);
    }

    /// Moves the selection `delta` rows, wrapping at either end.
    pub fn move_by(&mut self, delta: isize) {
        let len = Item::ALL.len() as isize;
        // `rem_euclid` keeps a step up from the first row on the last one.
        let at = (self.selected as isize + delta).rem_euclid(len);
        self.selected = usize::try_from(at).unwrap_or(0);
    }

    /// The action whose row is at `at`, for a click.
    pub fn item_at(area: Rect, at: Position) -> Option<Item> {
        if !area.contains(at) {
            return None;
        }
        let layout = Layout::new(area);
        if at.x < layout.row_x || at.x >= layout.row_x + layout.row_w {
            return None;
        }
        layout.rows(area).find_map(|(y, line)| match line {
            Line::Item(item) if y == at.y => Some(item),
            _ => None,
        })
    }

    /// Draws the splash centred in `area`, naming `project` (the brand label:
    /// the home folder shown as `~`) under the wordmark and in the hints.
    pub fn render(&self, look: Look, theme: &Theme, project: &str, frame: &mut Frame, area: Rect) {
        let area = area.intersection(frame.area());
        if area.is_empty() {
            return;
        }
        let buf = frame.buffer_mut();
        // The splash has the editor area to itself, so whatever was drawn there
        // before is wiped to the editor ground.
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buf[(x, y)].reset();
            }
        }
        buf.set_style(area, Style::new().bg(theme.bg));
        let layout = Layout::new(area);
        let actions = Actions::new(&layout, project);
        for (y, line) in layout.rows(area) {
            match line {
                Word(row) => wordmark_row(theme, buf, area, row, y),
                Brand => brand(theme, buf, area, y),
                Rule => rule(theme, buf, area, y),
                Blank => {}
                Path => path(theme, buf, area, project, y),
                Line::Item(item) => self.item_row(look, theme, buf, &actions, item, y),
            }
        }
    }

    /// One action row: the action as a button with its key on it, primary
    /// when it's the selected one, and its hint in `muted` beside it. `mono`
    /// has no fills, so there the selected button is reverse video.
    fn item_row(
        &self,
        look: Look,
        theme: &Theme,
        buf: &mut Buffer,
        actions: &Actions,
        item: Item,
        y: u16,
    ) {
        let kind = if item == self.selected() {
            ButtonKind::Primary
        } else {
            ButtonKind::Normal
        };
        let button = kit::button(look, theme, item.label(), item.key(), kind, false, theme.bg);
        kit::put(buf, actions.x, y, &button, actions.end);
        let hint = &actions.hints[item as usize];
        let hint_x = actions.x.saturating_add(HINT_X);
        if !hint.is_empty() && hint_x < actions.end {
            let room = usize::from(actions.end - hint_x);
            buf.set_stringn(hint_x, y, hint, room, Style::new().fg(theme.muted));
        }
    }
}

/// The colour of the wordmark's column `col`, along accent → accent2.
fn word_color(theme: &Theme, col: u16) -> Color {
    grad(
        &[theme.accent, theme.accent2],
        f64::from(col) / f64::from(WORD_W - 1),
    )
}

/// One row of the big wordmark, each column on the ramp; in `mono` the letters
/// take the terminal's own colour.
fn wordmark_row(theme: &Theme, buf: &mut Buffer, area: Rect, row: usize, y: u16) {
    let x0 = centred(area, WORD_W);
    for (col, c) in (0u16..).zip(WORDMARK[row].chars()) {
        let x = x0 + col;
        if c == ' ' || x >= area.right() {
            continue;
        }
        let style = if theme.ramps() {
            Style::new().fg(word_color(theme, col))
        } else {
            Style::new()
        };
        buf[(x, y)]
            .set_symbol(c.encode_utf8(&mut [0; 4]))
            .set_style(style);
    }
}

/// The rule under the wordmark: `─` along the same ramp, fading into the
/// ground at both ends; plain in `mono`.
fn rule(theme: &Theme, buf: &mut Buffer, area: Rect, y: u16) {
    let width = RULE_W.min(area.width);
    let x0 = centred(area, width);
    let span = f64::from(width.saturating_sub(1).max(1));
    for i in 0..width {
        let t = f64::from(i) / span;
        let cell = &mut buf[(x0 + i, y)];
        cell.set_symbol("─");
        if theme.ramps() {
            // Full strength over the middle half, fading over each outer
            // quarter to the cell's own ground.
            let strength = (t.min(1.0 - t) * 4.0).min(1.0);
            let ramp = grad(&[theme.accent, theme.accent2], t);
            cell.set_fg(mix(cell.bg, ramp, strength));
        }
    }
}

/// `✦ tinkery`: the tree brand row's wordmark, its letters ramping accent →
/// accent2 in bold, for when the big one doesn't fit.
fn brand(theme: &Theme, buf: &mut Buffer, area: Rect, y: u16) {
    if area.width < 9 {
        return;
    }
    let x = centred(area, 9);
    buf.set_string(x, y, "✦", Style::new().fg(theme.accent2));
    for (i, c) in (0u16..).zip(["t", "i", "n", "k", "e", "r", "y"]) {
        let fg = grad(&[theme.accent, theme.accent2], f64::from(i) / 6.0);
        let style = Style::new().fg(fg).add_modifier(Modifier::BOLD);
        buf.set_string(x + 2 + i, y, c, style);
    }
}

/// The project path, centred: its last folder in bold `strong`, the rest in
/// `muted`, its start cut with `…` when it doesn't fit.
fn path(theme: &Theme, buf: &mut Buffer, area: Rect, project: &str, y: u16) {
    let room = usize::from(ROW_W.min(area.width));
    let shown = cut_front(project, room);
    // The last folder starts after the last separator; a path cut inside its
    // last folder is all last folder.
    let split = shown.rfind(['/', '\\']).map_or(0, |i| i + 1);
    let (head, last) = shown.split_at(split);
    let width = u16::try_from(shown.width()).unwrap_or(u16::MAX);
    let x = centred(area, width);
    buf.set_string(x, y, head, Style::new().fg(theme.muted));
    let last_x = x + u16::try_from(head.width()).unwrap_or(u16::MAX);
    let strong = Style::new().fg(theme.strong).add_modifier(Modifier::BOLD);
    buf.set_string(last_x, y, last, strong);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    const PROJECT: &str = "~/src";

    fn draw_with(
        splash: &Splash,
        theme: &Theme,
        project: &str,
        w: u16,
        h: u16,
    ) -> anyhow::Result<Buffer> {
        let mut terminal = Terminal::new(TestBackend::new(w, h))?;
        terminal.draw(|frame| {
            let area = Rect::new(0, 0, w, h);
            splash.render(Look::default(), theme, project, frame, area);
        })?;
        Ok(terminal.backend().buffer().clone())
    }

    /// What a button reads between its round ends.
    fn pill(item: Item) -> String {
        format!(
            "{}{} {}{}",
            kit::CAP_L,
            item.label(),
            item.key(),
            kit::CAP_R
        )
    }

    fn draw(splash: &Splash, theme: &Theme, w: u16, h: u16) -> anyhow::Result<Buffer> {
        draw_with(splash, theme, PROJECT, w, h)
    }

    fn row(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width)
            .map(|x| buf[(x, y)].symbol())
            .collect::<String>()
            .trim()
            .to_string()
    }

    fn rows(buf: &Buffer) -> Vec<String> {
        (0..buf.area.height)
            .map(|y| row(buf, y))
            .filter(|text| !text.is_empty())
            .collect()
    }

    /// The column `text` starts at on row `y`.
    fn col(buf: &Buffer, y: u16, text: &str) -> Option<u16> {
        let line: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
        let at = line.find(text)?;
        u16::try_from(line[..at].chars().count()).ok()
    }

    /// RGB distance, for "closer to" checks on blended colours.
    fn distance(a: Color, b: Color) -> f64 {
        let rgb = |c: Color| match c {
            Color::Rgb(r, g, b) => [f64::from(r), f64::from(g), f64::from(b)],
            _ => [0.0; 3],
        };
        let (a, b) = (rgb(a), rgb(b));
        (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
    }

    // In 100 × 26 the 14-row splash starts on row 6: the wordmark on rows 6–10,
    // the rule on 11, the path on 13 and the actions on 15, 17 and 19.
    const TOP: u16 = 6;
    const RULE_Y: u16 = 11;
    const PATH_Y: u16 = 13;
    const ITEM_YS: [u16; 3] = [15, 17, 19];
    /// The wordmark is centred: (100 − 41) / 2.
    const WORD_X: u16 = 29;
    /// The action rows are centred too: (100 − 56) / 2.
    const ROW_X: u16 = 22;
    /// With `~/src` the widest hint is `choose a folder`, so the buttons and
    /// hints are 20 + 15 wide, centred in the rows: 22 + (56 − 35) / 2.
    const BUTTON_X: u16 = 32;
    const HINT_COL: u16 = BUTTON_X + HINT_X;

    #[test]
    fn selection_wraps_both_ways() {
        let mut splash = Splash::default();
        assert_eq!(splash.selected(), Item::NewFile);
        splash.move_by(-1);
        assert_eq!(splash.selected(), Item::OpenDirectory);
        splash.move_by(1);
        assert_eq!(splash.selected(), Item::NewFile);
        splash.move_by(2);
        assert_eq!(splash.selected(), Item::OpenDirectory);
    }

    #[test]
    fn every_wordmark_row_is_as_wide_as_the_wordmark() {
        for row in WORDMARK {
            assert_eq!(row.chars().count(), usize::from(WORD_W), "{row:?}");
            assert_eq!(row.width(), usize::from(WORD_W), "{row:?}");
        }
    }

    #[test]
    fn the_wordmark_is_the_bitmap_with_each_column_on_the_ramp() -> anyhow::Result<()> {
        let theme = Theme::default();
        let buf = draw(&Splash::default(), &theme, 100, 26)?;
        for (y, bitmap) in (TOP..).zip(WORDMARK) {
            let drawn: String = (WORD_X..WORD_X + WORD_W)
                .map(|x| buf[(x, y)].symbol())
                .collect();
            assert_eq!(drawn, bitmap, "row {y}");
            for (col, c) in (0u16..).zip(bitmap.chars()) {
                if c != ' ' {
                    let cell = &buf[(WORD_X + col, y)];
                    assert_eq!(cell.fg, word_color(&theme, col), "column {col}, row {y}");
                }
            }
        }
        // The first column is accent, the last accent2.
        assert_eq!(buf[(WORD_X, TOP + 1)].fg, theme.accent);
        assert_eq!(buf[(WORD_X + WORD_W - 1, TOP + 1)].fg, theme.accent2);
        Ok(())
    }

    #[test]
    fn the_widest_button_is_as_wide_as_the_column_kept_for_it() {
        let theme = Theme::default();
        let widths = Item::ALL.map(|item| {
            let kind = ButtonKind::Normal;
            let button = kit::button(
                Look::default(),
                &theme,
                item.label(),
                item.key(),
                kind,
                false,
                theme.bg,
            );
            kit::width(&button)
        });
        assert_eq!(widths.into_iter().max(), Some(BUTTON_W));
    }

    #[test]
    fn nothing_glows_behind_the_wordmark() -> anyhow::Result<()> {
        for theme in [Theme::default(), Theme::named("mono").expect("mono exists")] {
            let buf = draw(&Splash::default(), &theme, 100, 26)?;
            // Down to the path, every cell is on the editor ground.
            for y in 0..ITEM_YS[0] {
                for x in 0..100 {
                    assert_eq!(buf[(x, y)].bg, theme.bg, "({x}, {y})");
                }
            }
        }
        Ok(())
    }

    #[test]
    fn the_rule_runs_the_ramp_and_fades_at_both_ends() -> anyhow::Result<()> {
        let theme = Theme::default();
        let buf = draw(&Splash::default(), &theme, 100, 26)?;
        // 63 wide, centred: from 18 to 80.
        assert_eq!(row(&buf, RULE_Y), "─".repeat(63));
        assert_eq!(col(&buf, RULE_Y, "─"), Some(18));
        let middle = buf[(49, RULE_Y)].fg;
        assert_eq!(middle, grad(&[theme.accent, theme.accent2], 0.5));
        for end in [18, 80] {
            let cell = &buf[(end, RULE_Y)];
            assert_eq!(cell.fg, cell.bg, "column {end} fades out");
        }
        let near_end = &buf[(22, RULE_Y)];
        assert!(distance(near_end.fg, near_end.bg) < distance(middle, buf[(49, RULE_Y)].bg));
        Ok(())
    }

    #[test]
    fn the_path_dims_all_but_its_last_folder_and_is_cut_at_its_start() -> anyhow::Result<()> {
        let theme = Theme::default();
        let buf = draw(&Splash::default(), &theme, 100, 26)?;
        assert_eq!(row(&buf, PATH_Y), "~/src");
        let x = col(&buf, PATH_Y, "~/src").expect("the path is shown");
        assert_eq!(x, 47);
        for at in [x, x + 1] {
            assert_eq!(buf[(at, PATH_Y)].fg, theme.muted);
        }
        for at in x + 2..x + 5 {
            assert_eq!(buf[(at, PATH_Y)].fg, theme.strong);
            assert!(buf[(at, PATH_Y)].modifier.contains(Modifier::BOLD));
        }

        let long = format!("~/{}last", "deep/".repeat(20));
        let buf = draw_with(&Splash::default(), &theme, &long, 100, 26)?;
        let shown = row(&buf, PATH_Y);
        assert!(shown.starts_with('…'), "{shown}");
        assert!(shown.ends_with("/deep/last"), "{shown}");
        assert_eq!(shown.chars().count(), 56);
        let x = col(&buf, PATH_Y, "last").expect("the last folder is shown");
        assert_eq!(buf[(x, PATH_Y)].fg, theme.strong);
        assert_eq!(buf[(x - 2, PATH_Y)].fg, theme.muted);
        Ok(())
    }

    #[test]
    fn the_actions_are_buttons_with_hints_and_the_selected_one_is_primary() -> anyhow::Result<()> {
        let theme = Theme::default();
        let buf = draw(&Splash::default(), &theme, 100, 26)?;
        // A button, then its hint in the column two clear of the widest one.
        let line = |item: Item, hint: &str| {
            let gap = usize::from(HINT_X) - pill(item).chars().count();
            format!("{}{}{hint}", pill(item), " ".repeat(gap))
        };
        assert_eq!(row(&buf, ITEM_YS[0]), line(Item::NewFile, "in ~/src"));
        assert_eq!(row(&buf, ITEM_YS[1]), line(Item::NewDirectory, "in ~/src"));
        assert_eq!(
            row(&buf, ITEM_YS[2]),
            line(Item::OpenDirectory, "choose a folder")
        );
        for (y, item) in ITEM_YS.into_iter().zip(Item::ALL) {
            assert_eq!(col(&buf, y, &pill(item)), Some(BUTTON_X), "{item:?}");
            assert_eq!(buf[(HINT_COL, y)].fg, theme.muted, "{item:?}");
            assert_eq!(buf[(HINT_COL, y)].bg, theme.bg, "{item:?}");
            // No `✦` and no glow: the row outside the button is the ground.
            assert!(!row(&buf, y).contains('✦'), "{item:?}");
            assert_eq!(buf[(ROW_X, y)].bg, theme.bg, "{item:?}");
            assert_eq!(buf[(ROW_X + ROW_W - 1, y)].bg, theme.bg, "{item:?}");
        }
        // The selected one is primary: `acc_ink` on `accent`, key and all,
        // between round ends in `accent` on the ground.
        let (y, other) = (ITEM_YS[0], ITEM_YS[1]);
        let end = BUTTON_X + 11;
        for x in [BUTTON_X, end] {
            assert_eq!((buf[(x, y)].fg, buf[(x, y)].bg), (theme.accent, theme.bg));
        }
        for x in BUTTON_X + 1..end {
            let cell = &buf[(x, y)];
            assert_eq!((cell.fg, cell.bg), (theme.acc_ink, theme.accent), "{x}");
            assert!(cell.modifier.contains(Modifier::BOLD), "{x}");
        }
        assert_eq!(buf[(end - 1, y)].symbol(), "n");
        // The others are normal: `strong` on `btn`, the key in `accent`.
        let end = BUTTON_X + 16;
        for x in [BUTTON_X, end] {
            let cell = &buf[(x, other)];
            assert_eq!((cell.fg, cell.bg), (theme.btn, theme.bg));
        }
        for x in BUTTON_X + 1..end - 1 {
            let cell = &buf[(x, other)];
            assert_eq!((cell.fg, cell.bg), (theme.strong, theme.btn), "{x}");
            assert!(cell.modifier.contains(Modifier::BOLD), "{x}");
        }
        let key = &buf[(end - 1, other)];
        assert_eq!(key.symbol(), "d");
        assert_eq!((key.fg, key.bg), (theme.accent, theme.btn));
        assert!(key.modifier.contains(Modifier::BOLD));

        // Moving the selection moves the primary button with it.
        let mut splash = Splash::default();
        splash.move_by(1);
        let buf = draw(&splash, &theme, 100, 26)?;
        assert_eq!(buf[(BUTTON_X + 1, ITEM_YS[0])].bg, theme.btn);
        assert_eq!(buf[(BUTTON_X + 1, ITEM_YS[1])].bg, theme.accent);
        Ok(())
    }

    #[test]
    fn the_buttons_and_hints_stay_centred_and_a_long_path_is_cut_to_the_row() -> anyhow::Result<()>
    {
        let theme = Theme::default();
        // A path whose hint is wider than `choose a folder` moves the block
        // left: 20 + `in ~/a/longer/project` (21) is 41 wide, from
        // 22 + (56 − 41) / 2.
        let buf = draw_with(&Splash::default(), &theme, "~/a/longer/project", 100, 26)?;
        assert_eq!(col(&buf, ITEM_YS[0], &pill(Item::NewFile)), Some(29));
        assert_eq!(col(&buf, ITEM_YS[0], "in ~/a/longer/project"), Some(49));
        assert_eq!(col(&buf, ITEM_YS[2], "choose a folder"), Some(49));

        // One too long for the row is cut at its start, and the block then
        // fills the row from edge to edge.
        let long = format!("~/{}last", "deep/".repeat(20));
        let buf = draw_with(&Splash::default(), &theme, &long, 100, 26)?;
        assert_eq!(col(&buf, ITEM_YS[0], &pill(Item::NewFile)), Some(ROW_X));
        let shown = row(&buf, ITEM_YS[0]);
        assert!(shown.ends_with("/deep/last"), "{shown}");
        assert_eq!(col(&buf, ITEM_YS[0], "in …"), Some(ROW_X + HINT_X));
        assert_eq!(buf[(ROW_X + ROW_W - 1, ITEM_YS[0])].symbol(), "t");
        assert_eq!(buf[(ROW_X + ROW_W, ITEM_YS[0])].symbol(), " ");
        Ok(())
    }

    #[test]
    fn small_areas_drop_the_wordmark_then_the_hints_then_the_path() -> anyhow::Result<()> {
        let theme = Theme::default();
        let splash = Splash::default();

        // Fourteen rows hold the whole splash, the five-row wordmark with it,
        // and so does the least width the action rows' hints fit in.
        for (w, h) in [(100, 14), (ROW_W, 14)] {
            let shown = rows(&draw(&splash, &theme, w, h)?);
            assert_eq!(&shown[..5], &WORDMARK.map(str::trim), "{shown:?}");
        }

        // Too short for the wordmark: `✦ tinkery`, the path, rows with hints.
        let shown = rows(&draw(&splash, &theme, 100, 13)?);
        assert_eq!(shown[0], "✦ tinkery");
        assert_eq!(shown[1], "~/src");
        assert!(shown[2].contains("in ~/src"), "{shown:?}");
        assert_eq!(shown.len(), 5, "{shown:?}");

        // Too narrow for the hints too.
        let shown = rows(&draw(&splash, &theme, 50, 13)?);
        assert_eq!(shown[0], "✦ tinkery");
        assert_eq!(shown[1], "~/src");
        assert_eq!(shown[2], pill(Item::NewFile));
        assert_eq!(shown.len(), 5, "{shown:?}");
        // Without hints the buttons alone are centred: (50 − 18) / 2.
        let buf = draw(&splash, &theme, 50, 13)?;
        for (y, item) in [(6, Item::NewFile), (8, Item::NewDirectory)] {
            assert_eq!(col(&buf, y, &pill(item)), Some(16), "{item:?}");
        }

        // Too short for the path.
        let shown = rows(&draw(&splash, &theme, 100, 8)?);
        assert_eq!(shown.len(), 4, "{shown:?}");
        assert_eq!(shown[0], "✦ tinkery");
        assert!(shown[1].starts_with(&pill(Item::NewFile)), "{shown:?}");
        assert!(!shown.iter().any(|r| r.contains("~/src")), "{shown:?}");

        // Shorter still: the rows close up.
        let shown = rows(&draw(&splash, &theme, 100, 4)?);
        assert_eq!(shown.len(), 4, "{shown:?}");
        assert!(
            shown[3].starts_with(&pill(Item::OpenDirectory)),
            "{shown:?}"
        );
        Ok(())
    }

    #[test]
    fn a_click_finds_the_row_under_it_at_every_size() -> anyhow::Result<()> {
        let theme = Theme::default();
        for (w, h) in [
            (100, 26),
            (100, 14),
            (100, 13),
            (50, 13),
            (100, 8),
            (100, 4),
            (30, 3),
        ] {
            let area = Rect::new(0, 0, w, h);
            let buf = draw(&Splash::default(), &theme, w, h)?;
            for item in Item::ALL {
                let y = (0..h).find(|&y| col(&buf, y, item.label()).is_some());
                let Some(y) = y else {
                    // Cut off at the bottom: nothing there to click.
                    continue;
                };
                let x = col(&buf, y, item.label()).expect("found above");
                assert_eq!(
                    Splash::item_at(area, Position::new(x, y)),
                    Some(item),
                    "{item:?} at {w}×{h}"
                );
            }
            // The brand row isn't an action.
            let brand =
                (0..h).find(|&y| row(&buf, y).contains("tinkery") || row(&buf, y).contains('▀'));
            if let Some(y) = brand {
                assert_eq!(Splash::item_at(area, Position::new(w / 2, y)), None);
            }
        }
        // The whole row answers, not just the button; beside it, off its
        // span, nothing.
        let area = Rect::new(0, 0, 100, 26);
        for x in [ROW_X, ROW_X + 4, HINT_COL, ROW_X + ROW_W - 1] {
            assert_eq!(
                Splash::item_at(area, Position::new(x, ITEM_YS[1])),
                Some(Item::NewDirectory),
                "column {x}"
            );
        }
        assert_eq!(
            Splash::item_at(area, Position::new(ROW_X + ROW_W, ITEM_YS[1])),
            None
        );
        assert_eq!(
            Splash::item_at(area, Position::new(ROW_X - 1, ITEM_YS[1])),
            None
        );
        assert_eq!(
            Splash::item_at(area, Position::new(ROW_X + 4, ITEM_YS[1] + 1)),
            None
        );
        Ok(())
    }

    #[test]
    fn mono_has_a_plain_wordmark_and_rule_and_a_reversed_bold_button() -> anyhow::Result<()> {
        let mono = Theme::named("mono").expect("mono exists");
        let buf = draw(&Splash::default(), &mono, 100, 26)?;
        for (y, bitmap) in (TOP..).zip(WORDMARK) {
            for (col, c) in (0u16..).zip(bitmap.chars()) {
                let cell = &buf[(WORD_X + col, y)];
                assert_eq!(cell.symbol(), c.to_string());
                assert_eq!(cell.fg, Color::Reset);
            }
        }
        for x in 18..81 {
            assert_eq!(buf[(x, RULE_Y)].fg, Color::Reset);
        }
        // The buttons are text between blanks, in the columns the pills take:
        // the selected one reverse video, all of them bold, no glyph a
        // terminal's own font lacks, and no colour that isn't the terminal's.
        assert_eq!(col(&buf, ITEM_YS[0], " New file n "), Some(BUTTON_X));
        assert_eq!(col(&buf, ITEM_YS[1], " New directory d "), Some(BUTTON_X));
        for (y, width, selected) in [(ITEM_YS[0], 12, true), (ITEM_YS[1], 17, false)] {
            for x in BUTTON_X..BUTTON_X + width {
                let cell = &buf[(x, y)];
                assert!(cell.modifier.contains(Modifier::BOLD), "({x}, {y})");
                assert_eq!(
                    cell.modifier.contains(Modifier::REVERSED),
                    selected,
                    "({x}, {y})"
                );
            }
            // The hint isn't part of the button.
            assert!(buf[(HINT_COL, y)].modifier.is_empty(), "row {y}");
        }
        for y in 0..26 {
            for x in 0..100 {
                let cell = &buf[(x, y)];
                assert!(![kit::CAP_L, kit::CAP_R].contains(&cell.symbol()));
                assert!(!matches!(cell.fg, Color::Rgb(..)), "({x}, {y})");
                assert!(!matches!(cell.bg, Color::Rgb(..)), "({x}, {y})");
            }
        }
        Ok(())
    }
}
