mod harness;

use std::fs;
use std::path::Path;
use std::time::Duration;

use harness::{CAP_L, CAP_R, ROWS, Tinkery, buttons_text};
use tempfile::TempDir;
use vt100::Color;

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(5);
const PROMPT: &str = "a.txt has unsaved changes";
const EXPLANATION: &str = "Closing discards them unless you save.";

/// The default theme, tinkery: `bg`, `raised`, `btn`, `text`, `strong`,
/// `accent`, `acc_ink` and `err`.
const BG: Color = Color::Rgb(0x11, 0x0f, 0x0e);
const RAISED: Color = Color::Rgb(0x1f, 0x1b, 0x19);
const BTN: Color = Color::Rgb(0x33, 0x2a, 0x25);
const TEXT: Color = Color::Rgb(0xb5, 0xa8, 0x9d);
const STRONG: Color = Color::Rgb(0xf8, 0xf0, 0xe6);
const ACCENT: Color = Color::Rgb(0xff, 0x8a, 0x3d);
const ACC_INK: Color = Color::Rgb(0x1f, 0x0d, 0x02);
const ERR: Color = Color::Rgb(0xff, 0x5d, 0x4d);
const BLACK: Color = Color::Rgb(0, 0, 0);

/// At 100x30 the card is a panel 46 wide (the explanation's 38 cells + 8) and
/// 9 tall, centred (tinkery-forge spec F16): its edge on rows 10 and 18 from
/// column 27, the question and explanation on rows 12 and 13 from column 30,
/// and the buttons on row 16: ` Save S ` at 30, ` Discard D ` at 40 and
/// ` Cancel Esc ` at 53.
const CARD_X: u16 = 27;
const CARD_Y: u16 = 10;
const CARD_WIDTH: u16 = 46;
const TEXT_X: u16 = 30;
const BUTTON_ROW: u16 = 16;
const SAVE_X: u16 = 30;
const DISCARD_X: u16 = 40;
const CANCEL_X: u16 = 53;
const BUTTONS: &[(&str, &str)] = &[("Save", "S"), ("Discard", "D"), ("Cancel", "Esc")];

/// `a` moved `t` of the way to `b`, per channel, rounded, as the editor mixes.
fn mix(a: Color, b: Color, t: f64) -> Color {
    let (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) = (a, b) else {
        panic!("can't mix {a:?} and {b:?}");
    };
    let m = |p: u8, q: u8| (f64::from(p) + (f64::from(q) - f64::from(p)) * t).round() as u8;
    Color::Rgb(m(r1, r2), m(g1, g2), m(b1, b2))
}

fn status_line(tinkery: &Tinkery) -> String {
    tinkery.screen()[usize::from(ROWS - 1)].clone()
}

/// Opens `a.txt` holding `contents` in a fresh temp dir, with tinkery running
/// there on the default theme.
fn open_a(contents: &[u8]) -> (TempDir, Tinkery) {
    open_a_with("", contents)
}

/// The same with `config` as the config file.
fn open_a_with(config: &str, contents: &[u8]) -> (TempDir, Tinkery) {
    let dir = tempfile::tempdir().expect("create temp dir");
    fs::write(dir.path().join("a.txt"), contents).expect("write a.txt");
    let tinkery = Tinkery::spawn_in_with_config(dir.path(), config, &["a.txt"]);
    tinkery.wait_for_text("a.txt", START);
    tinkery.wait_for_text("Ln 1, Col 1", START);
    (dir, tinkery)
}

fn read_a(dir: &Path) -> Vec<u8> {
    fs::read(dir.join("a.txt")).expect("read a.txt")
}

/// Types `x` and waits for the dirty marker.
fn make_dirty(tinkery: &mut Tinkery) {
    tinkery.type_text("x");
    tinkery.wait_for_text("a.txt •", WAIT);
}

