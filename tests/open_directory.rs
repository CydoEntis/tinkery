//! `>open directory`: the folder browser, and opening the folder it picks as the
//! project (tinkery-splash spec S6–S9).

mod harness;

use std::fs;
use std::path::Path;
use std::time::Duration;

use harness::{CAP_L, CAP_R, HYDRA, ROWS, Tinkery, buttons_text, hints_text};
use tempfile::TempDir;
use vt100::Color;

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(10);

/// At 100x30 the browser is a panel the size of the cast's: 86 wide from
/// column 7, dropping from row 4, its title in the top edge. The header's strip
/// is row 6, row 7 is blank, the open row is row 8 and the folders run from
/// row 9, their text four cells in; the right-aligned text ends four cells in
/// from the card's edge. A row's pill runs the card's inside, from column 8 to
/// column 91, with `›` a cell into it.
const CARD_X: u16 = 7;
const CARD_Y: u16 = 4;
const CARD_WIDTH: u16 = 86;
const HEADER_ROW: u16 = 6;
const BLANK_ROW: u16 = 7;
const OPEN_ROW: u16 = 8;
const FIRST_ROW: u16 = 9;
const PILL_X: u16 = CARD_X + 1;
const PILL_END: u16 = CARD_X + CARD_WIDTH - 2;
const MARK_X: u16 = CARD_X + 2;
const TEXT_X: u16 = 11;
const RIGHT: u16 = 89;
const TITLE: &str = "▁ Open directory ▁";
const SCOPE: &str = "open · folders";
/// The run panel's title row at 30 rows.
const TITLE_ROW: u16 = 20;

/// The default theme, tinkery, which an empty config runs on: the roles the
/// browser is drawn in.
const DEFAULT: &str = "";
const ACCENT: Color = Color::Rgb(0xff, 0x8a, 0x3d);
const ACCENT2: Color = Color::Rgb(0xff, 0xd2, 0x7a);
const RAISED: Color = Color::Rgb(0x1f, 0x1b, 0x19);
const RAISED2: Color = Color::Rgb(0x29, 0x23, 0x20);
const HOV: Color = Color::Rgb(0x3a, 0x30, 0x2a);
const MUTED: Color = Color::Rgb(0x7d, 0x70, 0x67);
const TEXT: Color = Color::Rgb(0xb5, 0xa8, 0x9d);
const STRONG: Color = Color::Rgb(0xf8, 0xf0, 0xe6);
/// hydra's glow at the start of the tree's selected row.
const TREE_GLOW: Color = Color::Rgb(0x3c, 0x4e, 0x24);

/// The keys on the browser's last row, as keycaps with round ends.
fn hints() -> String {
    hints_text(&[
        ("⏎", "into"),
        ("←", "up"),
        ("Ctrl+⏎", "open here"),
        ("Esc", "close"),
    ])
}

/// The cell at (`col`, `y`), as text.
fn cell(tinkery: &Tinkery, col: u16, y: u16) -> String {
    row(tinkery, y)
        .chars()
        .nth(usize::from(col))
        .map(String::from)
        .unwrap_or_default()
}

/// `text` without the card's two edge cells, which share the edge's colour and
/// weight with what's inside, and without blanks.
fn inside(text: &str) -> String {
    text.replace(['▕', '▏', ' '], "")
}

/// Waits for row `y` to be the selected row and asserts it's drawn as one: a
/// pill of `hov` on the card's `raised` across its inside, `›` in the accent a
/// cell in, and no `⏎` at its right.
fn wait_for_pill(tinkery: &Tinkery, y: u16) {
    tinkery.wait_for_bg(MARK_X, y, HOV, WAIT);
    assert_eq!(cell(tinkery, PILL_X, y), CAP_L, "row {y}");
    assert_eq!(cell(tinkery, PILL_END, y), CAP_R, "row {y}");
    for x in [PILL_X, PILL_END] {
        assert_eq!(tinkery.fg_at(x, y), HOV, "the end at {x},{y}");
        assert_eq!(tinkery.bg_at(x, y), RAISED, "the end at {x},{y}");
    }
    assert_eq!(
        tinkery.bg_text(y, HOV).chars().count(),
        usize::from(CARD_WIDTH - 4),
        "the pill runs between its ends on row {y}"
    );
    assert_eq!(tinkery.text_col(y, "›"), Some(MARK_X), "row {y}");
    assert_eq!(tinkery.fg_at(MARK_X, y), ACCENT, "row {y}");
    assert!(tinkery.bold_at(MARK_X, y), "row {y}");
    assert_ne!(cell(tinkery, RIGHT - 1, y), "⏎", "row {y}");
}

