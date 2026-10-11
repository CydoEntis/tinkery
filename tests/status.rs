mod harness;

use std::path::Path;
use std::time::Duration;

use harness::{CAP_L, CAP_R, COLS, ROWS, Tinkery, hints_text};
use vt100::Color;

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(5);
const PROJECT: &str = "tests/fixtures/project";
const TINKERY: &str = "theme = \"tinkery\"\n";
const TINKERY_LIGHT: &str = "theme = \"tinkery-light\"\n";
const MONO: &str = "theme = \"mono\"\n";
const STATUS_ROW: u16 = ROWS - 1;
/// The brand block covers columns 0..18; the flat bar is past it.
const BLOCK_END: u16 = 18;

// tinkery's roles (tinkery-forge spec, Palette).
const ACCENT: Color = Color::Rgb(0xff, 0x8a, 0x3d);
const ACCENT2: Color = Color::Rgb(0xff, 0xd2, 0x7a);
const SURFACE: Color = Color::Rgb(0x17, 0x14, 0x13);
const ACC_INK: Color = Color::Rgb(0x1f, 0x0d, 0x02);
const MUTED: Color = Color::Rgb(0x7d, 0x70, 0x67);
const TEXT: Color = Color::Rgb(0xb5, 0xa8, 0x9d);
const STRONG: Color = Color::Rgb(0xf8, 0xf0, 0xe6);
const WARN: Color = Color::Rgb(0xf2, 0xc1, 0x4e);
const BTN: Color = Color::Rgb(0x33, 0x2a, 0x25);

/// What the splash's status bar says its keys do.
const SPLASH_HINTS: [(&str, &str); 3] = [("↑↓", "select"), ("⏎", "choose"), ("q", "quit")];

/// Tinkery in the fixture project, with `docs/guide.md` opened through the
/// picker so its path has a directory part.
fn open_guide() -> Tinkery {
    let mut tinkery = Tinkery::spawn_in_with_config(Path::new(PROJECT), TINKERY, &[]);
    tinkery.wait_for_text("Open directory", START);
    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text("cast · files · commands", WAIT);
    tinkery.type_text("guide");
    tinkery.wait_for_text("guide.md  docs", WAIT);
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone("cast · files · commands", WAIT);
    tinkery.wait_for_screen("guide.md in the status bar", WAIT, |screen| {
        screen[usize::from(STATUS_ROW)].contains("guide.md")
    });
    tinkery
}

#[test]
fn the_brand_block_ramps_from_accent_to_accent2_then_fades_into_the_bar() {
    let tinkery = Tinkery::spawn_with_config(TINKERY, &[]);
    tinkery.wait_for_text("Open directory", START);
    let status = &tinkery.screen()[usize::from(STATUS_ROW)];
    assert!(status.starts_with(" ✦ tinkery"), "{status:?}");
    // The old `tinkery  ` text prefix is gone.
    assert!(!status.starts_with("tinkery"), "{status:?}");

    // grad([accent, accent2], i/9) for i < 10, then mix(accent2, surface, (i − 9)/9).
    assert_eq!(tinkery.bg_at(0, STATUS_ROW), ACCENT);
    assert_eq!(tinkery.bg_at(4, STATUS_ROW), Color::Rgb(255, 170, 88));
    assert_eq!(tinkery.bg_at(9, STATUS_ROW), ACCENT2);
    assert_eq!(tinkery.bg_at(13, STATUS_ROW), Color::Rgb(152, 126, 76));
    assert_eq!(tinkery.bg_at(17, STATUS_ROW), Color::Rgb(49, 41, 30));
    // `✦ tinkery` at x = 1, in acc_ink, bold.
    assert_eq!(tinkery.text_col(STATUS_ROW, "✦ tinkery"), Some(1));
    for col in 1..10 {
        assert_eq!(tinkery.fg_at(col, STATUS_ROW), ACC_INK, "column {col}");
        assert!(tinkery.bold_at(col, STATUS_ROW), "column {col}");
    }
}

/// The columns past the brand block whose background isn't `surface`.
fn off_the_bar(tinkery: &Tinkery, surface: Color) -> Vec<u16> {
    (BLOCK_END..COLS)
        .filter(|&col| tinkery.bg_at(col, STATUS_ROW) != surface)
        .collect()
}

