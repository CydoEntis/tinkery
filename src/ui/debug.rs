//! The debug panel (tinkery-debugger spec D7): while a debug session is on it
//! takes the run panel's slot, with the paused program's call stack on the left
//! and the selected frame's variables, by scope, on the right. While the program
//! runs there is no stack to show, so the body is the program's output.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::dap::{Scope, StackFrame, Variable};
use crate::keymap::Action;
use crate::theme::Theme;
use crate::ui::kit::{self, Look, RowInk};
use crate::ui::run::{Hints, RunView, render_output};

const PAUSED_HINTS: &Hints = &[("tab", "stack/variables"), ("F6", "focus"), ("F4", "hide")];
const RUNNING_HINTS: &Hints = &[("alt+F6", "stop"), ("F4", "hide")];

/// Where a row's text starts in its side: past the selected row's pill end,
/// its mark and a blank.
const TEXT_X: u16 = 3;

/// How deep expanded values nest before the rest is left out: a value that
/// contains itself would otherwise expand without end.
const MAX_DEPTH: usize = 32;

/// The half of the panel that has the keys.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    #[default]
    Stack,
    Variables,
}

/// What a key in the panel asks of the app, which owns the session.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Show this frame's variables and open its file at its line.
    Frame(StackFrame),
    /// Ask the adapter for the children of this reference.
    Fetch(i64),
}

/// One row of the variables side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarRow {
    pub depth: usize,
    pub name: String,
    /// `None` for a scope's heading row.
    pub value: Option<String>,
    pub kind: Option<String>,
    /// Non-zero when the row has children.
    pub reference: i64,
    pub expanded: bool,
}

/// What the panel shows and where its selections are. Reset each time the
/// program stops, since adapters number frames and references afresh then.
#[derive(Debug, Default)]
pub struct DebugPanel {
    frames: Vec<StackFrame>,
    /// The stack row the keys are on.
    stack_row: usize,
    /// The frame whose variables are shown.
    shown: usize,
    scopes: Vec<Scope>,
    /// Children fetched so far, by reference.
    children: HashMap<i64, Vec<Variable>>,
    expanded: HashSet<i64>,
    pane: Pane,
    var_row: usize,
}

impl DebugPanel {
    /// A new stop's stack, the top frame selected. Returns the frame whose
    /// scopes to ask for.
    pub fn set_frames(&mut self, frames: Vec<StackFrame>) -> Option<i64> {
        *self = DebugPanel {
            pane: self.pane,
            frames,
            ..DebugPanel::default()
        };
        self.frames.first().map(|f| f.id)
    }

    /// Whether there's a stack to move through.
    pub fn has_frames(&self) -> bool {
        !self.frames.is_empty()
    }

    /// The program runs on: what was shown no longer holds.
    pub fn clear(&mut self) {
        self.set_frames(Vec::new());
    }

    /// The scopes of frame `frame`, if it's still the one shown. Scopes that
    /// aren't expensive start expanded; returns their references, whose
    /// variables to ask for.
    pub fn set_scopes(&mut self, frame: i64, scopes: Vec<Scope>) -> Vec<i64> {
        if self.frames.get(self.shown).map(|f| f.id) != Some(frame) {
            return Vec::new();
        }
        let open: Vec<i64> = scopes
            .iter()
            .filter(|s| !s.expensive && s.variables_reference != 0)
            .map(|s| s.variables_reference)
            .collect();
        self.expanded.extend(open.iter().copied());
        self.scopes = scopes;
        self.var_row = 0;
        open
    }

    pub fn set_variables(&mut self, reference: i64, variables: Vec<Variable>) {
        self.children.insert(reference, variables);
    }

    /// The variables side as rows: each scope, then, while it's expanded, its
    /// variables, nested the same way.
    pub fn rows(&self) -> Vec<VarRow> {
        let mut rows = Vec::new();
        for scope in &self.scopes {
            let reference = scope.variables_reference;
            let expanded = reference != 0 && self.expanded.contains(&reference);
            rows.push(VarRow {
                depth: 0,
                name: scope.name.clone(),
                value: None,
                kind: None,
                reference,
                expanded,
            });
            if expanded {
                self.push_children(reference, 1, &mut rows);
            }
        }
        rows
    }