/// Asserts row `y` isn't lit: no pill, no mark.
fn assert_no_pill(tinkery: &Tinkery, y: u16) {
    assert_eq!(tinkery.bg_text(y, HOV), "", "row {y}");
    assert_eq!(tinkery.bg_at(MARK_X, y), RAISED, "row {y}");
    assert_eq!(tinkery.text_col(y, "›"), None, "row {y}");
}

/// A folder holding `folders` (nested with `/`) and `files` (name, text).
fn folder(folders: &[&str], files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().expect("create temp folder");
    for name in folders {
        fs::create_dir_all(dir.path().join(name)).expect("create folder");
    }
    for (name, text) in files {
        fs::write(dir.path().join(name), text).expect("write file");
    }
    dir
}

/// The last part of `dir`'s path, which the header and brand row end with.
fn name(dir: &Path) -> String {
    dir.file_name()
        .expect("temp folders have names")
        .to_string_lossy()
        .into_owned()
}

fn row(tinkery: &Tinkery, row: u16) -> String {
    tinkery.screen()[usize::from(row)].clone()
}

/// Tinkery on `dir` as the project, tree showing.
fn open_in(dir: &Path, config: &str) -> Tinkery {
    let mut tinkery = Tinkery::spawn_in_with_config(dir, config, &["."]);
    tinkery.wait_for_text("Open directory", START);
    // The splash hides the tree; Ctrl+B shows it, with its brand on row 1.
    tinkery.send_keys("ctrl+b");
    tinkery.wait_for_screen("the tree's brand", WAIT, |screen| {
        screen[1].contains("✦ tinkery")
    });
    tinkery
}

/// Ctrl+P, `>open directory`, Enter.
fn open_browser(tinkery: &mut Tinkery) {
    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text("cast · files · commands", WAIT);
    tinkery.type_text(">open directory");
    tinkery.wait_for_text("cast · commands", WAIT);
    tinkery.wait_for_screen("Open directory listed", WAIT, |screen| {
        screen[usize::from(FIRST_ROW)].contains("Open directory")
    });
    tinkery.send_keys("enter");
    tinkery.wait_for_text(SCOPE, WAIT);
}

