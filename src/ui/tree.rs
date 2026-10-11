//! The file tree panel on the left: a card filled with `surface`, holding the
//! `✦ tinkery` brand and project folder, then one row per visible node. The row
//! with the selection, and the active file's, is a pill (`kit::sel_row`). The
//! `│` divider here is only drawn between splits.

use std::path::{Path, PathBuf};

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::theme::{Theme, grad};
use crate::ui::kit::{self, Card, Look};
use crate::workspace::Tree;

/// Columns the tree takes, its card's two edges included. The editor starts
/// one column further right.
pub const TREE_WIDTH: u16 = 28;

/// Rows above the first node: the card's top edge, the brand row and a blank.
const NODES_TOP: u16 = 3;

/// Rows below the last node: the card's bottom edge.
const NODES_BOTTOM: u16 = 1;

/// Where a row's content starts, from the card's left edge: after the edge, a
/// row pill's end and its `›`. The brand's `✦` and a top-level node's marker
/// both sit here.
const INSET: u16 = 3;

/// Where the project folder starts on the brand row: after `✦ tinkery` and a
/// blank.
const PROJECT_X: u16 = INSET + 10;

/// What the tree marks besides its own rows. Paths are absolute.
pub struct TreeMarks<'a> {
    /// The project folder as the brand row shows it (see `project_label`).
    pub project: &'a str,
    /// The focused split's active file, whose row is lit.
    pub active: Option<&'a Path>,
    /// Open files with unsaved changes, which get a `•`.
    pub dirty: &'a [PathBuf],
    /// The row the pointer is on, as an index into the tree's rows.
    pub hovered: Option<usize>,
}

/// The part of the tree's `area` the nodes are listed in, between the brand
/// and the card's bottom edge. Mouse clicks, scrolling and paging all count
/// rows from its top.
pub fn nodes_area(area: Rect) -> Rect {
    let top = NODES_TOP.min(area.height);
    Rect {
        y: area.y + top,
        height: (area.height - top).saturating_sub(NODES_BOTTOM),
        ..area
    }
}

/// `root` as the brand row names it: under the home folder it reads `~/a/b`.
pub fn project_label(root: &Path, home: Option<&Path>) -> String {
    let root = std::path::absolute(root).unwrap_or_else(|_| root.to_path_buf());
    match home.and_then(|home| root.strip_prefix(home).ok()) {
        Some(rest) => rest.components().fold(String::from("~"), |label, part| {
            format!("{label}/{}", part.as_os_str().to_string_lossy())
        }),
        None => root.display().to_string(),
    }
}

