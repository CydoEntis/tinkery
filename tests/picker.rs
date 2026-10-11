mod harness;

use std::path::Path;
use std::time::Duration;

use harness::{CAP_L, CAP_R, ROWS, Tinkery, hints_text};
use vt100::Color;

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(5);
/// `.gitignore` hides `*.log` and `build/`.
const PROJECT: &str = "tests/fixtures/project";
const LONG: &str = "tests/fixtures/long.txt";
/// The default theme, tinkery, which an empty config runs on: the roles the
/// cast is drawn in.
const DEFAULT: &str = "";
const ACCENT: Color = Color::Rgb(0xff, 0x8a, 0x3d);
const ACCENT2: Color = Color::Rgb(0xff, 0xd2, 0x7a);
const RAISED: Color = Color::Rgb(0x1f, 0x1b, 0x19);
const RAISED2: Color = Color::Rgb(0x29, 0x23, 0x20);
const HOV: Color = Color::Rgb(0x3a, 0x30, 0x2a);
const BTN: Color = Color::Rgb(0x33, 0x2a, 0x25);
const ERR: Color = Color::Rgb(0xff, 0x5d, 0x4d);
const MUTED: Color = Color::Rgb(0x7d, 0x70, 0x67);
const TEXT: Color = Color::Rgb(0xb5, 0xa8, 0x9d);
const STRONG: Color = Color::Rgb(0xf8, 0xf0, 0xe6);

/// At 100x30 the cast is a panel 86 wide from column 7, dropping from row 4
/// (tinkery-forge spec F15). Its title is in the top edge; `✦` and the query
/// are on a strip on row 6, row 7 is blank, `FILES` is on row 8 and the file
/// rows run from row 9, their text four cells in. A row's pill runs the card's
/// inside, from column 8 to column 91, with `›` a cell into it.
const CARD_X: u16 = 7;
const CARD_Y: u16 = 4;
const CARD_WIDTH: u16 = 86;
const QUERY_ROW: u16 = 6;
const BLANK_ROW: u16 = 7;
const HEADER_ROW: u16 = 8;
const FIRST_ROW: u16 = 9;
const PILL_X: u16 = CARD_X + 1;
const PILL_END: u16 = CARD_X + CARD_WIDTH - 2;
const MARK_X: u16 = CARD_X + 2;
const TEXT_X: u16 = 11;
const QUERY_X: u16 = 13;
/// Where the card's right-aligned text ends: four cells in from its edge.
const RIGHT: u16 = 89;
const SCOPE: &str = "cast · files · commands";
/// With no prefix, after five file rows: a blank, `COMMANDS` and two command
/// rows.
const COMMANDS_ROW: u16 = 15;
/// Then a blank, the hints, and the bottom edge.
const HINTS_ROW: u16 = 19;
const BOTTOM_ROW: u16 = 20;
/// Where the one row of a `>`, `:` or `/` section is, under its header, and
/// the hints under it.
const MODE_ROW: u16 = 9;
const MODE_HINTS_ROW: u16 = 11;
/// Three lines in two of its files hold `TODO`.
const SEARCH_PROJECT: &str = "tests/fixtures/search";

/// What the cast's hints read: the prefixes, then the keys, `enter` being what
/// Enter does in the mode shown. `↑` and `↓` are two keycaps a blank apart.
fn cast_hints(enter: &str) -> String {
    format!(
        "{}   {CAP_L}↑{CAP_R} {}",
        hints_text(&[(">", "commands"), (":", "line"), ("/", "text")]),
        hints_text(&[("↓", "move"), ("⏎", enter), ("Esc", "close")])
    )
}

/// Tinkery in the fixture project with no file open, so the root is its folder.
fn open_project() -> Tinkery {
    open_project_with(DEFAULT)
}

fn open_project_with(config: &str) -> Tinkery {
    let tinkery = Tinkery::spawn_in_with_config(Path::new(PROJECT), config, &[]);
    tinkery.wait_for_text("Open directory", START);
    tinkery
}

fn open_picker(tinkery: &mut Tinkery) {
    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text(SCOPE, WAIT);
}

fn row(tinkery: &Tinkery, row: u16) -> String {
    tinkery.screen()[usize::from(row)].clone()
}

/// The cell at (`col`, `y`), as text.
fn cell(tinkery: &Tinkery, col: u16, y: u16) -> String {
    row(tinkery, y)
        .chars()
        .nth(usize::from(col))
        .map(String::from)
        .unwrap_or_default()
}

