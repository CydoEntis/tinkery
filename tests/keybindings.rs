//! `>keybindings`: the card listing every command with its keys and config
//! name, searchable, in the catalog card's style (tinkery-catalog spec C2).

mod harness;

use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::time::Duration;

use harness::{CAP_L, CAP_R, ROWS, Tinkery, buttons_text, hints_text};
use vt100::Color;

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(10);

/// At 100x30 the card is a panel the size of the catalog's: 86 wide from
/// column 7, dropping from row 4, its title in the top edge, down to a row
/// short of the screen's bottom. The header's strip, which holds the query, is
/// row 6, row 7 is blank and the list is rows 8 to 25, the text four cells in;
/// the config names start 35 cells after the titles and the keys end four
/// cells in from the card's right edge. A blank, the keys on row 27 and the
/// bottom edge. A row's pill runs the card's inside, from column 8 to column
/// 91, with `›` a cell into it.
const CARD_X: u16 = 7;
const CARD_Y: u16 = 4;
const CARD_WIDTH: u16 = 86;
const HEADER_ROW: u16 = 6;
const BLANK_ROW: u16 = 7;
const FIRST_ROW: u16 = 8;
const LAST_ROW: u16 = 25;
const PILL_X: u16 = CARD_X + 1;
const PILL_END: u16 = CARD_X + CARD_WIDTH - 2;
const MARK_X: u16 = CARD_X + 2;
const TEXT_X: u16 = 11;
const NAME_X: u16 = 46;
const RIGHT: u16 = 89;
const FOOTER_ROW: u16 = 27;
const TITLE: &str = "▁ Keybindings ▁";

/// The default theme, tinkery, which an empty config runs on: the roles the
/// card is drawn in.
const DEFAULT: &str = "";
const ACCENT: Color = Color::Rgb(0xff, 0x8a, 0x3d);
const ACCENT2: Color = Color::Rgb(0xff, 0xd2, 0x7a);
const RAISED: Color = Color::Rgb(0x1f, 0x1b, 0x19);
const RAISED2: Color = Color::Rgb(0x29, 0x23, 0x20);
const HOV: Color = Color::Rgb(0x3a, 0x30, 0x2a);
const BTN: Color = Color::Rgb(0x33, 0x2a, 0x25);
const MUTED: Color = Color::Rgb(0x7d, 0x70, 0x67);
const TEXT: Color = Color::Rgb(0xb5, 0xa8, 0x9d);
const STRONG: Color = Color::Rgb(0xf8, 0xf0, 0xe6);

/// The keys on the card's last row, as keycaps with round ends. Delete, not
/// Backspace, resets: Backspace keeps correcting the query.
fn hints() -> String {
    hints_text(&[("⏎", "rebind"), ("Del", "reset"), ("Esc", "close")])
}

/// `key` as a row shows it: a keycap with round ends.
fn cap(key: &str) -> String {
    format!("{CAP_L}{key}{CAP_R}")
}

/// The cell at (`col`, `y`), as text.
fn cell(tinkery: &Tinkery, col: u16, y: u16) -> String {
    row(tinkery, y)
        .chars()
        .nth(usize::from(col))
        .map(String::from)
        .unwrap_or_default()
}

/// What row `y` holds inside the card's two edges, without the blanks around
/// it.
fn inside(tinkery: &Tinkery, y: u16) -> String {
    row(tinkery, y)
        .chars()
        .skip(usize::from(PILL_X))
        .take(usize::from(CARD_WIDTH - 2))
        .collect::<String>()
        .trim()
        .to_string()
}