/// Draws the tree into `area`: a card filled with `surface` on whatever is
/// already drawn there. Nodes are indented two columns per level with `│`
/// guides under their ancestors, folders carry `▾`/`▸`, names are cut with `…`.
///
/// The selected row, while the tree has focus, is `kit::sel_row`'s selected
/// row: a `hov` pill with `›` and a bold `strong` name, or reverse video in
/// `mono`. The active file's row, when the selection isn't on it or the tree
/// isn't focused, is the pill without the `›`, its name bold in `strong`; in
/// `mono`, which has no fills, the bold name alone. The row under the pointer
/// is that pill too, with its name as it was: only the active file's is bold.
/// An unfocused selection shows nothing. Returns where the selected row's
/// marker is, for the terminal cursor.
pub fn render_tree(
    look: Look,
    theme: &Theme,
    tree: &Tree,
    marks: &TreeMarks,
    focused: bool,
    area: Rect,
    frame: &mut Frame,
) -> Option<(u16, u16)> {
    let out = frame.buffer_mut();
    let card = Card {
        fill: theme.surface,
        ..Card::new(theme, "")
    };
    let inside = kit::card(look, theme, out, area, &card);
    if inside.is_empty() {
        return None;
    }
    let width = area.width;
    // Names stop three columns short of the card's edge, where the dirty mark
    // goes: the last cell before a row pill's end.
    let end = width.saturating_sub(3);

    let y = inside.y;
    put(out, inside, INSET, y, "✦", Style::new().fg(theme.accent2));
    let brand = ['t', 'i', 'n', 'k', 'e', 'r', 'y'];
    for (i, c) in (0u16..).zip(brand) {
        let fg = grad(&[theme.accent, theme.accent2], f64::from(i) / 6.0);
        let style = Style::new().fg(fg).add_modifier(Modifier::BOLD);
        let x = INSET + 2 + i;
        put(out, inside, x, y, c.encode_utf8(&mut [0; 4]), style);
    }
    let room = usize::from(end.saturating_sub(PROJECT_X));
    let project = cut_front(marks.project, room);
    let muted = Style::new().fg(theme.muted);
    put(out, inside, PROJECT_X, y, &project, muted);

    let nodes = nodes_area(area);
    let mut selected_at = None;
    for line in 0..nodes.height {
        let Some(index) = tree.row_at(usize::from(line)) else {
            break;
        };
        let row = &tree.rows()[index];
        let y = nodes.y + line;
        let path = std::path::absolute(&row.entry.path).unwrap_or_else(|_| row.entry.path.clone());
        let selected = index == tree.selected();
        let chosen = selected && focused;
        let active = marks.active == Some(path.as_path());
        // The active file's row and the one under the pointer are both the
        // kit's pill without the `›`; the name's weight tells them apart.
        let lit = active || marks.hovered == Some(index);
        let ink = kit::sel_row(
            look,
            theme,
            out,
            Rect::new(inside.x, y, inside.width, 1),
            chosen,
            lit && !chosen,
            theme.surface,
        );
        let depth = u16::try_from(row.depth).unwrap_or(u16::MAX);
        let guide = ink.of(Style::new().fg(theme.guide));
        for d in 0..depth {
            let x = d.saturating_mul(2).saturating_add(INSET + 1);
            put(out, inside, x, y, "│", guide);
        }
        let marker_x = depth.saturating_mul(2).saturating_add(INSET);
        if selected {
            selected_at = Some((area.x + marker_x.min(width.saturating_sub(2)), y));
        }
        let marker = match (row.entry.is_dir, row.expanded) {
            (true, true) => "▾",
            (true, false) => "▸",
            (false, _) => " ",
        };
        put(out, inside, marker_x, y, marker, ink.faint);
        let name_x = marker_x.saturating_add(2);
        let name_style = if chosen {
            ink.label
        } else if active {
            ink.of(Style::new().fg(theme.strong).add_modifier(Modifier::BOLD))
        } else {
            ink.of(Style::new().fg(theme.text))
        };
        let name = cut(&row.entry.name, usize::from(end.saturating_sub(name_x)));
        put(out, inside, name_x, y, &name, name_style);
        if marks.dirty.contains(&path) {
            put(
                out,
                inside,
                end,
                y,
                "•",
                ink.of(Style::new().fg(theme.warn)),
            );
        }
    }
    selected_at
}

/// A vertical `│` down `area` in `guide`: the rule between two splits, the only
/// vertical one on screen (README §5.6).
pub fn render_divider(theme: &Theme, area: Rect, frame: &mut Frame) {
    let out = frame.buffer_mut();
    for y in area.top()..area.bottom() {
        out.set_string(area.x, y, "│", Style::new().fg(theme.guide));
    }
}

/// Writes `text` on screen row `y`, `x` columns from the left edge of the card
/// whose inside is `inside`, clipped to that inside: a card's edge is never
/// written over.
fn put(out: &mut Buffer, inside: Rect, x: u16, y: u16, text: &str, style: Style) {
    // The card's left edge is the column before its inside.
    let at = inside.x.saturating_add(x).saturating_sub(1);
    if at < inside.x || at >= inside.right() || y < inside.y || y >= inside.bottom() {
        return;
    }
    out.set_stringn(at, y, text, usize::from(inside.right() - at), style);
}

