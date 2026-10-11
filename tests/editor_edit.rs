mod harness;

use std::time::Duration;

use harness::{ROWS, Tinkery};

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(5);
/// The editor's first row, below the tab header.
const TOP: u16 = 3;

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

/// Waits until editor row `row` (0 is the first, on screen row `TOP`) reads
/// `text` after the gutter.
fn wait_for_row(tinkery: &Tinkery, row: u16, number: usize, text: &str) {
    let expected = format!("{number}  {text}");
    tinkery.wait_for_text(&expected, WAIT);
    let line = tinkery.screen()[usize::from(TOP + row)].clone();
    assert_eq!(line.trim(), expected.trim_end(), "{:#?}", tinkery.screen());
}

/// An untitled buffer, with the starting cell of its text.
fn start_empty(config: &str) -> (Tinkery, u16) {
    let mut tinkery = Tinkery::spawn_with_config(config, &[]);
    tinkery.wait_for_text("Open directory", START);
    // Leave the splash for the untitled buffer under it.
    tinkery.send_keys("ctrl+n");
    tinkery.wait_for_text("   1", WAIT);
    // Text starts two blank cells after the line number. The terminal cursor
    // moves there only after the frame is drawn, so wait for it rather than read
    // it.
    let number = tinkery
        .text_col(TOP, "1")
        .unwrap_or_else(|| panic!("no gutter: {:#?}", tinkery.screen()));
    let x = number + 3;
    tinkery.wait_for_cursor(x, TOP, WAIT);
    (tinkery, x)
}

fn assert_one_line(tinkery: &Tinkery) {
    let screen = tinkery.screen();
    let second = screen.iter().any(|row| row.trim_start().starts_with("2 "));
    assert!(!second, "lines should have joined: {screen:#?}");
}

/// Every test here edits an untitled buffer, so Ctrl+Q asks first; discard.
fn quit(tinkery: &mut Tinkery) {
    tinkery.send_keys("ctrl+q");
    tinkery.wait_for_text("has unsaved changes", WAIT);
    tinkery.type_text("d");
    let status = tinkery.wait_exit(WAIT);
    assert!(status.success(), "tinkery exited with {status:?}");
}

#[test]
fn typing_inserts() {
    let (mut tinkery, x) = start_empty("");

    tinkery.type_text("hello");
    wait_for_row(&tinkery, 0, 1, "hello");
    wait_for_position(&tinkery, 1, 6);
    tinkery.wait_for_cursor(x + 5, TOP, WAIT);

    // Typing in the middle pushes the rest along.
    tinkery.send_keys("left");
    tinkery.send_keys("left");
    wait_for_position(&tinkery, 1, 4);
    tinkery.type_text("p 日");
    wait_for_row(&tinkery, 0, 1, "help 日lo");
    wait_for_position(&tinkery, 1, 7);
    // The wide character fills two cells.
    tinkery.wait_for_cursor(x + 7, TOP, WAIT);

    quit(&mut tinkery);
}

#[test]
fn enter_tab_and_typing_match_the_visual_check() {
    // R6's plain Enter, with no closer for the `{` to push down.
    let (mut tinkery, x) = start_empty("[editor]\nauto_pairs = false\n");

    tinkery.type_text("fn x() {");
    wait_for_row(&tinkery, 0, 1, "fn x() {");
    tinkery.send_keys("enter");
    wait_for_position(&tinkery, 2, 1);
    tinkery.send_keys("tab");
    wait_for_position(&tinkery, 2, 5);
    tinkery.type_text("y");
    wait_for_position(&tinkery, 2, 6);
    wait_for_row(&tinkery, 1, 2, "    y");
    wait_for_row(&tinkery, 0, 1, "fn x() {");
    tinkery.wait_for_cursor(x + 5, TOP + 1, WAIT);

    // Enter on an indented line keeps the indent.
    tinkery.send_keys("enter");
    wait_for_position(&tinkery, 3, 5);
    tinkery.type_text("z");
    wait_for_row(&tinkery, 2, 3, "    z");

    quit(&mut tinkery);
}