#[test]
fn open_directory_in_the_cast_opens_the_folder_browser() {
    let dir = folder(&["alpha", "Beta", ".hidden"], &[("notes.txt", "")]);
    let mut tinkery = open_in(dir.path(), DEFAULT);
    open_browser(&mut tinkery);
    let here = name(dir.path());

    // A panel the size of the cast's: an accent edge with the title in it,
    // around `raised`. No lit top edge.
    assert_eq!(tinkery.text_col(CARD_Y, TITLE), Some(CARD_X + 1));
    assert!(!row(&tinkery, CARD_Y).contains('▀'));
    assert_eq!(tinkery.fg_at(CARD_X + 1, CARD_Y), ACCENT);
    assert_eq!(tinkery.fg_at(CARD_X + 3, CARD_Y), ACCENT);
    assert_eq!(cell(&tinkery, CARD_X, HEADER_ROW), "▕");
    assert_eq!(cell(&tinkery, CARD_X + CARD_WIDTH - 1, HEADER_ROW), "▏");
    assert_eq!(tinkery.bg_at(TEXT_X, BLANK_ROW), RAISED);
    // The header's strip of `raised2`: `✦` and the folder, `open · folders`
    // at the right.
    assert_eq!(cell(&tinkery, PILL_X, HEADER_ROW), CAP_L);
    assert_eq!(cell(&tinkery, PILL_END, HEADER_ROW), CAP_R);
    assert_eq!(tinkery.fg_at(PILL_X, HEADER_ROW), RAISED2);
    assert_eq!(
        tinkery.bg_text(HEADER_ROW, RAISED2).chars().count(),
        usize::from(CARD_WIDTH - 4)
    );
    assert_eq!(tinkery.text_col(HEADER_ROW, "✦ "), Some(TEXT_X));
    assert_eq!(tinkery.fg_at(TEXT_X, HEADER_ROW), ACCENT2);
    assert_eq!(tinkery.fg_at(TEXT_X + 2, HEADER_ROW), TEXT);
    let header = row(&tinkery, HEADER_ROW);
    assert!(header.contains(&here), "{header:?}");
    assert_eq!(
        tinkery.text_col(HEADER_ROW, SCOPE),
        Some(RIGHT - u16::try_from(SCOPE.chars().count()).unwrap())
    );
    assert_eq!(tinkery.fg_at(RIGHT - 1, HEADER_ROW), MUTED);
    // Where the rule was, a blank row.
    for x in CARD_X + 1..CARD_X + CARD_WIDTH - 1 {
        assert_eq!(cell(&tinkery, x, BLANK_ROW), " ", "column {x}");
    }
    // The open row first, selected: the pill, its words `strong` bold.
    assert_eq!(tinkery.text_col(OPEN_ROW, "⏎ open "), Some(TEXT_X));
    assert!(row(&tinkery, OPEN_ROW).contains(&here));
    wait_for_pill(&tinkery, OPEN_ROW);
    assert_eq!(tinkery.fg_at(TEXT_X, OPEN_ROW), ACCENT);
    assert_eq!(inside(&tinkery.fg_text(OPEN_ROW, STRONG)), "open");
    assert_eq!(tinkery.fg_at(TEXT_X + 7, OPEN_ROW), TEXT);
    // Folders only, ignoring case, dot-folders last; none of them lit.
    assert_eq!(tinkery.text_col(FIRST_ROW, "▸ alpha"), Some(TEXT_X));
    assert_eq!(tinkery.text_col(FIRST_ROW + 1, "▸ Beta"), Some(TEXT_X));
    assert_eq!(tinkery.text_col(FIRST_ROW + 2, "▸ .hidden"), Some(TEXT_X));
    for y in FIRST_ROW..FIRST_ROW + 3 {
        assert_no_pill(&tinkery, y);
        assert_eq!(tinkery.fg_at(TEXT_X, y), MUTED, "row {y}");
        assert!(!tinkery.bold_at(TEXT_X + 2, y), "row {y}");
    }
    // A blank, then the keys as keycaps ending `Esc close`, then the edge.
    for x in CARD_X + 1..CARD_X + CARD_WIDTH - 1 {
        assert_eq!(cell(&tinkery, x, FIRST_ROW + 3), " ", "column {x}");
    }
    assert_eq!(tinkery.text_col(FIRST_ROW + 4, &hints()), Some(TEXT_X));
    assert!(
        row(&tinkery, FIRST_ROW + 4)
            .trim_end_matches([' ', '▏'])
            .ends_with(&format!("{CAP_L}Esc{CAP_R} close"))
    );
    assert_eq!(tinkery.text_col(FIRST_ROW + 5, "▔"), Some(CARD_X + 1));
    assert_eq!(row(&tinkery, FIRST_ROW + 5).matches('▔').count(), 84);
    assert!(
        !tinkery.screen()[..usize::from(ROWS - 1)]
            .iter()
            .skip(usize::from(CARD_Y))
            .any(|line| line
                .chars()
                .skip(usize::from(CARD_X))
                .collect::<String>()
                .contains("notes.txt"))
    );
}

