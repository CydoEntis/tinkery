//! Creating, renaming and refusing names from the file tree, on a temp copy of the
//! project fixture so the real one never changes. Delete goes to the OS trash, so
//! it's covered by unit tests with a fake trash instead.

mod harness;

use std::fs;
use std::path::Path;
use std::time::Duration;

use harness::{CAP_L, CAP_R, COLS, HYDRA, ROWS, Tinkery};
use vt100::Color;

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(5);
const TREE: usize = 28;
/// The row of the first node, below the tree's brand row.
const FIRST: usize = 3;
/// The prompt bar's row: just above the status line.
const BAR: usize = ROWS as usize - 2;
const STATUS: usize = ROWS as usize - 1;
/// hydra's `raised2`, the prompt bar's strip.
const RAISED2: Color = Color::Rgb(0x18, 0x24, 0x2f);
/// hydra's `bg`, the ground the strip's round ends sit on.
const BG: Color = Color::Rgb(0x07, 0x0b, 0x10);
/// hydra's `text`, the prompt bar's label.
const BAR_TEXT: Color = Color::Rgb(0xa7, 0xb4, 0xc2);
/// hydra's `muted`, the words of the prompt bar's hint.
const MUTED: Color = Color::Rgb(0x71, 0x80, 0x8f);
/// hydra's `btn`, a keycap.
const BTN: Color = Color::Rgb(0x1d, 0x2a, 0x37);
/// hydra's `strong`, the key on a keycap.
const STRONG: Color = Color::Rgb(0xf2, 0xf6, 0xf8);

const TOP: &[&str] = &[
    "  ▸ docs",
    "  ▸ src",
    "    .gitignore",
    "    notes.txt",
    "    README.md",
];

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create fixture copy");
    for entry in fs::read_dir(from).expect("read fixture") {
        let entry = entry.expect("read fixture entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy fixture file");
        }
    }
}

/// A temp copy of `tests/fixtures/project` with tinkery open on it, cwd inside it
/// so paths stay short.
fn open_copy() -> (tempfile::TempDir, Tinkery) {
    open_copy_with(HYDRA)
}

/// The same with `toml` as the config.
fn open_copy_with(toml: &str) -> (tempfile::TempDir, Tinkery) {
    let dir = tempfile::tempdir().expect("create temp dir");
    copy_dir(Path::new("tests/fixtures/project"), dir.path());
    let mut tinkery = Tinkery::spawn_in_with_config(dir.path(), toml, &["."]);
    tinkery.wait_for_text("Open directory", START);
    // The splash has the keys at launch and hides the tree; these tests work
    // in the tree.
    tinkery.send_keys("ctrl+e");
    wait_for_tree(&tinkery, TOP);
    (dir, tinkery)
}

