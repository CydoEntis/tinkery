//! Hover (tinkery-forge spec F21): moving the mouse lights the row, button,
//! tab or chip under it in `hov`, on every screen, and changes nothing else;
//! with a dialog open, nothing behind it lights.

mod harness;

use std::fs;
use std::time::Duration;

use harness::{ROWS, Tinkery};
use tempfile::TempDir;
use vt100::Color;

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(10);

/// The default theme, tinkery, which an empty config runs on.
const DEFAULT: &str = "";
const HOV: Color = Color::Rgb(0x3a, 0x30, 0x2a);
const BTN: Color = Color::Rgb(0x33, 0x2a, 0x25);
const ACCENT: Color = Color::Rgb(0xff, 0x8a, 0x3d);
const RAISED: Color = Color::Rgb(0x1f, 0x1b, 0x19);

/// The mark of a selected row, which the row under the pointer never gets.
const MARK: &str = "›";
/// A cell of the editor with nothing to light under it.
const NOWHERE: (u16, u16) = (60, 26);
const STATUS: u16 = ROWS - 1;

/// Three text files and a folder: nothing a language server starts for, so no
/// toast is drawn over what these tests read.
fn project() -> TempDir {
    let dir = tempfile::tempdir().expect("create project");
    fs::write(dir.path().join("alpha.txt"), "alpha\nneedle one\n").expect("write alpha.txt");
    fs::write(dir.path().join("beta.txt"), "beta\nneedle two\n").expect("write beta.txt");
    fs::write(dir.path().join("gamma.txt"), "gamma\n").expect("write gamma.txt");
    fs::create_dir(dir.path().join("sub")).expect("create sub");
    dir
}

/// Tinkery on `dir` as the project, on the splash.
fn on_splash(dir: &TempDir) -> Tinkery {
    let tinkery = Tinkery::spawn_in_with_config(dir.path(), DEFAULT, &["."]);
    tinkery.wait_for_text("Open directory", START);
    tinkery
}

/// Tinkery with `alpha.txt` of `dir` open in its one tab.
fn on_file(dir: &TempDir) -> Tinkery {
    let tinkery = Tinkery::spawn_in_with_config(dir.path(), DEFAULT, &["alpha.txt"]);
    tinkery.wait_for_text("needle one", START);
    tinkery
}

/// Waits for a row from `from` down to show `text` and returns it.
fn row_with(tinkery: &Tinkery, from: u16, text: &str) -> u16 {
    let found = |screen: &[String]| {
        screen
            .iter()
            .enumerate()
            .skip(usize::from(from))
            .find(|(_, line)| line.contains(text))
            .map(|(row, _)| row)
    };
    tinkery.wait_for_screen(&format!("a row showing {text:?}"), WAIT, |screen| {
        found(screen).is_some()
    });
    let row = found(&tinkery.screen()).expect("waited for it");
    u16::try_from(row).expect("a screen row")
}

/// The column `text` starts at on `row`.
fn col_of(tinkery: &Tinkery, row: u16, text: &str) -> u16 {
    tinkery
        .text_col(row, text)
        .unwrap_or_else(|| panic!("{text:?} is not on row {row}: {:#?}", tinkery.screen()))
}

/// Moves the pointer onto (`col`, `row`), which rests on `rest`, and waits for
/// it to light.
fn light(tinkery: &mut Tinkery, col: u16, row: u16, rest: Color) {
    assert_eq!(tinkery.bg_at(col, row), rest, "({col}, {row}) at rest");
    tinkery.move_to(col, row);
    tinkery.wait_for_bg(col, row, HOV, WAIT);
}

/// Moves the pointer off everything and waits for (`col`, `row`) to go back
/// to `rest`.
fn leave(tinkery: &mut Tinkery, col: u16, row: u16, rest: Color) {
    tinkery.move_to(NOWHERE.0, NOWHERE.1);
    tinkery.wait_for_bg(col, row, rest, WAIT);
}

