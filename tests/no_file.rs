//! The key list the editor area shows whenever no file is open and the splash
//! isn't up (tinkery-splash spec S11): after the last tab closes and after
//! another folder is opened. Where it's drawn, that typing doesn't reach the
//! untitled buffer under it, and what replaces it.

mod harness;

use std::fs;
use std::time::Duration;

use harness::{CAP_L, ROWS, Tinkery, pill_text};
use tempfile::TempDir;

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(5);
const STATUS: u16 = ROWS - 1;
/// The pills sit on row 1, under a blank row 0.
const PILL_ROW: u16 = 1;

/// Without the tree the editor area is the full width from row 3 (below the
/// tab header) to the status line: 100 × 26. The list's header starts a third
/// of the way down, on row 3 + 26 / 3 = 11, and its rows are two apart. The
/// block is 35 wide (the label column at 10, then `cast · files and
/// commands`), centred: (100 − 35) / 2 = 32.
const HEADER_ROW: u16 = 11;
const KEY_ROWS: [u16; 4] = [13, 15, 17, 19];
const X: u16 = 32;
const LABEL_X: u16 = X + 10;

const KEYS: [(&str, &str); 4] = [
    ("Ctrl+P", "cast · files and commands"),
    ("Ctrl+N", "new file"),
    ("Ctrl+B", "toggle tree"),
    ("Ctrl+Q", "quit"),
];

/// A temp folder holding `proj/a.txt` ("alpha") and `proj/b.txt` ("bravo"),
/// with tinkery started on `a.txt` inside `proj`, so the project is `proj` and
/// the tree is hidden.
fn open_file() -> (TempDir, Tinkery) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let proj = dir.path().join("proj");
    fs::create_dir(&proj).expect("create proj");
    fs::write(proj.join("a.txt"), "alpha\n").expect("write a.txt");
    fs::write(proj.join("b.txt"), "bravo\n").expect("write b.txt");
    let tinkery = Tinkery::spawn_in(&proj, &["a.txt"]);
    tinkery.wait_for_text("1  alpha", START);
    (dir, tinkery)
}

/// Waits for the key list, then checks every line of it is where the design
/// puts it, starting from column `x` with its header on `header`.
fn assert_key_list(tinkery: &Tinkery, folder: &str, x: u16, header: u16) {
    tinkery.wait_for_text("no file open", WAIT);
    let screen = tinkery.screen();
    assert_eq!(
        tinkery.text_col(header, &format!("{folder}/  no file open")),
        Some(x),
        "{screen:#?}"
    );
    for (i, (key, label)) in (0u16..).zip(KEYS) {
        let row = header + 2 + 2 * i;
        assert_eq!(tinkery.text_col(row, key), Some(x), "{screen:#?}");
        assert_eq!(tinkery.text_col(row, label), Some(x + 10), "{screen:#?}");
        // The rows are two apart, with nothing between.
        assert_eq!(
            screen[usize::from(row - 1)]
                .chars()
                .skip(usize::from(x))
                .collect::<String>()
                .trim(),
            "",
            "{screen:#?}"
        );
    }
}

/// The pill row shows no tab.
fn assert_no_tabs(tinkery: &Tinkery) {
    let screen = tinkery.screen();
    let pills = &screen[usize::from(PILL_ROW)];
    assert!(!pills.contains(CAP_L), "{screen:#?}");
    assert!(!pills.contains("untitled"), "{screen:#?}");
}

#[test]
fn closing_the_last_tab_shows_the_key_list_under_an_empty_tab_bar() {
    let (_dir, mut tinkery) = open_file();
    tinkery.send_keys("ctrl+w");
    tinkery.wait_for_text_gone("alpha", WAIT);
    assert_key_list(&tinkery, "proj", X, HEADER_ROW);
    for (row, (key, _)) in KEY_ROWS.into_iter().zip(KEYS) {
        assert_eq!(tinkery.text_col(row, key), Some(X));
    }
    assert_eq!(tinkery.text_col(KEY_ROWS[0], "cast"), Some(LABEL_X));
    assert_no_tabs(&tinkery);
    // The status bar names the project, not an untitled buffer.
    let status = tinkery.screen()[usize::from(STATUS)].clone();
    assert!(status.contains("proj"), "{status:?}");
    assert!(!status.contains("untitled"), "{status:?}");
}

