mod harness;

use std::fs;
use std::time::Duration;

use harness::{CAP_L, CAP_R, ROWS, Tinkery, pill_text};
use vt100::Color;

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(5);
const PROJECT: &str = "tests/fixtures/project";
/// The tree's width, its card's two edges included; the column after it is
/// editor ground, and the editor starts one further right.
const TREE: usize = 28;
/// The row of the first node, below the brand row.
const FIRST: u16 = 3;
/// The last node row: the tree's card ends on the row above the status line,
/// and that row is its bottom edge.
const LAST: u16 = ROWS - 3;
/// Where a top-level row's marker is, and the terminal cursor on the selected
/// row: after the card's edge, a pill's end and the selection's `›`.
const MARKER_X: u16 = 3;
/// The column of a pill's `›`, and of its two ends.
const MARK_X: u16 = 2;
const PILL_L: u16 = 1;
const PILL_R: u16 = 26;
/// Where the dirty `•` goes: the last cell before a pill's right end.
const DIRTY_X: u16 = 25;

// tinkery's roles, as the tinkery-forge spec's palette lists them.
const SURFACE: Color = Color::Rgb(0x17, 0x14, 0x13);
const BG: Color = Color::Rgb(0x11, 0x0f, 0x0e);
const ACCENT: Color = Color::Rgb(0xff, 0x8a, 0x3d);
const ACCENT2: Color = Color::Rgb(0xff, 0xd2, 0x7a);
const MUTED: Color = Color::Rgb(0x7d, 0x70, 0x67);
const TEXT: Color = Color::Rgb(0xb5, 0xa8, 0x9d);
const STRONG: Color = Color::Rgb(0xf8, 0xf0, 0xe6);
const GUIDE: Color = Color::Rgb(0x25, 0x1f, 0x1c);
const WARN: Color = Color::Rgb(0xf2, 0xc1, 0x4e);
const HOV: Color = Color::Rgb(0x3a, 0x30, 0x2a);
const LINE2: Color = Color::Rgb(0x40, 0x36, 0x2f);
/// `grad([accent, accent2], i/6)` for the seven letters of `tinkery`.
const BRAND: [Color; 7] = [
    Color::Rgb(0xff, 0x8a, 0x3d),
    Color::Rgb(0xff, 0x96, 0x47),
    Color::Rgb(0xff, 0xa2, 0x51),
    Color::Rgb(0xff, 0xae, 0x5c),
    Color::Rgb(0xff, 0xba, 0x66),
    Color::Rgb(0xff, 0xc6, 0x70),
    Color::Rgb(0xff, 0xd2, 0x7a),
];