/// A list in a panel: the row under the pointer, the one after the selected
/// row, lights as a pill without the mark; the selection keeps its mark and
/// its place; and the row goes back to `raised` when the pointer leaves the
/// card. Returns the selected row.
fn hover_the_row_after_the_selection(tinkery: &mut Tinkery, from: u16) -> u16 {
    let selected = row_with(tinkery, from, MARK);
    let mark_x = col_of(tinkery, selected, MARK);
    let (row, col) = (selected + 1, mark_x + 12);
    tinkery.wait_for_bg(col, selected, HOV, WAIT);
    light(tinkery, col, row, RAISED);
    // The pill runs the card's inside, and carries no mark.
    assert_eq!(tinkery.bg_at(mark_x, row), HOV);
    assert_eq!(tinkery.text_col(row, MARK), None);
    // The selection is where it was.
    assert_eq!(tinkery.text_col(selected, MARK), Some(mark_x));
    assert_eq!(tinkery.bg_at(col, selected), HOV);
    // Along the row it stays lit; off the card it rests again.
    tinkery.move_to(col + 20, row);
    tinkery.move_to(col + 21, row);
    tinkery.move_to(2, row);
    tinkery.wait_for_bg(col, row, RAISED, WAIT);
    assert_eq!(tinkery.text_col(selected, MARK), Some(mark_x));
    tinkery.move_to(col, row);
    tinkery.wait_for_bg(col, row, HOV, WAIT);
    selected
}

#[test]
fn the_tree_lights_the_row_under_the_pointer_and_keeps_the_active_file_lit() {
    let dir = project();
    let mut tinkery = on_splash(&dir);
    tinkery.send_keys("ctrl+b");
    let alpha = row_with(&tinkery, 3, "alpha.txt");
    let beta = row_with(&tinkery, 3, "beta.txt");
    let name_x = col_of(&tinkery, beta, "beta.txt");
    // A click opens `alpha.txt` and selects its row; a click in the editor
    // takes the keys back, and the active file's row stays lit, unmarked.
    tinkery.click(name_x, alpha);
    tinkery.wait_for_text("needle one", WAIT);
    tinkery.click(NOWHERE.0, NOWHERE.1);
    tinkery.wait_for_screen("the active file's row unmarked", WAIT, |screen| {
        !screen[usize::from(alpha)].contains(MARK)
    });
    tinkery.wait_for_bg(name_x, alpha, HOV, WAIT);
    assert!(tinkery.bold_at(name_x, alpha));
    let surface = tinkery.bg_at(name_x, beta);
    assert_ne!(surface, HOV);

    // The pointer's row is the same pill, its name as it was: not bold.
    light(&mut tinkery, name_x, beta, surface);
    assert!(!tinkery.bold_at(name_x, beta));
    assert_eq!(tinkery.text_col(beta, MARK), None);
    // Nothing was opened, selected or focused: the file shown is the same.
    assert!(tinkery.bold_at(name_x, alpha));
    assert!(tinkery.screen()[usize::from(STATUS)].contains("alpha.txt"));
    assert!(tinkery.screen().join("\n").contains("needle one"));

    // The pointer gone, its row rests and the active file's is still lit.
    leave(&mut tinkery, name_x, beta, surface);
    assert_eq!(tinkery.bg_at(name_x, alpha), HOV);
    // On the active file's own row it changes nothing.
    tinkery.move_to(name_x, alpha);
    light(&mut tinkery, name_x, beta, surface);
    assert_eq!(tinkery.bg_at(name_x, alpha), HOV);
    assert!(tinkery.bold_at(name_x, alpha));
}

#[test]
fn another_tab_lights_under_the_pointer_and_the_current_one_does_not() {
    let dir = project();
    let mut tinkery = on_file(&dir);
    tinkery.send_keys("ctrl+n");
    let untitled = row_with(&tinkery, 1, "untitled");
    assert_eq!(untitled, 1);
    let (other, current) = (
        col_of(&tinkery, 1, "alpha.txt"),
        col_of(&tinkery, 1, "untitled"),
    );
    tinkery.wait_for_bg(current, 1, ACCENT, WAIT);
    light(&mut tinkery, other, 1, RAISED);
    // The current tab is still the new one: hovering picked nothing.
    assert_eq!(tinkery.bg_at(current, 1), ACCENT);
    // Onto the current tab: it stays the accent, and the other rests.
    tinkery.move_to(current, 1);
    tinkery.wait_for_bg(other, 1, RAISED, WAIT);
    assert_eq!(tinkery.bg_at(current, 1), ACCENT);
    light(&mut tinkery, other, 1, RAISED);
    leave(&mut tinkery, other, 1, RAISED);
}

