mod harness;

use std::fs;
use std::time::Duration;

use harness::{CAP_L, CAP_R, ROWS, Tinkery, pill_text};
use tempfile::TempDir;
use vt100::Color;

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(5);
/// With the tree hidden the two splits share 100 columns: 0..50, the `│` in
/// column 50, then 51..100.
const DIVIDER: usize = 50;
/// With the tree showing they share the 71 columns right of it and the blank
/// column after it: 29..64, the `│` in column 64, then 65..100.
const TREE_DIVIDER: usize = 64;
const LEFT_X: u16 = 29;
const LEFT: usize = LEFT_X as usize;
/// The tree rows of `a.txt` and `b.txt`, below its brand row.
const A_ROW: u16 = 3;
const B_ROW: u16 = 4;
/// Where the terminal cursor sits on the tree's selected row: its marker, after
/// the card's edge, a pill's end and the `›`.
const TREE_MARKER_X: u16 = 3;
/// Each split's header: a blank row, the pills, a blank row; then its editor.
const PILL_ROW: u16 = 1;
const UNDER_ROW: u16 = 2;
const TOP: usize = 3;

// The `tinkery` roles the splits use (tinkery-forge spec, Palette).
const BG: Color = Color::Rgb(0x11, 0x0f, 0x0e);
const GUIDE: Color = Color::Rgb(0x25, 0x1f, 0x1c);
const BTN: Color = Color::Rgb(0x33, 0x2a, 0x25);
const STRONG: Color = Color::Rgb(0xf8, 0xf0, 0xe6);
const ACCENT: Color = Color::Rgb(0xff, 0x8a, 0x3d);
const ACC_INK: Color = Color::Rgb(0x1f, 0x0d, 0x02);
/// `mono`'s `btn`: the terminal's own dark grey.
const DARK_GREY: Color = Color::Idx(8);

/// A temp folder holding `a.txt` ("alpha") and `b.txt` ("bravo").
fn project() -> TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");
    fs::write(dir.path().join("a.txt"), "alpha\n").expect("write a.txt");
    fs::write(dir.path().join("b.txt"), "bravo\n").expect("write b.txt");
    dir
}

/// `a.txt` opened on its own, so the tree is hidden.
fn open_file(dir: &TempDir) -> Tinkery {
    let tinkery = Tinkery::spawn_in(dir.path(), &["a.txt"]);
    tinkery.wait_for_text("1  alpha", START);
    tinkery
}

/// The folder opened with the tree showing and a.txt open in the left split.
fn open_folder(dir: &TempDir) -> Tinkery {
    let mut tinkery = Tinkery::spawn_in(dir.path(), &["."]);
    tinkery.wait_for_text("Open directory", START);
    // The splash hides the tree; Ctrl+B shows it.
    tinkery.send_keys("ctrl+b");
    tinkery.wait_for_text("b.txt", WAIT);
    tinkery.click(3, A_ROW);
    tinkery.wait_for_text("1  alpha", WAIT);
    tinkery
}

