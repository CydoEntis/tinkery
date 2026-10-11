mod harness;

use std::path::Path;
use std::time::Duration;

use harness::{CAP_L, CAP_R, ROWS, Tinkery, hints_text};
use vt100::Color;

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(5);
/// `src/lib.rs` has `// TODO` on line 3 and `todo_free` on line 4, `src/main.rs`
/// an indented `// TODO` on line 2. `.gitignore` hides `ignored.log`, and
/// `data.bin` is binary; both hold `TODO` too.
const PROJECT: &str = "tests/fixtures/search";

/// The default theme, tinkery, which an empty config runs on: the roles the
/// panel is drawn in.
const DEFAULT: &str = "";
const ACCENT: Color = Color::Rgb(0xff, 0x8a, 0x3d);
const ACCENT2: Color = Color::Rgb(0xff, 0xd2, 0x7a);
const RAISED: Color = Color::Rgb(0x1f, 0x1b, 0x19);
const RAISED2: Color = Color::Rgb(0x29, 0x23, 0x20);
const HOV: Color = Color::Rgb(0x3a, 0x30, 0x2a);
const BTN: Color = Color::Rgb(0x33, 0x2a, 0x25);
const MUTED: Color = Color::Rgb(0x7d, 0x70, 0x67);
const TEXT: Color = Color::Rgb(0xb5, 0xa8, 0x9d);
const STRONG: Color = Color::Rgb(0xf8, 0xf0, 0xe6);
const BG: Color = Color::Rgb(0x11, 0x0f, 0x0e);
const WARN: Color = Color::Rgb(0xf2, 0xc1, 0x4e);
const ERR: Color = Color::Rgb(0xff, 0x5d, 0x4d);
const BLACK: Color = Color::Rgb(0, 0, 0);

/// At 100x30 the panel is 80 wide and 22 tall, centred (SPEC_V1_LAYOUT §7.3;
/// tinkery-forge spec F15): it starts at column 10 and row 4 with its title in
/// the top edge. The Search strip is on row 6 and the Replace strip on row 7,
/// row 8 is blank, the match count is on row 9 and the hits run from row 10.
const CARD_X: u16 = 10;
const CARD_Y: u16 = 4;
const CARD_WIDTH: u16 = 80;
const QUERY_ROW: u16 = 6;
const REPLACE_ROW: u16 = 7;
const STATUS_ROW: u16 = 9;
const FIRST_ROW: u16 = 10;
/// Then a blank, the hints, and the bottom edge.
const HINTS_ROW: u16 = 24;
const BOTTOM_ROW: u16 = 25;
/// A strip or a row's pill runs the card's inside, from column 11 to column
/// 88, with `›` a cell into a row's.
const PILL_X: u16 = CARD_X + 1;
const PILL_END: u16 = CARD_X + CARD_WIDTH - 2;
const MARK_X: u16 = CARD_X + 2;
/// Where `✦`, the match count, a hit's `path:line` and the hints start: four
/// cells in. Text ends four cells in from the other edge.
const LIST_X: u16 = 14;
const RIGHT: u16 = 86;
/// Where the fields' labels start, after `✦ `, and where their values start.
const LABEL_X: u16 = 16;
const VALUE_X: u16 = 25;
/// `src/main.rs:2`, the widest `path:line`, sets the column's width.
const PLACE_WIDTH: usize = 13;
/// In every theme's hints, round ends or square: the panel is open.
const OPEN: &str = "replace all";

const LIB_TODO: (&str, &str) = ("src/lib.rs:3", "// TODO: write the library");
const LIB_FN: (&str, &str) = ("src/lib.rs:4", "pub fn todo_free() {}");
const MAIN_TODO: (&str, &str) = ("src/main.rs:2", "// TODO tidy");

/// What the panel's hints read at 100x30, where there's no room for the
/// arrows: `tab` is the field Tab goes to.
fn hints(tab: &str) -> String {
    hints_text(&[
        ("⏎", "open"),
        ("Tab", tab),
        ("Alt+A", "replace all"),
        ("Esc", "close"),
    ])
}