#[test]
fn a_confirm_cards_waiting_button_lights_and_its_filled_one_does_not() {
    let dir = project();
    let mut tinkery = on_file(&dir);
    tinkery.type_text("x");
    tinkery.wait_for_text("xalpha", WAIT);
    tinkery.send_keys("ctrl+w");
    let buttons = row_with(&tinkery, 3, "Discard");
    let (save, discard) = (
        col_of(&tinkery, buttons, "Save"),
        col_of(&tinkery, buttons, "Discard"),
    );
    tinkery.wait_for_bg(save, buttons, ACCENT, WAIT);
    light(&mut tinkery, discard, buttons, BTN);
    // The focus is still on Save, and the card is still asking.
    assert_eq!(tinkery.bg_at(save, buttons), ACCENT);
    assert!(tinkery.screen().join("\n").contains("has unsaved changes"));
    // On the focused button nothing changes, and the other rests again.
    tinkery.move_to(save, buttons);
    tinkery.wait_for_bg(discard, buttons, BTN, WAIT);
    assert_eq!(tinkery.bg_at(save, buttons), ACCENT);
    // Enter still presses the focused button, not the one last pointed at.
    tinkery.move_to(discard, buttons);
    tinkery.wait_for_bg(discard, buttons, HOV, WAIT);
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone("has unsaved changes", WAIT);
    tinkery.wait_for_files("alpha.txt saved", WAIT, || {
        fs::read_to_string(dir.path().join("alpha.txt")).is_ok_and(|text| text.starts_with('x'))
    });
}

#[test]
fn with_a_dialog_open_nothing_behind_it_lights() {
    let dir = project();
    let mut tinkery = on_file(&dir);
    tinkery.send_keys("ctrl+n");
    tinkery.wait_for_text("untitled", WAIT);
    let other = col_of(&tinkery, 1, "alpha.txt");
    light(&mut tinkery, other, 1, RAISED);

    // The card opens with the pointer still on the tab: the tab is drawn at
    // rest and dimmed with everything else behind the card.
    tinkery.type_text("x");
    tinkery.send_keys("ctrl+w");
    let buttons = row_with(&tinkery, 3, "Discard");
    tinkery.wait_for_screen("the tab behind the card", WAIT, |screen| {
        screen[1].contains("alpha.txt")
    });
    let dimmed = tinkery.bg_at(other, 1);
    assert_ne!(dimmed, HOV);
    assert_ne!(dimmed, RAISED);
    // Moving over the tab lights nothing; the move onto a button, sent after
    // it, has been drawn by the time the tab is read.
    let discard = col_of(&tinkery, buttons, "Discard");
    tinkery.move_to(other + 2, 1);
    light(&mut tinkery, discard, buttons, BTN);
    assert_eq!(tinkery.bg_at(other, 1), dimmed);
    assert_eq!(tinkery.bg_at(other + 2, 1), dimmed);
    tinkery.move_to(other, 1);
    tinkery.wait_for_bg(discard, buttons, BTN, WAIT);
    assert_eq!(tinkery.bg_at(other, 1), dimmed);

    // The card gone, the tab under the pointer lights again.
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone("has unsaved changes", WAIT);
    tinkery.wait_for_bg(other, 1, HOV, WAIT);
}

