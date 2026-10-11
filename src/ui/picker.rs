//! The picker: a query over a list, filtered fuzzily as the query is typed, best
//! match first. The "cast" palette (design README §3) lists the project's files
//! then the commands, and its query's first character switches mode: `>` lists
//! only commands, `:` goes to a line, `/` searches the project's text. F5 lists
//! the `[[run]]` entries by name as a dialog.

use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32Str};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use crate::buffer::movement::Motion;
use crate::keymap::{Action, Input};
use crate::theme::Theme;
use crate::ui::kit::{self, Look, RowInk};
use crate::ui::prompt::{Outcome, PromptBar};

/// Widest the choices card gets, its edge included.
const MAX_WIDTH: u16 = 80;

/// One entry of a list of choices, such as a run command: picked by its name,
/// listed with a detail and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub name: String,
    pub detail: String,
    pub source: &'static str,
}

/// A global action as the cast lists it: its name in plain words and the key
/// bound to it, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub action: Action,
    pub title: &'static str,
    pub key: Option<String>,
}

/// Most choices a list of them shows; the selection scrolls through the rest.
const CHOICES_ROWS: u16 = 5;
/// Where a choice's detail starts, from where its name does.
const DETAIL_X: u16 = 12;

/// The cast card's width, before clamping to the screen, and the row it drops
/// from (design README §3).
const CAST_WIDTH: u16 = 86;
const CAST_TOP: u16 = 4;
/// Most file rows the cast shows; the selection scrolls through the rest.
const CAST_ROWS: usize = 5;
/// Most command rows under the files: the design keeps them to a taste of what
/// `>` would list.
const CAST_MIXED_COMMANDS: usize = 2;
/// Most command rows once `>` lists only commands.
const CAST_COMMAND_ROWS: usize = 8;
/// A list panel's rows other than its list: the edge, a blank, the query and a
/// blank above it; a blank, the hints and the edge below.
const CHROME: u16 = 7;
/// Right of the query: the palette's name.
const CAST_NAME: &str = "cast";
/// The prefixes the hints start with, each with what it switches to. `@`
/// (symbols) is left out until it does something.
const CAST_PREFIXES: [(&str, &str); 3] = [(">", "commands"), (":", "line"), ("/", "text")];

/// What a key did to the picker, when it did more than move or edit the query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Picked {
    /// Enter on an item: for a file, its path relative to the project root,
    /// `/`-separated; for a list of choices, the choice as given.
    Open(String),
    /// Enter on a command: run its action.
    Run(Action),
    /// Enter on `:N`: move the cursor to line N, counting from 1; it's known to
    /// be in the buffer.
    Line(usize),
    /// Enter on `/text`: open project search for the text.
    Search(String),
    /// Esc: close the picker and do nothing.
    Close,
}

/// What the cast lists, from the query's first character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// No prefix: files, then a few commands.
    Files,
    /// `>`: commands only.
    Commands,
    /// `:`: a line to go to.
    Line,
    /// `/`: text to search the project for.
    Text,
}

/// What `:` holds, checked against the buffer's lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineAsked {
    /// Nothing typed after `:` yet.
    Empty,
    /// A line in the buffer.
    Go(usize),
    /// A number past either end.
    OutOfRange,
    /// Not a number.
    NotANumber,
}

/// One file or command the query matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Match {
    /// Index into `files` or `commands`.
    item: usize,
    score: u32,
}