/// What the tree pane shows on `line`: its 28 columns with the card's edge, a
/// lit row's pill ends and the selection's `›` blanked, trailing spaces cut. A
/// character's index is its column.
fn tree_text(line: &str) -> String {
    let chrome = format!("▕▏▁▔›{CAP_L}{CAP_R}");
    line.chars()
        .take(TREE)
        .map(|c| if chrome.contains(c) { ' ' } else { c })
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// The tree's node rows from the first one down to the first empty one.
fn tree_rows(screen: &[String]) -> Vec<String> {
    screen
        .iter()
        .skip(usize::from(FIRST))
        .map(|line| tree_text(line))
        .take_while(|text| !text.is_empty())
        .collect()
}

fn wait_for_tree(tinkery: &Tinkery, expected: &[&str]) {
    tinkery.wait_for_screen(&format!("tree rows {expected:?}"), WAIT, |screen| {
        tree_rows(screen) == expected
    });
}

fn cell(line: &str, col: u16) -> Option<char> {
    line.chars().nth(usize::from(col))
}

/// Waits for `row` to read `text` on a pill of `hov` from one edge of the
/// card's inside to the other, with the `›` when `marked` and without it
/// otherwise, then checks the pill's colours.
fn wait_for_pill(tinkery: &Tinkery, row: u16, text: &str, marked: bool) {
    let what = format!("tree row {row} {text:?} on a pill, marked: {marked}");
    tinkery.wait_for_screen(&what, WAIT, |screen| {
        let line = &screen[usize::from(row)];
        tree_text(line) == text
            && line
                .chars()
                .nth(usize::from(PILL_L))
                .map(String::from)
                .as_deref()
                == Some(CAP_L)
            && line
                .chars()
                .nth(usize::from(PILL_R))
                .map(String::from)
                .as_deref()
                == Some(CAP_R)
            && (cell(line, MARK_X) == Some('›')) == marked
    });
    // A pill's end is its fill on the card's ground; the cells between are
    // the fill.
    for end in [PILL_L, PILL_R] {
        assert_eq!(tinkery.fg_at(end, row), HOV, "({end}, {row})");
        assert_eq!(tinkery.bg_at(end, row), SURFACE, "({end}, {row})");
    }
    for col in MARK_X..PILL_R {
        assert_eq!(tinkery.bg_at(col, row), HOV, "({col}, {row})");
    }
    // The card's edge stays on the editor ground beside it.
    assert_eq!(tinkery.bg_at(0, row), BG);
    assert_eq!(tinkery.bg_at(27, row), BG);
    if marked {
        assert_eq!(tinkery.fg_at(MARK_X, row), ACCENT);
        assert!(tinkery.bold_at(MARK_X, row));
    }
}

/// Waits for `row` to read `text` as the focused selection: a pill with `›`.
fn wait_for_selected(tinkery: &Tinkery, row: u16, text: &str) {
    wait_for_pill(tinkery, row, text, true);
}

/// Waits for `row` to read `text` as the active file while the selection is
/// elsewhere or the editor has focus: a pill without `›`.
fn wait_for_active(tinkery: &Tinkery, row: u16, text: &str) {
    wait_for_pill(tinkery, row, text, false);
}

/// Asserts no cell of `row` is lit: the card's inside is all `surface`.
fn assert_unlit(tinkery: &Tinkery, row: u16) {
    for col in PILL_L..=PILL_R {
        assert_eq!(tinkery.bg_at(col, row), SURFACE, "({col}, {row})");
    }
    let line = tinkery.screen()[usize::from(row)].clone();
    assert_ne!(cell(&line, MARK_X), Some('›'), "{line:?}");
}

/// The fixture project on the default theme, `tinkery`, with the tree focused:
/// it opens on the splash with the tree hidden, and Ctrl+E shows it with the
/// keys. Rust files go to the scripted `fake_lsp`: the real rust-analyzer can
/// crash on CI at any moment and put its message in a toast over the editor.
fn open_project() -> Tinkery {
    let config = format!(
        "[lsp.rust]\ncommand = '{}'\n",
        env!("CARGO_BIN_EXE_fake_lsp")
    );
    let mut tinkery = Tinkery::spawn_with_config(&config, &[PROJECT]);
    tinkery.wait_for_text("Open directory", START);
    tinkery.send_keys("ctrl+e");
    tinkery
}

const TOP: &[&str] = &[
    "   ▸ docs",
    "   ▸ src",
    "     .gitignore",
    "     notes.txt",
    "     README.md",
];

const SRC_OPEN: &[&str] = &[
    "   ▸ docs",
    "   ▾ src",
    "    │▸ util",
    "    │  main.rs",
    "     .gitignore",
    "     notes.txt",
    "     README.md",
];

#[test]
fn folder_opens_with_a_28_column_tree_on_surface_under_the_brand() {
    let tinkery = open_project();
    wait_for_tree(&tinkery, TOP);
    // The first row starts selected: a pill with the `›`.
    wait_for_selected(&tinkery, FIRST, "   ▸ docs");
    assert_eq!(tinkery.fg_at(MARKER_X + 2, FIRST), STRONG);
    assert!(tinkery.bold_text(FIRST).ends_with("docs"));

    let screen = tinkery.screen();
    assert!(
        tree_text(&screen[1]).starts_with("   ✦ tinkery "),
        "{:?}",
        screen[1]
    );
    assert!(
        tree_text(&screen[1]).ends_with("project"),
        "{:?}",
        screen[1]
    );
    assert_eq!(tree_text(&screen[2]), "");
    // The brand keeps its ramp, and the project folder is `muted`.
    assert_eq!(tinkery.fg_at(MARKER_X, 1), ACCENT2);
    for (i, color) in (MARKER_X + 2..).zip(BRAND) {
        assert_eq!(tinkery.fg_at(i, 1), color, "brand letter at column {i}");
    }
    assert!(tinkery.bold_text(1).contains("tinkery"));
    assert_eq!(tinkery.fg_at(MARKER_X + 10, 1), MUTED);

    // The card: a flush edge in `line2` on the editor ground, from row 0 to
    // the row above the status line, around 26 columns of `surface`.
    let tree = |row: u16| {
        screen[usize::from(row)]
            .chars()
            .take(TREE)
            .collect::<String>()
    };
    // The corners are blank.
    assert_eq!(tree(0).trim_end(), format!(" {}", "▁".repeat(26)));
    assert_eq!(tree(ROWS - 2).trim_end(), format!(" {}", "▔".repeat(26)));
    for row in 1..ROWS - 2 {
        let line = tree(row);
        assert!(line.starts_with('▕') && line.ends_with('▏'), "{line:?}");
    }
    for row in [0, 1, 2, LAST, ROWS - 2] {
        for col in [0, 27] {
            assert_eq!(tinkery.bg_at(col, row), BG, "({col}, {row})");
            assert_eq!(tinkery.fg_at(col, row), LINE2, "({col}, {row})");
        }
    }
    for col in 1..27 {
        for row in [0, ROWS - 2] {
            assert_eq!(tinkery.bg_at(col, row), BG, "({col}, {row})");
            assert_eq!(tinkery.fg_at(col, row), LINE2, "({col}, {row})");
        }
        for row in [1, 2, LAST] {
            assert_eq!(tinkery.bg_at(col, row), SURFACE, "({col}, {row})");
        }
    }
    // No divider: the next column is editor ground.
    for (y, line) in screen.iter().enumerate().take(usize::from(ROWS - 1)) {
        assert_ne!(line.chars().nth(TREE), Some('│'), "row {y}: {line:?}");
    }
    assert_eq!(tinkery.bg_at(28, ROWS - 2), BG);
    // Names are `text`, markers `muted`; an unselected row isn't lit.
    assert_eq!(tinkery.fg_at(MARKER_X, FIRST + 1), MUTED);
    assert_eq!(tinkery.fg_at(MARKER_X + 2, FIRST + 1), TEXT);
    assert_unlit(&tinkery, FIRST + 1);
    // Right of the tree, nothing is open yet: the splash has the editor area.
    assert!(
        screen.iter().any(|line| line
            .chars()
            .skip(TREE + 1)
            .collect::<String>()
            .contains("New file")),
        "{screen:#?}"
    );
}

#[test]
fn nodes_have_guides_cut_names_and_a_dirty_mark() {
    let dir = tempfile::tempdir().expect("create temp dir");
    fs::create_dir(dir.path().join("lib")).expect("create lib");
    fs::write(
        dir.path()
            .join("lib")
            .join("a_rather_long_file_name_here.rs"),
        "fn long() {}\n",
    )
    .expect("write long file");
    let path = dir.path().to_str().expect("temp path is UTF-8");
    // An empty config: the default theme, whose colours this test reads.
    let mut tinkery = Tinkery::spawn_with_config("", &[path]);
    tinkery.wait_for_text("Open directory", START);
    tinkery.send_keys("ctrl+e");
    tinkery.send_keys("enter");
    wait_for_tree(&tinkery, &["   ▾ lib", "    │  a_rather_long_fil…"]);
    // The selected folder keeps its marker, a cell after the `›`; on a lit
    // row the marker is `text`.
    wait_for_selected(&tinkery, FIRST, "   ▾ lib");
    assert_eq!(tinkery.fg_at(MARKER_X, FIRST), TEXT);
    assert_eq!(tinkery.fg_at(MARKER_X + 1, FIRST + 1), GUIDE);

    tinkery.send_keys("down");
    tinkery.send_keys("enter");
    tinkery.wait_for_text("fn long", WAIT);
    // The open file's row is a pill with no `›`, its name bold in `strong`,
    // while the editor has focus.
    wait_for_active(&tinkery, FIRST + 1, "    │  a_rather_long_fil…");
    assert_eq!(tinkery.fg_at(MARKER_X + 4, FIRST + 1), STRONG);
    assert_eq!(tinkery.bold_text(FIRST + 1), "a_rather_long_fil…");
    assert_eq!(tinkery.fg_at(MARKER_X + 1, FIRST + 1), GUIDE);
    // The folder above isn't lit.
    assert_unlit(&tinkery, FIRST);
    assert_eq!(tinkery.fg_at(MARKER_X, FIRST), MUTED);

    tinkery.type_text("x");
    tinkery.wait_for_text("xfn long", WAIT);
    wait_for_tree(&tinkery, &["   ▾ lib", "    │  a_rather_long_fil…•"]);
    assert_eq!(tinkery.fg_at(DIRTY_X, FIRST + 1), WARN);
    assert_eq!(tinkery.bg_at(DIRTY_X, FIRST + 1), HOV);
}

#[test]
fn mono_reverses_the_selection_and_bolds_the_active_name() {
    let mut tinkery = Tinkery::spawn_with_config("theme = \"mono\"\n", &[PROJECT]);
    tinkery.wait_for_text("Open directory", START);
    tinkery.send_keys("ctrl+e");
    wait_for_tree(&tinkery, TOP);
    // Reverse video across the card's inside, with the `›`; no pill ends.
    tinkery.wait_for_reversed(FIRST, &format!("{:<26}", " ›▸ docs"), WAIT);
    let screen = tinkery.screen().join("\n");
    assert!(
        !screen.contains(CAP_L) && !screen.contains(CAP_R),
        "{screen}"
    );
    assert!(tinkery.bold_text(FIRST).ends_with("docs"));

    tinkery.send_keys("down");
    tinkery.send_keys("down");
    tinkery.send_keys("down");
    tinkery.wait_for_reversed(FIRST + 3, &format!("{:<26}", " ›  notes.txt"), WAIT);
    tinkery.send_keys("enter");
    tinkery.wait_for_text("notes for the tree test", WAIT);
    // The editor has focus: nothing in the tree is reversed or marked, and
    // the open file's name is bold.
    tinkery.wait_for_reversed(FIRST + 3, "", WAIT);
    assert_eq!(
        tree_text(&tinkery.screen()[usize::from(FIRST + 3)]),
        "     notes.txt"
    );
    assert!(!tinkery.screen()[usize::from(FIRST + 3)].contains('›'));
    assert!(
        tinkery.bold_text(FIRST + 3).starts_with("notes.txt"),
        "{:?}",
        tinkery.bold_text(FIRST + 3)
    );
    assert!(!tinkery.bold_text(FIRST + 4).starts_with("README.md"));
}

#[test]
fn gitignored_files_are_hidden() {
    // The fixture's `.gitignore` names `*.log` and `build/`, both present on disk.
    assert!(fs::metadata(format!("{PROJECT}/debug.log")).is_ok());
    assert!(fs::metadata(format!("{PROJECT}/build/out.txt")).is_ok());
    let tinkery = open_project();
    wait_for_tree(&tinkery, TOP);
    let contents = tinkery.screen().join("\n");
    assert!(!contents.contains("debug.log"), "{contents}");
    assert!(!contents.contains("build"), "{contents}");
}

#[test]
fn dot_git_and_nested_ignores_are_hidden() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let root = dir.path();
    fs::create_dir(root.join(".git")).expect("create .git");
    fs::write(root.join(".git").join("HEAD"), "ref: refs/heads/main\n").expect("write HEAD");
    fs::write(root.join(".gitignore"), "secret.txt\n").expect("write .gitignore");
    fs::write(root.join("secret.txt"), "").expect("write secret");
    fs::write(root.join("visible.txt"), "").expect("write visible");
    fs::create_dir(root.join("lib")).expect("create lib");
    fs::write(root.join("lib").join("secret.txt"), "").expect("write nested secret");
    fs::write(root.join("lib").join("mod.rs"), "").expect("write mod.rs");

    let path = root.to_str().expect("temp path is UTF-8");
    let mut tinkery = Tinkery::spawn(&[path]);
    tinkery.wait_for_text("Open directory", START);
    tinkery.send_keys("ctrl+e");
    wait_for_tree(
        &tinkery,
        &["   ▸ lib", "     .gitignore", "     visible.txt"],
    );
    tinkery.send_keys("enter");
    wait_for_tree(
        &tinkery,
        &[
            "   ▾ lib",
            "    │  mod.rs",
            "     .gitignore",
            "     visible.txt",
        ],
    );
}

