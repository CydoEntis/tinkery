mod harness;

use std::time::Duration;

use harness::{CAP_L, CAP_R, COLS, ROWS, Tinkery};

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(5);
/// Three `foo`s: `one foo`, `two foo three`, `foo`.
const FIND: &str = "tests/fixtures/find.txt";
/// `foo Foo FOO` then `foooo`: four `foo`s ignoring case, two matching it.
const FIND_CASE: &str = "tests/fixtures/find_case.txt";
/// The default theme's accent, hydra's lime `#c3f53c`.
const ACCENT: vt100::Color = vt100::Color::Rgb(0xc3, 0xf5, 0x3c);
/// hydra's `acc_ink`, the text on an `on` chip.
const ACC_INK: vt100::Color = vt100::Color::Rgb(0x0a, 0x12, 0x04);
/// hydra's `raised2`, the bar's strip.
const RAISED2: vt100::Color = vt100::Color::Rgb(0x18, 0x24, 0x2f);
/// hydra's `btn`, an `off` chip.
const BTN: vt100::Color = vt100::Color::Rgb(0x1d, 0x2a, 0x37);
/// hydra's `strong`, the pattern and the count.
const STRONG: vt100::Color = vt100::Color::Rgb(0xf2, 0xf6, 0xf8);
/// hydra's `muted`, the text on an `off` chip.
const MUTED: vt100::Color = vt100::Color::Rgb(0x71, 0x80, 0x8f);
/// hydra's `err`, a bad pattern and why.
const ERR: vt100::Color = vt100::Color::Rgb(0xff, 0x6b, 0x6b);
/// hydra's `bg`, which the current match's text is drawn in, and the ground
/// the strip's round ends sit on.
const BG: vt100::Color = vt100::Color::Rgb(0x07, 0x0b, 0x10);
/// hydra's `warn`, behind the current match.
const WARN: vt100::Color = vt100::Color::Rgb(0xff, 0xb5, 0x47);
/// hydra's `find_match_bg`, `mix(bg, warn, .3)` (SPEC_V1_LAYOUT §13), behind
/// the other matches.
const MATCH: vt100::Color = vt100::Color::Rgb(0x51, 0x3e, 0x21);
/// The find bar sits on the row above the status line.
const BAR_ROW: u16 = ROWS - 2;
/// Text starts after the gutter `   1  `.
const TEXT_X: u16 = 6;

fn open(path: &str) -> Tinkery {
    let tinkery = Tinkery::spawn(&[path]);
    tinkery.wait_for_text("Ln 1, Col 1", START);
    tinkery
}

fn row(tinkery: &Tinkery, row: u16) -> String {
    tinkery.screen()[usize::from(row)].clone()
}

/// What the find bar's strip holds between its round ends.
fn bar(tinkery: &Tinkery) -> String {
    let row = row(tinkery, BAR_ROW);
    harness::strip_inside(&row)
        .unwrap_or_else(|| panic!("the find bar is a strip with round ends: {row:?}"))
        .to_string()
}

/// Waits for the find bar to be a strip from edge to edge that ends with
/// `count`, as `n/m` or an error.
fn wait_for_count(tinkery: &Tinkery, count: &str) {
    tinkery.wait_for_screen(&format!("the find bar to show {count:?}"), WAIT, |lines| {
        harness::strip_inside(&lines[usize::from(BAR_ROW)])
            .is_some_and(|bar| bar.starts_with(" Find  ") && bar.ends_with(&format!("  {count} ")))
    });
}

/// The same with square ends (`pill_caps` off, or `mono`): each end is a blank.
fn wait_for_square_count(tinkery: &Tinkery, count: &str) {
    tinkery.wait_for_screen(&format!("the find bar to show {count:?}"), WAIT, |lines| {
        let bar = &lines[usize::from(BAR_ROW)];
        bar.starts_with("  Find  ") && bar.ends_with(&format!(" Aa   .*   {count}"))
    });
    // No Nerd Font glyph anywhere on the screen.
    let screen = tinkery.screen().join("\n");
    assert!(
        !screen.contains(CAP_L) && !screen.contains(CAP_R),
        "{screen}"
    );
}

/// Waits for the status line to show `Ln <line>, Col <col>` (see `shows_position`).
fn wait_for_position(tinkery: &Tinkery, line: usize, col: usize) {
    let position = format!("Ln {line}, Col {col}");
    tinkery.wait_for_screen(&position, WAIT, |lines| {
        harness::shows_position(&lines[usize::from(ROWS - 1)], &position)
    });
}