/// An entry of the list the arrows move through.
#[derive(Debug, Clone, Copy)]
enum Item<'a> {
    File(&'a str),
    Command(&'a Command),
}

#[derive(Debug, Clone)]
pub struct Picker {
    query: PromptBar,
    /// Every file in the project, or the choices; `None` until the background
    /// walk reports back.
    files: Option<Vec<String>>,
    /// Shown when nothing matches the query.
    no_match: &'static str,
    /// For a list of choices, each one's detail and source, in `files` order;
    /// `None` for the cast palette.
    choices: Option<Vec<Choice>>,
    /// The files the query matches, best first.
    matches: Vec<Match>,
    /// What the cast lists under COMMANDS, in the order given.
    commands: Vec<Command>,
    /// The commands the query matches, best first.
    command_matches: Vec<Match>,
    /// How many lines the buffer `:` moves in has.
    lines: usize,
    /// Index into the files listed, then the commands listed after them.
    selected: usize,
    matcher: Matcher,
    /// Commands are matched as words, not paths.
    command_matcher: Matcher,
}

impl Picker {
    /// The cast palette over `commands`, for a buffer of `lines` lines, holding
    /// `query` (`:` for go to line), waiting for the file list.
    pub fn cast(commands: Vec<Command>, lines: usize, query: &str) -> Self {
        let mut picker = Picker {
            query: PromptBar::new("Go to file", query),
            files: None,
            no_match: "no matching files",
            choices: None,
            matches: Vec::new(),
            commands,
            command_matches: Vec::new(),
            lines,
            selected: 0,
            matcher: Matcher::new(Config::DEFAULT.match_paths()),
            command_matcher: Matcher::new(Config::DEFAULT),
        };
        picker.refilter();
        picker
    }

    /// A picker titled `title` over `choices`, listed in the order given and
    /// matched by name. It's drawn as a dialog over the dimmed screen.
    pub fn choices(title: &'static str, choices: Vec<Choice>) -> Self {
        let names = choices.iter().map(|c| c.name.clone()).collect();
        let mut picker = Picker {
            query: PromptBar::new(title, ""),
            files: None,
            no_match: "no match",
            choices: Some(choices),
            matches: Vec::new(),
            commands: Vec::new(),
            command_matches: Vec::new(),
            lines: 0,
            selected: 0,
            matcher: Matcher::new(Config::DEFAULT),
            command_matcher: Matcher::new(Config::DEFAULT),
        };
        picker.set_files(names);
        picker
    }

    /// The walk's result: the files to pick from, relative to the root.
    pub fn set_files(&mut self, files: Vec<String>) {
        self.files = Some(files);
        self.refilter();
    }

    /// The matching paths, best first.
    #[cfg(test)]
    pub fn matches(&self) -> impl Iterator<Item = &str> {
        let files = self.files.as_deref().unwrap_or_default();
        self.matches.iter().map(|m| files[m.item].as_str())
    }

    /// The selected file's path, or the selected command's name.
    #[cfg(test)]
    pub fn selected(&self) -> Option<&str> {
        match self.item(self.selected)? {
            Item::File(path) => Some(path),
            Item::Command(command) => Some(command.title),
        }
    }

    /// What the query's first character asks for; a list of choices has no
    /// modes.
    fn mode(&self) -> Mode {
        if self.choices.is_some() {
            return Mode::Files;
        }
        match self.query.text().chars().next() {
            Some('>') => Mode::Commands,
            Some(':') => Mode::Line,
            Some('/') => Mode::Text,
            _ => Mode::Files,
        }
    }

    /// The query without the prefix that picked the mode.
    fn needle(&self) -> &str {
        let text = self.query.text();
        if self.mode() == Mode::Files {
            return text;
        }
        let mut chars = text.chars();
        chars.next();
        chars.as_str()
    }

    /// How many files the arrows move through: none outside the files mode.
    fn files_listed(&self) -> usize {
        if self.mode() == Mode::Files {
            self.matches.len()
        } else {
            0
        }
    }

    /// How many commands the arrows move through, after the files.
    fn commands_listed(&self) -> usize {
        match self.mode() {
            Mode::Files => self.command_matches.len().min(CAST_MIXED_COMMANDS),
            Mode::Commands => self.command_matches.len(),
            Mode::Line | Mode::Text => 0,
        }
    }

    /// The `index`th entry the arrows move through: the files, then the
    /// commands.
    fn item(&self, index: usize) -> Option<Item<'_>> {
        let files = self.files_listed();
        if index < files {
            let file = self.matches[index].item;
            return self
                .files
                .as_ref()
                .and_then(|all| all.get(file))
                .map(|path| Item::File(path));
        }
        if index - files < self.commands_listed() {
            let command = self.command_matches[index - files].item;
            return self.commands.get(command).map(Item::Command);
        }
        None
    }

    /// The line `:` asks for, checked against the buffer's.
    fn line_asked(&self) -> LineAsked {
        let text = self.needle().trim();
        if text.is_empty() {
            return LineAsked::Empty;
        }
        match text.parse::<usize>() {
            Ok(line) if (1..=self.lines).contains(&line) => LineAsked::Go(line),
            Ok(_) => LineAsked::OutOfRange,
            // Too many digits for a usize is still a number, just too big.
            Err(_) if text.chars().all(|c| c.is_ascii_digit()) => LineAsked::OutOfRange,
            Err(_) => LineAsked::NotANumber,
        }
    }

    /// Up and Down move through the list, Enter acts on the selection or on
    /// what the prefix asks for, and Esc closes; everything else edits the
    /// query.
    pub fn handle(&mut self, input: Input) -> Option<Picked> {
        match input {
            Input::Action(Action::Move(Motion::Up)) => {
                self.selected = self.selected.saturating_sub(1);
                None
            }
            Input::Action(Action::Move(Motion::Down)) => {
                if self.selected + 1 < self.files_listed() + self.commands_listed() {
                    self.selected += 1;
                }
                None
            }
            _ => {
                let before = self.query.text().to_string();
                let outcome = self.query.handle(input);
                self.after_edit(&before, outcome)
            }
        }
    }

    /// Pasted text goes into the query as if typed; a line break is Enter.
    pub fn paste(&mut self, text: &str) -> Option<Picked> {
        let before = self.query.text().to_string();
        let outcome = self.query.paste(text);
        self.after_edit(&before, outcome)
    }

    fn after_edit(&mut self, before: &str, outcome: Option<Outcome>) -> Option<Picked> {
        if self.query.text() != before {
            self.refilter();
        }
        match outcome? {
            Outcome::Submit => self.submit(),
            Outcome::Cancel => Some(Picked::Close),
        }
    }

    /// Enter: the selected file or command, the line asked for when it's in
    /// the buffer, or the text to search for. A line out of range does nothing,
    /// so the cast stays open showing why.
    fn submit(&self) -> Option<Picked> {
        match self.mode() {
            Mode::Files | Mode::Commands => match self.item(self.selected)? {
                Item::File(path) => Some(Picked::Open(path.to_string())),
                Item::Command(command) => Some(Picked::Run(command.action)),
            },
            Mode::Line => match self.line_asked() {
                LineAsked::Go(line) => Some(Picked::Line(line)),
                LineAsked::Empty | LineAsked::OutOfRange | LineAsked::NotANumber => None,
            },
            Mode::Text => Some(Picked::Search(self.needle().to_string())),
        }
    }

    /// Scores every file and command against the query. An empty query lists
    /// them in the order given; otherwise the best score comes first, files
    /// tying going to the shorter path, as it's the more likely one to have been
    /// meant, and commands tying keeping their order.
    fn refilter(&mut self) {
        self.selected = 0;
        let mode = self.mode();
        let needle = self.needle().to_string();
        let pattern = Pattern::parse(&needle, CaseMatching::Smart, Normalization::Smart);
        let sorted = !needle.trim().is_empty();
        let mut chars = Vec::new();

        self.command_matches = if matches!(mode, Mode::Files | Mode::Commands) {
            let mut found: Vec<Match> = self
                .commands
                .iter()
                .enumerate()
                .filter_map(|(item, command)| {
                    let score = pattern.score(
                        Utf32Str::new(command.title, &mut chars),
                        &mut self.command_matcher,
                    )?;
                    Some(Match { item, score })
                })
                .collect();
            if sorted {
                // Stable, so ties keep the commands' order.
                found.sort_by_key(|m| std::cmp::Reverse(m.score));
            }
            found
        } else {
            Vec::new()
        };

        let Some(files) = &self.files else {
            return;
        };
        if mode != Mode::Files {
            self.matches.clear();
            return;
        }
        self.matches = files
            .iter()
            .enumerate()
            .filter_map(|(item, path)| {
                let score = pattern.score(Utf32Str::new(path, &mut chars), &mut self.matcher)?;
                Some(Match { item, score })
            })
            .collect();
        if sorted {
            self.matches.sort_by(|a, b| {
                b.score
                    .cmp(&a.score)
                    .then_with(|| files[a.item].len().cmp(&files[b.item].len()))
                    .then_with(|| files[a.item].cmp(&files[b.item]))
            });
        }
    }

    /// The char indices in `text` of the letters the query matched, sorted.
    fn matched(&self, text: &str, matcher: &mut Matcher, indices: &mut Vec<u32>) {
        let pattern = Pattern::parse(self.needle(), CaseMatching::Smart, Normalization::Smart);
        let mut chars = Vec::new();
        indices.clear();
        pattern.indices(Utf32Str::new(text, &mut chars), matcher, indices);
        indices.sort_unstable();
        indices.dedup();
    }

    /// Where the picker is drawn in `area`, so a click outside it can close it.
    pub fn card(&self, area: Rect) -> Rect {
        if self.choices.is_some() {
            Self::choices_card(area)
        } else {
            self.cast_card(area)
        }
    }

    /// Draws the picker over `area` and returns where the cursor goes, in the
    /// query. `hovered` is the screen row the pointer is on, inside the card
    /// (`kit::row_under`); the choice drawn there is lit.
    pub fn render(
        &self,
        look: Look,
        theme: &Theme,
        frame: &mut Frame,
        area: Rect,
        hovered: Option<u16>,
    ) -> (u16, u16) {
        match &self.choices {
            Some(choices) => self.render_choices(look, theme, frame, (area, hovered), choices),
            None => self.render_cast(look, theme, frame, area, hovered),
        }
    }

    /// The cast's sections, top to bottom, for the mode the query picks.
    fn cast_sections(&self) -> Vec<Section> {
        // One row for a message when there's nothing to list.
        let rows = |listed: usize, most: usize| listed.clamp(1, most);
        match self.mode() {
            Mode::Files => {
                let mut sections = vec![Section::Files(rows(self.matches.len(), CAST_ROWS))];
                let commands = self.commands_listed();
                if commands > 0 {
                    sections.push(Section::Commands(commands));
                }
                sections
            }
            Mode::Commands => vec![Section::Commands(rows(
                self.command_matches.len(),
                CAST_COMMAND_ROWS,
            ))],
            Mode::Line => vec![Section::Line],
            Mode::Text => vec![Section::Text],
        }
    }

    /// The cast card in `area`: 86 wide or the screen less four, centred, from
    /// row 4, as tall as its sections need (design README §3).
    fn cast_card(&self, area: Rect) -> Rect {
        let width = CAST_WIDTH.min(area.width.saturating_sub(4));
        let y = area.y + CAST_TOP.min(area.height);
        let height = (CHROME + sections_height(&self.cast_sections())).min(area.bottom() - y);
        Rect {
            x: area.x + (area.width - width) / 2,
            y,
            width,
            height,
        }
    }

    /// Dims `area` and draws the cast over it as a panel (tinkery-forge spec
    /// F15): its title, for the mode, in the edge; `✦` and the query on a
    /// strip, with the palette's name and what it lists at the right; each
    /// section under its heading, the selected row a pill; then the keys, on
    /// the row above the bottom edge.
    fn render_cast(
        &self,
        look: Look,
        theme: &Theme,
        frame: &mut Frame,
        area: Rect,
        hovered: Option<u16>,
    ) -> (u16, u16) {
        let out = frame.buffer_mut();
        kit::dim(theme, out, area);
        let card = self.cast_card(area);
        let mode = self.mode();
        let inside = kit::panel_at(look, theme, out, card, mode.title());
        let sections = self.cast_sections();
        if inside.is_empty() || card.height < CHROME + sections_height(&sections) || card.width < 40
        {
            return (card.x, card.y);
        }
        let row = Row {
            look,
            theme,
            card,
            hovered,
        };

        let quiet = kit::ink(theme, theme.muted, theme.raised2);
        let scope = vec![
            Span::styled(CAST_NAME, Style::new().fg(theme.accent)),
            Span::styled(mode.scope(), Style::new().fg(quiet)),
        ];
        // The field ends two blanks short of the palette's name, so a long
        // query never runs into it.
        let field = kit::header(look, theme, out, card, scope);
        // A line that can't be gone to shows in `err` where it's typed too.
        let bad_line = mode == Mode::Line
            && matches!(
                self.line_asked(),
                LineAsked::OutOfRange | LineAsked::NotANumber
            );
        let query_fg = if bad_line { theme.err } else { theme.strong };
        let query = Style::new().fg(query_fg).bg(theme.raised2);
        let cursor = self.query.render_value(out, field, query);

        let heading = Style::new()
            .fg(theme.muted)
            .bg(theme.raised)
            .add_modifier(Modifier::BOLD);
        let mut y = card.y + 4;
        for (i, section) in sections.iter().enumerate() {
            if i > 0 {
                y += 1;
            }
            out.set_string(row.left(), y, section.header(), heading);
            y += 1;
            match *section {
                Section::Files(rows) => self.render_files(out, &row, y, rows),
                Section::Commands(rows) => self.render_commands(out, &row, y, rows),
                Section::Line => self.render_line(out, &row, y),
                Section::Text => self.render_text(out, &row, y),
            }
            y += section.rows();
        }

        let keys = [("↑ ↓", "move"), ("⏎", mode.enter()), ("Esc", "close")];
        let all: Vec<(&str, &str)> = CAST_PREFIXES.iter().chain(&keys).copied().collect();
        let room = row.right().saturating_sub(row.left());
        let mut hints = kit::hints(look, theme, &all, theme.raised);
        // A card too narrow for all of it keeps the keys, which end the line.
        if kit::width(&hints) > room {
            hints = kit::hints(look, theme, &keys, theme.raised);
        }
        kit::put(out, row.left(), y + 1, &hints, row.right());
        cursor
    }

    /// Up to `rows` files from row `y`, scrolled just far enough to keep a
    /// selected file on the last row.
    fn render_files(&self, out: &mut Buffer, row: &Row, y: u16, rows: usize) {
        let Some(files) = &self.files else {
            out.set_string(row.left(), y, "listing files…", row.muted());
            return;
        };
        if self.matches.is_empty() {
            out.set_string(row.left(), y, self.no_match, row.muted());
            return;
        }
        let last = self.selected.min(self.matches.len() - 1);
        let first = last.saturating_sub(rows - 1);
        let mut matcher = self.matcher.clone();
        let mut indices = Vec::new();
        for (line, (index, m)) in self
            .matches
            .iter()
            .enumerate()
            .skip(first)
            .take(rows)
            .enumerate()
        {
            let path = files[m.item].as_str();
            self.matched(path, &mut matcher, &mut indices);
            let y = y + u16::try_from(line).unwrap_or(u16::MAX);
            let ink = row.ink(out, y, index == self.selected);
            cast_file(out, row.text(y), path, &indices, ink);
        }
    }

    /// Up to `rows` commands from row `y`: the name with its matched letters
    /// lit and the key right-aligned, scrolled just far enough to keep a
    /// selected command on the last row.
    fn render_commands(&self, out: &mut Buffer, row: &Row, y: u16, rows: usize) {
        if self.command_matches.is_empty() {
            out.set_string(row.left(), y, "no matching commands", row.muted());
            return;
        }
        let before = self.files_listed();
        let first = self
            .selected
            .checked_sub(before)
            .map_or(0, |at| at.saturating_sub(rows - 1));
        let mut matcher = self.command_matcher.clone();
        let mut indices = Vec::new();
        for (line, (index, m)) in self
            .command_matches
            .iter()
            .enumerate()
            .skip(first)
            .take(rows)
            .enumerate()
        {
            let Some(command) = self.commands.get(m.item) else {
                continue;
            };
            let y = y + u16::try_from(line).unwrap_or(u16::MAX);
            let ink = row.ink(out, y, before + index == self.selected);
            let mut text_end = row.right();
            if let Some(key) = &command.key {
                let key_x = row.right().saturating_sub(width_of(key));
                out.set_string(key_x, y, key, ink.faint);
                text_end = key_x.saturating_sub(2);
            }
            self.matched(command.title, &mut matcher, &mut indices);
            lit_text(out, (row.left(), y), text_end, command.title, &indices, ink);
        }
    }

    /// The row under `LINE`: a hint until a number is typed, then the line to go
    /// to as the selected row, or the value in `err` with why it can't be gone
    /// to.
    fn render_line(&self, out: &mut Buffer, row: &Row, y: u16) {
        let theme = row.theme;
        let range = format!("1–{}", self.lines);
        let left = row.left();
        let room = usize::from(row.right().saturating_sub(left));
        match self.line_asked() {
            LineAsked::Empty => {
                let hint = format!("type a line number, {range}");
                out.set_stringn(left, y, hint, room, row.muted());
            }
            LineAsked::Go(line) => {
                let ink = row.ink(out, y, true);
                let go = format!("Go to line {line}");
                let (x, _) = out.set_stringn(left, y, go, room, ink.label);
                let rest = room.saturating_sub(usize::from(x - left));
                let of = format!("  of {}", self.lines);
                out.set_stringn(x, y, of, rest, ink.faint);
            }
            asked @ (LineAsked::OutOfRange | LineAsked::NotANumber) => {
                let value = self.needle().trim();
                let err = Style::new().fg(theme.err).bg(theme.raised);
                let (x, _) = out.set_stringn(left, y, value, room, err);
                let why = if asked == LineAsked::OutOfRange {
                    format!("  is out of range: lines {range}")
                } else {
                    format!("  is not a line number: lines {range}")
                };
                let rest = room.saturating_sub(usize::from(x - left));
                out.set_stringn(x, y, why, rest, row.muted());
            }
        }
    }

    /// The row under `TEXT`: a hint until something is typed, then the search
    /// Enter starts as the selected row.
    fn render_text(&self, out: &mut Buffer, row: &Row, y: u16) {
        let left = row.left();
        let room = usize::from(row.right().saturating_sub(left));
        let text = self.needle();
        if text.is_empty() {
            let hint = "type text to search the project for";
            out.set_stringn(left, y, hint, room, row.muted());
            return;
        }
        let ink = row.ink(out, y, true);
        let (x, _) = out.set_stringn(left, y, "Search the project for ", room, ink.label);
        let rest = room.saturating_sub(usize::from(x - left));
        out.set_stringn(x, y, text, rest, ink.hit);
    }

    /// Where the card for a list of choices goes in `area`: as wide as go to
    /// file's was, tall enough for five choices, centred.
    fn choices_card(area: Rect) -> Rect {
        let width = (area.width * 3 / 5).clamp(40, MAX_WIDTH).min(area.width);
        let height = (CHROME + CHOICES_ROWS).min(area.height.saturating_sub(4));
        Rect {
            x: area.x + (area.width - width) / 2,
            y: area.y + (area.height - height) / 2,
            width,
            height,
        }
    }

    /// Dims `area` and draws a list of choices over it as a panel (tinkery-forge
    /// spec F15): its title in the edge; `✦` and the query on a strip, with
    /// how many match at the right; one row per match (its name with the
    /// matched letters in the accent, the detail and, right-aligned, the
    /// source), the selected one a pill; then the keys. Returns where the
    /// cursor goes.
    fn render_choices(
        &self,
        look: Look,
        theme: &Theme,
        frame: &mut Frame,
        (area, hovered): (Rect, Option<u16>),
        choices: &[Choice],
    ) -> (u16, u16) {
        let out = frame.buffer_mut();
        kit::dim(theme, out, area);
        let card = Self::choices_card(area);
        let inside = kit::panel_at(look, theme, out, card, self.query.label);
        // The chrome and one choice.
        if inside.is_empty() || card.height <= CHROME || card.width < 20 {
            return (card.x, card.y);
        }
        let row = Row {
            look,
            theme,
            card,
            hovered,
        };

        let total = self.files.as_ref().map_or(0, Vec::len);
        let quiet = kit::ink(theme, theme.muted, theme.raised2);
        let count = Span::styled(
            format!("{} of {total}", self.matches.len()),
            Style::new().fg(quiet),
        );
        let field = kit::header(look, theme, out, card, vec![count]);
        let query = Style::new().fg(theme.strong).bg(theme.raised2);
        let cursor = self.query.render_value(out, field, query);

        let keys = [("↑ ↓", "select"), ("⏎", "run"), ("Esc", "close")];
        let hints = kit::hints(look, theme, &keys, theme.raised);
        kit::put(out, row.left(), card.bottom() - 2, &hints, row.right());

        let list_y = card.y + 4;
        if self.matches.is_empty() {
            let room = usize::from(row.right().saturating_sub(row.left()));
            out.set_stringn(row.left(), list_y, self.no_match, room, row.muted());
            return cursor;
        }
        let rows = usize::from(card.height - CHROME);
        // Scrolls just far enough to keep the selection on the last row.
        let first = self.selected.saturating_sub(rows.saturating_sub(1));
        let mut matcher = self.matcher.clone();
        let mut indices = Vec::new();
        for (line, (index, m)) in self
            .matches
            .iter()
            .enumerate()
            .skip(first)
            .take(rows)
            .enumerate()
        {
            let Some(choice) = choices.get(m.item) else {
                continue;
            };
            let y = list_y + u16::try_from(line).unwrap_or(u16::MAX);
            let ink = row.ink(out, y, index == self.selected);

            let source_x = row.right().saturating_sub(width_of(choice.source));
            out.set_string(source_x, y, choice.source, ink.faint);
            let text_end = source_x.saturating_sub(2);

            self.matched(&choice.name, &mut matcher, &mut indices);
            let x = lit_text(out, (row.left(), y), text_end, &choice.name, &indices, ink);
            // A detected command is named after itself; saying it twice tells
            // nothing.
            if choice.detail != choice.name {
                let detail_x = (row.left() + DETAIL_X).max(x + 2);
                let room = usize::from(text_end.saturating_sub(detail_x));
                out.set_stringn(detail_x, y, &choice.detail, room, ink.faint);
            }
        }
        cursor
    }
}