#[test]
fn arrows_browse_and_enter_opens_a_file() {
    let mut tinkery = open_project();
    wait_for_selected(&tinkery, FIRST, "   ▸ docs");

    tinkery.send_keys("down");
    wait_for_selected(&tinkery, FIRST + 1, "   ▸ src");
    tinkery.send_keys("right");
    wait_for_tree(&tinkery, SRC_OPEN);
    // ← collapses, Enter expands again.
    tinkery.send_keys("left");
    wait_for_tree(&tinkery, TOP);
    tinkery.send_keys("enter");
    wait_for_tree(&tinkery, SRC_OPEN);
    tinkery.send_keys("down");
    wait_for_selected(&tinkery, FIRST + 2, "    │▸ util");
    tinkery.send_keys("down");
    wait_for_selected(&tinkery, FIRST + 3, "    │  main.rs");
    tinkery.send_keys("up");
    wait_for_selected(&tinkery, FIRST + 2, "    │▸ util");
    tinkery.send_keys("down");
    wait_for_selected(&tinkery, FIRST + 3, "    │  main.rs");

    tinkery.send_keys("enter");
    tinkery.wait_for_text("hello from main", WAIT);
    let status = tinkery.screen()[usize::from(ROWS - 1)].clone();
    assert!(status.contains("main.rs"), "{status:?}");
    // The editor has focus now: arrows move its cursor, not the tree selection.
    tinkery.send_keys("down");
    tinkery.wait_for_text("Ln 2, Col 1", WAIT);
    wait_for_active(&tinkery, FIRST + 3, "    │  main.rs");
}