#[test]
fn many_folders_scroll_past_the_card() {
    let names: Vec<String> = (0..14).map(|i| format!("dir{i:02}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let dir = folder(&refs, &[]);
    let mut tinkery = open_in(dir.path(), DEFAULT);
    open_browser(&mut tinkery);
    // Ten rows, then a blank and the keys.
    assert_eq!(tinkery.text_col(FIRST_ROW + 9, "▸ dir09"), Some(TEXT_X));
    assert_eq!(tinkery.text_col(FIRST_ROW + 11, &hints()), Some(TEXT_X));
    for _ in 0..14 {
        tinkery.send_keys("down");
    }
    tinkery.wait_for_screen("the last folder on the last row", WAIT, |screen| {
        screen[usize::from(FIRST_ROW + 9)].contains("▸ dir13")
    });
    assert!(row(&tinkery, FIRST_ROW).contains("▸ dir04"));
    // The pill went with the selection.
    wait_for_pill(&tinkery, FIRST_ROW + 9);
    assert_no_pill(&tinkery, OPEN_ROW);
    assert_no_pill(&tinkery, FIRST_ROW);
}

#[test]
fn mono_reverses_the_selected_row() {
    let dir = folder(&["alpha"], &[]);
    let mut tinkery = open_in(dir.path(), "theme = \"mono\"\n");
    open_browser(&mut tinkery);
    // The whole inside of the card, from edge to edge, with the mark.
    let selected = tinkery.reversed_text(OPEN_ROW);
    assert_eq!(selected.chars().count(), 84, "{selected:?}");
    assert!(selected.starts_with(" › ⏎ open "), "{selected:?}");
    // It carries no colours: one style across it, in the terminal's own.
    let colours = |tinkery: &Tinkery, y: u16| {
        let (fg, bg) = (tinkery.fg_at(PILL_X, y), tinkery.bg_at(PILL_X, y));
        for x in PILL_X..=PILL_END {
            assert_eq!(tinkery.fg_at(x, y), fg, "{x},{y}");
            assert_eq!(tinkery.bg_at(x, y), bg, "{x},{y}");
        }
        assert!(!matches!(fg, Color::Rgb(..)) && !matches!(bg, Color::Rgb(..)));
    };
    colours(&tinkery, OPEN_ROW);
    // No Nerd Font glyph anywhere.
    let screen = tinkery.screen().join("\n");
    assert!(
        !screen.contains(CAP_L) && !screen.contains(CAP_R),
        "{screen}"
    );
    assert!(row(&tinkery, FIRST_ROW + 2).contains(" Esc  close"));
    tinkery.send_keys("down");
    tinkery.wait_for_reversed(FIRST_ROW, &format!("{:<84}", " › ▸ alpha"), WAIT);
    colours(&tinkery, FIRST_ROW);
    assert_eq!(tinkery.reversed_text(OPEN_ROW), "");
}

#[test]
fn the_browser_goes_in_and_up_filters_and_closes() {
    let dir = folder(&["alpha/inner", "Beta", ".hidden"], &[]);
    let here = name(dir.path());
    let mut tinkery = open_in(dir.path(), DEFAULT);
    open_browser(&mut tinkery);

    // Enter on a folder goes into it.
    tinkery.send_keys("down");
    tinkery.send_keys("enter");
    tinkery.wait_for_screen("inside alpha", WAIT, |screen| {
        screen[usize::from(FIRST_ROW)].contains("▸ inner")
    });
    assert!(row(&tinkery, HEADER_ROW).contains("alpha"));
    // ← goes back up, with the folder just left selected.
    tinkery.send_keys("left");
    tinkery.wait_for_screen("back in the project", WAIT, |screen| {
        screen[usize::from(FIRST_ROW + 1)].contains("▸ Beta")
    });
    wait_for_pill(&tinkery, FIRST_ROW);
    assert_no_pill(&tinkery, OPEN_ROW);

    // Typing filters the folders; the open row stays.
    tinkery.type_text("bet");
    tinkery.wait_for_screen("only Beta", WAIT, |screen| {
        screen[usize::from(FIRST_ROW)].contains("▸ Beta")
            && !screen[usize::from(FIRST_ROW + 1)].contains("▸")
    });
    assert_eq!(tinkery.text_col(OPEN_ROW, "⏎ open "), Some(TEXT_X));
    // The best match is selected, its matched letters in the accent, bold.
    // "b" alone already lists only Beta, so wait for the whole query, which
    // follows the folder in the header.
    let typed = format!("{}bet ", std::path::MAIN_SEPARATOR);
    tinkery.wait_for_screen("the whole query", WAIT, |screen| {
        screen[usize::from(HEADER_ROW)].contains(&typed)
    });
    wait_for_pill(&tinkery, FIRST_ROW);
    assert_eq!(inside(&tinkery.fg_text(FIRST_ROW, ACCENT)), "›Bet");
    assert_eq!(inside(&tinkery.fg_text(FIRST_ROW, STRONG)), "a");
    assert_eq!(inside(&tinkery.bold_text(FIRST_ROW)), "›Beta");
    // Backspace edits the query while there is one, then goes up.
    for _ in 0..3 {
        tinkery.send_keys("backspace");
    }
    tinkery.wait_for_screen("every folder again", WAIT, |screen| {
        screen[usize::from(FIRST_ROW + 2)].contains("▸ .hidden")
    });
    tinkery.send_keys("backspace");
    tinkery.wait_for_screen("the parent folder", WAIT, |screen| {
        !screen[usize::from(HEADER_ROW)].contains(&here)
    });

    // Esc closes it.
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(SCOPE, WAIT);
    // So does a click outside the card, changing nothing.
    open_browser(&mut tinkery);
    tinkery.click(2, ROWS - 3);
    tinkery.wait_for_text_gone(SCOPE, WAIT);
    let brand: String = row(&tinkery, 1).chars().take(28).collect();
    assert!(
        brand.contains(&here[here.len().saturating_sub(6)..]),
        "{brand:?}"
    );
}

#[test]
fn a_folder_that_cannot_be_read_says_why_and_the_browser_stays() {
    let dir = folder(&["gone", "kept"], &[]);
    let mut tinkery = open_in(dir.path(), HYDRA);
    open_browser(&mut tinkery);
    fs::remove_dir(dir.path().join("gone")).expect("remove folder");
    tinkery.send_keys("down");
    tinkery.send_keys("enter");
    tinkery.wait_for_screen("the reason in the status line", WAIT, |screen| {
        screen[usize::from(ROWS - 1)].contains("cannot open")
    });
    assert!(row(&tinkery, ROWS - 1).contains("gone"));
    assert!(row(&tinkery, HEADER_ROW).contains(SCOPE));
    assert!(row(&tinkery, FIRST_ROW).contains("▸ gone"));
}

/// A command that runs until it's stopped.
fn long_command() -> &'static str {
    if cfg!(windows) {
        "ping -n 60 127.0.0.1 >NUL"
    } else {
        "sleep 60"
    }
}

#[test]
fn opening_a_folder_makes_it_the_project() {
    let old_toml = format!("[[run]]\nname = \"wait\"\ncommand = '{}'\n", long_command());
    let old = folder(&[], &[("old-file.txt", ""), (".tinkery.toml", &old_toml)]);
    let new_toml = "[[run]]\nname = \"hello\"\ncommand = \"echo switched-ok\"\n";
    let new = folder(
        &["sub"],
        &[("new-file.txt", ""), (".tinkery.toml", new_toml)],
    );
    let mut tinkery = open_in(old.path(), HYDRA);
    tinkery.wait_for_text("old-file.txt", WAIT);
    tinkery.send_keys("f5");
    tinkery.wait_for_screen("the old command running", WAIT, |screen| {
        screen[usize::from(TITLE_ROW)].contains("wait  running")
    });

    // A typed path jumps there; Enter on the open row opens it.
    open_browser(&mut tinkery);
    tinkery.type_text(&new.path().display().to_string());
    tinkery.send_keys("enter");
    tinkery.wait_for_screen("the new folder in the browser", WAIT, |screen| {
        screen[usize::from(FIRST_ROW)].contains("▸ sub")
    });
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone(SCOPE, WAIT);

    // The tree and brand show the new folder beside the no file open key
    // list, not the splash, and the tree has the keys; the run was stopped.
    tinkery.wait_for_text("new-file.txt", WAIT);
    assert!(
        !tinkery
            .screen()
            .iter()
            .any(|line| line.contains("old-file.txt"))
    );
    let brand: String = row(&tinkery, 1).chars().take(28).collect();
    let new_name = name(new.path());
    assert!(
        brand.contains(&new_name[new_name.len().saturating_sub(6)..]),
        "{brand:?}"
    );
    tinkery.wait_for_text("no file open", WAIT);
    assert!(
        !tinkery
            .screen()
            .iter()
            .any(|line| line.contains("Open directory")),
        "no splash after a switch"
    );
    // The selected first row glows only while the tree has focus.
    tinkery.wait_for_bg(0, 3, TREE_GLOW, WAIT);
    tinkery.wait_for_screen("the old command stopped", WAIT, |screen| {
        screen[usize::from(TITLE_ROW)].contains("wait  stopped")
    });

    // Ctrl+P lists the new folder's files.
    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text("cast · files · commands", WAIT);
    tinkery.wait_for_text("new-file.txt", WAIT);
    tinkery.type_text("file");
    tinkery.wait_for_screen("only the new file", WAIT, |screen| {
        screen[usize::from(FIRST_ROW)].contains("new-file.txt")
    });
    assert!(
        !tinkery
            .screen()
            .iter()
            .any(|line| line.contains("old-file.txt"))
    );
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone("cast · files · commands", WAIT);

    // The old run isn't restarted in the new folder: Ctrl+F5 is F5 again,
    // which runs the new folder's `.tinkery.toml`.
    tinkery.send_keys("ctrl+f5");
    tinkery.wait_for_screen("the new command's output", WAIT, |screen| {
        screen[usize::from(TITLE_ROW)].contains("hello")
            && screen[usize::from(TITLE_ROW + 1)].starts_with(" switched-ok")
    });
}

/// A project holding `a.txt`, `b.txt` and the folder `other` (with
/// `marker.txt`), tinkery open on `a.txt` with `x` typed into it.
fn edited_project() -> (TempDir, Tinkery) {
    let dir = folder(
        &["other"],
        &[
            (
                "a.txt", "hello
",
            ),
            (
                "b.txt", "bye
",
            ),
            ("other/marker.txt", ""),
        ],
    );
    let mut tinkery = Tinkery::spawn_in(dir.path(), &["a.txt"]);
    tinkery.wait_for_text("hello", START);
    tinkery.type_text("x");
    tinkery.wait_for_text("xhello", WAIT);
    (dir, tinkery)
}

/// Opens `b.txt` through Ctrl+P and types `y` into it.
fn edit_b(tinkery: &mut Tinkery) {
    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text("cast · files · commands", WAIT);
    tinkery.type_text("b.txt");
    tinkery.wait_for_screen("b.txt listed", WAIT, |screen| {
        screen[usize::from(FIRST_ROW)].contains("b.txt")
    });
    tinkery.send_keys("enter");
    tinkery.wait_for_text("bye", WAIT);
    tinkery.type_text("y");
    tinkery.wait_for_text("ybye", WAIT);
}

/// Picks `other` in the folder browser and opens it.
fn open_other(tinkery: &mut Tinkery) {
    open_browser(tinkery);
    tinkery.send_keys("down");
    tinkery.send_keys("enter");
    tinkery.wait_for_screen("inside other", WAIT, |screen| {
        screen[usize::from(HEADER_ROW)].contains("other")
    });
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone(SCOPE, WAIT);
}

fn read(dir: &Path, name: &str) -> String {
    fs::read_to_string(dir.join(name)).expect("read file")
}

#[test]
fn unsaved_files_ask_once_and_cancel_changes_nothing() {
    let (dir, mut tinkery) = edited_project();
    edit_b(&mut tinkery);
    open_other(&mut tinkery);
    // One card for both files, naming them, with its three buttons.
    tinkery.wait_for_text("2 files have unsaved changes", WAIT);
    tinkery.wait_for_text("a.txt, b.txt", WAIT);
    tinkery.wait_for_text(
        &buttons_text(&[("Save all", "S"), ("Discard", "D"), ("Cancel", "Esc")]),
        WAIT,
    );
    assert!(
        !tinkery
            .screen()
            .iter()
            .any(|line| line.contains("save or close unsaved files first"))
    );

    tinkery.type_text("c");
    tinkery.wait_for_text_gone("2 files have unsaved changes", WAIT);
    // Nothing changed: both edited tabs are open, nothing saved, no switch.
    assert!(tinkery.screen().iter().any(|line| line.contains("ybye")));
    assert!(
        !tinkery
            .screen()
            .iter()
            .any(|line| line.contains("marker.txt"))
    );
    assert_eq!(
        read(dir.path(), "a.txt"),
        "hello
"
    );
    assert_eq!(
        read(dir.path(), "b.txt"),
        "bye
"
    );
    tinkery.send_keys("alt+,");
    tinkery.wait_for_text("xhello", WAIT);
}

#[test]
fn save_all_saves_each_file_then_switches() {
    let (dir, mut tinkery) = edited_project();
    edit_b(&mut tinkery);
    open_other(&mut tinkery);
    tinkery.wait_for_text("2 files have unsaved changes", WAIT);
    tinkery.send_keys("enter");
    tinkery.wait_for_text("marker.txt", WAIT);
    assert_eq!(
        read(dir.path(), "a.txt"),
        "xhello
"
    );
    assert_eq!(
        read(dir.path(), "b.txt"),
        "ybye
"
    );
    assert!(!tinkery.screen().iter().any(|line| line.contains("ybye")));
}

#[test]
fn save_all_names_an_untitled_file_through_save_as() {
    let (dir, mut tinkery) = edited_project();
    tinkery.send_keys("ctrl+n");
    tinkery.wait_for_text("untitled", WAIT);
    tinkery.type_text("fresh");
    tinkery.wait_for_text("fresh", WAIT);
    open_other(&mut tinkery);
    tinkery.wait_for_text("2 files have unsaved changes", WAIT);
    tinkery.wait_for_text("a.txt, untitled", WAIT);
    tinkery.type_text("s");
    tinkery.wait_for_text(" Save as  ", WAIT);
    tinkery.type_text("named.txt");
    tinkery.send_keys("enter");
    tinkery.wait_for_text("marker.txt", WAIT);
    assert_eq!(
        read(dir.path(), "a.txt"),
        "xhello
"
    );
    assert_eq!(read(dir.path(), "named.txt"), "fresh");
}

#[test]
fn cancelling_the_save_as_cancels_the_switch() {
    let (dir, mut tinkery) = edited_project();
    tinkery.send_keys("ctrl+n");
    tinkery.wait_for_text("untitled", WAIT);
    tinkery.type_text("fresh");
    tinkery.wait_for_text("fresh", WAIT);
    open_other(&mut tinkery);
    tinkery.wait_for_text("2 files have unsaved changes", WAIT);
    tinkery.type_text("s");
    tinkery.wait_for_text(" Save as  ", WAIT);
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(" Save as  ", WAIT);
    // a.txt was saved before the untitled tab asked; nothing was closed and
    // the project stayed.
    assert_eq!(
        read(dir.path(), "a.txt"),
        "xhello
"
    );
    assert!(tinkery.screen().iter().any(|line| line.contains("fresh")));
    assert!(
        !tinkery
            .screen()
            .iter()
            .any(|line| line.contains("marker.txt"))
    );
    tinkery.send_keys("alt+,");
    tinkery.wait_for_text("xhello", WAIT);
}

#[test]
fn discard_switches_without_saving() {
    let (dir, mut tinkery) = edited_project();
    edit_b(&mut tinkery);
    open_other(&mut tinkery);
    tinkery.wait_for_text("2 files have unsaved changes", WAIT);
    tinkery.type_text("d");
    tinkery.wait_for_text("marker.txt", WAIT);
    assert!(!tinkery.screen().iter().any(|line| line.contains("ybye")));
    assert_eq!(
        read(dir.path(), "a.txt"),
        "hello
"
    );
    assert_eq!(
        read(dir.path(), "b.txt"),
        "bye
"
    );
}