/// Ctrl+Q, then checks the card's copy sits where the panel puts it, whatever
/// the theme and the edge.
fn ask_to_quit(tinkery: &mut Tinkery) {
    tinkery.send_keys("ctrl+q");
    tinkery.wait_for_text(PROMPT, WAIT);
    let screen = tinkery.screen();
    assert_eq!(
        tinkery.text_col(CARD_Y + 2, PROMPT),
        Some(TEXT_X),
        "{screen:#?}"
    );
    assert_eq!(
        tinkery.text_col(CARD_Y + 3, EXPLANATION),
        Some(TEXT_X),
        "{screen:#?}"
    );
}

/// Ctrl+Q, then checks the card is a flush panel over a dimmed screen with its
/// copy and round buttons in place, in the default theme's colours.
fn open_quit_prompt(tinkery: &mut Tinkery) {
    ask_to_quit(tinkery);
    let screen = tinkery.screen();
    // The flush edge: eighth blocks in accent on the dimmed editor, the
    // corners blank.
    let behind = mix(BG, BLACK, 0.45);
    for (row, edge) in [(CARD_Y, "▁"), (CARD_Y + 8, "▔")] {
        assert_eq!(
            tinkery.text_col(row, &edge.repeat(44)),
            Some(CARD_X + 1),
            "{screen:#?}"
        );
        assert_eq!(tinkery.fg_at(CARD_X + 1, row), ACCENT);
        assert_eq!(tinkery.bg_at(CARD_X + 1, row), behind);
        assert_eq!(tinkery.bg_at(CARD_X, row), behind);
    }
    for row in CARD_Y + 1..CARD_Y + 8 {
        assert_eq!(tinkery.text_col(row, "▕"), Some(CARD_X), "{screen:#?}");
        assert_eq!(
            tinkery.text_col(row, "▏"),
            Some(CARD_X + CARD_WIDTH - 1),
            "{screen:#?}"
        );
        assert_eq!(tinkery.fg_at(CARD_X, row), ACCENT);
        assert_eq!(tinkery.bg_at(CARD_X, row), behind);
        assert_eq!(tinkery.bg_at(CARD_X + 1, row), RAISED);
        assert_eq!(tinkery.bg_at(CARD_X + CARD_WIDTH - 2, row), RAISED);
    }
    // The question in strong bold, the explanation in text.
    assert_eq!(tinkery.fg_at(TEXT_X, CARD_Y + 2), STRONG);
    assert!(tinkery.bold_at(TEXT_X, CARD_Y + 2));
    assert_eq!(tinkery.fg_at(TEXT_X, CARD_Y + 3), TEXT);
    assert!(!tinkery.bold_at(TEXT_X, CARD_Y + 3));
    // The action first, the way out last.
    assert_eq!(
        tinkery.text_col(BUTTON_ROW, &buttons_text(BUTTONS)),
        Some(SAVE_X),
        "{screen:#?}"
    );
    // Below the card the editor is darkened.
    assert_eq!(tinkery.bg_at(50, 22), behind);
}

fn assert_exits(tinkery: &mut Tinkery) {
    let status = tinkery.wait_exit(WAIT);
    assert!(status.success(), "tinkery exited with {status:?}");
}

#[test]
fn the_save_message_holds_the_path_slot_until_the_next_key() {
    // aurora's `ok` and `strong`.
    const OK: vt100::Color = vt100::Color::Rgb(0x7f, 0xe3, 0xc9);
    const STRONG: vt100::Color = vt100::Color::Rgb(0xf4, 0xf2, 0xfb);
    let dir = tempfile::tempdir().expect("create temp dir");
    fs::write(dir.path().join("a.txt"), "hello").expect("write a.txt");
    let mut tinkery = Tinkery::spawn_in_with_config(dir.path(), "theme = \"aurora\"\n", &["a.txt"]);
    tinkery.wait_for_text("Ln 1, Col 1", START);
    let status_row = ROWS - 1;
    assert_eq!(tinkery.text_col(status_row, "a.txt"), Some(20));

    tinkery.type_text("x");
    tinkery.send_keys("ctrl+s");
    tinkery.wait_for_text("✓ saved a.txt", WAIT);
    assert_eq!(tinkery.text_col(status_row, "✓ saved a.txt"), Some(20));
    assert_eq!(tinkery.fg_at(20, status_row), OK);

    // Any key gives the slot back to the path.
    tinkery.send_keys("left");
    tinkery.wait_for_text("Ln 1, Col 1", WAIT);
    tinkery.wait_for_text_gone("saved a.txt", WAIT);
    assert_eq!(tinkery.text_col(status_row, "a.txt"), Some(20));
    assert_eq!(tinkery.fg_at(20, status_row), STRONG);
}

