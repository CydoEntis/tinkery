//! The folder browser `>open directory` opens (tinkery-splash spec S6): a card in
//! the cast's style listing one folder's sub-folders, to walk the disk and pick
//! a folder to open as the project.

use std::io;
use std::path::{Component, Path, PathBuf};

use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Matcher, Utf32Str};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use crate::buffer::movement::Motion;
use crate::keymap::{Action, Input};
use crate::theme::Theme;
use crate::ui::kit::{self, Look, RowInk};
use crate::ui::prompt::{Outcome, PromptBar};

/// The card's width before clamping to the screen, and the row it drops from:
/// the cast palette's, so the browser reads as the palette turning into it.
const WIDTH: u16 = 86;
const TOP: u16 = 4;
/// Most folder rows shown; the selection scrolls through the rest.
const FOLDER_ROWS: usize = 10;
/// The rows other than the folders: the edge, a blank, the header, a blank
/// and the open row above them; a blank, the hints and the edge below.
const CHROME: u16 = 8;
/// Right of the header: what Enter does here and what's listed.
const SCOPE: &str = "open · folders";
/// The keys, on the panel's last row.
const HINTS: [(&str, &str); 4] = [
    ("⏎", "into"),
    ("←", "up"),
    ("Ctrl+⏎", "open here"),
    ("Esc", "close"),
];

/// What a key did to the browser, when it did more than move or filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Browsed {
    /// Open this folder as the project.
    Open(PathBuf),
    /// A folder couldn't be read or a typed path names none; the browser stays
    /// where it was and the status line says this.
    Failed(String),
    /// Esc: close and change nothing.
    Close,
}

#[derive(Debug, Clone)]
pub struct DirPicker {
    /// The folder shown, absolute.
    dir: PathBuf,
    /// Its sub-folders' names, in `list_folders` order.
    folders: Vec<String>,
    query: PromptBar,
    /// Indices into `folders` the query matches, best first.
    matches: Vec<usize>,
    /// 0 is the open row; `n` is `matches[n - 1]`.
    selected: usize,
    matcher: Matcher,
}

impl DirPicker {
    /// The browser on `dir`, or why `dir` can't be listed.
    pub fn new(dir: &Path) -> Result<Self, String> {
        let dir = clean(dir);
        let folders = list_folders(&dir).map_err(|err| cannot_open(&dir, &err))?;
        let mut picker = DirPicker {
            dir,
            folders,
            query: PromptBar::new("Open directory", ""),
            matches: Vec::new(),
            selected: 0,
            matcher: Matcher::new(Config::DEFAULT),
        };
        picker.refilter();
        Ok(picker)
    }

    /// The folder shown.
    #[cfg(test)]
    fn dir(&self) -> &Path {
        &self.dir
    }

    /// The folder names listed, best match first.
    #[cfg(test)]
    fn listed(&self) -> Vec<&str> {
        self.matches
            .iter()
            .map(|&i| self.folders[i].as_str())
            .collect()
    }

    /// Up and Down move through the open row and the folders; Enter acts on
    /// the selection or jumps to a typed path; ← or Backspace with nothing typed
    /// go up; Esc closes; everything else edits the query.
    pub fn handle(&mut self, input: Input) -> Option<Browsed> {
        let empty = self.query.text().is_empty();
        match input {
            Input::Action(Action::Move(Motion::Up)) => {
                self.selected = self.selected.saturating_sub(1);
                None
            }
            Input::Action(Action::Move(Motion::Down)) => {
                self.selected = (self.selected + 1).min(self.matches.len());
                None
            }
            Input::Action(Action::Move(Motion::Left) | Action::Backspace) if empty => self.up(),
            Input::Action(Action::OpenFolderHere) => Some(Browsed::Open(self.dir.clone())),
            _ => {
                let before = self.query.text().to_string();
                let outcome = self.query.handle(input);
                self.after_edit(&before, outcome)
            }
        }
    }