#[test]
fn the_cast_palette_lights_the_row_under_the_pointer_and_enter_opens_the_selection() {
    let dir = project();
    let mut tinkery = on_splash(&dir);
    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text("cast · files · commands", WAIT);
    tinkery.wait_for_text("gamma.txt", WAIT);
    let selected = hover_the_row_after_the_selection(&mut tinkery, 7);
    // Enter opens the selected file, not the one the pointer is on.
    let screen = tinkery.screen();
    let (chosen, pointed) = (
        &screen[usize::from(selected)],
        &screen[usize::from(selected) + 1],
    );
    let name = |line: &str| {
        ["alpha.txt", "beta.txt", "gamma.txt"]
            .into_iter()
            .find(|name| line.contains(name))
            .unwrap_or_else(|| panic!("no file on {line:?}"))
    };
    assert_ne!(name(chosen), name(pointed));
    let chosen = name(chosen);
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone("cast · files · commands", WAIT);
    tinkery.wait_for_screen("the selected file open", WAIT, |screen| {
        screen[usize::from(STATUS)].contains(chosen)
    });
}

#[test]
fn the_run_picker_lights_the_row_under_the_pointer() {
    let dir = project();
    let entries: String = (0..3)
        .map(|i| format!("[[run]]\nname = \"job{i}\"\ncommand = \"echo {i}\"\n"))
        .collect();
    fs::write(dir.path().join(".tinkery.toml"), entries).expect("write .tinkery.toml");
    let mut tinkery = on_splash(&dir);
    tinkery.send_keys("f5");
    tinkery.wait_for_text("3 of 3", WAIT);
    let selected = hover_the_row_after_the_selection(&mut tinkery, 7);
    let screen = tinkery.screen();
    assert!(
        screen[usize::from(selected)].contains("job0"),
        "{screen:#?}"
    );
    assert!(
        screen[usize::from(selected) + 1].contains("job1"),
        "{screen:#?}"
    );
}

#[test]
fn the_folder_browser_lights_the_row_under_the_pointer() {
    let dir = project();
    let mut tinkery = on_splash(&dir);
    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text("cast · files · commands", WAIT);
    tinkery.type_text(">open directory");
    tinkery.wait_for_text("cast · commands", WAIT);
    tinkery.wait_for_screen("Open directory listed", WAIT, |screen| {
        screen[9].contains("Open directory")
    });
    tinkery.send_keys("enter");
    tinkery.wait_for_text("open · folders", WAIT);
    tinkery.wait_for_text("▸ sub", WAIT);
    // The open row is selected; the folder under it is the pointer's.
    let selected = hover_the_row_after_the_selection(&mut tinkery, 7);
    let screen = tinkery.screen();
    assert!(
        screen[usize::from(selected)].contains("open"),
        "{screen:#?}"
    );
    assert!(
        screen[usize::from(selected) + 1].contains("▸ sub"),
        "{screen:#?}"
    );
}

#[test]
fn project_search_lights_the_hit_and_the_off_chip_under_the_pointer() {
    let dir = project();
    let mut tinkery = on_splash(&dir);
    tinkery.send_keys("alt+f");
    tinkery.wait_for_text("replace all", WAIT);
    tinkery.type_text("needle");
    tinkery.wait_for_text("Search   needle", WAIT);
    tinkery.send_keys("enter");
    tinkery.wait_for_text("beta.txt:2", WAIT);
    tinkery.wait_for_text("2 matches in 2 files", WAIT);
    let selected = hover_the_row_after_the_selection(&mut tinkery, 9);
    let screen = tinkery.screen();
    assert!(
        screen[usize::from(selected)].contains("alpha.txt:2"),
        "{screen:#?}"
    );

    // Both chips are off; the one under the pointer lights and stays off.
    let strip = row_with(&tinkery, 5, "Aa");
    let (case, regex) = (col_of(&tinkery, strip, "Aa"), col_of(&tinkery, strip, ".*"));
    light(&mut tinkery, case, strip, BTN);
    assert_eq!(tinkery.bg_at(regex, strip), BTN);
    tinkery.move_to(regex, strip);
    tinkery.wait_for_bg(regex, strip, HOV, WAIT);
    tinkery.wait_for_bg(case, strip, BTN, WAIT);
    // The same hits, found the same way: nothing was toggled.
    assert!(tinkery.screen().join("\n").contains("2 matches in 2 files"));
    // A chip that is on is the accent whatever is over it.
    tinkery.send_keys("alt+c");
    tinkery.wait_for_bg(case, strip, ACCENT, WAIT);
    tinkery.move_to(case, strip);
    tinkery.wait_for_bg(regex, strip, BTN, WAIT);
    assert_eq!(tinkery.bg_at(case, strip), ACCENT);
}

