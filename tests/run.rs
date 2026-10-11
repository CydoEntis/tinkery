mod harness;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use harness::{CAP_L, CAP_R, ROWS, Tinkery, hints_text};
use vt100::Color;

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(10);
/// `.tinkery.toml` has one entry, `dev`, which runs the `colors` script: a red
/// line, a two second pause, then a line on stderr.
const PROJECT: &str = "tests/fixtures/run-project";
/// The same `dev` entry plus `hello`.
const MULTI: &str = "tests/fixtures/run-project/multi";
/// Runs the `palette` script, which prints the eight ANSI colours.
const COLOURS: &str = "tests/fixtures/run-project/colours";

/// The default theme's (hydra's) roles the panel maps states and ANSI colours to.
const ERR: Color = Color::Rgb(0xff, 0x6b, 0x6b);
const OK: Color = Color::Rgb(0x7f, 0xd9, 0x62);
const WARN: Color = Color::Rgb(0xff, 0xb5, 0x47);
const MUTED: Color = Color::Rgb(0x71, 0x80, 0x8f);
const STRONG: Color = Color::Rgb(0xf2, 0xf6, 0xf8);
const SURFACE: Color = Color::Rgb(0x0c, 0x13, 0x1b);
const BG: Color = Color::Rgb(0x07, 0x0b, 0x10);
/// hydra's `syn.function`, `syn.keyword` and `syn.type`.
const FUNCTION: Color = Color::Rgb(0x5a, 0xa9, 0xff);
const KEYWORD: Color = Color::Rgb(0xa5, 0x93, 0xff);
const TYPE: Color = Color::Rgb(0x3d, 0xd6, 0xc0);

/// The run picker is a panel titled `Run`: at 100x30, 60 wide and 12 tall at
/// (20, 9), with the flush edge its title sits in on row 9, the query's strip
/// on row 11, a row per entry from row 13 and the keys on row 19. An entry's
/// name starts at column 24, its command at 36, and its source ends at 76.
const PICKER_TITLE: &str = "▁ Run ▁";
const PICKER_X: u16 = 20;
const PICKER_Y: u16 = 9;
const PICKER_QUERY_ROW: u16 = 11;
const PICKER_FIRST_ROW: u16 = 13;
const PICKER_HINTS_ROW: u16 = 19;
const PICKER_TEXT_X: u16 = 24;
const PICKER_DETAIL_X: u16 = 36;
const PICKER_RIGHT: u16 = 76;
/// The default theme's (tinkery's) roles the picker is drawn in; the tests
/// that read them run on an empty config.
const PICKER_ACCENT: Color = Color::Rgb(0xff, 0x8a, 0x3d);
const PICKER_RAISED: Color = Color::Rgb(0x1f, 0x1b, 0x19);
const PICKER_RAISED2: Color = Color::Rgb(0x29, 0x23, 0x20);
const PICKER_HOV: Color = Color::Rgb(0x3a, 0x30, 0x2a);
const PICKER_MUTED: Color = Color::Rgb(0x7d, 0x70, 0x67);
const PICKER_TEXT: Color = Color::Rgb(0xb5, 0xa8, 0x9d);
const PICKER_STRONG: Color = Color::Rgb(0xf8, 0xf0, 0xe6);

/// The hints at the right of the title row while running, and once done, with
/// the blank that ends the row.
const RUNNING_HINTS: &str = "shift+F5 stop   ctrl+F5 restart   F4 hide ";
const DONE_HINTS: &str = "F5 run again   F4 hide ";

/// At 30 rows the panel is 9 tall: the editor keeps rows 0 to 19, the panel's
/// title is row 20 and its output starts on row 21, one space in.
const TITLE_ROW: u16 = 20;
const OUTPUT_ROW: u16 = 21;

/// `PATH` with the fixture folder first, so `colors` resolves to the fixture's
/// script under both `sh` and `cmd`, from any folder.
fn fixture_path() -> Vec<(&'static str, OsString)> {
    let fixture = std::path::absolute(PROJECT).expect("fixture path");
    let mut paths: Vec<PathBuf> = vec![fixture];
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path));
    }
    let joined = std::env::join_paths(paths).expect("PATH entries join");
    vec![("PATH", joined)]
}

