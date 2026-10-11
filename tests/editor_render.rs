mod harness;

use std::fs;
use std::time::Duration;

use harness::{COLS, ROWS, Tinkery, pill_text};
use vt100::Color;

const START: Duration = Duration::from_secs(10);
const EXIT: Duration = Duration::from_secs(5);
const WAIT: Duration = Duration::from_secs(5);
const FIXTURE: &str = "tests/fixtures/render.txt";

fn status_line(tinkery: &Tinkery) -> String {
    tinkery.screen()[usize::from(ROWS) - 1].clone()
}

fn quit(tinkery: &mut Tinkery) {
    tinkery.send_keys("ctrl+q");
    let status = tinkery.wait_exit(EXIT);
    assert!(status.success(), "tinkery exited with {status:?}");
}

/// Column of `text` on `row`, panicking with the screen when it's missing.
fn col_of(tinkery: &Tinkery, row: u16, text: &str) -> u16 {
    tinkery
        .text_col(row, text)
        .unwrap_or_else(|| panic!("{text:?} not on row {row}: {:#?}", tinkery.screen()))
}

#[test]
fn opens_file() {
    let mut tinkery = Tinkery::spawn(&[FIXTURE]);
    tinkery.wait_for_text("fn main() {", START);

    let text_col = col_of(&tinkery, 3, "fn main() {");
    // Each line's number sits in the same right-aligned column, left of the text.
    for (row, number) in (3u16..7).zip(["1", "2", "3", "4"]) {
        let number_col = col_of(&tinkery, row, number);
        assert!(number_col < text_col, "row {row}: {:#?}", tinkery.screen());
        assert_eq!(
            number_col,
            col_of(&tinkery, 3, "1"),
            "gutter misaligned: {:#?}",
            tinkery.screen()
        );
    }
    let status = status_line(&tinkery);
    assert!(status.contains("render.txt"), "status line: {status:?}");

    quit(&mut tinkery);
}

#[test]
fn tabs_and_wide_chars() {
    let mut tinkery = Tinkery::spawn(&[FIXTURE]);
    tinkery.wait_for_text("😀 ok", START);

    let base = col_of(&tinkery, 3, "fn main() {");
    // `\tlet`: the tab fills to column 4 of the text area.
    assert_eq!(col_of(&tinkery, 4, "let x = 1;"), base + 4);
    // `日本語 ok`: three 2-wide characters and a space.
    assert_eq!(col_of(&tinkery, 5, "日本語"), base);
    assert_eq!(col_of(&tinkery, 5, "ok"), base + 7);
    // `😀 ok`: one 2-wide emoji and a space.
    assert_eq!(col_of(&tinkery, 6, "😀"), base);
    assert_eq!(col_of(&tinkery, 6, "ok"), base + 3);

    quit(&mut tinkery);
}

#[test]
fn long_line_is_cut() {
    let dir = tempfile::tempdir().unwrap();
    let long = format!("start{}END", "x".repeat(300));
    fs::write(dir.path().join("long.txt"), format!("{long}\nsecond\n")).unwrap();

    let mut tinkery = Tinkery::spawn_in(dir.path(), &["long.txt"]);
    tinkery.wait_for_text("second", START);

    let screen = tinkery.screen();
    // The first line fills the row to the right edge and stops there.
    assert_eq!(screen[3].chars().count(), usize::from(COLS), "{screen:#?}");
    assert!(screen[3].ends_with('x'), "{screen:#?}");
    assert!(!screen.iter().any(|row| row.contains("END")), "{screen:#?}");
    // Nothing wrapped: the next row is the file's second line.
    assert!(screen[4].contains("second"), "{screen:#?}");
    assert!(screen[4].trim_start().starts_with('2'), "{screen:#?}");

    quit(&mut tinkery);
}