fn find(tinkery: &mut Tinkery, text: &str) {
    tinkery.send_keys("ctrl+f");
    tinkery.wait_for_text(" Find  ", WAIT);
    tinkery.type_text(text);
    // A prefix of the text may already show the same count, so wait for all of it.
    tinkery.wait_for_text(&format!(" Find  {text}"), WAIT);
}

#[test]
fn ctrl_f_highlights_every_match_as_you_type_and_jumps_to_the_first() {
    let mut tinkery = open(FIND);
    tinkery.wait_for_text("3  foo", WAIT);
    find(&mut tinkery, "foo");
    wait_for_count(&tinkery, "1/3");
    assert!(bar(&tinkery).starts_with(" Find  foo"));
    let chips = format!("{CAP_L}Aa{CAP_R} {CAP_L}.*{CAP_R}  1/3 ");
    assert!(bar(&tinkery).ends_with(&chips));
    // The bar is a strip of `raised2` whose round ends sit on `bg`; the
    // pattern and count are in `strong`.
    for x in [0, COLS - 1] {
        assert_eq!(tinkery.fg_at(x, BAR_ROW), RAISED2, "{x}");
        assert_eq!(tinkery.bg_at(x, BAR_ROW), BG, "{x}");
    }
    assert_eq!(tinkery.fg_at(8, BAR_ROW), STRONG);
    let count = tinkery.text_col(BAR_ROW, "1/3").expect("the count shows");
    assert_eq!(tinkery.fg_at(count, BAR_ROW), STRONG);
    // The chips are `muted` on `btn` while off, their round ends `btn` on the
    // strip; everything else between the strip's ends is on `raised2`.
    assert_eq!(tinkery.bg_text(BAR_ROW, BTN), "Aa.*");
    assert_eq!(tinkery.fg_text(BAR_ROW, MUTED), "Aa.*");
    // (A blank has no colour of its own to read, so only the glyphs count.)
    let caps = format!("{CAP_L}{CAP_R}{CAP_L}{CAP_R}");
    assert_eq!(tinkery.fg_text(BAR_ROW, BTN).replace(' ', ""), caps);
    assert_eq!(
        tinkery.bg_text(BAR_ROW, RAISED2).chars().count(),
        usize::from(COLS) - 2 - "Aa.*".len()
    );
    assert_eq!(tinkery.bold_text(BAR_ROW), "");
    // The bar sits above the status line, which stays on the last row.
    assert!(row(&tinkery, ROWS - 1).starts_with(" ✦ tinkery"));
    // The current match is `bg` on `warn`, the others on `find_match_bg`, and
    // nothing else has either.
    tinkery.wait_for_bg_text(3, WARN, "foo", WAIT);
    tinkery.wait_for_bg_text(4, MATCH, "foo", WAIT);
    tinkery.wait_for_bg_text(5, MATCH, "foo", WAIT);
    assert_eq!(tinkery.bg_text(3, MATCH), "");
    assert_eq!(tinkery.text_col(3, "foo"), Some(TEXT_X + 4));
    assert_eq!(tinkery.fg_at(TEXT_X + 4, 3), BG);
    // The other matches keep the text's own colour.
    let plain = tinkery.fg_at(TEXT_X, 4);
    assert_eq!(tinkery.fg_at(TEXT_X + 4, 4), plain);
    // Matches aren't drawn as a selection.
    for row in 3..=5 {
        assert_eq!(tinkery.reversed_text(row), "");
    }
    wait_for_position(&tinkery, 1, 5);
    // The terminal cursor is in the bar, after the text.
    tinkery.wait_for_cursor(11, BAR_ROW, WAIT);

    // Matches update while typing: only line 2 has `foo` before a space.
    tinkery.type_text(" ");
    wait_for_count(&tinkery, "1/1");
    tinkery.wait_for_bg_text(4, WARN, "foo ", WAIT);
    tinkery.wait_for_bg_text(3, MATCH, "", WAIT);
    tinkery.wait_for_bg_text(3, WARN, "", WAIT);
    tinkery.wait_for_bg_text(5, MATCH, "", WAIT);
    wait_for_position(&tinkery, 2, 5);
    tinkery.send_keys("backspace");
    wait_for_count(&tinkery, "1/3");
}

#[test]
fn ctrl_f_starts_from_the_selection() {
    let mut tinkery = open(FIND);
    tinkery.wait_for_text("3  foo", WAIT);
    tinkery.send_keys("down");
    for _ in 0..3 {
        tinkery.send_keys("shift+right");
    }
    tinkery.wait_for_reversed(4, "two", WAIT);
    tinkery.send_keys("ctrl+f");
    wait_for_count(&tinkery, "1/1");
    assert!(bar(&tinkery).starts_with(" Find  two"));
    tinkery.wait_for_bg_text(4, WARN, "two", WAIT);
    wait_for_position(&tinkery, 2, 1);
}