#[test]
fn page_keys_move_the_selection_by_the_rows_the_tree_shows() {
    let dir = tempfile::tempdir().expect("create temp dir");
    for i in 0..60 {
        fs::write(dir.path().join(format!("f{i:02}.txt")), "").expect("write file");
    }
    let path = dir.path().to_str().expect("temp path is UTF-8");
    // An empty config: the default theme, whose colours this test reads.
    let mut tinkery = Tinkery::spawn_with_config("", &[path]);
    tinkery.wait_for_text("Open directory", START);
    tinkery.send_keys("ctrl+e");
    // Node rows run from FIRST to the row above the card's bottom edge.
    let last = LAST;
    let visible = usize::from(last - FIRST + 1);
    wait_for_selected(&tinkery, FIRST, "     f00.txt");
    tinkery.wait_for_text(&format!("f{:02}.txt", visible - 1), WAIT);

    // One page lands on the first row that wasn't showing, at the bottom: no
    // row is skipped unseen.
    tinkery.send_keys("pagedown");
    wait_for_selected(&tinkery, last, &format!("     f{visible:02}.txt"));
    assert_eq!(
        tree_text(&tinkery.screen()[usize::from(FIRST)]),
        "     f01.txt"
    );

    // The row under the last node is the card's edge, not a node.
    assert_eq!(tree_text(&tinkery.screen()[usize::from(last + 1)]), "");

    tinkery.send_keys("pageup");
    wait_for_selected(&tinkery, FIRST, "     f00.txt");
}

