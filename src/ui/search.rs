//! The project search panel: a panel over the dimmed screen with a Search strip
//! and its case and regex chips, a Replace strip under it, and every line in
//! the project the last search found, as `path:line  text`. Enter searches;
//! once the hits are in, Enter opens one. Tab moves between the fields, and
//! Alt+A replaces the hits in every listed file.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::buffer::movement::Motion;
use crate::keymap::{Action, Input};
use crate::search::{Hit, Query};
use crate::theme::{Theme, mix};
use crate::ui::kit::{self, Look, RowInk};
use crate::ui::prompt::{Outcome, PromptBar};

/// What a key asked the editor to do, when it did more than move or edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Searched {
    /// Search the project for this, replacing the hits listed.
    Start(Query),
    /// Open this hit's file at its line and column.
    Open(Hit),
    /// Alt+A: replace the listed hits' query with `with` in these files, the
    /// listed ones, by the paths the list shows.
    Replace {
        query: Query,
        with: String,
        files: Vec<String>,
    },
    /// Esc: close the panel, stopping any search still running.
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSearch {
    query: PromptBar,
    /// What Alt+A puts in place of each match.
    replace: PromptBar,
    /// Keys go to the Replace field instead of the query; Tab moves between them.
    in_replace: bool,
    case_sensitive: bool,
    regex: bool,
    /// What the listed hits were found for; `None` until a search starts.
    searched: Option<Query>,
    /// Sorted by path, then line.
    hits: Vec<Hit>,
    /// Index into `hits`.
    selected: usize,
    /// The arrows have moved the selection since the search started, so it
    /// follows its hit as more stream in; until then it stays on the first.
    moved: bool,
    /// Which search the hits belong to; batches from an older one are dropped.
    id: u64,
    /// The background search hasn't said it's finished yet.
    running: bool,
    /// Why the last search found nothing, when its regex is invalid.
    error: Option<String>,
}

impl Default for ProjectSearch {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectSearch {
    pub fn new() -> Self {
        ProjectSearch {
            query: PromptBar::new("Search", ""),
            replace: PromptBar::new("Replace", ""),
            in_replace: false,
            case_sensitive: false,
            regex: false,
            searched: None,
            hits: Vec::new(),
            selected: 0,
            moved: false,
            id: 0,
            running: false,
            error: None,
        }
    }

    /// The panel holding `text` in its Search field, as cast's `/text` opens
    /// it.
    pub fn with_query(text: &str) -> Self {
        ProjectSearch {
            query: PromptBar::new("Search", text),
            ..Self::new()
        }
    }

    /// What Enter would search for: the Search field with the case and regex
    /// toggles.
    pub fn query(&self) -> Query {
        Query {
            pattern: self.query.text().to_string(),
            case_sensitive: self.case_sensitive,
            regex: self.regex,
        }
    }

    #[cfg(test)]
    pub fn hits(&self) -> &[Hit] {
        &self.hits
    }

    pub fn selected(&self) -> Option<&Hit> {
        self.hits.get(self.selected)
    }

    /// The editor started search `id` for `query`: the old hits go.
    pub fn start(&mut self, id: u64, query: Query) {
        self.id = id;
        self.searched = Some(query);
        self.hits.clear();
        self.selected = 0;
        self.moved = false;
        self.running = true;
        self.error = None;
    }

    /// The search couldn't start, for the reason given.
    pub fn fail(&mut self, error: String) {
        self.hits.clear();
        self.selected = 0;
        self.running = false;
        self.error = Some(error);
    }

    /// A batch of hits from search `id`. Once the arrows have moved the
    /// selection it stays on its hit, so hits streaming in don't move it out from
    /// under them.
    pub fn add(&mut self, id: u64, hits: Vec<Hit>) {
        if id != self.id || hits.is_empty() {
            return;
        }
        let kept = self
            .selected()
            .filter(|_| self.moved)
            .map(|hit| (hit.path.clone(), hit.line.line));
        self.hits.extend(hits);
        self.hits
            .sort_by(|a, b| a.path.cmp(&b.path).then(a.line.line.cmp(&b.line.line)));
        if let Some((path, line)) = kept {
            self.selected = self
                .hits
                .iter()
                .position(|hit| hit.path == path && hit.line.line == line)
                .unwrap_or(0);
        }
    }

    /// Search `id` has looked at every file.
    pub fn finish(&mut self, id: u64) {
        if id == self.id {
            self.running = false;
        }
    }

    /// Up and Down move through the hits, Alt+C and Alt+R toggle case and regex
    /// (searching again once a search has run), Enter searches or, when the hits
    /// listed are for what's typed, opens the selected one; Tab moves between the
    /// query and the Replace field; Alt+A replaces; Esc closes.
    pub fn handle(&mut self, input: Input) -> Option<Searched> {
        match input {
            Input::Action(Action::Move(Motion::Up)) => {
                self.selected = self.selected.saturating_sub(1);
                self.moved = true;
                None
            }
            Input::Action(Action::Move(Motion::Down)) => {
                if self.selected + 1 < self.hits.len() {
                    self.selected += 1;
                }
                self.moved = true;
                None
            }
            Input::Action(Action::FindCase) => {
                self.case_sensitive = !self.case_sensitive;
                self.research()
            }
            Input::Action(Action::FindRegex) => {
                self.regex = !self.regex;
                self.research()
            }
            // The find bar's Enter, as the panel reads keys in its scope.
            Input::Action(Action::FindNext) => self.submit(),
            Input::Action(Action::Tab) => {
                self.in_replace = !self.in_replace;
                None
            }
            Input::Action(Action::ProjectReplace) => self.replace_all(),
            input => match self.field().handle(input)? {
                Outcome::Submit => self.submit(),
                Outcome::Cancel => Some(Searched::Close),
            },
        }
    }