#[test]
fn enter_and_shift_enter_step_through_matches_wrapping() {
    let mut tinkery = open(FIND);
    find(&mut tinkery, "foo");
    wait_for_count(&tinkery, "1/3");
    wait_for_position(&tinkery, 1, 5);

    tinkery.send_keys("enter");
    wait_for_count(&tinkery, "2/3");
    wait_for_position(&tinkery, 2, 5);
    tinkery.send_keys("enter");
    wait_for_count(&tinkery, "3/3");
    wait_for_position(&tinkery, 3, 1);
    tinkery.send_keys("enter");
    wait_for_count(&tinkery, "1/3");
    wait_for_position(&tinkery, 1, 5);

    tinkery.send_keys("shift+enter");
    wait_for_count(&tinkery, "3/3");
    wait_for_position(&tinkery, 3, 1);
    tinkery.send_keys("shift+enter");
    wait_for_count(&tinkery, "2/3");
    wait_for_position(&tinkery, 2, 5);
    // Enter in the bar never edits the buffer: still three lines plus the empty
    // one after the final line break.
    tinkery.wait_for_text("3  foo", WAIT);
    // No fifth line number in the gutter.
    assert!(!tinkery.screen().join("\n").contains("   5"));
}

/// Waits for the chips that are on to read `chips`: `acc_ink` on `accent`,
/// bold, between round ends of `accent`. A chip that is off is on `btn`.
fn wait_for_chips(tinkery: &Tinkery, chips: &str) {
    tinkery.wait_for_bg_text(BAR_ROW, ACCENT, chips, WAIT);
    assert_eq!(tinkery.fg_text(BAR_ROW, ACC_INK), chips);
    assert_eq!(tinkery.bold_text(BAR_ROW), chips);
    let caps = if chips.is_empty() {
        String::new()
    } else {
        format!("{CAP_L}{CAP_R}")
    };
    assert_eq!(tinkery.fg_text(BAR_ROW, ACCENT).replace(' ', ""), caps);
    let off = "Aa.*".replace(chips, "");
    assert_eq!(tinkery.bg_text(BAR_ROW, BTN), off);
}

#[test]
fn alt_c_toggles_case_and_alt_r_toggles_regex_and_a_bad_regex_says_so() {
    let mut tinkery = open(FIND_CASE);
    tinkery.wait_for_text("2  foooo", WAIT);
    find(&mut tinkery, "foo");
    wait_for_count(&tinkery, "1/4");
    tinkery.wait_for_bg_text(3, WARN, "foo", WAIT);
    tinkery.wait_for_bg_text(3, MATCH, "FooFOO", WAIT);
    wait_for_chips(&tinkery, "");

    tinkery.send_keys("alt+c");
    wait_for_count(&tinkery, "1/2");
    wait_for_chips(&tinkery, "Aa");
    tinkery.wait_for_bg_text(3, MATCH, "", WAIT);
    tinkery.wait_for_bg_text(3, WARN, "foo", WAIT);
    tinkery.wait_for_bg_text(4, MATCH, "foo", WAIT);
    tinkery.send_keys("alt+c");
    wait_for_count(&tinkery, "1/4");
    wait_for_chips(&tinkery, "");

    // `fo+` is literal text until regex is on.
    for _ in 0..3 {
        tinkery.send_keys("backspace");
    }
    tinkery.type_text("fo+");
    wait_for_count(&tinkery, "0/0");
    tinkery.wait_for_bg_text(3, WARN, "", WAIT);
    tinkery.wait_for_bg_text(3, MATCH, "", WAIT);
    tinkery.send_keys("alt+r");
    wait_for_count(&tinkery, "1/4");
    wait_for_chips(&tinkery, ".*");
    tinkery.wait_for_bg_text(4, MATCH, "foooo", WAIT);

    // An unclosed group is an error, with nothing highlighted.
    tinkery.type_text("(");
    wait_for_count(&tinkery, "invalid regex");
    // Both the pattern and the reason are in `err`.
    tinkery.wait_for_fg(BAR_ROW, ERR, "fo+(invalid regex", WAIT);
    for row in 3..=4 {
        tinkery.wait_for_bg_text(row, WARN, "", WAIT);
        tinkery.wait_for_bg_text(row, MATCH, "", WAIT);
    }
    tinkery.send_keys("backspace");
    wait_for_count(&tinkery, "1/4");
}

