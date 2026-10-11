mod harness;

use std::fs;
use std::path::Path;
use std::time::Duration;

use harness::{CAP_L, CAP_R, COLS, ROWS, Tinkery, pill_text, square_pill_text};
use tempfile::TempDir;
use vt100::Color;

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(5);
const PROMPT: &str = "has unsaved changes";
/// The tree's width: the design's `tw`.
const TREE: u16 = 28;
/// The first pill's left end, two columns right of the tree.
const PILL_X: u16 = TREE + 2;
/// The pills sit on row 1, between a blank row 0 and a blank row 2.
const PILL_ROW: u16 = 1;
const UNDER_ROW: u16 = 2;
/// The tree rows of `a.txt` and `b.txt`, below its brand row.
const A_ROW: u16 = 3;
const B_ROW: u16 = 4;

// The `tinkery` roles the header uses (tinkery-forge spec, Palette).
const BG: Color = Color::Rgb(0x11, 0x0f, 0x0e);
const RAISED: Color = Color::Rgb(0x1f, 0x1b, 0x19);
const TEXT: Color = Color::Rgb(0xb5, 0xa8, 0x9d);
const ACCENT: Color = Color::Rgb(0xff, 0x8a, 0x3d);
const ACC_INK: Color = Color::Rgb(0x1f, 0x0d, 0x02);
const WARN: Color = Color::Rgb(0xf2, 0xc1, 0x4e);

/// A temp folder holding `a.txt` ("alpha") and `b.txt` ("bravo"), opened as the
/// project with the tree showing, cwd inside it, under the config `toml`.
fn open_project_with(toml: &str) -> (TempDir, Tinkery) {
    let dir = tempfile::tempdir().expect("create temp dir");
    fs::write(dir.path().join("a.txt"), "alpha\n").expect("write a.txt");
    fs::write(dir.path().join("b.txt"), "bravo\n").expect("write b.txt");
    let mut tinkery = Tinkery::spawn_in_with_config(dir.path(), toml, &["."]);
    tinkery.wait_for_text("Open directory", START);
    // The splash hides the tree; Ctrl+B shows it.
    tinkery.send_keys("ctrl+b");
    tinkery.wait_for_text("b.txt", WAIT);
    (dir, tinkery)
}

fn open_project() -> (TempDir, Tinkery) {
    open_project_with("")
}

/// Opens both files from the tree, leaving b.txt active.
fn open_files(tinkery: &mut Tinkery) {
    tinkery.click(3, A_ROW);
    tinkery.wait_for_text("1  alpha", WAIT);
    tinkery.click(3, B_ROW);
    tinkery.wait_for_text("1  bravo", WAIT);
}

/// `open_files`, then waits for the two round pills with b.txt's the current.
fn open_both(tinkery: &mut Tinkery) {
    open_files(tinkery);
    wait_for_tabs(tinkery, &["a.txt", "b.txt"], 1);
}

/// What the screen shows right of the tree on `row`, trailing blanks cut.
fn right_of_tree(tinkery: &Tinkery, row: u16) -> String {
    let line = tinkery.screen()[usize::from(row)].clone();
    let shown: String = line.chars().skip(usize::from(TREE)).collect();
    shown.trim_end().to_string()
}

/// Waits for the pill row to show `names` (see `pill_text`) with `active` the
/// one filled as the current tab.
fn wait_for_tabs(tinkery: &Tinkery, names: &[&str], active: usize) {
    let text = pill_text(names);
    tinkery.wait_for_screen(&format!("pills {text:?}"), WAIT, |screen| {
        let row: String = screen[usize::from(PILL_ROW)]
            .chars()
            .skip(usize::from(PILL_X))
            .collect();
        row.trim_end() == text
    });
    tinkery.wait_for_current_pill(PILL_ROW, PILL_X, names, active, WAIT);
}