#[test]
fn typing_on_the_key_list_does_nothing_and_ctrl_n_replaces_it() {
    let (_dir, mut tinkery) = open_file();
    tinkery.send_keys("ctrl+w");
    tinkery.wait_for_text("no file open", WAIT);
    tinkery.type_text("zq");
    tinkery.send_keys("enter");
    tinkery.send_keys("backspace");
    // Ctrl+B shows the tree beside it; the tree's update proves the keys
    // above were handled first.
    tinkery.send_keys("ctrl+b");
    tinkery.wait_for_text("b.txt", WAIT);
    let screen = tinkery.screen().join("\n");
    assert!(screen.contains("no file open"), "{screen}");
    assert!(!screen.contains("zq"), "{screen}");
    assert!(!screen.contains("untitled"), "{screen}");

    // Ctrl+N gives one empty untitled tab, which takes the typing.
    tinkery.send_keys("ctrl+n");
    tinkery.wait_for_text_gone("no file open", WAIT);
    tinkery.wait_for_text(&pill_text(&["untitled"]), WAIT);
    tinkery.type_text("hi");
    tinkery.wait_for_text("1  hi", WAIT);
    let pills = tinkery.screen()[usize::from(PILL_ROW)].clone();
    assert_eq!(pills.matches("untitled").count(), 1, "{pills:?}");
}

#[test]
fn opening_a_file_from_ctrl_p_replaces_the_key_list() {
    let (_dir, mut tinkery) = open_file();
    tinkery.send_keys("ctrl+w");
    tinkery.wait_for_text("no file open", WAIT);
    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text("cast", WAIT);
    tinkery.type_text("b.txt");
    tinkery.wait_for_text("✦ b.txt", WAIT);
    tinkery.send_keys("enter");
    tinkery.wait_for_text("1  bravo", WAIT);
    tinkery.wait_for_text_gone("no file open", WAIT);
    let pills = tinkery.screen()[usize::from(PILL_ROW)].clone();
    assert!(pills.contains(&pill_text(&["b.txt"])), "{pills:?}");
    assert!(!pills.contains("untitled"), "{pills:?}");
}

#[test]
fn opening_a_folder_shows_the_key_list_beside_its_tree() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let other = dir.path().join("other");
    fs::create_dir(&other).expect("create other");
    fs::write(other.join("inside.txt"), "inside\n").expect("write inside.txt");
    let mut tinkery = Tinkery::spawn_in(dir.path(), &["."]);
    tinkery.wait_for_text("Open directory", START);
    open_other(&mut tinkery);

    // With the tree, the list sits in the editor area right of it; its lines
    // still start in one column, two rows apart.
    tinkery.wait_for_text("inside.txt", WAIT);
    tinkery.wait_for_text("no file open", WAIT);
    let header = (0..ROWS)
        .find(|&row| tinkery.text_col(row, "no file open").is_some())
        .expect("the header is on screen");
    let x = tinkery
        .text_col(header, "other/")
        .expect("the folder heads the list");
    assert_key_list(&tinkery, "other", x, header);
    assert_no_tabs(&tinkery);

    // The tree has the keys: Enter opens the file it selects in place of the
    // list.
    tinkery.send_keys("enter");
    tinkery.wait_for_text("1  inside", WAIT);
    tinkery.wait_for_text_gone("no file open", WAIT);
}

/// Opens `other`, the one folder in the project, through the splash's folder
/// browser: `o`, ↓ to `other`, Enter into it, Enter on its open row. The
/// browser's header is row 6 and its folders start on row 9.
fn open_other(tinkery: &mut Tinkery) {
    tinkery.type_text("o");
    tinkery.wait_for_text("open · folders", WAIT);
    tinkery.wait_for_screen("other in the browser", WAIT, |screen| {
        screen[9].contains("▸ other")
    });
    tinkery.send_keys("down");
    tinkery.send_keys("enter");
    tinkery.wait_for_screen("inside other", WAIT, |screen| screen[6].contains("other"));
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone("open · folders", WAIT);
}