    /// The field keys type into.
    fn field(&mut self) -> &mut PromptBar {
        if self.in_replace {
            &mut self.replace
        } else {
            &mut self.query
        }
    }

    /// Pasted text goes into the field as if typed; a line break is Enter. A tab
    /// moves between the fields as the key does: on Windows Tab followed quickly
    /// by typing arrives as one paste.
    pub fn paste(&mut self, text: &str) -> Option<Searched> {
        for (i, part) in text.split('\t').enumerate() {
            if i > 0 {
                self.in_replace = !self.in_replace;
            }
            match self.field().paste(part) {
                None => {}
                Some(Outcome::Submit) => return self.submit(),
                Some(Outcome::Cancel) => return Some(Searched::Close),
            }
        }
        None
    }

    /// Alt+A: once a search has finished with hits, replace them in every
    /// file listed, with what the Replace field holds.
    fn replace_all(&self) -> Option<Searched> {
        let query = self.searched.clone()?;
        if self.running || self.hits.is_empty() {
            return None;
        }
        let mut files: Vec<String> = self.hits.iter().map(|hit| hit.path.clone()).collect();
        // Sorted by path already, so each file's hits sit together.
        files.dedup();
        Some(Searched::Replace {
            query,
            with: self.replace.text().to_string(),
            files,
        })
    }

    fn submit(&mut self) -> Option<Searched> {
        let query = self.query();
        if query.pattern.is_empty() {
            return None;
        }
        if self.searched.as_ref() == Some(&query) {
            self.selected().cloned().map(Searched::Open)
        } else {
            Some(Searched::Start(query))
        }
    }

    /// A toggle changed what matches: the listed hits are stale, so search again
    /// if there are any to replace, as the find bar does.
    fn research(&mut self) -> Option<Searched> {
        let query = self.query();
        (self.searched.is_some() && !query.pattern.is_empty()).then_some(Searched::Start(query))
    }

    /// What the row under the fields says: the error, how many matches in how
    /// many files, or that the search is still going (SPEC_V1_LAYOUT §7.3).
    pub fn status(&self) -> String {
        if let Some(err) = &self.error {
            return err.clone();
        }
        if self.searched.is_none() {
            return "type to search the project (.gitignore respected)".to_string();
        }
        let matches: usize = self.hits.iter().map(|hit| hit.line.matches.len()).sum();
        // Hits are sorted by path, so each file's sit together.
        let files = self
            .hits
            .iter()
            .enumerate()
            .filter(|&(i, hit)| i == 0 || self.hits[i - 1].path != hit.path)
            .count();
        let count = if matches == 0 && !self.running {
            "no matches".to_string()
        } else {
            format!(
                "{} in {}",
                plural(matches, "match", "matches"),
                plural(files, "file", "files")
            )
        };
        if self.running {
            format!("searching… {count}")
        } else {
            count
        }
    }

    /// Where the card goes in `area` (SPEC_V1_LAYOUT §7.3): centred, 20 cells
    /// narrower than the screen and 8 rows shorter, within 60..=140 by 14..=40.
    pub fn card(area: Rect) -> Rect {
        let width = area.width.saturating_sub(20).clamp(60, 140).min(area.width);
        let height = area.height.saturating_sub(8).clamp(14, 40).min(area.height);
        Rect {
            x: area.x + (area.width - width) / 2,
            y: area.y + (area.height - height) / 2,
            width,
            height,
        }
    }

    /// Where the `Aa` chip starts in `card`: the two chips end four cells in
    /// from its right edge, and never start before the query's field.
    fn chips_x(card: Rect) -> u16 {
        let right = card.right().saturating_sub(4);
        right
            .saturating_sub(2 * CHIP_WIDTH + 1)
            .max(card.x.saturating_add(6))
    }

    /// The chip under `at` when the panel is drawn in `area`: 0 for `Aa`, 1
    /// for `.*`. The one the pointer there lights.
    pub fn chip_at(area: Rect, at: Position) -> Option<usize> {
        let card = Self::card(area);
        let right = card.right().saturating_sub(4);
        if at.y != card.y + 2 || at.x >= right {
            return None;
        }
        let offset = at.x.checked_sub(Self::chips_x(card))?;
        let (chip, within) = (offset / (CHIP_WIDTH + 1), offset % (CHIP_WIDTH + 1));
        (chip < 2 && within < CHIP_WIDTH).then_some(usize::from(chip))
    }