/// Waits for the active tab's name, the only bold text on the pill row beside
/// the tree's `tinkery` brand, to be `name`.
fn wait_for_active(tinkery: &Tinkery, name: &str) {
    tinkery.wait_for_bold(PILL_ROW, &format!("tinkery{name}"), WAIT);
}

fn status_line(tinkery: &Tinkery) -> String {
    tinkery.screen()[usize::from(ROWS - 1)].clone()
}

fn read(dir: &Path, name: &str) -> String {
    fs::read_to_string(dir.join(name)).expect("read file")
}

#[test]
fn pills_show_each_buffer_with_the_active_one_and_dirty_marks() {
    let (_dir, mut tinkery) = open_project();
    // No pills while the splash stands in for the untitled tab.
    tinkery.wait_for_text("New file", WAIT);
    assert_eq!(tinkery.text_col(PILL_ROW, "untitled"), None);
    // Opening a file replaces the splash and its untitled tab; a second gets
    // its own.
    open_both(&mut tinkery);
    wait_for_active(&tinkery, "b.txt");

    // The visual check: both names, each a pill, the dirty one marked.
    tinkery.type_text("x");
    wait_for_tabs(&tinkery, &["a.txt", "b.txt •"], 1);
    wait_for_active(&tinkery, "b.txt");
    // From x = 30, each pill steps on by its label plus three: ` a.txt ` is 7.
    assert_eq!(tinkery.text_col(PILL_ROW, "a.txt"), Some(PILL_X + 2));
    assert_eq!(
        tinkery.text_col(PILL_ROW, &format!("{CAP_L} b.txt")),
        Some(PILL_X + 10)
    );
    // Rows 0 and 2 are blank right of the tree: no thread under the pills.
    for row in [0, UNDER_ROW] {
        assert_eq!(right_of_tree(&tinkery, row), "", "{:#?}", tinkery.screen());
    }

    // Opening an already-open file switches to its tab instead of opening another.
    tinkery.click(3, A_ROW);
    wait_for_active(&tinkery, "a.txt");
    tinkery.wait_for_text("1  alpha", WAIT);
    wait_for_tabs(&tinkery, &["a.txt", "b.txt •"], 0);
    tinkery.click(3, B_ROW);
    tinkery.wait_for_text("1  xbravo", WAIT);
    wait_for_tabs(&tinkery, &["a.txt", "b.txt •"], 1);
}