fn status_line(tinkery: &Tinkery) -> String {
    row(tinkery, ROWS - 1)
}

/// `text` without the card's two edge cells, which share the edge's colour and
/// weight with what's inside, and without blanks: the terminal may leave a
/// blank in the last colour.
fn inside(text: &str) -> String {
    text.replace(['▕', '▏', ' '], "")
}

/// Asserts row `y` is drawn as the selected row: a pill of `hov` on the card's
/// `raised` across its inside, `›` in the accent, bold, a cell in; and nothing
/// of the old look, a glow from the card's edge or `⏎` at the right.
fn assert_pill(tinkery: &Tinkery, y: u16) {
    assert_eq!(cell(tinkery, PILL_X, y), CAP_L, "row {y}");
    assert_eq!(cell(tinkery, PILL_END, y), CAP_R, "row {y}");
    for x in [PILL_X, PILL_END] {
        assert_eq!(tinkery.fg_at(x, y), HOV, "the end at {x},{y}");
        assert_eq!(tinkery.bg_at(x, y), RAISED, "the end at {x},{y}");
    }
    assert_eq!(
        tinkery.bg_text(y, HOV).chars().count(),
        usize::from(CARD_WIDTH - 4),
        "the pill runs between its ends on row {y}"
    );
    assert_eq!(tinkery.text_col(y, "›"), Some(MARK_X), "row {y}");
    assert_eq!(tinkery.fg_at(MARK_X, y), ACCENT, "row {y}");
    assert!(tinkery.bold_at(MARK_X, y), "row {y}");
    assert!(!row(tinkery, y).contains('⏎'), "row {y}");
}

/// Waits for row `y` to be the selected row.
fn wait_for_pill(tinkery: &Tinkery, y: u16) {
    tinkery.wait_for_bg(MARK_X, y, HOV, WAIT);
    assert_pill(tinkery, y);
}

/// Asserts row `y` isn't lit: no pill, no mark.
fn assert_no_pill(tinkery: &Tinkery, y: u16) {
    assert_eq!(tinkery.bg_text(y, HOV), "", "row {y}");
    assert_eq!(tinkery.bg_at(MARK_X, y), RAISED, "row {y}");
    assert_eq!(tinkery.text_col(y, "›"), None, "row {y}");
    assert_eq!(cell(tinkery, PILL_X, y), " ", "row {y}");
}

/// Waits for `file` (its name, two blanks, its folder) to be the selected row
/// `y`.
fn wait_for_selected(tinkery: &Tinkery, y: u16, file: &str) {
    // The text first: the pill may already be on row `y` for the row before.
    tinkery.wait_for_screen(&format!("{file:?} on row {y}"), WAIT, |lines| {
        let shown: String = lines[usize::from(y)]
            .chars()
            .skip(usize::from(TEXT_X))
            .collect();
        shown.starts_with(&format!("{file} "))
    });
    wait_for_pill(tinkery, y);
}

