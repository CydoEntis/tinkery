mod harness;

use std::time::Duration;

use harness::{ROWS, Tinkery};

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(5);

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

/// Waits until the editor rows read exactly `lines` (after the gutter), with
/// nothing numbered below them.
fn wait_for_lines(tinkery: &Tinkery, lines: &[&str]) {
    let last = format!("{}  {}", lines.len(), lines[lines.len() - 1]);
    tinkery.wait_for_text(last.trim_end(), WAIT);
    // The next line's gutter: its number right-aligned in three cells.
    let next = format!(" {:>3}", lines.len() + 1);
    tinkery.wait_for_text_gone(&next, WAIT);
    let screen = tinkery.screen();
    for (row, text) in lines.iter().enumerate() {
        let expected = format!("{}  {text}", row + 1);
        // Rows 0-2 are the tab header.
        assert_eq!(screen[row + 3].trim(), expected.trim_end(), "{screen:#?}");
    }
}

#[test]
fn undo_redo_typing() {
    let mut tinkery = Tinkery::spawn_with_config("", &[]);
    tinkery.wait_for_text("Open directory", START);
    // Leave the splash for the untitled buffer under it.
    tinkery.send_keys("ctrl+n");

    // Each step waits for the screen: on Windows, keys that arrive in one burst with
    // a line break are taken for a paste, which undoes as a single step.
    tinkery.type_text("abc");
    wait_for_position(&tinkery, 1, 4);
    tinkery.send_keys("enter");
    wait_for_position(&tinkery, 2, 1);
    tinkery.type_text("def");
    wait_for_lines(&tinkery, &["abc", "def"]);
    wait_for_position(&tinkery, 2, 4);

    // The second run of typing goes first; the cursor returns to where it began.
    tinkery.send_keys("ctrl+z");
    tinkery.wait_for_text_gone("def", WAIT);
    wait_for_lines(&tinkery, &["abc", ""]);
    wait_for_position(&tinkery, 2, 1);

    // `abc` and the Enter that ended it undo together.
    tinkery.send_keys("ctrl+z");
    tinkery.wait_for_text_gone("abc", WAIT);
    wait_for_lines(&tinkery, &[""]);
    wait_for_position(&tinkery, 1, 1);

    tinkery.send_keys("ctrl+y");
    wait_for_lines(&tinkery, &["abc", ""]);
    wait_for_position(&tinkery, 2, 1);
    tinkery.send_keys("ctrl+y");
    wait_for_lines(&tinkery, &["abc", "def"]);
    wait_for_position(&tinkery, 2, 4);

    // Undo marked the buffer changed, so quitting still asks.
    tinkery.send_keys("ctrl+q");
    tinkery.wait_for_text("has unsaved changes", WAIT);
    tinkery.type_text("d");
    let status = tinkery.wait_exit(WAIT);
    assert!(status.success(), "tinkery exited with {status:?}");
}