#[test]
fn esc_closes_the_bar_and_keeps_the_cursor_on_the_current_match() {
    let mut tinkery = open(FIND);
    find(&mut tinkery, "foo");
    wait_for_count(&tinkery, "1/3");
    tinkery.send_keys("enter");
    wait_for_count(&tinkery, "2/3");

    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(" Find  ", WAIT);
    for row in 3..=4 {
        tinkery.wait_for_bg_text(row, WARN, "", WAIT);
        tinkery.wait_for_bg_text(row, MATCH, "", WAIT);
    }
    wait_for_position(&tinkery, 2, 5);
    tinkery.wait_for_cursor(TEXT_X + 4, 4, WAIT);
    // Typing goes into the buffer again, at the match.
    tinkery.type_text("x");
    tinkery.wait_for_text("2  two xfoo three", WAIT);
}

#[test]
fn mono_underlines_matches_and_reverses_the_current_one() {
    // `mono` can't blend `find_match_bg`, so matches are told apart by weight.
    let mut tinkery = Tinkery::spawn_with_config("theme = \"mono\"\n", &[FIND]);
    tinkery.wait_for_text("Ln 1, Col 1", START);
    tinkery.wait_for_text("3  foo", WAIT);
    find(&mut tinkery, "foo");
    // `mono` never rounds an end: the bar has no Nerd Font glyph in it.
    wait_for_square_count(&tinkery, "1/3");
    tinkery.wait_for_reversed(3, "foo", WAIT);
    tinkery.wait_for_underlined(4, "foo", WAIT);
    tinkery.wait_for_underlined(5, "foo", WAIT);
    assert_eq!(tinkery.underlined_text(3), "");
    assert_eq!(tinkery.reversed_text(4), "");

    tinkery.send_keys("enter");
    wait_for_square_count(&tinkery, "2/3");
    tinkery.wait_for_reversed(4, "foo", WAIT);
    tinkery.wait_for_underlined(3, "foo", WAIT);
    tinkery.wait_for_reversed(3, "", WAIT);

    // An `on` chip is bold, and an `off` one is not the dark grey it sits on,
    // which `mono`'s `muted` is.
    tinkery.send_keys("alt+c");
    tinkery.wait_for_bold(BAR_ROW, "Aa", WAIT);
    let off = tinkery
        .text_col(BAR_ROW, ".*")
        .expect("the regex chip shows");
    assert_ne!(tinkery.fg_at(off, BAR_ROW), tinkery.bg_at(off, BAR_ROW));
}

#[test]
fn pill_caps_off_squares_the_bar_and_its_chips() {
    let mut tinkery = Tinkery::spawn_with_config("[ui]\npill_caps = false\n", &[FIND]);
    tinkery.wait_for_text("Ln 1, Col 1", START);
    tinkery.wait_for_text("3  foo", WAIT);
    find(&mut tinkery, "foo");
    wait_for_square_count(&tinkery, "1/3");
    // Nothing moved: the pattern and the cursor are where round ends put them.
    assert!(row(&tinkery, BAR_ROW).starts_with("  Find  foo "));
    tinkery.wait_for_cursor(11, BAR_ROW, WAIT);
    // A square end is a blank of the pill's own colour: the strip's are
    // `raised2`, a chip's its fill.
    for x in [0, COLS - 1] {
        assert_eq!(tinkery.bg_at(x, BAR_ROW), RAISED2, "{x}");
    }
    assert_eq!(tinkery.bg_text(BAR_ROW, BTN), " Aa  .* ");
    assert_eq!(
        tinkery.bg_text(BAR_ROW, RAISED2).chars().count(),
        usize::from(COLS) - " Aa  .* ".len()
    );
    tinkery.send_keys("alt+c");
    tinkery.wait_for_bg_text(BAR_ROW, ACCENT, " Aa ", WAIT);
    assert_eq!(tinkery.fg_text(BAR_ROW, ACC_INK).trim(), "Aa");
    assert_eq!(tinkery.bold_text(BAR_ROW), "Aa");
    assert_eq!(tinkery.bg_text(BAR_ROW, BTN), " .* ");
    wait_for_square_count(&tinkery, "1/3");

    // The replace bar is the same strip.
    tinkery.send_keys("ctrl+r");
    tinkery.wait_for_text("│ Replace  ", WAIT);
    wait_for_square_count(&tinkery, "1/3");
    assert_eq!(tinkery.bg_at(0, BAR_ROW), RAISED2);
}