    fn push_children(&self, reference: i64, depth: usize, rows: &mut Vec<VarRow>) {
        if depth > MAX_DEPTH {
            return;
        }
        for variable in self.children.get(&reference).into_iter().flatten() {
            let child = variable.variables_reference;
            let expanded = child != 0 && self.expanded.contains(&child);
            rows.push(VarRow {
                depth,
                name: variable.name.clone(),
                value: Some(variable.value.clone()),
                kind: variable.kind.clone(),
                reference: child,
                expanded,
            });
            if expanded {
                self.push_children(child, depth + 1, rows);
            }
        }
    }

    /// Acts on one of the panel's keys.
    pub fn handle(&mut self, action: Action) -> Option<Step> {
        match (action, self.pane) {
            (Action::DebugSwitchPane, Pane::Stack) => self.pane = Pane::Variables,
            (Action::DebugSwitchPane, Pane::Variables) => self.pane = Pane::Stack,
            (Action::DebugUp, Pane::Stack) => self.stack_row = self.stack_row.saturating_sub(1),
            (Action::DebugDown, Pane::Stack) => {
                self.stack_row = (self.stack_row + 1).min(self.frames.len().saturating_sub(1));
            }
            (Action::DebugUp, Pane::Variables) => self.var_row = self.var_row.saturating_sub(1),
            (Action::DebugDown, Pane::Variables) => {
                self.var_row = (self.var_row + 1).min(self.rows().len().saturating_sub(1));
            }
            (Action::DebugActivate, Pane::Stack) => {
                let frame = self.frames.get(self.stack_row)?.clone();
                self.shown = self.stack_row;
                self.scopes.clear();
                self.children.clear();
                self.expanded.clear();
                self.var_row = 0;
                return Some(Step::Frame(frame));
            }
            (Action::DebugActivate, Pane::Variables) => {
                let row = self.rows().into_iter().nth(self.var_row)?;
                return if row.expanded {
                    self.collapse(&row);
                    None
                } else {
                    self.expand(&row)
                };
            }
            (Action::DebugExpand, Pane::Variables) => {
                let row = self.rows().into_iter().nth(self.var_row)?;
                return self.expand(&row);
            }
            (Action::DebugCollapse, Pane::Variables) => {
                let rows = self.rows();
                let row = rows.get(self.var_row)?;
                if row.expanded {
                    self.collapse(row);
                } else if let Some(parent) = rows[..self.var_row]
                    .iter()
                    .rposition(|r| r.depth < row.depth)
                {
                    self.var_row = parent;
                }
            }
            _ => {}
        }
        None
    }

    fn expand(&mut self, row: &VarRow) -> Option<Step> {
        if row.reference == 0 || row.expanded {
            return None;
        }
        self.expanded.insert(row.reference);
        (!self.children.contains_key(&row.reference)).then_some(Step::Fetch(row.reference))
    }

    fn collapse(&mut self, row: &VarRow) {
        self.expanded.remove(&row.reference);
        self.var_row = self.var_row.min(self.rows().len().saturating_sub(1));
    }
}

/// Everything the panel is drawn from.
pub struct DebugView<'a> {
    pub panel: &'a DebugPanel,
    /// The program's output, and the name it runs under.
    pub run: Option<&'a RunView>,
    pub paused: bool,
    /// The panel has the keys.
    pub focused: bool,
}