#[test]
fn ctrl_p_casts_a_panel_of_project_files_respecting_gitignore() {
    let mut tinkery = open_project();
    open_picker(&mut tinkery);
    tinkery.wait_for_text("main.rs  src", WAIT);

    // A panel: an accent edge with the title set into it, around `raised`.
    // No lit top edge.
    let edge = row(&tinkery, CARD_Y);
    assert_eq!(tinkery.text_col(CARD_Y, "▁ Go to file ▁"), Some(CARD_X + 1));
    assert_eq!(edge.matches('▁').count(), 84 - " Go to file ".len());
    assert!(!edge.contains('▀'), "{edge:?}");
    assert_eq!(tinkery.fg_at(CARD_X + 1, CARD_Y), ACCENT);
    assert_eq!(tinkery.fg_at(CARD_X + 4, CARD_Y), ACCENT);
    assert!(tinkery.bold_at(CARD_X + 4, CARD_Y));
    assert_eq!(tinkery.fg_at(CARD_X + 84, CARD_Y), ACCENT);
    for y in CARD_Y + 1..BOTTOM_ROW {
        assert_eq!(cell(&tinkery, CARD_X, y), "▕", "row {y}");
        assert_eq!(cell(&tinkery, CARD_X + 85, y), "▏", "row {y}");
        assert_eq!(tinkery.fg_at(CARD_X, y), ACCENT, "row {y}");
    }
    assert_eq!(tinkery.text_col(BOTTOM_ROW, "▔"), Some(CARD_X + 1));
    assert_eq!(row(&tinkery, BOTTOM_ROW).matches('▔').count(), 84);
    assert_eq!(tinkery.bg_at(CARD_X + 1, CARD_Y + 1), RAISED);

    // The query's strip of `raised2`, between two ends: `✦`, then the
    // palette's name and what it lists, right-aligned.
    assert_eq!(cell(&tinkery, PILL_X, QUERY_ROW), CAP_L);
    assert_eq!(cell(&tinkery, PILL_END, QUERY_ROW), CAP_R);
    assert_eq!(tinkery.fg_at(PILL_X, QUERY_ROW), RAISED2);
    assert_eq!(tinkery.bg_at(PILL_X, QUERY_ROW), RAISED);
    assert_eq!(
        tinkery.bg_text(QUERY_ROW, RAISED2).chars().count(),
        usize::from(CARD_WIDTH - 4)
    );
    assert_eq!(tinkery.text_col(QUERY_ROW, "✦"), Some(TEXT_X));
    assert_eq!(tinkery.fg_at(TEXT_X, QUERY_ROW), ACCENT2);
    assert_eq!(
        tinkery.text_col(QUERY_ROW, SCOPE),
        Some(RIGHT - SCOPE.chars().count() as u16)
    );
    assert_eq!(inside(&tinkery.fg_text(QUERY_ROW, ACCENT)), "cast");
    assert_eq!(tinkery.fg_at(RIGHT - 1, QUERY_ROW), MUTED);
    // Where the rule was, a blank row.
    assert_eq!(inside(&row(&tinkery, BLANK_ROW)), "");
    for y in CARD_Y..=BOTTOM_ROW {
        for x in CARD_X..CARD_X + CARD_WIDTH {
            assert_ne!(cell(&tinkery, x, y), "─", "a rule at {x},{y}");
        }
    }
    // Section headings in `muted` bold.
    assert_eq!(tinkery.text_col(HEADER_ROW, "FILES"), Some(TEXT_X));
    assert_eq!(inside(&tinkery.bold_text(HEADER_ROW)), "FILES");
    assert_eq!(tinkery.fg_at(TEXT_X, HEADER_ROW), MUTED);

    // Path order with an empty query, five at a time: the name, then the folder.
    let expected = [
        ".gitignore",
        "README.md",
        "guide.md  docs",
        "notes.txt",
        "main.rs  src",
    ];
    for (i, file) in expected.iter().enumerate() {
        let y = FIRST_ROW + u16::try_from(i).unwrap();
        assert_eq!(tinkery.text_col(y, file), Some(TEXT_X), "{file}");
    }
    let screen = tinkery.screen().join("\n");
    assert!(!screen.contains("helpers.rs"), "only five rows: {screen}");
    assert!(!screen.contains("debug.log"), "{screen}");
    assert!(!screen.contains("out.txt"), "{screen}");
    assert!(!screen.contains("HEAD"), "{screen}");
    // The first file starts selected: `strong` bold on the pill. No other
    // row is lit, and theirs are `fg` with the folder in `muted`.
    wait_for_selected(&tinkery, FIRST_ROW, ".gitignore");
    assert_eq!(inside(&tinkery.fg_text(FIRST_ROW, STRONG)), ".gitignore");
    assert_eq!(inside(&tinkery.bold_text(FIRST_ROW)), "›.gitignore");
    for y in FIRST_ROW + 1..FIRST_ROW + 5 {
        assert_no_pill(&tinkery, y);
        assert!(!tinkery.bold_at(TEXT_X, y), "row {y}");
    }
    assert_eq!(inside(&tinkery.fg_text(FIRST_ROW + 2, MUTED)), "docs");

    // Then the first two commands, each with its key right-aligned in `muted`.
    assert_eq!(tinkery.text_col(COMMANDS_ROW, "COMMANDS"), Some(TEXT_X));
    assert_eq!(inside(&tinkery.bold_text(COMMANDS_ROW)), "COMMANDS");
    assert_eq!(tinkery.fg_at(TEXT_X, COMMANDS_ROW), MUTED);
    for (i, (name, key)) in [("Quit", "Ctrl+Q"), ("Save", "Ctrl+S")]
        .into_iter()
        .enumerate()
    {
        let y = COMMANDS_ROW + 1 + u16::try_from(i).unwrap();
        assert_eq!(tinkery.text_col(y, name), Some(TEXT_X), "{name}");
        let key_x = RIGHT - key.chars().count() as u16;
        assert_eq!(tinkery.text_col(y, key), Some(key_x), "{key}");
        assert_eq!(tinkery.fg_at(key_x, y), MUTED, "{key}");
        assert_no_pill(&tinkery, y);
    }

    // The last line is the keys as keycaps, ending `Esc close`.
    assert_eq!(inside(&row(&tinkery, HINTS_ROW - 1)), "");
    assert_eq!(
        tinkery.text_col(HINTS_ROW, &cast_hints("open")),
        Some(TEXT_X)
    );
    assert!(
        row(&tinkery, HINTS_ROW)
            .trim_end_matches([' ', '▏'])
            .ends_with(&format!("{CAP_L}Esc{CAP_R} close")),
        "{:?}",
        row(&tinkery, HINTS_ROW)
    );
    assert_eq!(tinkery.fg_at(TEXT_X + 1, HINTS_ROW), STRONG);
    assert_eq!(tinkery.bg_at(TEXT_X + 1, HINTS_ROW), BTN);
    assert!(tinkery.bold_at(TEXT_X + 1, HINTS_ROW));
    assert_eq!(tinkery.fg_at(TEXT_X + 4, HINTS_ROW), MUTED);
}

