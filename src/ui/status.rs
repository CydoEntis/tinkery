//! The status bar on the last row (design README §2.5, tinkery-forge spec F12):
//! the brand block on its ramp, then everything flat on `surface`: the path
//! slot at x = 20, and on the right the debug session's state, the cursor
//! position, the language, its server's state and the diagnostic counts — or,
//! while the splash is up, its keys as keycap hints and Tinkery's version.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use unicode_width::UnicodeWidthStr;

use crate::lsp::ServerState;
use crate::theme::{Theme, grad, mix};
use crate::ui::kit::{self, Look};

/// Cells the brand block covers: ten on the accent ramp, then eight fading into
/// the bar.
const BLOCK_WIDTH: u16 = 18;
/// Where the path (or the diagnostic under the cursor in its place) starts.
const PATH_X: u16 = 20;
/// Between the right-hand segments (README §2.5).
const GAP: &str = "    ";

/// How a note or diagnostic went, which picks its glyph and colour.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    #[default]
    Ok,
    Warn,
    Err,
}

impl Tone {
    pub fn glyph(self) -> &'static str {
        match self {
            Tone::Ok => "✓",
            Tone::Warn => "⚠",
            Tone::Err => "✕",
        }
    }

    pub fn color(self, theme: &Theme) -> Color {
        match self {
            Tone::Ok => theme.ok,
            Tone::Warn => theme.warn,
            Tone::Err => theme.err,
        }
    }
}

/// Where a debug session is, for its segment (tinkery-debugger spec D4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Debugging<'a> {
    /// The program is running (or starting).
    Running,
    /// The program is stopped; the text is where, `main.rs:12`, or empty when
    /// the adapter didn't say.
    Paused(&'a str),
}

/// What the status line shows about the active buffer.
#[derive(Debug, Default, Clone, Copy)]
pub struct Status<'a> {
    /// The buffer's path as the user knows it (relative to the project), or
    /// `untitled`.
    pub path: &'a str,
    /// Unsaved changes, shown as ` •` after the name.
    pub dirty: bool,
    /// The diagnostic under the cursor, which takes the path's place. A note
    /// from an action is a toast instead (`ui::toast`).
    pub message: Option<(Tone, &'a str)>,
    /// The cursor as 0-based (line, char column).
    pub position: (usize, usize),
    /// The buffer's language as people write it (`Rust`, `Plain text`).
    pub language: &'a str,
    /// The state and name of the server following the buffer; `None` for a
    /// language without one.
    pub server: Option<(ServerState, &'a str)>,
    /// The language server's warnings and errors for the buffer.
    pub warnings: usize,
    pub errors: usize,
    /// The splash is up (tinkery-splash spec S3): there's no cursor or language to
    /// report, so the right side says how to drive the splash instead.
    pub splash: bool,
    /// The debug session's state, while there is one. It leads the right side
    /// and is never dropped for room: it says the program is still alive.
    pub debug: Option<Debugging<'a>>,
}

/// A piece of text and how to draw it.
type Run = (String, Style);

/// Which optional pieces fit at a width. As the row narrows they go in this
/// order: server, language, the path's directory (README §2.5, SPEC_V1_LAYOUT
/// §11), because the file name and position matter most while editing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Shown {
    server: bool,
    language: bool,
    dir: bool,
}

/// The fullest `Shown` whose left and right sides fit side by side in `width`.
/// When even the leanest doesn't fit, the left side is cut as drawn.
fn layout(look: Look, theme: &Theme, status: &Status, width: u16) -> Shown {
    let shown = |server, language, dir| Shown {
        server,
        language,
        dir,
    };
    // On the splash `language` stands for its keys, which say more than the
    // project's parent folders, so the directory goes first there.
    let steps = if status.splash {
        [shown(false, true, true), shown(false, true, false)]
    } else {
        [shown(true, true, true), shown(false, true, true)]
    };
    let last = if status.splash {
        shown(false, false, false)
    } else {
        shown(false, false, true)
    };
    let leanest = shown(false, false, false);
    steps
        .into_iter()
        .chain([last])
        .find(|&shown| {
            let left = runs_width(&left_runs(theme, status, shown.dir));
            let right = runs_width(&right_runs(look, theme, status, shown));
            // The left slot stops a cell short of the right side, which ends at W − 2.
            u32::from(PATH_X) + u32::from(left) + 1 + u32::from(right) + 2 <= u32::from(width)
        })
        .unwrap_or(leanest)
}