/// `a` moved `t` of the way to `b`, per channel, rounded, as the editor mixes.
fn mix(a: Color, b: Color, t: f64) -> Color {
    let (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) = (a, b) else {
        panic!("can't mix {a:?} and {b:?}");
    };
    let m = |p: u8, q: u8| (f64::from(p) + (f64::from(q) - f64::from(p)) * t).round() as u8;
    Color::Rgb(m(r1, r2), m(g1, g2), m(b1, b2))
}

/// A hit row as the list shows it: `path:line` padded to the column, two
/// blanks, then the line.
fn hit_row((place, text): (&str, &str)) -> String {
    format!("{place:<PLACE_WIDTH$}  {text}")
}

/// Tinkery in `dir` with no file open, so the root is that folder.
fn open_in(dir: &Path) -> Tinkery {
    let tinkery = Tinkery::spawn_in_with_config(dir, DEFAULT, &[]);
    tinkery.wait_for_text("Open directory", START);
    tinkery
}

/// Opens the panel, types `query` and presses Enter.
fn search(tinkery: &mut Tinkery, query: &str) {
    tinkery.send_keys("alt+f");
    tinkery.wait_for_text(OPEN, WAIT);
    tinkery.type_text(query);
    tinkery.wait_for_text(&format!("Search   {query}"), WAIT);
    tinkery.send_keys("enter");
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

/// Waits for the row under the fields to say exactly `status`, e.g.
/// `3 matches in 2 files`, and not `searching… 3 matches in 2 files`.
fn wait_for_status(tinkery: &Tinkery, status: &str) {
    tinkery.wait_for_screen(&format!("the panel to say {status:?}"), WAIT, |lines| {
        let shown: String = lines[usize::from(STATUS_ROW)]
            .chars()
            .skip(usize::from(LIST_X))
            .take(usize::from(RIGHT - LIST_X))
            .collect();
        shown.trim_end() == status
    });
}

/// The chips on the Search strip that are on: `acc_ink` on the accent.
fn chips_on(tinkery: &Tinkery) -> String {
    tinkery.bg_text(QUERY_ROW, ACCENT).replace(' ', "")
}

/// Asserts row `y` is drawn as the selected row: a pill of `hov` on the
/// card's `raised` across its inside, `›` in the accent, bold, a cell in; and
/// nothing of the old look, a glow from the card's edge or `⏎` at the right.
fn assert_pill(tinkery: &Tinkery, y: u16) {
    assert_eq!(cell(tinkery, PILL_X, y), CAP_L, "row {y}");
    assert_eq!(cell(tinkery, PILL_END, y), CAP_R, "row {y}");
    for x in [PILL_X, PILL_END] {
        assert_eq!(tinkery.fg_at(x, y), HOV, "the end at {x},{y}");
        assert_eq!(tinkery.bg_at(x, y), RAISED, "the end at {x},{y}");
    }
    assert_eq!(tinkery.text_col(y, "›"), Some(MARK_X), "row {y}");
    assert_eq!(tinkery.fg_at(MARK_X, y), ACCENT, "row {y}");
    assert!(tinkery.bold_at(MARK_X, y), "row {y}");
    assert_ne!(
        tinkery.bg_at(CARD_X, y),
        mix(RAISED, ACCENT, 0.3),
        "row {y}"
    );
    assert!(!row(tinkery, y).contains('⏎'), "row {y}");
}

/// Waits for `hit` to be the selected row `y`: the pill.
fn wait_for_selected(tinkery: &Tinkery, y: u16, hit: (&str, &str)) {
    tinkery.wait_for_bg(MARK_X, y, HOV, WAIT);
    assert_pill(tinkery, y);
    assert_eq!(tinkery.text_col(y, &hit_row(hit)), Some(LIST_X), "{hit:?}");
}

/// Asserts row `y` isn't lit: no pill, no mark.
fn assert_no_pill(tinkery: &Tinkery, y: u16) {
    assert_eq!(tinkery.bg_text(y, HOV), "", "row {y}");
    assert!(!row(tinkery, y).contains('›'), "row {y}");
}

/// Waits for the status line to show `Ln <line>, Col <col>` (see `shows_position`).
fn wait_for_position(tinkery: &Tinkery, line: usize, col: usize) {
    let position = format!("Ln {line}, Col {col}");
    tinkery.wait_for_screen(&position, WAIT, |lines| {
        harness::shows_position(&lines[usize::from(ROWS - 1)], &position)
    });
}

#[test]
fn alt_f_lists_hits_as_path_line_text_respecting_gitignore_and_skipping_binaries() {
    let mut tinkery = open_in(Path::new(PROJECT));
    search(&mut tinkery, "TODO");
    wait_for_status(&tinkery, "3 matches in 2 files");
    assert_eq!(tinkery.text_col(QUERY_ROW, "Search   TODO"), Some(LABEL_X));
    // The chips end the strip's text, both off: `muted` on `btn`.
    let chips = format!("{CAP_L}Aa{CAP_R} {CAP_L}.*{CAP_R}");
    assert_eq!(tinkery.text_col(QUERY_ROW, &chips), Some(RIGHT - 9));
    assert_eq!(tinkery.bg_text(QUERY_ROW, BTN), "Aa.*");
    assert_eq!(tinkery.fg_at(RIGHT - 8, QUERY_ROW), MUTED);
    assert_eq!(chips_on(&tinkery), "");
    // Sorted by path, then line, the text lined up after the widest place.
    for (i, hit) in [LIB_TODO, LIB_FN, MAIN_TODO].into_iter().enumerate() {
        let y = FIRST_ROW + u16::try_from(i).unwrap();
        assert_eq!(tinkery.text_col(y, &hit_row(hit)), Some(LIST_X), "{hit:?}");
    }
    let screen = tinkery.screen().join("\n");
    assert!(!screen.contains("ignored.log"), "{screen}");
    assert!(!screen.contains("data.bin"), "{screen}");
    // The first hit starts selected, its match in `find_current` colours; the
    // others rest and show their match on `find_match_bg`.
    wait_for_selected(&tinkery, FIRST_ROW, LIB_TODO);
    assert_eq!(tinkery.bg_text(FIRST_ROW, WARN), "TODO");
    let match_bg = mix(BG, WARN, 0.3);
    assert_eq!(tinkery.bg_text(FIRST_ROW + 1, match_bg), "todo");
    assert_eq!(tinkery.bg_text(FIRST_ROW + 2, match_bg), "TODO");
    assert_no_pill(&tinkery, FIRST_ROW + 1);
    assert_no_pill(&tinkery, FIRST_ROW + 2);
    // The cursor stays in the query.
    tinkery.wait_for_cursor(VALUE_X + "TODO".len() as u16, QUERY_ROW, WAIT);
}

#[test]
fn the_panel_has_an_accent_edge_two_strips_and_keycap_hints_over_a_dimmed_screen() {
    let mut tinkery = Tinkery::spawn_in_with_config(Path::new(PROJECT), DEFAULT, &["src/lib.rs"]);
    tinkery.wait_for_text("Fixture library", START);
    let y = (0..ROWS)
        .find(|&y| row(&tinkery, y).contains("Fixture library"))
        .unwrap();
    // Above the card, so it's only dimmed.
    assert!(y < CARD_Y, "the editor's first line is on row {y}");
    let x = tinkery.text_col(y, "Fixture").unwrap();
    let (fg, bg) = (tinkery.fg_at(x, y), tinkery.bg_at(x, y));
    search(&mut tinkery, "TODO");
    wait_for_status(&tinkery, "3 matches in 2 files");
    // Text fades into its ground, then all of it goes towards black.
    tinkery.wait_for_fg_at(x, y, mix(mix(fg, bg, 0.6), BLACK, 0.45), WAIT);
    tinkery.wait_for_bg(x, y, mix(bg, BLACK, 0.45), WAIT);

    // The title is in the top edge, which is the accent's, as the sides are.
    let edge = row(&tinkery, CARD_Y);
    assert_eq!(
        tinkery.text_col(CARD_Y, "▁ Project search ▁"),
        Some(CARD_X + 1),
        "{edge}"
    );
    assert!(!edge.contains('▀'), "no lit edge: {edge}");
    assert_eq!(tinkery.fg_at(CARD_X + 30, CARD_Y), ACCENT);
    assert_eq!(cell(&tinkery, CARD_X, QUERY_ROW), "▕");
    assert_eq!(cell(&tinkery, CARD_X + CARD_WIDTH - 1, QUERY_ROW), "▏");
    assert_eq!(cell(&tinkery, CARD_X + 1, BOTTOM_ROW), "▔");
    assert_eq!(tinkery.fg_at(CARD_X, QUERY_ROW), ACCENT);

    // Search and Replace are strips of `raised2` on the card's `raised`, each
    // starting with `✦`; the one with the keys has its label in `text`.
    for (y, label) in [(QUERY_ROW, "Search"), (REPLACE_ROW, "Replace")] {
        assert_eq!(cell(&tinkery, PILL_X, y), CAP_L, "{label}");
        assert_eq!(cell(&tinkery, PILL_END, y), CAP_R, "{label}");
        for x in [PILL_X, PILL_END] {
            assert_eq!(tinkery.fg_at(x, y), RAISED2, "{label}: the end at {x}");
            assert_eq!(tinkery.bg_at(x, y), RAISED, "{label}: the end at {x}");
        }
        assert_eq!(tinkery.bg_at(LABEL_X, y), RAISED2, "{label}");
        assert_eq!(
            tinkery.text_col(y, &format!("✦ {label}")),
            Some(LIST_X),
            "{label}"
        );
        assert_eq!(tinkery.fg_at(LIST_X, y), ACCENT2, "{label}");
    }
    assert_eq!(tinkery.fg_at(LABEL_X, QUERY_ROW), TEXT);
    assert_eq!(tinkery.fg_at(VALUE_X, QUERY_ROW), STRONG);
    assert_eq!(tinkery.fg_at(LABEL_X, REPLACE_ROW), MUTED);
    assert_eq!(
        tinkery.text_col(REPLACE_ROW, "tab to replace"),
        Some(VALUE_X)
    );
    assert_eq!(tinkery.fg_at(VALUE_X, REPLACE_ROW), MUTED);
    // A blank under the strips, then the count in `muted`.
    let blank: String = row(&tinkery, QUERY_ROW + 2)
        .chars()
        .skip(usize::from(PILL_X))
        .take(usize::from(CARD_WIDTH - 2))
        .collect();
    assert_eq!(blank.trim(), "");
    assert_eq!(tinkery.fg_at(LIST_X, STATUS_ROW), MUTED);
    // The last line is the keys, as keycaps, ending `Esc close`.
    assert_eq!(
        tinkery.text_col(HINTS_ROW, &hints("replace")),
        Some(LIST_X),
        "{}",
        row(&tinkery, HINTS_ROW)
    );
    assert!(hints("replace").ends_with(&format!("{CAP_L}Esc{CAP_R} close")));
    assert_eq!(tinkery.bg_at(LIST_X + 1, HINTS_ROW), BTN);
    assert_eq!(tinkery.fg_at(LIST_X + 1, HINTS_ROW), STRONG);
    assert_eq!(tinkery.fg_at(LIST_X + 4, HINTS_ROW), MUTED);
    // The file is the project here, so the hits are `lib.rs:3` and on. The
    // selected one: the file name `strong` bold, its line in `text`.
    tinkery.wait_for_bg(MARK_X, FIRST_ROW, HOV, WAIT);
    assert_pill(&tinkery, FIRST_ROW);
    assert_eq!(tinkery.text_col(FIRST_ROW, "lib.rs:3"), Some(LIST_X));
    assert_eq!(tinkery.fg_at(LIST_X, FIRST_ROW), STRONG);
    assert!(tinkery.bold_at(LIST_X, FIRST_ROW));
    assert_eq!(tinkery.fg_at(LIST_X + 6, FIRST_ROW), TEXT);
    // A resting one: the file name `strong`, its line in `muted`.
    assert_no_pill(&tinkery, FIRST_ROW + 1);
    assert_eq!(tinkery.text_col(FIRST_ROW + 1, "lib.rs:4"), Some(LIST_X));
    assert_eq!(tinkery.fg_at(LIST_X, FIRST_ROW + 1), STRONG);
    assert!(!tinkery.bold_at(LIST_X, FIRST_ROW + 1));
    assert_eq!(tinkery.fg_at(LIST_X + 6, FIRST_ROW + 1), MUTED);
}

#[test]
fn tab_gives_the_replace_strip_the_keys_and_its_label_turns_text() {
    let mut tinkery = open_in(Path::new(PROJECT));
    search(&mut tinkery, "TODO");
    wait_for_status(&tinkery, "3 matches in 2 files");
    assert_eq!(tinkery.fg_at(LABEL_X, QUERY_ROW), TEXT);
    assert_eq!(tinkery.fg_at(LABEL_X, REPLACE_ROW), MUTED);
    tinkery.send_keys("tab");
    tinkery.wait_for_cursor(VALUE_X, REPLACE_ROW, WAIT);
    tinkery.wait_for_fg_at(LABEL_X, REPLACE_ROW, TEXT, WAIT);
    assert_eq!(tinkery.fg_at(LABEL_X, QUERY_ROW), MUTED);
    tinkery.wait_for_text_gone("tab to replace", WAIT);
    // Tab now names the field it goes back to.
    assert_eq!(tinkery.text_col(HINTS_ROW, &hints("search")), Some(LIST_X));
    tinkery.type_text("DONE");
    tinkery.wait_for_text("Replace  DONE", WAIT);
    assert_eq!(
        tinkery.text_col(REPLACE_ROW, "Replace  DONE"),
        Some(LABEL_X)
    );
    assert_eq!(tinkery.fg_at(VALUE_X, REPLACE_ROW), STRONG);
    tinkery.wait_for_cursor(VALUE_X + 4, REPLACE_ROW, WAIT);
    // The hits stay as they were, the first still selected.
    wait_for_selected(&tinkery, FIRST_ROW, LIB_TODO);
    tinkery.send_keys("tab");
    tinkery.wait_for_cursor(VALUE_X + 4, QUERY_ROW, WAIT);
    tinkery.wait_for_fg_at(LABEL_X, QUERY_ROW, TEXT, WAIT);
    assert_eq!(tinkery.text_col(HINTS_ROW, &hints("replace")), Some(LIST_X));
}

#[test]
fn a_click_outside_the_card_closes_it_and_one_inside_does_not() {
    let mut tinkery = open_in(Path::new(PROJECT));
    tinkery.send_keys("alt+f");
    tinkery.wait_for_text(OPEN, WAIT);
    tinkery.click(50, 15);
    // Still open: typing goes to the query.
    tinkery.type_text("x");
    tinkery.wait_for_text("Search   x", WAIT);
    tinkery.click(2, 2);
    tinkery.wait_for_text_gone(OPEN, WAIT);
}

#[test]
fn mono_dims_with_the_modifier_reverses_the_selected_hit_and_underlines_matches() {
    let mut tinkery = Tinkery::spawn_in_with_config(Path::new(PROJECT), "theme = \"mono\"\n", &[]);
    tinkery.wait_for_text("Open directory", START);
    search(&mut tinkery, "TODO");
    wait_for_status(&tinkery, "3 matches in 2 files");
    // The frame with the count has the selection and the dim too.
    let reversed = tinkery.reversed_text(FIRST_ROW);
    assert!(reversed.contains(&hit_row(LIB_TODO)), "{reversed:?}");
    assert!(reversed.trim_start().starts_with('›'), "{reversed:?}");
    // Reverse video runs the card's inside and no other row has any.
    assert_eq!(reversed.chars().count(), usize::from(CARD_WIDTH - 2));
    assert_eq!(tinkery.reversed_text(FIRST_ROW + 1), "");
    // Matches stay bold and underlined, on the selected row and off it.
    assert_eq!(tinkery.underlined_text(FIRST_ROW), "TODO");
    assert_eq!(tinkery.underlined_text(FIRST_ROW + 1), "todo");
    assert_eq!(tinkery.underlined_text(FIRST_ROW + 2), "TODO");
    for (y, hit) in [(FIRST_ROW, LIB_TODO), (FIRST_ROW + 1, LIB_FN)] {
        let at = tinkery.text_col(y, hit.1).unwrap();
        let matched = at + u16::try_from(hit.1.to_lowercase().find("todo").unwrap()).unwrap();
        assert!(tinkery.bold_at(matched, y), "row {y}");
        assert!(!tinkery.bold_at(matched + 4, y), "row {y}");
    }
    // No Nerd Font glyph: the strips and keycaps have square ends.
    let screen = tinkery.screen().join("\n");
    assert!(
        !screen.contains(CAP_L) && !screen.contains(CAP_R),
        "{screen}"
    );
    assert_eq!(
        tinkery.text_col(
            HINTS_ROW,
            " ⏎  open    Tab  replace    Alt+A  replace all    Esc  close"
        ),
        Some(LIST_X),
        "{}",
        row(&tinkery, HINTS_ROW)
    );
    let status = ROWS - 1;
    // The splash is still up under the card, so its keys are in the status line.
    let position = tinkery.text_col(status, "quit").unwrap();
    assert!(
        tinkery.dim_at(position, status),
        "the status line is dimmed"
    );
    assert!(!tinkery.dim_at(LIST_X, STATUS_ROW), "the card isn't");
}

#[test]
fn tinkery_light_draws_the_same_panel_in_its_own_colours() {
    let mut tinkery =
        Tinkery::spawn_in_with_config(Path::new(PROJECT), "theme = \"tinkery-light\"\n", &[]);
    tinkery.wait_for_text("Open directory", START);
    search(&mut tinkery, "TODO");
    wait_for_status(&tinkery, "3 matches in 2 files");
    // tinkery-light's `raised`, `raised2`, `hov`, `accent`, `bg` and `warn`.
    let raised = Color::Rgb(0xe4, 0xdc, 0xd0);
    let raised2 = Color::Rgb(0xd9, 0xcf, 0xc2);
    let hov = Color::Rgb(0xd3, 0xc7, 0xb8);
    let accent = Color::Rgb(0xb8, 0x4a, 0x08);
    let bg = Color::Rgb(0xf5, 0xf0, 0xe9);
    let warn = Color::Rgb(0x9a, 0x6a, 0x00);
    tinkery.wait_for_bg(MARK_X, FIRST_ROW, hov, WAIT);
    assert_eq!(tinkery.fg_at(MARK_X, FIRST_ROW), accent);
    assert_eq!(cell(&tinkery, PILL_X, FIRST_ROW), CAP_L);
    assert_eq!(tinkery.fg_at(PILL_X, FIRST_ROW), hov);
    assert_eq!(tinkery.bg_at(PILL_X, FIRST_ROW), raised);
    assert_eq!(tinkery.bg_at(LABEL_X, QUERY_ROW), raised2);
    assert_eq!(tinkery.bg_at(LABEL_X, REPLACE_ROW), raised2);
    assert_eq!(tinkery.bg_at(LIST_X, STATUS_ROW), raised);
    assert_eq!(tinkery.fg_at(CARD_X, QUERY_ROW), accent);
    assert_eq!(tinkery.bg_text(FIRST_ROW, warn), "TODO");
    assert_eq!(tinkery.bg_text(FIRST_ROW + 1, mix(bg, warn, 0.3)), "todo");
    assert_eq!(tinkery.text_col(HINTS_ROW, &hints("replace")), Some(LIST_X));
}

#[test]
fn alt_c_and_alt_r_toggle_case_and_regex_and_search_again() {
    let mut tinkery = open_in(Path::new(PROJECT));
    search(&mut tinkery, "T.DO");
    wait_for_status(&tinkery, "no matches");
    // As a regex `.` matches the `O`; case still ignored, so `todo_free` too.
    tinkery.send_keys("alt+r");
    wait_for_status(&tinkery, "3 matches in 2 files");
    assert_eq!(chips_on(&tinkery), ".*");
    assert_eq!(tinkery.bg_text(QUERY_ROW, BTN), "Aa");
    tinkery.send_keys("alt+c");
    wait_for_status(&tinkery, "2 matches in 2 files");
    assert_eq!(chips_on(&tinkery), "Aa.*");
    assert_eq!(tinkery.bg_text(QUERY_ROW, BTN), "");
    tinkery.wait_for_text_gone("todo_free", WAIT);
    assert_eq!(
        tinkery.text_col(FIRST_ROW, &hit_row(LIB_TODO)),
        Some(LIST_X)
    );
    assert_eq!(
        tinkery.text_col(FIRST_ROW + 1, &hit_row(MAIN_TODO)),
        Some(LIST_X)
    );

    // A bad regex says so instead of listing anything.
    tinkery.type_text("(");
    tinkery.send_keys("enter");
    wait_for_status(&tinkery, "invalid regex");
    assert_eq!(tinkery.fg_at(LIST_X, STATUS_ROW), ERR);
    tinkery.wait_for_text_gone("src/lib.rs", WAIT);

    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(OPEN, WAIT);
}

#[test]
fn arrows_move_through_hits_and_enter_opens_the_file_at_the_match() {
    let mut tinkery = open_in(Path::new(PROJECT));
    search(&mut tinkery, "TODO");
    wait_for_status(&tinkery, "3 matches in 2 files");
    tinkery.send_keys("down");
    tinkery.send_keys("down");
    wait_for_selected(&tinkery, FIRST_ROW + 2, MAIN_TODO);
    assert_no_pill(&tinkery, FIRST_ROW);
    tinkery.send_keys("up");
    wait_for_selected(&tinkery, FIRST_ROW + 1, LIB_FN);
    assert_no_pill(&tinkery, FIRST_ROW + 2);
    tinkery.send_keys("down");
    wait_for_selected(&tinkery, FIRST_ROW + 2, MAIN_TODO);
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone(OPEN, WAIT);
    tinkery.wait_for_text("1  fn main() {", WAIT);
    // `    // TODO`: the match starts in column 8.
    wait_for_position(&tinkery, 2, 8);
    tinkery.wait_for_screen("main.rs in the tab bar", WAIT, |screen| {
        screen[1].contains("main.rs")
    });

    // Another hit opens beside it in a second tab.
    search(&mut tinkery, "TODO");
    wait_for_status(&tinkery, "3 matches in 2 files");
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone(OPEN, WAIT);
    tinkery.wait_for_text("1  //! Fixture library.", WAIT);
    wait_for_position(&tinkery, 3, 4);
    tinkery.wait_for_screen("both files in the tab bar", WAIT, |screen| {
        screen[1].contains("main.rs") && screen[1].contains("lib.rs")
    });
}

#[test]
fn unsaved_edits_in_open_buffers_are_searched_from_memory() {
    let mut tinkery = open_in(Path::new(PROJECT));
    search(&mut tinkery, "tidy");
    wait_for_status(&tinkery, "1 match in 1 file");
    tinkery.send_keys("enter");
    wait_for_position(&tinkery, 2, 13);
    // Unsaved, so only the buffer has it.
    tinkery.type_text("XYZZY");
    tinkery.wait_for_text("// TODO XYZZYtidy", WAIT);
    search(&mut tinkery, "xyzzy");
    wait_for_status(&tinkery, "1 match in 1 file");
    tinkery.wait_for_bg(MARK_X, FIRST_ROW, HOV, WAIT);
    assert_eq!(
        tinkery.text_col(FIRST_ROW, "src/main.rs:2  // TODO XYZZYtidy"),
        Some(LIST_X)
    );
}

#[test]
fn many_files_stream_in_and_the_panel_keeps_taking_keys() -> std::io::Result<()> {
    let dir = tempfile::tempdir()?;
    for n in 0..3000 {
        std::fs::write(
            dir.path().join(format!("f{n:04}.txt")),
            "nothing\nneedle here\n",
        )?;
    }
    let mut tinkery = open_in(dir.path());
    search(&mut tinkery, "needle");
    // Keys reach the panel straight away, while the search may still be going.
    tinkery.send_keys("down");
    wait_for_status(&tinkery, "3000 matches in 3000 files");
    // The frame with the final count has the selection drawn too.
    assert!(
        (FIRST_ROW..ROWS)
            .any(|y| tinkery.bg_at(MARK_X, y) == HOV
                && row(&tinkery, y).contains(".txt:2  needle here")),
        "{}",
        tinkery.screen().join("\n")
    );
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(OPEN, WAIT);
    Ok(())
}