#[test]
fn the_find_bars_off_chip_lights_under_the_pointer() {
    let dir = project();
    let mut tinkery = on_file(&dir);
    tinkery.send_keys("ctrl+f");
    let bar = row_with(&tinkery, 3, "Aa");
    let (case, regex) = (col_of(&tinkery, bar, "Aa"), col_of(&tinkery, bar, ".*"));
    light(&mut tinkery, case, bar, BTN);
    assert_eq!(tinkery.bg_at(regex, bar), BTN);
    tinkery.move_to(regex, bar);
    tinkery.wait_for_bg(regex, bar, HOV, WAIT);
    tinkery.wait_for_bg(case, bar, BTN, WAIT);
    leave(&mut tinkery, regex, bar, BTN);
}

#[test]
fn the_catalog_lights_the_row_under_the_pointer() {
    let dir = project();
    let mut tinkery = on_splash(&dir);
    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text("cast · files · commands", WAIT);
    tinkery.type_text(">language servers");
    tinkery.wait_for_text("cast · commands", WAIT);
    tinkery.wait_for_screen("Language servers listed", WAIT, |screen| {
        screen[9].contains("Language servers")
    });
    tinkery.send_keys("enter");
    tinkery.wait_for_screen("the catalog", WAIT, |screen| {
        screen[6].contains("✦ language servers")
            && screen
                .iter()
                .any(|row| row.contains("copy command") && row.contains("close"))
    });
    let selected = hover_the_row_after_the_selection(&mut tinkery, 7);
    assert_eq!(selected, 8);
}

#[test]
fn the_keybindings_card_lights_the_row_under_the_pointer() {
    let dir = project();
    let mut tinkery = on_splash(&dir);
    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text("cast · files · commands", WAIT);
    tinkery.type_text(">keybindings");
    tinkery.wait_for_text("cast · commands", WAIT);
    tinkery.wait_for_screen("Keybindings listed", WAIT, |screen| {
        screen[9].contains("Keybindings")
    });
    tinkery.send_keys("enter");
    tinkery.wait_for_screen("the keybindings card", WAIT, |screen| {
        screen[4].contains("Keybindings")
            && screen
                .iter()
                .any(|row| row.contains("rebind") && row.contains("close"))
    });
    let selected = hover_the_row_after_the_selection(&mut tinkery, 7);
    assert_eq!(selected, 8);
}

#[test]
fn a_splash_action_lights_under_the_pointer_and_the_selected_one_does_not() {
    let dir = project();
    let mut tinkery = on_splash(&dir);
    let new_file = row_with(&tinkery, 3, "New file");
    let new_dir = row_with(&tinkery, 3, "New directory");
    let (selected, other) = (
        col_of(&tinkery, new_file, "New file"),
        col_of(&tinkery, new_dir, "New directory"),
    );
    tinkery.wait_for_bg(selected, new_file, ACCENT, WAIT);
    light(&mut tinkery, other, new_dir, BTN);
    // The whole row answers a click, so the whole row lights its button.
    tinkery.move_to(other + 30, new_dir);
    tinkery.move_to(other + 31, new_dir);
    assert_eq!(tinkery.bg_at(selected, new_file), ACCENT);
    // On the selected action nothing changes, and the other rests again.
    tinkery.move_to(selected, new_file);
    tinkery.wait_for_bg(other, new_dir, BTN, WAIT);
    assert_eq!(tinkery.bg_at(selected, new_file), ACCENT);
    // Enter still runs the selected action: a prompt for a new file's name.
    tinkery.move_to(other, new_dir);
    tinkery.wait_for_bg(other, new_dir, HOV, WAIT);
    tinkery.send_keys("enter");
    tinkery.wait_for_screen("the new file prompt", WAIT, |screen| {
        screen[usize::from(STATUS) - 1].contains("New file")
            && screen[usize::from(STATUS) - 1].contains("create")
    });
}