    /// Pasted text goes into the query as if typed; a line break is Enter.
    pub fn paste(&mut self, text: &str) -> Option<Browsed> {
        let before = self.query.text().to_string();
        let outcome = self.query.paste(text);
        self.after_edit(&before, outcome)
    }

    fn after_edit(&mut self, before: &str, outcome: Option<Outcome>) -> Option<Browsed> {
        if self.query.text() != before {
            self.refilter();
        }
        match outcome? {
            Outcome::Submit => self.submit(),
            Outcome::Cancel => Some(Browsed::Close),
        }
    }

    /// Enter: a typed path jumps there; otherwise the open row opens the folder
    /// shown and a folder row goes into it.
    fn submit(&mut self) -> Option<Browsed> {
        let typed = self.query.text().trim().to_string();
        if looks_like_path(&typed) {
            let target = clean(&self.dir.join(&typed));
            if !target.is_dir() {
                return Some(Browsed::Failed(format!("no such folder: {typed}")));
            }
            return self.go(target, None);
        }
        match self.selected {
            0 => Some(Browsed::Open(self.dir.clone())),
            n => {
                let name = self.folders.get(*self.matches.get(n - 1)?)?.clone();
                let target = self.dir.join(name);
                self.go(target, None)
            }
        }
    }

    /// Moves to the parent folder, selecting the one just left; nothing above
    /// a drive or `/`.
    fn up(&mut self) -> Option<Browsed> {
        let parent = self.dir.parent()?.to_path_buf();
        let came_from = self
            .dir
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        self.go(parent, came_from)
    }

    /// Shows `dir` with an empty query and `select` (a folder name) selected,
    /// or the open row; a folder that can't be read leaves everything as it was.
    fn go(&mut self, dir: PathBuf, select: Option<String>) -> Option<Browsed> {
        match list_folders(&dir) {
            Ok(folders) => {
                self.dir = dir;
                self.folders = folders;
                self.query = PromptBar::new(self.query.label, "");
                self.refilter();
                if let Some(name) = select
                    && let Some(at) = self.matches.iter().position(|&i| self.folders[i] == name)
                {
                    self.selected = at + 1;
                }
                None
            }
            Err(err) => Some(Browsed::Failed(cannot_open(&dir, &err))),
        }
    }

    /// Scores every folder against the query. Nothing typed lists them all in
    /// order with the open row selected; otherwise the best match comes first,
    /// ties keeping their order, and is selected, as Enter is most likely meant
    /// to go into it.
    fn refilter(&mut self) {
        let needle = self.query.text().to_string();
        if needle.trim().is_empty() {
            self.matches = (0..self.folders.len()).collect();
            self.selected = 0;
            return;
        }
        let pattern = Pattern::parse(&needle, CaseMatching::Smart, Normalization::Smart);
        let mut chars = Vec::new();
        let mut found: Vec<(usize, u32)> = self
            .folders
            .iter()
            .enumerate()
            .filter_map(|(i, name)| {
                let score = pattern.score(Utf32Str::new(name, &mut chars), &mut self.matcher)?;
                Some((i, score))
            })
            .collect();
        // Stable, so ties keep the listing's order.
        found.sort_by_key(|&(_, score)| std::cmp::Reverse(score));
        self.matches = found.into_iter().map(|(i, _)| i).collect();
        self.selected = usize::from(!self.matches.is_empty());
    }

    /// The char indices in `name` of the letters the query matched, sorted.
    fn matched(&self, name: &str, matcher: &mut Matcher, indices: &mut Vec<u32>) {
        indices.clear();
        let needle = self.query.text();
        if needle.trim().is_empty() {
            return;
        }
        let pattern = Pattern::parse(needle, CaseMatching::Smart, Normalization::Smart);
        let mut chars = Vec::new();
        pattern.indices(Utf32Str::new(name, &mut chars), matcher, indices);
        indices.sort_unstable();
        indices.dedup();
    }

    /// How many folder rows the card has room for: one for a message when
    /// there's nothing to list.
    fn rows(&self) -> usize {
        self.matches.len().clamp(1, FOLDER_ROWS)
    }