/// Draws the one-row status bar.
pub fn render_status(frame: &mut Frame, look: Look, theme: &Theme, area: Rect, status: &Status) {
    let buf = frame.buffer_mut();
    let bar = Style::new().fg(theme.text).bg(theme.surface);
    buf.set_style(area, bar);
    let right_edge = area.x + area.width;

    let block = BLOCK_WIDTH.min(area.width);
    for i in 0..block {
        buf[(area.x + i, area.y)].set_bg(block_bg(theme, i));
    }
    put(
        buf,
        area.x + 1,
        area.y,
        right_edge,
        &[(
            "✦ tinkery".to_string(),
            Style::new().fg(theme.acc_ink).add_modifier(Modifier::BOLD),
        )],
    );

    let shown = layout(look, theme, status, area.width);
    let right = right_runs(look, theme, status, shown);
    // Right-aligned to W − 2 like the design's `putRight`, which ends just before it.
    let right_x = right_edge
        .saturating_sub(2)
        .saturating_sub(runs_width(&right))
        .max(area.x);
    // The left slot stops a cell short of the right side so they never touch.
    let left_end = right_x.saturating_sub(1);
    let left = left_runs(theme, status, shown.dir);
    put(buf, area.x + PATH_X, area.y, left_end, &left);
    put(buf, right_x, area.y, right_edge, &right);
}

/// The block's bg at cell `i` (README §2.5). Themes without RGB colours (`mono`)
/// can't ramp, so the block is flat `accent` there.
fn block_bg(theme: &Theme, i: u16) -> Color {
    let rgb = |c: Color| matches!(c, Color::Rgb(..));
    if !(rgb(theme.accent) && rgb(theme.accent2) && rgb(theme.surface)) {
        return theme.accent;
    }
    let i = f64::from(i);
    if i < 10.0 {
        grad(&[theme.accent, theme.accent2], i / 9.0)
    } else {
        mix(theme.accent2, theme.surface, (i - 9.0) / 9.0)
    }
}

/// The path slot: the diagnostic with its glyph while there is one, else the path,
/// with its directory only when `dir`.
fn left_runs(theme: &Theme, status: &Status, dir: bool) -> Vec<Run> {
    if let Some((tone, text)) = status.message {
        let style = Style::new().fg(tone.color(theme));
        return vec![
            (format!("{} ", tone.glyph()), style),
            (text.to_string(), Style::new().fg(theme.text)),
        ];
    }
    // Either separator, so a Windows path splits the same as a Unix one.
    let split = status.path.rfind(['/', '\\']).map_or(0, |i| i + 1);
    // The splash's project is a quiet label, all muted (design/splash.png);
    // only a file's name is lit.
    if status.splash {
        let shown = if dir {
            status.path
        } else {
            &status.path[split..]
        };
        return vec![(shown.to_string(), Style::new().fg(theme.muted))];
    }
    let (directory, name) = status.path.split_at(split);
    let mut runs = Vec::new();
    if dir {
        runs.push((directory.to_string(), Style::new().fg(theme.muted)));
    }
    runs.push((name.to_string(), Style::new().fg(theme.strong)));
    if status.dirty {
        runs.push((" •".to_string(), Style::new().fg(theme.warn)));
    }
    runs
}

