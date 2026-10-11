//! The card `>keybindings` opens: every command with its keys and its name in
//! `[keys]`, searchable, in the catalog's style (tinkery-catalog spec C2), its
//! keys drawn as keycaps (tinkery-forge spec F19).
//! Commands that only work in one place (the tree, the splash…) come after the
//! rest, under a heading naming that place.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::buffer::movement::Motion;
use crate::keymap::{Action, Input, Keymap, Scope};
use crate::theme::Theme;
use crate::ui::kit::{self, Look, RowInk};
use crate::ui::prompt::PromptBar;

/// The catalog's width and the row it drops from, so both cards read alike.
const WIDTH: u16 = 86;
const TOP: u16 = 4;
/// The rows other than the list: the edge, a blank, the header's strip and a
/// blank above it; a blank, the keys and the edge below.
const CHROME: u16 = 7;
// Delete, not Backspace, resets: Backspace keeps correcting the query.
const HINTS: [(&str, &str); 3] = [("⏎", "rebind"), ("Del", "reset"), ("Esc", "close")];
/// What the selected row shows in place of its keys while Enter waits for one.
pub const WAITING: &str = "press a key… (esc cancels)";
/// What a command with no key shows in place of its keys.
const NO_KEY: &str = "—";
/// Cells for the title, so the config names line up: the longest title and two
/// blanks.
const TITLE_WIDTH: u16 = 35;
/// The fewest cells a title keeps when the keys are wide (all of a shorter
/// title), so a row still says what it's for on a narrow screen.
const TITLE_MIN: u16 = 12;

/// The places with keys of their own, in the order the card lists them, and
/// the heading each is listed under.
const SCOPES: [(Scope, &str); 7] = [
    (Scope::Tree, "file tree"),
    (Scope::Find, "find bar"),
    (Scope::Search, "project search"),
    (Scope::Folders, "folder browser"),
    (Scope::Catalog, "language servers"),
    (Scope::Splash, "splash"),
    (Scope::Debug, "debug panel"),
];

/// One command's row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub action: Action,
    /// What the cast palette calls it.
    pub title: &'static str,
    /// Its keys as the palette labels them, the palette's pick first; empty
    /// when it has none.
    pub keys: Vec<String>,
    /// Its name on the left of `[keys]`.
    pub name: &'static str,
    pub scope: Scope,
}

impl Row {
    /// Whether every word of `query` is in the title, a key or the config
    /// name, ignoring case.
    fn matches(&self, query: &str) -> bool {
        let hay = format!("{} {} {}", self.title, self.keys.join(" "), self.name).to_lowercase();
        query
            .to_lowercase()
            .split_whitespace()
            .all(|word| hay.contains(word))
    }
}

/// One line of the card's list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Line {
    /// The row at this index into `shown`.
    Row(usize),
    /// A scope's heading, as the catalog labels its debuggers.
    Heading(&'static str),
    Blank,
}

/// What a key did to the card, when it did more than move or filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Esc: close it.
    Close,
    /// Delete on a row: take its `[keys]` entry away, so its defaults are back.
    Reset(Action),
}

#[derive(Debug, Clone)]
pub struct Keybindings {
    /// Every command: those that work anywhere, then each scope's.
    rows: Vec<Row>,
    /// What's typed, filtering the rows.
    query: PromptBar,
    /// The indices into `rows` the query matches, in order.
    shown: Vec<usize>,
    /// Index into `shown`.
    selected: usize,
    /// The first line of the list in view. It moves only when the selection
    /// would leave the view, so ↑ moves the selection rather than the list.
    top: usize,
    /// The command Enter is rebinding, while the card waits for its new key.
    /// The app hands that key over whole, as only the keymap can name it.
    waiting: Option<Action>,
}

impl Keybindings {
    /// The card over every action, with its keys in `keymap`.
    pub fn new(keymap: &Keymap) -> Self {
        let row = |action: Action| Row {
            action,
            title: action.title(),
            keys: keymap.key_labels(action),
            name: action.name(),
            scope: action.scope(),
        };
        let mut rows: Vec<Row> = Action::all()
            .filter(|a| a.scope() == Scope::Global)
            .map(row)
            .collect();
        for (scope, _) in SCOPES {
            rows.extend(Action::all().filter(|a| a.scope() == scope).map(row));
        }
        let shown = (0..rows.len()).collect();
        Keybindings {
            rows,
            query: PromptBar::new("Keybindings", ""),
            shown,
            selected: 0,
            top: 0,
            waiting: None,
        }
    }

    /// The command waiting for a new key, if Enter started a rebind.
    pub fn waiting(&self) -> Option<Action> {
        self.waiting
    }

    /// Ends the wait for a key: it came, or Esc cancelled it.
    pub fn stop_waiting(&mut self) {
        self.waiting = None;
    }

    /// Shows the keys `keymap` gives each command, after a rebind or reset.
    /// The query is matched again, as a key it named may have moved; the
    /// selection stays on its command while that is still listed.
    pub fn refresh(&mut self, keymap: &Keymap) {
        for row in &mut self.rows {
            row.keys = keymap.key_labels(row.action);
        }
        let selected = self.selected_action();
        let query = self.query.text();
        self.shown = (0..self.rows.len())
            .filter(|&i| self.rows[i].matches(query))
            .collect();
        self.selected = selected
            .and_then(|action| {
                self.shown
                    .iter()
                    .position(|&i| self.rows[i].action == action)
            })
            .unwrap_or(0);
        self.top = 0;
    }