#[test]
fn ctrl_s_saves() {
    let (dir, mut tinkery) = open_a(b"hello\r\nworld");
    assert!(!status_line(&tinkery).contains('●'));

    make_dirty(&mut tinkery);
    tinkery.send_keys("ctrl+s");
    tinkery.wait_for_text("saved a.txt", WAIT);
    let status = status_line(&tinkery);
    assert!(!status.contains('●'), "still dirty: {status:?}");
    // CRLF and the missing final newline survive the save.
    assert_eq!(read_a(dir.path()), b"xhello\r\nworld");

    // A clean buffer quits without asking.
    tinkery.send_keys("ctrl+q");
    assert_exits(&mut tinkery);
}

#[test]
fn ctrl_s_on_an_untitled_buffer_asks_for_a_path() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let mut tinkery = Tinkery::spawn_in(dir.path(), &[]);
    tinkery.wait_for_text("Open directory", START);
    // Leave the splash for the untitled buffer under it.
    tinkery.send_keys("ctrl+n");
    tinkery.type_text("x");
    tinkery.wait_for_text("untitled •", WAIT);
    tinkery.send_keys("ctrl+s");
    tinkery.wait_for_text(" Save as  ", WAIT);
    // Esc leaves it unsaved.
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(" Save as  ", WAIT);
    assert!(status_line(&tinkery).contains("untitled •"));
    assert_eq!(fs::read_dir(dir.path()).expect("list dir").count(), 0);

    tinkery.send_keys("ctrl+q");
    tinkery.wait_for_text("untitled has unsaved changes", WAIT);
    tinkery.type_text("d");
    assert_exits(&mut tinkery);
}

#[test]
fn quit_guard_save() {
    let (dir, mut tinkery) = open_a(b"hello\n");
    make_dirty(&mut tinkery);
    open_quit_prompt(&mut tinkery);
    tinkery.type_text("s");
    assert_exits(&mut tinkery);
    assert_eq!(read_a(dir.path()), b"xhello\n");
}

#[test]
fn quit_guard_discard() {
    let (dir, mut tinkery) = open_a(b"hello\n");
    make_dirty(&mut tinkery);
    open_quit_prompt(&mut tinkery);
    tinkery.type_text("d");
    assert_exits(&mut tinkery);
    assert_eq!(read_a(dir.path()), b"hello\n");
}

#[test]
fn quit_guard_cancel() {
    let (dir, mut tinkery) = open_a(b"hello\n");
    make_dirty(&mut tinkery);

    // C closes the prompt and editing carries on.
    open_quit_prompt(&mut tinkery);
    tinkery.type_text("c");
    tinkery.wait_for_text_gone(PROMPT, WAIT);
    tinkery.type_text("y");
    tinkery.wait_for_text("xyhello", WAIT);

    // So does Esc.
    open_quit_prompt(&mut tinkery);
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(PROMPT, WAIT);
    tinkery.type_text("z");
    tinkery.wait_for_text("xyzhello", WAIT);
    assert_eq!(read_a(dir.path()), b"hello\n");

    open_quit_prompt(&mut tinkery);
    tinkery.type_text("d");
    assert_exits(&mut tinkery);
}