/// The columns `from..to` of `line`, trailing spaces cut.
fn cols(line: &str, from: usize, to: usize) -> String {
    line.chars()
        .skip(from)
        .take(to.saturating_sub(from))
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// What the left and right splits show on `row`, either side of `divider`; the
/// left split starts at `left`.
fn halves(screen: &[String], row: usize, left: usize, divider: usize) -> (String, String) {
    (
        cols(&screen[row], left, divider),
        cols(&screen[row], divider + 1, usize::MAX),
    )
}

/// Whether `divider` holds a `│` on every row above the status line.
fn divided(screen: &[String], divider: usize) -> bool {
    screen[..usize::from(ROWS - 1)]
        .iter()
        .all(|line| line.chars().nth(divider) == Some('│'))
}

/// Waits for the two pill rows to read `left` and `right`, each a split's tab
/// names (see `pill_text`) with its active one filled as the current tab.
fn wait_for_bars(
    tinkery: &Tinkery,
    left_x: usize,
    divider: usize,
    left: (&[&str], usize),
    right: (&[&str], usize),
) {
    // Each split's first pill starts one column in.
    let (left_text, right_text) = (
        format!(" {}", pill_text(left.0)),
        format!(" {}", pill_text(right.0)),
    );
    tinkery.wait_for_screen(
        &format!("pills {left_text:?} | {right_text:?}"),
        WAIT,
        |screen| {
            halves(screen, usize::from(PILL_ROW), left_x, divider)
                == (left_text.clone(), right_text.clone())
        },
    );
    let col = |x: usize| u16::try_from(x).expect("a screen column");
    tinkery.wait_for_current_pill(PILL_ROW, col(left_x + 1), left.0, left.1, WAIT);
    tinkery.wait_for_current_pill(PILL_ROW, col(divider + 2), right.0, right.1, WAIT);
}

/// Waits for the focused split's active tab, the only bold name on the pill
/// row, to be `name`. With the tree showing, its `tinkery` brand is bold too.
fn wait_for_focused(tinkery: &Tinkery, tree: bool, name: &str) {
    let brand = if tree { "tinkery" } else { "" };
    tinkery.wait_for_bold(PILL_ROW, &format!("{brand}{name}"), WAIT);
}

#[test]
fn alt_v_shows_two_editors_side_by_side_and_alt_v_again_closes_the_right_one() {
    let dir = project();
    let mut tinkery = open_file(&dir);
    tinkery.send_keys("alt+v");

    // The visual check: two editors of about equal width split by a `│`, each with
    // its own pills, and only the focused (new, right) split's name lit.
    let bar = format!(" {}", pill_text(&["a.txt"]));
    tinkery.wait_for_screen("two splits on a.txt", WAIT, |screen| {
        divided(screen, DIVIDER)
            && halves(screen, usize::from(PILL_ROW), 0, DIVIDER) == (bar.clone(), bar.clone())
            && halves(screen, TOP, 0, DIVIDER) == ("   1  alpha".into(), "   1  alpha".into())
    });
    wait_for_focused(&tinkery, false, "a.txt");
    assert_eq!(tinkery.text_col(PILL_ROW, "a.txt"), Some(3));
    let cursor = tinkery.cursor();
    assert!(
        usize::from(cursor.0) > DIVIDER,
        "cursor {cursor:?} should be in the right split\n{:#?}",
        tinkery.screen()
    );

    // Each split has its own tabs and active tab: a new file opens on the right.
    tinkery.send_keys("ctrl+n");
    wait_for_bars(
        &tinkery,
        0,
        DIVIDER,
        (&["a.txt"], 0),
        (&["a.txt", "untitled"], 1),
    );
    wait_for_focused(&tinkery, false, "untitled");
    tinkery.wait_for_screen("right split empty", WAIT, |screen| {
        halves(screen, TOP, 0, DIVIDER) == ("   1  alpha".into(), "   1".into())
    });

    // Alt+V again: one editor, keeping the tab only the right split had.
    tinkery.send_keys("alt+v");
    tinkery.wait_for_screen("one editor", WAIT, |screen| {
        cols(&screen[usize::from(PILL_ROW)], 0, 100)
            == format!(" {}", pill_text(&["a.txt", "untitled"]))
            && screen[TOP].matches("alpha").count() == 1
            && !divided(screen, DIVIDER)
    });
    tinkery.wait_for_current_pill(PILL_ROW, 1, &["a.txt", "untitled"], 0, WAIT);
    wait_for_focused(&tinkery, false, "a.txt");
    tinkery.send_keys("ctrl+q");
    assert!(tinkery.wait_exit(WAIT).success());
}

#[test]
fn tinkery_splits_have_a_guide_rule_and_the_unfocused_ones_tab_is_on_btn() {
    let dir = project();
    let mut tinkery =
        Tinkery::spawn_in_with_config(dir.path(), "theme = \"tinkery\"\n", &["a.txt"]);
    tinkery.wait_for_text("1  alpha", START);
    tinkery.send_keys("alt+v");
    wait_for_focused(&tinkery, false, "a.txt");
    tinkery.wait_for_screen("two splits", WAIT, |screen| divided(screen, DIVIDER));

    // One `│` in `guide` between the editors, the only vertical rule.
    let divider = u16::try_from(DIVIDER).expect("fits");
    for row in [0, PILL_ROW, UNDER_ROW, 3, ROWS - 2] {
        assert_eq!(tinkery.fg_at(divider, row), GUIDE, "row {row}");
    }

    // The left split lost focus: its current tab is `strong` on `btn`, not
    // bold. The right one has focus: `acc_ink` bold on `accent`.
    assert_eq!(tinkery.fg_at(1, PILL_ROW), BTN);
    assert_eq!(tinkery.bg_at(1, PILL_ROW), BG);
    assert_eq!(tinkery.bg_at(2, PILL_ROW), BTN);
    assert_eq!(tinkery.fg_at(3, PILL_ROW), STRONG);
    assert_eq!(tinkery.bg_at(3, PILL_ROW), BTN);
    assert!(!tinkery.bold_at(3, PILL_ROW));
    assert_eq!(tinkery.fg_at(9, PILL_ROW), BTN);
    assert_eq!(tinkery.fg_at(52, PILL_ROW), ACCENT);
    assert_eq!(tinkery.bg_at(52, PILL_ROW), BG);
    assert_eq!(tinkery.bg_at(53, PILL_ROW), ACCENT);
    assert_eq!(tinkery.fg_at(54, PILL_ROW), ACC_INK);
    assert!(tinkery.bold_at(54, PILL_ROW));
    assert_eq!(tinkery.fg_at(60, PILL_ROW), ACCENT);

    // The row under the pills is blank ground in both splits: no thread.
    let under = tinkery.screen()[usize::from(UNDER_ROW)].clone();
    assert_eq!(under.replace('│', "").trim(), "", "{under:?}");
    for x in (0..100).filter(|x| *x != divider) {
        assert_eq!(tinkery.bg_at(x, UNDER_ROW), BG, "column {x}");
    }

    // F6 swaps which split has the accent.
    tinkery.send_keys("f6");
    tinkery.wait_for_bg(2, PILL_ROW, ACCENT, WAIT);
    tinkery.wait_for_fg_at(3, PILL_ROW, ACC_INK, WAIT);
    tinkery.wait_for_bg(53, PILL_ROW, BTN, WAIT);
    tinkery.wait_for_fg_at(54, PILL_ROW, STRONG, WAIT);
    wait_for_focused(&tinkery, false, "a.txt");
    assert!(tinkery.bold_at(3, PILL_ROW));
    assert!(!tinkery.bold_at(54, PILL_ROW));
}

#[test]
fn mono_underlines_the_unfocused_splits_current_tab() {
    let dir = project();
    let mut tinkery = Tinkery::spawn_in_with_config(dir.path(), "theme = \"mono\"\n", &["a.txt"]);
    tinkery.wait_for_text("1  alpha", START);
    tinkery.send_keys("alt+v");
    tinkery.wait_for_screen("two splits", WAIT, |screen| divided(screen, DIVIDER));

    // The right split has focus: its pill is reverse video and bold, ends and
    // all. The left one's name is underlined, not bold, on the terminal's dark
    // grey, and nothing else on the row is underlined.
    tinkery.wait_for_reversed(PILL_ROW, "  a.txt  ", WAIT);
    tinkery.wait_for_bold(PILL_ROW, "  a.txt  ", WAIT);
    tinkery.wait_for_underlined(PILL_ROW, "a.txt", WAIT);
    assert_eq!(tinkery.bg_at(3, PILL_ROW), DARK_GREY);
    assert!(!tinkery.bold_at(3, PILL_ROW));
    assert!(tinkery.bold_at(54, PILL_ROW));
    // No Nerd Font glyph, and nothing under the pills.
    let screen = tinkery.screen();
    let all = screen.join("\n");
    assert!(!all.contains(CAP_L) && !all.contains(CAP_R), "{all}");
    assert_eq!(
        halves(&screen, usize::from(PILL_ROW), 0, DIVIDER),
        ("   a.txt".into(), "   a.txt".into())
    );
    assert_eq!(screen[usize::from(UNDER_ROW)].replace('│', "").trim(), "");

    // F6 swaps them: the underline follows the split that lost focus.
    tinkery.send_keys("f6");
    tinkery.wait_for_bg(54, PILL_ROW, DARK_GREY, WAIT);
    tinkery.wait_for_reversed(PILL_ROW, "  a.txt  ", WAIT);
    tinkery.wait_for_underlined(PILL_ROW, "a.txt", WAIT);
    assert!(tinkery.bold_at(3, PILL_ROW));
    assert!(!tinkery.bold_at(54, PILL_ROW));
}

#[test]
fn f6_cycles_tree_left_right_and_skips_the_hidden_tree() {
    let dir = project();
    let mut tinkery = open_folder(&dir);
    tinkery.send_keys("alt+v");
    // b.txt opens in the focused right split, which then shows it.
    tinkery.click(3, B_ROW);
    wait_for_bars(
        &tinkery,
        LEFT,
        TREE_DIVIDER,
        (&["a.txt"], 0),
        (&["a.txt", "b.txt"], 1),
    );
    tinkery.wait_for_text("1  bravo", WAIT);

    // From the right split F6 wraps to the tree, where typing edits nothing...
    tinkery.send_keys("f6");
    tinkery.wait_for_cursor(TREE_MARKER_X, B_ROW, WAIT);
    tinkery.type_text("0");
    // ...then the left split, then the right one.
    tinkery.send_keys("f6");
    tinkery.type_text("1");
    tinkery.wait_for_text("1  1alpha", WAIT);
    tinkery.send_keys("f6");
    tinkery.type_text("2");
    tinkery.wait_for_text("1  2bravo", WAIT);
    tinkery.send_keys("f6");
    tinkery.wait_for_cursor(TREE_MARKER_X, B_ROW, WAIT);

    // With the tree hidden F6 goes left, right, left.
    tinkery.send_keys("ctrl+b");
    tinkery.send_keys("f6");
    tinkery.type_text("3");
    tinkery.wait_for_screen("typed on the left", WAIT, |screen| {
        halves(screen, TOP, 0, DIVIDER) == ("   1  13alpha".into(), "   1  2bravo".into())
    });
    tinkery.send_keys("f6");
    tinkery.type_text("4");
    tinkery.wait_for_text("1  24bravo", WAIT);
    tinkery.send_keys("f6");
    tinkery.type_text("5");
    tinkery.wait_for_text("1  135alpha", WAIT);
}

#[test]
fn a_click_focuses_its_split_and_tab_and_open_actions_go_there() {
    let dir = project();
    let mut tinkery = open_folder(&dir);
    tinkery.send_keys("alt+v");
    wait_for_bars(
        &tinkery,
        LEFT,
        TREE_DIVIDER,
        (&["a.txt"], 0),
        (&["a.txt"], 0),
    );

    // A click in the left editor focuses it: a new tab opens there.
    tinkery.click(LEFT_X + 10, 5);
    tinkery.send_keys("ctrl+n");
    wait_for_bars(
        &tinkery,
        LEFT,
        TREE_DIVIDER,
        (&["a.txt", "untitled"], 1),
        (&["a.txt"], 0),
    );
    wait_for_focused(&tinkery, true, "untitled");

    // A click in the right editor focuses that one: the tree opens b.txt there.
    tinkery.click(80, 5);
    wait_for_focused(&tinkery, true, "a.txt");
    tinkery.click(3, B_ROW);
    wait_for_bars(
        &tinkery,
        LEFT,
        TREE_DIVIDER,
        (&["a.txt", "untitled"], 1),
        (&["a.txt", "b.txt"], 1),
    );
    wait_for_focused(&tinkery, true, "b.txt");

    // Tab keys move along the focused split's tabs only.
    tinkery.send_keys("alt+,");
    wait_for_focused(&tinkery, true, "a.txt");
    tinkery.wait_for_screen("right split back on a.txt", WAIT, |screen| {
        halves(screen, TOP, LEFT, TREE_DIVIDER) == ("   1".into(), "   1  alpha".into())
    });
    // Ctrl+W closes the right split's tab; the left split keeps its own a.txt tab.
    tinkery.send_keys("ctrl+w");
    wait_for_bars(
        &tinkery,
        LEFT,
        TREE_DIVIDER,
        (&["a.txt", "untitled"], 1),
        (&["b.txt"], 0),
    );

    // A click on the left split's pills focuses the left split too.
    let a = tinkery
        .text_col(PILL_ROW, "a.txt")
        .expect("left a.txt pill");
    tinkery.click(a, PILL_ROW);
    wait_for_focused(&tinkery, true, "a.txt");
    // Typing lands at the left split's own cursor, where the click below the text
    // left it on the last line.
    tinkery.type_text("z");
    tinkery.wait_for_text("2  z", WAIT);
    wait_for_bars(
        &tinkery,
        LEFT,
        TREE_DIVIDER,
        (&["a.txt •", "untitled"], 0),
        (&["b.txt"], 0),
    );
}

#[test]
fn edits_to_a_buffer_open_in_both_splits_show_in_both() {
    let dir = project();
    let mut tinkery = open_file(&dir);
    tinkery.send_keys("alt+v");
    wait_for_focused(&tinkery, false, "a.txt");

    // Typed on the right, shown on both sides, both tabs marked dirty.
    tinkery.send_keys("end");
    tinkery.type_text("!");
    let dirty = format!(" {}", pill_text(&["a.txt •"]));
    tinkery.wait_for_screen("edit in both", WAIT, |screen| {
        halves(screen, TOP, 0, DIVIDER) == ("   1  alpha!".into(), "   1  alpha!".into())
            && halves(screen, usize::from(PILL_ROW), 0, DIVIDER) == (dirty.clone(), dirty.clone())
    });

    // Typed on the left, at the left split's own cursor (still at the start).
    tinkery.click(10, 3);
    tinkery.send_keys("home");
    tinkery.type_text("<");
    tinkery.wait_for_screen("second edit in both", WAIT, |screen| {
        halves(screen, TOP, 0, DIVIDER) == ("   1  <alpha!".into(), "   1  <alpha!".into())
    });

    // Saving from either split saves the one buffer.
    tinkery.send_keys("ctrl+s");
    wait_for_bars(&tinkery, 0, DIVIDER, (&["a.txt"], 0), (&["a.txt"], 0));
    assert_eq!(
        fs::read_to_string(dir.path().join("a.txt")).expect("read a.txt"),
        "<alpha!\n"
    );
    tinkery.send_keys("ctrl+q");
    assert!(tinkery.wait_exit(WAIT).success());
}
