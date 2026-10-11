mod harness;

use std::fs;
use std::time::Duration;

use harness::{ROWS, Tinkery};

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(5);
const RENDER: &str = "tests/fixtures/render.txt";
const LONG: &str = "tests/fixtures/long.txt";

/// Waits for the status line to show `Ln <line>, Col <col>` (see `shows_position`).
fn wait_for_position(tinkery: &Tinkery, line: usize, col: usize) {
    let position = format!("Ln {line}, Col {col}");
    tinkery.wait_for_text(&position, WAIT);
    let status = tinkery.screen()[usize::from(ROWS - 1)].clone();
    assert!(
        harness::shows_position(&status, &position),
        "status line {status:?} should show {position:?}"
    );
}

/// Column of `text` on `row`, panicking with the screen when it's missing.
fn col_of(tinkery: &Tinkery, row: u16, text: &str) -> u16 {
    tinkery
        .text_col(row, text)
        .unwrap_or_else(|| panic!("{text:?} not on row {row}: {:#?}", tinkery.screen()))
}

/// The screen row whose trimmed text is exactly `text`.
fn row_of(tinkery: &Tinkery, text: &str) -> u16 {
    let screen = tinkery.screen();
    let row = screen
        .iter()
        .position(|line| line.trim() == text)
        .unwrap_or_else(|| panic!("no row reads {text:?}: {screen:#?}"));
    u16::try_from(row).expect("screen rows fit in u16")
}

/// The screen row of buffer line `line` (1-based) while the view is at the top:
/// the editor starts below the three rows of the tab header.
fn row(line: u16) -> u16 {
    line + 2
}

/// Opens a scratch file holding `text`.
fn open(text: &str) -> (tempfile::TempDir, Tinkery) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = dir.path().join("mouse.txt");
    fs::write(&path, text).expect("write fixture");
    let path = path.to_str().expect("temp path is UTF-8").to_string();
    let tinkery = Tinkery::spawn_with_config("", &[&path]);
    tinkery.wait_for_text("Ln 1, Col 1", START);
    (dir, tinkery)
}

#[test]
fn click_places_cursor() {
    let mut tinkery = Tinkery::spawn(&[RENDER]);
    tinkery.wait_for_text("😀 ok", START);

    // Past the gutter and a tab: `\tlet x = 1;`, on the `x`.
    let x = col_of(&tinkery, row(2), "x = 1;");
    tinkery.click(x, row(2));
    wait_for_position(&tinkery, 2, 6);
    tinkery.wait_for_cursor(x, row(2), WAIT);

    // Scrolled sideways: End on the long line 5 scrolls it, then click on `END`.
    tinkery.click(col_of(&tinkery, row(5), "long"), row(5));
    wait_for_position(&tinkery, 5, 1);
    tinkery.send_keys("end");
    let text = fs::read_to_string(RENDER).expect("read fixture");
    let line = text.lines().nth(4).expect("fixture has a line 5");
    let end_col = line.find("END").expect("line 5 ends in END") + 1;
    wait_for_position(&tinkery, 5, line.trim_end().len() + 1);
    let x = col_of(&tinkery, row(5), "END");
    tinkery.click(x, row(5));
    wait_for_position(&tinkery, 5, end_col);
    tinkery.wait_for_cursor(x, row(5), WAIT);
}

#[test]
fn click_accounts_for_vertical_scroll() {
    let mut tinkery = Tinkery::spawn(&[LONG]);
    tinkery.wait_for_text("line 1", START);
    tinkery.send_keys("ctrl+end");
    wait_for_position(&tinkery, 200, 9);
    tinkery.wait_for_text("200  line 200", WAIT);

    let row = row_of(&tinkery, "180  line 180");
    // On the `1` of `180`.
    let x = col_of(&tinkery, row, "line 180") + 5;
    tinkery.click(x, row);
    wait_for_position(&tinkery, 180, 6);
    tinkery.wait_for_cursor(x, row, WAIT);
}

#[test]
fn click_past_end() {
    let (_dir, mut tinkery) = open("one\ntwo");
    tinkery.wait_for_text("2  two", WAIT);
    let base = col_of(&tinkery, row(1), "one");

    tinkery.click(90, row(1));
    wait_for_position(&tinkery, 1, 4);
    tinkery.wait_for_cursor(base + 3, row(1), WAIT);

    // Below the last line: the last line, at the clicked column.
    tinkery.click(base + 1, 15);
    wait_for_position(&tinkery, 2, 2);
    tinkery.wait_for_cursor(base + 1, row(2), WAIT);

    // Below and past the end: the end of the last line.
    tinkery.click(80, 20);
    wait_for_position(&tinkery, 2, 4);
    tinkery.wait_for_cursor(base + 3, row(2), WAIT);
}