#[test]
fn the_focused_button_is_a_filled_pill_and_the_others_wait_on_btn() {
    let (_dir, mut tinkery) = open_a(b"hello\n");
    make_dirty(&mut tinkery);
    open_quit_prompt(&mut tinkery);
    // ` Save S ` is a pill of accent on the card, in acc_ink bold.
    for (x, cap) in [(SAVE_X, CAP_L), (SAVE_X + 7, CAP_R)] {
        assert_eq!(tinkery.text_col(BUTTON_ROW, cap), Some(x));
        assert_eq!(tinkery.fg_at(x, BUTTON_ROW), ACCENT, "{cap:?}");
        assert_eq!(tinkery.bg_at(x, BUTTON_ROW), RAISED, "{cap:?}");
    }
    assert_eq!(tinkery.bg_text(BUTTON_ROW, ACCENT), "Save S");
    for x in SAVE_X + 1..SAVE_X + 7 {
        assert_eq!(tinkery.fg_at(x, BUTTON_ROW), ACC_INK, "{x}");
        assert!(tinkery.bold_at(x, BUTTON_ROW), "{x}");
    }
    // The others wait: strong bold on btn, their keys in accent.
    assert_eq!(tinkery.bg_text(BUTTON_ROW, BTN), "Discard DCancel Esc");
    for (x, key) in [(DISCARD_X, DISCARD_X + 9), (CANCEL_X, CANCEL_X + 8)] {
        assert_eq!(tinkery.fg_at(x, BUTTON_ROW), BTN);
        assert_eq!(tinkery.bg_at(x, BUTTON_ROW), RAISED);
        assert_eq!(tinkery.fg_at(x + 1, BUTTON_ROW), STRONG);
        assert!(tinkery.bold_at(x + 1, BUTTON_ROW));
        assert_eq!(tinkery.fg_at(key, BUTTON_ROW), ACCENT);
        assert!(tinkery.bold_at(key, BUTTON_ROW));
    }
    // The key is beside the label now, not underlined in it.
    assert_eq!(tinkery.underlined_text(BUTTON_ROW), "");
    tinkery.type_text("d");
    assert_exits(&mut tinkery);
}

#[test]
fn enter_presses_the_focused_button_which_starts_on_save() {
    let (dir, mut tinkery) = open_a(b"hello\n");
    make_dirty(&mut tinkery);
    open_quit_prompt(&mut tinkery);
    tinkery.send_keys("enter");
    assert_exits(&mut tinkery);
    assert_eq!(read_a(dir.path()), b"xhello\n");
}

#[test]
fn arrows_and_tab_move_the_focus() {
    let (dir, mut tinkery) = open_a(b"hello\n");
    make_dirty(&mut tinkery);
    open_quit_prompt(&mut tinkery);
    let (save, discard, cancel) = (SAVE_X + 1, DISCARD_X + 1, CANCEL_X + 1);

    // Right fills Discard, in red since it loses the changes; Left goes back,
    // and stops at the first.
    tinkery.send_keys("right");
    tinkery.wait_for_bg(discard, BUTTON_ROW, ERR, WAIT);
    assert_eq!(tinkery.bg_text(BUTTON_ROW, ERR), "Discard D");
    // Its text is the theme's ground, which reads on the red.
    assert_eq!(tinkery.fg_at(discard, BUTTON_ROW), BG);
    assert_eq!(tinkery.bg_at(save, BUTTON_ROW), BTN);
    assert_eq!(tinkery.bg_text(BUTTON_ROW, ACCENT), "");
    tinkery.send_keys("left");
    tinkery.wait_for_bg(save, BUTTON_ROW, ACCENT, WAIT);
    assert_eq!(tinkery.bg_at(discard, BUTTON_ROW), BTN);
    tinkery.send_keys("left");
    tinkery.send_keys("tab");
    tinkery.wait_for_bg(discard, BUTTON_ROW, ERR, WAIT);
    // Cancel loses nothing, so focused it is the accent. Tab past the last
    // wraps to the first.
    tinkery.send_keys("tab");
    tinkery.wait_for_bg(cancel, BUTTON_ROW, ACCENT, WAIT);
    assert_eq!(tinkery.bg_text(BUTTON_ROW, ACCENT), "Cancel Esc");
    tinkery.send_keys("tab");
    tinkery.wait_for_bg(save, BUTTON_ROW, ACCENT, WAIT);
    tinkery.send_keys("tab");
    tinkery.wait_for_bg(discard, BUTTON_ROW, ERR, WAIT);
    tinkery.send_keys("tab");
    tinkery.wait_for_bg(cancel, BUTTON_ROW, ACCENT, WAIT);

    // Enter on Cancel closes the card and editing carries on.
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone(PROMPT, WAIT);
    tinkery.type_text("y");
    tinkery.wait_for_text("xyhello", WAIT);

    // The card opens again with Save filled; Right, Enter discards.
    open_quit_prompt(&mut tinkery);
    assert_eq!(tinkery.bg_at(save, BUTTON_ROW), ACCENT);
    tinkery.send_keys("right");
    tinkery.wait_for_bg(discard, BUTTON_ROW, ERR, WAIT);
    tinkery.send_keys("enter");
    assert_exits(&mut tinkery);
    assert_eq!(read_a(dir.path()), b"hello\n");
}