/// Draws the panel in `area`: a title row on `surface`, then the stack and the
/// variables while paused, or the program's output while it runs. Returns
/// where the terminal cursor goes while the panel has focus.
pub fn render_debug_panel(
    look: Look,
    theme: &Theme,
    view: &DebugView,
    area: Rect,
    frame: &mut Frame,
) -> Option<(u16, u16)> {
    if area.height == 0 || area.width == 0 {
        return None;
    }
    let out = frame.buffer_mut();
    out.set_style(area, Style::new().bg(theme.bg).fg(theme.fg));
    let title = Rect { height: 1, ..area };
    render_title(look, theme, view, title, out);
    let body = Rect {
        y: area.y + 1,
        height: area.height - 1,
        ..area
    };
    if body.height == 0 {
        return None;
    }
    if !view.paused || view.panel.frames.is_empty() {
        if let Some(run) = view.run {
            render_output(theme, run, body, frame);
        }
        return None;
    }
    let stack_width = (body.width * 2 / 5).max(20).min(body.width);
    let stack = Rect {
        width: stack_width,
        ..body
    };
    let divider = stack.right();
    let vars = Rect {
        x: divider.saturating_add(1),
        width: body.right().saturating_sub(divider.saturating_add(1)),
        ..body
    };
    let out = frame.buffer_mut();
    if divider < body.right() {
        for y in body.top()..body.bottom() {
            out.set_string(divider, y, "│", Style::new().fg(theme.guide));
        }
    }
    let stack_cursor = render_stack(look, theme, view, stack, out);
    let vars_cursor = render_variables(look, theme, view, vars, out);
    if !view.focused {
        return None;
    }
    match view.panel.pane {
        Pane::Stack => stack_cursor,
        Pane::Variables => vars_cursor,
    }
}

/// The title row, as the run panel's: the state glyph and word, the session's
/// name in `strong` bold, and the keys as keycap hints ending one cell from the
/// right; they are left out when they would crowd the state word.
fn render_title(look: Look, theme: &Theme, view: &DebugView, row: Rect, out: &mut Buffer) {
    out.set_style(row, Style::new().bg(theme.surface).fg(theme.text));
    let (glyph, word, color, hints) = if view.paused {
        ("‖", "paused", theme.accent2, PAUSED_HINTS)
    } else {
        ("●", "running", theme.warn, RUNNING_HINTS)
    };
    let name = view.run.map_or("debug", |run| run.name.as_str());
    let state = Style::new().fg(color);
    let right = row.right();
    let mut x = row.x + 1;
    for (text, style) in [
        (glyph, state),
        (" ", Style::new()),
        (
            name,
            Style::new().fg(theme.strong).add_modifier(Modifier::BOLD),
        ),
        ("  ", Style::new()),
        (word, state),
    ] {
        let room = usize::from(right.saturating_sub(x));
        x = out.set_stringn(x, row.y, text, room, style).0;
    }
    let hints = kit::hints(look, theme, hints, theme.surface);
    let hints_x = right.saturating_sub(kit::width(&hints).saturating_add(1));
    if hints_x >= x + 3 {
        kit::put(out, hints_x, row.y, &hints, right);
    }
}

/// A side's label row: `muted`, or `accent` while that side has the keys.
fn label(theme: &Theme, text: &str, lit: bool, area: Rect, out: &mut Buffer) {
    let color = if lit { theme.accent } else { theme.muted };
    out.set_stringn(
        area.x + 1,
        area.y,
        text,
        usize::from(area.width.saturating_sub(1)),
        Style::new().fg(color).add_modifier(Modifier::BOLD),
    );
}

/// The first of `count` rows to draw so that `selected` shows in `height`.
fn first_row(selected: usize, height: usize) -> usize {
    (selected + 1).saturating_sub(height.max(1))
}

/// Draws row `y` of the list in `list`, the selected one as the kit's selected
/// row on the panel's `bg`, and returns the styles its text goes in and the
/// column that text stops before: a cell short of the side's edge, where the
/// selected row's pill ends.
fn list_row(
    look: Look,
    theme: &Theme,
    out: &mut Buffer,
    list: Rect,
    y: u16,
    selected: bool,
) -> (RowInk, u16) {
    let row = Rect::new(list.x, y, list.width, 1);
    // Nothing reports the mouse's position yet, so no row is hovered.
    let ink = kit::sel_row(look, theme, out, row, selected, false, theme.bg);
    (ink, list.right().saturating_sub(1))
}

