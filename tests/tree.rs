mod harness;

use std::fs;
use std::time::Duration;

use harness::{ROWS, Tinkery, pill_text};
use vt100::Color;

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(5);
const PROJECT: &str = "tests/fixtures/project";
/// The tree's width; the column after it is editor ground, and the editor
/// starts one further right.
const TREE: usize = 28;
/// The row of the first node, below the brand row.
const FIRST: u16 = 3;

// hydra's roles, as README §4.1 lists them.
const SURFACE: Color = Color::Rgb(0x0c, 0x13, 0x1b);
const BG: Color = Color::Rgb(0x07, 0x0b, 0x10);
const ACCENT2: Color = Color::Rgb(0x5a, 0xa9, 0xff);
const MUTED: Color = Color::Rgb(0x71, 0x80, 0x8f);
const TEXT: Color = Color::Rgb(0xa7, 0xb4, 0xc2);
const STRONG: Color = Color::Rgb(0xf2, 0xf6, 0xf8);
const GUIDE: Color = Color::Rgb(0x17, 0x22, 0x2e);
const WARN: Color = Color::Rgb(0xff, 0xb5, 0x47);
/// `grad([accent, accent2], i/6)` for the seven letters of `tinkery`.
const BRAND: [Color; 7] = [
    Color::Rgb(0xc3, 0xf5, 0x3c),
    Color::Rgb(0xb2, 0xe8, 0x5d),
    Color::Rgb(0xa0, 0xdc, 0x7d),
    Color::Rgb(0x8f, 0xcf, 0x9e),
    Color::Rgb(0x7d, 0xc2, 0xbe),
    Color::Rgb(0x6c, 0xb6, 0xdf),
    Color::Rgb(0x5a, 0xa9, 0xff),
];
/// The glow row's bg at column 0, `mix(surface, accent, .26)`, and at column 27,
/// `grad([.., mix(surface, accent, .08), surface], 27/28)`.
const GLOW_START: Color = Color::Rgb(0x3c, 0x4e, 0x24);
const GLOW_END: Color = Color::Rgb(0x0d, 0x14, 0x1b);