/// The tree pane's node rows from the first one down to the first empty one.
fn tree_rows(screen: &[String]) -> Vec<String> {
    screen
        .iter()
        .skip(FIRST)
        .map(|line| {
            line.chars()
                .take(TREE)
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .take_while(|text| !text.is_empty())
        .collect()
}

fn wait_for_tree(tinkery: &Tinkery, expected: &[&str]) {
    tinkery.wait_for_screen(&format!("tree rows {expected:?}"), WAIT, |screen| {
        tree_rows(screen) == expected
    });
}

/// Waits for `row` to read `text` and glow across the pane, as the focused
/// selection and the open file do: hydra's `mix(surface, accent, .26)` at column
/// 0 fading to `surface` at 27.
fn wait_for_selected(tinkery: &Tinkery, row: u16, text: &str) {
    tinkery.wait_for_screen(&format!("tree row {row} {text:?}"), WAIT, |screen| {
        let line: String = screen[usize::from(row)].chars().take(TREE).collect();
        line.trim_end() == text
    });
    tinkery.wait_for_bg(0, row, Color::Rgb(0x3c, 0x4e, 0x24), WAIT);
    tinkery.wait_for_bg(27, row, Color::Rgb(0x0d, 0x14, 0x1b), WAIT);
}

/// What Enter and Esc do in the bar for `label`.
fn hint_for(label: &str) -> [(&'static str, &'static str); 2] {
    let enter = if label == "Rename" {
        "rename"
    } else {
        "create"
    };
    [("⏎", enter), ("esc", "cancel")]
}

/// Waits for the prompt bar to be a strip from edge to edge reading
/// ` Label  value`, with what Enter and Esc do at the right, a cell short of
/// the strip's end, then checks its colours: `raised2` with round ends on `bg`,
/// the label in `text`, each key a `btn` keycap in `strong` bold, the words in
/// `muted`.
fn wait_for_bar(tinkery: &Tinkery, label: &str, value: &str) {
    let hint = harness::hints_text(&hint_for(label));
    let hint_x = COLS - u16::try_from(hint.chars().count()).expect("short hint") - 2;
    let field = format!(" {label}  {value}");
    let expected = format!(
        "{CAP_L}{field:<width$}{hint} {CAP_R}",
        width = usize::from(hint_x) - 1
    );
    tinkery.wait_for_screen(&format!("prompt bar {expected:?}"), WAIT, |screen| {
        screen[BAR] == expected
    });
    let row = BAR as u16;
    // The strip's ends are its fill on the ground behind it.
    for x in [0, COLS - 1] {
        assert_eq!(tinkery.fg_at(x, row), RAISED2, "{x}");
        assert_eq!(tinkery.bg_at(x, row), BG, "{x}");
    }
    assert_eq!(tinkery.fg_at(2, row), BAR_TEXT);
    // The keys, and nothing else, are on `btn`, in `strong` and bold; their
    // round ends are `btn` on the strip.
    assert_eq!(tinkery.bg_text(row, BTN), "⏎esc");
    assert_eq!(tinkery.bold_text(row), "⏎esc");
    assert_eq!(tinkery.fg_at(hint_x + 1, row), STRONG);
    // (A blank has no colour of its own to read, so only the glyphs count.)
    let caps = format!("{CAP_L}{CAP_R}{CAP_L}{CAP_R}");
    assert_eq!(tinkery.fg_text(row, BTN).replace(' ', ""), caps);
    // Everything else between the strip's ends is on `raised2`.
    assert_eq!(
        tinkery.bg_text(row, RAISED2).chars().count(),
        usize::from(COLS) - 2 - "⏎esc".chars().count()
    );
    // ` create` and ` cancel`: the words follow their keycaps.
    assert_eq!(tinkery.fg_at(hint_x + 4, row), MUTED);
    assert_eq!(tinkery.fg_at(COLS - 3, row), MUTED);
}

fn wait_for_status(tinkery: &Tinkery, text: &str) {
    tinkery.wait_for_screen(&format!("status with {text:?}"), WAIT, |screen| {
        screen[STATUS].contains(text)
    });
}

#[test]
fn a_creates_a_file_beside_the_selected_file_and_opens_it() {
    let (dir, mut tinkery) = open_copy();
    for _ in 0..4 {
        tinkery.send_keys("down");
    }
    wait_for_selected(&tinkery, 7, "    README.md");

    tinkery.send_keys("a");
    wait_for_bar(&tinkery, "New file", "");
    tinkery.wait_for_cursor(12, BAR as u16, WAIT);
    tinkery.type_text("notes.md");
    wait_for_bar(&tinkery, "New file", "notes.md");
    tinkery.send_keys("enter");

    wait_for_tree(
        &tinkery,
        &[
            "  ▸ docs",
            "  ▸ src",
            "    .gitignore",
            "    notes.md",
            "    notes.txt",
            "    README.md",
        ],
    );
    wait_for_selected(&tinkery, 6, "    notes.md");
    // The bar is gone and the editor shows the new, empty file.
    tinkery.wait_for_screen("the bar closed", WAIT, |screen| {
        !screen[BAR].contains("New file")
    });
    // The message holds the path slot until the next key.
    wait_for_status(&tinkery, "✓ created notes.md");
    wait_for_status(&tinkery, "Ln 1, Col 1");
    let screen = tinkery.screen();
    let editor: String = screen[3].chars().skip(TREE + 1).collect();
    assert_eq!(editor.trim_end(), "   1", "{screen:#?}");
    assert_eq!(
        fs::read(dir.path().join("notes.md")).expect("read new file"),
        b""
    );

    // Typing goes into the new file.
    tinkery.type_text("hi");
    tinkery.send_keys("ctrl+s");
    tinkery.wait_for_files("notes.md saved", WAIT, || {
        fs::read_to_string(dir.path().join("notes.md")).is_ok_and(|text| text == "hi")
    });
}

#[test]
fn a_and_shift_a_create_inside_the_selected_folder() {
    let (dir, mut tinkery) = open_copy();
    wait_for_selected(&tinkery, 3, "  ▸ docs");

    tinkery.send_keys("a");
    wait_for_bar(&tinkery, "New file", "");
    tinkery.type_text("new.md");
    tinkery.send_keys("enter");
    // The folder opens to show the new file, which is selected.
    wait_for_tree(
        &tinkery,
        &[
            "  ▾ docs",
            "   │  guide.md",
            "   │  new.md",
            "  ▸ src",
            "    .gitignore",
            "    notes.txt",
            "    README.md",
        ],
    );
    wait_for_selected(&tinkery, 5, "   │  new.md");
    assert!(dir.path().join("docs").join("new.md").is_file());

    // Back in the tree, Shift+A makes a folder next to the selected file.
    tinkery.send_keys("ctrl+e");
    tinkery.send_keys("shift+a");
    wait_for_bar(&tinkery, "New folder", "");
    tinkery.type_text("drafts");
    tinkery.send_keys("enter");
    wait_for_tree(
        &tinkery,
        &[
            "  ▾ docs",
            "   │▸ drafts",
            "   │  guide.md",
            "   │  new.md",
            "  ▸ src",
            "    .gitignore",
            "    notes.txt",
            "    README.md",
        ],
    );
    wait_for_selected(&tinkery, 4, "   │▸ drafts");
    assert!(dir.path().join("docs").join("drafts").is_dir());
}

#[test]
fn r_renames_and_the_open_buffer_follows() {
    let (dir, mut tinkery) = open_copy();
    for _ in 0..3 {
        tinkery.send_keys("down");
    }
    tinkery.send_keys("enter");
    tinkery.wait_for_text("notes for the tree test", WAIT);

    tinkery.send_keys("ctrl+e");
    tinkery.send_keys("r");
    wait_for_bar(&tinkery, "Rename", "notes.txt");
    for _ in 0..3 {
        tinkery.send_keys("backspace");
    }
    tinkery.type_text("md");
    wait_for_bar(&tinkery, "Rename", "notes.md");
    tinkery.send_keys("enter");

    wait_for_tree(
        &tinkery,
        &[
            "  ▸ docs",
            "  ▸ src",
            "    .gitignore",
            "    notes.md",
            "    README.md",
        ],
    );
    wait_for_selected(&tinkery, 6, "    notes.md");
    wait_for_status(&tinkery, "renamed to notes.md");
    assert!(!dir.path().join("notes.txt").exists());

    // The buffer now belongs to notes.md: saving writes there.
    tinkery.send_keys("ctrl+e");
    tinkery.type_text("x");
    tinkery.send_keys("ctrl+s");
    tinkery.wait_for_files("notes.md saved", WAIT, || {
        fs::read_to_string(dir.path().join("notes.md"))
            .is_ok_and(|text| text.starts_with("xnotes for the tree test"))
    });
    assert!(!dir.path().join("notes.txt").exists());
}

#[test]
fn an_existing_or_invalid_name_changes_nothing() {
    let (dir, mut tinkery) = open_copy();
    let notes = fs::read(dir.path().join("notes.txt")).expect("read notes");
    for _ in 0..4 {
        tinkery.send_keys("down");
    }
    wait_for_selected(&tinkery, 7, "    README.md");

    tinkery.send_keys("a");
    wait_for_bar(&tinkery, "New file", "");
    tinkery.type_text("notes.txt");
    tinkery.send_keys("enter");
    wait_for_status(&tinkery, "notes.txt already exists");
    wait_for_tree(&tinkery, TOP);
    wait_for_selected(&tinkery, 7, "    README.md");
    assert_eq!(
        fs::read(dir.path().join("notes.txt")).expect("read notes"),
        notes
    );

    tinkery.send_keys("a");
    wait_for_bar(&tinkery, "New file", "");
    tinkery.type_text("bad/name");
    tinkery.send_keys("enter");
    wait_for_status(&tinkery, "invalid name");
    wait_for_tree(&tinkery, TOP);

    // Renaming onto another entry is refused too.
    tinkery.send_keys("r");
    wait_for_bar(&tinkery, "Rename", "README.md");
    for _ in 0.."README.md".len() {
        tinkery.send_keys("backspace");
    }
    tinkery.type_text("notes.txt");
    tinkery.send_keys("enter");
    wait_for_status(&tinkery, "notes.txt already exists");
    wait_for_tree(&tinkery, TOP);
    assert_eq!(
        fs::read(dir.path().join("notes.txt")).expect("read notes"),
        notes
    );
    assert!(dir.path().join("README.md").exists());

    // Esc closes the bar without doing anything.
    tinkery.send_keys("a");
    wait_for_bar(&tinkery, "New file", "");
    tinkery.type_text("never.md");
    tinkery.send_keys("esc");
    tinkery.wait_for_screen("the bar closed", WAIT, |screen| {
        !screen[BAR].contains("New file")
    });
    assert!(!dir.path().join("never.md").exists());
}

/// Opens the fixture copy with `toml` as the config and asks to rename
/// `notes.txt`, returning once the prompt is up.
fn renaming_with(toml: &str) -> (tempfile::TempDir, Tinkery) {
    let (dir, mut tinkery) = open_copy_with(toml);
    for _ in 0..3 {
        tinkery.send_keys("down");
    }
    tinkery.wait_for_screen("notes.txt selected", WAIT, |screen| {
        let line: String = screen[6].chars().take(TREE).collect();
        line.trim_end() == "    notes.txt"
    });
    tinkery.send_keys("r");
    tinkery.wait_for_text("Rename  notes.txt", WAIT);
    (dir, tinkery)
}

/// The prompt bar with square ends: every pill end a blank, so the label, the
/// keys and the words sit in the cells they have with round ends.
const SQUARE_BAR: &str = "  Rename  notes.txt";
const SQUARE_HINT: &str = " ⏎  rename    esc  cancel";

fn wait_for_square_bar(tinkery: &Tinkery) {
    let hint_x = usize::from(COLS) - SQUARE_HINT.chars().count() - 2;
    let expected = format!("{SQUARE_BAR:<hint_x$}{SQUARE_HINT}");
    tinkery.wait_for_screen(&format!("prompt bar {expected:?}"), WAIT, |screen| {
        screen[BAR] == expected
    });
    // No Nerd Font glyph anywhere on the screen.
    let screen = tinkery.screen().join("\n");
    assert!(
        !screen.contains(CAP_L) && !screen.contains(CAP_R),
        "{screen}"
    );
    tinkery.wait_for_cursor(19, BAR as u16, WAIT);
}

#[test]
fn pill_caps_off_squares_the_prompt_bar_and_its_keycaps() {
    let (_dir, tinkery) = renaming_with("theme = \"hydra\"\n[ui]\npill_caps = false\n");
    wait_for_square_bar(&tinkery);
    let row = BAR as u16;
    // A square end is a blank of the pill's own colour: the strip's are
    // `raised2`, a keycap's `btn`.
    for x in [0, COLS - 1] {
        assert_eq!(tinkery.bg_at(x, row), RAISED2, "{x}");
    }
    assert_eq!(
        tinkery.bg_text(row, RAISED2).chars().count(),
        usize::from(COLS) - " ⏎  esc ".chars().count()
    );
    assert_eq!(tinkery.bg_text(row, BTN), " ⏎  esc ");
    assert_eq!(tinkery.bold_text(row), "⏎esc");
    assert_eq!(tinkery.fg_at(COLS - 3, row), MUTED);
}

#[test]
fn mono_draws_the_prompt_bar_without_nerd_font_glyphs() {
    // `mono` never rounds an end, whatever `pill_caps` says.
    let (_dir, tinkery) = renaming_with("theme = \"mono\"\n[ui]\npill_caps = true\n");
    wait_for_square_bar(&tinkery);
    let row = BAR as u16;
    // The keys are bold, and the words beside them are not the strip's own
    // dark grey, which `mono`'s `muted` is.
    assert_eq!(tinkery.bold_text(row), "⏎esc");
    let word = COLS - 3;
    assert_ne!(tinkery.fg_at(word, row), tinkery.bg_at(word, row));
}