#[test]
fn brackets_close_themselves() {
    let (mut tinkery, x) = start_empty("");

    tinkery.type_text("foo(");
    wait_for_row(&tinkery, 0, 1, "foo()");
    wait_for_position(&tinkery, 1, 5);
    tinkery.wait_for_cursor(x + 4, TOP, WAIT);

    // The typed closer steps over the one Tinkery put in.
    tinkery.type_text("bar)");
    wait_for_row(&tinkery, 0, 1, "foo(bar)");
    wait_for_position(&tinkery, 1, 9);
    tinkery.wait_for_cursor(x + 8, TOP, WAIT);

    // Before a word an opener comes alone.
    tinkery.send_keys("home");
    tinkery.type_text("[");
    wait_for_row(&tinkery, 0, 1, "[foo(bar)");

    quit(&mut tinkery);
}

#[test]
fn backspace_in_an_empty_pair_deletes_both() {
    let (mut tinkery, _) = start_empty("");

    tinkery.type_text("x[");
    wait_for_row(&tinkery, 0, 1, "x[]");
    tinkery.send_keys("backspace");
    // "x" alone is also a prefix of the old row, so wait for the cursor first.
    wait_for_position(&tinkery, 1, 2);
    wait_for_row(&tinkery, 0, 1, "x");

    quit(&mut tinkery);
}

#[test]
fn enter_in_a_pair_opens_an_indented_line() {
    let (mut tinkery, x) = start_empty("");

    tinkery.type_text("if x {");
    wait_for_row(&tinkery, 0, 1, "if x {}");
    tinkery.send_keys("enter");
    wait_for_position(&tinkery, 2, 5);
    tinkery.type_text("y");
    wait_for_row(&tinkery, 1, 2, "    y");
    wait_for_row(&tinkery, 0, 1, "if x {");
    wait_for_row(&tinkery, 2, 3, "}");
    tinkery.wait_for_cursor(x + 5, TOP + 1, WAIT);

    quit(&mut tinkery);
}

#[test]
fn auto_pairs_off_types_brackets_alone() {
    let (mut tinkery, _) = start_empty("[editor]\nauto_pairs = false\n");

    tinkery.type_text("foo(");
    wait_for_row(&tinkery, 0, 1, "foo(");
    wait_for_position(&tinkery, 1, 5);
    tinkery.type_text(")");
    wait_for_row(&tinkery, 0, 1, "foo()");
    wait_for_position(&tinkery, 1, 6);

    quit(&mut tinkery);
}

#[test]
fn backspace_and_delete_join_lines() {
    let (mut tinkery, _) = start_empty("");

    tinkery.type_text("ab");
    tinkery.send_keys("enter");
    tinkery.type_text("cd");
    wait_for_row(&tinkery, 1, 2, "cd");

    // Backspace at column 0 joins with the line above.
    tinkery.send_keys("home");
    wait_for_position(&tinkery, 2, 1);
    tinkery.send_keys("backspace");
    wait_for_row(&tinkery, 0, 1, "abcd");
    wait_for_position(&tinkery, 1, 3);
    assert_one_line(&tinkery);

    tinkery.send_keys("backspace");
    wait_for_row(&tinkery, 0, 1, "acd");
    tinkery.send_keys("delete");
    wait_for_row(&tinkery, 0, 1, "ad");
    wait_for_position(&tinkery, 1, 2);

    // Delete at line end joins with the (empty) line below.
    tinkery.send_keys("end");
    tinkery.send_keys("enter");
    wait_for_position(&tinkery, 2, 1);
    tinkery.send_keys("left");
    wait_for_position(&tinkery, 1, 3);
    tinkery.send_keys("delete");
    // Row 0 already read `ad`; typing proves the delete was handled first.
    tinkery.type_text("x");
    wait_for_row(&tinkery, 0, 1, "adx");
    assert_one_line(&tinkery);

    // Nothing to delete at either edge of the document.
    tinkery.send_keys("ctrl+end");
    tinkery.send_keys("delete");
    tinkery.send_keys("ctrl+home");
    tinkery.send_keys("backspace");
    tinkery.type_text("!");
    wait_for_row(&tinkery, 0, 1, "!adx");

    quit(&mut tinkery);
}

#[test]
fn tab_inserts_a_tab_character_when_spaces_are_off() {
    let (mut tinkery, x) = start_empty("[editor]\ninsert_spaces = false\ntab_width = 8\n");

    tinkery.type_text("ab");
    tinkery.send_keys("tab");
    // One char, drawn as far as the next 8-column stop.
    wait_for_position(&tinkery, 1, 4);
    tinkery.wait_for_cursor(x + 8, TOP, WAIT);
    tinkery.type_text("c");
    wait_for_row(&tinkery, 0, 1, "ab      c");

    quit(&mut tinkery);
}