#[test]
fn no_path_opens_untitled_under_the_splash() {
    let mut tinkery = Tinkery::spawn(&[]);
    tinkery.wait_for_text("New file", START);
    // Esc leaves the splash for the untitled buffer it stood in for.
    tinkery.send_keys("esc");
    tinkery.wait_for_text("untitled", WAIT);
    tinkery.wait_for_text_gone("New file", WAIT);

    let screen = tinkery.screen();
    assert!(status_line(&tinkery).contains("untitled"), "{screen:#?}");
    // The tab header (blank, pill, blank), one empty line numbered 1, and
    // nothing else.
    assert_eq!(screen[0].trim(), "", "{screen:#?}");
    assert_eq!(screen[1].trim(), pill_text(&["untitled"]), "{screen:#?}");
    assert_eq!(screen[2].trim(), "", "{screen:#?}");
    assert_eq!(screen[3].trim(), "1", "{screen:#?}");
    assert!(
        screen[4..usize::from(ROWS) - 1]
            .iter()
            .all(|row| row.is_empty())
    );

    quit(&mut tinkery);
}

#[test]
fn missing_file_opens_empty_with_its_name() {
    let dir = tempfile::tempdir().unwrap();
    let mut tinkery = Tinkery::spawn_in(dir.path(), &["missing.txt"]);
    tinkery.wait_for_text("missing.txt", START);

    let screen = tinkery.screen();
    assert!(status_line(&tinkery).contains("missing.txt"), "{screen:#?}");
    assert_eq!(screen[3].trim(), "1", "{screen:#?}");
    // Opening alone never creates the file; saving (#7) does.
    assert!(!dir.path().join("missing.txt").exists());

    quit(&mut tinkery);
}

#[test]
fn non_utf8_file_is_not_opened() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("bad.txt"), [b'o', b'k', 0xff, 0xfe, b'\n']).unwrap();

    let mut tinkery = Tinkery::spawn_in(dir.path(), &["bad.txt"]);
    // The reason is a toast; the status line keeps the untitled buffer's name.
    tinkery.wait_for_toast("✕ cannot open bad.txt: not UTF-8", START);

    let screen = tinkery.screen();
    assert!(status_line(&tinkery).contains("untitled"), "{screen:#?}");
    assert_eq!(screen[3].trim(), "1", "{screen:#?}");

    quit(&mut tinkery);
}

// The `aurora` roles the gutter and cursor line use (README §4.1).
const AURORA_BG: Color = Color::Rgb(0x0b, 0x0a, 0x10);
const AURORA_ACCENT: Color = Color::Rgb(0xb6, 0x9c, 0xff);
const AURORA_CUR_LINE: Color = Color::Rgb(0x13, 0x11, 0x1c);
const AURORA_GUTTER: Color = Color::Rgb(0x3a, 0x35, 0x50);
const AURORA_TEXT: Color = Color::Rgb(0xa6, 0xa2, 0xbb);
const AURORA_COMMENT: Color = Color::Rgb(0x5c, 0x57, 0x73);
const AURORA: &str = "theme = \"aurora\"\n";
/// The first editor row, below the tab header.
const TOP: u16 = 3;

#[test]
fn gutter_is_a_mark_cell_a_three_cell_number_and_two_blanks() {
    let mut tinkery = Tinkery::spawn(&[FIXTURE]);
    tinkery.wait_for_text("fn main() {", START);

    let screen = tinkery.screen();
    // One cell for the diagnostic mark, the number right-aligned in three, two
    // blanks, then the text: no divider rule.
    assert!(screen[3].starts_with("   1  fn main() {"), "{screen:#?}");
    assert!(screen[5].starts_with("   3  日本語 ok"), "{screen:#?}");
    assert_eq!(col_of(&tinkery, TOP, "fn main() {"), 6);
    let editor = &screen[3..usize::from(ROWS) - 1];
    assert!(editor.iter().all(|row| !row.contains('│')), "{screen:#?}");

    quit(&mut tinkery);
}

