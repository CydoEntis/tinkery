mod harness;

use std::time::Duration;

use harness::{COLS, ROWS, Tinkery};
use vt100::Color;

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(5);
const STATUS_ROW: u16 = ROWS - 1;

/// nord's `sidebar_bg`.
const NORD_SIDEBAR: Color = Color::Rgb(0x27, 0x2c, 0x36);
/// tinkery's `surface`, the status line's ground.
const TINKERY_SURFACE: Color = Color::Rgb(0x17, 0x14, 0x13);
/// tinkery's `bg`, the editor ground.
const TINKERY_BG: Color = Color::Rgb(0x11, 0x0f, 0x0e);
/// nord's `bg`, the editor ground.
const NORD_BG: Color = Color::Rgb(0x2e, 0x34, 0x40);

/// The status bar's brand block covers columns 0..18; the bar is past it.
const BLOCK_END: u16 = 18;

/// Every cell of the status line past the brand block has background `color`,
/// but for the splash's keys, which sit on keycaps of one other colour.
fn assert_status_bg(tinkery: &Tinkery, color: Color) {
    tinkery.wait_for_bg(BLOCK_END, STATUS_ROW, color, WAIT);
    let row: Vec<char> = tinkery.screen()[usize::from(STATUS_ROW)].chars().collect();
    let off: Vec<u16> = (BLOCK_END..COLS)
        .filter(|&col| tinkery.bg_at(col, STATUS_ROW) != color)
        .collect();
    let keys: String = off
        .iter()
        .filter_map(|&col| row.get(usize::from(col)))
        .collect();
    // A long path drops the keys, and then the whole bar is `color`.
    assert!(keys.is_empty() || keys == "↑↓⏎q", "off the bar: {keys:?}");
    if let Some(&first) = off.first() {
        let cap = tinkery.bg_at(first, STATUS_ROW);
        for col in off {
            assert_eq!(tinkery.bg_at(col, STATUS_ROW), cap, "keycap column {col}");
        }
    }
}

#[test]
fn nord_paints_the_status_line_with_its_sidebar_colour() {
    let mut tinkery = Tinkery::spawn_with_config("theme = \"nord\"\n", &[]);
    tinkery.wait_for_text("Open directory", START);
    assert_status_bg(&tinkery, NORD_SIDEBAR);
    // Leave the splash, whose card covers the middle of the editor area.
    tinkery.send_keys("ctrl+n");
    // The editor ground below the first line is nord's background.
    tinkery.wait_for_bg(50, 10, NORD_BG, WAIT);
}

#[test]
fn an_override_changes_the_colour() {
    let mut tinkery = Tinkery::spawn_with_config(
        "theme = \"nord\"\n[theme_overrides]\nsidebar_bg = \"#123456\"\n",
        &[],
    );
    tinkery.wait_for_text("Open directory", START);
    assert_status_bg(&tinkery, Color::Rgb(0x12, 0x34, 0x56));
    tinkery.send_keys("ctrl+n");
    tinkery.wait_for_bg(50, 10, NORD_BG, WAIT);
}

/// `""` is the harness's way to ask for the real default, which `spawn` isn't.
#[test]
fn an_empty_config_starts_in_tinkery() {
    let mut tinkery = Tinkery::spawn_with_config("", &[]);
    tinkery.wait_for_text("Open directory", START);
    assert_status_bg(&tinkery, TINKERY_SURFACE);
    tinkery.send_keys("ctrl+n");
    tinkery.wait_for_bg(50, 10, TINKERY_BG, WAIT);
}

#[test]
fn an_unknown_theme_falls_back_to_tinkery_and_says_so() {
    let tinkery = Tinkery::spawn_with_config("theme = \"solarized\"\n", &[]);
    tinkery.wait_for_text("theme: unknown theme \"solarized\", using tinkery", START);
    assert_status_bg(&tinkery, TINKERY_SURFACE);
}

#[test]
fn a_bad_override_falls_back_to_tinkery_and_says_so() {
    let tinkery = Tinkery::spawn_with_config(
        "theme = \"nord\"\n[theme_overrides]\nkeyword = \"blurple\"\n",
        &[],
    );
    tinkery.wait_for_text(
        "theme: bad colour \"blurple\" for keyword, using tinkery",
        START,
    );
    assert_status_bg(&tinkery, TINKERY_SURFACE);
    let screen = tinkery.screen();
    assert_eq!(
        screen[usize::from(harness::TOAST_ROW)]
            .matches("theme:")
            .count(),
        1,
        "one message: {screen:#?}"
    );
    assert_eq!(screen.join(" ").matches("theme:").count(), 1, "{screen:#?}");
}

#[test]
fn mono_draws_the_brand_block_on_flat_accent() {
    let tinkery = Tinkery::spawn_with_config("theme = \"mono\"\n", &[]);
    tinkery.wait_for_text("Open directory", START);
    // mono has no ramps: every block cell is its accent, the terminal's white.
    let accent = tinkery.bg_at(0, STATUS_ROW);
    assert_ne!(accent, Color::Default);
    for col in 0..BLOCK_END {
        assert_eq!(tinkery.bg_at(col, STATUS_ROW), accent, "block column {col}");
    }
    // Past the block the bar is on the terminal's own background.
    assert_eq!(tinkery.bg_at(BLOCK_END, STATUS_ROW), Color::Default);
    assert_eq!(tinkery.text_col(STATUS_ROW, "✦ tinkery"), Some(1));
}
