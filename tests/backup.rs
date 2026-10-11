mod harness;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use harness::{ROWS, Tinkery, buttons_text};
use tempfile::TempDir;

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(5);
const QUESTION: &str = "Recover unsaved changes to a.txt?";
/// At 100x30 the card is a panel 59 wide (the 51-cell explanation + 8) and 9
/// tall, centred (tinkery-forge spec F16): from column 20 and row 10, with its
/// text at column 23 and the buttons on row 16, ` Recover R ` then
/// ` Discard D ` at 36.
const CARD_X: u16 = 20;
const CARD_Y: u16 = 10;
const TEXT_X: u16 = 23;
const BUTTON_ROW: u16 = 16;
const DISCARD_X: u16 = 36;
/// Hydra's `accent`, the focused button's fill, and its `btn`, where the
/// other waits.
const ACCENT: vt100::Color = vt100::Color::Rgb(0xc3, 0xf5, 0x3c);
const BTN: vt100::Color = vt100::Color::Rgb(0x1d, 0x2a, 0x37);

/// A project folder holding `a.txt` and a separate data dir that survives relaunches.
struct Setup {
    files: TempDir,
    data: TempDir,
}

impl Setup {
    fn new(contents: &str) -> Self {
        let files = tempfile::tempdir().expect("create temp dir");
        let data = tempfile::tempdir().expect("create temp data dir");
        fs::write(files.path().join("a.txt"), contents).expect("write a.txt");
        Self { files, data }
    }

    fn launch(&self) -> Tinkery {
        let tinkery = Tinkery::spawn_in_with_data(self.files.path(), self.data.path(), &["a.txt"]);
        tinkery.wait_for_text("a.txt", START);
        tinkery
    }

    fn backups(&self) -> PathBuf {
        self.data.path().join("backups")
    }

    fn backup_files(&self) -> Vec<PathBuf> {
        backup_files(&self.backups())
    }

    fn read_a(&self) -> String {
        fs::read_to_string(self.files.path().join("a.txt")).expect("read a.txt")
    }
}

/// Finished backups only: a write in progress shows up as a dot-prefixed temp file.
fn backup_files(dir: &Path) -> Vec<PathBuf> {
    match fs::read_dir(dir) {
        Ok(entries) => entries
            .map(|e| e.expect("read backups entry").path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "bak"))
            .collect(),
        Err(_) => Vec::new(),
    }
}

fn status_line(tinkery: &Tinkery) -> String {
    tinkery.screen()[usize::from(ROWS - 1)].clone()
}

/// Edits `a.txt`, waits for its backup to land, then kills tinkery without
/// letting it clean up.
fn edit_and_crash(setup: &Setup) {
    let mut tinkery = setup.launch();
    tinkery.wait_for_text("Ln 1, Col 1", START);
    tinkery.type_text("xy");
    tinkery.wait_for_text("xyhello", WAIT);
    let typed = Instant::now();
    tinkery.wait_for_files("a backup file", WAIT, || {
        setup
            .backup_files()
            .iter()
            .any(|p| fs::read_to_string(p).is_ok_and(|t| t == "xyhello\n"))
    });
    // R12 allows 2 s; a slow CI box gets a little slack on top.
    let took = typed.elapsed();
    assert!(took < Duration::from_secs(3), "backup took {took:?}");
    let backups = setup.backup_files();
    assert_eq!(backups.len(), 1, "{backups:?}");
    let name = backups[0]
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    // Named by a hash of the path: 16 hex digits.
    assert!(
        name.len() == 20 && name[..16].chars().all(|c| c.is_ascii_hexdigit()),
        "{name}"
    );
    tinkery.kill();
    assert_eq!(setup.read_a(), "hello\n", "nothing was saved");
}

/// Waits for the recover card and checks its copy, with the backup's time, and
/// buttons sit where the panel puts them.
fn wait_for_card(setup: &Setup, tinkery: &Tinkery) {
    tinkery.wait_for_text(QUESTION, START);
    let backups = setup.backup_files();
    let written = fs::metadata(&backups[0])
        .and_then(|m| m.modified())
        .expect("backup time");
    let explanation = format!(
        "A backup from {} is newer than the file on disk.",
        chrono::DateTime::<chrono::Local>::from(written).format("%H:%M")
    );
    let screen = tinkery.screen();
    for (row, edge) in [(CARD_Y, "▁"), (CARD_Y + 8, "▔")] {
        assert_eq!(
            tinkery.text_col(row, &edge.repeat(57)),
            Some(CARD_X + 1),
            "{screen:#?}"
        );
        assert_eq!(tinkery.fg_at(CARD_X + 1, row), ACCENT);
    }
    assert_eq!(
        tinkery.text_col(CARD_Y + 2, QUESTION),
        Some(TEXT_X),
        "{screen:#?}"
    );
    assert_eq!(
        tinkery.text_col(CARD_Y + 3, &explanation),
        Some(TEXT_X),
        "{screen:#?}"
    );
    // Recover, the action, first and focused; no Cancel, since Esc doesn't
    // answer this card.
    assert_eq!(
        tinkery.text_col(
            BUTTON_ROW,
            &buttons_text(&[("Recover", "R"), ("Discard", "D")])
        ),
        Some(TEXT_X),
        "{screen:#?}"
    );
    assert_eq!(tinkery.bg_text(BUTTON_ROW, ACCENT), "Recover R");
    assert_eq!(tinkery.bg_text(BUTTON_ROW, BTN), "Discard D");
    assert_eq!(tinkery.text_col(BUTTON_ROW, "Discard"), Some(DISCARD_X + 1));
    assert!(
        !screen[usize::from(BUTTON_ROW)].contains("Esc"),
        "{screen:#?}"
    );
    assert_eq!(tinkery.underlined_text(BUTTON_ROW), "");
}