#[test]
fn past_the_brand_block_the_bar_is_flat_surface() {
    // With a file open, clean then dirty.
    let mut tinkery = open_guide();
    assert_eq!(off_the_bar(&tinkery, SURFACE), Vec::<u16>::new());
    tinkery.type_text("z");
    tinkery.wait_for_screen("the dirty mark", WAIT, |screen| {
        screen[usize::from(STATUS_ROW)].contains("guide.md •")
    });
    assert_eq!(off_the_bar(&tinkery, SURFACE), Vec::<u16>::new());

    // A note is a toast over the editor, so it leaves the bar alone: the path
    // keeps its slot under it.
    let tinkery = Tinkery::spawn_in_with_config(
        Path::new(PROJECT),
        "theme = \"solarized\"\n",
        &["docs/guide.md"],
    );
    tinkery.wait_for_toast("✕ theme: unknown theme \"solarized\", using tinkery", START);
    tinkery.wait_for_screen("guide.md in the status bar", WAIT, |screen| {
        screen[usize::from(STATUS_ROW)].contains("guide.md")
    });
    assert_eq!(tinkery.text_col(STATUS_ROW, "guide.md"), Some(20));
    assert_eq!(off_the_bar(&tinkery, SURFACE), Vec::<u16>::new());
}

#[test]
fn the_splash_keys_are_keycap_hints_on_the_flat_bar() {
    let tinkery = Tinkery::spawn_with_config(TINKERY, &[]);
    tinkery.wait_for_text("Open directory", START);
    let status = tinkery.screen()[usize::from(STATUS_ROW)].clone();
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    let keys = hints_text(&SPLASH_HINTS);
    // The hints, six blanks, the version, ending two cells short of the edge.
    let line = format!("{keys}      {version}");
    let width = u16::try_from(line.chars().count()).expect("a short line");
    assert_eq!(
        tinkery.text_col(STATUS_ROW, &line),
        Some(COLS - 2 - width),
        "{status:?}"
    );

    // Only the keys themselves are off the bar's colour, on `btn`.
    let caps = off_the_bar(&tinkery, SURFACE);
    for &col in &caps {
        assert_eq!(tinkery.bg_at(col, STATUS_ROW), BTN, "column {col}");
        assert_eq!(tinkery.fg_at(col, STATUS_ROW), STRONG, "column {col}");
        assert!(tinkery.bold_at(col, STATUS_ROW), "column {col}");
    }
    assert_eq!(tinkery.bg_text(STATUS_ROW, BTN), "↑↓⏎q", "{status:?}");
    // Each cap's round ends are `btn` drawn on the bar; the word is muted.
    for (key, what) in SPLASH_HINTS {
        let x = tinkery
            .text_col(STATUS_ROW, &hints_text(&[(key, what)]))
            .expect("the hint");
        let key_width = u16::try_from(key.chars().count()).expect("a short key");
        for end in [x, x + 1 + key_width] {
            assert_eq!(tinkery.fg_at(end, STATUS_ROW), BTN, "{key} end");
            assert_eq!(tinkery.bg_at(end, STATUS_ROW), SURFACE, "{key} end");
        }
        let word = x + key_width + 3;
        assert_eq!(tinkery.fg_at(word, STATUS_ROW), MUTED, "{what}");
        assert!(!tinkery.bold_at(word, STATUS_ROW), "{what}");
    }
    let version_x = tinkery.text_col(STATUS_ROW, &version).expect("the version");
    assert_eq!(tinkery.fg_at(version_x, STATUS_ROW), MUTED);
}

#[test]
fn tinkery_light_draws_the_same_bar_in_its_own_colours() {
    // tinkery-light's accent, accent2, surface, acc_ink and btn.
    let accent = Color::Rgb(0xb8, 0x4a, 0x08);
    let accent2 = Color::Rgb(0xb0, 0x7d, 0x0c);
    let surface = Color::Rgb(0xed, 0xe6, 0xdc);
    let acc_ink = Color::Rgb(0xff, 0xf6, 0xec);
    let btn = Color::Rgb(0xdc, 0xd2, 0xc5);
    let tinkery = Tinkery::spawn_with_config(TINKERY_LIGHT, &[]);
    tinkery.wait_for_text("Open directory", START);
    let status = tinkery.screen()[usize::from(STATUS_ROW)].clone();
    assert_eq!(tinkery.bg_at(0, STATUS_ROW), accent);
    assert_eq!(tinkery.bg_at(9, STATUS_ROW), accent2);
    assert_eq!(tinkery.fg_at(1, STATUS_ROW), acc_ink);
    assert!(tinkery.bold_at(1, STATUS_ROW));
    assert!(status.contains(&hints_text(&SPLASH_HINTS)), "{status:?}");
    for col in off_the_bar(&tinkery, surface) {
        assert_eq!(tinkery.bg_at(col, STATUS_ROW), btn, "column {col}");
    }
    assert_eq!(tinkery.bg_text(STATUS_ROW, btn), "↑↓⏎q", "{status:?}");
}