    /// Dims `area` and draws the panel centred in it (tinkery-forge spec F15):
    /// its title in the edge; the Search strip with its chips and the Replace
    /// strip under it; a blank; the match count; the hits, the selected one a
    /// pill; then the keys, on the row above the bottom edge. Returns where the
    /// cursor goes, in the field that takes the keys. `hovered` is what the
    /// pointer is on: the screen row inside the card (`kit::row_under`), whose
    /// hit is lit, and the chip (`chip_at`), lit while it's off.
    pub fn render(
        &self,
        look: Look,
        theme: &Theme,
        frame: &mut Frame,
        area: Rect,
        hovered: (Option<u16>, Option<usize>),
    ) -> (u16, u16) {
        let (hovered_row, hovered_chip) = hovered;
        let out = frame.buffer_mut();
        kit::dim(theme, out, area);
        let card = Self::card(area);
        let inside = kit::panel_at(look, theme, out, card, TITLE);
        if inside.is_empty() || card.height < CHROME + 1 || card.width < 40 {
            return (card.x, card.y);
        }
        let (left, right) = (card.x + 4, card.right() - 4);

        let field = kit::header(look, theme, out, card, Vec::new());
        // The chips keep their own fills, which `header` would paint over, so
        // they're drawn here, where its right-hand text would end.
        let chips = self.chips(look, theme, hovered_chip);
        let chips_x = Self::chips_x(card);
        kit::put(out, chips_x, field.y, &chips, right);
        let query_field = Rect {
            width: chips_x.saturating_sub(2).saturating_sub(field.x),
            ..field
        };
        let query_at = render_field(theme, out, query_field, &self.query, !self.in_replace);

        let replace_y = card.y + 3;
        let strip = Rect::new(card.x + 1, replace_y, card.width - 2, 1);
        kit::strip(look, theme, out, strip, theme.raised2, theme.raised);
        let mark = Style::new()
            .fg(theme.accent2)
            .bg(theme.raised2)
            .add_modifier(Modifier::BOLD);
        out.set_string(left, replace_y, kit::HEADER_MARK, mark);
        let replace_field = Rect::new(field.x, replace_y, right.saturating_sub(field.x), 1);
        let replace_at = render_field(theme, out, replace_field, &self.replace, self.in_replace);
        if !self.in_replace && self.replace.text().is_empty() {
            let quiet = kit::ink(theme, theme.muted, theme.raised2);
            out.set_stringn(
                replace_at.0,
                replace_at.1,
                "tab to replace",
                usize::from(right.saturating_sub(replace_at.0)),
                Style::new().fg(quiet).bg(theme.raised2),
            );
        }

        let status_fg = if self.error.is_some() {
            theme.err
        } else {
            theme.muted
        };
        out.set_stringn(
            left,
            card.y + 5,
            self.status(),
            usize::from(right - left),
            Style::new().fg(status_fg).bg(theme.raised),
        );

        let rows = usize::from(card.height - CHROME);
        // The `path:line` column is as wide as the widest, up to 34, so the
        // lines' text starts in one column.
        let place_width = self
            .hits
            .iter()
            .map(|hit| place(hit).width())
            .max()
            .unwrap_or(0)
            .min(34);
        // Scrolls just far enough to keep the selection on the last row.
        let first = self.selected.saturating_sub(rows.saturating_sub(1));
        for (line, (index, hit)) in self
            .hits
            .iter()
            .enumerate()
            .skip(first)
            .take(rows)
            .enumerate()
        {
            let y = card.y + 6 + u16::try_from(line).unwrap_or(u16::MAX);
            let row = Rect::new(card.x + 1, y, card.width - 2, 1);
            let selected = index == self.selected;
            let lit = hovered_row == Some(y);
            let ink = kit::sel_row(look, theme, out, row, selected, lit, theme.raised);
            let text = Rect::new(left, y, right - left, 1);
            render_hit(theme, out, text, hit, place_width, selected, ink);
        }

        let hints = self.hints(look, theme, right - left);
        kit::put(out, left, card.bottom() - 2, &hints, right);
        if self.in_replace {
            replace_at
        } else {
            query_at
        }
    }

    /// The case and regex chips at the right of the Search strip, a blank
    /// apart, as the find bar draws them: `acc_ink` on the accent and bold
    /// while on, `muted` on `btn` while off, and then lit when it's the one
    /// at `hovered`.
    fn chips(&self, look: Look, theme: &Theme, hovered: Option<usize>) -> Vec<Span<'static>> {
        let fill = theme.raised2;
        let chip = |index: usize, text: &str, on: bool| {
            kit::toggle(look, theme, text, on, hovered == Some(index), fill)
        };
        let mut chips = chip(0, "Aa", self.case_sensitive);
        chips.push(Span::styled(" ", Style::new().bg(fill)));
        chips.extend(chip(1, ".*", self.regex));
        chips
    }

    /// The keys, ending `Esc close`. All of them need more room than the card
    /// has on a 100-cell screen, so the ones easiest to guess go first: the
    /// arrows, then Tab.
    fn hints(&self, look: Look, theme: &Theme, room: u16) -> Vec<Span<'static>> {
        let tab = if self.in_replace { "search" } else { "replace" };
        let all = [
            ("↑ ↓", "move"),
            ("⏎", "open"),
            ("Tab", tab),
            ("Alt+A", "replace all"),
            ("Esc", "close"),
        ];
        let no_tab = [all[1], all[3], all[4]];
        let fits = |pairs: &[(&str, &str)]| {
            let hints = kit::hints(look, theme, pairs, theme.raised);
            (kit::width(&hints) <= room).then_some(hints)
        };
        fits(&all)
            .or_else(|| fits(&all[1..]))
            .or_else(|| fits(&no_tab))
            .unwrap_or_else(|| kit::hints(look, theme, &[all[1], all[4]], theme.raised))
    }
}

/// What the panel's top edge says.
const TITLE: &str = "Project search";
/// A chip's cells: its two letters between two ends.
const CHIP_WIDTH: u16 = 4;
/// The rows other than the hits: the edge, a blank, the two strips, a blank
/// and the match count above them; a blank, the hints and the edge below.
const CHROME: u16 = 9;
/// The fields' labels, `Search` and `Replace`, padded to one width so the values
/// line up.
const LABEL_WIDTH: u16 = 7;