fn row(tinkery: &Tinkery, row: u16) -> String {
    tinkery.screen()[usize::from(row)].clone()
}

/// Tinkery on the fixture's `main.rs`, so the project root is the fixture folder.
fn open_project() -> Tinkery {
    let tinkery = Tinkery::spawn_in_with_env(Path::new(PROJECT), &fixture_path(), &["main.rs"]);
    tinkery.wait_for_text("the editor stays visible", START);
    tinkery
}

#[test]
fn f5_runs_the_only_entry_and_streams_its_output_in_colour() {
    let mut tinkery = open_project();
    tinkery.send_keys("f5");
    tinkery.wait_for_text("red line", WAIT);
    // The red line arrived while the script is still in its pause.
    let title = row(&tinkery, TITLE_ROW);
    assert!(title.starts_with(" ● dev  running   colors"), "{title:?}");
    // The title on `surface`: `●` and the state word in `warn`, the name in
    // `strong`, the command in `muted`, the running hints ending a cell short of
    // the right edge.
    assert_eq!(tinkery.bg_at(0, TITLE_ROW), SURFACE);
    assert_eq!(tinkery.fg_at(1, TITLE_ROW), WARN);
    assert_eq!(tinkery.fg_at(3, TITLE_ROW), STRONG);
    assert_eq!(tinkery.fg_at(8, TITLE_ROW), WARN);
    assert_eq!(tinkery.fg_at(18, TITLE_ROW), MUTED);
    assert_hints(&tinkery, RUNNING_HINTS);
    // Red output takes the theme's `err`, on `bg`.
    tinkery.wait_for_fg(OUTPUT_ROW, ERR, "red line", WAIT);
    assert_eq!(tinkery.bg_at(1, OUTPUT_ROW), BG);
    assert_eq!(tinkery.text_col(OUTPUT_ROW, "red line"), Some(1));
    // The editor above stays in view.
    assert!(row(&tinkery, 4).contains("the editor stays visible"));

    // stderr comes in too, then the title shows the exit code.
    tinkery.wait_for_screen("the stderr line", WAIT, |screen| {
        screen[usize::from(OUTPUT_ROW) + 1].starts_with(" second line")
    });
    tinkery.wait_for_screen("the exit code in the title", WAIT, |screen| {
        screen[usize::from(TITLE_ROW)].starts_with(" ✓ dev  exited 0")
    });
    assert_eq!(tinkery.fg_at(1, TITLE_ROW), OK);
    assert_eq!(tinkery.fg_at(8, TITLE_ROW), OK);
    assert_hints(&tinkery, DONE_HINTS);
}

/// The title row ends with `hints` and a blank, in `muted`.
fn assert_hints(tinkery: &Tinkery, hints: &str) {
    let at = harness::COLS - u16::try_from(hints.chars().count()).expect("short hints");
    let title = row(tinkery, TITLE_ROW);
    assert_eq!(
        tinkery.text_col(TITLE_ROW, hints.trim_end()),
        Some(at),
        "{title:?}"
    );
    assert_eq!(tinkery.fg_at(at, TITLE_ROW), MUTED);
}

/// The first output row is the restart rule, `── restarted HH:MM:SS ──…` in
/// `muted`, one cell in.
fn assert_restart_rule(tinkery: &Tinkery) {
    let line = row(tinkery, OUTPUT_ROW);
    let rule: Vec<char> = line.chars().collect();
    let text: String = rule.iter().take(14).collect();
    assert_eq!(text, " ── restarted ", "{line:?}");
    let time: String = rule.iter().skip(14).take(8).collect();
    let digits = time.chars().enumerate().all(|(i, c)| {
        if i % 3 == 2 {
            c == ':'
        } else {
            c.is_ascii_digit()
        }
    });
    assert!(digits, "not HH:MM:SS: {line:?}");
    let tail: String = rule.iter().skip(22).take(3).collect();
    assert_eq!(tail, " ──", "{line:?}");
    assert_eq!(tinkery.fg_at(5, OUTPUT_ROW), MUTED);
    assert_eq!(tinkery.fg_at(14, OUTPUT_ROW), MUTED);
}