/// How many cells `text` takes.
fn width_of(text: &str) -> u16 {
    u16::try_from(text.width()).unwrap_or(u16::MAX)
}

impl Mode {
    /// The cast's title, in its panel's edge.
    fn title(self) -> &'static str {
        match self {
            Mode::Files => "Go to file",
            Mode::Commands => "Commands",
            Mode::Line => "Go to line",
            Mode::Text => "Search",
        }
    }

    /// What the cast lists, after its name at the right of the query.
    fn scope(self) -> &'static str {
        match self {
            Mode::Files => " · files · commands",
            Mode::Commands => " · commands",
            Mode::Line => " · line",
            Mode::Text => " · text",
        }
    }

    /// What Enter does, in the hints.
    fn enter(self) -> &'static str {
        match self {
            Mode::Files => "open",
            Mode::Commands => "run",
            Mode::Line => "go",
            Mode::Text => "search",
        }
    }
}

/// One part of the cast, under its header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    /// This many file rows.
    Files(usize),
    /// This many command rows.
    Commands(usize),
    Line,
    Text,
}

impl Section {
    fn header(self) -> &'static str {
        match self {
            Section::Files(_) => "FILES",
            Section::Commands(_) => "COMMANDS",
            Section::Line => "LINE",
            Section::Text => "TEXT",
        }
    }

    fn rows(self) -> u16 {
        match self {
            Section::Files(rows) | Section::Commands(rows) => u16::try_from(rows).unwrap_or(1),
            Section::Line | Section::Text => 1,
        }
    }
}