#[test]
fn tinkery_fills_the_current_pill_with_accent_and_draws_no_thread() {
    let (_dir, mut tinkery) = open_project_with("theme = \"tinkery\"\n");
    open_both(&mut tinkery);
    tinkery.type_text("x");
    wait_for_tabs(&tinkery, &["a.txt", "b.txt •"], 1);

    // Another tab: ends in `raised` on the ground, `text` on `raised`.
    let a = PILL_X;
    assert_eq!(tinkery.fg_at(a, PILL_ROW), RAISED);
    assert_eq!(tinkery.bg_at(a, PILL_ROW), BG);
    assert_eq!(tinkery.bg_at(a + 1, PILL_ROW), RAISED);
    assert_eq!(tinkery.fg_at(a + 2, PILL_ROW), TEXT);
    assert_eq!(tinkery.bg_at(a + 2, PILL_ROW), RAISED);
    assert!(!tinkery.bold_at(a + 2, PILL_ROW));
    assert_eq!(tinkery.fg_at(a + 8, PILL_ROW), RAISED);
    // The gap between two pills is the ground.
    assert_eq!(tinkery.bg_at(a + 9, PILL_ROW), BG);
    // The current tab: ends in `accent` on the ground, the name bold in
    // `acc_ink` on `accent`, the dirty dot in `acc_ink` too.
    let b = PILL_X + 10;
    assert_eq!(tinkery.fg_at(b, PILL_ROW), ACCENT);
    assert_eq!(tinkery.bg_at(b, PILL_ROW), BG);
    assert_eq!(tinkery.bg_at(b + 1, PILL_ROW), ACCENT);
    for x in b + 2..b + 7 {
        assert_eq!(tinkery.fg_at(x, PILL_ROW), ACC_INK, "column {x}");
        assert_eq!(tinkery.bg_at(x, PILL_ROW), ACCENT, "column {x}");
        assert!(tinkery.bold_at(x, PILL_ROW), "column {x}");
    }
    assert_eq!(tinkery.text_col(PILL_ROW, "•"), Some(b + 8));
    assert_eq!(tinkery.fg_at(b + 8, PILL_ROW), ACC_INK);
    assert_eq!(tinkery.bg_at(b + 8, PILL_ROW), ACCENT);
    // ` b.txt • ` is 9 wide, so the right end is 10 cells on.
    assert_eq!(tinkery.fg_at(b + 10, PILL_ROW), ACCENT);
    assert_eq!(tinkery.bg_at(b + 10, PILL_ROW), BG);
    assert_eq!(
        right_of_tree(&tinkery, PILL_ROW),
        format!("  {}", pill_text(&["a.txt", "b.txt •"]))
    );

    // The row under the pills is blank ground from the tree to the edge.
    assert_eq!(right_of_tree(&tinkery, UNDER_ROW), "");
    for x in TREE + 1..COLS {
        assert_eq!(tinkery.bg_at(x, UNDER_ROW), BG, "column {x}");
    }

    // Switching tab moves the accent, straight away, and the tab left behind
    // keeps its dot, now in `warn`.
    tinkery.send_keys("alt+,");
    wait_for_tabs(&tinkery, &["a.txt", "b.txt •"], 0);
    tinkery.wait_for_bg(a + 1, PILL_ROW, ACCENT, WAIT);
    tinkery.wait_for_fg_at(a + 2, PILL_ROW, ACC_INK, WAIT);
    tinkery.wait_for_bg(b + 1, PILL_ROW, RAISED, WAIT);
    tinkery.wait_for_fg_at(b + 2, PILL_ROW, TEXT, WAIT);
    tinkery.wait_for_fg_at(b + 8, PILL_ROW, WARN, WAIT);
    assert_eq!(tinkery.bg_at(b + 8, PILL_ROW), RAISED);
    assert_eq!(right_of_tree(&tinkery, UNDER_ROW), "");
}

#[test]
fn pill_caps_off_squares_the_pills_and_keeps_them_where_they_were() {
    let (_dir, mut tinkery) = open_project_with("theme = \"tinkery\"\n[ui]\npill_caps = false\n");
    open_files(&mut tinkery);
    let text = format!("  {}", square_pill_text(&["a.txt", "b.txt"]));
    tinkery.wait_for_screen("square pills", WAIT, |screen| {
        let row: String = screen[usize::from(PILL_ROW)]
            .chars()
            .skip(usize::from(TREE))
            .collect();
        row.trim_end() == text
    });
    tinkery.wait_for_current_pill(PILL_ROW, PILL_X, &["a.txt", "b.txt"], 1, WAIT);
    let screen = tinkery.screen().join("\n");
    assert!(
        !screen.contains(CAP_L) && !screen.contains(CAP_R),
        "{screen}"
    );
    // Each end is a blank of the pill's fill, in the cell the cap had.
    let (a, b) = (PILL_X, PILL_X + 10);
    for x in [a, a + 8] {
        assert_eq!(tinkery.bg_at(x, PILL_ROW), RAISED, "column {x}");
    }
    for x in [b, b + 8] {
        assert_eq!(tinkery.bg_at(x, PILL_ROW), ACCENT, "column {x}");
    }
    assert_eq!(tinkery.bg_at(a + 9, PILL_ROW), BG);
    assert_eq!(tinkery.text_col(PILL_ROW, "a.txt"), Some(a + 2));
    // A square end is still part of its pill: a click on it selects the tab.
    tinkery.click(a, PILL_ROW);
    tinkery.wait_for_bg(a, PILL_ROW, ACCENT, WAIT);
    tinkery.wait_for_text("1  alpha", WAIT);
}