    #[cfg(test)]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// The command the selected row is for, if any row is shown: what Enter
    /// rebinds and Delete resets.
    pub fn selected_action(&self) -> Option<Action> {
        let &row = self.shown.get(self.selected)?;
        Some(self.rows[row].action)
    }

    /// ↑ and ↓ move the selection, stopping at either end, and scroll the
    /// list as the card drawn in `area` needs; Esc closes; Enter waits for a
    /// new key for the selected row; Delete resets it; anything else edits the
    /// query.
    pub fn handle(&mut self, input: Input, area: Rect) -> Option<Step> {
        match input {
            Input::Action(Action::Move(Motion::Up)) => {
                self.selected = self.selected.saturating_sub(1);
                self.top = self.first_line(self.room(area));
            }
            Input::Action(Action::Move(Motion::Down)) => {
                self.selected = (self.selected + 1).min(self.shown.len().saturating_sub(1));
                self.top = self.first_line(self.room(area));
            }
            Input::Action(Action::Cancel) => return Some(Step::Close),
            Input::Action(Action::Newline) => self.waiting = self.selected_action(),
            Input::Action(Action::Delete) => return self.selected_action().map(Step::Reset),
            _ => {
                let before = self.query.text().to_string();
                self.query.handle(input);
                if self.query.text() != before {
                    self.refilter();
                }
            }
        }
        None
    }

    /// Pasted text goes into the query as if typed; a line break does nothing.
    pub fn paste(&mut self, text: &str) {
        let line: String = text.chars().filter(|c| !matches!(c, '\r' | '\n')).collect();
        let before = self.query.text().to_string();
        self.query.paste(&line);
        if self.query.text() != before {
            self.refilter();
        }
    }

    fn refilter(&mut self) {
        let query = self.query.text();
        self.shown = (0..self.rows.len())
            .filter(|&i| self.rows[i].matches(query))
            .collect();
        self.selected = 0;
        self.top = 0;
    }

    /// How many lines of the list the card drawn in `area` shows.
    fn room(&self, area: Rect) -> usize {
        usize::from(self.card(area).height.saturating_sub(CHROME))
    }

    /// The first line to draw in a list `room` lines tall: `top`, moved just
    /// far enough to bring the selected row into view, along with the heading
    /// and blank above it when scrolling up to it.
    fn first_line(&self, room: usize) -> usize {
        let lines = self.lines();
        let at = lines
            .iter()
            .position(|&line| line == Line::Row(self.selected))
            .unwrap_or(0);
        let lead = lines[..at]
            .iter()
            .rposition(|line| matches!(line, Line::Row(_)))
            .map_or(0, |row| row + 1);
        let top = if at < self.top { lead } else { self.top };
        top.max((at + 1).saturating_sub(room.max(1)))
    }

    /// The list as drawn: the rows shown, each scope's after a blank and its
    /// heading.
    fn lines(&self) -> Vec<Line> {
        let mut lines = Vec::new();
        let mut scope = Scope::Global;
        for (at, &row) in self.shown.iter().enumerate() {
            let row_scope = self.rows[row].scope;
            if row_scope != scope {
                scope = row_scope;
                if let Some(&(_, heading)) = SCOPES.iter().find(|(s, _)| *s == scope) {
                    if !lines.is_empty() {
                        lines.push(Line::Blank);
                    }
                    lines.push(Line::Heading(heading));
                }
            }
            lines.push(Line::Row(at));
        }
        lines
    }

    /// The selected row, as a number that changes when the selection moves.
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// Where the card goes in `area`: the catalog's width, centred, from row
    /// 4, as tall as every row needs or down to a row short of the bottom. Its
    /// height doesn't follow the query, so the card holds still while typing.
    pub fn card(&self, area: Rect) -> Rect {
        let width = WIDTH.min(area.width.saturating_sub(4));
        let y = area.y + TOP.min(area.height);
        // Every row, a heading for each scope and a blank before it.
        let all = self.rows.len() + 2 * SCOPES.len();
        let height = CHROME
            .saturating_add(u16::try_from(all).unwrap_or(u16::MAX))
            .min(area.bottom().saturating_sub(y + 1));
        Rect {
            x: area.x + (area.width - width) / 2,
            y,
            width,
            height,
        }
    }