#[test]
fn click_on_wide_char() {
    let mut tinkery = Tinkery::spawn(&[RENDER]);
    tinkery.wait_for_text("😀 ok", START);

    // `日本語 ok`: the right half of `本` is still `本`.
    let base = col_of(&tinkery, row(3), "日本語");
    tinkery.click(base + 3, row(3));
    wait_for_position(&tinkery, 3, 2);
    tinkery.wait_for_cursor(base + 2, row(3), WAIT);

    // The spec's visual check: `ok` after `日本語 ` is column 5.
    tinkery.click(col_of(&tinkery, row(3), "ok"), row(3));
    wait_for_position(&tinkery, 3, 5);

    // Either half of the emoji lands on it.
    let base = col_of(&tinkery, row(4), "😀");
    tinkery.click(base + 1, row(4));
    wait_for_position(&tinkery, 4, 1);
    tinkery.wait_for_cursor(base, row(4), WAIT);
}

#[test]
fn clicks_land_past_the_six_cell_gutter() {
    // The gutter is a mark cell, three for the number and two blanks, so the
    // text starts in column 6.
    let (_dir, mut tinkery) = open("abcdef\nghij");
    tinkery.wait_for_text("   2  ghij", WAIT);
    assert_eq!(col_of(&tinkery, row(1), "abcdef"), 6);

    tinkery.click(8, row(1));
    wait_for_position(&tinkery, 1, 3);
    tinkery.wait_for_cursor(8, row(1), WAIT);
    tinkery.click(6, row(1));
    wait_for_position(&tinkery, 1, 1);
    tinkery.wait_for_cursor(6, row(1), WAIT);
    // Anywhere in the gutter is the line's start.
    tinkery.click(10, row(2));
    wait_for_position(&tinkery, 2, 5);
    tinkery.click(3, row(2));
    wait_for_position(&tinkery, 2, 1);
    tinkery.wait_for_cursor(6, row(2), WAIT);

    // A drag from column 7 to 10 covers `bcd`.
    tinkery.drag((7, row(1)), (10, row(1)));
    tinkery.wait_for_reversed(row(1), "bcd", WAIT);
    wait_for_position(&tinkery, 1, 5);
}

#[test]
fn drag_selects() {
    let (_dir, mut tinkery) = open("hello world");
    tinkery.wait_for_text("1  hello world", WAIT);
    let base = col_of(&tinkery, row(1), "hello");

    // From `w` back to `e`: the selection is the same either way round.
    tinkery.drag((base + 6, row(1)), (base + 1, row(1)));
    tinkery.wait_for_reversed(row(1), "ello ", WAIT);
    wait_for_position(&tinkery, 1, 2);

    tinkery.type_text("x");
    tinkery.wait_for_text("1  hxworld", WAIT);
    tinkery.wait_for_reversed(row(1), "", WAIT);
    wait_for_position(&tinkery, 1, 3);

    // Undo puts the dragged-over text back in one step.
    tinkery.send_keys("ctrl+z");
    tinkery.wait_for_text("1  hello world", WAIT);
}

#[test]
fn double_click_selects_word() {
    let (_dir, mut tinkery) = open("foo bar_baz qux");
    tinkery.wait_for_text("1  foo bar_baz qux", WAIT);
    let x = col_of(&tinkery, row(1), "baz");

    tinkery.double_click(x, row(1));
    tinkery.wait_for_reversed(row(1), "bar_baz", WAIT);
    wait_for_position(&tinkery, 1, 12);

    tinkery.type_text("x");
    tinkery.wait_for_text("1  foo x qux", WAIT);
    tinkery.wait_for_reversed(row(1), "", WAIT);
}

#[test]
fn wheel_scrolls() {
    let mut tinkery = Tinkery::spawn(&[LONG]);
    tinkery.wait_for_text("26  line 26", START);
    assert_eq!(tinkery.screen()[3].trim(), "1  line 1");

    tinkery.scroll_down(20, 10);
    tinkery.wait_for_text("29  line 29", WAIT);
    assert_eq!(tinkery.screen()[3].trim(), "4  line 4");
    tinkery.scroll_down(20, 10);
    tinkery.wait_for_text("32  line 32", WAIT);
    assert_eq!(tinkery.screen()[3].trim(), "7  line 7");
    wait_for_position(&tinkery, 1, 1);

    tinkery.scroll_up(20, 10);
    tinkery.wait_for_text_gone("32  line 32", WAIT);
    assert_eq!(tinkery.screen()[3].trim(), "4  line 4");
    wait_for_position(&tinkery, 1, 1);

    // The cursor never moved: typing lands on line 1 and brings it back into view.
    tinkery.type_text("x");
    tinkery.wait_for_text("1  xline 1", WAIT);
    wait_for_position(&tinkery, 1, 2);
}
