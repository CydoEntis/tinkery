mod harness;

use std::fs;
use std::path::Path;
use std::time::Duration;

use harness::{Tinkery, buttons_text};

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(5);
/// `notes.txt` has `TODO` on line 1 and `todo`, `TODO` on line 3; `src/app.rs` a
/// `// TODO` on line 2; `README.md` none. Four matches in two files.
const FIXTURE: &str = "tests/fixtures/project_replace";

/// At 100x30 the panel's card starts at column 10, row 4: the Replace field is
/// on row 6 with its label at column 14 and its value at 23, the match count on
/// row 7 two cells in, and the hits from row 8.
const REPLACE_ROW: u16 = 6;
const LABEL_X: u16 = 14;
const VALUE_X: u16 = 23;
const STATUS_ROW: u16 = 7;
const STATUS_X: usize = 12;
const FIRST_ROW: u16 = 8;
const CARD_X: u16 = 10;
/// The panel's key hints, there while it's open.
const FOOTER: &str = "alt+a replace all   esc close";
/// The selected row's glow at its left end: hydra's `raised` (`#0f1821`) lit 30 %
/// towards its accent (`#c3f53c`).
const GLOW: vt100::Color = vt100::Color::Rgb(0x45, 0x5a, 0x29);
/// Hydra's `accent`, `err` and `btn`: a confirm card's focused button, the
/// same when pressing it loses work, and a button that waits.
const ACCENT: vt100::Color = vt100::Color::Rgb(0xc3, 0xf5, 0x3c);
const ERR: vt100::Color = vt100::Color::Rgb(0xff, 0x6b, 0x6b);
const BTN: vt100::Color = vt100::Color::Rgb(0x1d, 0x2a, 0x37);
/// The confirm card's button row at 100x30, where ` Replace R ` starts and
/// where ` Cancel Esc ` does.
const BUTTON_ROW: u16 = 16;
const REPLACE_X: u16 = 11;
const CANCEL_X: u16 = 24;

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            fs::create_dir(&target)?;
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// A temp copy of the fixture, so replacing never touches the real one.
fn fixture_copy() -> std::io::Result<tempfile::TempDir> {
    let dir = tempfile::tempdir()?;
    copy_dir(Path::new(FIXTURE), dir.path())?;
    Ok(dir)
}

/// Tinkery in `dir` with no file open, so the root is that folder. Opening
/// `src/app.rs` would start the real rust-analyzer, which on CI can crash at any
/// moment and put its own message over the one a test waits for; a server that
/// isn't found is reported once, as the file opens, and never again.
fn open_in(dir: &Path) -> Tinkery {
    let tinkery = Tinkery::spawn_in_with_config(
        dir,
        "theme = \"hydra\"\n[lsp.rust]\ncommand = \"tinkery-no-such-server\"\n",
        &[],
    );
    tinkery.wait_for_text("Open directory", START);
    tinkery
}

/// Waits for the panel's count row to say exactly `status`, e.g.
/// `4 matches in 2 files`, and not `searching… 4 matches in 2 files`.
fn wait_for_status(tinkery: &Tinkery, status: &str) {
    tinkery.wait_for_screen(&format!("the panel to say {status:?}"), WAIT, |lines| {
        let shown: String = lines[usize::from(STATUS_ROW)]
            .chars()
            .skip(STATUS_X)
            .collect();
        shown.trim_end() == status
    });
}

/// Waits for a toast to show `message`.
fn wait_for_message(tinkery: &Tinkery, message: &str) {
    tinkery.wait_for_toast(message, WAIT);
}

/// Opens the panel and searches for `query`.
fn search(tinkery: &mut Tinkery, query: &str) {
    tinkery.send_keys("alt+f");
    tinkery.wait_for_text(FOOTER, WAIT);
    tinkery.type_text(query);
    tinkery.wait_for_text(&format!("Search   {query}"), WAIT);
    tinkery.send_keys("enter");
}

const QUESTION: &str = "Replace 4 matches in 2 files?";
const EXPLANATION: &str =
    "Open buffers are edited in place (undo with Ctrl+Z); other files are saved.";

/// Searches for `todo`, Tab, `done` in the Replace field, then Alt+A, and checks
/// the confirm card.
fn replace_todo(tinkery: &mut Tinkery) {
    search(tinkery, "todo");
    wait_for_status(tinkery, "4 matches in 2 files");
    // A key and quick typing after it can arrive as one paste on Windows, so wait
    // for each key to land before the next.
    tinkery.send_keys("tab");
    tinkery.wait_for_cursor(VALUE_X, REPLACE_ROW, WAIT);
    tinkery.type_text("done");
    tinkery.wait_for_text("Replace  done", WAIT);
    tinkery.send_keys("alt+a");
    tinkery.wait_for_text(QUESTION, WAIT);
    // At 100x30 the card is a panel 83 wide (the 75-cell explanation + 8) and
    // 9 tall, centred from column 8 and row 10, with its text from column 11
    // (tinkery-forge spec F16).
    let screen = tinkery.screen();
    assert_eq!(
        tinkery.text_col(10, &"▁".repeat(81)),
        Some(9),
        "{screen:#?}"
    );
    assert_eq!(tinkery.text_col(12, QUESTION), Some(11), "{screen:#?}");
    assert_eq!(tinkery.text_col(13, EXPLANATION), Some(11), "{screen:#?}");
    assert_eq!(
        tinkery.text_col(
            BUTTON_ROW,
            &buttons_text(&[("Replace", "R"), ("Cancel", "Esc")])
        ),
        Some(REPLACE_X),
        "{screen:#?}"
    );
    // Replacing every match loses what was there, so the focused button is
    // red; Cancel waits.
    assert_eq!(tinkery.bg_text(BUTTON_ROW, ERR), "Replace R");
    assert_eq!(tinkery.bg_text(BUTTON_ROW, BTN), "Cancel Esc");
    assert_eq!(tinkery.bg_text(BUTTON_ROW, ACCENT), "");
    assert_eq!(tinkery.underlined_text(BUTTON_ROW), "");
}

