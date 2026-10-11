mod harness;

use std::time::Duration;

use harness::{COLS, ROWS, Tinkery};

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(5);
/// Three `foo`s: `one foo`, `two foo three`, `foo`.
const FIND: &str = "tests/fixtures/find.txt";
/// The find bar sits on the row above the status line.
const BAR_ROW: u16 = ROWS - 2;
/// Find covers `[0, floor(W/2))` and the `│` rule sits at `floor(W/2)`.
const RULE_X: u16 = COLS / 2;
/// The Replace field runs from after the rule: ` Replace  ` and then its text.
const REPLACE_TEXT_X: u16 = RULE_X + 11;
/// hydra's `guide`, the rule between the fields.
const GUIDE: vt100::Color = vt100::Color::Rgb(0x17, 0x22, 0x2e);
/// hydra's `text`, the label of the field with focus.
const TEXT: vt100::Color = vt100::Color::Rgb(0xa7, 0xb4, 0xc2);
/// hydra's `muted`, the other label.
const MUTED: vt100::Color = vt100::Color::Rgb(0x71, 0x80, 0x8f);

fn open(path: &str) -> Tinkery {
    let tinkery = Tinkery::spawn(&[path]);
    tinkery.wait_for_text("Ln 1, Col 1", START);
    tinkery.wait_for_text("3  foo", WAIT);
    tinkery
}

fn bar(tinkery: &Tinkery) -> String {
    tinkery.screen()[usize::from(BAR_ROW)].clone()
}

/// Waits for the find bar to end with `count`, as `n/m` or an error.
fn wait_for_count(tinkery: &Tinkery, count: &str) {
    tinkery.wait_for_screen(&format!("the find bar to show {count:?}"), WAIT, |lines| {
        let bar = &lines[usize::from(BAR_ROW)];
        bar.starts_with(" Find  ") && bar.ends_with(&format!("  {count}"))
    });
}

/// Waits for the status line to show `Ln <line>, Col <col>` (see `shows_position`).
fn wait_for_position(tinkery: &Tinkery, line: usize, col: usize) {
    let position = format!("Ln {line}, Col {col}");
    tinkery.wait_for_screen(&position, WAIT, |lines| {
        harness::shows_position(&lines[usize::from(ROWS - 1)], &position)
    });
}

/// Ctrl+R, `find` in the Find field, Tab, `with` in the Replace field.
fn replace(tinkery: &mut Tinkery, find: &str, with: &str) {
    tinkery.send_keys("ctrl+r");
    tinkery.wait_for_text(" Replace  ", WAIT);
    tinkery.type_text(find);
    tinkery.wait_for_text(&format!(" Find  {find}"), WAIT);
    // A key and quick typing after it can arrive as one paste on Windows, so wait
    // for each key to land before the next.
    tinkery.send_keys("tab");
    tinkery.wait_for_cursor(REPLACE_TEXT_X, BAR_ROW, WAIT);
    tinkery.type_text(with);
    tinkery.wait_for_text(&format!(" Replace  {with}"), WAIT);
}

#[test]
fn ctrl_r_shows_find_and_replace_fields_and_tab_moves_between_them() {
    let mut tinkery = open(FIND);
    tinkery.send_keys("ctrl+r");
    tinkery.wait_for_text(" Replace  ", WAIT);
    let row = bar(&tinkery);
    assert!(row.starts_with(" Find  "), "{row:?}");
    assert_eq!(tinkery.text_col(BAR_ROW, "│ Replace  "), Some(RULE_X));
    assert_eq!(tinkery.fg_at(RULE_X, BAR_ROW), GUIDE);
    // Typing starts in Find, whose label is lit while the other is quiet.
    tinkery.wait_for_cursor(7, BAR_ROW, WAIT);
    assert_eq!(tinkery.fg_at(1, BAR_ROW), TEXT);
    assert_eq!(tinkery.fg_at(RULE_X + 2, BAR_ROW), MUTED);
    tinkery.type_text("foo");
    wait_for_count(&tinkery, "1/3");
    tinkery.wait_for_cursor(10, BAR_ROW, WAIT);
    // The field stays put while the count changes width.
    assert_eq!(tinkery.text_col(BAR_ROW, "│ Replace  "), Some(RULE_X));

    tinkery.send_keys("tab");
    tinkery.wait_for_cursor(REPLACE_TEXT_X, BAR_ROW, WAIT);
    // Focus moved, and the labels' colours with it.
    assert_eq!(tinkery.fg_at(1, BAR_ROW), MUTED);
    assert_eq!(tinkery.fg_at(RULE_X + 2, BAR_ROW), TEXT);
    tinkery.type_text("xy");
    tinkery.wait_for_text(" Replace  xy", WAIT);
    tinkery.wait_for_cursor(REPLACE_TEXT_X + 2, BAR_ROW, WAIT);
    // Typing in Replace leaves the pattern and its matches alone.
    assert!(bar(&tinkery).starts_with(" Find  foo "));
    wait_for_count(&tinkery, "1/3");

    tinkery.send_keys("tab");
    tinkery.wait_for_cursor(10, BAR_ROW, WAIT);
    tinkery.type_text("x");
    wait_for_count(&tinkery, "0/0");
}

#[test]
fn enter_in_replace_replaces_the_current_match_and_moves_to_the_next() {
    let mut tinkery = open(FIND);
    replace(&mut tinkery, "foo", "bar");
    wait_for_count(&tinkery, "1/3");

    tinkery.send_keys("enter");
    tinkery.wait_for_text("1  one bar", WAIT);
    wait_for_count(&tinkery, "1/2");
    wait_for_position(&tinkery, 2, 5);
    tinkery.wait_for_reversed(3, "", WAIT);
    tinkery.wait_for_text("2  two foo three", WAIT);

    tinkery.send_keys("enter");
    tinkery.wait_for_text("2  two bar three", WAIT);
    wait_for_count(&tinkery, "1/1");
    wait_for_position(&tinkery, 3, 1);
    tinkery.wait_for_text("3  foo", WAIT);
    // The buffer has unsaved changes now.
    tinkery.wait_for_text("find.txt •", WAIT);
}

#[test]
fn alt_a_replaces_all_reports_the_count_and_undoes_in_one_step() {
    let mut tinkery = open(FIND);
    replace(&mut tinkery, "foo", "quux");
    wait_for_count(&tinkery, "1/3");

    tinkery.send_keys("alt+a");
    tinkery.wait_for_text("Replaced 3", WAIT);
    tinkery.wait_for_text("1  one quux", WAIT);
    tinkery.wait_for_text("2  two quux three", WAIT);
    tinkery.wait_for_text("3  quux", WAIT);
    wait_for_count(&tinkery, "0/0");

    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(" Find  ", WAIT);
    tinkery.send_keys("ctrl+z");
    tinkery.wait_for_text("1  one foo", WAIT);
    tinkery.wait_for_text("2  two foo three", WAIT);
    tinkery.wait_for_text("3  foo", WAIT);
}

#[test]
fn regex_replace_expands_groups() {
    let mut tinkery = open(FIND);
    replace(&mut tinkery, r"(\w+) foo", "$1-baz");
    // Literal text until regex is on.
    wait_for_count(&tinkery, "0/0");
    tinkery.send_keys("alt+r");
    wait_for_count(&tinkery, "1/2");

    tinkery.send_keys("alt+a");
    tinkery.wait_for_text("Replaced 2", WAIT);
    tinkery.wait_for_text("1  one-baz", WAIT);
    tinkery.wait_for_text("2  two-baz three", WAIT);
    tinkery.wait_for_text("3  foo", WAIT);
}