#[test]
fn ansi_colours_take_the_theme_roles() {
    let mut tinkery = Tinkery::spawn_in_with_env(Path::new(COLOURS), &fixture_path(), &[]);
    tinkery.wait_for_text("Open directory", START);
    tinkery.send_keys("f5");
    tinkery.wait_for_text("palette  exited 0", WAIT);
    let first = OUTPUT_ROW;
    let second = OUTPUT_ROW + 1;
    assert!(row(&tinkery, first).starts_with(" red green yellow blue"));
    assert!(row(&tinkery, second).starts_with(" magenta cyan white black"));
    for (row, text, color) in [
        (first, "red", ERR),
        (first, "green", OK),
        (first, "yellow", WARN),
        (first, "blue", FUNCTION),
        (second, "magenta", KEYWORD),
        (second, "cyan", TYPE),
        (second, "white", STRONG),
        (second, "black", MUTED),
    ] {
        assert_eq!(
            tinkery.fg_text(row, color).trim(),
            text,
            "row {row}, {color:?}"
        );
    }
}

#[test]
fn f4_toggles_the_panel_at_about_a_third_of_the_height() {
    let mut tinkery = open_project();
    tinkery.send_keys("f4");
    tinkery.wait_for_text("F5 runs a command from .tinkery.toml", WAIT);
    assert_eq!(
        tinkery.text_col(OUTPUT_ROW, "F5 runs a command"),
        Some(1),
        "{:?}",
        tinkery.screen()
    );
    assert!(row(&tinkery, TITLE_ROW).starts_with(" run"));
    // Nothing else moved: the status line is still last.
    assert!(row(&tinkery, ROWS - 1).contains("Ln 1, Col 1"));
    tinkery.send_keys("f4");
    tinkery.wait_for_text_gone("F5 runs a command", WAIT);

    // A run opens the panel; F4 hides its output and brings it back.
    tinkery.send_keys("f5");
    tinkery.wait_for_text("dev  exited 0", WAIT);
    tinkery.send_keys("f4");
    tinkery.wait_for_text_gone("red line", WAIT);
    tinkery.wait_for_text_gone("dev  exited", WAIT);
    tinkery.send_keys("f4");
    tinkery.wait_for_text("red line", WAIT);
    assert!(row(&tinkery, TITLE_ROW).starts_with(" ✓ dev  exited 0"));
}

/// The cell at (`col`, `y`), as text.
fn cell(tinkery: &Tinkery, col: u16, y: u16) -> String {
    row(tinkery, y)
        .chars()
        .nth(usize::from(col))
        .map(String::from)
        .unwrap_or_default()
}

/// `text` without the panel's two edge cells, which share the edge's colour
/// and weight with what's inside, and without blanks.
fn inside(text: &str) -> String {
    text.replace(['▕', '▏', ' '], "")
}