/// The position, the language and its server as `shown` allows, then the counts
/// while the server reports anything.
fn right_runs(look: Look, theme: &Theme, status: &Status, shown: Shown) -> Vec<Run> {
    let text = Style::new().fg(theme.text);
    if status.splash {
        return splash_runs(look, theme, shown.language);
    }
    let gap = || (GAP.to_string(), Style::new());
    let mut runs = Vec::new();
    match status.debug {
        // `●` in the run panel's running colour, so the two read as one state.
        Some(Debugging::Running) => {
            runs.push(("● ".to_string(), Style::new().fg(theme.warn)));
            runs.push(("debugging".to_string(), text));
            runs.push(gap());
        }
        // `accent2`, the paused line's own colour in the editor.
        Some(Debugging::Paused(at)) => {
            runs.push(("‖ ".to_string(), Style::new().fg(theme.accent2)));
            let label = if at.is_empty() {
                "paused".to_string()
            } else {
                format!("paused {at}")
            };
            runs.push((label, text));
            runs.push(gap());
        }
        None => {}
    }
    let (line, col) = status.position;
    runs.push((format!("Ln {}, Col {}", line + 1, col + 1), text));
    if shown.language {
        runs.push(gap());
        runs.push((status.language.to_string(), text));
    }
    if shown.server
        && let Some((state, name)) = status.server
    {
        let (glyph, glyph_color, label, label_color) = match state {
            ServerState::Ready => ("● ", theme.ok, name, theme.text),
            ServerState::Starting => ("○ ", theme.warn, name, theme.text),
            ServerState::NotFound => ("○ ", theme.muted, "no server", theme.muted),
            ServerState::Crashed => ("✕ ", theme.err, name, theme.err),
        };
        runs.push(gap());
        runs.push((glyph.to_string(), Style::new().fg(glyph_color)));
        runs.push((label.to_string(), Style::new().fg(label_color)));
    }
    if status.warnings + status.errors > 0 {
        runs.push(gap());
        runs.push((format!("✕ {}", status.errors), Style::new().fg(theme.err)));
        runs.push(("  ".to_string(), Style::new()));
        runs.push((
            format!("⚠ {}", status.warnings),
            Style::new().fg(theme.warn),
        ));
    }
    runs
}

/// What the splash's keys do, in the order the bar lists them.
const SPLASH_HINTS: [(&str, &str); 3] = [("↑↓", "select"), ("⏎", "choose"), ("q", "quit")];

/// The splash's keys as keycap hints on the bar, then the version. The keys go
/// where the language would, so a long message or path drops them before it's
/// cut.
fn splash_runs(look: Look, theme: &Theme, keys: bool) -> Vec<Run> {
    let mut runs = Vec::new();
    if keys {
        runs.extend(
            kit::hints(look, theme, &SPLASH_HINTS, theme.surface)
                .into_iter()
                .map(|span| (span.content.into_owned(), span.style)),
        );
        // Wider than GAP: design/splash.png sets the version six cells clear
        // of `q quit`.
        runs.push((" ".repeat(6), Style::new()));
    }
    runs.push((
        format!("v{}", env!("CARGO_PKG_VERSION")),
        Style::new().fg(theme.muted),
    ));
    runs
}

/// Writes `runs` from `x`, cutting at `end`. Styles patch the bar's, so a run
/// without a bg keeps the cell's.
fn put(buf: &mut Buffer, mut x: u16, y: u16, end: u16, runs: &[Run]) {
    for (text, style) in runs {
        if x >= end {
            return;
        }
        let (next, _) = buf.set_stringn(x, y, text, usize::from(end - x), *style);
        x = next;
    }
}

fn runs_width(runs: &[Run]) -> u16 {
    runs.iter().map(|(s, _)| width(s)).sum()
}