#[test]
fn clicking_selects_rows_and_opens_files() {
    let mut tinkery = open_project();
    wait_for_tree(&tinkery, TOP);

    // A file row: selected and opened. The editor takes the keys, so the row
    // is lit as the active file, without the `›`.
    tinkery.click(5, FIRST + 3);
    tinkery.wait_for_text("notes for the tree test", WAIT);
    wait_for_active(&tinkery, FIRST + 3, "     notes.txt");

    // A folder row: selected and expanded.
    tinkery.click(3, FIRST + 1);
    wait_for_tree(&tinkery, SRC_OPEN);
    wait_for_selected(&tinkery, FIRST + 1, "   ▾ src");

    tinkery.click(10, FIRST + 3);
    tinkery.wait_for_text("hello from main", WAIT);
    wait_for_active(&tinkery, FIRST + 3, "    │  main.rs");
    // The folder the selection was on is no longer lit.
    assert_unlit(&tinkery, FIRST + 1);

    // The card's bottom edge isn't a row: like the brand rows, clicking it
    // opens nothing and gives the tree the keys, so the selection shows.
    tinkery.click(5, ROWS - 2);
    wait_for_selected(&tinkery, FIRST + 3, "    │  main.rs");
    wait_for_tree(&tinkery, SRC_OPEN);

    // Clicking the brand rows above the nodes opens nothing.
    tinkery.click(5, 1);
    tinkery.assert_running_for(Duration::from_millis(200));
    wait_for_tree(&tinkery, SRC_OPEN);

    // Clicking the editor gives it focus back: typing edits the file.
    let x = tinkery
        .text_col(3, "fn main")
        .expect("main.rs is showing on line 1");
    tinkery.click(x, 3);
    tinkery.type_text("x");
    tinkery.wait_for_text("xfn main", WAIT);
}