    /// Dims `area` and draws the panel over it: `Keybindings` in its top edge;
    /// the query on the header's strip; the list, scrolled just far enough to
    /// keep the selected row, a row pill, in view; a blank and the keys.
    /// Returns where the cursor goes, in the query. `hovered` is the screen
    /// row the pointer is on, inside the card (`kit::row_under`); the row
    /// drawn there is lit.
    pub fn render(
        &self,
        look: Look,
        theme: &Theme,
        frame: &mut Frame,
        area: Rect,
        hovered: Option<u16>,
    ) -> (u16, u16) {
        let out = frame.buffer_mut();
        kit::dim(theme, out, area);
        let card = self.card(area);
        let inside = kit::panel_at(look, theme, out, card, self.query.label);
        if inside.is_empty() || card.height <= CHROME || card.width < 40 {
            return (card.x, card.y);
        }
        let left = card.x + 4;
        let right = card.right() - 4;

        let field = kit::header(look, theme, out, card, Vec::new());
        let query = Style::new().fg(theme.strong).bg(theme.raised2);
        let cursor = self.query.render_value(out, field, query);

        let quiet = Style::new().fg(theme.muted).bg(theme.raised);
        let room = usize::from(card.height - CHROME);
        let top = card.y + 4;
        let lines = self.lines();
        if lines.is_empty() {
            out.set_string(left, top, "no matching commands", quiet);
        }
        // `top` was set for this screen; clamping again keeps the selection
        // in view after a resize.
        let first = self.first_line(room);
        for (offset, line) in lines.iter().skip(first).take(room).enumerate() {
            let y = top + u16::try_from(offset).unwrap_or(u16::MAX);
            match *line {
                Line::Row(i) => {
                    let row = &self.rows[self.shown[i]];
                    let state = if i != self.selected {
                        Lit::No
                    } else if self.waiting.is_some() {
                        Lit::Waiting
                    } else {
                        Lit::Selected
                    };
                    let lit = hovered == Some(y);
                    render_row(look, theme, out, card, y, row, (state, lit));
                }
                Line::Heading(heading) => {
                    out.set_string(left, y, heading, quiet.add_modifier(Modifier::BOLD));
                }
                Line::Blank => {}
            }
        }

        let hints = kit::hints(look, theme, &HINTS, theme.raised);
        kit::put(out, left, card.bottom() - 2, &hints, right);
        cursor
    }
}

/// Whether a row is the selected one, and whether it then waits for a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lit {
    No,
    Selected,
    Waiting,
}

/// The title, the config name in `muted`, and the keys at the right as
/// keycaps, on the row `sel_row` draws: a pill when selected, its title in
/// `strong` bold; the pill alone when `lit`, under the pointer. While it
/// waits for a new key, `WAITING` stands in for its keys, in `accent`.
fn render_row(
    look: Look,
    theme: &Theme,
    out: &mut Buffer,
    card: Rect,
    y: u16,
    row: &Row,
    (state, lit): (Lit, bool),
) {
    let left = card.x + 4;
    let right = card.right() - 4;
    let whole = Rect::new(card.x + 1, y, card.width.saturating_sub(2), 1);
    let selected = state != Lit::No;
    let ink = kit::sel_row(look, theme, out, whole, selected, lit, theme.raised);

    // The keys get what the title's minimum and two blanks leave, cut with
    // `…` beyond that.
    let title_min = width_of(row.title).min(TITLE_MIN);
    let keys_room = right.saturating_sub(left + title_min + 2);
    let keys = if state == Lit::Waiting {
        let accent = ink.of(Style::new().fg(theme.accent));
        vec![Span::styled(cut(WAITING, keys_room), accent)]
    } else if row.keys.is_empty() {
        let plain = ink.of(Style::new().fg(theme.text));
        vec![Span::styled(cut(NO_KEY, keys_room), plain)]
    } else {
        caps(look, theme, ink, &row.keys, keys_room)
    };
    let keys_x = right.saturating_sub(kit::width(&keys));
    kit::put(out, keys_x, y, &keys, right);

    out.set_stringn(
        left,
        y,
        row.title,
        usize::from(keys_x.saturating_sub(left + 2)),
        ink.label,
    );
    // Two blanks short of the keys, so a long name never runs into them.
    let name_x = left + TITLE_WIDTH;
    if name_x + 2 < keys_x {
        out.set_stringn(
            name_x,
            y,
            row.name,
            usize::from(keys_x - name_x - 2),
            ink.faint,
        );
    }
}

/// `keys` as keycaps on the row `ink` is for, a blank apart: as many whole
/// ones as fit in `room` cells, then ` …` when some are left out. The caps sit
/// on the row's own fill; where the row carries no colours (the selected row
/// in `mono`, which is reverse video) they carry none either, and are the
/// keys in bold.
fn caps(look: Look, theme: &Theme, ink: RowInk, keys: &[String], room: u16) -> Vec<Span<'static>> {
    const MORE: &str = "…";
    // A cap is its key and two ends; a blank parts it from the one before,
    // and one more with `MORE` follows unless it's the last.
    let mut used: u16 = 0;
    let mut fit = 0;
    for (i, key) in keys.iter().enumerate() {
        let gap = u16::from(i > 0);
        let more = if i + 1 < keys.len() { 2 } else { 0 };
        let cells = gap + width_of(key) + 2;
        if used + cells + more > room {
            break;
        }
        used += cells;
        fit = i + 1;
    }
    let ground = ink.label.bg;
    let shown = keys[..fit].join(" ");
    let mut out = kit::keycaps(look, theme, &shown, ground.unwrap_or(theme.raised));
    if ground.is_none() {
        for span in &mut out {
            span.style = ink.of(span.style);
        }
    }
    if fit < keys.len() && used < room {
        let more = if fit == 0 {
            MORE.to_string()
        } else {
            format!(" {MORE}")
        };
        out.push(Span::styled(more, ink.faint));
    }
    out
}