/// `text` in at most `room` columns, its end replaced by `…` when it doesn't fit.
pub(crate) fn cut(text: &str, room: usize) -> String {
    if text.width() <= room {
        return text.to_string();
    }
    let mut kept = String::new();
    let mut used = 0;
    for c in text.chars() {
        let w = c.width().unwrap_or(0);
        if used + w + 1 > room {
            break;
        }
        kept.push(c);
        used += w;
    }
    if room > 0 {
        kept.push('…');
    }
    kept
}

/// `text` in at most `room` columns, its start replaced by `…` when it doesn't
/// fit: the end of a path says most about it.
pub(crate) fn cut_front(text: &str, room: usize) -> String {
    if text.width() <= room {
        return text.to_string();
    }
    if room == 0 {
        return String::new();
    }
    let mut kept = Vec::new();
    let mut used = 0;
    for c in text.chars().rev() {
        let w = c.width().unwrap_or(0);
        if used + w + 1 > room {
            break;
        }
        kept.push(c);
        used += w;
    }
    std::iter::once('…').chain(kept.into_iter().rev()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Corners;
    use crate::ui::kit::{CAP_L, CAP_R, SEL_MARK};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;
    use std::fs;

    const LOOK: Look = Look {
        pill_caps: true,
        corners: Corners::Flush,
    };

    fn draw(
        theme: &Theme,
        tree: &Tree,
        marks: &TreeMarks,
        focused: bool,
    ) -> anyhow::Result<(Buffer, Option<(u16, u16)>)> {
        let mut terminal = Terminal::new(TestBackend::new(30, 8))?;
        let mut at = None;
        terminal.draw(|frame| {
            let area = Rect::new(0, 0, 28, 8);
            at = render_tree(LOOK, theme, tree, marks, focused, area, frame)
        })?;
        Ok((terminal.backend().buffer().clone(), at))
    }

    fn row(buffer: &Buffer, y: u16) -> String {
        (0..28).map(|x| buffer[(x, y)].symbol()).collect()
    }

    /// A row of the tree whose inside reads `inside`, between the card's edges.
    fn framed(inside: &str) -> String {
        format!("▕{inside:<26}▏")
    }

    /// Asserts row `y` is a pill of `hov` from one edge of the card's inside
    /// to the other, on `surface`.
    fn assert_pill(buffer: &Buffer, theme: &Theme, y: u16) {
        for (x, cap) in [(1, CAP_L), (26, CAP_R)] {
            let cell = &buffer[(x, y)];
            assert_eq!(cell.symbol(), cap, "{x},{y}");
            assert_eq!((cell.fg, cell.bg), (theme.hov, theme.surface), "{x},{y}");
        }
        for x in 2..26 {
            assert_eq!(buffer[(x, y)].bg, theme.hov, "{x},{y}");
        }
    }

    #[test]
    fn the_tree_is_a_card_filled_with_surface_and_the_last_row_is_its_edge() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        for i in 0..9 {
            fs::write(dir.path().join(format!("f{i}.txt")), "")?;
        }
        let tree = Tree::new(dir.path());
        let theme = Theme::default();
        let marks = TreeMarks {
            project: "",
            active: None,
            dirty: &[],
            hovered: None,
        };
        let (buffer, _) = draw(&theme, &tree, &marks, false)?;
        assert_eq!(row(&buffer, 0), format!(" {} ", "▁".repeat(26)));
        assert_eq!(row(&buffer, 7), format!(" {} ", "▔".repeat(26)));
        for y in 1..7 {
            let (left, right) = (&buffer[(0, y)], &buffer[(27, y)]);
            assert_eq!((left.symbol(), right.symbol()), ("▕", "▏"), "{y}");
            // The edge sits on what was there, not on the card's fill.
            assert_eq!((left.fg, left.bg), (theme.line2, Color::Reset), "{y}");
            assert_eq!((right.fg, right.bg), (theme.line2, Color::Reset), "{y}");
            for x in 1..27 {
                assert_eq!(buffer[(x, y)].bg, theme.surface, "{x},{y}");
            }
        }
        // Nodes fill rows 3 to 6; the row after them is the edge.
        let area = Rect::new(0, 0, 28, 8);
        assert_eq!(nodes_area(area), Rect::new(0, 3, 28, 4));
        assert_eq!(row(&buffer, 3), framed("    f0.txt"));
        assert_eq!(row(&buffer, 6), framed("    f3.txt"));
        // With no room under the brand, no row is listed.
        assert_eq!(nodes_area(Rect::new(0, 0, 28, 3)).height, 0);
        assert_eq!(nodes_area(Rect::new(0, 0, 28, 2)).height, 0);
        Ok(())
    }

    #[test]
    fn rows_are_indented_with_guides_and_the_active_file_is_a_pill() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        fs::create_dir(dir.path().join("src"))?;
        let long = dir
            .path()
            .join("src")
            .join("a_very_long_file_name_indeed.rs");
        fs::write(&long, "")?;
        fs::write(dir.path().join("notes.txt"), "")?;
        let mut tree = Tree::new(dir.path());
        tree.expand();

        let theme = Theme::default();
        let long = std::path::absolute(&long)?;
        let marks = TreeMarks {
            project: "~/src",
            active: Some(&long),
            dirty: std::slice::from_ref(&long),
            hovered: None,
        };
        let (buffer, at) = draw(&theme, &tree, &marks, false)?;
        assert_eq!(row(&buffer, 1), framed("  ✦ tinkery ~/src"));
        assert_eq!(row(&buffer, 3), framed("  ▾ src"));
        assert_eq!(
            row(&buffer, 4),
            framed(&format!("{CAP_L}  │  a_very_long_file_…•{CAP_R}"))
        );
        assert_eq!(row(&buffer, 5), framed("    notes.txt"));
        assert_eq!(at, Some((3, 3)));

        // The brand keeps its ramp, from `accent` to `accent2`.
        assert_eq!(buffer[(3, 1)].fg, theme.accent2);
        assert_eq!(buffer[(5, 1)].fg, theme.accent);
        assert_eq!(buffer[(11, 1)].fg, theme.accent2);
        assert_ne!(buffer[(8, 1)].fg, theme.accent);
        assert_ne!(buffer[(8, 1)].fg, theme.accent2);
        assert!(buffer[(7, 1)].modifier.contains(Modifier::BOLD));
        assert_eq!(buffer[(13, 1)].fg, theme.muted);
        assert_eq!(buffer[(4, 4)].fg, theme.guide);
        assert_eq!(buffer[(25, 4)].fg, theme.warn);
        assert_eq!(buffer[(3, 3)].fg, theme.muted);

        // The active row is a pill with no `›`, its name bold in `strong`;
        // unfocused, the selection on row 3 isn't drawn at all.
        assert_pill(&buffer, &theme, 4);
        assert!(!row(&buffer, 4).contains(SEL_MARK));
        assert_eq!(buffer[(7, 4)].fg, theme.strong);
        assert!(buffer[(7, 4)].modifier.contains(Modifier::BOLD));
        for y in [3, 5] {
            for x in 1..27 {
                assert_eq!(buffer[(x, y)].bg, theme.surface, "{x},{y}");
            }
            assert!(!buffer[(5, y)].modifier.contains(Modifier::BOLD), "{y}");
            assert_eq!(buffer[(5, y)].fg, theme.text, "{y}");
        }
        Ok(())
    }

    #[test]
    fn the_row_under_the_pointer_is_lit_and_only_the_active_file_is_bold() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        fs::create_dir(dir.path().join("lib"))?;
        fs::write(dir.path().join("a.txt"), "")?;
        fs::write(dir.path().join("b.txt"), "")?;
        let tree = Tree::new(dir.path());
        let b = std::path::absolute(dir.path().join("b.txt"))?;
        // Rows 3, 4 and 5 are `lib`, `a.txt` and `b.txt`; the pointer is on
        // `a.txt` and `b.txt` is the active file.
        let marks = TreeMarks {
            project: "",
            active: Some(&b),
            dirty: &[],
            hovered: Some(1),
        };
        let theme = Theme::default();
        let (buffer, _) = draw(&theme, &tree, &marks, false)?;
        // Both are the pill without the mark.
        for y in [4, 5] {
            assert_pill(&buffer, &theme, y);
            assert_eq!(buffer[(2, y)].symbol(), " ", "{y}");
        }
        // The pointer's row keeps its name as it was; the active file's is
        // `strong` and bold.
        assert_eq!(buffer[(5, 4)].fg, theme.text);
        assert!(!buffer[(5, 4)].modifier.contains(Modifier::BOLD));
        assert_eq!(buffer[(5, 5)].fg, theme.strong);
        assert!(buffer[(5, 5)].modifier.contains(Modifier::BOLD));
        assert_eq!(buffer[(2, 3)].bg, theme.surface);

        // The pointer on the active file changes nothing, and with it gone
        // the active file is still lit.
        let on_active = TreeMarks {
            hovered: Some(2),
            ..marks
        };
        let away = TreeMarks {
            hovered: None,
            ..marks
        };
        let (over, _) = draw(&theme, &tree, &on_active, false)?;
        let (rest, _) = draw(&theme, &tree, &away, false)?;
        assert_eq!(over, rest);
        assert_pill(&rest, &theme, 5);
        assert_eq!(rest[(2, 4)].bg, theme.surface);

        // The focused selection under the pointer stays the marked pill.
        let (buffer, _) = draw(&theme, &tree, &marks, true)?;
        let selected = TreeMarks {
            hovered: Some(0),
            ..marks
        };
        let (pointed, _) = draw(&theme, &tree, &selected, true)?;
        assert_eq!(pointed[(2, 3)].symbol(), SEL_MARK);
        assert_eq!(buffer[(2, 3)].symbol(), SEL_MARK);

        // `mono` has no fills: the pointer shows nothing there.
        let mono = Theme::named("mono").expect("mono exists");
        let (lit, _) = draw(&mono, &tree, &marks, false)?;
        let (unlit, _) = draw(&mono, &tree, &away, false)?;
        assert_eq!(lit, unlit);
        Ok(())
    }

    #[test]
    fn the_focused_selection_is_a_marked_pill_and_mono_reverses_it() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        fs::create_dir(dir.path().join("lib"))?;
        fs::write(dir.path().join("a.txt"), "")?;
        fs::write(dir.path().join("b.txt"), "")?;
        let mut tree = Tree::new(dir.path());
        tree.move_by(1);
        let a = std::path::absolute(dir.path().join("a.txt"))?;
        let b = std::path::absolute(dir.path().join("b.txt"))?;
        let marks = TreeMarks {
            project: "",
            active: Some(&b),
            dirty: std::slice::from_ref(&a),
            hovered: None,
        };

        let theme = Theme::default();
        let (buffer, at) = draw(&theme, &tree, &marks, true)?;
        assert_eq!(at, Some((3, 4)));
        assert_eq!(
            row(&buffer, 4),
            framed(&format!("{CAP_L}{SEL_MARK}  a.txt{:15}•{CAP_R}", ""))
        );
        assert_pill(&buffer, &theme, 4);
        let mark = &buffer[(2, 4)];
        assert_eq!(mark.fg, theme.accent);
        assert!(mark.modifier.contains(Modifier::BOLD));
        assert_eq!(buffer[(5, 4)].fg, theme.strong);
        assert!(buffer[(5, 4)].modifier.contains(Modifier::BOLD));
        assert!(!buffer[(5, 4)].modifier.contains(Modifier::REVERSED));
        assert_eq!(buffer[(25, 4)].fg, theme.warn);
        // The active file, off the selection, is lit too, without the mark.
        assert_pill(&buffer, &theme, 5);
        assert_eq!(buffer[(2, 5)].symbol(), " ");
        assert_eq!(buffer[(5, 5)].fg, theme.strong);
        assert!(buffer[(5, 5)].modifier.contains(Modifier::BOLD));
        assert_eq!(buffer[(1, 3)].bg, theme.surface);

        // A selected folder keeps its marker, a cell after the `›`.
        tree.move_by(-1);
        let (buffer, at) = draw(&theme, &tree, &marks, true)?;
        assert_eq!(at, Some((3, 3)));
        assert!(row(&buffer, 3).starts_with(&format!("▕{CAP_L}{SEL_MARK}▸ lib")));
        // On a lit row the marker is `text`: `muted` is too close to `hov`.
        assert_eq!(buffer[(3, 3)].fg, theme.text);
        tree.move_by(1);

        let mono = Theme::named("mono").expect("mono exists");
        let (buffer, _) = draw(&mono, &tree, &marks, true)?;
        for y in 0..8 {
            let line = row(&buffer, y);
            assert!(!line.contains(CAP_L) && !line.contains(CAP_R), "{line:?}");
            for x in 0..28 {
                let cell = &buffer[(x, y)];
                // Reverse video across the card's inside on the selected row only.
                let reversed = y == 4 && (1..27).contains(&x);
                assert_eq!(
                    cell.modifier.contains(Modifier::REVERSED),
                    reversed,
                    "{x},{y}"
                );
                // Terminal colours only: nothing blended.
                assert!(!matches!(cell.fg, Color::Rgb(..)), "{x},{y}");
                assert!(!matches!(cell.bg, Color::Rgb(..)), "{x},{y}");
            }
        }
        assert_eq!(buffer[(2, 4)].symbol(), SEL_MARK);
        assert!(buffer[(5, 4)].modifier.contains(Modifier::BOLD));
        // One pair of colours across the row, so the reverse isn't broken up.
        let ground = (buffer[(1, 4)].fg, buffer[(1, 4)].bg);
        for x in 1..27 {
            assert_eq!((buffer[(x, 4)].fg, buffer[(x, 4)].bg), ground, "{x}");
        }
        // The active file is bold and nothing else about its row changes.
        assert!(buffer[(5, 5)].modifier.contains(Modifier::BOLD));
        assert_eq!(buffer[(5, 5)].fg, mono.strong);
        assert!(!buffer[(5, 3)].modifier.contains(Modifier::BOLD));

        // Unfocused, mono draws no selection.
        let (buffer, _) = draw(&mono, &tree, &marks, false)?;
        for x in 0..28 {
            assert!(!buffer[(x, 4)].modifier.contains(Modifier::REVERSED));
        }
        assert!(!row(&buffer, 4).contains(SEL_MARK));
        Ok(())
    }

    #[test]
    fn names_and_paths_are_cut_with_an_ellipsis() {
        assert_eq!(cut("main.rs", 7), "main.rs");
        assert_eq!(cut("main.rs", 5), "main…");
        assert_eq!(cut("main.rs", 0), "");
        assert_eq!(cut_front("~/a/project", 8), "…project");
    }

    #[test]
    fn the_project_reads_from_home() -> anyhow::Result<()> {
        let home = std::path::absolute("home")?;
        let root = home.join("src").join("tinkery");
        assert_eq!(project_label(&root, Some(&home)), "~/src/tinkery");
        let elsewhere = std::path::absolute("elsewhere")?;
        assert_eq!(
            project_label(&elsewhere, Some(&home)),
            elsewhere.display().to_string()
        );
        Ok(())
    }
}