#[test]
fn recover_after_a_crash_loads_the_backup() {
    let setup = Setup::new("hello\n");
    edit_and_crash(&setup);

    let mut tinkery = setup.launch();
    wait_for_card(&setup, &tinkery);
    tinkery.type_text("r");
    tinkery.wait_for_text_gone(QUESTION, WAIT);
    tinkery.wait_for_text("xyhello", WAIT);
    tinkery.wait_for_text("a.txt •", WAIT);
    assert!(status_line(&tinkery).contains("a.txt •"));
    // The backup stays until the recovered text is saved.
    assert_eq!(setup.backup_files().len(), 1);

    tinkery.send_keys("ctrl+s");
    tinkery.wait_for_text("saved a.txt", WAIT);
    assert_eq!(setup.read_a(), "xyhello\n");
    tinkery.wait_for_files("the backup to go", WAIT, || setup.backup_files().is_empty());
    tinkery.send_keys("ctrl+q");
    assert!(tinkery.wait_exit(WAIT).success());
}

#[test]
fn discard_after_a_crash_deletes_the_backup() {
    let setup = Setup::new("hello\n");
    edit_and_crash(&setup);

    let mut tinkery = setup.launch();
    wait_for_card(&setup, &tinkery);
    tinkery.type_text("d");
    tinkery.wait_for_text_gone(QUESTION, WAIT);
    tinkery.wait_for_text("hello", WAIT);
    let status = status_line(&tinkery);
    assert!(!status.contains('●'), "dirty after discard: {status:?}");
    assert!(!tinkery.screen().iter().any(|l| l.contains("xyhello")));
    tinkery.wait_for_files("the backup to go", WAIT, || setup.backup_files().is_empty());

    // Nothing is offered the next time.
    tinkery.send_keys("ctrl+q");
    assert!(tinkery.wait_exit(WAIT).success());
    let mut tinkery = setup.launch();
    tinkery.wait_for_text("Ln 1, Col 1", START);
    tinkery.assert_running_for(Duration::from_millis(300));
    assert!(!tinkery.screen().iter().any(|l| l.contains(QUESTION)));
    tinkery.send_keys("ctrl+q");
    assert!(tinkery.wait_exit(WAIT).success());
}

#[test]
fn quitting_cleanly_deletes_the_backup() {
    let setup = Setup::new("hello\n");
    let mut tinkery = setup.launch();
    tinkery.wait_for_text("Ln 1, Col 1", START);
    tinkery.type_text("x");
    tinkery.wait_for_text("a.txt •", WAIT);
    tinkery.wait_for_files("a backup file", WAIT, || !setup.backup_files().is_empty());
    tinkery.send_keys("ctrl+q");
    tinkery.wait_for_text("has unsaved changes", WAIT);
    tinkery.type_text("d");
    assert!(tinkery.wait_exit(WAIT).success());
    assert!(setup.backup_files().is_empty());
    assert_eq!(setup.read_a(), "hello\n");
}

#[test]
fn enter_recovers_and_a_click_on_discard_discards() {
    let setup = Setup::new("hello\n");
    edit_and_crash(&setup);

    // Recover is the default: Enter presses it.
    let mut tinkery = setup.launch();
    wait_for_card(&setup, &tinkery);
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone(QUESTION, WAIT);
    tinkery.wait_for_text("xyhello", WAIT);
    tinkery.kill();

    // The backup is still there, so it's offered again; a click discards it.
    let mut tinkery = setup.launch();
    wait_for_card(&setup, &tinkery);
    tinkery.click(DISCARD_X + 2, BUTTON_ROW);
    tinkery.wait_for_text_gone(QUESTION, WAIT);
    tinkery.wait_for_files("the backup to go", WAIT, || setup.backup_files().is_empty());
    assert!(!tinkery.screen().iter().any(|l| l.contains("xyhello")));
    tinkery.send_keys("ctrl+q");
    assert!(tinkery.wait_exit(WAIT).success());
}
