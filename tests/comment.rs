mod harness;

use std::fs;
use std::time::Duration;

use harness::Tinkery;
use tempfile::TempDir;

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(5);

/// Opens `name` holding `contents` in a fresh temp dir, with tinkery running there.
fn open(name: &str, contents: &str) -> (TempDir, Tinkery) {
    let dir = tempfile::tempdir().expect("create temp dir");
    fs::write(dir.path().join(name), contents).expect("write file");
    let tinkery = Tinkery::spawn_in(dir.path(), &[name]);
    tinkery.wait_for_text(name, START);
    tinkery.wait_for_text("Ln 1, Col 1", START);
    (dir, tinkery)
}

/// Ctrl+/ comments the line and a second press takes the comment off, whichever
/// of the two bound keys sends it.
fn toggles_a_rust_line(key: &str) {
    let (_dir, mut tinkery) = open("a.rs", "let x = 1;\n");
    tinkery.wait_for_text("1  let x = 1;", WAIT);
    tinkery.send_keys(key);
    tinkery.wait_for_text("1  // let x = 1;", WAIT);
    tinkery.send_keys(key);
    tinkery.wait_for_text_gone("//", WAIT);
    tinkery.wait_for_text("1  let x = 1;", WAIT);
}

#[test]
fn ctrl_slash_comments_and_uncomments_a_rust_line() {
    toggles_a_rust_line("ctrl+/");
}

#[test]
fn ctrl_7_comments_and_uncomments_a_rust_line() {
    toggles_a_rust_line("ctrl+7");
}

#[test]
fn a_file_with_no_language_has_no_comments() {
    let (_dir, mut tinkery) = open("a.txt", "plain words\n");
    tinkery.wait_for_text("1  plain words", WAIT);
    tinkery.send_keys("ctrl+/");
    tinkery.wait_for_text("no comments for this file", WAIT);
    let screen = tinkery.screen();
    assert!(
        screen.iter().any(|row| row.contains("1  plain words")),
        "{screen:#?}"
    );
    assert!(
        !screen.iter().any(|row| row.contains("a.txt •")),
        "{screen:#?}"
    );
}

#[test]
fn ctrl_slash_wraps_an_html_line_and_unwraps_it() {
    let (_dir, mut tinkery) = open("a.html", "<p>hi</p>\n");
    tinkery.wait_for_text("1  <p>hi</p>", WAIT);
    tinkery.send_keys("ctrl+/");
    tinkery.wait_for_text("1  <!-- <p>hi</p> -->", WAIT);
    tinkery.send_keys("ctrl+/");
    tinkery.wait_for_text_gone("<!--", WAIT);
    tinkery.wait_for_text("1  <p>hi</p>", WAIT);
}

#[test]
fn ctrl_slash_wraps_a_css_line_and_unwraps_it() {
    let (_dir, mut tinkery) = open("a.css", "p { color: red; }\n");
    tinkery.wait_for_text("1  p { color: red; }", WAIT);
    tinkery.send_keys("ctrl+/");
    tinkery.wait_for_text("1  /* p { color: red; } */", WAIT);
    tinkery.send_keys("ctrl+/");
    tinkery.wait_for_text_gone("/*", WAIT);
    tinkery.wait_for_text("1  p { color: red; }", WAIT);
}