/// What the header's strip holds on `screen` after its `✦`: the query.
fn query(screen: &[String]) -> String {
    screen[usize::from(HEADER_ROW)]
        .chars()
        .skip(usize::from(TEXT_X + 2))
        .take(usize::from(RIGHT - TEXT_X - 2))
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// Waits for row `y` to be the selected row and asserts it's drawn as one: a
/// pill of `hov` on the card's `raised` across its inside, `›` in the accent a
/// cell into it, and the title in `strong` bold.
fn assert_selected(tinkery: &Tinkery, y: u16) {
    tinkery.wait_for_bg(MARK_X, y, HOV, WAIT);
    assert_eq!(cell(tinkery, MARK_X, y), "›");
    assert_eq!(tinkery.fg_at(MARK_X, y), ACCENT);
    for (x, end) in [(PILL_X, CAP_L), (PILL_END, CAP_R)] {
        assert_eq!(cell(tinkery, x, y), end, "{x}");
        assert_eq!(tinkery.fg_at(x, y), HOV, "{x}");
        assert_eq!(tinkery.bg_at(x, y), RAISED, "{x}");
    }
    assert_eq!(tinkery.fg_at(TEXT_X, y), STRONG);
    assert!(tinkery.bold_at(TEXT_X, y));
}

/// Asserts row `y` isn't the selected row: no pill and no mark.
fn assert_not_selected(tinkery: &Tinkery, y: u16) {
    assert_eq!(cell(tinkery, MARK_X, y), " ");
    for x in [PILL_X, MARK_X, TEXT_X, PILL_END] {
        assert_eq!(tinkery.bg_at(x, y), RAISED, "{x}");
    }
}

fn row(tinkery: &Tinkery, row: u16) -> String {
    tinkery.screen()[usize::from(row)].clone()
}

/// Tinkery on an empty project with `config`.
fn tinkery_with(config: &str) -> Tinkery {
    let project = tempfile::tempdir().expect("create project");
    let tinkery = Tinkery::spawn_in_with_config(project.path(), config, &["."]);
    tinkery.wait_for_text("Open directory", START);
    tinkery
}

/// Ctrl+P, `>keybindings`, Enter.
fn open_card(tinkery: &mut Tinkery) {
    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text("cast · files · commands", WAIT);
    tinkery.type_text(">keybindings");
    tinkery.wait_for_text("cast · commands", WAIT);
    tinkery.wait_for_screen("Keybindings listed", WAIT, |screen| {
        // The cast's first command row, under its `COMMANDS` label.
        screen[9].contains("Keybindings")
    });
    tinkery.send_keys("enter");
    // The title can arrive before the rest of the card (ConPTY streams the
    // redraw in pieces), so wait for the keys on its last row too.
    tinkery.wait_for_screen("the keybindings card", WAIT, |screen| {
        screen[usize::from(CARD_Y)].contains(TITLE)
            && screen[usize::from(FOOTER_ROW)].contains("rebind")
            && screen[usize::from(FOOTER_ROW)].contains("close")
    });
}

/// Column where `text` starts when it stands where a row's keys go, ending
/// at `RIGHT`: the dash of a command with no key, or the wait for one.
fn plain_x(text: &str) -> u16 {
    RIGHT - u16::try_from(text.chars().count()).expect("short text")
}

/// Column where `key` starts on a row whose last keycap it is: the cap's
/// round end is the cell before `RIGHT`.
fn keys_x(key: &str) -> u16 {
    plain_x(key) - 1
}

/// Whether screen row `y` holds `text` from the list's text column.
fn starts(screen: &[String], y: u16, text: &str) -> bool {
    screen[usize::from(y)]
        .chars()
        .skip(usize::from(TEXT_X))
        .collect::<String>()
        .starts_with(text)
}

/// Types `query` and waits for row `y` to be `title`.
fn filter_at(tinkery: &mut Tinkery, query: &str, y: u16, title: &str) {
    tinkery.type_text(query);
    tinkery.wait_for_screen(&format!("{query} filtered"), WAIT, |screen| {
        starts(screen, y, title)
    });
}

/// Types `query` and waits for the first row to be `title`.
fn filter(tinkery: &mut Tinkery, query: &str, title: &str) {
    filter_at(tinkery, query, FIRST_ROW, title);
}

/// Deletes `n` letters of the query.
fn erase(tinkery: &mut Tinkery, n: usize) {
    for _ in 0..n {
        tinkery.send_keys("backspace");
    }
}

#[test]
fn keybindings_in_the_cast_lists_every_command_with_its_keys_and_name() {
    let mut tinkery = tinkery_with(DEFAULT);
    open_card(&mut tinkery);

    // A panel: the title set into its accent edge, then the header's strip
    // with `✦` and the query, still empty, and a blank row.
    assert_eq!(tinkery.text_col(CARD_Y, TITLE), Some(CARD_X + 1));
    assert_eq!(tinkery.fg_at(CARD_X + 1, CARD_Y), ACCENT);
    assert!(tinkery.bold_at(CARD_X + 3, CARD_Y));
    assert_eq!(cell(&tinkery, CARD_X, HEADER_ROW), "▕");
    assert_eq!(cell(&tinkery, CARD_X + CARD_WIDTH - 1, HEADER_ROW), "▏");
    assert_eq!(cell(&tinkery, PILL_X, HEADER_ROW), CAP_L);
    assert_eq!(cell(&tinkery, PILL_END, HEADER_ROW), CAP_R);
    assert_eq!(tinkery.fg_at(PILL_X, HEADER_ROW), RAISED2);
    assert_eq!(cell(&tinkery, TEXT_X, HEADER_ROW), "✦");
    assert_eq!(tinkery.fg_at(TEXT_X, HEADER_ROW), ACCENT2);
    assert_eq!(tinkery.bg_at(TEXT_X, HEADER_ROW), RAISED2);
    assert_eq!(query(&tinkery.screen()), "");
    assert_eq!(tinkery.cursor(), (TEXT_X + 2, HEADER_ROW));
    assert_eq!(inside(&tinkery, BLANK_ROW), "");
    assert_eq!(
        row(&tinkery, FOOTER_ROW + 1)
            .chars()
            .skip(usize::from(PILL_X))
            .take(usize::from(CARD_WIDTH - 2))
            .collect::<String>(),
        "▔".repeat(usize::from(CARD_WIDTH - 2))
    );

    for (i, (title, name, keys)) in [("Quit", "quit", "Ctrl+Q"), ("Save", "save", "Ctrl+S")]
        .into_iter()
        .enumerate()
    {
        let y = FIRST_ROW + u16::try_from(i).expect("two rows");
        assert_eq!(tinkery.text_col(y, title), Some(TEXT_X), "{title}");
        assert_eq!(tinkery.text_col(y, name), Some(NAME_X), "{title}");
        // Its key is a keycap ending four cells in from the card's edge.
        assert_eq!(tinkery.text_col(y, &cap(keys)), Some(keys_x(keys) - 1));
    }
    // The config name is `muted`, apart from the title and the keys; a key
    // is `strong` bold on `btn`, its round ends `btn` on the card.
    let save = FIRST_ROW + 1;
    assert_not_selected(&tinkery, save);
    let title = tinkery.fg_at(TEXT_X, save);
    assert_eq!(tinkery.fg_at(NAME_X, save), MUTED);
    assert_ne!(title, MUTED);
    assert_eq!(tinkery.bg_text(save, BTN), "Ctrl+S");
    assert_eq!(tinkery.fg_at(keys_x("Ctrl+S"), save), STRONG);
    assert!(tinkery.bold_at(keys_x("Ctrl+S"), save));
    for x in [keys_x("Ctrl+S") - 1, RIGHT - 1] {
        assert_eq!(tinkery.fg_at(x, save), BTN, "{x}");
        assert_eq!(tinkery.bg_at(x, save), RAISED, "{x}");
    }
    // The first row is the selected one: its keycap sits on the pill, and
    // its name is in `text`, which reads there where `muted` wouldn't.
    assert_selected(&tinkery, FIRST_ROW);
    assert_eq!(tinkery.bg_text(FIRST_ROW, BTN), "Ctrl+Q");
    for x in [keys_x("Ctrl+Q") - 1, RIGHT - 1] {
        assert_eq!(tinkery.fg_at(x, FIRST_ROW), BTN, "{x}");
        assert_eq!(tinkery.bg_at(x, FIRST_ROW), HOV, "{x}");
    }
    assert_eq!(tinkery.fg_at(NAME_X, FIRST_ROW), TEXT);

    // A blank, then the keys as keycaps.
    assert_eq!(inside(&tinkery, FOOTER_ROW - 1), "");
    assert_eq!(inside(&tinkery, FOOTER_ROW), hints());
    assert_eq!(tinkery.text_col(FOOTER_ROW, &hints()), Some(TEXT_X));
    assert_eq!(tinkery.bg_text(FOOTER_ROW, BTN), "⏎DelEsc");
    assert_eq!(tinkery.fg_at(TEXT_X + 4, FOOTER_ROW), MUTED);

    // A command with no key shows a dash, which is no keycap.
    filter(&mut tinkery, "keybindings", "Keybindings");
    assert_eq!(tinkery.text_col(FIRST_ROW, "keybindings"), Some(NAME_X));
    assert_eq!(tinkery.text_col(FIRST_ROW, "—"), Some(plain_x("—")));
    assert_eq!(tinkery.bg_text(FIRST_ROW, BTN), "");
    assert_eq!(tinkery.fg_at(plain_x("—"), FIRST_ROW), TEXT);
}

#[test]
fn scope_only_commands_come_under_a_muted_heading() {
    let mut tinkery = tinkery_with(DEFAULT);
    open_card(&mut tinkery);
    filter(&mut tinkery, "tree", "Toggle file tree");

    // The commands that work anywhere, a blank, then the tree's own.
    assert_eq!(
        tinkery.text_col(FIRST_ROW + 1, "Focus file tree"),
        Some(TEXT_X)
    );
    assert_eq!(inside(&tinkery, FIRST_ROW + 2), "");
    assert_eq!(inside(&tinkery, FIRST_ROW + 3), "file tree");
    assert_eq!(tinkery.fg_at(TEXT_X, FIRST_ROW + 3), MUTED);
    assert_eq!(tinkery.text_col(FIRST_ROW + 3, "file tree"), Some(TEXT_X));
    assert!(tinkery.bold_at(TEXT_X, FIRST_ROW + 3));
    assert_ne!(
        tinkery.fg_at(TEXT_X, FIRST_ROW + 3),
        tinkery.fg_at(TEXT_X, FIRST_ROW + 4)
    );
    assert_eq!(
        tinkery.text_col(FIRST_ROW + 4, "New file in tree"),
        Some(TEXT_X)
    );
    assert_eq!(
        tinkery.text_col(FIRST_ROW + 5, &cap("Shift+A")),
        Some(keys_x("Shift+A") - 1)
    );

    // The splash's and the debug panel's under theirs, with nothing above.
    erase(&mut tinkery, 4);
    filter_at(&mut tinkery, "splash: quit", FIRST_ROW + 1, "Splash: quit");
    assert_eq!(inside(&tinkery, FIRST_ROW), "splash");
    erase(&mut tinkery, 12);
    filter_at(&mut tinkery, "debug_up", FIRST_ROW + 1, "Debug panel: up");
    assert_eq!(inside(&tinkery, FIRST_ROW), "debug panel");
}

#[test]
fn typing_filters_on_title_key_or_config_name() {
    let mut tinkery = tinkery_with(DEFAULT);
    open_card(&mut tinkery);
    // The header's strip holds what's typed, in `strong`, the cursor after it.
    filter(&mut tinkery, "save as", "Save as…");
    assert_eq!(query(&tinkery.screen()), "save as");
    assert_eq!(tinkery.text_col(HEADER_ROW, "✦ save as"), Some(TEXT_X));
    assert_eq!(tinkery.fg_at(TEXT_X + 2, HEADER_ROW), STRONG);
    assert_eq!(tinkery.bg_at(TEXT_X + 2, HEADER_ROW), RAISED2);
    tinkery.wait_for_cursor(TEXT_X + 2 + 7, HEADER_ROW, WAIT);
    assert_eq!(inside(&tinkery, FIRST_ROW + 1), "");
    erase(&mut tinkery, 7);
    filter(&mut tinkery, "f12", "Go to definition");
    erase(&mut tinkery, 3);
    filter(&mut tinkery, "go_to_line", "Go to line…");
    assert_eq!(
        tinkery.text_col(FIRST_ROW, &cap("Ctrl+G")),
        Some(keys_x("Ctrl+G") - 1)
    );
    // Nothing matching says so, in `muted` on the card.
    tinkery.type_text("zzz");
    tinkery.wait_for_text("no matching commands", WAIT);
    assert_eq!(inside(&tinkery, FIRST_ROW), "no matching commands");
    assert_eq!(tinkery.fg_at(TEXT_X, FIRST_ROW), MUTED);
    assert_eq!(tinkery.bg_at(TEXT_X, FIRST_ROW), RAISED);
}

#[test]
fn arrows_move_the_selection_and_esc_or_a_click_outside_close() {
    let mut tinkery = tinkery_with(DEFAULT);
    open_card(&mut tinkery);

    // The first row starts selected: a pill with its mark.
    assert_selected(&tinkery, FIRST_ROW);
    assert_not_selected(&tinkery, FIRST_ROW + 1);
    tinkery.send_keys("down");
    assert_selected(&tinkery, FIRST_ROW + 1);
    assert_not_selected(&tinkery, FIRST_ROW);
    tinkery.send_keys("up");
    tinkery.send_keys("up");
    assert_selected(&tinkery, FIRST_ROW);

    // Past the bottom the list scrolls, keeping the selection in view.
    for _ in 0..20 {
        tinkery.send_keys("down");
    }
    tinkery.wait_for_screen("the list scrolled", WAIT, |screen| {
        !screen[usize::from(FIRST_ROW)].contains("Quit")
    });
    // The selection is on the list's last row, a blank and the keys under it.
    assert_selected(&tinkery, LAST_ROW);
    assert_eq!(inside(&tinkery, LAST_ROW + 1), "");
    assert_eq!(inside(&tinkery, FOOTER_ROW), hints());

    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(TITLE, WAIT);

    // A click outside the card closes it too; one inside doesn't.
    open_card(&mut tinkery);
    tinkery.click(CARD_X + 10, FIRST_ROW + 2);
    // Events are handled in order, so the card filtering what's typed after
    // the click shows the click left it open.
    filter(&mut tinkery, "save as", "Save as…");
    assert_eq!(tinkery.text_col(CARD_Y, TITLE), Some(CARD_X + 1));
    tinkery.click(2, ROWS - 3);
    tinkery.wait_for_text_gone(TITLE, WAIT);
}

#[test]
fn a_paste_goes_into_the_filter() {
    let mut tinkery = tinkery_with(DEFAULT);
    open_card(&mut tinkery);
    // A bracketed paste with a line break, which the filter leaves out.
    tinkery.write(b"\x1b[200~toggle tree\r\x1b[201~");
    tinkery.wait_for_screen("the paste filtered", WAIT, |screen| {
        starts(screen, FIRST_ROW, "Toggle file tree")
    });
    assert_eq!(query(&tinkery.screen()), "toggle tree");
    assert_eq!(inside(&tinkery, FIRST_ROW + 1), "");
}

/// What the selected row shows while it waits for a new key.
const WAITING: &str = "press a key… (Esc cancels)";

/// Tinkery on `a.txt` (`alpha`) in `project`, with `TINKERY_CONFIG` pointing at
/// `config`.
fn tinkery_on_file(project: &Path, config: &Path) -> Tinkery {
    fs::write(project.join("a.txt"), "alpha\n").expect("write a.txt");
    let env = [("TINKERY_CONFIG", OsString::from(config))];
    let tinkery = Tinkery::spawn_in_with_env(project, &env, &["a.txt"]);
    tinkery.wait_for_text("alpha", START);
    tinkery
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

#[test]
fn enter_then_a_key_rebinds_save_and_the_new_key_saves() {
    let project = tempfile::tempdir().expect("create project");
    let home = tempfile::tempdir().expect("create config home");
    let config = home.path().join("config.toml");
    fs::write(&config, "# mine\ntheme = \"hydra\"\n").expect("write config");
    let mut tinkery = tinkery_on_file(project.path(), &config);
    open_card(&mut tinkery);
    filter(&mut tinkery, "save", "Save");

    tinkery.send_keys("enter");
    tinkery.wait_for_text(WAITING, WAIT);
    assert_eq!(tinkery.text_col(FIRST_ROW, WAITING), Some(plain_x(WAITING)));
    tinkery.send_keys("alt+w");
    tinkery.wait_for_text("save bound to Alt+W", WAIT);
    tinkery.wait_for_screen("the row shows Alt+W", WAIT, |screen| {
        screen[usize::from(FIRST_ROW)].contains(&cap("Alt+W"))
    });
    assert_eq!(
        tinkery.text_col(FIRST_ROW, &cap("Alt+W")),
        Some(keys_x("Alt+W") - 1)
    );
    assert_eq!(
        read(&config),
        "# mine\ntheme = \"hydra\"\n\n[keys]\nsave = \"alt+w\"\n"
    );

    // Applied at once: Alt+W saves a.txt, with no restart.
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(TITLE, WAIT);
    tinkery.type_text("x");
    tinkery.wait_for_text("xalpha", WAIT);
    tinkery.send_keys("alt+w");
    let file = project.path().join("a.txt");
    tinkery.wait_for_files("a.txt saved by Alt+W", WAIT, || read(&file) == "xalpha\n");
}

#[test]
fn delete_on_a_row_brings_back_its_default_and_backspace_edits_the_query() {
    let project = tempfile::tempdir().expect("create project");
    let home = tempfile::tempdir().expect("create config home");
    let config = home.path().join("config.toml");
    fs::write(&config, "[keys]\nsave = \"alt+w\"\nclose_tab = \"alt+q\"\n").expect("write config");
    let mut tinkery = tinkery_on_file(project.path(), &config);
    open_card(&mut tinkery);
    filter(&mut tinkery, "save", "Save");
    assert_eq!(
        tinkery.text_col(FIRST_ROW, &cap("Alt+W")),
        Some(keys_x("Alt+W") - 1)
    );

    tinkery.send_keys("delete");
    tinkery.wait_for_text("save reset to its default keys", WAIT);
    tinkery.wait_for_screen("the row shows Ctrl+S", WAIT, |screen| {
        screen[usize::from(FIRST_ROW)].contains(&cap("Ctrl+S"))
    });
    assert_eq!(read(&config), "[keys]\nclose_tab = \"alt+q\"\n");

    // Backspace still corrects the query rather than resetting anything.
    tinkery.send_keys("backspace");
    tinkery.wait_for_screen("the query lost its e", WAIT, |screen| {
        query(screen) == "sav"
    });
    assert_eq!(read(&config), "[keys]\nclose_tab = \"alt+q\"\n");

    // Ctrl+S saves again.
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(TITLE, WAIT);
    tinkery.type_text("y");
    tinkery.wait_for_text("yalpha", WAIT);
    tinkery.send_keys("ctrl+s");
    let file = project.path().join("a.txt");
    tinkery.wait_for_files("a.txt saved by Ctrl+S", WAIT, || read(&file) == "yalpha\n");
}

#[test]
fn an_open_config_tab_shows_the_rebind_and_saving_it_keeps_it() {
    let project = tempfile::tempdir().expect("create project");
    let home = tempfile::tempdir().expect("create config home");
    let config = home.path().join("config.toml");
    let mut tinkery = tinkery_on_file(project.path(), &config);
    // `>settings` opens config.toml, written from the template.
    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text("cast · files · commands", WAIT);
    tinkery.type_text(">settings");
    tinkery.wait_for_screen("Settings listed", WAIT, |screen| {
        screen[9].contains("Settings")
    });
    tinkery.send_keys("enter");
    tinkery.wait_for_text("# tab_width = 4", WAIT);

    open_card(&mut tinkery);
    filter(&mut tinkery, "save", "Save");
    tinkery.send_keys("enter");
    tinkery.wait_for_text(WAITING, WAIT);
    tinkery.send_keys("alt+w");
    tinkery.wait_for_text("save bound to Alt+W", WAIT);
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(TITLE, WAIT);
    // The tab shows the new entry, and saving it with the new key keeps it.
    tinkery.wait_for_text("save = \"alt+w\"", WAIT);
    tinkery.send_keys("alt+w");
    tinkery.wait_for_text("settings applied", WAIT);
    // A stale tab would have written the template back, Ctrl+S with it.
    assert!(read(&config).contains("[keys]\nsave = \"alt+w\"\n"));
}

#[test]
fn esc_while_waiting_cancels_and_writes_nothing() {
    let project = tempfile::tempdir().expect("create project");
    let home = tempfile::tempdir().expect("create config home");
    let config = home.path().join("config.toml");
    let mut tinkery = tinkery_on_file(project.path(), &config);
    open_card(&mut tinkery);
    filter(&mut tinkery, "save", "Save");

    tinkery.send_keys("enter");
    tinkery.wait_for_text(WAITING, WAIT);
    // Not a keycap: in the accent, on the selected row's pill.
    assert_eq!(tinkery.text_col(FIRST_ROW, WAITING), Some(plain_x(WAITING)));
    // The panel's edge and the row's mark are the accent's too.
    assert_eq!(tinkery.fg_text(FIRST_ROW, ACCENT), format!("▕›{WAITING}▏"));
    assert_eq!(tinkery.bg_at(plain_x(WAITING), FIRST_ROW), HOV);
    assert_eq!(tinkery.bg_text(FIRST_ROW, BTN), "");
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(WAITING, WAIT);
    // The card is still open, the row as it was.
    assert_eq!(tinkery.text_col(CARD_Y, TITLE), Some(CARD_X + 1));
    assert_eq!(
        tinkery.text_col(FIRST_ROW, &cap("Ctrl+S")),
        Some(keys_x("Ctrl+S") - 1)
    );
    assert!(!config.exists());
    // A second Esc closes it.
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(TITLE, WAIT);
    assert!(!config.exists());
}

/// What the move prompt asks when Ctrl+K, Hover's key, is pressed for Save.
const MOVE_CTRL_K: &str = "Ctrl+K is used by Show hover — move it here?";

/// Tinkery on a.txt with Hover on Ctrl+K, the card filtered to Save, Enter and
/// Ctrl+K pressed, and the move prompt showing.
fn ask_to_move_ctrl_k(project: &Path, config: &Path) -> Tinkery {
    fs::write(config, "[keys]\nhover = \"ctrl+k\"\n").expect("write config");
    let mut tinkery = tinkery_on_file(project, config);
    open_card(&mut tinkery);
    filter(&mut tinkery, "save", "Save");
    tinkery.send_keys("enter");
    tinkery.wait_for_text(WAITING, WAIT);
    tinkery.send_keys("ctrl+k");
    tinkery.wait_for_text(MOVE_CTRL_K, WAIT);
    tinkery.wait_for_text("Show hover is left with no key.", WAIT);
    // The card is a panel 52 wide (the 44-cell question + 8) from column 24
    // and row 10: Yes first and focused, in tinkery's accent, and No, which
    // is what Esc answers, on its btn.
    let screen = tinkery.screen();
    assert_eq!(tinkery.text_col(12, MOVE_CTRL_K), Some(27), "{screen:#?}");
    assert_eq!(
        tinkery.text_col(16, &buttons_text(&[("Yes", "Y"), ("No", "Esc")])),
        Some(27),
        "{screen:#?}"
    );
    assert_eq!(tinkery.bg_text(16, Color::Rgb(0xff, 0x8a, 0x3d)), "Yes Y");
    assert_eq!(tinkery.bg_text(16, Color::Rgb(0x33, 0x2a, 0x25)), "No Esc");
    tinkery
}

#[test]
fn yes_moves_a_key_another_command_has() {
    let project = tempfile::tempdir().expect("create project");
    let home = tempfile::tempdir().expect("create config home");
    let config = home.path().join("config.toml");
    let mut tinkery = ask_to_move_ctrl_k(project.path(), &config);
    // Nothing is written until it's answered.
    assert_eq!(read(&config), "[keys]\nhover = \"ctrl+k\"\n");

    tinkery.type_text("y");
    tinkery.wait_for_text("Ctrl+K moved from hover to save", WAIT);
    tinkery.wait_for_text_gone(MOVE_CTRL_K, WAIT);
    assert_eq!(read(&config), "[keys]\nhover = []\nsave = \"ctrl+k\"\n");
    // Save's row shows its new key; Hover's has none.
    tinkery.wait_for_screen("Save shows Ctrl+K", WAIT, |screen| {
        screen[usize::from(FIRST_ROW)].contains(&cap("Ctrl+K"))
    });
    erase(&mut tinkery, 4);
    filter(&mut tinkery, "hover", "Show hover");
    assert_eq!(tinkery.text_col(FIRST_ROW, "—"), Some(plain_x("—")));

    // Ctrl+K saves now.
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(TITLE, WAIT);
    tinkery.type_text("x");
    tinkery.wait_for_text("xalpha", WAIT);
    tinkery.send_keys("ctrl+k");
    let file = project.path().join("a.txt");
    tinkery.wait_for_files("a.txt saved by Ctrl+K", WAIT, || read(&file) == "xalpha\n");
}

#[test]
fn no_or_esc_on_the_move_prompt_writes_nothing() {
    let project = tempfile::tempdir().expect("create project");
    let home = tempfile::tempdir().expect("create config home");
    let config = home.path().join("config.toml");
    let mut tinkery = ask_to_move_ctrl_k(project.path(), &config);
    tinkery.type_text("n");
    tinkery.wait_for_text_gone(MOVE_CTRL_K, WAIT);
    // Back on the card, Save's row as it was.
    assert_eq!(tinkery.text_col(CARD_Y, TITLE), Some(CARD_X + 1));
    assert_eq!(
        tinkery.text_col(FIRST_ROW, &cap("Ctrl+S")),
        Some(keys_x("Ctrl+S") - 1)
    );
    assert_eq!(read(&config), "[keys]\nhover = \"ctrl+k\"\n");

    tinkery.send_keys("enter");
    tinkery.wait_for_text(WAITING, WAIT);
    tinkery.send_keys("ctrl+k");
    tinkery.wait_for_text(MOVE_CTRL_K, WAIT);
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(MOVE_CTRL_K, WAIT);
    assert_eq!(tinkery.text_col(CARD_Y, TITLE), Some(CARD_X + 1));
    assert_eq!(read(&config), "[keys]\nhover = \"ctrl+k\"\n");
}

#[test]
fn mono_reverses_the_selected_row_and_its_keycaps_are_bold_without_a_fill() {
    let mut tinkery = tinkery_with("theme = \"mono\"\n");
    open_card(&mut tinkery);
    // The card's whole inside, the mark a cell in and the key, between the
    // blanks its cap's ends are, four cells in from the card's right edge.
    let quit = format!(" › {:<35}{:<35} Ctrl+Q    ", "Quit", "quit");
    tinkery.wait_for_reversed(FIRST_ROW, &quit, WAIT);
    // No Nerd Font glyph in `mono`: a pill's ends are blanks.
    let screen = tinkery.screen().join("\n");
    assert!(
        !screen.contains(CAP_L) && !screen.contains(CAP_R),
        "{screen}"
    );
    // On the selected row the title and the key are bold, and nothing has a
    // colour or a fill of its own to break the reverse video up.
    assert_eq!(tinkery.bold_text(FIRST_ROW), "▕›QuitCtrl+Q▏");
    let fill = tinkery.bg_at(TEXT_X, FIRST_ROW);
    let ink = tinkery.fg_at(TEXT_X, FIRST_ROW);
    for x in PILL_X..=PILL_END {
        assert_eq!(tinkery.bg_at(x, FIRST_ROW), fill, "{x}");
        assert_eq!(tinkery.fg_at(x, FIRST_ROW), ink, "{x}");
    }
    // Any other row's key is a bold keycap, and so are the hints' keys.
    assert_eq!(tinkery.bold_text(FIRST_ROW + 1), "▕Ctrl+S▏");
    assert_eq!(
        inside(&tinkery, FOOTER_ROW),
        "⏎  rebind    Del  reset    Esc  close"
    );
    assert_eq!(tinkery.bold_text(FOOTER_ROW), "▕⏎DelEsc▏");

    tinkery.send_keys("down");
    let save = format!(" › {:<35}{:<35} Ctrl+S    ", "Save", "save");
    tinkery.wait_for_reversed(FIRST_ROW + 1, &save, WAIT);
    assert_eq!(tinkery.reversed_text(FIRST_ROW), "");

    // Waiting for a key, the row asks for one in the same reverse video.
    tinkery.send_keys("enter");
    let waiting = format!(" › {:<35}{:<17}{WAITING}   ", "Save", "save");
    tinkery.wait_for_reversed(FIRST_ROW + 1, &waiting, WAIT);
}