fn width(s: &str) -> u16 {
    u16::try_from(s.width()).unwrap_or(u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOOK: Look = Look {
        pill_caps: true,
        corners: crate::config::Corners::Flush,
    };

    #[test]
    fn the_block_ramps_from_accent_to_accent2_then_fades_into_the_bar() {
        let theme = Theme::named("tinkery").expect("tinkery exists");
        assert_eq!(block_bg(&theme, 0), theme.accent);
        assert_eq!(block_bg(&theme, 9), theme.accent2);
        assert_eq!(
            block_bg(&theme, 10),
            mix(theme.accent2, theme.surface, 1.0 / 9.0)
        );
        assert_eq!(
            block_bg(&theme, 17),
            mix(theme.accent2, theme.surface, 8.0 / 9.0)
        );
    }

    fn text(runs: &[Run]) -> String {
        runs.iter().map(|(s, _)| s.as_str()).collect()
    }

    fn shown(server: bool, language: bool, dir: bool) -> Shown {
        Shown {
            server,
            language,
            dir,
        }
    }

    #[test]
    fn narrowing_drops_the_server_then_the_language_then_the_directory() {
        let theme = Theme::named("tinkery").expect("tinkery exists");
        let status = Status {
            path: "src/ui/status.rs",
            dirty: true,
            position: (51, 21),
            language: "Rust",
            server: Some((ServerState::Ready, "rust-analyzer")),
            ..Status::default()
        };
        assert_eq!(
            text(&right_runs(LOOK, &theme, &status, shown(true, true, true))),
            "Ln 52, Col 22    Rust    ● rust-analyzer"
        );
        // Path 16 + dirty mark 2 = 18; right side 13, +8 for the language, +19
        // for the server. Each width is 20 + left + 1 + right + 2.
        let full = 20 + 18 + 1 + (13 + 8 + 19) + 2;
        assert_eq!(
            layout(LOOK, &theme, &status, full + 10),
            shown(true, true, true)
        );
        assert_eq!(layout(LOOK, &theme, &status, full), shown(true, true, true));
        assert_eq!(
            layout(LOOK, &theme, &status, full - 1),
            shown(false, true, true)
        );
        assert_eq!(
            layout(LOOK, &theme, &status, full - 19),
            shown(false, true, true)
        );
        assert_eq!(
            layout(LOOK, &theme, &status, full - 20),
            shown(false, false, true)
        );
        assert_eq!(
            layout(LOOK, &theme, &status, full - 27),
            shown(false, false, true)
        );
        assert_eq!(
            layout(LOOK, &theme, &status, full - 28),
            shown(false, false, false)
        );
        assert_eq!(
            layout(LOOK, &theme, &status, 10),
            shown(false, false, false)
        );

        let lean = left_runs(&theme, &status, false);
        assert_eq!(text(&lean), "status.rs •");
        assert_eq!(lean[0].1.fg, Some(theme.strong));
        assert_eq!(
            text(&left_runs(&theme, &status, true)),
            "src/ui/status.rs •"
        );
    }

    #[test]
    fn each_server_state_has_its_glyph_and_colours() {
        let theme = Theme::named("tinkery").expect("tinkery exists");
        let server = |state| {
            let status = Status {
                language: "Rust",
                server: Some((state, "rust-analyzer")),
                ..Status::default()
            };
            let runs = right_runs(LOOK, &theme, &status, shown(true, true, true));
            let n = runs.len();
            (text(&runs[n - 2..]), runs[n - 2].1.fg, runs[n - 1].1.fg)
        };
        assert_eq!(
            server(ServerState::Ready),
            (
                "● rust-analyzer".to_string(),
                Some(theme.ok),
                Some(theme.text)
            )
        );
        assert_eq!(
            server(ServerState::Starting),
            (
                "○ rust-analyzer".to_string(),
                Some(theme.warn),
                Some(theme.text)
            )
        );
        assert_eq!(
            server(ServerState::NotFound),
            (
                "○ no server".to_string(),
                Some(theme.muted),
                Some(theme.muted)
            )
        );
        assert_eq!(
            server(ServerState::Crashed),
            (
                "✕ rust-analyzer".to_string(),
                Some(theme.err),
                Some(theme.err)
            )
        );

        // No server configured: no segment at all.
        let status = Status {
            language: "Plain text",
            ..Status::default()
        };
        assert_eq!(
            text(&right_runs(LOOK, &theme, &status, shown(true, true, true))),
            "Ln 1, Col 1    Plain text"
        );
    }

    #[test]
    fn a_message_drops_the_server_and_language_before_it_is_cut() {
        let theme = Theme::named("tinkery").expect("tinkery exists");
        let status = Status {
            message: Some((Tone::Warn, "rust: server not found (rust-analyzer)")),
            language: "Rust",
            server: Some((ServerState::NotFound, "rust-analyzer")),
            ..Status::default()
        };
        // Message 40, position 11, language 8, server 15.
        assert_eq!(
            layout(LOOK, &theme, &status, 20 + 40 + 1 + 34 + 2),
            shown(true, true, true)
        );
        assert_eq!(
            layout(LOOK, &theme, &status, 20 + 40 + 1 + 19 + 2),
            shown(false, true, true)
        );
        assert_eq!(
            layout(LOOK, &theme, &status, 20 + 40 + 1 + 18 + 2),
            shown(false, false, true)
        );
    }

    #[test]
    fn the_splash_shows_its_keys_and_the_version_instead_of_the_cursor() {
        let theme = Theme::named("tinkery").expect("tinkery exists");
        let status = Status {
            path: "~/src",
            language: "Plain text",
            splash: true,
            ..Status::default()
        };
        let (l, r) = (kit::CAP_L, kit::CAP_R);
        let keys = right_runs(LOOK, &theme, &status, shown(true, true, true));
        assert_eq!(
            text(&keys),
            format!(
                "{l}↑↓{r} select   {l}⏎{r} choose   {l}q{r} quit      v{}",
                env!("CARGO_PKG_VERSION")
            )
        );
        // Each key is a keycap: round ends in `btn` on the bar, the key in
        // `strong` bold on `btn`; what it does is muted on the bar.
        assert_eq!(text(&keys[..4]), format!("{l}↑↓{r} select"));
        for end in [&keys[0], &keys[2]] {
            assert_eq!((end.1.fg, end.1.bg), (Some(theme.btn), Some(theme.surface)));
        }
        assert_eq!(
            (keys[1].1.fg, keys[1].1.bg),
            (Some(theme.strong), Some(theme.btn))
        );
        assert!(keys[1].1.add_modifier.contains(Modifier::BOLD));
        assert_eq!(
            (keys[3].1.fg, keys[3].1.bg),
            (Some(theme.muted), Some(theme.surface))
        );
        let version = keys.last().expect("the version");
        assert_eq!(version.1.fg, Some(theme.muted));
        // Squared off, the ends are blanks of `btn` and nothing moves.
        let square = Look {
            pill_caps: false,
            corners: crate::config::Corners::Flush,
        };
        let squared = right_runs(square, &theme, &status, shown(true, true, true));
        assert_eq!(
            text(&squared),
            format!(
                " ↑↓  select    ⏎  choose    q  quit      v{}",
                env!("CARGO_PKG_VERSION")
            )
        );
        assert_eq!(squared[0].1.bg, Some(theme.btn));
        assert_eq!(runs_width(&squared), runs_width(&keys));
        assert_eq!(text(&left_runs(&theme, &status, true)), "~/src");
        // The whole project path is muted, its last folder too.
        assert!(
            left_runs(&theme, &status, true)
                .iter()
                .all(|(_, style)| style.fg == Some(theme.muted))
        );
        // Too narrow for the keys beside a message: only the version stays.
        assert_eq!(
            text(&right_runs(
                LOOK,
                &theme,
                &status,
                shown(false, false, true)
            )),
            format!("v{}", env!("CARGO_PKG_VERSION"))
        );

        // A long path loses its directory before the keys go.
        let status = Status {
            path: "/home/someone/work/clients/project",
            splash: true,
            ..Status::default()
        };
        // Path 34, name 7; right side 35 of hints + 6 + 6 = 47.
        assert_eq!(
            layout(LOOK, &theme, &status, 20 + 34 + 1 + 47 + 2),
            shown(false, true, true)
        );
        assert_eq!(
            layout(LOOK, &theme, &status, 20 + 34 + 1 + 47 + 1),
            shown(false, true, false)
        );
        assert_eq!(
            layout(LOOK, &theme, &status, 20 + 7 + 1 + 47 + 2),
            shown(false, true, false)
        );
        assert_eq!(
            layout(LOOK, &theme, &status, 20 + 7 + 1 + 47 + 1),
            shown(false, false, false)
        );
        // The harness's 100 columns hold the keys beside a path of up to 30
        // cells whole.
        assert_eq!(20 + 30 + 1 + 47 + 2, 100);
    }

    #[test]
    fn a_debug_session_leads_the_right_side() {
        let theme = Theme::named("tinkery").expect("tinkery exists");
        let status = |debug| Status {
            language: "Rust",
            debug: Some(debug),
            ..Status::default()
        };
        let running = right_runs(
            LOOK,
            &theme,
            &status(Debugging::Running),
            shown(false, true, true),
        );
        assert_eq!(text(&running), "● debugging    Ln 1, Col 1    Rust");
        assert_eq!(running[0].1.fg, Some(theme.warn));
        let paused = right_runs(
            LOOK,
            &theme,
            &status(Debugging::Paused("main.rs:12")),
            shown(false, false, true),
        );
        assert_eq!(text(&paused), "‖ paused main.rs:12    Ln 1, Col 1");
        assert_eq!(paused[0].1.fg, Some(theme.accent2));
        assert_eq!(paused[1].1.fg, Some(theme.text));
        let nowhere = right_runs(
            LOOK,
            &theme,
            &status(Debugging::Paused("")),
            shown(false, false, true),
        );
        assert_eq!(text(&nowhere), "‖ paused    Ln 1, Col 1");
    }

    #[test]
    fn mono_has_a_flat_accent_block() {
        let theme = Theme::named("mono").expect("mono exists");
        for i in 0..BLOCK_WIDTH {
            assert_eq!(block_bg(&theme, i), theme.accent);
        }
    }

    #[test]
    fn mono_draws_the_splash_keys_without_cap_glyphs() {
        let theme = Theme::named("mono").expect("mono exists");
        let keys = splash_runs(LOOK, &theme, true);
        let line = text(&keys);
        assert!(line.starts_with(" ↑↓  select    ⏎  choose    q  quit"));
        assert!(!line.contains(kit::CAP_L) && !line.contains(kit::CAP_R));
        assert_eq!(
            (keys[1].1.fg, keys[1].1.bg),
            (Some(theme.strong), Some(theme.btn))
        );
    }

    /// The bar as `render_status` draws it, `width` cells wide.
    fn drawn(theme: &Theme, status: &Status, width: u16) -> Buffer {
        let backend = ratatui::backend::TestBackend::new(width, 1);
        let mut terminal = ratatui::Terminal::new(backend).expect("a test terminal");
        terminal
            .draw(|frame| render_status(frame, LOOK, theme, frame.area(), status))
            .expect("draw the bar");
        terminal.backend().buffer().clone()
    }

    #[test]
    fn right_of_the_block_every_cell_is_surface_but_the_keycaps() {
        let theme = Theme::named("tinkery").expect("tinkery exists");
        let file = Status {
            path: "src/ui/status.rs",
            dirty: true,
            language: "Rust",
            server: Some((ServerState::Ready, "rust-analyzer")),
            warnings: 2,
            errors: 1,
            debug: Some(Debugging::Paused("main.rs:12")),
            ..Status::default()
        };
        let message = Status {
            message: Some((Tone::Err, "could not save")),
            ..file
        };
        for status in [file, message] {
            let buf = drawn(&theme, &status, 120);
            for x in BLOCK_WIDTH..120 {
                assert_eq!(buf[(x, 0)].bg, theme.surface, "column {x}");
            }
        }
        // On the splash only the keys' own cells leave the bar's colour.
        let splash = Status {
            path: "~/src",
            splash: true,
            ..Status::default()
        };
        let buf = drawn(&theme, &splash, 100);
        let row: String = (0..100).map(|x| buf[(x, 0)].symbol()).collect();
        let caps: Vec<u16> = (BLOCK_WIDTH..100)
            .filter(|&x| buf[(x, 0)].bg != theme.surface)
            .collect();
        assert!(caps.iter().all(|&x| buf[(x, 0)].bg == theme.btn), "{row}");
        let on_btn: String = caps.iter().map(|&x| buf[(x, 0)].symbol()).collect();
        assert_eq!(on_btn, "↑↓⏎q", "{row}");
        // The brand keeps its ramp and its bold `acc_ink` name.
        assert_eq!(buf[(0, 0)].bg, theme.accent);
        assert_eq!(buf[(9, 0)].bg, theme.accent2);
        for x in 1..10 {
            let cell = &buf[(x, 0)];
            assert_eq!(cell.fg, theme.acc_ink, "column {x}");
            assert!(cell.modifier.contains(Modifier::BOLD), "column {x}");
        }
    }
}