#[test]
fn mono_reverses_the_current_pill_and_draws_no_cap_or_thread() {
    let (_dir, mut tinkery) = open_project_with("theme = \"mono\"\n");
    open_files(&mut tinkery);
    // The whole pill is reverse video, its two square ends included.
    tinkery.wait_for_reversed(PILL_ROW, "  b.txt  ", WAIT);
    // All of it is bold, beside the tree's brand.
    tinkery.wait_for_bold(PILL_ROW, "tinkery  b.txt  ", WAIT);
    assert_eq!(
        right_of_tree(&tinkery, PILL_ROW),
        format!("  {}", square_pill_text(&["a.txt", "b.txt"]))
    );
    // No Nerd Font glyph anywhere, and nothing under the pills.
    let screen = tinkery.screen().join("\n");
    assert!(
        !screen.contains(CAP_L) && !screen.contains(CAP_R),
        "{screen}"
    );
    assert_eq!(right_of_tree(&tinkery, UNDER_ROW), "");
    tinkery.send_keys("alt+,");
    tinkery.wait_for_reversed(PILL_ROW, "  a.txt  ", WAIT);

    // A dirty current pill keeps its dot inside the fill, in the pill's own
    // colour, not a `warn` block cut into it.
    tinkery.type_text("y");
    tinkery.wait_for_reversed(PILL_ROW, "  a.txt •  ", WAIT);
    let name = tinkery
        .text_col(PILL_ROW, "a.txt")
        .expect("a.txt on screen");
    let dot = tinkery
        .text_col(PILL_ROW, "•")
        .expect("dirty dot on screen");
    assert_eq!(tinkery.fg_at(dot, PILL_ROW), tinkery.fg_at(name, PILL_ROW));
    assert_eq!(tinkery.bg_at(dot, PILL_ROW), tinkery.bg_at(name, PILL_ROW));
}

#[test]
fn alt_keys_move_between_tabs_and_ctrl_w_closes_with_the_guard() {
    let (dir, mut tinkery) = open_project();
    open_both(&mut tinkery);
    tinkery.send_keys("ctrl+n");
    wait_for_tabs(&tinkery, &["a.txt", "b.txt", "untitled"], 2);
    wait_for_active(&tinkery, "untitled");

    tinkery.send_keys("alt+,");
    wait_for_active(&tinkery, "b.txt");
    tinkery.send_keys("alt+.");
    wait_for_active(&tinkery, "untitled");
    // Past the end wraps round to the first.
    tinkery.send_keys("alt+.");
    wait_for_active(&tinkery, "a.txt");
    tinkery.wait_for_text("1  alpha", WAIT);
    tinkery.send_keys("alt+3");
    wait_for_active(&tinkery, "untitled");
    tinkery.send_keys("alt+2");
    wait_for_active(&tinkery, "b.txt");

    // A dirty tab asks first; cancel keeps it.
    tinkery.type_text("y");
    wait_for_tabs(&tinkery, &["a.txt", "b.txt •", "untitled"], 1);
    tinkery.send_keys("ctrl+w");
    tinkery.wait_for_text(PROMPT, WAIT);
    tinkery.type_text("c");
    tinkery.wait_for_text_gone(PROMPT, WAIT);
    wait_for_tabs(&tinkery, &["a.txt", "b.txt •", "untitled"], 1);
    // Discard closes it without touching the file; the next tab takes over.
    tinkery.send_keys("ctrl+w");
    tinkery.wait_for_text(PROMPT, WAIT);
    tinkery.type_text("d");
    wait_for_tabs(&tinkery, &["a.txt", "untitled"], 1);
    wait_for_active(&tinkery, "untitled");
    assert_eq!(read(dir.path(), "b.txt"), "bravo\n");

    // Clean tabs close straight away.
    tinkery.send_keys("ctrl+w");
    wait_for_tabs(&tinkery, &["a.txt"], 0);
    // The last one leaves no tab, and the no file open key list.
    tinkery.send_keys("ctrl+w");
    tinkery.wait_for_text("no file open", WAIT);
    wait_for_tabs(&tinkery, &[], 0);
    tinkery.send_keys("ctrl+q");
    assert!(tinkery.wait_exit(WAIT).success());
}

