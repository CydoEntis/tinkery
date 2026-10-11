mod harness;

use std::ffi::OsString;
use std::time::Duration;

use harness::{TOAST_ROW, Tinkery};

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(5);
const PROJECT: &str = "tests/fixtures/project";
/// The row of the tree's first node, below the brand row, and of its fifth
/// and last: `docs`, `src`, `.gitignore`, `notes.txt`, `README.md`.
const FIRST: u16 = 3;
const LAST: u16 = 7;
/// The column of the selection's `›` in the tree.
const MARK_X: usize = 2;

/// The fixture project with the tree focused and its first row selected, on
/// `config`. `TINKERY_MOTION=1` takes back the harness's `TINKERY_MOTION=0`,
/// so `[ui] motion` decides.
fn open_project(config: &str) -> Tinkery {
    let dir = std::env::current_dir().expect("test process has a current directory");
    let env = [("TINKERY_MOTION", OsString::from("1"))];
    let mut tinkery = Tinkery::spawn_in_with_config_and_env(&dir, config, &env, &[PROJECT]);
    tinkery.wait_for_text("Open directory", START);
    tinkery.send_keys("ctrl+e");
    wait_for_selected(&tinkery, FIRST);
    tinkery
}

/// The tree rows of `screen` that have the selection's `›`.
fn marked(screen: &[String]) -> Vec<u16> {
    (0u16..)
        .zip(screen)
        .filter(|(_, line)| line.chars().nth(MARK_X) == Some('›'))
        .map(|(row, _)| row)
        .collect()
}

fn wait_for_selected(tinkery: &Tinkery, row: u16) {
    tinkery.wait_for_screen(&format!("the selection on row {row}"), WAIT, |screen| {
        marked(screen) == [row]
    });
}

#[test]
fn with_motion_off_the_first_frame_after_a_move_is_the_final_one() {
    let mut tinkery = open_project("[ui]\nmotion = false\n");
    tinkery.record_frames();
    // From the first row to the last, four rows down: gliding, the pill
    // would be drawn on the rows between.
    tinkery.send_keys("ctrl+end");
    wait_for_selected(&tinkery, LAST);
    let frames = tinkery.frames();
    let rows: Vec<Vec<u16>> = frames.iter().map(|frame| marked(frame)).collect();
    // Whatever was drawn before the key was read shows the old row; from the
    // first frame after it, the pill is on the new one and stays there.
    let moved = rows.iter().position(|row| row != &[FIRST]);
    let moved = moved.unwrap_or_else(|| panic!("no frame shows the move: {rows:?}"));
    assert!(
        rows[moved..].iter().all(|row| row == &[LAST]),
        "the pill was drawn on rows {rows:?}"
    );
    // And that first frame is the final one, cell for cell.
    assert_eq!(frames[moved], tinkery.screen());
}

#[test]
fn with_motion_on_the_selection_lands_on_its_row_and_text_never_moves() {
    let mut tinkery = open_project("");
    let before = tinkery.screen();
    tinkery.record_frames();
    tinkery.send_keys("ctrl+end");
    wait_for_selected(&tinkery, LAST);
    // Every frame on the way has the pill on one row, from the first to the
    // last, moving one way, and each node's name where it always was.
    let name = |line: &str| -> String { line.chars().skip(MARK_X + 1).take(20).collect() };
    let mut at = FIRST;
    for frame in tinkery.frames() {
        let rows = marked(&frame);
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert!((at..=LAST).contains(&rows[0]), "{rows:?} after row {at}");
        at = rows[0];
        for row in FIRST..=LAST {
            let (now, then) = (&frame[usize::from(row)], &before[usize::from(row)]);
            assert_eq!(name(now), name(then), "row {row}");
        }
    }
    assert_eq!(at, LAST);
}

#[test]
fn with_motion_on_a_toast_comes_to_rest_and_still_leaves_when_its_hold_is_up() {
    let mut tinkery = open_project("");
    // Nothing is running, which F5's stop says in a toast.
    tinkery.send_keys("shift+f5");
    // In place it ends two cells short of the right edge, as with motion off.
    tinkery.wait_for_screen("the toast in place", WAIT, |screen| {
        let line = &screen[usize::from(TOAST_ROW)];
        line.chars().count() == usize::from(harness::COLS) - 2 && line.ends_with(harness::CAP_R)
    });
    let line = tinkery.screen()[usize::from(TOAST_ROW)].clone();
    assert!(line.contains("nothing is running"), "{line:?}");
    tinkery.wait_for_screen("the toast gone", WAIT, |screen| {
        !screen[usize::from(TOAST_ROW)].contains(harness::CAP_R)
    });
}

#[test]
fn settings_lists_motion_under_ui() {
    let project = tempfile::tempdir().expect("create project");
    let home = tempfile::tempdir().expect("create config home");
    let config = home.path().join("tinkery").join("config.toml");
    std::fs::write(project.path().join("a.txt"), "alpha\n").expect("write a.txt");
    let env = [("TINKERY_CONFIG", OsString::from(&config))];
    let mut tinkery = Tinkery::spawn_in_with_env(project.path(), &env, &["a.txt"]);
    tinkery.wait_for_text("alpha", START);
    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text("cast · files · commands", WAIT);
    tinkery.type_text(">settings");
    tinkery.wait_for_text("cast · commands", WAIT);
    tinkery.wait_for_screen("Settings listed", WAIT, |screen| {
        screen[9].contains("Settings")
    });
    tinkery.send_keys("enter");
    tinkery.wait_for_text("# corners = \"flush\"", WAIT);
    tinkery.wait_for_text("# motion = true", WAIT);
    let written = std::fs::read_to_string(&config).expect("config.toml written");
    assert!(written.contains("# motion = true"), "{written}");
}