#[test]
fn clicking_a_button_presses_it_and_outside_cancels() {
    let (dir, mut tinkery) = open_a(b"hello\n");
    make_dirty(&mut tinkery);

    // A click inside the card but off the buttons does nothing: on its copy,
    // between two buttons, on its edge.
    open_quit_prompt(&mut tinkery);
    tinkery.click(TEXT_X, CARD_Y + 4);
    tinkery.click(SAVE_X + 8, BUTTON_ROW);
    tinkery.click(CARD_X, CARD_Y);
    tinkery.assert_running_for(Duration::from_millis(300));
    assert!(tinkery.screen().iter().any(|line| line.contains(PROMPT)));
    // Outside it is Esc.
    tinkery.click(5, 25);
    tinkery.wait_for_text_gone(PROMPT, WAIT);

    // The round end of a button is the button.
    open_quit_prompt(&mut tinkery);
    tinkery.click(CANCEL_X, BUTTON_ROW);
    tinkery.wait_for_text_gone(PROMPT, WAIT);

    open_quit_prompt(&mut tinkery);
    tinkery.click(DISCARD_X + 3, BUTTON_ROW);
    assert_exits(&mut tinkery);
    assert_eq!(read_a(dir.path()), b"hello\n");
}

#[test]
fn rounded_corners_draw_the_card_inside_a_line() {
    let (_dir, mut tinkery) = open_a_with("[ui]\ncorners = \"rounded\"\n", b"hello\n");
    make_dirty(&mut tinkery);
    ask_to_quit(&mut tinkery);
    let screen = tinkery.screen();
    for (row, edge) in [
        (CARD_Y, format!("╭{}╮", "─".repeat(44))),
        (CARD_Y + 8, format!("╰{}╯", "─".repeat(44))),
    ] {
        assert_eq!(tinkery.text_col(row, &edge), Some(CARD_X), "{screen:#?}");
        assert_eq!(tinkery.fg_at(CARD_X, row), ACCENT);
    }
    for row in CARD_Y + 1..CARD_Y + 8 {
        let line: Vec<char> = screen[usize::from(row)].chars().collect();
        assert_eq!(line[usize::from(CARD_X)], '│', "{screen:#?}");
        assert_eq!(
            line[usize::from(CARD_X + CARD_WIDTH - 1)],
            '│',
            "{screen:#?}"
        );
        assert_eq!(tinkery.fg_at(CARD_X, row), ACCENT);
        assert_eq!(tinkery.bg_at(CARD_X + 1, row), RAISED);
    }
    // No flush glyph is left, and the buttons are where they were.
    for line in &screen {
        assert!(!line.contains(['▁', '▔', '▕', '▏']), "{screen:#?}");
    }
    assert_eq!(
        tinkery.text_col(BUTTON_ROW, &buttons_text(BUTTONS)),
        Some(SAVE_X),
        "{screen:#?}"
    );
    assert_eq!(tinkery.bg_at(SAVE_X + 1, BUTTON_ROW), ACCENT);
    tinkery.type_text("d");
    assert_exits(&mut tinkery);
}