/// How many rows `sections` take: each its header and its rows, with a blank
/// between two.
fn sections_height(sections: &[Section]) -> u16 {
    let rows: u16 = sections.iter().map(|s| 1 + s.rows()).sum();
    rows + u16::try_from(sections.len().saturating_sub(1)).unwrap_or(0)
}

/// Where the rows of a list go in `card`, and how they're drawn.
struct Row<'a> {
    look: Look,
    theme: &'a Theme,
    card: Rect,
    /// The screen row the pointer is on.
    hovered: Option<u16>,
}

impl Row<'_> {
    /// Where a row's text starts: four cells in, after the selection's mark.
    fn left(&self) -> u16 {
        self.card.x + 4
    }

    /// Where a row's text ends: four cells in from the card's right edge.
    fn right(&self) -> u16 {
        self.card.right().saturating_sub(4)
    }

    /// The cells of row `y` its text may take.
    fn text(&self, y: u16) -> Rect {
        Rect::new(self.left(), y, self.right().saturating_sub(self.left()), 1)
    }

    /// What a row that isn't a choice says (nothing matches, a hint) is in.
    fn muted(&self) -> Style {
        Style::new().fg(self.theme.muted).bg(self.theme.raised)
    }

    /// Draws row `y` as the one Enter acts on when `selected`, lit when the
    /// pointer is on it, and returns the styles its text goes in.
    fn ink(&self, out: &mut Buffer, y: u16, selected: bool) -> RowInk {
        let row = Rect::new(self.card.x + 1, y, self.card.width.saturating_sub(2), 1);
        kit::sel_row(
            self.look,
            self.theme,
            out,
            row,
            selected,
            self.hovered == Some(y),
            self.theme.raised,
        )
    }
}

/// Draws `text` from `at`, stopping before `end`: the letters whose char
/// indices are in `matched` (sorted) in `ink.hit`, the rest in `ink.label`.
/// Returns the column after the last letter drawn.
fn lit_text(
    out: &mut Buffer,
    (x, y): (u16, u16),
    end: u16,
    text: &str,
    matched: &[u32],
    ink: RowInk,
) -> u16 {
    let mut x = x;
    for (at, ch) in text.chars().enumerate() {
        let cell = ch.to_string();
        let cells = width_of(&cell);
        if x + cells > end {
            break;
        }
        let hit = u32::try_from(at).is_ok_and(|at| matched.binary_search(&at).is_ok());
        out.set_string(x, y, &cell, if hit { ink.hit } else { ink.label });
        x += cells;
    }
    x
}