    /// Where the card goes in `area`: the cast's width, centred, from row 4, as
    /// tall as its rows need.
    pub fn card(&self, area: Rect) -> Rect {
        let width = WIDTH.min(area.width.saturating_sub(4));
        let y = area.y + TOP.min(area.height);
        let rows = u16::try_from(self.rows()).unwrap_or(1);
        let height = (CHROME + rows).min(area.bottom() - y);
        Rect {
            x: area.x + (area.width - width) / 2,
            y,
            width,
            height,
        }
    }

    /// Dims `area` and draws the browser over it as a panel (tinkery-forge spec
    /// F15): its title in the edge; `✦`, the folder and the query on a strip,
    /// with `open · folders` at the right; the open row; the folders, the
    /// selected row a pill; then the keys, on the row above the bottom edge.
    /// Returns where the cursor goes. `hovered` is the screen row the pointer
    /// is on, inside the card (`kit::row_under`); the row drawn there is lit.
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
        let rows = self.rows();
        let full = CHROME + u16::try_from(rows).unwrap_or(1);
        if inside.is_empty() || card.height < full || card.width < 40 {
            return (card.x, card.y);
        }
        let list = List {
            look,
            theme,
            card,
            hovered,
        };

        let quiet = kit::ink(theme, theme.muted, theme.raised2);
        let scope = vec![Span::styled(SCOPE, Style::new().fg(quiet))];
        // The field ends two blanks short of the scope, so a long path never
        // runs into it.
        let field = kit::header(look, theme, out, card, scope);
        let cursor = self.render_header(theme, out, field);

        let open_y = card.y + 4;
        self.render_open_row(out, &list, open_y);
        self.render_folders(out, &list, open_y + 1, rows);

        let hints = kit::hints(look, theme, &HINTS, theme.raised);
        let hints_y = open_y + 1 + u16::try_from(rows).unwrap_or(1) + 1;
        kit::put(out, list.left(), hints_y, &hints, list.right());
        cursor
    }

    /// The folder shown in `text`, then the query in `strong`, on the header's
    /// strip; the folder is cut from the front with `…` when both don't fit. A
    /// typed absolute path stands alone, as the folder in front of it would
    /// read as part of it.
    fn render_header(&self, theme: &Theme, out: &mut Buffer, field: Rect) -> (u16, u16) {
        if field.width == 0 {
            return (field.x, field.y);
        }
        let on = Style::new().bg(theme.raised2);
        let typed = self.query.text();
        let mut x = field.x;
        if !Path::new(typed).is_absolute() {
            let mut shown = self.dir.display().to_string();
            if !shown.ends_with(std::path::MAIN_SEPARATOR) {
                shown.push(std::path::MAIN_SEPARATOR);
            }
            // The query keeps room for a few letters and the cursor.
            let query_room = typed.width() + 1;
            let room = usize::from(field.width).saturating_sub(query_room.max(8));
            let shown = cut_front(&shown, room);
            x = out
                .set_stringn(x, field.y, &shown, room, on.fg(theme.text))
                .0;
        }
        let rest = Rect::new(x, field.y, field.right().saturating_sub(x), 1);
        self.query.render_value(out, rest, on.fg(theme.strong))
    }

    /// `⏎ open <folder>`: what Enter does with nothing else picked.
    fn render_open_row(&self, out: &mut Buffer, list: &List, y: u16) {
        let left = list.left();
        let ink = list.ink(out, y, self.selected == 0);
        let end = usize::from(list.right().saturating_sub(left));
        let enter = ink.of(Style::new().fg(list.theme.accent));
        let (x, _) = out.set_stringn(left, y, "⏎ ", end, enter);
        let used = usize::from(x - left);
        let (x, _) = out.set_stringn(x, y, "open ", end.saturating_sub(used), ink.label);
        let used = usize::from(x - left);
        let path = self.dir.display().to_string();
        let room = end.saturating_sub(used);
        out.set_stringn(x, y, cut_front(&path, room), room, ink.faint);
    }

    /// Up to `rows` folders from row `y`, scrolled just far enough to keep a
    /// selected folder on the last row: `▸` then the name, its matched letters
    /// in the accent.
    fn render_folders(&self, out: &mut Buffer, list: &List, y: u16, rows: usize) {
        let left = list.left();
        if self.matches.is_empty() {
            let why = if self.folders.is_empty() {
                "no folders here"
            } else {
                "no matching folders"
            };
            let muted = Style::new().fg(list.theme.muted).bg(list.theme.raised);
            out.set_string(left, y, why, muted);
            return;
        }
        let first = self.selected.saturating_sub(1).saturating_sub(rows - 1);
        let mut matcher = self.matcher.clone();
        let mut indices = Vec::new();
        for (line, (index, &folder)) in self
            .matches
            .iter()
            .enumerate()
            .skip(first)
            .take(rows)
            .enumerate()
        {
            let row_y = y + u16::try_from(line).unwrap_or(u16::MAX);
            let ink = list.ink(out, row_y, index + 1 == self.selected);
            out.set_string(left, row_y, "▸", ink.faint);
            let name = &self.folders[folder];
            self.matched(name, &mut matcher, &mut indices);
            let mut x = left + 2;
            for (at, ch) in name.chars().enumerate() {
                let cell = ch.to_string();
                let cells = width_of(&cell);
                if x + cells > list.right() {
                    break;
                }
                let is_hit = u32::try_from(at).is_ok_and(|at| indices.binary_search(&at).is_ok());
                out.set_string(x, row_y, &cell, if is_hit { ink.hit } else { ink.label });
                x += cells;
            }
        }
    }
}