#[test]
fn alt_enter_asks_then_replaces_in_every_listed_file_and_searches_again() -> std::io::Result<()> {
    let dir = fixture_copy()?;
    let notes = dir.path().join("notes.txt");
    let code = dir.path().join("src").join("app.rs");
    let notes_before = fs::read_to_string(&notes)?;
    let code_before = fs::read_to_string(&code)?;
    let mut tinkery = open_in(dir.path());
    search(&mut tinkery, "todo");
    wait_for_status(&tinkery, "4 matches in 2 files");
    assert_eq!(tinkery.text_col(REPLACE_ROW, "Replace  "), Some(LABEL_X));
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(FOOTER, WAIT);

    replace_todo(&mut tinkery);
    tinkery.type_text("r");
    wait_for_message(&tinkery, "Replaced 4 in 2 files");
    tinkery.wait_for_text_gone("Replace 4 matches", WAIT);
    // The list refreshed: nothing matches any more.
    wait_for_status(&tinkery, "no matches");
    tinkery.wait_for_text_gone("notes.txt:", WAIT);

    // Saved with the line endings each file had.
    let done = |text: &str| text.replace("TODO", "done").replace("todo", "done");
    assert_eq!(fs::read_to_string(&notes)?, done(&notes_before));
    assert_eq!(fs::read_to_string(&code)?, done(&code_before));
    Ok(())
}

#[test]
fn cancel_closes_the_prompt_and_changes_nothing() -> std::io::Result<()> {
    let dir = fixture_copy()?;
    let notes = dir.path().join("notes.txt");
    let before = fs::read(&notes)?;
    let mut tinkery = open_in(dir.path());
    replace_todo(&mut tinkery);
    tinkery.type_text("c");
    tinkery.wait_for_text_gone("Replace 4 matches", WAIT);
    wait_for_status(&tinkery, "4 matches in 2 files");
    // So do Tab to Cancel and Enter.
    tinkery.send_keys("alt+a");
    tinkery.wait_for_text(QUESTION, WAIT);
    tinkery.send_keys("tab");
    tinkery.wait_for_bg(CANCEL_X + 1, BUTTON_ROW, ACCENT, WAIT);
    assert_eq!(tinkery.bg_text(BUTTON_ROW, ACCENT), "Cancel Esc");
    assert_eq!(tinkery.bg_text(BUTTON_ROW, BTN), "Replace R");
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone("Replace 4 matches", WAIT);
    wait_for_status(&tinkery, "4 matches in 2 files");
    assert_eq!(fs::read(&notes)?, before);
    Ok(())
}

#[test]
fn an_open_buffer_is_edited_in_place_and_left_unsaved() -> std::io::Result<()> {
    let dir = fixture_copy()?;
    let code = dir.path().join("src").join("app.rs");
    let code_before = fs::read(&code)?;
    let mut tinkery = open_in(dir.path());
    // Open `src/app.rs` from its hit, the third.
    search(&mut tinkery, "todo");
    wait_for_status(&tinkery, "4 matches in 2 files");
    tinkery.send_keys("down");
    tinkery.send_keys("down");
    tinkery.wait_for_bg(CARD_X, FIRST_ROW + 2, GLOW, WAIT);
    assert!(tinkery.screen()[usize::from(FIRST_ROW + 2)].contains("src/app.rs:2  // TODO tidy"));
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone(FOOTER, WAIT);
    tinkery.wait_for_text("2      // TODO tidy", WAIT);

    replace_todo(&mut tinkery);
    tinkery.type_text("r");
    wait_for_message(&tinkery, "Replaced 4 in 2 files");
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(FOOTER, WAIT);
    tinkery.wait_for_text("2      // done tidy", WAIT);
    tinkery.wait_for_text("app.rs •", WAIT);
    assert_eq!(fs::read(&code)?, code_before);

    // One undo takes it all back.
    tinkery.send_keys("ctrl+z");
    tinkery.wait_for_text("2      // TODO tidy", WAIT);
    Ok(())
}

#[test]
fn a_file_that_cannot_be_written_is_reported_and_the_others_still_go() -> std::io::Result<()> {
    let dir = fixture_copy()?;
    let notes = dir.path().join("notes.txt");
    let code = dir.path().join("src").join("app.rs");
    let notes_before = fs::read(&notes)?;
    let mut perms = fs::metadata(&notes)?.permissions();
    perms.set_readonly(true);
    fs::set_permissions(&notes, perms.clone())?;

    let mut tinkery = open_in(dir.path());
    replace_todo(&mut tinkery);
    tinkery.type_text("r");
    wait_for_message(
        &tinkery,
        "Replaced 1 in 1 file · cannot write notes.txt: read-only",
    );
    // Only the file that failed still has matches.
    wait_for_status(&tinkery, "3 matches in 1 file");
    let notes_after = fs::read(&notes)?;
    // tempdir can't delete a read-only file on Windows.
    #[expect(
        clippy::permissions_set_readonly_false,
        reason = "only to let the temp dir clean up"
    )]
    perms.set_readonly(false);
    fs::set_permissions(&notes, perms)?;
    assert_eq!(notes_after, notes_before);
    assert!(fs::read_to_string(&code)?.contains("// done tidy"));
    Ok(())
}