fn render_stack(
    look: Look,
    theme: &Theme,
    view: &DebugView,
    area: Rect,
    out: &mut Buffer,
) -> Option<(u16, u16)> {
    let panel = view.panel;
    let lit = view.focused && panel.pane == Pane::Stack;
    label(theme, "call stack", lit, area, out);
    let list = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    };
    let height = usize::from(list.height);
    let first = first_row(panel.stack_row, height);
    let mut cursor = None;
    for (index, stack_frame) in panel.frames.iter().enumerate().skip(first).take(height) {
        let y = list.y + u16::try_from(index - first).unwrap_or(u16::MAX);
        let selected = index == panel.stack_row;
        let (ink, right) = list_row(look, theme, out, list, y, selected);
        let mut x = list.x.saturating_add(TEXT_X).min(right);
        if selected {
            cursor = Some((x, y));
        }
        let mut put = |text: &str, style: Style| {
            let room = usize::from(right.saturating_sub(x));
            x = out.set_stringn(x, y, text, room, style).0;
        };
        // The frame whose variables are shown, marked as the paused line is.
        if index == panel.shown {
            put("▶ ", ink.of(Style::new().fg(theme.accent2)));
        } else {
            put("  ", ink.of(Style::new()));
        }
        let name = if selected {
            ink.label
        } else {
            ink.of(Style::new().fg(theme.text))
        };
        put(&stack_frame.name, name);
        put("  ", ink.of(Style::new()));
        put(&location(stack_frame), ink.faint);
    }
    cursor
}

/// `file:line` for a frame, by its file's name.
fn location(frame: &StackFrame) -> String {
    let source = frame.source.as_ref();
    let file = source
        .and_then(|s| s.path.as_deref())
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .or_else(|| source.and_then(|s| s.name.clone()))
        .unwrap_or_else(|| "?".to_string());
    format!("{file}:{}", frame.line)
}