#[test]
fn typing_main_puts_main_rs_first_with_the_matched_letters_lit() {
    let mut tinkery = open_project();
    open_picker(&mut tinkery);
    tinkery.wait_for_text("README.md", WAIT);
    tinkery.type_text("main");
    tinkery.wait_for_text("✦ main", WAIT);
    tinkery.wait_for_text_gone("README.md", WAIT);
    wait_for_selected(&tinkery, FIRST_ROW, "main.rs  src");
    // Accent on the selection's mark and the matched letters; the rest of the
    // name `strong`, all of it bold, and the folder plain `text`.
    assert_eq!(inside(&tinkery.fg_text(FIRST_ROW, ACCENT)), "›main");
    assert_eq!(inside(&tinkery.fg_text(FIRST_ROW, STRONG)), ".rs");
    assert_eq!(inside(&tinkery.bold_text(FIRST_ROW)), "›main.rs");
    assert_eq!(inside(&tinkery.fg_text(FIRST_ROW, TEXT)), "src");
    // The query is `strong` on the strip, and the cursor sits after it.
    assert_eq!(tinkery.fg_at(QUERY_X, QUERY_ROW), STRONG);
    assert_eq!(tinkery.bg_at(QUERY_X, QUERY_ROW), RAISED2);
    tinkery.wait_for_cursor(QUERY_X + "main".len() as u16, QUERY_ROW, WAIT);

    // Nothing matching says so, and nothing is selected.
    tinkery.type_text("zzz");
    tinkery.wait_for_text("no matching files", WAIT);
    assert_eq!(
        tinkery.text_col(FIRST_ROW, "no matching files"),
        Some(TEXT_X)
    );
    assert_eq!(tinkery.fg_at(TEXT_X, FIRST_ROW), MUTED);
    assert_no_pill(&tinkery, FIRST_ROW);
}

