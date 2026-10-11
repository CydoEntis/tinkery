//! `>settings`: config.toml in a tab, made from a template when missing, and
//! applied when saved there.

mod harness;

use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::time::Duration;

use harness::Tinkery;
use tempfile::TempDir;

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(10);

/// Tinkery on `a.txt` in `project`, with `TINKERY_CONFIG` pointing at `config`.
fn open_with_config(project: &Path, config: &Path) -> Tinkery {
    fs::write(project.join("a.txt"), "alpha\n").expect("write a.txt");
    let env = [("TINKERY_CONFIG", OsString::from(config))];
    let tinkery = Tinkery::spawn_in_with_env(project, &env, &["a.txt"]);
    tinkery.wait_for_text("alpha", START);
    tinkery
}

/// Ctrl+P, `>settings`, Enter.
fn run_settings(tinkery: &mut Tinkery) {
    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text("cast · files · commands", WAIT);
    tinkery.type_text(">settings");
    tinkery.wait_for_text("cast · commands", WAIT);
    tinkery.wait_for_screen("Settings listed", WAIT, |screen| {
        // The cast's first command row, under its `COMMANDS` label.
        screen[9].contains("Settings")
    });
    tinkery.send_keys("enter");
}

/// The tab row: the one holding a.txt's tab.
fn tab_row(tinkery: &Tinkery) -> String {
    tinkery
        .screen()
        .into_iter()
        .find(|row| row.contains("a.txt"))
        .unwrap_or_default()
}

#[test]
fn settings_creates_a_missing_config_from_the_template_and_opens_it() {
    let project = tempfile::tempdir().expect("create project");
    let home = tempfile::tempdir().expect("create config home");
    // Neither the folder nor the file exists yet.
    let config = home.path().join("tinkery").join("config.toml");
    let mut tinkery = open_with_config(project.path(), &config);

    run_settings(&mut tinkery);
    tinkery.wait_for_text("# tab_width = 4", WAIT);
    tinkery.wait_for_text("# insert_spaces = true", WAIT);
    tinkery.wait_for_text("# auto_pairs = true", WAIT);
    tinkery.wait_for_text("# theme = \"tinkery\"", WAIT);
    let written = fs::read_to_string(&config).expect("config.toml written");
    assert!(written.contains("[keys]"), "{written}");
    assert!(written.contains("# save = \"ctrl+s\""), "{written}");
    assert!(tab_row(&tinkery).contains("config.toml"));
}

#[test]
fn settings_opens_an_existing_config_and_switches_to_its_tab() {
    let project = tempfile::tempdir().expect("create project");
    let home = tempfile::tempdir().expect("create config home");
    let config = home.path().join("config.toml");
    fs::write(&config, "# mine\ntheme = \"hydra\"\n").expect("write config");
    let mut tinkery = open_with_config(project.path(), &config);

    run_settings(&mut tinkery);
    tinkery.wait_for_text("# mine", WAIT);
    // Back to a.txt, then `>settings` again: the same tab comes forward.
    tinkery.send_keys("alt+,");
    tinkery.wait_for_text_gone("# mine", WAIT);
    run_settings(&mut tinkery);
    tinkery.wait_for_text("# mine", WAIT);
    assert_eq!(tab_row(&tinkery).matches("config.toml").count(), 1);
    // An existing file is left as it was.
    assert_eq!(
        fs::read_to_string(&config).expect("read config"),
        "# mine\ntheme = \"hydra\"\n"
    );
}

/// Tinkery on `a.txt` with a config file holding `text`, opened in a tab
/// through `>settings`. The folders are returned to outlive the test.
fn editing_config(text: &str) -> (TempDir, TempDir, Tinkery) {
    let project = tempfile::tempdir().expect("create project");
    let home = tempfile::tempdir().expect("create config home");
    let config = home.path().join("config.toml");
    fs::write(&config, text).expect("write config");
    let mut tinkery = open_with_config(project.path(), &config);
    run_settings(&mut tinkery);
    tinkery.wait_for_text("[editor]", WAIT);
    (project, home, tinkery)
}

/// Back on a.txt, Tab at its start and Ctrl+S: what the Tab inserted.
fn tab_in_a(tinkery: &mut Tinkery, project: &Path) -> String {
    tinkery.send_keys("alt+,");
    tinkery.wait_for_text_gone("[editor]", WAIT);
    tinkery.send_keys("ctrl+home");
    tinkery.send_keys("tab");
    tinkery.send_keys("ctrl+s");
    tinkery.wait_for_text("saved a.txt", WAIT);
    let text = fs::read_to_string(project.join("a.txt")).expect("read a.txt");
    text.strip_suffix("alpha\n")
        .expect("the tab went before alpha")
        .to_string()
}

#[test]
fn saving_the_config_applies_tab_width_at_once() {
    let (project, _home, mut tinkery) = editing_config("[editor]\ntab_width = 4\n");
    // Ctrl+End lands below the last line; the 4 ends the line above.
    tinkery.send_keys("ctrl+end");
    tinkery.send_keys("up");
    tinkery.send_keys("end");
    tinkery.send_keys("backspace");
    tinkery.type_text("2");
    tinkery.wait_for_text("tab_width = 2", WAIT);
    tinkery.send_keys("ctrl+s");
    tinkery.wait_for_text("settings applied", WAIT);
    assert_eq!(tab_in_a(&mut tinkery, project.path()), "  ");
}

#[test]
fn a_config_that_fails_to_parse_keeps_the_settings_in_use() {
    let (project, _home, mut tinkery) = editing_config("[editor]\ntab_width = 2\n");
    tinkery.send_keys("ctrl+end");
    tinkery.type_text("= =");
    tinkery.wait_for_text("= =", WAIT);
    tinkery.send_keys("ctrl+s");
    tinkery.wait_for_text("config error: line 3", WAIT);
    assert_eq!(tab_in_a(&mut tinkery, project.path()), "  ");
}

#[test]
fn a_config_naming_a_bad_theme_keeps_the_settings_in_use() {
    let (project, _home, mut tinkery) =
        editing_config("theme = \"nord\"\n[editor]\ntab_width = 2\n");
    // Select `nord`, nine cells in, and type over it.
    tinkery.send_keys("ctrl+home");
    for _ in 0..9 {
        tinkery.send_keys("right");
    }
    for _ in 0..4 {
        tinkery.send_keys("shift+right");
    }
    tinkery.type_text("nope");
    tinkery.wait_for_text("theme = \"nope\"", WAIT);
    tinkery.send_keys("ctrl+s");
    tinkery.wait_for_text("config error: theme: unknown theme", WAIT);
    assert_eq!(tab_in_a(&mut tinkery, project.path()), "  ");
}