#[test]
fn opening_another_file_keeps_unsaved_changes_in_their_tab() {
    let notes_path = format!("{PROJECT}/notes.txt");
    let before = fs::read(&notes_path).expect("read notes");
    let mut tinkery = open_project();
    wait_for_tree(&tinkery, TOP);
    tinkery.click(5, FIRST + 3);
    tinkery.wait_for_text("notes for the tree test", WAIT);
    tinkery.type_text("x");
    tinkery.wait_for_text("xnotes for the tree test", WAIT);

    // The other file opens in a tab of its own, without asking.
    tinkery.click(5, FIRST + 4);
    tinkery.wait_for_text("# Project fixture", WAIT);
    tinkery.wait_for_text(&pill_text(&["notes.txt •", "README.md"]), WAIT);
    assert!(
        !tinkery
            .screen()
            .join(
                "
"
            )
            .contains("has unsaved changes")
    );
    // Back on the first tab, the edit is still there and still unsaved.
    tinkery.click(5, FIRST + 3);
    tinkery.wait_for_text("xnotes for the tree test", WAIT);
    assert_eq!(fs::read(&notes_path).expect("read notes"), before);
}

#[test]
fn ctrl_b_toggles_the_tree_and_ctrl_e_switches_focus() {
    let mut tinkery = open_project();
    wait_for_tree(&tinkery, TOP);
    tinkery.click(5, FIRST + 4);
    tinkery.wait_for_text("# Project fixture", WAIT);
    let editor_x = tinkery
        .text_col(3, "# Project")
        .expect("README.md is showing on line 1");
    assert!(usize::from(editor_x) > TREE, "editor at column {editor_x}");

    // Ctrl+B hides the tree: the editor starts at column 0.
    tinkery.send_keys("ctrl+b");
    tinkery.wait_for_screen("the editor at column 0", WAIT, |screen| {
        screen[3].starts_with("   1  # Project fixture")
    });
    tinkery.send_keys("ctrl+b");
    wait_for_tree(&tinkery, TOP);

    // Ctrl+E moves focus to the tree: the cursor sits on the selected row's
    // marker and arrows move the selection.
    tinkery.send_keys("ctrl+e");
    tinkery.wait_for_cursor(MARKER_X, FIRST + 4, WAIT);
    tinkery.send_keys("up");
    wait_for_selected(&tinkery, FIRST + 3, "     notes.txt");
    // And back to the editor, where typing edits.
    tinkery.send_keys("ctrl+e");
    tinkery.type_text("y");
    tinkery.wait_for_text("y# Project fixture", WAIT);

    // With the tree hidden, Ctrl+E brings it back focused.
    tinkery.send_keys("ctrl+b");
    tinkery.wait_for_screen("the editor at column 0", WAIT, |screen| {
        screen[3].starts_with("   1  y# Project fixture")
    });
    tinkery.send_keys("ctrl+e");
    // README.md now has unsaved changes, so its row carries the dirty mark.
    let mut dirty = TOP.to_vec();
    dirty[4] = "     README.md           •";
    wait_for_tree(&tinkery, &dirty);
    tinkery.wait_for_cursor(MARKER_X, FIRST + 3, WAIT);
}

#[test]
fn tree_keys_are_remappable_by_name() {
    let toml = "[keys]\ntoggle_tree = \"alt+b\"\nfocus_tree = \"alt+e\"\n";
    let mut tinkery = Tinkery::spawn_with_config(toml, &[PROJECT]);
    tinkery.wait_for_text("Open directory", START);
    tinkery.send_keys("alt+b");
    wait_for_tree(&tinkery, TOP);
    tinkery.send_keys("alt+b");
    tinkery.wait_for_text_gone("README.md", WAIT);
    tinkery.send_keys("alt+e");
    wait_for_tree(&tinkery, TOP);
    tinkery.send_keys("ctrl+b");
    // Ctrl+B is no longer bound: the tree stays.
    tinkery.assert_running_for(Duration::from_millis(300));
    wait_for_tree(&tinkery, TOP);
}