#[test]
fn f5_with_several_entries_picks_one_by_name() {
    let mut tinkery =
        Tinkery::spawn_in_with_config_and_env(Path::new(MULTI), "", &fixture_path(), &[]);
    tinkery.wait_for_text("Open directory", START);
    tinkery.send_keys("f5");
    tinkery.wait_for_text(PICKER_TITLE, WAIT);
    tinkery.wait_for_text("hello", WAIT);
    // A panel: an accent edge with the title in it, and no lit top edge.
    assert_eq!(tinkery.text_col(PICKER_Y, PICKER_TITLE), Some(PICKER_X + 1));
    assert_eq!(tinkery.fg_at(PICKER_X + 1, PICKER_Y), PICKER_ACCENT);
    assert_eq!(tinkery.fg_at(PICKER_X + 3, PICKER_Y), PICKER_ACCENT);
    assert!(!row(&tinkery, PICKER_Y).contains('▀'));
    assert_eq!(cell(&tinkery, PICKER_X, PICKER_Y + 1), "▕");
    assert_eq!(cell(&tinkery, PICKER_X + 59, PICKER_Y + 1), "▏");
    assert_eq!(tinkery.text_col(PICKER_Y + 11, "▔"), Some(PICKER_X + 1));
    // The query's strip of `raised2`, with how many match at its right.
    assert_eq!(tinkery.text_col(PICKER_QUERY_ROW, "✦"), Some(PICKER_TEXT_X));
    assert_eq!(cell(&tinkery, PICKER_X + 1, PICKER_QUERY_ROW), CAP_L);
    assert_eq!(cell(&tinkery, PICKER_X + 58, PICKER_QUERY_ROW), CAP_R);
    assert_eq!(
        tinkery
            .bg_text(PICKER_QUERY_ROW, PICKER_RAISED2)
            .chars()
            .count(),
        56
    );
    assert_eq!(
        tinkery.text_col(PICKER_QUERY_ROW, "2 of 2"),
        Some(PICKER_RIGHT - 6)
    );
    // A row per entry: its name, its command, and its source right-aligned.
    // The first is selected: a pill of `hov` with `›`, its name `strong`
    // bold and the rest `text`; no glow and no `⏎`.
    let first = PICKER_FIRST_ROW;
    assert_eq!(tinkery.text_col(first, "dev"), Some(PICKER_TEXT_X));
    assert_eq!(tinkery.text_col(first, "colors"), Some(PICKER_DETAIL_X));
    assert_eq!(
        tinkery.text_col(first, ".tinkery.toml"),
        Some(PICKER_RIGHT - 13)
    );
    assert!(!row(&tinkery, first).contains('⏎'));
    assert_eq!(cell(&tinkery, PICKER_X + 1, first), CAP_L);
    assert_eq!(cell(&tinkery, PICKER_X + 58, first), CAP_R);
    assert_eq!(tinkery.fg_at(PICKER_X + 1, first), PICKER_HOV);
    assert_eq!(tinkery.bg_at(PICKER_X + 1, first), PICKER_RAISED);
    assert_eq!(tinkery.bg_text(first, PICKER_HOV).chars().count(), 56);
    assert_eq!(tinkery.text_col(first, "›"), Some(PICKER_X + 2));
    assert_eq!(tinkery.fg_at(PICKER_X + 2, first), PICKER_ACCENT);
    assert_eq!(inside(&tinkery.fg_text(first, PICKER_STRONG)), "dev");
    assert_eq!(inside(&tinkery.bold_text(first)), "›dev");
    assert_eq!(tinkery.fg_at(PICKER_DETAIL_X, first), PICKER_TEXT);
    assert_eq!(tinkery.fg_at(PICKER_RIGHT - 13, first), PICKER_TEXT);
    // The other is plain, its command and source in `muted`.
    let second = first + 1;
    assert_eq!(tinkery.text_col(second, "hello"), Some(PICKER_TEXT_X));
    assert_eq!(
        tinkery.text_col(second, "echo hello from tinkery"),
        Some(PICKER_DETAIL_X)
    );
    assert_eq!(tinkery.fg_at(PICKER_DETAIL_X, second), PICKER_MUTED);
    assert_eq!(tinkery.fg_at(PICKER_RIGHT - 13, second), PICKER_MUTED);
    assert_eq!(tinkery.bg_text(second, PICKER_HOV), "");
    assert_eq!(tinkery.text_col(second, "›"), None);
    // The keys, as keycaps, ending `Esc close`.
    let hints = format!(
        "{CAP_L}↑{CAP_R} {}",
        hints_text(&[("↓", "select"), ("⏎", "run"), ("Esc", "close")])
    );
    assert_eq!(
        tinkery.text_col(PICKER_HINTS_ROW, &hints),
        Some(PICKER_TEXT_X)
    );

    // Typing filters by name, the matched letters in the accent.
    tinkery.type_text("hel");
    tinkery.wait_for_text("✦ hel", WAIT);
    tinkery.wait_for_text("1 of 2", WAIT);
    assert_eq!(tinkery.text_col(first, "hello"), Some(PICKER_TEXT_X));
    assert_eq!(inside(&tinkery.fg_text(first, PICKER_ACCENT)), "›hel");
    assert_eq!(inside(&tinkery.bold_text(first)), "›hello");
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone(PICKER_TITLE, WAIT);
    tinkery.wait_for_text("hello from tinkery", WAIT);
    tinkery.wait_for_text("hello  exited 0", WAIT);
}