/// What the tree pane shows on `line`: its first 28 columns, trailing spaces cut.
fn tree_text(line: &str) -> String {
    line.chars()
        .take(TREE)
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

/// Waits for `row` to read `text` and glow across the whole pane, as the
/// focused selection and the active file do.
fn wait_for_selected(tinkery: &Tinkery, row: u16, text: &str) {
    tinkery.wait_for_screen(&format!("tree row {row} {text:?}"), WAIT, |screen| {
        tree_text(&screen[usize::from(row)]) == text
    });
    tinkery.wait_for_bg(0, row, GLOW_START, WAIT);
    tinkery.wait_for_bg(27, row, GLOW_END, WAIT);
}

/// The fixture project with the tree focused: it opens on the splash with the
/// tree hidden, and Ctrl+E shows it with the keys.
/// Rust files go to the scripted `fake_lsp`: the real rust-analyzer can crash
/// on CI at any moment and put its message over the path in the status line.
fn open_project() -> Tinkery {
    let config = format!(
        "theme = \"hydra\"\n[lsp.rust]\ncommand = '{}'\n",
        env!("CARGO_BIN_EXE_fake_lsp")
    );
    let mut tinkery = Tinkery::spawn_with_config(&config, &[PROJECT]);
    tinkery.wait_for_text("Open directory", START);
    tinkery.send_keys("ctrl+e");
    tinkery
}

const TOP: &[&str] = &[
    "  ▸ docs",
    "  ▸ src",
    "    .gitignore",
    "    notes.txt",
    "    README.md",
];

const SRC_OPEN: &[&str] = &[
    "  ▸ docs",
    "  ▾ src",
    "   │▸ util",
    "   │  main.rs",
    "    .gitignore",
    "    notes.txt",
    "    README.md",
];

#[test]
fn folder_opens_with_a_28_column_tree_on_surface_under_the_brand() {
    let tinkery = open_project();
    wait_for_tree(&tinkery, TOP);
    // The first row starts selected, glowing across the pane.
    wait_for_selected(&tinkery, FIRST, "  ▸ docs");

    let screen = tinkery.screen();
    assert!(
        tree_text(&screen[1]).starts_with("  ✦ tinkery "),
        "{:?}",
        screen[1]
    );
    assert!(
        tree_text(&screen[1]).ends_with("project"),
        "{:?}",
        screen[1]
    );
    assert_eq!(tree_text(&screen[0]), "");
    assert_eq!(tree_text(&screen[2]), "");
    assert_eq!(tinkery.fg_at(2, 1), ACCENT2);
    for (i, color) in (4..).zip(BRAND) {
        assert_eq!(tinkery.fg_at(i, 1), color, "brand letter at column {i}");
    }
    assert!(tinkery.bold_text(1).contains("tinkery"));
    assert_eq!(tinkery.fg_at(12, 1), MUTED);

    // Surface over every row down to the one above the status line, and no
    // divider: the next column is editor ground.
    for row in [0, 1, 2, ROWS - 2] {
        for col in 0..28 {
            assert_eq!(tinkery.bg_at(col, row), SURFACE, "({col}, {row})");
        }
    }
    for (y, line) in screen.iter().enumerate().take(usize::from(ROWS - 1)) {
        assert_ne!(line.chars().nth(TREE), Some('│'), "row {y}: {line:?}");
    }
    assert_eq!(tinkery.bg_at(28, ROWS - 2), BG);
    // Names are `text`, markers `muted`.
    assert_eq!(tinkery.fg_at(2, FIRST + 1), MUTED);
    assert_eq!(tinkery.fg_at(4, FIRST + 1), TEXT);
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
    let mut tinkery = Tinkery::spawn(&[path]);
    tinkery.wait_for_text("Open directory", START);
    tinkery.send_keys("ctrl+e");
    tinkery.send_keys("enter");
    wait_for_tree(&tinkery, &["  ▾ lib", "   │  a_rather_long_file…"]);
    assert_eq!(tinkery.fg_at(2, FIRST), MUTED);
    assert_eq!(tinkery.fg_at(3, FIRST + 1), GUIDE);

    tinkery.send_keys("down");
    tinkery.send_keys("enter");
    tinkery.wait_for_text("fn long", WAIT);
    // The open file's row glows, its name bold in `strong`, while the editor
    // has focus.
    wait_for_selected(&tinkery, FIRST + 1, "   │  a_rather_long_file…");
    assert_eq!(tinkery.fg_at(6, FIRST + 1), STRONG);
    assert!(
        tinkery
            .bold_text(FIRST + 1)
            .starts_with("a_rather_long_file…")
    );
    // The folder above isn't lit.
    assert_eq!(tinkery.bg_at(0, FIRST), SURFACE);

    tinkery.type_text("x");
    tinkery.wait_for_text("xfn long", WAIT);
    wait_for_tree(&tinkery, &["  ▾ lib", "   │  a_rather_long_file…•"]);
    assert_eq!(tinkery.fg_at(25, FIRST + 1), WARN);
}

#[test]
fn mono_reverses_the_selection_and_bolds_the_active_name() {
    let mut tinkery = Tinkery::spawn_with_config("theme = \"mono\"\n", &[PROJECT]);
    tinkery.wait_for_text("Open directory", START);
    tinkery.send_keys("ctrl+e");
    wait_for_tree(&tinkery, TOP);
    tinkery.wait_for_reversed(FIRST, &format!("{:<TREE$}", "  ▸ docs"), WAIT);

    tinkery.send_keys("down");
    tinkery.send_keys("down");
    tinkery.send_keys("down");
    tinkery.wait_for_reversed(FIRST + 3, &format!("{:<TREE$}", "    notes.txt"), WAIT);
    tinkery.send_keys("enter");
    tinkery.wait_for_text("notes for the tree test", WAIT);
    // The editor has focus: nothing in the tree is reversed, and the open file's
    // name is bold.
    tinkery.wait_for_reversed(FIRST + 3, "", WAIT);
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
    wait_for_tree(&tinkery, &["  ▸ lib", "    .gitignore", "    visible.txt"]);
    tinkery.send_keys("enter");
    wait_for_tree(
        &tinkery,
        &[
            "  ▾ lib",
            "   │  mod.rs",
            "    .gitignore",
            "    visible.txt",
        ],
    );
}

#[test]
fn arrows_browse_and_enter_opens_a_file() {
    let mut tinkery = open_project();
    wait_for_selected(&tinkery, FIRST, "  ▸ docs");

    tinkery.send_keys("down");
    wait_for_selected(&tinkery, FIRST + 1, "  ▸ src");
    tinkery.send_keys("right");
    wait_for_tree(&tinkery, SRC_OPEN);
    // ← collapses, Enter expands again.
    tinkery.send_keys("left");
    wait_for_tree(&tinkery, TOP);
    tinkery.send_keys("enter");
    wait_for_tree(&tinkery, SRC_OPEN);
    tinkery.send_keys("down");
    wait_for_selected(&tinkery, FIRST + 2, "   │▸ util");
    tinkery.send_keys("down");
    wait_for_selected(&tinkery, FIRST + 3, "   │  main.rs");
    tinkery.send_keys("up");
    wait_for_selected(&tinkery, FIRST + 2, "   │▸ util");
    tinkery.send_keys("down");
    wait_for_selected(&tinkery, FIRST + 3, "   │  main.rs");

    tinkery.send_keys("enter");
    tinkery.wait_for_text("hello from main", WAIT);
    let status = tinkery.screen()[usize::from(ROWS - 1)].clone();
    assert!(status.contains("main.rs"), "{status:?}");
    // The editor has focus now: arrows move its cursor, not the tree selection.
    tinkery.send_keys("down");
    tinkery.wait_for_text("Ln 2, Col 1", WAIT);
    wait_for_selected(&tinkery, FIRST + 3, "   │  main.rs");
}

#[test]
fn page_keys_move_the_selection_by_the_rows_the_tree_shows() {
    let dir = tempfile::tempdir().expect("create temp dir");
    for i in 0..60 {
        fs::write(dir.path().join(format!("f{i:02}.txt")), "").expect("write file");
    }
    let path = dir.path().to_str().expect("temp path is UTF-8");
    let mut tinkery = Tinkery::spawn(&[path]);
    tinkery.wait_for_text("Open directory", START);
    tinkery.send_keys("ctrl+e");
    // Node rows run from FIRST to the row above the status line.
    let last = ROWS - 2;
    let visible = usize::from(last - FIRST + 1);
    wait_for_selected(&tinkery, FIRST, "    f00.txt");
    tinkery.wait_for_text(&format!("f{:02}.txt", visible - 1), WAIT);

    // One page lands on the first row that wasn't showing, at the bottom: no
    // row is skipped unseen.
    tinkery.send_keys("pagedown");
    wait_for_selected(&tinkery, last, &format!("    f{visible:02}.txt"));
    assert_eq!(
        tree_text(&tinkery.screen()[usize::from(FIRST)]),
        "    f01.txt"
    );

    tinkery.send_keys("pageup");
    wait_for_selected(&tinkery, FIRST, "    f00.txt");
}

#[test]
fn clicking_selects_rows_and_opens_files() {
    let mut tinkery = open_project();
    wait_for_tree(&tinkery, TOP);

    // A file row: selected and opened.
    tinkery.click(5, FIRST + 3);
    tinkery.wait_for_text("notes for the tree test", WAIT);
    wait_for_selected(&tinkery, FIRST + 3, "    notes.txt");

    // A folder row: selected and expanded.
    tinkery.click(3, FIRST + 1);
    wait_for_tree(&tinkery, SRC_OPEN);
    wait_for_selected(&tinkery, FIRST + 1, "  ▾ src");

    tinkery.click(10, FIRST + 3);
    tinkery.wait_for_text("hello from main", WAIT);
    wait_for_selected(&tinkery, FIRST + 3, "   │  main.rs");

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
    tinkery.wait_for_cursor(2, FIRST + 4, WAIT);
    tinkery.send_keys("up");
    wait_for_selected(&tinkery, FIRST + 3, "    notes.txt");
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
    dirty[4] = "    README.md            •";
    wait_for_tree(&tinkery, &dirty);
    tinkery.wait_for_cursor(2, FIRST + 3, WAIT);
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