#[test]
fn mono_has_a_flat_accent_block_and_keycaps_without_cap_glyphs() {
    let mut tinkery = Tinkery::spawn_with_config(MONO, &[]);
    tinkery.wait_for_text("Open directory", START);
    let status = tinkery.screen()[usize::from(STATUS_ROW)].clone();
    // No ramp: every block cell is the accent, the terminal's white.
    let accent = tinkery.bg_at(0, STATUS_ROW);
    assert_ne!(accent, Color::Default);
    for col in 0..BLOCK_END {
        assert_eq!(tinkery.bg_at(col, STATUS_ROW), accent, "column {col}");
    }
    assert!(tinkery.bold_at(1, STATUS_ROW));
    // The keys are squared off: a blank of the cap's fill each side, never a
    // Nerd Font glyph, and as wide as the round ones.
    assert!(
        !status.contains(CAP_L) && !status.contains(CAP_R),
        "{status:?}"
    );
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    let line = format!(" ↑↓  select    ⏎  choose    q  quit      {version}");
    let width = u16::try_from(line.chars().count()).expect("a short line");
    assert_eq!(
        tinkery.text_col(STATUS_ROW, &line),
        Some(COLS - 2 - width),
        "{status:?}"
    );
    let caps = off_the_bar(&tinkery, Color::Default);
    assert_eq!(caps.len(), 10, "{status:?}");
    let fill = tinkery.bg_at(caps[0], STATUS_ROW);
    assert_eq!(
        tinkery.bg_text(STATUS_ROW, fill),
        " ↑↓  ⏎  q ",
        "{status:?}"
    );

    // With a file open the bar is the terminal's own background throughout.
    tinkery.send_keys("ctrl+n");
    tinkery.wait_for_text("Ln 1, Col 1", WAIT);
    assert_eq!(off_the_bar(&tinkery, Color::Default), Vec::<u16>::new());
}

#[test]
fn the_path_shows_its_directory_muted_and_its_name_strong() {
    let mut tinkery = open_guide();
    let status = &tinkery.screen()[usize::from(STATUS_ROW)];
    let slot: String = status.chars().skip(20).collect();
    // Either separator, as the platform writes it.
    assert!(
        slot.starts_with("docs/guide.md") || slot.starts_with("docs\\guide.md"),
        "{status:?}"
    );
    let name = tinkery
        .text_col(STATUS_ROW, "guide.md")
        .expect("the file name");
    assert_eq!(name, 20 + 5);
    assert_eq!(tinkery.fg_at(20, STATUS_ROW), MUTED);
    assert_eq!(tinkery.fg_at(name - 1, STATUS_ROW), MUTED);
    assert_eq!(tinkery.fg_at(name, STATUS_ROW), STRONG);
    assert!(!status.contains('•'), "{status:?}");

    // An edit adds ` •` in warn after the name.
    tinkery.type_text("z");
    tinkery.wait_for_screen("the dirty mark", WAIT, |screen| {
        screen[usize::from(STATUS_ROW)].contains("guide.md •")
    });
    let dot = tinkery.text_col(STATUS_ROW, "•").expect("the dirty mark");
    assert_eq!(dot, name + 9);
    assert_eq!(tinkery.fg_at(dot, STATUS_ROW), WARN);
}

#[test]
fn the_position_and_language_end_two_cells_from_the_right_edge() {
    let mut tinkery = Tinkery::spawn_with_config(TINKERY, &[]);
    tinkery.wait_for_text("Open directory", START);
    // Leave the splash, so typing reaches the untitled buffer.
    tinkery.send_keys("ctrl+n");
    tinkery.wait_for_text("Ln 1, Col 1", WAIT);
    // An untitled buffer has no language; it reads `Plain text`, 4 cells after
    // the position.
    let language = "Plain text";
    let lang_col = tinkery
        .text_col(STATUS_ROW, language)
        .expect("the language");
    assert_eq!(lang_col + language.len() as u16, COLS - 2);
    assert_eq!(tinkery.fg_at(lang_col, STATUS_ROW), TEXT);
    let position = "Ln 1, Col 1";
    let col = tinkery
        .text_col(STATUS_ROW, position)
        .expect("the position");
    assert_eq!(col + position.len() as u16 + 4, lang_col);
    assert_eq!(tinkery.fg_at(col, STATUS_ROW), TEXT);

    // A longer position grows to the left; the right end stays put.
    tinkery.type_text("abcdefghij");
    let position = "Ln 1, Col 11";
    tinkery.wait_for_text(position, WAIT);
    let col = tinkery
        .text_col(STATUS_ROW, position)
        .expect("the position");
    assert_eq!(col + position.len() as u16 + 4, lang_col);
    assert_eq!(
        tinkery.text_col(STATUS_ROW, language),
        Some(lang_col),
        "the language stays put"
    );
}

#[test]
fn a_rust_file_names_its_language() {
    let tinkery = Tinkery::spawn_in_with_config(Path::new(PROJECT), TINKERY, &["src/main.rs"]);
    tinkery.wait_for_text("Ln 1, Col 1", START);
    tinkery.wait_for_screen("Rust in the status bar", WAIT, |screen| {
        screen[usize::from(STATUS_ROW)].contains("Ln 1, Col 1    Rust")
    });
}