#[test]
fn the_run_picker_scrolls_past_five_entries_and_closes_on_esc() {
    let dir = tempfile::tempdir().unwrap();
    let entries: String = (0..7)
        .map(|i| format!("[[run]]\nname = \"job{i}\"\ncommand = \"echo {i}\"\n"))
        .collect();
    std::fs::write(dir.path().join(".tinkery.toml"), entries).unwrap();
    let mut tinkery = Tinkery::spawn_in_with_config(dir.path(), "", &[]);
    tinkery.wait_for_text("Open directory", START);
    tinkery.send_keys("f5");
    tinkery.wait_for_text(PICKER_TITLE, WAIT);
    tinkery.wait_for_text("7 of 7", WAIT);
    // Five rows, as before; the sixth entry isn't shown until it's reached.
    let last = PICKER_FIRST_ROW + 4;
    assert_eq!(tinkery.text_col(last, "job4"), Some(PICKER_TEXT_X));
    assert!(!tinkery.screen().join("\n").contains("job5"));
    for _ in 0..6 {
        tinkery.send_keys("down");
    }
    tinkery.wait_for_screen("the last entry on the last row", WAIT, |screen| {
        screen[usize::from(last)].contains("job6")
    });
    tinkery.wait_for_bg(PICKER_X + 2, last, PICKER_HOV, WAIT);
    assert_eq!(tinkery.text_col(last, "›"), Some(PICKER_X + 2));
    assert_eq!(
        tinkery.text_col(PICKER_FIRST_ROW, "job2"),
        Some(PICKER_TEXT_X)
    );
    assert_eq!(tinkery.bg_text(PICKER_FIRST_ROW, PICKER_HOV), "");
    // Nothing matching says so.
    tinkery.type_text("zzz");
    tinkery.wait_for_text("no match", WAIT);
    assert_eq!(
        tinkery.text_col(PICKER_FIRST_ROW, "no match"),
        Some(PICKER_TEXT_X)
    );
    tinkery.wait_for_text("0 of 7", WAIT);
    // Esc closes it and runs nothing.
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(PICKER_TITLE, WAIT);
    assert!(!tinkery.screen().join("\n").contains("running"));
}

#[test]
fn mono_dims_behind_the_run_picker_and_reverses_the_selected_row() {
    let mut tinkery = Tinkery::spawn_in_with_config_and_env(
        Path::new(MULTI),
        "theme = \"mono\"\n",
        &fixture_path(),
        &[],
    );
    tinkery.wait_for_text("Open directory", START);
    tinkery.send_keys("f5");
    tinkery.wait_for_text(PICKER_TITLE, WAIT);
    // The whole first row of the 60-wide card's inside, from column 21: the
    // mark, the name, the command and the source.
    let first = PICKER_FIRST_ROW;
    let selected = format!(" › {:<12}{:<27}.tinkery.toml   ", "dev", "colors");
    tinkery.wait_for_reversed(first, &selected, WAIT);
    // It carries no colours: one style across it, in the terminal's own.
    let (fg, bg) = (
        tinkery.fg_at(PICKER_X + 1, first),
        tinkery.bg_at(PICKER_X + 1, first),
    );
    for x in PICKER_X + 1..PICKER_X + 59 {
        assert_eq!(tinkery.fg_at(x, first), fg, "column {x}");
        assert_eq!(tinkery.bg_at(x, first), bg, "column {x}");
    }
    assert!(!matches!(fg, Color::Rgb(..)) && !matches!(bg, Color::Rgb(..)));
    assert!(!tinkery.reversed_text(first + 1).contains("hello"));
    // No Nerd Font glyph anywhere.
    let screen = tinkery.screen().join("\n");
    assert!(
        !screen.contains(CAP_L) && !screen.contains(CAP_R),
        "{screen}"
    );
    assert!(row(&tinkery, PICKER_HINTS_ROW).contains(" Esc  close"));
    let status = ROWS - 1;
    // The splash is still up under the card, so its keys are in the status line.
    let position = tinkery.text_col(status, "quit").expect("the splash's keys");
    assert!(
        tinkery.dim_at(position, status),
        "the status line is dimmed"
    );
    assert!(!tinkery.dim_at(PICKER_TEXT_X, first + 1), "the card isn't");
}

#[test]
fn a_malformed_project_file_says_so_in_a_toast() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".tinkery.toml"), "[[run]]\nname = = 1\n").unwrap();
    let mut tinkery = Tinkery::spawn_in(dir.path(), &[]);
    tinkery.wait_for_text("Open directory", START);
    tinkery.send_keys("f5");
    tinkery.wait_for_toast(".tinkery.toml line 2", WAIT);
    tinkery.assert_running_for(Duration::from_millis(200));
}