#[test]
fn the_focused_cursor_row_is_flat_and_lights_its_number() {
    let mut tinkery = Tinkery::spawn_with_config(AURORA, &[FIXTURE]);
    tinkery.wait_for_text("fn main() {", START);

    // `cur_line` from the mark cell to the right edge: a flat fill, no ramp.
    tinkery.wait_for_bg(0, TOP, AURORA_CUR_LINE, START);
    for x in 0..COLS {
        assert_eq!(tinkery.bg_at(x, TOP), AURORA_CUR_LINE, "column {x}");
    }
    // Other rows stay on the ground.
    assert_eq!(tinkery.bg_at(0, TOP + 1), AURORA_BG);
    assert_eq!(tinkery.bg_at(COLS / 2, TOP + 1), AURORA_BG);
    // The cursor line's number is `accent` bold; the rest are `gutter`.
    assert_eq!(tinkery.fg_at(3, TOP), AURORA_ACCENT);
    assert!(tinkery.bold_at(3, TOP));
    assert_eq!(tinkery.fg_at(3, TOP + 1), AURORA_GUTTER);
    assert!(!tinkery.bold_at(3, TOP + 1));

    // The fill follows the cursor.
    tinkery.send_keys("down");
    tinkery.wait_for_bg(0, TOP + 1, AURORA_CUR_LINE, WAIT);
    assert_eq!(tinkery.bg_at(COLS - 1, TOP + 1), AURORA_CUR_LINE);
    assert_eq!(tinkery.bg_at(0, TOP), AURORA_BG);
    assert_eq!(tinkery.fg_at(3, TOP + 1), AURORA_ACCENT);

    quit(&mut tinkery);
}

#[test]
fn an_unfocused_split_has_no_fill_and_a_text_number() {
    let mut tinkery = Tinkery::spawn_with_config(AURORA, &[FIXTURE]);
    tinkery.wait_for_text("fn main() {", START);
    // Two splits: 0..50, the divider, then the new, focused one at 51..100.
    tinkery.send_keys("alt+v");
    let right = 51;
    // The flat fill at `right` was there before the split too, so the lit
    // number is what says the split has been drawn.
    tinkery.wait_for_fg_at(right + 3, TOP, AURORA_ACCENT, WAIT);
    assert_eq!(tinkery.bg_at(right, TOP), AURORA_CUR_LINE);
    assert_eq!(tinkery.bg_at(right + 25, TOP), AURORA_CUR_LINE);
    assert_eq!(tinkery.bg_at(COLS - 1, TOP), AURORA_CUR_LINE);
    assert!(tinkery.bold_at(right + 3, TOP));

    // The left split shows the same cursor row unlit.
    tinkery.wait_for_bg(0, TOP, AURORA_BG, WAIT);
    assert_eq!(tinkery.bg_at(25, TOP), AURORA_BG);
    assert_eq!(tinkery.fg_at(3, TOP), AURORA_TEXT);
    assert!(!tinkery.bold_at(3, TOP));

    quit(&mut tinkery);
}

#[test]
fn comments_are_italic() {
    let mut tinkery = Tinkery::spawn_with_config(AURORA, &["tests/fixtures/highlight/sample.rs"]);
    tinkery.wait_for_text("// A sample for the highlight tests.", START);

    let comment = col_of(&tinkery, TOP, "// A sample");
    // Highlighting may land a frame after the text; the comment colour says it has.
    tinkery.wait_for_fg_at(comment, TOP, AURORA_COMMENT, WAIT);
    assert!(tinkery.italic_at(comment, TOP));
    assert!(tinkery.italic_at(comment + 5, TOP));
    // Code is upright.
    let code = col_of(&tinkery, TOP + 1, "use std::fmt;");
    assert!(!tinkery.italic_at(code, TOP + 1));

    quit(&mut tinkery);
}

#[test]
fn mono_has_no_fill_but_a_bold_number() {
    let mut tinkery = Tinkery::spawn_with_config("theme = \"mono\"\n", &[FIXTURE]);
    tinkery.wait_for_text("fn main() {", START);

    assert!(tinkery.screen()[3].starts_with("   1  fn main() {"));
    assert!(tinkery.bold_at(3, TOP));
    assert!(!tinkery.bold_at(3, TOP + 1));
    assert_eq!(tinkery.bg_at(0, TOP), Color::Default);
    assert_eq!(tinkery.bg_at(COLS / 2, TOP), Color::Default);

    quit(&mut tinkery);
}