#[test]
fn arrows_move_the_selection_and_enter_opens_it_in_a_tab() {
    let mut tinkery = open_project();
    open_picker(&mut tinkery);
    wait_for_selected(&tinkery, FIRST_ROW, ".gitignore");
    tinkery.send_keys("down");
    tinkery.send_keys("down");
    wait_for_selected(&tinkery, FIRST_ROW + 2, "guide.md  docs");
    tinkery.send_keys("up");
    wait_for_selected(&tinkery, FIRST_ROW + 1, "README.md");
    assert_no_pill(&tinkery, FIRST_ROW);
    assert_no_pill(&tinkery, FIRST_ROW + 2);
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone(SCOPE, WAIT);
    tinkery.wait_for_text("1  # Project fixture", WAIT);
    // The status line is drawn after the editor, so it may lag a moment behind.
    tinkery.wait_for_screen("README.md in the status line", WAIT, |screen| {
        screen[usize::from(ROWS) - 1].contains("README.md")
    });

    // Past the fifth row the list scrolls to keep the selection on the last.
    open_picker(&mut tinkery);
    wait_for_selected(&tinkery, FIRST_ROW, ".gitignore");
    for _ in 0..5 {
        tinkery.send_keys("down");
    }
    wait_for_selected(&tinkery, FIRST_ROW + 4, "helpers.rs  src/util");
    assert_eq!(tinkery.text_col(FIRST_ROW, "README.md"), Some(TEXT_X));
    // One more and the pill is on the first command, under its heading.
    tinkery.send_keys("down");
    wait_for_pill(&tinkery, COMMANDS_ROW + 1);
    assert_eq!(tinkery.text_col(COMMANDS_ROW + 1, "Quit"), Some(TEXT_X));
    assert_no_pill(&tinkery, FIRST_ROW + 4);
    // The selected command's key stays where every row's is, in `text`.
    assert_eq!(
        tinkery.text_col(COMMANDS_ROW + 1, "Ctrl+Q"),
        Some(RIGHT - 6)
    );
    assert_eq!(tinkery.fg_at(RIGHT - 6, COMMANDS_ROW + 1), TEXT);
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(SCOPE, WAIT);

    // A filtered pick opens beside it in a second tab.
    open_picker(&mut tinkery);
    tinkery.type_text("helpers");
    wait_for_selected(&tinkery, FIRST_ROW, "helpers.rs  src/util");
    tinkery.send_keys("enter");
    tinkery.wait_for_text("1  pub fn help() {}", WAIT);
    tinkery.wait_for_screen("both files in the tab bar", WAIT, |screen| {
        screen[1].contains("README.md") && screen[1].contains("helpers.rs")
    });
}

#[test]
fn esc_closes_the_picker_without_opening_anything() {
    let mut tinkery = open_project();
    open_picker(&mut tinkery);
    tinkery.wait_for_text("notes.txt", WAIT);
    tinkery.type_text("notes");
    tinkery.wait_for_text("✦ notes", WAIT);
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(SCOPE, WAIT);
    tinkery.wait_for_text_gone("notes.txt", WAIT);
    // Nothing was opened, so the status bar is still the splash's.
    assert!(status_line(&tinkery).contains(&hints_text(&[("q", "quit")])));
    // Nothing was opened, so the splash is back; leaving it, typing goes to the
    // editor, not to a hidden query.
    tinkery.wait_for_text("New file", WAIT);
    tinkery.send_keys("ctrl+n");
    tinkery.type_text("x");
    tinkery.wait_for_text("1  x", WAIT);
}

#[test]
fn a_click_outside_the_card_closes_it_and_one_inside_does_not() {
    let mut tinkery = open_project();
    open_picker(&mut tinkery);
    tinkery.wait_for_text("notes.txt", WAIT);
    tinkery.click(50, FIRST_ROW + 2);
    // Still open: typing goes to the query.
    tinkery.type_text("x");
    tinkery.wait_for_text("✦ x", WAIT);
    // The panel's edge is part of the card.
    tinkery.click(CARD_X, CARD_Y);
    tinkery.click(CARD_X + CARD_WIDTH - 1, QUERY_ROW);
    tinkery.type_text("y");
    tinkery.wait_for_text("✦ xy", WAIT);
    tinkery.click(2, 20);
    tinkery.wait_for_text_gone(SCOPE, WAIT);
    // Nothing was opened, so the status bar is still the splash's.
    assert!(status_line(&tinkery).contains(&hints_text(&[("q", "quit")])));
}