/// One field across `area` on its strip of `raised2`: the label (`text` while
/// the field takes the keys, else `muted`) padded to `LABEL_WIDTH`, two blanks,
/// then the value in `strong`. Returns where the cursor goes.
fn render_field(
    theme: &Theme,
    out: &mut Buffer,
    area: Rect,
    field: &PromptBar,
    focused: bool,
) -> (u16, u16) {
    if area.width == 0 {
        return (area.x, area.y);
    }
    let on = Style::new().bg(theme.raised2);
    let quiet = kit::ink(theme, theme.muted, theme.raised2);
    let label = on.fg(if focused { theme.text } else { quiet });
    out.set_stringn(area.x, area.y, field.label, usize::from(area.width), label);
    let x = (area.x + LABEL_WIDTH + 2).min(area.right());
    let value = Rect {
        x,
        width: area.right() - x,
        ..area
    };
    field.render_value(out, value, on.fg(theme.strong))
}

/// `count` and the noun that goes with it, e.g. `1 file` or `2 files`.
fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// A hit's `path:line`, the line counting from 1.
fn place(hit: &Hit) -> String {
    format!("{}:{}", hit.path, hit.line.line + 1)
}

/// One hit across `area`, the cells a row's text may take, in the styles
/// `sel_row` gave for it: `path:line` padded to `place_width` (the file name
/// `strong`, the rest faint), then two blanks and the line, scrolled so its
/// first match shows, each match on `find_match_bg`. The selected row's
/// matches are in `find_current` colours.
fn render_hit(
    theme: &Theme,
    out: &mut Buffer,
    area: Rect,
    hit: &Hit,
    place_width: usize,
    selected: bool,
    ink: RowInk,
) {
    // A match is told by its ground, which `mono` has none to mix, and which
    // must not take the row's fill as the rest of the row's text does.
    let matched = if !theme.ramps() {
        ink.of(Style::new().add_modifier(Modifier::BOLD | Modifier::UNDERLINED))
    } else if selected {
        Style::new().fg(theme.bg).bg(theme.warn)
    } else {
        Style::new().fg(theme.fg).bg(mix(theme.bg, theme.warn, 0.3))
    };
    let name_style = if selected {
        ink.label
    } else {
        ink.of(Style::new().fg(theme.strong))
    };

    let (dir, name) = match hit.path.rfind('/') {
        Some(at) => hit.path.split_at(at + 1),
        None => ("", hit.path.as_str()),
    };
    let line = format!(":{}", hit.line.line + 1);
    let mut cells: Vec<(char, Style)> = dir
        .chars()
        .map(|c| (c, ink.faint))
        .chain(name.chars().map(|c| (c, name_style)))
        .chain(line.chars().map(|c| (c, ink.faint)))
        .collect();
    // Too long for the column: keep the end, where the file name and line are.
    if cells.len() > place_width {
        let cut = cells.len() + 1 - place_width;
        cells.splice(..cut, [('…', ink.faint)]);
    }
    let end = area.right();
    let place_end = area
        .x
        .saturating_add(u16::try_from(place_width).unwrap_or(u16::MAX))
        .min(end);
    put_cells(out, area.x, area.y, place_end, &cells);

    let text_x = place_end.saturating_add(2);
    let room = usize::from(end.saturating_sub(text_x));
    let first_match = hit.line.matches.first().map_or(0, |m| m.start);
    // Scrolled so the match lands 20 cells in when it would be past the room.
    let start = if first_match + 20 > room {
        first_match.saturating_sub(20)
    } else {
        0
    };
    let text = ink.of(Style::new().fg(theme.fg));
    let mut cells: Vec<(char, Style)> = Vec::new();
    if start > 0 {
        cells.push(('…', ink.faint));
    }
    cells.extend(
        hit.line
            .text
            .chars()
            .enumerate()
            .skip(start)
            .map(|(at, c)| {
                let in_match = hit.line.matches.iter().any(|m| m.contains(&at));
                (c, if in_match { matched } else { text })
            }),
    );
    put_cells(out, text_x, area.y, end, &cells);
}