/// One file row of the cast in `room`: the file's name, its matched letters
/// `ink.hit` and the rest `ink.label`, then two blanks and its folder in
/// `ink.faint`: the design lights only the name, so a query that hits the
/// folder (`src`, `util`) doesn't paint the folder of every row. `matched`
/// holds the char indices of the matched letters in `path`, sorted.
fn cast_file(out: &mut Buffer, room: Rect, path: &str, matched: &[u32], ink: RowInk) {
    let chars: Vec<char> = path.chars().collect();
    // The name starts after the last `/`; a file at the root has no folder.
    let base = chars.iter().rposition(|&c| c == '/').map_or(0, |at| at + 1);
    // Draws the char at `at` at `x` and moves `x` past it; false once it
    // wouldn't fit.
    let mut put = |x: &mut u16, at: usize, hit: Style, rest: Style| -> bool {
        let cell = chars[at].to_string();
        let cells = width_of(&cell);
        if *x + cells > room.right() {
            return false;
        }
        let is_hit = u32::try_from(at).is_ok_and(|at| matched.binary_search(&at).is_ok());
        out.set_string(*x, room.y, &cell, if is_hit { hit } else { rest });
        *x += cells;
        true
    };
    let mut x = room.x;
    for at in base..chars.len() {
        if !put(&mut x, at, ink.hit, ink.label) {
            return;
        }
    }
    // Two blanks, then the folder without the `/` that ends it.
    x += 2;
    for at in 0..base.saturating_sub(1) {
        if !put(&mut x, at, ink.faint, ink.faint) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use crate::theme::mix;
    use crate::ui::kit::{CAP_L, CAP_R};

    fn picker(files: &[&str]) -> Picker {
        let mut picker = Picker::cast(Vec::new(), 1, "");
        picker.set_files(files.iter().map(|f| f.to_string()).collect());
        picker
    }

    fn type_query(picker: &mut Picker, text: &str) {
        for c in text.chars() {
            picker.handle(Input::Text(c));
        }
    }

    const FILES: &[&str] = &[
        "README.md",
        "docs/guide.md",
        "src/domain.rs",
        "src/main.rs",
        "src/util/helpers.rs",
    ];

    #[test]
    fn an_empty_query_lists_every_file_in_order() {
        let p = picker(FILES);
        assert_eq!(p.matches().collect::<Vec<_>>(), FILES);
        assert_eq!(p.selected(), Some("README.md"));
    }

    #[test]
    fn typing_filters_fuzzily_with_the_best_match_first() {
        let mut p = picker(FILES);
        type_query(&mut p, "main");
        let matches: Vec<_> = p.matches().collect();
        assert_eq!(matches.first(), Some(&"src/main.rs"));
        assert!(!matches.contains(&"README.md"));
        type_query(&mut p, "x");
        assert_eq!(p.matches().count(), 0);
        p.handle(Input::Action(Action::Backspace));
        assert_eq!(p.query.text(), "main");
        assert_eq!(p.matches().next(), Some("src/main.rs"));
    }

    #[test]
    fn arrows_move_within_the_list_and_enter_opens_the_selection() {
        let mut p = picker(FILES);
        p.handle(Input::Action(Action::Move(Motion::Up)));
        assert_eq!(p.selected(), Some("README.md"));
        for _ in 0..10 {
            p.handle(Input::Action(Action::Move(Motion::Down)));
        }
        assert_eq!(p.selected(), Some("src/util/helpers.rs"));
        p.handle(Input::Action(Action::Move(Motion::Up)));
        assert_eq!(
            p.handle(Input::Action(Action::Newline)),
            Some(Picked::Open("src/main.rs".into()))
        );
        assert_eq!(p.handle(Input::Action(Action::Cancel)), Some(Picked::Close));
    }

    #[test]
    fn enter_with_nothing_matched_or_listed_does_nothing() {
        let mut p = Picker::cast(Vec::new(), 1, "");
        assert_eq!(p.handle(Input::Action(Action::Newline)), None);
        let mut p = picker(FILES);
        type_query(&mut p, "zzz");
        assert_eq!(p.handle(Input::Action(Action::Newline)), None);
    }

    fn run_choices() -> Vec<Choice> {
        [("dev", "npm run dev"), ("test", "cargo test")]
            .map(|(name, detail)| Choice {
                name: name.into(),
                detail: detail.into(),
                source: ".tinkery.toml",
            })
            .to_vec()
    }

    /// What a hint line reads with round ends.
    fn hints_text(pairs: &[(&str, &str)]) -> String {
        pairs
            .iter()
            .map(|(key, what)| format!("{CAP_L}{key}{CAP_R} {what}"))
            .collect::<Vec<_>>()
            .join("   ")
    }

    /// Asserts row `y` of `card` is the selected row: a pill of `hov` on
    /// `raised` across the card's inside, with `›` in the accent, bold.
    fn assert_selected(buffer: &Buffer, theme: &Theme, card: Rect, y: u16) {
        let (left, right) = (&buffer[(card.x + 1, y)], &buffer[(card.right() - 2, y)]);
        assert_eq!((left.symbol(), right.symbol()), (CAP_L, CAP_R), "row {y}");
        assert_eq!((left.fg, left.bg), (theme.hov, theme.raised), "row {y}");
        assert_eq!((right.fg, right.bg), (theme.hov, theme.raised), "row {y}");
        for x in card.x + 2..card.right() - 2 {
            assert_eq!(buffer[(x, y)].bg, theme.hov, "{x},{y}");
        }
        let mark = &buffer[(card.x + 2, y)];
        assert_eq!(mark.symbol(), "›", "row {y}");
        assert_eq!(mark.fg, theme.accent, "row {y}");
        assert!(mark.modifier.contains(Modifier::BOLD), "row {y}");
    }

    /// Asserts row `y` of `card` isn't lit: the card's fill, no pill, no mark.
    fn assert_resting(buffer: &Buffer, theme: &Theme, card: Rect, y: u16) {
        for x in card.x + 1..card.right() - 1 {
            let cell = &buffer[(x, y)];
            assert_eq!(cell.bg, theme.raised, "{x},{y}");
            assert!(!matches!(cell.symbol(), CAP_L | CAP_R | "›"), "{x},{y}");
        }
    }

    /// Asserts `card` is drawn the new way: no lit top edge, no glow, no `⏎`
    /// at a row's right, no rule.
    fn assert_no_old_look(buffer: &Buffer, theme: &Theme, card: Rect) {
        let glow = mix(theme.raised, theme.accent, 0.3);
        for y in card.top()..card.bottom() {
            for x in card.left()..card.right() {
                let cell = &buffer[(x, y)];
                assert!(!matches!(cell.symbol(), "▀" | "─"), "{x},{y}");
                assert_ne!(cell.bg, glow, "{x},{y}");
                assert_ne!(cell.bg, theme.accent, "accent is never a fill: {x},{y}");
            }
            assert_ne!(buffer[(card.right() - 5, y)].symbol(), "⏎", "row {y}");
        }
    }

    #[test]
    fn choices_are_a_panel_with_details_sources_and_a_pill() -> anyhow::Result<()> {
        let p = Picker::choices("Run", run_choices());
        let theme = Theme::default();
        let buffer = draw(&p, &theme)?;
        // 60 wide, the chrome and five rows tall, centred.
        let card = Rect::new(20, 9, 60, 12);
        assert_eq!(Picker::choices_card(Rect::new(0, 0, 100, 30)), card);
        assert_eq!(p.card(Rect::new(0, 0, 100, 30)), card);
        // The panel's edge, with the title in it.
        let edge = card_row(&buffer, card, card.y);
        assert!(edge.starts_with(" ▁ Run ▁▁▁"), "{edge:?}");
        assert_eq!(buffer[(card.x + 1, card.y)].fg, theme.accent);
        assert_eq!(buffer[(card.x + 3, card.y)].fg, theme.accent);
        assert_eq!(buffer[(card.x, card.y + 1)].symbol(), "▕");
        assert_eq!(buffer[(card.right() - 1, card.y + 1)].symbol(), "▏");
        assert_eq!(buffer[(card.x + 1, card.bottom() - 1)].symbol(), "▔");
        // The query's strip, with how many match at its right.
        let query = card_row(&buffer, card, card.y + 2);
        assert!(query.starts_with(&format!("▕{CAP_L}  ✦ ")), "{query:?}");
        assert!(query.ends_with(&format!("2 of 2  {CAP_R}▏")), "{query:?}");
        assert_eq!(buffer[(card.x + 4, card.y + 2)].fg, theme.accent2);
        assert_eq!(buffer[(card.x + 10, card.y + 2)].bg, theme.raised2);
        assert_eq!(buffer[(card.right() - 5, card.y + 2)].fg, theme.muted);
        assert_eq!(text_of(&buffer, card, card.y + 3), "");

        let first = text_of(&buffer, card, card.y + 4);
        assert!(first.starts_with("dev         npm run dev"), "{first:?}");
        assert!(first.ends_with(".tinkery.toml"), "{first:?}");
        assert_selected(&buffer, &theme, card, card.y + 4);
        // The selected row's name is `strong` bold, and what says less `text`.
        let name = &buffer[(card.x + 4, card.y + 4)];
        assert_eq!(name.fg, theme.strong);
        assert!(name.modifier.contains(Modifier::BOLD));
        assert_eq!(buffer[(card.x + 16, card.y + 4)].fg, theme.text);
        assert_eq!(buffer[(card.right() - 5, card.y + 4)].fg, theme.text);
        // Any other row is `fg`, with its detail and source in `muted`.
        let second = text_of(&buffer, card, card.y + 5);
        assert!(second.starts_with("test        cargo test"), "{second:?}");
        assert_resting(&buffer, &theme, card, card.y + 5);
        assert_eq!(buffer[(card.x + 4, card.y + 5)].fg, theme.fg);
        assert!(buffer[(card.x + 4, card.y + 5)].modifier.is_empty());
        assert_eq!(buffer[(card.x + 16, card.y + 5)].fg, theme.muted);
        assert_eq!(buffer[(card.right() - 5, card.y + 5)].fg, theme.muted);

        // The keys, on the row above the bottom edge.
        let hints = text_of(&buffer, card, card.bottom() - 2);
        assert_eq!(
            hints,
            format!(
                "{CAP_L}↑{CAP_R} {}",
                hints_text(&[("↓", "select"), ("⏎", "run"), ("Esc", "close")])
            )
        );
        assert_eq!(text_of(&buffer, card, card.bottom() - 3), "");
        assert_no_old_look(&buffer, &theme, card);
        Ok(())
    }

    #[test]
    fn choices_scroll_past_the_fifth_row_and_say_when_none_match() -> anyhow::Result<()> {
        let choices: Vec<Choice> = (0..7)
            .map(|i| Choice {
                name: format!("job{i}"),
                detail: format!("run {i}"),
                source: "detected",
            })
            .collect();
        let mut p = Picker::choices("Run", choices);
        let theme = Theme::default();
        let card = p.card(Rect::new(0, 0, 100, 30));
        let buffer = draw(&p, &theme)?;
        assert!(text_of(&buffer, card, card.y + 8).starts_with("job4 "));
        assert_eq!(text_of(&buffer, card, card.y + 9), "");
        assert!(card_row(&buffer, card, card.y + 2).contains("7 of 7"));
        for _ in 0..6 {
            p.handle(Input::Action(Action::Move(Motion::Down)));
        }
        let buffer = draw(&p, &theme)?;
        assert!(text_of(&buffer, card, card.y + 4).starts_with("job2 "));
        assert!(text_of(&buffer, card, card.y + 8).starts_with("job6 "));
        assert_selected(&buffer, &theme, card, card.y + 8);
        assert_resting(&buffer, &theme, card, card.y + 4);

        type_query(&mut p, "zzz");
        let buffer = draw(&p, &theme)?;
        assert_eq!(text_of(&buffer, card, card.y + 4), "no match");
        assert_eq!(buffer[(card.x + 4, card.y + 4)].fg, theme.muted);
        assert_resting(&buffer, &theme, card, card.y + 4);
        assert!(card_row(&buffer, card, card.y + 2).contains("✦ zzz"));
        assert!(card_row(&buffer, card, card.y + 2).contains("0 of 7"));
        Ok(())
    }

    #[test]
    fn in_mono_the_selected_choice_is_reverse_video_with_no_colours() -> anyhow::Result<()> {
        let mut p = Picker::choices("Run", run_choices());
        type_query(&mut p, "d");
        let mono = Theme::named("mono").expect("mono exists");
        let buffer = draw(&p, &mono)?;
        let card = p.card(Rect::new(0, 0, 100, 30));
        let y = card.y + 4;
        assert_eq!(
            text_of(&buffer, card, y),
            format!("{:<12}{:<27}.tinkery.toml", "dev", "npm run dev")
        );
        // The whole inside of the card, one style from edge to edge: nothing
        // on the row has a colour of its own, the matched letter included.
        let reversed = Theme::highlight(mono.hov, mono.strong);
        for x in card.x + 1..card.right() - 1 {
            let cell = &buffer[(x, y)];
            assert!(cell.modifier.contains(Modifier::REVERSED), "{x}");
            assert_eq!((Some(cell.fg), Some(cell.bg)), (reversed.fg, reversed.bg));
            assert!(!matches!(cell.symbol(), CAP_L | CAP_R), "{x}");
        }
        assert_eq!(buffer[(card.x + 2, y)].symbol(), "›");
        assert!(!buffer[(card.x, y)].modifier.contains(Modifier::REVERSED));
        // No Nerd Font glyph anywhere, and no other row reversed.
        for row in card.top()..card.bottom() {
            let line = card_row(&buffer, card, row);
            assert!(!line.contains(CAP_L) && !line.contains(CAP_R), "{line:?}");
            if row != y {
                let cell = &buffer[(card.x + 6, row)];
                assert!(!cell.modifier.contains(Modifier::REVERSED), "row {row}");
            }
        }
        Ok(())
    }

    #[test]
    fn choices_are_listed_in_order_and_picked_by_name() {
        let mut p = Picker::choices("Run", run_choices());
        assert_eq!(p.matches().collect::<Vec<_>>(), ["dev", "test"]);
        type_query(&mut p, "te");
        assert_eq!(
            p.handle(Input::Action(Action::Newline)),
            Some(Picked::Open("test".into()))
        );
    }

    #[test]
    fn a_pasted_line_break_opens_the_best_match() {
        let mut p = picker(FILES);
        assert_eq!(
            p.paste("helpers\n"),
            Some(Picked::Open("src/util/helpers.rs".into()))
        );
    }

    #[test]
    fn the_cast_card_drops_from_row_four_and_fits_its_rows() {
        let area = Rect::new(0, 0, 160, 45);
        let mut p = Picker::cast(Vec::new(), 1, "");
        // Waiting for the walk: one row for the message.
        assert_eq!(p.card(area), Rect::new(37, 4, 86, 9));
        p.set_files(FILES.iter().map(|f| f.to_string()).collect());
        assert_eq!(p.card(area), Rect::new(37, 4, 86, 13));
        // Clamped to the screen less four at the smallest size.
        assert_eq!(p.card(Rect::new(0, 0, 88, 30)).width, 84);
    }

    #[test]
    fn the_cast_says_so_while_the_walk_is_still_listing_files() -> anyhow::Result<()> {
        // Before the walk reports back, which a PTY test can't wait for.
        let p = Picker::cast(Vec::new(), 1, "");
        let theme = Theme::default();
        let buffer = draw(&p, &theme)?;
        let card = p.card(Rect::new(0, 0, 100, 30));
        assert_eq!(text_of(&buffer, card, card.y + 5), "listing files…");
        assert_eq!(buffer[(card.x + 4, card.y + 5)].fg, theme.muted);
        // A message isn't a row to pick: it's never lit.
        assert_resting(&buffer, &theme, card, card.y + 5);
        assert!(card_row(&buffer, card, card.y).starts_with(" ▁ Go to file ▁"));
        Ok(())
    }

    #[test]
    fn a_file_row_shows_the_name_lit_then_its_folder() -> anyhow::Result<()> {
        let mut p = picker(FILES);
        type_query(&mut p, "main");
        let theme = Theme::default();
        let buffer = draw(&p, &theme)?;
        let card = p.card(Rect::new(0, 0, 100, 30));
        let row = card.y + 5;
        let line: String = (card.x..card.right())
            .map(|x| buffer[(x, row)].symbol())
            .collect();
        assert!(
            line.starts_with(&format!("▕{CAP_L}› main.rs  src ")),
            "{line:?}"
        );
        assert_selected(&buffer, &theme, card, row);
        // Accent and bold: the selection's mark and the matched letters.
        let lit: String = (card.x + 1..card.right() - 1)
            .filter(|&x| {
                let cell = &buffer[(x, row)];
                cell.fg == theme.accent && cell.modifier.contains(Modifier::BOLD)
            })
            .map(|x| buffer[(x, row)].symbol())
            .collect();
        assert_eq!(lit, "›main");
        // The rest of the name is `strong` bold, the folder `text`.
        let rest = &buffer[(card.x + 8, row)];
        assert_eq!((rest.symbol(), rest.fg), (".", theme.strong));
        assert!(rest.modifier.contains(Modifier::BOLD));
        let folder = &buffer[(card.x + 13, row)];
        assert_eq!((folder.symbol(), folder.fg), ("s", theme.text));
        assert!(!folder.modifier.contains(Modifier::BOLD));
        assert_no_old_look(&buffer, &theme, card);
        Ok(())
    }

    #[test]
    fn letters_matched_in_the_folder_are_not_lit() -> anyhow::Result<()> {
        let mut p = picker(FILES);
        type_query(&mut p, "util");
        let theme = Theme::default();
        let buffer = draw(&p, &theme)?;
        let card = p.card(Rect::new(0, 0, 100, 30));
        let row = card.y + 5;
        let line: String = (card.x..card.right())
            .map(|x| buffer[(x, row)].symbol())
            .collect();
        assert!(
            line.starts_with(&format!("▕{CAP_L}› helpers.rs  src/util ")),
            "{line:?}"
        );
        // On the selected row the folder is `text`, and never lit.
        let folder = card.x + 16..card.x + 24;
        for x in folder {
            let cell = &buffer[(x, row)];
            assert_eq!(cell.fg, theme.text, "{:?} at {x}", cell.symbol());
            assert!(!cell.modifier.contains(Modifier::BOLD));
        }
        Ok(())
    }

    fn commands() -> Vec<Command> {
        [
            (Action::Save, Some("Ctrl+S")),
            (Action::ToggleSplit, Some("Alt+V")),
            (Action::Run, Some("F5")),
            (Action::CycleFocus, None),
        ]
        .map(|(action, key)| Command {
            action,
            title: action.title(),
            key: key.map(str::to_string),
        })
        .to_vec()
    }

    fn cast(query: &str) -> Picker {
        let mut p = Picker::cast(commands(), 200, query);
        p.set_files(FILES.iter().map(|f| f.to_string()).collect());
        p
    }

    fn draw(p: &Picker, theme: &Theme) -> anyhow::Result<Buffer> {
        let mut terminal = Terminal::new(TestBackend::new(100, 30))?;
        terminal.draw(|frame| {
            p.render(Look::default(), theme, frame, frame.area(), None);
        })?;
        Ok(terminal.backend().buffer().clone())
    }

    /// What row `y` of `card` says where a row's text goes: four cells in from
    /// each edge, without the blanks around it.
    fn text_of(buffer: &Buffer, card: Rect, y: u16) -> String {
        let cells: String = (card.x + 4..card.right() - 4)
            .map(|x| buffer[(x, y)].symbol())
            .collect();
        cells.trim().to_string()
    }

    /// The text of row `y` across `card`.
    fn card_row(buffer: &Buffer, card: Rect, y: u16) -> String {
        (card.x..card.right())
            .map(|x| buffer[(x, y)].symbol())
            .collect()
    }

    #[test]
    fn with_no_prefix_two_commands_follow_the_files_and_enter_runs_one() {
        let mut p = cast("");
        // Five file rows, a blank, the header and two command rows.
        assert_eq!(p.card(Rect::new(0, 0, 160, 45)).height, 17);
        for _ in 0..FILES.len() {
            p.handle(Input::Action(Action::Move(Motion::Down)));
        }
        assert_eq!(p.selected(), Some("Save"));
        p.handle(Input::Action(Action::Move(Motion::Down)));
        assert_eq!(p.selected(), Some("Split right"));
        // Only two commands are listed, so the selection stops on the second.
        p.handle(Input::Action(Action::Move(Motion::Down)));
        assert_eq!(p.selected(), Some("Split right"));
        assert_eq!(
            p.handle(Input::Action(Action::Newline)),
            Some(Picked::Run(Action::ToggleSplit))
        );
    }

    #[test]
    fn a_command_row_shows_its_name_and_its_key_at_the_right() -> anyhow::Result<()> {
        let p = cast(">");
        let theme = Theme::default();
        let buffer = draw(&p, &theme)?;
        let card = p.card(Rect::new(0, 0, 100, 30));
        assert!(card_row(&buffer, card, card.y).starts_with(" ▁ Commands ▁"));
        assert_eq!(text_of(&buffer, card, card.y + 4), "COMMANDS");
        let heading = &buffer[(card.x + 4, card.y + 4)];
        assert_eq!(heading.fg, theme.muted);
        assert!(heading.modifier.contains(Modifier::BOLD));
        // The selected row is a pill, its key where every row's is.
        let first = card_row(&buffer, card, card.y + 5);
        assert!(first.starts_with(&format!("▕{CAP_L}› Save ")), "{first:?}");
        assert!(first.ends_with(&format!(" Ctrl+S  {CAP_R}▏")), "{first:?}");
        assert_selected(&buffer, &theme, card, card.y + 5);
        assert_eq!(buffer[(card.x + 4, card.y + 5)].fg, theme.strong);
        assert_eq!(buffer[(card.right() - 10, card.y + 5)].fg, theme.text);
        let second = card_row(&buffer, card, card.y + 6);
        assert!(second.starts_with("▕   Split right "), "{second:?}");
        assert!(second.ends_with(" Alt+V   ▏"), "{second:?}");
        assert_resting(&buffer, &theme, card, card.y + 6);
        assert_eq!(buffer[(card.x + 4, card.y + 6)].fg, theme.fg);
        assert_eq!(buffer[(card.right() - 9, card.y + 6)].fg, theme.muted);
        // A command without a key shows none.
        assert_eq!(text_of(&buffer, card, card.y + 8), "Cycle focus");
        // The prefixes and the keys, ending with the way out.
        assert_eq!(
            text_of(&buffer, card, card.bottom() - 2),
            format!(
                "{}   {CAP_L}↑{CAP_R} {}",
                hints_text(&[(">", "commands"), (":", "line"), ("/", "text")]),
                hints_text(&[("↓", "move"), ("⏎", "run"), ("Esc", "close")])
            )
        );
        assert_no_old_look(&buffer, &theme, card);
        Ok(())
    }

    #[test]
    fn a_prefix_of_gt_lists_only_commands_matched_by_name() {
        let mut p = cast(">split");
        assert_eq!(p.matches().count(), 0);
        assert_eq!(p.selected(), Some("Split right"));
        assert_eq!(
            p.handle(Input::Action(Action::Newline)),
            Some(Picked::Run(Action::ToggleSplit))
        );
        type_query(&mut p, "zzz");
        assert_eq!(p.selected(), None);
        assert_eq!(p.handle(Input::Action(Action::Newline)), None);
    }

    #[test]
    fn a_colon_goes_to_a_line_in_the_buffer_and_only_there() {
        let mut p = cast(":");
        assert_eq!(p.handle(Input::Action(Action::Newline)), None);
        type_query(&mut p, "120");
        assert_eq!(
            p.handle(Input::Action(Action::Newline)),
            Some(Picked::Line(120))
        );
        for query in [":0", ":201", ":99999999999999999999999", ":abc"] {
            let mut p = cast(query);
            assert_eq!(p.handle(Input::Action(Action::Newline)), None, "{query}");
        }
        assert_eq!(
            cast(":200").handle(Input::Action(Action::Newline)),
            Some(Picked::Line(200))
        );
    }

    #[test]
    fn a_line_out_of_range_shows_its_value_in_err() -> anyhow::Result<()> {
        let p = cast(":999");
        let theme = Theme::default();
        let buffer = draw(&p, &theme)?;
        let card = p.card(Rect::new(0, 0, 100, 30));
        assert!(card_row(&buffer, card, card.y).starts_with(" ▁ Go to line ▁"));
        assert_eq!(text_of(&buffer, card, card.y + 4), "LINE");
        assert_eq!(
            text_of(&buffer, card, card.y + 5),
            "999  is out of range: lines 1–200"
        );
        // Nothing to go to, so no row is selected.
        assert_resting(&buffer, &theme, card, card.y + 5);
        assert_eq!(buffer[(card.x + 4, card.y + 5)].fg, theme.err);
        assert_eq!(buffer[(card.x + 9, card.y + 5)].fg, theme.muted);
        // Where it's typed too, after `✦ :`.
        assert_eq!(buffer[(card.x + 7, card.y + 2)].fg, theme.err);
        assert!(card_row(&buffer, card, card.y + 2).contains("cast · line"));

        let buffer = draw(&cast(":120"), &theme)?;
        assert_eq!(text_of(&buffer, card, card.y + 5), "Go to line 120  of 200");
        assert_selected(&buffer, &theme, card, card.y + 5);
        assert_eq!(buffer[(card.x + 4, card.y + 5)].fg, theme.strong);
        assert_eq!(buffer[(card.x + 20, card.y + 5)].fg, theme.text);
        assert_eq!(buffer[(card.x + 7, card.y + 2)].fg, theme.strong);
        assert!(
            text_of(&buffer, card, card.bottom() - 2)
                .ends_with(&hints_text(&[("⏎", "go"), ("Esc", "close")]))
        );
        Ok(())
    }

    #[test]
    fn a_slash_shows_the_search_enter_starts_as_the_selected_row() -> anyhow::Result<()> {
        let theme = Theme::default();
        let p = cast("/");
        let card = p.card(Rect::new(0, 0, 100, 30));
        let buffer = draw(&p, &theme)?;
        assert!(card_row(&buffer, card, card.y).starts_with(" ▁ Search ▁"));
        assert_eq!(text_of(&buffer, card, card.y + 4), "TEXT");
        assert_eq!(
            text_of(&buffer, card, card.y + 5),
            "type text to search the project for"
        );
        assert_resting(&buffer, &theme, card, card.y + 5);

        let buffer = draw(&cast("/foo"), &theme)?;
        assert_eq!(
            text_of(&buffer, card, card.y + 5),
            "Search the project for foo"
        );
        assert_selected(&buffer, &theme, card, card.y + 5);
        let word = &buffer[(card.x + 27, card.y + 5)];
        assert_eq!((word.symbol(), word.fg), ("f", theme.accent));
        assert!(word.modifier.contains(Modifier::BOLD));
        assert!(card_row(&buffer, card, card.y + 2).contains("cast · text"));
        assert!(
            text_of(&buffer, card, card.bottom() - 2)
                .ends_with(&hints_text(&[("⏎", "search"), ("Esc", "close")]))
        );
        Ok(())
    }

    #[test]
    fn the_cast_is_a_panel_with_the_query_on_a_strip() -> anyhow::Result<()> {
        // Four files and two commands hold an `r`.
        let p = cast("r");
        let card = p.card(Rect::new(0, 0, 100, 30));
        for name in ["tinkery", "tinkery-light"] {
            let theme = Theme::named(name).expect("a listed theme exists");
            let buffer = draw(&p, &theme)?;
            // An accent edge with the title in it, around `raised`.
            let edge = card_row(&buffer, card, card.y);
            assert!(edge.starts_with(" ▁ Go to file ▁"), "{name}: {edge:?}");
            assert_eq!(buffer[(card.x + 20, card.y)].fg, theme.accent, "{name}");
            assert_eq!(buffer[(card.x + 20, card.y + 1)].bg, theme.raised, "{name}");
            assert_eq!(text_of(&buffer, card, card.y + 1), "", "{name}");
            // The strip: `raised2` between two ends, across the inside.
            let y = card.y + 2;
            let query = card_row(&buffer, card, y);
            assert!(
                query.starts_with(&format!("▕{CAP_L}  ✦ r ")),
                "{name}: {query:?}"
            );
            assert!(
                query.ends_with(&format!("cast · files · commands  {CAP_R}▏")),
                "{name}: {query:?}"
            );
            let cap = &buffer[(card.x + 1, y)];
            assert_eq!((cap.fg, cap.bg), (theme.raised2, theme.raised), "{name}");
            for x in card.x + 2..card.right() - 2 {
                assert_eq!(buffer[(x, y)].bg, theme.raised2, "{name}: {x}");
            }
            assert_eq!(buffer[(card.x + 4, y)].fg, theme.accent2, "{name}");
            assert_eq!(buffer[(card.x + 6, y)].fg, theme.strong, "{name}");
            assert_eq!(buffer[(card.right() - 27, y)].fg, theme.accent, "{name}");
            assert_eq!(buffer[(card.right() - 5, y)].fg, theme.muted, "{name}");
            // The row under it is blank, where the rule was.
            assert_eq!(text_of(&buffer, card, card.y + 3), "", "{name}");
            // Section headings in `muted` bold.
            for (row, heading) in [(card.y + 4, "FILES"), (card.y + 10, "COMMANDS")] {
                assert_eq!(text_of(&buffer, card, row), heading, "{name}");
                let cell = &buffer[(card.x + 4, row)];
                assert_eq!(cell.fg, theme.muted, "{name}");
                assert!(cell.modifier.contains(Modifier::BOLD), "{name}");
            }
            assert_selected(&buffer, &theme, card, card.y + 5);
            assert_no_old_look(&buffer, &theme, card);
            // The keys, on the row above the bottom edge.
            assert_eq!(
                text_of(&buffer, card, card.bottom() - 2),
                format!(
                    "{}   {CAP_L}↑{CAP_R} {}",
                    hints_text(&[(">", "commands"), (":", "line"), ("/", "text")]),
                    hints_text(&[("↓", "move"), ("⏎", "open"), ("Esc", "close")])
                ),
                "{name}"
            );
            let key = &buffer[(card.x + 5, card.bottom() - 2)];
            assert_eq!((key.fg, key.bg), (theme.strong, theme.btn), "{name}");
            assert_eq!(buffer[(card.x + 1, card.bottom() - 1)].symbol(), "▔");
        }
        Ok(())
    }

    #[test]
    fn a_narrow_cast_keeps_the_keys_that_end_its_hints() -> anyhow::Result<()> {
        let p = cast("");
        let theme = Theme::default();
        let mut terminal = Terminal::new(TestBackend::new(60, 30))?;
        terminal.draw(|frame| {
            p.render(Look::default(), &theme, frame, frame.area(), None);
        })?;
        let buffer = terminal.backend().buffer();
        let card = p.card(Rect::new(0, 0, 60, 30));
        assert_eq!(card.width, 56);
        assert_eq!(
            text_of(buffer, card, card.bottom() - 2),
            format!(
                "{CAP_L}↑{CAP_R} {}",
                hints_text(&[("↓", "move"), ("⏎", "open"), ("Esc", "close")])
            )
        );
        Ok(())
    }

    #[test]
    fn the_selection_moves_its_pill_from_files_into_commands() -> anyhow::Result<()> {
        let mut p = cast("");
        let theme = Theme::default();
        let card = p.card(Rect::new(0, 0, 100, 30));
        let buffer = draw(&p, &theme)?;
        assert_selected(&buffer, &theme, card, card.y + 5);
        for y in card.y + 6..card.y + 10 {
            assert_resting(&buffer, &theme, card, y);
        }
        for _ in 0..FILES.len() {
            p.handle(Input::Action(Action::Move(Motion::Down)));
        }
        let buffer = draw(&p, &theme)?;
        // Five file rows, a blank and `COMMANDS`, then the selected command.
        assert_eq!(text_of(&buffer, card, card.y + 11), "COMMANDS");
        assert_selected(&buffer, &theme, card, card.y + 12);
        assert!(text_of(&buffer, card, card.y + 12).starts_with("Save "));
        for y in card.y + 5..card.y + 10 {
            assert_resting(&buffer, &theme, card, y);
        }
        assert_resting(&buffer, &theme, card, card.y + 13);
        Ok(())
    }

    #[test]
    fn in_mono_the_selected_cast_row_is_reverse_video_with_no_colours() -> anyhow::Result<()> {
        let mono = Theme::named("mono").expect("mono exists");
        let reversed = Theme::highlight(mono.hov, mono.strong);
        for query in ["main", ">sav", ":120", "/foo"] {
            let p = cast(query);
            let card = p.card(Rect::new(0, 0, 100, 30));
            let buffer = draw(&p, &mono)?;
            let y = card.y + 5;
            for x in card.x + 1..card.right() - 1 {
                let cell = &buffer[(x, y)];
                assert!(cell.modifier.contains(Modifier::REVERSED), "{query}: {x}");
                assert_eq!(
                    (Some(cell.fg), Some(cell.bg)),
                    (reversed.fg, reversed.bg),
                    "{query}: {x}"
                );
            }
            assert_eq!(buffer[(card.x + 2, y)].symbol(), "›", "{query}");
            // Terminal colours only, and no Nerd Font glyph.
            for row in card.top()..card.bottom() {
                for x in card.left()..card.right() {
                    let cell = &buffer[(x, row)];
                    let rgb = |c| matches!(c, ratatui::style::Color::Rgb(..));
                    assert!(!rgb(cell.fg) && !rgb(cell.bg), "{query}: {x},{row}");
                    assert!(!matches!(cell.symbol(), CAP_L | CAP_R), "{query}");
                    if row != y {
                        let on = cell.modifier.contains(Modifier::REVERSED);
                        assert!(!on, "{query}: {x},{row}");
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn a_slash_searches_the_project_for_the_rest() {
        let mut p = cast("/");
        type_query(&mut p, "foo");
        assert_eq!(
            p.handle(Input::Action(Action::Newline)),
            Some(Picked::Search("foo".into()))
        );
        // Deleting the prefix lists files again.
        let mut p = cast("/");
        assert_eq!(p.matches().count(), 0);
        p.handle(Input::Action(Action::Backspace));
        assert_eq!(p.matches().count(), FILES.len());
    }
}