/// How many cells `text` takes.
fn width_of(text: &str) -> u16 {
    u16::try_from(text.width()).unwrap_or(u16::MAX)
}

/// `text` if it fits in `room` cells, else as much as fits followed by `…`.
fn cut(text: &str, room: u16) -> String {
    if width_of(text) <= room {
        return text.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let w = u16::try_from(c.width().unwrap_or(0)).unwrap_or(u16::MAX);
        if used + w + 1 > room {
            break;
        }
        out.push(c);
        used += w;
    }
    if room > 0 {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use crate::config::{KeyBinding, KeysConfig};
    use crate::ui::kit::{CAP_L, CAP_R};

    fn fresh() -> Keybindings {
        Keybindings::new(&Keymap::default())
    }

    fn row(k: &Keybindings, action: Action) -> &Row {
        k.rows()
            .iter()
            .find(|r| r.action == action)
            .unwrap_or_else(|| panic!("no row for {action:?}"))
    }

    /// The screen the unit tests draw on.
    const SCREEN: Rect = Rect::new(0, 0, 100, 30);

    fn type_text(k: &mut Keybindings, text: &str) {
        for c in text.chars() {
            k.handle(Input::Text(c), SCREEN);
        }
    }

    fn shown(k: &Keybindings) -> Vec<&'static str> {
        k.shown.iter().map(|&i| k.rows[i].name).collect()
    }

    #[test]
    fn every_action_has_a_row_with_title_keys_and_config_name() {
        let k = fresh();
        assert_eq!(k.rows().len(), Action::all().count());
        let save = row(&k, Action::Save);
        assert_eq!((save.title, save.name), ("Save", "save"));
        assert_eq!(save.keys, ["Ctrl+S"]);
        let keys = row(&k, Action::Keybindings);
        assert!(keys.keys.is_empty());
        assert_eq!(keys.name, "keybindings");
        let tree = row(&k, Action::TreeNewFolder);
        assert_eq!(tree.keys, ["Shift+A"]);
        assert_eq!(tree.scope, Scope::Tree);
    }

    #[test]
    fn a_rebound_command_shows_every_key_it_has() -> anyhow::Result<()> {
        let keys: KeysConfig = [(
            "save".to_string(),
            KeyBinding::Many(vec!["ctrl+s".into(), "alt+w".into()]),
        )]
        .into_iter()
        .collect();
        let k = Keybindings::new(&Keymap::new(&keys)?);
        assert_eq!(row(&k, Action::Save).keys, ["Alt+W", "Ctrl+S"]);
        Ok(())
    }

    #[test]
    fn global_commands_come_first_then_each_scope_under_its_heading() {
        let k = fresh();
        let first_scoped = k
            .rows()
            .iter()
            .position(|r| r.scope != Scope::Global)
            .unwrap_or(0);
        assert!(
            k.rows()[first_scoped..]
                .iter()
                .all(|r| r.scope != Scope::Global)
        );
        assert_eq!(k.rows()[0].action, Action::Quit);
        let lines = k.lines();
        let headings: Vec<&str> = lines
            .iter()
            .filter_map(|l| match l {
                Line::Heading(h) => Some(*h),
                _ => None,
            })
            .collect();
        assert_eq!(
            headings,
            [
                "file tree",
                "find bar",
                "project search",
                "folder browser",
                "language servers",
                "splash",
                "debug panel"
            ]
        );
        // A blank before each heading, and each scope's rows under it.
        let tree = lines
            .iter()
            .position(|l| *l == Line::Heading("file tree"))
            .unwrap_or(0);
        assert_eq!(lines[tree - 1], Line::Blank);
        let Line::Row(i) = lines[tree + 1] else {
            panic!("no row under the tree heading");
        };
        assert_eq!(k.rows[k.shown[i]].action, Action::TreeNewFile);
    }

    #[test]
    fn typing_filters_on_title_key_or_config_name() {
        let mut k = fresh();
        type_text(&mut k, "save as");
        assert_eq!(shown(&k), ["save_as"]);
        let mut k = fresh();
        type_text(&mut k, "ctrl+shift+left");
        assert_eq!(shown(&k), ["select_word_left"]);
        let mut k = fresh();
        type_text(&mut k, "SPLASH_Q");
        assert_eq!(shown(&k), ["splash_quit"]);
        // Backspace corrects the query; the selection starts over.
        k.handle(Input::Action(Action::Backspace), SCREEN);
        k.handle(Input::Action(Action::Backspace), SCREEN);
        assert!(shown(&k).len() > 1);
        assert_eq!(k.selected, 0);
        let mut k = fresh();
        type_text(&mut k, "zzz");
        assert!(k.lines().is_empty());
        assert_eq!(k.selected_action(), None);
    }

    #[test]
    fn arrows_stop_at_the_ends_and_esc_closes() {
        let mut k = fresh();
        let press = |k: &mut Keybindings, a: Action| k.handle(Input::Action(a), SCREEN);
        assert_eq!(press(&mut k, Action::Move(Motion::Up)), None);
        assert_eq!(k.selected_action(), Some(Action::Quit));
        press(&mut k, Action::Move(Motion::Down));
        assert_eq!(k.selected_action(), Some(Action::Save));
        for _ in 0..500 {
            press(&mut k, Action::Move(Motion::Down));
        }
        assert_eq!(k.selected_action(), Some(Action::DebugSwitchPane));
        assert_eq!(press(&mut k, Action::Cancel), Some(Step::Close));
    }

    #[test]
    fn enter_waits_for_a_key_and_delete_resets_while_backspace_edits_the_query() {
        let mut k = fresh();
        let press = |k: &mut Keybindings, a: Action| k.handle(Input::Action(a), SCREEN);
        press(&mut k, Action::Move(Motion::Down));
        assert_eq!(press(&mut k, Action::Newline), None);
        assert_eq!(k.waiting(), Some(Action::Save));
        k.stop_waiting();
        assert_eq!(k.waiting(), None);
        assert_eq!(
            press(&mut k, Action::Delete),
            Some(Step::Reset(Action::Save))
        );
        // Backspace with nothing typed does nothing; with a query it erases.
        assert_eq!(press(&mut k, Action::Backspace), None);
        assert_eq!(k.selected_action(), Some(Action::Save));
        type_text(&mut k, "zzz");
        press(&mut k, Action::Backspace);
        assert_eq!(k.query.text(), "zz");
        // With no row shown, neither has anything to act on.
        assert_eq!(press(&mut k, Action::Delete), None);
        press(&mut k, Action::Newline);
        assert_eq!(k.waiting(), None);
    }

    #[test]
    fn refresh_matches_the_query_again() -> anyhow::Result<()> {
        let mut k = fresh();
        type_text(&mut k, "alt+k");
        assert_eq!(shown(&k), ["hover"]);
        // Hover loses Alt+K to Save: Save is listed and Hover no longer is.
        let keys: KeysConfig = [
            ("hover".to_string(), KeyBinding::Many(Vec::new())),
            ("save".to_string(), KeyBinding::One("alt+k".into())),
        ]
        .into_iter()
        .collect();
        k.refresh(&Keymap::new(&keys)?);
        assert_eq!(shown(&k), ["save"]);
        assert_eq!(k.selected_action(), Some(Action::Save));
        // A selection still listed stays put.
        let mut k = fresh();
        type_text(&mut k, "save");
        k.handle(Input::Action(Action::Move(Motion::Down)), SCREEN);
        let before = k.selected_action();
        k.refresh(&Keymap::default());
        assert_eq!(k.selected_action(), before);
        Ok(())
    }

    #[test]
    fn the_waiting_row_asks_for_a_key_and_refresh_shows_new_keys() -> anyhow::Result<()> {
        let mut k = fresh();
        k.handle(Input::Action(Action::Move(Motion::Down)), SCREEN);
        k.handle(Input::Action(Action::Newline), SCREEN);
        let theme = Theme::default();
        let card = k.card(SCREEN);
        let buffer = draw(&k, &theme)?;
        let save = text_row(&buffer, card, card.y + 5);
        assert!(save.starts_with("Save "), "{save:?}");
        assert!(save.ends_with(&format!(" {WAITING}")), "{save:?}");
        // In the accent on the selected row's pill, and not a keycap.
        let at = card.right() - 4 - width_of(WAITING);
        let asked = &buffer[(at, card.y + 5)];
        assert_eq!((asked.fg, asked.bg), (theme.accent, theme.hov));
        assert!(!save.contains(CAP_L), "{save:?}");

        k.stop_waiting();
        let keys: KeysConfig = [("save".to_string(), KeyBinding::One("alt+w".into()))]
            .into_iter()
            .collect();
        k.refresh(&Keymap::new(&keys)?);
        assert_eq!(row(&k, Action::Save).keys, ["Alt+W"]);
        assert_eq!(k.selected_action(), Some(Action::Save));
        let save = text_row(&draw(&k, &theme)?, card, card.y + 5);
        assert!(save.ends_with(&format!(" {CAP_L}Alt+W{CAP_R}")), "{save:?}");
        Ok(())
    }

    #[test]
    fn a_paste_goes_into_the_query_without_its_line_break() {
        let mut k = fresh();
        k.paste("toggle tree\n");
        assert_eq!(k.query.text(), "toggle tree");
        assert_eq!(shown(&k), ["toggle_tree"]);
    }

    fn draw(k: &Keybindings, theme: &Theme) -> anyhow::Result<Buffer> {
        let mut terminal = Terminal::new(TestBackend::new(100, 30))?;
        terminal.draw(|frame| {
            k.render(Look::default(), theme, frame, frame.area(), None);
        })?;
        Ok(terminal.backend().buffer().clone())
    }

    /// Row `y` of the card, edge to edge.
    fn card_row(buffer: &Buffer, card: Rect, y: u16) -> String {
        (card.x..card.right())
            .map(|x| buffer[(x, y)].symbol())
            .collect()
    }

    /// The cells of row `y` a row's text takes: four in from either edge.
    fn text_row(buffer: &Buffer, card: Rect, y: u16) -> String {
        (card.x + 4..card.right() - 4)
            .map(|x| buffer[(x, y)].symbol())
            .collect()
    }

    fn text(spans: &[Span]) -> String {
        spans.iter().map(|span| span.content.as_ref()).collect()
    }

    #[test]
    fn the_card_is_a_panel_with_the_query_on_a_strip_pill_rows_and_keycaps() -> anyhow::Result<()> {
        let k = fresh();
        let theme = Theme::default();
        let card = k.card(Rect::new(0, 0, 100, 30));
        assert_eq!(card, Rect::new(7, 4, 86, 25));
        let buffer = draw(&k, &theme)?;
        // The panel's edge in the accent, its title set into the top of it.
        let top = card_row(&buffer, card, card.y);
        assert!(top.starts_with(" ▁ Keybindings ▁"), "{top:?}");
        assert_eq!(buffer[(card.x + 1, card.y)].fg, theme.accent);
        let bottom = card_row(&buffer, card, card.bottom() - 1);
        assert_eq!(bottom, format!(" {} ", "▔".repeat(84)));

        // The header: a strip of `raised2`, the query, still empty, after `✦`.
        let header = card_row(&buffer, card, card.y + 2);
        assert_eq!(header, format!("▕{CAP_L}  ✦ {:<76}  {CAP_R}▏", ""));
        let mark = &buffer[(card.x + 4, card.y + 2)];
        assert_eq!((mark.fg, mark.bg), (theme.accent2, theme.raised2));
        assert_eq!(text_row(&buffer, card, card.y + 3).trim(), "");

        // The first row is selected: a pill of `hov` with `›` in the accent,
        // its title in `strong` bold, its key a keycap on the pill.
        let quit = card_row(&buffer, card, card.y + 4);
        let words = format!("{:<35}{:<35}", "Quit", "quit");
        assert_eq!(
            quit,
            format!("▕{CAP_L}› {words}{CAP_L}Ctrl+Q{CAP_R}  {CAP_R}▏")
        );
        let mark = &buffer[(card.x + 2, card.y + 4)];
        assert_eq!((mark.fg, mark.bg), (theme.accent, theme.hov));
        let title = &buffer[(card.x + 4, card.y + 4)];
        assert_eq!((title.fg, title.bg), (theme.strong, theme.hov));
        assert!(title.modifier.contains(Modifier::BOLD));
        // `muted` wouldn't read on the pill: the name is in `text` there.
        let name = &buffer[(card.x + 39, card.y + 4)];
        assert_eq!((name.fg, name.bg), (theme.text, theme.hov));
        let right = card.right() - 4;
        for x in [right - 8, right - 1] {
            let end = &buffer[(x, card.y + 4)];
            assert_eq!((end.fg, end.bg), (theme.btn, theme.hov), "{x}");
        }
        let key = &buffer[(right - 7, card.y + 4)];
        assert_eq!((key.fg, key.bg), (theme.strong, theme.btn));
        assert!(key.modifier.contains(Modifier::BOLD));

        // Any other row is left on the card, its keycap on `raised`.
        let save = card_row(&buffer, card, card.y + 5);
        let words = format!("{:<35}{:<35}", "Save", "save");
        assert_eq!(save, format!("▕   {words}{CAP_L}Ctrl+S{CAP_R}   ▏"));
        for x in [card.x + 1, card.x + 2, card.x + 4] {
            assert_eq!(buffer[(x, card.y + 5)].bg, theme.raised, "{x}");
        }
        assert_eq!(buffer[(card.x + 4, card.y + 5)].fg, theme.fg);
        assert_eq!(buffer[(card.x + 39, card.y + 5)].fg, theme.muted);
        let end = &buffer[(right - 8, card.y + 5)];
        assert_eq!((end.fg, end.bg), (theme.btn, theme.raised));
        let key = &buffer[(right - 7, card.y + 5)];
        assert_eq!((key.fg, key.bg), (theme.strong, theme.btn));

        // A blank, then the keys as keycaps on the card.
        assert_eq!(text_row(&buffer, card, card.bottom() - 3).trim(), "");
        let foot = text_row(&buffer, card, card.bottom() - 2);
        let hints = text(&kit::hints(Look::default(), &theme, &HINTS, theme.raised));
        assert_eq!(foot.trim_end(), hints, "{foot:?}");
        assert_eq!(
            hints,
            format!("{CAP_L}⏎{CAP_R} rebind   {CAP_L}Del{CAP_R} reset   {CAP_L}Esc{CAP_R} close")
        );
        let key = &buffer[(card.x + 5, card.bottom() - 2)];
        assert_eq!((key.fg, key.bg), (theme.strong, theme.btn));
        Ok(())
    }

    #[test]
    fn every_key_of_a_command_is_its_own_keycap() -> anyhow::Result<()> {
        let keys: KeysConfig = [(
            "save".to_string(),
            KeyBinding::Many(vec!["ctrl+s".into(), "alt+w".into()]),
        )]
        .into_iter()
        .collect();
        let mut k = Keybindings::new(&Keymap::new(&keys)?);
        type_text(&mut k, "save");
        let theme = Theme::default();
        let card = k.card(SCREEN);
        let buffer = draw(&k, &theme)?;
        let save = text_row(&buffer, card, card.y + 4);
        assert!(
            save.ends_with(&format!(" {CAP_L}Alt+W{CAP_R} {CAP_L}Ctrl+S{CAP_R}")),
            "{save:?}"
        );
        // The blank between two caps is the row's fill, not the caps'.
        let gap = &buffer[(card.right() - 4 - 9, card.y + 4)];
        assert_eq!((gap.symbol(), gap.bg), (" ", theme.hov));
        // The query is what the header's strip holds.
        let header = text_row(&buffer, card, card.y + 2);
        assert_eq!(header.trim_end(), "✦ save");
        let typed = &buffer[(card.x + 6, card.y + 2)];
        assert_eq!((typed.fg, typed.bg), (theme.strong, theme.raised2));
        Ok(())
    }

    #[test]
    fn a_scope_heading_is_muted_and_bold_and_a_keyless_row_shows_a_dash() -> anyhow::Result<()> {
        let mut k = fresh();
        type_text(&mut k, "tree");
        let theme = Theme::default();
        let card = k.card(Rect::new(0, 0, 100, 30));
        let buffer = draw(&k, &theme)?;
        // Toggle and focus first, then the tree's own under its heading.
        assert!(text_row(&buffer, card, card.y + 4).starts_with("Toggle file tree"));
        assert!(text_row(&buffer, card, card.y + 5).starts_with("Focus file tree"));
        assert_eq!(text_row(&buffer, card, card.y + 6).trim(), "");
        let heading = text_row(&buffer, card, card.y + 7);
        assert_eq!(heading.trim_end(), "file tree", "{heading:?}");
        let cell = &buffer[(card.x + 4, card.y + 7)];
        assert_eq!((cell.fg, cell.bg), (theme.muted, theme.raised));
        assert!(cell.modifier.contains(Modifier::BOLD));
        assert!(text_row(&buffer, card, card.y + 8).starts_with("New file in tree"));

        let mut k = fresh();
        type_text(&mut k, "keybindings");
        let buffer = draw(&k, &theme)?;
        let row = text_row(&buffer, card, card.y + 4);
        assert_eq!(row, format!("{:<35}{:<42}—", "Keybindings", "keybindings"));
        // Not a keycap: in `text`, on the selected row's pill.
        let dash = &buffer[(card.right() - 5, card.y + 4)];
        assert_eq!((dash.fg, dash.bg), (theme.text, theme.hov));

        let mut k = fresh();
        type_text(&mut k, "zzz");
        let buffer = draw(&k, &theme)?;
        let none = text_row(&buffer, card, card.y + 4);
        assert_eq!(none.trim_end(), "no matching commands");
        let cell = &buffer[(card.x + 4, card.y + 4)];
        assert_eq!((cell.fg, cell.bg), (theme.muted, theme.raised));
        Ok(())
    }

    #[test]
    fn the_list_scrolls_to_keep_the_selected_row_in_view() -> anyhow::Result<()> {
        let mut k = fresh();
        for _ in 0..500 {
            k.handle(Input::Action(Action::Move(Motion::Down)), SCREEN);
        }
        let theme = Theme::default();
        let card = k.card(Rect::new(0, 0, 100, 30));
        let buffer = draw(&k, &theme)?;
        // The last row of the list: a blank and the keys are under it.
        let last = text_row(&buffer, card, card.bottom() - 4);
        assert!(
            last.starts_with("Debug panel: stack or variables"),
            "{last:?}"
        );
        assert_eq!(buffer[(card.x + 2, card.bottom() - 4)].bg, theme.hov);
        assert_eq!(text_row(&buffer, card, card.bottom() - 3).trim(), "");
        Ok(())
    }

    #[test]
    fn up_moves_the_selection_within_the_view_without_scrolling() -> anyhow::Result<()> {
        let mut k = fresh();
        let theme = Theme::default();
        let card = k.card(SCREEN);
        for _ in 0..25 {
            k.handle(Input::Action(Action::Move(Motion::Down)), SCREEN);
        }
        let scrolled = card_row(&draw(&k, &theme)?, card, card.y + 4);
        let bottom = card.bottom() - 4;
        let lit = theme.hov;
        assert_eq!(draw(&k, &theme)?[(card.x + 2, bottom)].bg, lit);
        for _ in 0..3 {
            k.handle(Input::Action(Action::Move(Motion::Up)), SCREEN);
        }
        let buffer = draw(&k, &theme)?;
        assert_eq!(card_row(&buffer, card, card.y + 4), scrolled);
        assert_eq!(buffer[(card.x + 2, bottom - 3)].bg, lit);
        assert_eq!(buffer[(card.x + 2, bottom)].bg, theme.raised);
        // Going on up past the top scrolls back a line at a time.
        for _ in 0..22 {
            k.handle(Input::Action(Action::Move(Motion::Up)), SCREEN);
        }
        let buffer = draw(&k, &theme)?;
        assert!(text_row(&buffer, card, card.y + 4).starts_with("Quit"));
        Ok(())
    }

    #[test]
    fn keys_that_dont_fit_are_left_out_whole_and_the_title_keeps_its_room() -> anyhow::Result<()> {
        let keys: KeysConfig = [(
            "doc_start".to_string(),
            KeyBinding::Many(vec![
                "ctrl+shift+alt+pageup".into(),
                "ctrl+alt+shift+home".into(),
                "ctrl+alt+f12".into(),
            ]),
        )]
        .into_iter()
        .collect();
        let mut k = Keybindings::new(&Keymap::new(&keys)?);
        type_text(&mut k, "doc_start");
        let screen = Rect::new(0, 0, 45, 30);
        let card = k.card(screen);
        assert_eq!(card.width, 41);
        let mut terminal = Terminal::new(TestBackend::new(45, 30))?;
        terminal.draw(|frame| {
            k.render(
                Look::default(),
                &Theme::default(),
                frame,
                frame.area(),
                None,
            );
        })?;
        let row = text_row(terminal.backend().buffer(), card, card.y + 4);
        // The title up to two blanks short of the keys; one whole keycap, and
        // `…` for the two there's no room for.
        assert_eq!(
            row,
            format!("Go to start of   {CAP_L}Ctrl+Alt+F12{CAP_R} …")
        );
        assert_eq!(cut("abcdef", 4), "abc…");
        assert_eq!(cut("abc", 4), "abc");
        Ok(())
    }

    #[test]
    fn caps_fit_whole_keys_into_their_room() {
        let theme = Theme::default();
        let look = Look {
            pill_caps: false,
            ..Look::default()
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 1));
        let area = buf.area;
        let ink = kit::sel_row(look, &theme, &mut buf, area, false, false, theme.raised);
        let keys = ["F5".to_string(), "Alt+W".to_string(), "Ctrl+S".to_string()];
        let shown = |room: u16| text(&caps(look, &theme, ink, &keys, room));
        // ` F5 `, a blank, ` Alt+W `, a blank, ` Ctrl+S `: 21 cells.
        assert_eq!(shown(21), " F5   Alt+W   Ctrl+S ");
        assert_eq!(shown(20), " F5   Alt+W  …");
        assert_eq!(shown(14), " F5   Alt+W  …");
        // The ` …` needs its two cells too.
        assert_eq!(shown(13), " F5  …");
        assert_eq!(shown(6), " F5  …");
        assert_eq!(shown(5), "…");
        assert_eq!(shown(0), "");
        for room in 0..30 {
            assert!(kit::width(&caps(look, &theme, ink, &keys, room)) <= room);
        }
    }

    #[test]
    fn mono_reverses_the_selected_row_and_its_keycaps_are_bold_without_a_fill() -> anyhow::Result<()>
    {
        let mut k = fresh();
        let mono = Theme::named("mono").expect("mono exists");
        let card = k.card(Rect::new(0, 0, 100, 30));
        let buffer = draw(&k, &mono)?;
        // The card's inside from edge to edge, with the mark and no caps.
        let quit = card_row(&buffer, card, card.y + 4);
        let words = format!("{:<35}{:<35}", "Quit", "quit");
        assert_eq!(quit, format!("▕ › {words} Ctrl+Q    ▏"));
        let lit = &buffer[(card.x + 1, card.y + 4)];
        for x in card.x + 1..card.right() - 1 {
            let cell = &buffer[(x, card.y + 4)];
            assert!(cell.modifier.contains(Modifier::REVERSED), "{x}");
            // A keycap's fill would break the reverse video up.
            assert_eq!((cell.fg, cell.bg), (lit.fg, lit.bg), "{x}");
        }
        let right = card.right() - 4;
        for x in right - 7..right - 1 {
            assert!(buffer[(x, card.y + 4)].modifier.contains(Modifier::BOLD));
        }
        assert!(
            !buffer[(right - 10, card.y + 4)]
                .modifier
                .contains(Modifier::BOLD)
        );
        for x in [card.x, card.right() - 1] {
            let edge = &buffer[(x, card.y + 4)];
            assert!(!edge.modifier.contains(Modifier::REVERSED), "{x}");
        }
        // Off the selected row a keycap is bold on `btn`.
        let key = &buffer[(right - 7, card.y + 5)];
        assert!(key.modifier.contains(Modifier::BOLD));
        assert!(!key.modifier.contains(Modifier::REVERSED));
        assert_eq!((key.fg, key.bg), (mono.strong, mono.btn));

        k.handle(Input::Action(Action::Move(Motion::Down)), SCREEN);
        // Waiting for a key, the row asks for one in plain text.
        k.handle(Input::Action(Action::Newline), SCREEN);
        let buffer = draw(&k, &mono)?;
        let save = text_row(&buffer, card, card.y + 5);
        assert!(save.ends_with(&format!(" {WAITING}")), "{save:?}");
        let asked = &buffer[(right - 3, card.y + 5)];
        assert!(asked.modifier.contains(Modifier::REVERSED));
        assert_eq!((asked.fg, asked.bg), (lit.fg, lit.bg));
        assert!(
            !buffer[(card.x + 4, card.y + 4)]
                .modifier
                .contains(Modifier::REVERSED)
        );
        Ok(())
    }
}