fn render_variables(
    look: Look,
    theme: &Theme,
    view: &DebugView,
    area: Rect,
    out: &mut Buffer,
) -> Option<(u16, u16)> {
    if area.width == 0 {
        return None;
    }
    let panel = view.panel;
    let lit = view.focused && panel.pane == Pane::Variables;
    label(theme, "variables", lit, area, out);
    let list = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    };
    let rows = panel.rows();
    let height = usize::from(list.height);
    let first = first_row(panel.var_row, height);
    let mut cursor = None;
    for (index, row) in rows.iter().enumerate().skip(first).take(height) {
        let y = list.y + u16::try_from(index - first).unwrap_or(u16::MAX);
        // The variables side only shows a selection while it has the keys,
        // so one pill says which side does.
        let selected = lit && index == panel.var_row;
        let (ink, right) = list_row(look, theme, out, list, y, selected);
        let indent = u16::try_from(row.depth * 2).unwrap_or(u16::MAX);
        let mut x = list
            .x
            .saturating_add(TEXT_X)
            .saturating_add(indent)
            .min(right);
        if index == panel.var_row {
            cursor = Some((x, y));
        }
        let mut put = |text: &str, style: Style| {
            let room = usize::from(right.saturating_sub(x));
            x = out.set_stringn(x, y, text, room, style).0;
        };
        let chevron = match (row.reference != 0, row.expanded) {
            (false, _) => "  ",
            (true, false) => "▸ ",
            (true, true) => "▾ ",
        };
        put(chevron, ink.faint);
        // The selected row's name is the row's label, whatever it rests in.
        let name = |resting: Style| {
            if selected { ink.label } else { ink.of(resting) }
        };
        let Some(value) = &row.value else {
            let heading = Style::new().fg(theme.text).add_modifier(Modifier::BOLD);
            put(&row.name, name(heading));
            continue;
        };
        put(&row.name, name(Style::new().fg(theme.strong)));
        put(" = ", ink.faint);
        put(value, ink.of(Style::new().fg(theme.text)));
        if let Some(kind) = row.kind.as_deref().filter(|k| !k.is_empty()) {
            put("  ", ink.of(Style::new()));
            put(kind, ink.faint);
        }
    }
    cursor
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dap::Source;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::path::PathBuf;

    fn frame(id: i64, name: &str, file: &str, line: usize) -> StackFrame {
        StackFrame {
            id,
            name: name.into(),
            source: Some(Source {
                name: None,
                path: Some(PathBuf::from(file)),
            }),
            line,
            column: 1,
        }
    }

    fn variable(name: &str, value: &str, kind: &str, reference: i64) -> Variable {
        Variable {
            name: name.into(),
            value: value.into(),
            kind: Some(kind.into()),
            variables_reference: reference,
        }
    }

    fn scope(name: &str, reference: i64) -> Scope {
        Scope {
            name: name.into(),
            variables_reference: reference,
            expensive: false,
        }
    }

    /// A panel paused in `inner` called from `main`, its locals loaded.
    fn paused() -> DebugPanel {
        let mut panel = DebugPanel::default();
        let top = panel.set_frames(vec![
            frame(1, "inner", "/p/main.py", 3),
            frame(2, "main", "/p/main.py", 9),
        ]);
        assert_eq!(top, Some(1));
        assert_eq!(panel.set_scopes(1, vec![scope("Locals", 10)]), vec![10]);
        panel.set_variables(
            10,
            vec![
                variable("n", "3", "int", 0),
                variable("items", "[1, 2]", "list", 11),
            ],
        );
        panel
    }

    fn names(panel: &DebugPanel) -> Vec<String> {
        panel.rows().iter().map(|r| r.name.clone()).collect()
    }

    #[test]
    fn scopes_start_expanded_and_values_expand_on_demand() {
        let mut panel = paused();
        assert_eq!(names(&panel), ["Locals", "n", "items"]);
        panel.handle(Action::DebugSwitchPane);
        panel.handle(Action::DebugDown);
        panel.handle(Action::DebugDown);
        assert_eq!(panel.handle(Action::DebugExpand), Some(Step::Fetch(11)));
        panel.set_variables(11, vec![variable("0", "1", "int", 0)]);
        assert_eq!(names(&panel), ["Locals", "n", "items", "0"]);
        assert_eq!(panel.rows()[3].depth, 2);
        // Loaded once, so expanding again asks for nothing.
        panel.handle(Action::DebugCollapse);
        assert_eq!(names(&panel), ["Locals", "n", "items"]);
        assert_eq!(panel.handle(Action::DebugActivate), None);
        assert_eq!(names(&panel), ["Locals", "n", "items", "0"]);
        // Left on a leaf goes to its parent.
        panel.handle(Action::DebugDown);
        panel.handle(Action::DebugCollapse);
        assert_eq!(panel.var_row, 2);
    }

    #[test]
    fn enter_on_a_frame_shows_it_and_drops_the_old_variables() {
        let mut panel = paused();
        panel.handle(Action::DebugDown);
        let step = panel.handle(Action::DebugActivate);
        assert_eq!(step, Some(Step::Frame(frame(2, "main", "/p/main.py", 9))));
        assert!(panel.rows().is_empty());
        // A late answer for the frame no longer shown is dropped.
        assert!(panel.set_scopes(1, vec![scope("Locals", 10)]).is_empty());
        assert_eq!(panel.set_scopes(2, vec![scope("Locals", 20)]), vec![20]);
    }

    #[test]
    fn up_moves_back_and_stops_at_the_top() {
        let mut panel = paused();
        panel.handle(Action::DebugDown);
        panel.handle(Action::DebugUp);
        panel.handle(Action::DebugUp);
        let step = panel.handle(Action::DebugActivate);
        assert_eq!(step, Some(Step::Frame(frame(1, "inner", "/p/main.py", 3))));

        let mut panel = paused();
        panel.handle(Action::DebugSwitchPane);
        panel.handle(Action::DebugDown);
        panel.handle(Action::DebugDown);
        panel.handle(Action::DebugUp);
        // On `n`, which has no children, not on `items`.
        assert_eq!(panel.handle(Action::DebugExpand), None);
        panel.handle(Action::DebugDown);
        assert_eq!(panel.handle(Action::DebugExpand), Some(Step::Fetch(11)));
    }

    fn draw(theme: &Theme, panel: &DebugPanel, focused: bool) -> anyhow::Result<Buffer> {
        draw_at(theme, panel, (true, focused), 80)
    }

    fn draw_at(
        theme: &Theme,
        panel: &DebugPanel,
        (paused, focused): (bool, bool),
        width: u16,
    ) -> anyhow::Result<Buffer> {
        let mut terminal = Terminal::new(TestBackend::new(width, 6))?;
        let view = DebugView {
            panel,
            run: None,
            paused,
            focused,
        };
        terminal.draw(|f| {
            render_debug_panel(Look::default(), theme, &view, f.area(), f);
        })?;
        Ok(terminal.backend().buffer().clone())
    }

    /// What `hints` read as on the title row: each key between the two ends
    /// of its cap, a blank and its word, three blanks between hints.
    fn hint_text(hints: &Hints, (left, right): (&str, &str)) -> String {
        let each: Vec<String> = hints
            .iter()
            .map(|(key, word)| format!("{left}{key}{right} {word}"))
            .collect();
        each.join("   ")
    }

    const CAPS: (&str, &str) = (kit::CAP_L, kit::CAP_R);
    /// At 80 cells the stack side is columns 0 to 31, the divider column 32
    /// and the variables side columns 33 to 79.
    const STACK_END: u16 = 31;
    const VARS_X: u16 = 33;
    const VARS_END: u16 = 79;

    fn is_rgb(color: ratatui::style::Color) -> bool {
        matches!(color, ratatui::style::Color::Rgb(..))
    }

    fn row_text(buffer: &Buffer, y: u16) -> String {
        (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect()
    }

    #[test]
    fn frames_on_the_left_and_variables_on_the_right() -> anyhow::Result<()> {
        let theme = Theme::default();
        let buffer = draw(&theme, &paused(), false)?;
        assert!(row_text(&buffer, 0).starts_with(" ‖ debug  paused"));
        assert!(row_text(&buffer, 1).contains("call stack"));
        assert!(row_text(&buffer, 1).contains("variables"));
        let top = row_text(&buffer, 2);
        assert!(top.contains("▶ inner  main.py:3"), "{top:?}");
        assert!(top.contains("▾ Locals"), "{top:?}");
        assert!(row_text(&buffer, 3).contains("main  main.py:9"));
        assert!(row_text(&buffer, 3).contains("n = 3  int"));
        assert!(row_text(&buffer, 4).contains("▸ items = [1, 2]  list"));
        // Every row's text starts three cells into its side, after the mark.
        let mark = kit::SEL_MARK;
        assert!(
            top.starts_with(&format!("{}{mark} ▶ inner", kit::CAP_L)),
            "{top:?}"
        );
        assert!(row_text(&buffer, 3).starts_with("     main  main.py:9"));
        assert_eq!(buffer[(VARS_X + 3, 2)].symbol(), "▾");
        assert_eq!(buffer[(VARS_X + 7, 3)].symbol(), "n");
        Ok(())
    }

    #[test]
    fn the_selected_frame_is_a_pill_of_hov_on_the_panels_bg() -> anyhow::Result<()> {
        let theme = Theme::default();
        let buffer = draw(&theme, &paused(), false)?;
        // Its two ends are the fill on the ground, at the edges of the side.
        for (x, cap) in [(0, kit::CAP_L), (STACK_END, kit::CAP_R)] {
            let end = &buffer[(x, 2)];
            assert_eq!(end.symbol(), cap, "{x}");
            assert_eq!((end.fg, end.bg), (theme.hov, theme.bg), "{x}");
        }
        for x in 1..STACK_END {
            assert_eq!(buffer[(x, 2)].bg, theme.hov, "{x}");
        }
        let mark = &buffer[(1, 2)];
        assert_eq!(mark.symbol(), kit::SEL_MARK);
        assert_eq!(mark.fg, theme.accent);
        assert!(mark.modifier.contains(Modifier::BOLD));
        // The shown frame's marker keeps its colour, the name is `strong`
        // bold, and the location `text`, which reads on the fill.
        assert_eq!(buffer[(3, 2)].symbol(), "▶");
        assert_eq!(buffer[(3, 2)].fg, theme.accent2);
        assert_eq!(buffer[(5, 2)].symbol(), "i");
        assert_eq!(buffer[(5, 2)].fg, theme.strong);
        assert!(buffer[(5, 2)].modifier.contains(Modifier::BOLD));
        assert_eq!(buffer[(12, 2)].symbol(), "m");
        assert_eq!(buffer[(12, 2)].fg, theme.text);
        // The divider is outside the pill, and the other frame rests on `bg`.
        assert_eq!(buffer[(STACK_END + 1, 2)].symbol(), "│");
        assert_eq!(buffer[(STACK_END + 1, 2)].bg, theme.bg);
        for x in 0..=STACK_END {
            assert_eq!(buffer[(x, 3)].bg, theme.bg, "{x}");
        }
        assert_eq!(buffer[(5, 3)].fg, theme.text);
        assert!(!buffer[(5, 3)].modifier.contains(Modifier::BOLD));
        assert_eq!(buffer[(11, 3)].symbol(), "m");
        assert_eq!(buffer[(11, 3)].fg, theme.muted);
        Ok(())
    }

    #[test]
    fn a_variable_is_a_pill_only_while_its_side_has_the_keys() -> anyhow::Result<()> {
        let theme = Theme::default();
        let mut panel = paused();
        panel.handle(Action::DebugSwitchPane);
        panel.handle(Action::DebugDown);
        let lit = |buffer: &Buffer, y: u16| {
            (VARS_X..=VARS_END)
                .filter(|&x| buffer[(x, y)].bg == theme.hov || buffer[(x, y)].fg == theme.hov)
                .count()
        };
        // Without the keys the variables side shows no selection at all.
        let buffer = draw(&theme, &panel, false)?;
        for y in 2..5 {
            assert_eq!(lit(&buffer, y), 0, "{y}");
        }

        let buffer = draw(&theme, &panel, true)?;
        assert_eq!(lit(&buffer, 2), 0);
        assert_eq!(lit(&buffer, 3), usize::from(VARS_END - VARS_X + 1));
        assert_eq!(lit(&buffer, 4), 0);
        for (x, cap) in [(VARS_X, kit::CAP_L), (VARS_END, kit::CAP_R)] {
            let end = &buffer[(x, 3)];
            assert_eq!(end.symbol(), cap, "{x}");
            assert_eq!((end.fg, end.bg), (theme.hov, theme.bg), "{x}");
        }
        let mark = &buffer[(VARS_X + 1, 3)];
        assert_eq!(mark.symbol(), kit::SEL_MARK);
        assert_eq!((mark.fg, mark.bg), (theme.accent, theme.hov));
        // `n = 3  int`, a level in: the name `strong` bold, the rest `text`.
        let at = VARS_X + 7;
        let text: String = (at..at + 10).map(|x| buffer[(x, 3)].symbol()).collect();
        assert_eq!(text, "n = 3  int");
        assert_eq!(buffer[(at, 3)].fg, theme.strong);
        assert!(buffer[(at, 3)].modifier.contains(Modifier::BOLD));
        for x in [at + 2, at + 4, at + 7] {
            assert_eq!(buffer[(x, 3)].fg, theme.text, "{x}");
            assert_eq!(buffer[(x, 3)].bg, theme.hov, "{x}");
        }
        // The stack keeps its selection beside it.
        assert_eq!(buffer[(1, 2)].symbol(), kit::SEL_MARK);
        assert_eq!(buffer[(5, 2)].bg, theme.hov);
        // A resting variable keeps the colours it had.
        assert_eq!(buffer[(VARS_X + 3, 2)].fg, theme.muted);
        assert_eq!(buffer[(VARS_X + 5, 2)].fg, theme.text);
        assert!(buffer[(VARS_X + 5, 2)].modifier.contains(Modifier::BOLD));
        assert_eq!(buffer[(VARS_X + 7, 4)].fg, theme.strong);
        assert!(!buffer[(VARS_X + 7, 4)].modifier.contains(Modifier::BOLD));
        Ok(())
    }

    #[test]
    fn the_titles_hints_are_keycaps_and_go_when_there_is_no_room() -> anyhow::Result<()> {
        let theme = Theme::default();
        let panel = paused();
        let buffer = draw(&theme, &panel, false)?;
        let hints = hint_text(PAUSED_HINTS, CAPS);
        assert_eq!(hints.chars().count(), 46);
        let title = row_text(&buffer, 0);
        assert!(title.ends_with(&format!("   {hints} ")), "{title:?}");
        // The state keeps its colour; a key is `strong` bold on `btn`, its
        // ends `btn` on the row's `surface`, and its word `muted`.
        assert_eq!(buffer[(1, 0)].fg, theme.accent2);
        assert_eq!(buffer[(10, 0)].fg, theme.accent2);
        let at = 80 - 1 - 46;
        let (left, key, right) = (&buffer[(at, 0)], &buffer[(at + 1, 0)], &buffer[(at + 4, 0)]);
        assert_eq!((left.symbol(), right.symbol()), CAPS);
        assert_eq!((left.fg, left.bg), (theme.btn, theme.surface));
        assert_eq!((right.fg, right.bg), (theme.btn, theme.surface));
        assert_eq!(
            (key.symbol(), key.fg, key.bg),
            ("t", theme.strong, theme.btn)
        );
        assert!(key.modifier.contains(Modifier::BOLD));
        let word = &buffer[(at + 6, 0)];
        assert_eq!((word.symbol(), word.fg), ("s", theme.muted));
        assert_eq!(word.bg, theme.surface);

        // ` ‖ debug  paused` is 16 cells and the hints keep three clear of it.
        let title = row_text(&draw_at(&theme, &panel, (true, false), 66)?, 0);
        assert_eq!(title, format!(" ‖ debug  paused   {hints} "));
        let title = row_text(&draw_at(&theme, &panel, (true, false), 65)?, 0);
        assert_eq!(title.trim_end(), " ‖ debug  paused");

        // Running, the keys are the ones that stop it.
        let buffer = draw_at(&theme, &panel, (false, false), 80)?;
        let title = row_text(&buffer, 0);
        assert!(title.starts_with(" ● debug  running"), "{title:?}");
        let hints = hint_text(RUNNING_HINTS, CAPS);
        assert!(title.ends_with(&format!("   {hints} ")), "{title:?}");
        assert_eq!(buffer[(1, 0)].fg, theme.warn);
        Ok(())
    }

    #[test]
    fn mono_selects_in_reverse_video() -> anyhow::Result<()> {
        let mono = Theme::named("mono").expect("mono exists");
        let mut panel = paused();
        let buffer = draw(&mono, &panel, true)?;
        // The whole row, edge to edge of its side, with the mark and no caps.
        for x in 0..=STACK_END {
            assert!(buffer[(x, 2)].modifier.contains(Modifier::REVERSED), "{x}");
            assert!(!buffer[(x, 3)].modifier.contains(Modifier::REVERSED), "{x}");
        }
        assert!(
            !buffer[(STACK_END + 1, 2)]
                .modifier
                .contains(Modifier::REVERSED)
        );
        assert!(row_text(&buffer, 2).starts_with(&format!(" {} ▶ inner", kit::SEL_MARK)));
        // The title's keycaps are bold text between blanks.
        let title = row_text(&buffer, 0);
        let hints = hint_text(PAUSED_HINTS, (" ", " "));
        assert!(title.ends_with(&format!("   {hints} ")), "{title:?}");
        let at = 80 - 1 - 46;
        for x in at + 1..at + 4 {
            assert!(buffer[(x, 0)].modifier.contains(Modifier::BOLD), "{x}");
        }
        assert!(!buffer[(at + 6, 0)].modifier.contains(Modifier::BOLD));
        // No Nerd Font glyph and no blended colour anywhere in the panel.
        for y in 0..6 {
            let text = row_text(&buffer, y);
            assert!(
                !text.contains(kit::CAP_L) && !text.contains(kit::CAP_R),
                "{text:?}"
            );
            for x in 0..80 {
                let cell = &buffer[(x, y)];
                assert!(!is_rgb(cell.fg) && !is_rgb(cell.bg), "{x},{y}");
            }
        }

        panel.handle(Action::DebugSwitchPane);
        panel.handle(Action::DebugDown);
        let buffer = draw(&mono, &panel, true)?;
        // Past the stack side, where `main` also ends in an `n`.
        let x = (33..80)
            .find(|&x| buffer[(x, 3)].symbol() == "n" && buffer[(x + 1, 3)].symbol() == " ")
            .expect("n = 3 on row 3");
        assert!(buffer[(x, 3)].modifier.contains(Modifier::REVERSED));
        // Its text keeps no colours of its own, so the reversal reads.
        assert_eq!(buffer[(x, 3)].fg, mono.hov);
        Ok(())
    }
}