#[test]
fn ctrl_n_then_alt_s_saves_relative_to_the_project_root() {
    let parent = tempfile::tempdir().expect("create temp dir");
    let root = parent.path().join("proj");
    fs::create_dir(&root).expect("create proj");
    fs::write(root.join("a.txt"), "alpha\n").expect("write a.txt");
    // cwd is the parent, so a path relative to the root is not one relative to cwd.
    let mut tinkery = Tinkery::spawn_in(parent.path(), &["proj"]);
    tinkery.wait_for_text("Open directory", START);
    // The splash hides the tree; Ctrl+B shows it, so the new file appears there.
    tinkery.send_keys("ctrl+b");
    tinkery.wait_for_text("a.txt", WAIT);

    // The first Ctrl+N leaves the splash for its untitled tab, the second opens
    // another.
    tinkery.send_keys("ctrl+n");
    wait_for_active(&tinkery, "untitled");
    tinkery.send_keys("ctrl+n");
    wait_for_tabs(&tinkery, &["untitled", "untitled"], 1);
    tinkery.type_text("hello");
    tinkery.wait_for_text("1  hello", WAIT);
    wait_for_tabs(&tinkery, &["untitled", "untitled •"], 1);

    // A name another file already has changes nothing.
    tinkery.send_keys("alt+s");
    tinkery.wait_for_text(" Save as  ", WAIT);
    tinkery.type_text("a.txt");
    tinkery.send_keys("enter");
    tinkery.wait_for_text("a.txt already exists", WAIT);
    assert_eq!(read(&root, "a.txt"), "alpha\n");

    tinkery.send_keys("alt+s");
    tinkery.wait_for_text(" Save as  ", WAIT);
    tinkery.type_text("new.txt");
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone(" Save as  ", WAIT);
    wait_for_tabs(&tinkery, &["untitled", "new.txt"], 1);
    assert_eq!(read(&root, "new.txt"), "hello");
    assert!(!parent.path().join("new.txt").exists());
    assert!(!status_line(&tinkery).contains('•'));
    // The tree shows the new file.
    tinkery.wait_for_text("  new.txt", WAIT);

    // Save as again offers the current path, relative to the root, and moves the
    // tab to the new name; the old file stays as it was.
    tinkery.type_text("!");
    tinkery.send_keys("alt+s");
    tinkery.wait_for_text(" Save as  new.txt", WAIT);
    let hint = harness::hints_text(&[("⏎", "save"), ("esc", "cancel")]);
    tinkery.wait_for_text(&hint, WAIT);
    for _ in 0.."new.txt".len() {
        tinkery.send_keys("backspace");
    }
    tinkery.type_text("renamed.txt");
    tinkery.send_keys("enter");
    wait_for_tabs(&tinkery, &["untitled", "renamed.txt"], 1);
    assert_eq!(read(&root, "renamed.txt"), "hello!");
    assert_eq!(read(&root, "new.txt"), "hello");
    tinkery.send_keys("ctrl+q");
    assert!(tinkery.wait_exit(WAIT).success());
}