#[test]
fn pill_caps_off_squares_the_buttons() {
    let (_dir, mut tinkery) = open_a_with("[ui]\npill_caps = false\n", b"hello\n");
    make_dirty(&mut tinkery);
    ask_to_quit(&mut tinkery);
    let screen = tinkery.screen();
    for line in &screen {
        assert!(
            !line.contains(CAP_L) && !line.contains(CAP_R),
            "{screen:#?}"
        );
    }
    // Each end is a blank of the button's fill, so nothing moved.
    assert_eq!(
        tinkery.text_col(BUTTON_ROW, " Save S    Discard D    Cancel Esc "),
        Some(SAVE_X),
        "{screen:#?}"
    );
    assert_eq!(tinkery.bg_text(BUTTON_ROW, ACCENT), " Save S ");
    assert_eq!(tinkery.bg_text(BUTTON_ROW, BTN), " Discard D  Cancel Esc ");
    tinkery.type_text("d");
    assert_exits(&mut tinkery);
}

#[test]
fn mono_reverses_the_focused_button_and_dims_the_screen_behind() {
    for corners in ["flush", "rounded"] {
        let config = format!("theme = \"mono\"\n[ui]\ncorners = \"{corners}\"\n");
        let (_dir, mut tinkery) = open_a_with(&config, b"hello\n");
        make_dirty(&mut tinkery);
        ask_to_quit(&mut tinkery);
        let screen = tinkery.screen();
        // No Nerd Font glyph, whatever `pill_caps` says.
        for line in &screen {
            assert!(
                !line.contains(CAP_L) && !line.contains(CAP_R),
                "{screen:#?}"
            );
        }
        assert_eq!(
            tinkery.text_col(BUTTON_ROW, " Save S    Discard D    Cancel Esc "),
            Some(SAVE_X),
            "{screen:#?}"
        );
        let top = if corners == "flush" { "▁" } else { "─" };
        assert_eq!(
            tinkery.text_col(CARD_Y, &top.repeat(44)),
            Some(CARD_X + 1),
            "{screen:#?}"
        );
        // The focused button is the only reverse video on its row, and every
        // button is bold.
        tinkery.wait_for_reversed(BUTTON_ROW, " Save S ", WAIT);
        for x in [SAVE_X + 1, DISCARD_X + 1, CANCEL_X + 1] {
            assert!(tinkery.bold_at(x, BUTTON_ROW), "{x}");
        }
        tinkery.send_keys("right");
        tinkery.wait_for_reversed(BUTTON_ROW, " Discard D ", WAIT);
        tinkery.send_keys("right");
        tinkery.wait_for_reversed(BUTTON_ROW, " Cancel Esc ", WAIT);
        // The screen behind is dimmed with the DIM attribute, not a blend: the
        // text beside the card and the status line under it. The card isn't.
        // (The harness's terminal keeps one weight a cell, so a bold cell
        // behind the card reads as bold; these are plain ones.)
        let text_row = 3;
        assert_eq!(tinkery.text_col(text_row, "xhello"), Some(6), "{screen:#?}");
        assert!(tinkery.dim_at(6, text_row));
        let position = tinkery
            .text_col(ROWS - 1, "Ln 1, Col 2")
            .expect("the status line shows the position");
        assert!(tinkery.dim_at(position, ROWS - 1));
        assert!(tinkery.dim_at(CARD_X - 1, BUTTON_ROW));
        assert!(!tinkery.dim_at(CARD_X, BUTTON_ROW));
        assert!(!tinkery.dim_at(TEXT_X, CARD_Y + 2));
        assert!(!tinkery.dim_at(SAVE_X + 1, BUTTON_ROW));
        for (x, y) in [
            (5, text_row),
            (TEXT_X, CARD_Y + 2),
            (SAVE_X + 1, BUTTON_ROW),
        ] {
            assert!(!matches!(tinkery.fg_at(x, y), Color::Rgb(..)), "{x},{y}");
            assert!(!matches!(tinkery.bg_at(x, y), Color::Rgb(..)), "{x},{y}");
        }
        tinkery.send_keys("enter");
        tinkery.wait_for_text_gone(PROMPT, WAIT);
        assert!(!tinkery.dim_at(6, text_row));
        tinkery.send_keys("ctrl+q");
        tinkery.wait_for_text(PROMPT, WAIT);
        tinkery.type_text("d");
        assert_exits(&mut tinkery);
    }
}