#[test]
fn mono_dims_with_the_modifier_and_reverses_the_selected_row() {
    let mut tinkery = open_project_with("theme = \"mono\"\n");
    open_picker(&mut tinkery);
    tinkery.wait_for_text("main.rs  src", WAIT);
    // The whole inside of the card, from edge to edge.
    let selected = format!("{:<84}", " › .gitignore");
    tinkery.wait_for_reversed(FIRST_ROW, &selected, WAIT);
    // It carries no colours: one style across it, in the terminal's own.
    let (fg, bg) = (
        tinkery.fg_at(PILL_X, FIRST_ROW),
        tinkery.bg_at(PILL_X, FIRST_ROW),
    );
    for x in PILL_X..=PILL_END {
        assert_eq!(tinkery.fg_at(x, FIRST_ROW), fg, "column {x}");
        assert_eq!(tinkery.bg_at(x, FIRST_ROW), bg, "column {x}");
    }
    assert!(!matches!(fg, Color::Rgb(..)) && !matches!(bg, Color::Rgb(..)));
    assert_eq!(tinkery.reversed_text(FIRST_ROW + 1), "");
    // No Nerd Font glyph anywhere: square ends, plain keys.
    let screen = tinkery.screen().join("\n");
    assert!(
        !screen.contains(CAP_L) && !screen.contains(CAP_R),
        "{screen}"
    );
    assert!(row(&tinkery, HINTS_ROW).contains(" Esc  close"));
    let status = ROWS - 1;
    // The splash is still up under the card, so its keys are in the status line.
    let position = tinkery.text_col(status, "quit").unwrap();
    assert!(
        tinkery.dim_at(position, status),
        "the status line is dimmed"
    );
    assert!(!tinkery.dim_at(TEXT_X, HEADER_ROW), "the card isn't");

    // Matched letters on the selected row get no colour either.
    tinkery.type_text("main");
    tinkery.wait_for_reversed(FIRST_ROW, &format!("{:<84}", " › main.rs  src"), WAIT);
    for x in PILL_X..=PILL_END {
        assert_eq!(tinkery.fg_at(x, FIRST_ROW), fg, "column {x}");
        assert_eq!(tinkery.bg_at(x, FIRST_ROW), bg, "column {x}");
    }
}

#[test]
fn tinkery_light_draws_the_cast_in_its_own_colours() {
    // tinkery-light's `raised`, `raised2`, `hov`, `accent` and `strong`.
    let raised = Color::Rgb(0xe4, 0xdc, 0xd0);
    let raised2 = Color::Rgb(0xd9, 0xcf, 0xc2);
    let hov = Color::Rgb(0xd3, 0xc7, 0xb8);
    let accent = Color::Rgb(0xb8, 0x4a, 0x08);
    let strong = Color::Rgb(0x1c, 0x17, 0x14);
    let mut tinkery = open_project_with("theme = \"tinkery-light\"\n");
    open_picker(&mut tinkery);
    tinkery.type_text("main");
    tinkery.wait_for_text("✦ main", WAIT);
    tinkery.wait_for_text_gone("README.md", WAIT);
    tinkery.wait_for_bg(MARK_X, FIRST_ROW, hov, WAIT);
    assert_eq!(tinkery.fg_at(CARD_X + 1, CARD_Y), accent);
    assert_eq!(tinkery.bg_at(TEXT_X, HEADER_ROW), raised);
    assert_eq!(
        tinkery.bg_text(QUERY_ROW, raised2).chars().count(),
        usize::from(CARD_WIDTH - 4)
    );
    assert_eq!(tinkery.fg_at(QUERY_X, QUERY_ROW), strong);
    assert_eq!(cell(&tinkery, PILL_X, FIRST_ROW), CAP_L);
    assert_eq!(tinkery.fg_at(PILL_X, FIRST_ROW), hov);
    assert_eq!(tinkery.bg_at(PILL_X, FIRST_ROW), raised);
    assert_eq!(
        tinkery.bg_text(FIRST_ROW, hov).chars().count(),
        usize::from(CARD_WIDTH - 4)
    );
    assert_eq!(inside(&tinkery.fg_text(FIRST_ROW, accent)), "›main");
    assert_eq!(inside(&tinkery.fg_text(FIRST_ROW, strong)), ".rs");
}

#[test]
fn the_ui_settings_square_the_pills_and_round_the_panel() {
    let config = "[ui]\npill_caps = false\ncorners = \"rounded\"\n";
    let mut tinkery = open_project_with(config);
    open_picker(&mut tinkery);
    tinkery.wait_for_text("main.rs  src", WAIT);
    tinkery.wait_for_bg(MARK_X, FIRST_ROW, HOV, WAIT);
    // Rounded lines for the edge, with the title in the top one.
    assert_eq!(tinkery.text_col(CARD_Y, "╭─ Go to file ─"), Some(CARD_X));
    assert_eq!(cell(&tinkery, CARD_X + 85, CARD_Y), "╮");
    assert_eq!(cell(&tinkery, CARD_X, FIRST_ROW), "│");
    assert_eq!(cell(&tinkery, CARD_X, BOTTOM_ROW), "╰");
    // Square ends: a pill is its fill from edge to edge, and no cap glyph is
    // drawn anywhere.
    let screen = tinkery.screen().join("\n");
    assert!(
        !screen.contains(CAP_L) && !screen.contains(CAP_R),
        "{screen}"
    );
    assert_eq!(
        tinkery.bg_text(FIRST_ROW, HOV).chars().count(),
        usize::from(CARD_WIDTH - 2)
    );
    assert_eq!(
        tinkery.bg_text(QUERY_ROW, RAISED2).chars().count(),
        usize::from(CARD_WIDTH - 2)
    );
    // Nothing moved: the mark, the text and the hints are where they were.
    assert_eq!(tinkery.text_col(FIRST_ROW, "›"), Some(MARK_X));
    assert_eq!(tinkery.text_col(FIRST_ROW, ".gitignore"), Some(TEXT_X));
    assert_eq!(tinkery.text_col(QUERY_ROW, "✦"), Some(TEXT_X));
    assert_eq!(tinkery.text_col(HINTS_ROW, " >  commands"), Some(TEXT_X));
    assert!(row(&tinkery, HINTS_ROW).contains(" Esc  close"));
}