#[test]
fn click_selects_a_pill_and_middle_click_closes_it() {
    let (_dir, mut tinkery) = open_project();
    open_both(&mut tinkery);
    // The blank rows above and below the pills are not tabs.
    let a = tinkery.text_col(PILL_ROW, "a.txt").expect("a.txt pill");
    tinkery.click(a, 0);
    tinkery.click(a, UNDER_ROW);
    tinkery.assert_running_for(Duration::from_millis(200));
    wait_for_active(&tinkery, "b.txt");
    // Any cell of a pill selects it, from end to end.
    tinkery.click(PILL_X, PILL_ROW);
    wait_for_active(&tinkery, "a.txt");
    tinkery.wait_for_text("1  alpha", WAIT);
    tinkery.click(PILL_X + 10, PILL_ROW);
    wait_for_active(&tinkery, "b.txt");
    tinkery.click(a, PILL_ROW);
    wait_for_active(&tinkery, "a.txt");
    // Clicking a tab gives the editor focus: typing edits a.txt.
    tinkery.type_text("z");
    tinkery.wait_for_text("1  zalpha", WAIT);
    wait_for_tabs(&tinkery, &["a.txt •", "b.txt"], 0);

    // Middle click on a clean tab closes it.
    let b = tinkery.text_col(PILL_ROW, "b.txt").expect("b.txt pill");
    tinkery.middle_click(b, PILL_ROW);
    wait_for_tabs(&tinkery, &["a.txt •"], 0);

    // On a dirty one it asks first.
    let a = tinkery.text_col(PILL_ROW, "a.txt").expect("a.txt pill");
    tinkery.middle_click(a, PILL_ROW);
    tinkery.wait_for_text(PROMPT, WAIT);
    tinkery.type_text("c");
    tinkery.wait_for_text_gone(PROMPT, WAIT);
    wait_for_tabs(&tinkery, &["a.txt •"], 0);
    tinkery.middle_click(a, PILL_ROW);
    tinkery.wait_for_text(PROMPT, WAIT);
    tinkery.type_text("d");
    tinkery.wait_for_text("no file open", WAIT);
    wait_for_tabs(&tinkery, &[], 0);
}

#[test]
fn ctrl_q_asks_about_each_dirty_tab_in_turn() {
    let (dir, mut tinkery) = open_project();
    open_both(&mut tinkery);
    tinkery.type_text("2");
    tinkery.click(3, A_ROW);
    tinkery.wait_for_text("1  alpha", WAIT);
    tinkery.type_text("1");
    // A clean untitled tab in between is never asked about.
    tinkery.send_keys("ctrl+n");
    wait_for_tabs(&tinkery, &["a.txt •", "b.txt •", "untitled"], 2);

    tinkery.send_keys("ctrl+q");
    tinkery.wait_for_text("a.txt has unsaved changes", WAIT);
    // The question is about a.txt, shown as the active tab.
    wait_for_tabs(&tinkery, &["a.txt •", "b.txt •", "untitled"], 0);
    tinkery.wait_for_text("1  1alpha", WAIT);
    tinkery.type_text("s");
    // Then b.txt.
    wait_for_tabs(&tinkery, &["a.txt", "b.txt •", "untitled"], 1);
    tinkery.wait_for_text("b.txt has unsaved changes", WAIT);
    tinkery.wait_for_text("1  2bravo", WAIT);
    assert_eq!(read(dir.path(), "a.txt"), "1alpha\n");

    // Cancel stops the quit with b.txt still unsaved.
    tinkery.type_text("c");
    tinkery.wait_for_text_gone(PROMPT, WAIT);
    tinkery.assert_running_for(Duration::from_millis(300));
    wait_for_tabs(&tinkery, &["a.txt", "b.txt •", "untitled"], 1);

    // Asked again, discard: nothing is left to ask about, so tinkery quits.
    tinkery.send_keys("ctrl+q");
    tinkery.wait_for_text("b.txt has unsaved changes", WAIT);
    wait_for_active(&tinkery, "b.txt");
    tinkery.type_text("d");
    assert!(tinkery.wait_exit(WAIT).success());
    assert_eq!(read(dir.path(), "b.txt"), "bravo\n");
}