/// Draws `cells` from `x` on row `y`, stopping before a cell would cross `end`.
fn put_cells(out: &mut Buffer, mut x: u16, y: u16, end: u16, cells: &[(char, Style)]) {
    for &(c, style) in cells {
        let width = u16::try_from(c.width().unwrap_or(0)).unwrap_or(1);
        if x + width > end {
            break;
        }
        out.set_string(x, y, c.to_string(), style);
        x += width;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::LineHit;
    use crate::ui::kit::{CAP_L, CAP_R};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;

    /// What a hint line reads with round ends.
    fn hints_text(pairs: &[(&str, &str)]) -> String {
        pairs
            .iter()
            .map(|(key, what)| format!("{CAP_L}{key}{CAP_R} {what}"))
            .collect::<Vec<_>>()
            .join("   ")
    }

    #[expect(
        clippy::single_range_in_vec_init,
        reason = "a list of one match, not a range of numbers"
    )]
    fn hit(path: &str, line: usize, text: &str) -> Hit {
        Hit {
            path: path.to_string(),
            line: LineHit {
                line,
                col: 0,
                text: text.to_string(),
                matches: vec![0..4],
            },
        }
    }

    fn type_query(search: &mut ProjectSearch, text: &str) {
        for c in text.chars() {
            assert_eq!(search.handle(Input::Text(c)), None);
        }
    }

    fn query(pattern: &str, case_sensitive: bool, regex: bool) -> Query {
        Query {
            pattern: pattern.to_string(),
            case_sensitive,
            regex,
        }
    }

    #[test]
    fn enter_searches_then_opens_the_selected_hit() {
        let mut search = ProjectSearch::new();
        let enter = Input::Action(Action::FindNext);
        // Nothing to search for yet.
        assert_eq!(search.handle(enter), None);
        type_query(&mut search, "TODO");
        let started = search.handle(enter);
        assert_eq!(started, Some(Searched::Start(query("TODO", false, false))));
        search.start(1, query("TODO", false, false));
        assert_eq!(search.status(), "searching… 0 matches in 0 files");
        // Enter before any hit arrives has nothing to open.
        assert_eq!(search.handle(enter), None);
        search.add(1, vec![hit("b.rs", 0, "TODO b"), hit("a.rs", 4, "TODO a")]);
        search.finish(1);
        assert_eq!(search.status(), "2 matches in 2 files");
        assert_eq!(search.selected().map(|h| h.path.as_str()), Some("a.rs"));
        search.handle(Input::Action(Action::Move(Motion::Down)));
        search.handle(Input::Action(Action::Move(Motion::Down)));
        assert_eq!(
            search.handle(enter),
            Some(Searched::Open(hit("b.rs", 0, "TODO b")))
        );
        // Editing the query makes Enter search again.
        type_query(&mut search, "!");
        assert_eq!(
            search.handle(enter),
            Some(Searched::Start(query("TODO!", false, false)))
        );
        assert_eq!(
            search.handle(Input::Action(Action::Cancel)),
            Some(Searched::Close)
        );
    }

    #[test]
    fn toggles_search_again_once_a_search_has_run() {
        let mut search = ProjectSearch::new();
        type_query(&mut search, "a.c");
        // Before the first Enter a toggle only changes the next search.
        assert_eq!(search.handle(Input::Action(Action::FindCase)), None);
        search.start(1, query("a.c", true, false));
        assert_eq!(
            search.handle(Input::Action(Action::FindRegex)),
            Some(Searched::Start(query("a.c", true, true)))
        );
        search.fail("invalid regex".into());
        assert_eq!(search.status(), "invalid regex");
    }

    #[test]
    fn tab_moves_to_replace_and_alt_a_asks_for_the_listed_files() {
        let mut search = ProjectSearch::new();
        let replace = Input::Action(Action::ProjectReplace);
        type_query(&mut search, "TODO");
        // Nothing listed yet, so nothing to replace.
        assert_eq!(search.handle(replace), None);
        assert_eq!(search.handle(Input::Action(Action::Tab)), None);
        type_query(&mut search, "DONE");
        search.start(1, query("TODO", false, false));
        search.add(
            1,
            vec![
                hit("b.rs", 0, "TODO"),
                hit("a.rs", 4, "TODO"),
                hit("a.rs", 1, "TODO"),
            ],
        );
        // Still searching: the list isn't complete.
        assert_eq!(search.handle(replace), None);
        search.finish(1);
        assert_eq!(
            search.handle(replace),
            Some(Searched::Replace {
                query: query("TODO", false, false),
                with: "DONE".into(),
                files: vec!["a.rs".into(), "b.rs".into()],
            })
        );
        // Tab goes back to the query, which typing then edits.
        search.handle(Input::Action(Action::Tab));
        type_query(&mut search, "!");
        assert_eq!(search.query().pattern, "TODO!");
        assert_eq!(search.replace.text(), "DONE");
        // A pasted tab switches fields too.
        assert_eq!(search.paste("?\tX"), None);
        assert_eq!(search.query().pattern, "TODO!?");
        assert_eq!(search.replace.text(), "DONEX");
    }

    #[test]
    fn late_batches_keep_a_moved_selection_and_stale_ones_are_dropped() {
        let mut search = ProjectSearch::new();
        search.start(2, query("x", false, false));
        search.add(2, vec![hit("m.rs", 0, "x"), hit("z.rs", 0, "x")]);
        // Unmoved, the selection stays on whatever sorts first.
        search.add(2, vec![hit("c.rs", 0, "x")]);
        assert_eq!(search.selected().map(|h| h.path.as_str()), Some("c.rs"));
        search.handle(Input::Action(Action::Move(Motion::Down)));
        search.handle(Input::Action(Action::Move(Motion::Down)));
        search.add(2, vec![hit("a.rs", 0, "x")]);
        assert_eq!(search.selected().map(|h| h.path.as_str()), Some("z.rs"));
        search.add(1, vec![hit("b.rs", 0, "x")]);
        assert_eq!(search.hits().len(), 4);
        search.finish(1);
        assert_eq!(search.status(), "searching… 4 matches in 4 files");
        search.finish(2);
        assert_eq!(search.status(), "4 matches in 4 files");
    }

    fn listed() -> ProjectSearch {
        let mut search = ProjectSearch::new();
        type_query(&mut search, "TODO");
        search.start(1, query("TODO", false, false));
        search.add(
            1,
            vec![hit("src/lib.rs", 2, "TODO later"), hit("x.rs", 0, "TODO")],
        );
        search.finish(1);
        search
    }

    /// The panel over a screen of the theme's `fg` on its `bg`, at 100x30.
    fn draw(search: &ProjectSearch, theme: &Theme) -> anyhow::Result<(Buffer, (u16, u16))> {
        let mut terminal = Terminal::new(TestBackend::new(100, 30))?;
        let mut at = (0, 0);
        terminal.draw(|frame| {
            let area = frame.area();
            frame
                .buffer_mut()
                .set_style(area, Style::new().bg(theme.bg).fg(theme.fg));
            at = search.render(Look::default(), theme, frame, area, (None, None));
        })?;
        Ok((terminal.backend().buffer().clone(), at))
    }

    fn card_row(buffer: &Buffer, card: Rect, y: u16) -> String {
        (card.x..card.right())
            .map(|x| buffer[(x, y)].symbol())
            .collect()
    }

    /// What row `y` of `card` says where a row's text goes: four cells in from
    /// each edge, without the blanks around it.
    fn text_of(buffer: &Buffer, card: Rect, y: u16) -> String {
        let cells: String = (card.x + 4..card.right() - 4)
            .map(|x| buffer[(x, y)].symbol())
            .collect();
        cells.trim().to_string()
    }

    /// The cells of row `y` of `card` drawn on `bg`, as text.
    fn on_bg(buffer: &Buffer, card: Rect, y: u16, bg: Color) -> String {
        (card.x..card.right())
            .filter(|&x| buffer[(x, y)].bg == bg)
            .map(|x| buffer[(x, y)].symbol())
            .collect()
    }

    /// Asserts row `y` of `card` is a strip: a pill of `raised2` on `raised`
    /// across the card's inside, with `✦` in `accent2`, bold, four cells in.
    fn assert_strip(buffer: &Buffer, theme: &Theme, card: Rect, y: u16) {
        let (left, right) = (&buffer[(card.x + 1, y)], &buffer[(card.right() - 2, y)]);
        assert_eq!((left.symbol(), right.symbol()), (CAP_L, CAP_R), "row {y}");
        assert_eq!((left.fg, left.bg), (theme.raised2, theme.raised), "row {y}");
        assert_eq!(
            (right.fg, right.bg),
            (theme.raised2, theme.raised),
            "row {y}"
        );
        let mark = &buffer[(card.x + 4, y)];
        assert_eq!(mark.symbol(), "✦", "row {y}");
        assert_eq!(
            (mark.fg, mark.bg),
            (theme.accent2, theme.raised2),
            "row {y}"
        );
        assert!(mark.modifier.contains(Modifier::BOLD), "row {y}");
    }

    /// Asserts row `y` of `card` is the selected row: a pill of `hov` on
    /// `raised` across the card's inside, with `›` in the accent, bold.
    fn assert_selected(buffer: &Buffer, theme: &Theme, card: Rect, y: u16) {
        let (left, right) = (&buffer[(card.x + 1, y)], &buffer[(card.right() - 2, y)]);
        assert_eq!((left.symbol(), right.symbol()), (CAP_L, CAP_R), "row {y}");
        assert_eq!((left.fg, left.bg), (theme.hov, theme.raised), "row {y}");
        assert_eq!((right.fg, right.bg), (theme.hov, theme.raised), "row {y}");
        let mark = &buffer[(card.x + 2, y)];
        assert_eq!(mark.symbol(), "›", "row {y}");
        assert_eq!((mark.fg, mark.bg), (theme.accent, theme.hov), "row {y}");
        assert!(mark.modifier.contains(Modifier::BOLD), "row {y}");
    }

    /// Asserts row `y` of `card` isn't lit: no pill, no mark.
    fn assert_resting(buffer: &Buffer, theme: &Theme, card: Rect, y: u16) {
        for x in card.x + 1..card.right() - 1 {
            let cell = &buffer[(x, y)];
            assert_ne!(cell.bg, theme.hov, "{x},{y}");
            assert!(!matches!(cell.symbol(), CAP_L | CAP_R | "›"), "{x},{y}");
        }
    }

    #[test]
    fn the_search_is_a_panel_with_two_strips_pill_rows_and_hints() -> anyhow::Result<()> {
        let search = listed();
        // SPEC_V1_LAYOUT §7.3 at 100x30: 80 wide, 22 tall, centred.
        let card = ProjectSearch::card(Rect::new(0, 0, 100, 30));
        assert_eq!(card, Rect::new(10, 4, 80, 22));
        for name in ["tinkery", "tinkery-light"] {
            let theme = Theme::named(name).expect("a listed theme exists");
            let (buffer, at) = draw(&search, &theme)?;
            // Outside the card the screen is dimmed: text into its ground, then
            // all of it towards black.
            let black = Color::Rgb(0, 0, 0);
            assert_eq!(buffer[(0, 0)].bg, mix(theme.bg, black, 0.45), "{name}");
            assert_eq!(
                buffer[(0, 0)].fg,
                mix(mix(theme.fg, theme.bg, 0.6), black, 0.45),
                "{name}"
            );
            // An accent edge with the title in it.
            let edge = card_row(&buffer, card, card.y);
            assert!(edge.starts_with(" ▁ Project search ▁"), "{name}: {edge:?}");
            assert_eq!(buffer[(card.x + 30, card.y)].fg, theme.accent, "{name}");
            assert_eq!(buffer[(card.x, card.y + 1)].symbol(), "▕", "{name}");
            assert_eq!(buffer[(card.x + 1, card.bottom() - 1)].symbol(), "▔");
            assert_eq!(text_of(&buffer, card, card.y + 1), "", "{name}");

            // The Search strip, which has the keys: its label in `text`, the
            // query in `strong`, the chips at the right, both off.
            let y = card.y + 2;
            assert_strip(&buffer, &theme, card, y);
            let line = card_row(&buffer, card, y);
            assert!(
                line.starts_with(&format!("▕{CAP_L}  ✦ Search   TODO ")),
                "{name}: {line:?}"
            );
            assert!(
                line.ends_with(&format!("{CAP_L}Aa{CAP_R} {CAP_L}.*{CAP_R}  {CAP_R}▏")),
                "{name}: {line:?}"
            );
            let label = &buffer[(card.x + 6, y)];
            assert_eq!((label.fg, label.bg), (theme.text, theme.raised2), "{name}");
            let value = &buffer[(card.x + 15, y)];
            assert_eq!((value.fg, value.bg), (theme.strong, theme.raised2));
            assert_eq!(on_bg(&buffer, card, y, theme.btn), "Aa.*", "{name}");
            assert_eq!(buffer[(card.right() - 6, y)].fg, theme.muted, "{name}");
            assert_eq!(on_bg(&buffer, card, y, theme.accent), "", "{name}");
            assert_eq!(at, (card.x + 19, y), "{name}");

            // The Replace strip under it, waiting: label and hint in `muted`.
            let y = card.y + 3;
            assert_strip(&buffer, &theme, card, y);
            let line = card_row(&buffer, card, y);
            assert!(
                line.starts_with(&format!("▕{CAP_L}  ✦ Replace  tab to replace ")),
                "{name}: {line:?}"
            );
            let label = &buffer[(card.x + 6, y)];
            assert_eq!((label.fg, label.bg), (theme.muted, theme.raised2), "{name}");
            assert_eq!(buffer[(card.x + 15, y)].fg, theme.muted, "{name}");

            // A blank, the count in `muted`, then the hits.
            assert_eq!(text_of(&buffer, card, card.y + 4), "", "{name}");
            let count = text_of(&buffer, card, card.y + 5);
            assert_eq!(count, "2 matches in 2 files", "{name}");
            let cell = &buffer[(card.x + 4, card.y + 5)];
            assert_eq!((cell.fg, cell.bg), (theme.muted, theme.raised), "{name}");
            assert_resting(&buffer, &theme, card, card.y + 5);
            // `src/lib.rs:3` sets the path column's width; the text lines up
            // after it.
            let (first, second) = (card.y + 6, card.y + 7);
            assert_eq!(
                text_of(&buffer, card, first),
                "src/lib.rs:3  TODO later",
                "{name}"
            );
            assert_eq!(
                text_of(&buffer, card, second),
                "x.rs:1        TODO",
                "{name}"
            );
            // The first hit is selected: a pill, its file name `strong` bold,
            // the folder and line in `text`, its match in `find_current`.
            assert_selected(&buffer, &theme, card, first);
            assert_eq!(buffer[(card.x + 4, first)].fg, theme.text, "{name}");
            let file = &buffer[(card.x + 8, first)];
            assert_eq!((file.fg, file.bg), (theme.strong, theme.hov), "{name}");
            assert!(file.modifier.contains(Modifier::BOLD), "{name}");
            assert_eq!(buffer[(card.x + 14, first)].fg, theme.text, "{name}");
            assert_eq!(on_bg(&buffer, card, first, theme.warn), "TODO", "{name}");
            assert_eq!(buffer[(card.x + 18, first)].fg, theme.bg, "{name}");
            let rest = &buffer[(card.x + 23, first)];
            assert_eq!((rest.fg, rest.bg), (theme.fg, theme.hov), "{name}");
            // The other rests: its file name `strong`, its match on
            // `find_match_bg`.
            assert_resting(&buffer, &theme, card, second);
            let file = &buffer[(card.x + 4, second)];
            assert_eq!((file.fg, file.bg), (theme.strong, theme.raised), "{name}");
            assert_eq!(buffer[(card.x + 8, second)].fg, theme.muted, "{name}");
            let match_bg = mix(theme.bg, theme.warn, 0.3);
            assert_eq!(on_bg(&buffer, card, second, match_bg), "TODO", "{name}");
            assert_eq!(buffer[(card.x + 18, second)].fg, theme.fg, "{name}");

            // A blank, then the keys on the row above the bottom edge.
            assert_eq!(text_of(&buffer, card, card.bottom() - 3), "", "{name}");
            assert_eq!(
                text_of(&buffer, card, card.bottom() - 2),
                hints_text(&[
                    ("⏎", "open"),
                    ("Tab", "replace"),
                    ("Alt+A", "replace all"),
                    ("Esc", "close"),
                ]),
                "{name}"
            );
            assert_eq!(buffer[(card.x + 4, card.bottom() - 2)].symbol(), CAP_L);
            // Nothing of the old look: no lit edge, no glow, no `⏎` at the
            // selected row's right.
            let glow = mix(theme.raised, theme.accent, 0.3);
            for row in card.top()..card.bottom() {
                for x in card.left()..card.right() {
                    let cell = &buffer[(x, row)];
                    assert_ne!(cell.symbol(), "▀", "{name}: {x},{row}");
                    assert_ne!(cell.bg, glow, "{name}: {x},{row}");
                }
            }
            assert!(!card_row(&buffer, card, first).contains('⏎'), "{name}");
        }
        Ok(())
    }

    #[test]
    fn the_field_with_the_keys_has_its_label_in_text_and_a_chip_that_is_on_is_accent()
    -> anyhow::Result<()> {
        let mut search = listed();
        let theme = Theme::default();
        let card = ProjectSearch::card(Rect::new(0, 0, 100, 30));
        search.handle(Input::Action(Action::FindRegex));
        search.handle(Input::Action(Action::Tab));
        type_query(&mut search, "ab");
        search.handle(Input::Action(Action::Move(Motion::Down)));
        let (buffer, at) = draw(&search, &theme)?;
        let (search_y, replace_y) = (card.y + 2, card.y + 3);
        assert_eq!(buffer[(card.x + 6, search_y)].fg, theme.muted);
        assert_eq!(buffer[(card.x + 6, replace_y)].fg, theme.text);
        let line = card_row(&buffer, card, replace_y);
        assert!(
            line.starts_with(&format!("▕{CAP_L}  ✦ Replace  ab ")),
            "{line:?}"
        );
        assert!(!line.contains("tab to replace"), "{line:?}");
        assert_eq!(buffer[(card.x + 15, replace_y)].fg, theme.strong);
        // The cursor is in the Replace field, after what's typed.
        assert_eq!(at, (card.x + 17, replace_y));
        // `.*` is on: `acc_ink` on the accent, bold; `Aa` stays on `btn`.
        assert_eq!(on_bg(&buffer, card, search_y, theme.accent), ".*");
        assert_eq!(on_bg(&buffer, card, search_y, theme.btn), "Aa");
        let on = &buffer[(card.right() - 7, search_y)];
        assert_eq!(on.fg, theme.acc_ink);
        assert!(on.modifier.contains(Modifier::BOLD));
        // The pill moved with the selection.
        assert_resting(&buffer, &theme, card, card.y + 6);
        assert_selected(&buffer, &theme, card, card.y + 7);
        // Tab now goes back to the Search field.
        let hints = text_of(&buffer, card, card.bottom() - 2);
        assert!(hints.contains(&hints_text(&[("Tab", "search")])), "{hints}");
        assert!(hints.ends_with(&hints_text(&[("Esc", "close")])), "{hints}");
        Ok(())
    }

    #[test]
    fn the_hints_drop_the_arrows_then_tab_as_the_card_narrows() {
        let theme = Theme::default();
        let search = ProjectSearch::new();
        let look = Look {
            pill_caps: false,
            ..Look::default()
        };
        let text = |room: u16| -> String {
            search
                .hints(look, &theme, room)
                .iter()
                .map(|span| span.content.as_ref())
                .collect()
        };
        let all = " ↑   ↓  move    ⏎  open    Tab  replace    Alt+A  replace all    Esc  close";
        assert_eq!(text(75), all);
        assert_eq!(text(74), all[" ↑   ↓  move   ".len()..]);
        assert_eq!(text(60), text(74));
        assert_eq!(text(59), " ⏎  open    Alt+A  replace all    Esc  close");
        assert_eq!(text(43), " ⏎  open    Esc  close");
        // Every one of them ends `Esc close`.
        for room in [0, 43, 59, 74, 75, 200] {
            assert!(text(room).ends_with(" Esc  close"), "{room}");
        }
    }

    #[test]
    fn mono_reverses_the_selected_hit_and_keeps_matches_bold_and_underlined() -> anyhow::Result<()>
    {
        let search = listed();
        let mono = Theme::named("mono").expect("mono exists");
        let reversed = Theme::highlight(mono.hov, mono.strong);
        let card = ProjectSearch::card(Rect::new(0, 0, 100, 30));
        let mut terminal = Terminal::new(TestBackend::new(100, 30))?;
        terminal.draw(|frame| {
            search.render(Look::default(), &mono, frame, frame.area(), (None, None));
        })?;
        let buffer = terminal.backend().buffer();
        // The screen dims with the modifier; the card doesn't.
        assert!(buffer[(0, 0)].modifier.contains(Modifier::DIM));
        let (first, second) = (card.y + 6, card.y + 7);
        for x in card.x + 1..card.right() - 1 {
            let cell = &buffer[(x, first)];
            assert!(cell.modifier.contains(Modifier::REVERSED), "{x}");
            // Nothing on the row has a colour of its own.
            assert_eq!((Some(cell.fg), Some(cell.bg)), (reversed.fg, reversed.bg));
        }
        assert_eq!(buffer[(card.x + 2, first)].symbol(), "›");
        // On both rows the match, and nothing else of the line, is bold and
        // underlined.
        let marked = Modifier::BOLD | Modifier::UNDERLINED;
        for y in [first, second] {
            let matched: String = (card.x + 4..card.right() - 4)
                .filter(|&x| buffer[(x, y)].modifier.contains(marked))
                .map(|x| buffer[(x, y)].symbol())
                .collect();
            assert_eq!(matched, "TODO", "row {y}");
        }
        for row in card.top()..card.bottom() {
            for x in card.left()..card.right() {
                let cell = &buffer[(x, row)];
                // Terminal colours only, and no Nerd Font glyph.
                let rgb = |c| matches!(c, Color::Rgb(..));
                assert!(!rgb(cell.fg) && !rgb(cell.bg), "{x},{row}");
                assert!(!matches!(cell.symbol(), CAP_L | CAP_R), "{x},{row}");
                assert!(!cell.modifier.contains(Modifier::DIM), "{x},{row}");
                if row != first {
                    assert!(!cell.modifier.contains(Modifier::REVERSED), "{x},{row}");
                }
            }
        }
        Ok(())
    }
}