/// `LONG` open on its own: 200 lines, the tree hidden.
fn open_long() -> Tinkery {
    let tinkery = Tinkery::spawn_with_config(DEFAULT, &[LONG]);
    tinkery.wait_for_text("Ln 1, Col 1", START);
    tinkery
}

#[test]
fn gt_lists_only_commands_with_their_keys_and_enter_runs_one() {
    let mut tinkery = open_long();
    open_picker(&mut tinkery);
    tinkery.type_text(">");
    tinkery.wait_for_text("cast · commands", WAIT);
    tinkery.wait_for_text_gone("FILES", WAIT);
    assert!(row(&tinkery, CARD_Y).contains("▁ Commands ▁"));
    assert_eq!(tinkery.text_col(HEADER_ROW, "COMMANDS"), Some(TEXT_X));
    // Every global command in order, the first selected; its key is where
    // every row's is.
    wait_for_pill(&tinkery, FIRST_ROW);
    assert_eq!(tinkery.text_col(FIRST_ROW, "Quit"), Some(TEXT_X));
    assert_eq!(tinkery.text_col(FIRST_ROW, "Ctrl+Q"), Some(RIGHT - 6));
    assert_eq!(tinkery.fg_at(RIGHT - 6, FIRST_ROW), TEXT);
    assert_eq!(tinkery.text_col(FIRST_ROW + 1, "Save"), Some(TEXT_X));
    assert_eq!(tinkery.text_col(FIRST_ROW + 1, "Ctrl+S"), Some(RIGHT - 6));
    assert_eq!(tinkery.fg_at(RIGHT - 6, FIRST_ROW + 1), MUTED);
    assert_no_pill(&tinkery, FIRST_ROW + 1);
    // Eight rows, a blank, then the hints, where Enter runs.
    assert_eq!(
        tinkery.text_col(FIRST_ROW + 9, &cast_hints("run")),
        Some(TEXT_X)
    );

    tinkery.type_text("split");
    // "sp" already ranks Split right first, so wait for the whole query to
    // land before checking which letters are lit.
    tinkery.wait_for_cursor(QUERY_X + ">split".len() as u16, QUERY_ROW, WAIT);
    tinkery.wait_for_screen("Split right selected", WAIT, |screen| {
        screen[usize::from(FIRST_ROW)]
            .chars()
            .skip(usize::from(TEXT_X))
            .collect::<String>()
            .starts_with("Split right ")
    });
    wait_for_pill(&tinkery, FIRST_ROW);
    assert_eq!(tinkery.text_col(FIRST_ROW, "Alt+V"), Some(RIGHT - 5));
    assert_eq!(inside(&tinkery.fg_text(FIRST_ROW, ACCENT)), "›Split");
    assert_eq!(inside(&tinkery.bold_text(FIRST_ROW)), "›Splitright");
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone("cast · commands", WAIT);
    // Two editors on the file, side by side; text starts on row 3, under the
    // pills and the blank row below them.
    tinkery.wait_for_screen("two splits", WAIT, |screen| {
        screen[3].matches("1  line 1").count() == 2
    });
}