#[test]
fn shift_f5_stops_the_command_and_ctrl_f5_restarts_it() {
    let mut tinkery = open_project();
    tinkery.send_keys("f5");
    tinkery.wait_for_text("red line", WAIT);
    // Stopped during the script's pause, so its stderr line never comes.
    tinkery.send_keys("shift+f5");
    tinkery.wait_for_screen("the stopped title", WAIT, |screen| {
        screen[usize::from(TITLE_ROW)].starts_with(" ■ dev  stopped")
    });
    assert_eq!(tinkery.fg_at(1, TITLE_ROW), MUTED);
    assert_hints(&tinkery, DONE_HINTS);
    assert!(row(&tinkery, OUTPUT_ROW).starts_with(" red line"));

    // Ctrl+F5 after a stop starts it again in a cleared panel.
    tinkery.send_keys("ctrl+f5");
    tinkery.wait_for_screen("the restarted marker", WAIT, |screen| {
        screen[usize::from(OUTPUT_ROW)].contains("restarted")
            && screen[usize::from(TITLE_ROW)].starts_with(" ● dev  running")
    });
    assert_restart_rule(&tinkery);
    tinkery.wait_for_screen("the output again under the marker", WAIT, |screen| {
        screen[usize::from(OUTPUT_ROW) + 1].starts_with(" red line")
    });
    assert!(!row(&tinkery, OUTPUT_ROW + 2).contains("red line"));

    // Ctrl+F5 while it's running stops it and starts it once more.
    tinkery.send_keys("ctrl+f5");
    tinkery.wait_for_screen("a second restart", WAIT, |screen| {
        screen[usize::from(OUTPUT_ROW)].contains("restarted")
            && screen[usize::from(OUTPUT_ROW) + 1].starts_with(" red line")
            && screen[usize::from(TITLE_ROW)].starts_with(" ● dev  running")
    });
    // Only the new run finishes; the killed one sent nothing more.
    tinkery.wait_for_screen("the new run's exit", WAIT, |screen| {
        screen[usize::from(TITLE_ROW)].starts_with(" ✓ dev  exited 0")
    });
    let screen = tinkery.screen();
    assert_eq!(
        screen.iter().filter(|l| l.contains("second line")).count(),
        1,
        "{}",
        screen.join("\n")
    );
}

#[test]
fn f5_in_a_project_with_nothing_to_run_says_how_to_add_one() {
    let dir = tempfile::tempdir().unwrap();
    let mut tinkery = Tinkery::spawn_in(dir.path(), &[]);
    tinkery.wait_for_text("Open directory", START);
    tinkery.send_keys("f5");
    tinkery.wait_for_toast("add a [[run]] entry to .tinkery.toml", WAIT);
    assert!(!tinkery.screen().join("\n").contains(PICKER_TITLE));
}

#[test]
fn f5_without_run_entries_offers_detected_commands() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("go.mod"), "module example.com/x\n").unwrap();
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"scripts": {"dev": "vite"}}"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("pnpm-lock.yaml"), "").unwrap();
    let mut tinkery = Tinkery::spawn_in(dir.path(), &[]);
    tinkery.wait_for_text("Open directory", START);
    tinkery.send_keys("f5");
    tinkery.wait_for_text(PICKER_TITLE, WAIT);
    tinkery.wait_for_text("pnpm run dev", WAIT);
    tinkery.wait_for_text("go run .", WAIT);
    // Each says it was detected, at the right of its row.
    let screen = tinkery.screen();
    let pnpm = screen
        .iter()
        .position(|line| line.contains("pnpm run dev"))
        .expect("the pnpm row");
    let pnpm = u16::try_from(pnpm).unwrap();
    assert_eq!(
        tinkery.text_col(pnpm, "detected"),
        Some(PICKER_RIGHT - 8),
        "{:?}",
        row(&tinkery, pnpm)
    );
    assert!(!row(&tinkery, pnpm).contains('⏎'));
    // Offered, not started: nothing runs until one is picked.
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(PICKER_TITLE, WAIT);
    assert!(!tinkery.screen().join("\n").contains("running"));
}