/// Where the browser's rows go in `card`, and how they're drawn.
struct List<'a> {
    look: Look,
    theme: &'a Theme,
    card: Rect,
    /// The screen row the pointer is on.
    hovered: Option<u16>,
}

impl List<'_> {
    /// Where a row's text starts: four cells in, after the selection's mark.
    fn left(&self) -> u16 {
        self.card.x + 4
    }

    /// Where a row's text ends: four cells in from the card's right edge.
    fn right(&self) -> u16 {
        self.card.right().saturating_sub(4)
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

/// The folders directly inside `dir`, by name: sorted ignoring case, those
/// starting with `.` last, as they're rarely the one wanted. Links to folders
/// count as folders. An entry that can't be read is left out; `dir` itself
/// failing is the error.
pub fn list_folders(dir: &Path) -> io::Result<Vec<String>> {
    let mut names: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort_by(|a, b| {
        a.starts_with('.')
            .cmp(&b.starts_with('.'))
            .then_with(|| a.to_lowercase().cmp(&b.to_lowercase()))
            .then_with(|| a.cmp(b))
    });
    Ok(names)
}

/// `path` made absolute with `.` and `..` worked out from the text alone, so
/// the header shows `C:\dev` rather than `C:\dev\tinkery\..` and going up from it
/// goes where it says.
pub fn clean(path: &Path) -> PathBuf {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                // A root has no parent; `..` there stays at the root.
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Whether the query names a path rather than filtering: absolute, or holding
/// a separator.
fn looks_like_path(text: &str) -> bool {
    !text.is_empty()
        && (Path::new(text).is_absolute()
            || text.contains('/')
            || text.contains(std::path::MAIN_SEPARATOR))
}

fn cannot_open(dir: &Path, err: &io::Error) -> String {
    format!("cannot open {}: {err}", dir.display())
}

/// How many cells `text` takes.
fn width_of(text: &str) -> u16 {
    u16::try_from(text.width()).unwrap_or(u16::MAX)
}

/// `text` cut to `room` cells by dropping its start for `…`, as the end of a
/// path says most about where it is.
fn cut_front(text: &str, room: usize) -> String {
    if text.width() <= room {
        return text.to_string();
    }
    if room == 0 {
        return String::new();
    }
    let mut kept: Vec<char> = Vec::new();
    let mut used = 1;
    for c in text.chars().rev() {
        let w = c.to_string().width();
        if used + w > room {
            break;
        }
        used += w;
        kept.push(c);
    }
    std::iter::once('…').chain(kept.into_iter().rev()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use ratatui::style::Modifier;

    use crate::theme::mix;
    use crate::ui::kit::{CAP_L, CAP_R};

    fn folder(names: &[&str]) -> anyhow::Result<tempfile::TempDir> {
        let dir = tempfile::tempdir()?;
        for name in names {
            fs::create_dir_all(dir.path().join(name))?;
        }
        fs::write(dir.path().join("file.txt"), "not a folder")?;
        Ok(dir)
    }

    fn browse(dir: &Path) -> anyhow::Result<DirPicker> {
        DirPicker::new(dir).map_err(anyhow::Error::msg)
    }

    fn type_query(p: &mut DirPicker, text: &str) {
        for c in text.chars() {
            p.handle(Input::Text(c));
        }
    }

    fn press(p: &mut DirPicker, action: Action) -> Option<Browsed> {
        p.handle(Input::Action(action))
    }

    #[test]
    fn folders_are_listed_ignoring_case_with_dot_folders_last_and_no_files() -> anyhow::Result<()> {
        let dir = folder(&["beta", "Alpha", ".git", "gamma", ".cache", "_build"])?;
        assert_eq!(
            list_folders(dir.path())?,
            ["_build", "Alpha", "beta", "gamma", ".cache", ".git"]
        );
        Ok(())
    }

    #[test]
    fn a_missing_folder_is_an_error() -> anyhow::Result<()> {
        let dir = folder(&[])?;
        assert!(list_folders(&dir.path().join("gone")).is_err());
        assert!(DirPicker::new(&dir.path().join("gone")).is_err());
        Ok(())
    }

    #[test]
    fn clean_works_out_dots_from_the_text() -> anyhow::Result<()> {
        let dir = folder(&["a/b"])?;
        let root = clean(dir.path());
        assert_eq!(
            clean(&root.join("a").join("..").join(".").join("a")),
            root.join("a")
        );
        assert!(clean(Path::new(".")).is_absolute());
        Ok(())
    }

    #[test]
    fn typing_filters_fuzzily_keeping_the_open_row() -> anyhow::Result<()> {
        let dir = folder(&["docs", "src", "scripts", "target"])?;
        let mut p = browse(dir.path())?;
        assert_eq!(p.listed(), ["docs", "scripts", "src", "target"]);
        assert_eq!(p.selected, 0);
        type_query(&mut p, "src");
        assert_eq!(p.listed().first(), Some(&"src"));
        assert!(!p.listed().contains(&"docs"));
        // The best match is selected; Up reaches the open row.
        assert_eq!(p.selected, 1);
        press(&mut p, Action::Move(Motion::Up));
        assert_eq!(p.selected, 0);
        assert_eq!(
            press(&mut p, Action::Newline),
            Some(Browsed::Open(clean(dir.path())))
        );
        type_query(&mut p, "zzz");
        assert!(p.listed().is_empty());
        assert_eq!(p.selected, 0);
        Ok(())
    }

    #[test]
    fn enter_goes_in_and_left_or_backspace_go_up() -> anyhow::Result<()> {
        let dir = folder(&["a/inner", "b"])?;
        let root = clean(dir.path());
        let mut p = browse(&root)?;
        press(&mut p, Action::Move(Motion::Down));
        assert_eq!(press(&mut p, Action::Newline), None);
        assert_eq!(p.dir(), root.join("a"));
        assert_eq!(p.listed(), ["inner"]);
        assert_eq!(p.selected, 0);
        press(&mut p, Action::Move(Motion::Left));
        assert_eq!(p.dir(), root);
        // Back up, the folder just left is selected.
        assert_eq!(p.selected, 1);
        press(&mut p, Action::Move(Motion::Down));
        press(&mut p, Action::Newline);
        assert_eq!(p.dir(), root.join("b"));
        press(&mut p, Action::Backspace);
        assert_eq!(p.dir(), root);
        // With a query, ← and Backspace edit it instead.
        type_query(&mut p, "ab");
        press(&mut p, Action::Move(Motion::Left));
        press(&mut p, Action::Backspace);
        assert_eq!(p.query.text(), "b");
        assert_eq!(p.dir(), root);
        Ok(())
    }

    #[test]
    fn going_up_stops_at_the_root() -> anyhow::Result<()> {
        let dir = folder(&[])?;
        let root = clean(dir.path())
            .ancestors()
            .last()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let mut p = browse(&root)?;
        assert_eq!(press(&mut p, Action::Move(Motion::Left)), None);
        assert_eq!(p.dir(), root);
        Ok(())
    }

    #[test]
    fn a_typed_path_jumps_there_and_a_wrong_one_says_so() -> anyhow::Result<()> {
        let dir = folder(&["a/deep", "b"])?;
        let root = clean(dir.path());
        let mut p = browse(&root.join("b"))?;
        let sep = std::path::MAIN_SEPARATOR;
        type_query(&mut p, &format!("..{sep}a{sep}deep"));
        assert_eq!(press(&mut p, Action::Newline), None);
        assert_eq!(p.dir(), root.join("a").join("deep"));
        assert_eq!(p.query.text(), "");

        type_query(&mut p, &root.join("b").display().to_string());
        assert_eq!(press(&mut p, Action::Newline), None);
        assert_eq!(p.dir(), root.join("b"));

        type_query(&mut p, &format!("nope{sep}x"));
        assert_eq!(
            press(&mut p, Action::Newline),
            Some(Browsed::Failed(format!("no such folder: nope{sep}x")))
        );
        assert_eq!(p.dir(), root.join("b"));
        Ok(())
    }

    #[test]
    fn a_folder_that_cannot_be_read_says_why_and_stays() -> anyhow::Result<()> {
        let dir = folder(&["gone", "kept"])?;
        let root = clean(dir.path());
        let mut p = browse(&root)?;
        fs::remove_dir(root.join("gone"))?;
        press(&mut p, Action::Move(Motion::Down));
        let Some(Browsed::Failed(why)) = press(&mut p, Action::Newline) else {
            panic!("entering a vanished folder should fail");
        };
        assert!(why.starts_with("cannot open "), "{why}");
        assert!(why.contains("gone"), "{why}");
        assert_eq!(p.dir(), root);
        assert_eq!(p.selected, 1);
        Ok(())
    }

    #[test]
    fn ctrl_enter_opens_the_folder_shown_and_esc_closes() -> anyhow::Result<()> {
        let dir = folder(&["a", "b"])?;
        let mut p = browse(dir.path())?;
        press(&mut p, Action::Move(Motion::Down));
        assert_eq!(
            press(&mut p, Action::OpenFolderHere),
            Some(Browsed::Open(clean(dir.path())))
        );
        assert_eq!(press(&mut p, Action::Cancel), Some(Browsed::Close));
        Ok(())
    }

    fn draw(p: &DirPicker, theme: &Theme) -> anyhow::Result<Buffer> {
        let mut terminal = Terminal::new(TestBackend::new(100, 30))?;
        terminal.draw(|frame| {
            p.render(Look::default(), theme, frame, frame.area(), None);
        })?;
        Ok(terminal.backend().buffer().clone())
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

    /// What the hints read with round ends.
    fn hints_text() -> String {
        HINTS
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

    #[test]
    fn the_browser_is_a_panel_with_header_open_row_folders_and_hints() -> anyhow::Result<()> {
        let names: Vec<String> = (0..12).map(|i| format!("dir{i:02}")).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let dir = folder(&refs)?;
        let mut p = browse(dir.path())?;
        let area = Rect::new(0, 0, 100, 30);
        let card = p.card(area);
        // The cast's card: 86 wide from row 4, as tall as its rows need.
        assert_eq!(card, Rect::new(7, 4, 86, 18));
        for name in ["tinkery", "tinkery-light"] {
            let theme = Theme::named(name).expect("a listed theme exists");
            let buffer = draw(&p, &theme)?;
            // An accent edge with the title in it; no lit edge, no rule.
            let edge = card_row(&buffer, card, card.y);
            assert!(edge.starts_with(" ▁ Open directory ▁"), "{name}: {edge:?}");
            assert_eq!(buffer[(card.x + 30, card.y)].fg, theme.accent, "{name}");
            assert_eq!(buffer[(card.x, card.y + 1)].symbol(), "▕", "{name}");
            assert_eq!(buffer[(card.x + 1, card.bottom() - 1)].symbol(), "▔");
            // The header's strip: `✦`, the folder, and the scope at the right.
            let y = card.y + 2;
            let header = card_row(&buffer, card, y);
            assert!(
                header.starts_with(&format!("▕{CAP_L}  ✦ ")),
                "{name}: {header:?}"
            );
            assert!(
                header.ends_with(&format!("open · folders  {CAP_R}▏")),
                "{name}: {header:?}"
            );
            for x in card.x + 2..card.right() - 2 {
                assert_eq!(buffer[(x, y)].bg, theme.raised2, "{name}: {x}");
            }
            assert_eq!(buffer[(card.x + 4, y)].fg, theme.accent2, "{name}");
            assert_eq!(buffer[(card.x + 6, y)].fg, theme.text, "{name}");
            assert_eq!(buffer[(card.right() - 5, y)].fg, theme.muted, "{name}");
            assert_eq!(text_of(&buffer, card, card.y + 3), "", "{name}");
            // The open row, selected: a pill, its words `strong` bold.
            let open = text_of(&buffer, card, card.y + 4);
            assert!(open.starts_with("⏎ open "), "{name}: {open:?}");
            assert_selected(&buffer, &theme, card, card.y + 4);
            assert_eq!(buffer[(card.x + 4, card.y + 4)].fg, theme.accent, "{name}");
            let word = &buffer[(card.x + 6, card.y + 4)];
            assert_eq!(word.fg, theme.strong, "{name}");
            assert!(word.modifier.contains(Modifier::BOLD), "{name}");
            assert_eq!(buffer[(card.x + 11, card.y + 4)].fg, theme.text, "{name}");
            // Ten folders, none lit.
            assert_eq!(text_of(&buffer, card, card.y + 5), "▸ dir00", "{name}");
            assert_eq!(text_of(&buffer, card, card.y + 14), "▸ dir09", "{name}");
            for row in card.y + 5..card.y + 15 {
                assert_resting(&buffer, &theme, card, row);
            }
            assert_eq!(buffer[(card.x + 4, card.y + 5)].fg, theme.muted, "{name}");
            assert_eq!(buffer[(card.x + 6, card.y + 5)].fg, theme.fg, "{name}");
            // A blank, then the keys on the row above the bottom edge.
            assert_eq!(text_of(&buffer, card, card.y + 15), "", "{name}");
            assert_eq!(text_of(&buffer, card, card.y + 16), hints_text(), "{name}");
            assert!(hints_text().ends_with(&format!("{CAP_L}Esc{CAP_R} close")));
            let glow = mix(theme.raised, theme.accent, 0.3);
            for row in card.top()..card.bottom() {
                for x in card.left()..card.right() {
                    let cell = &buffer[(x, row)];
                    assert!(!matches!(cell.symbol(), "▀" | "─"), "{name}: {x},{row}");
                    assert_ne!(cell.bg, glow, "{name}: {x},{row}");
                    assert_ne!(cell.bg, theme.accent, "{name}: {x},{row}");
                }
                // No `⏎` marks the selected row's right.
                assert_ne!(buffer[(card.right() - 5, row)].symbol(), "⏎", "{name}");
            }
        }

        // The selection scrolls the list past the card, taking the pill along.
        let theme = Theme::default();
        for _ in 0..12 {
            press(&mut p, Action::Move(Motion::Down));
        }
        let buffer = draw(&p, &theme)?;
        assert_eq!(text_of(&buffer, card, card.y + 5), "▸ dir02");
        assert_eq!(text_of(&buffer, card, card.y + 14), "▸ dir11");
        assert_selected(&buffer, &theme, card, card.y + 14);
        assert_resting(&buffer, &theme, card, card.y + 4);
        assert_eq!(buffer[(card.x + 4, card.y + 14)].fg, theme.text);
        assert_eq!(buffer[(card.x + 6, card.y + 14)].fg, theme.strong);
        Ok(())
    }

    #[test]
    fn matched_letters_stay_accent_bold_and_no_match_says_so() -> anyhow::Result<()> {
        let dir = folder(&["docs", "src"])?;
        let mut p = browse(dir.path())?;
        let theme = Theme::default();
        let card = p.card(Rect::new(0, 0, 100, 30));
        type_query(&mut p, "sr");
        let buffer = draw(&p, &theme)?;
        assert_eq!(text_of(&buffer, card, card.y + 5), "▸ src");
        assert_selected(&buffer, &theme, card, card.y + 5);
        assert_resting(&buffer, &theme, card, card.y + 4);
        for (x, lit) in [(6, true), (7, true), (8, false)] {
            let cell = &buffer[(card.x + x, card.y + 5)];
            let color = if lit { theme.accent } else { theme.strong };
            assert_eq!(cell.fg, color, "{x}");
            assert!(cell.modifier.contains(Modifier::BOLD), "{x}");
        }
        // The query follows the folder on the strip, in `strong`.
        let header = card_row(&buffer, card, card.y + 2);
        let typed = header.find("sr ").expect("the query after the folder");
        let typed = u16::try_from(header[..typed].chars().count())?;
        assert_eq!(buffer[(card.x + typed, card.y + 2)].fg, theme.strong);
        assert_eq!(buffer[(card.x + typed, card.y + 2)].bg, theme.raised2);

        type_query(&mut p, "zzz");
        let buffer = draw(&p, &theme)?;
        assert_eq!(text_of(&buffer, card, card.y + 5), "no matching folders");
        assert_eq!(buffer[(card.x + 4, card.y + 5)].fg, theme.muted);
        assert_resting(&buffer, &theme, card, card.y + 5);
        assert_selected(&buffer, &theme, card, card.y + 4);
        Ok(())
    }

    #[test]
    fn mono_reverses_the_selected_row_and_gives_it_no_colours() -> anyhow::Result<()> {
        let dir = folder(&["a"])?;
        let mut p = browse(dir.path())?;
        let mono = Theme::named("mono").expect("mono exists");
        let reversed = Theme::highlight(mono.hov, mono.strong);
        let card = p.card(Rect::new(0, 0, 100, 30));
        for selected in [card.y + 4, card.y + 5] {
            let buffer = draw(&p, &mono)?;
            for x in card.x + 1..card.right() - 1 {
                let cell = &buffer[(x, selected)];
                assert!(cell.modifier.contains(Modifier::REVERSED), "{x}");
                // Nothing on the row has a colour of its own.
                assert_eq!((Some(cell.fg), Some(cell.bg)), (reversed.fg, reversed.bg));
            }
            assert_eq!(buffer[(card.x + 2, selected)].symbol(), "›");
            for row in card.top()..card.bottom() {
                for x in card.left()..card.right() {
                    let cell = &buffer[(x, row)];
                    // Terminal colours only, and no Nerd Font glyph.
                    let rgb = |c| matches!(c, ratatui::style::Color::Rgb(..));
                    assert!(!rgb(cell.fg) && !rgb(cell.bg), "{x},{row}");
                    assert!(!matches!(cell.symbol(), CAP_L | CAP_R), "{x},{row}");
                    if row != selected {
                        assert!(!cell.modifier.contains(Modifier::REVERSED), "{x},{row}");
                    }
                }
            }
            press(&mut p, Action::Move(Motion::Down));
        }
        Ok(())
    }

    #[test]
    fn a_long_folder_is_cut_from_the_front() {
        assert_eq!(cut_front("abcdef", 4), "…def");
        assert_eq!(cut_front("abc", 4), "abc");
    }
}