#[test]
fn ctrl_g_casts_with_a_colon_and_enter_goes_to_the_line() {
    let mut tinkery = open_long();
    tinkery.send_keys("ctrl+g");
    tinkery.wait_for_text("cast · line", WAIT);
    assert!(row(&tinkery, CARD_Y).contains("▁ Go to line ▁"));
    assert_eq!(tinkery.text_col(QUERY_ROW, "✦ :"), Some(TEXT_X));
    tinkery.wait_for_cursor(QUERY_X + 1, QUERY_ROW, WAIT);
    assert_eq!(tinkery.text_col(HEADER_ROW, "LINE"), Some(TEXT_X));
    assert_eq!(
        tinkery.text_col(MODE_ROW, "type a line number, 1–200"),
        Some(TEXT_X)
    );
    // A hint isn't a row to pick.
    assert_no_pill(&tinkery, MODE_ROW);
    assert_eq!(
        tinkery.text_col(MODE_HINTS_ROW, &cast_hints("go")),
        Some(TEXT_X)
    );
    // The bottom bar it used to open is gone.
    let screen = tinkery.screen().join("\n");
    assert!(!screen.contains("Go to line:"), "{screen}");

    tinkery.type_text("120");
    tinkery.wait_for_text("Go to line 120  of 200", WAIT);
    wait_for_pill(&tinkery, MODE_ROW);
    assert_eq!(inside(&tinkery.bold_text(MODE_ROW)), "›Gotoline120");
    assert_eq!(inside(&tinkery.fg_text(MODE_ROW, TEXT)), "of200");
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone("cast · line", WAIT);
    tinkery.wait_for_text("Ln 120, Col 1", WAIT);
    tinkery.wait_for_text("line 120", WAIT);

    // Esc closes it and moves nothing.
    tinkery.send_keys("ctrl+g");
    tinkery.wait_for_text("cast · line", WAIT);
    tinkery.type_text("5");
    tinkery.wait_for_text("Go to line 5  of 200", WAIT);
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone("cast · line", WAIT);
    assert!(status_line(&tinkery).contains("Ln 120, Col 1"));
}

#[test]
fn a_line_out_of_range_shows_in_err_and_enter_keeps_the_cast_open() {
    let mut tinkery = open_long();
    tinkery.send_keys("ctrl+g");
    tinkery.wait_for_text("cast · line", WAIT);
    tinkery.type_text("999");
    tinkery.wait_for_text("999  is out of range: lines 1–200", WAIT);
    assert_eq!(tinkery.text_col(MODE_ROW, "999"), Some(TEXT_X));
    assert_eq!(tinkery.fg_text(MODE_ROW, ERR).replace(' ', ""), "999");
    assert_eq!(tinkery.fg_text(QUERY_ROW, ERR).replace(' ', ""), ":999");
    assert_no_pill(&tinkery, MODE_ROW);

    tinkery.send_keys("enter");
    // Still open and taking keys: the query can be fixed.
    for _ in 0..3 {
        tinkery.send_keys("backspace");
    }
    tinkery.type_text("7");
    tinkery.wait_for_text("Go to line 7  of 200", WAIT);
    assert!(status_line(&tinkery).contains("Ln 1, Col 1"));
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone("cast · line", WAIT);
    tinkery.wait_for_text("Ln 7, Col 1", WAIT);
}

#[test]
fn slash_text_opens_project_search_for_the_text() {
    let mut tinkery = Tinkery::spawn_in_with_config(Path::new(SEARCH_PROJECT), DEFAULT, &[]);
    tinkery.wait_for_text("Open directory", START);
    open_picker(&mut tinkery);
    tinkery.type_text("/TODO");
    tinkery.wait_for_text("cast · text", WAIT);
    // The mode switches on "/", before the rest of the query has arrived.
    tinkery.wait_for_text("Search the project for TODO", WAIT);
    assert!(row(&tinkery, CARD_Y).contains("▁ Search ▁"));
    assert_eq!(tinkery.text_col(HEADER_ROW, "TEXT"), Some(TEXT_X));
    assert_eq!(
        tinkery.text_col(MODE_ROW, "Search the project for TODO"),
        Some(TEXT_X)
    );
    wait_for_pill(&tinkery, MODE_ROW);
    // The text to search for is lit, as a match is.
    assert_eq!(inside(&tinkery.fg_text(MODE_ROW, ACCENT)), "›TODO");
    assert_eq!(
        tinkery.text_col(MODE_HINTS_ROW, &cast_hints("search")),
        Some(TEXT_X)
    );
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone("cast · text", WAIT);
    // The panel holds the text and has searched for it.
    tinkery.wait_for_text("Search   TODO", WAIT);
    tinkery.wait_for_text("3 matches in 2 files", WAIT);
}
