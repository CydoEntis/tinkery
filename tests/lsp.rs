//! The language server client against `fake_lsp`, the scripted server in
//! `src/bin/fake_lsp.rs`, driven through the real binary.

mod harness;

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use harness::{CAP_L, CAP_R, ROWS, Tinkery};
use serde_json::Value;
use tempfile::TempDir;

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(10);

/// `config.toml` pointing Rust files at `command`. A literal string, so Windows
/// backslashes stay as they are.
fn rust_server(command: &str) -> String {
    format!("[lsp.rust]\ncommand = '{command}'\n")
}

fn fake() -> &'static str {
    env!("CARGO_BIN_EXE_fake_lsp")
}

fn status_line(tinkery: &Tinkery) -> String {
    tinkery.screen()[usize::from(ROWS - 1)].clone()
}

/// A project holding `a.rs` and `b.rs`, with the fake's log (and script, if any)
/// outside it so the tree never shows them.
struct Project {
    dir: TempDir,
    files: TempDir,
}

impl Project {
    fn new(script: Option<&str>) -> Self {
        let dir = tempfile::tempdir().expect("create project dir");
        fs::write(dir.path().join("a.rs"), "fn a() {}\n").expect("write a.rs");
        fs::write(dir.path().join("b.rs"), "fn b() {}\n").expect("write b.rs");
        let files = tempfile::tempdir().expect("create log dir");
        if let Some(script) = script {
            fs::write(files.path().join("script.json"), script).expect("write script");
        }
        Self { dir, files }
    }

    fn log_path(&self) -> PathBuf {
        self.files.path().join("log.jsonl")
    }

    fn env(&self) -> Vec<(&'static str, OsString)> {
        let mut env = vec![("FAKE_LSP_LOG", self.log_path().into_os_string())];
        let script = self.files.path().join("script.json");
        if script.exists() {
            env.push(("FAKE_LSP_SCRIPT", script.into_os_string()));
        }
        env
    }

    fn open(&self, config: &str, file: &str) -> Tinkery {
        let tinkery =
            Tinkery::spawn_in_with_config_and_env(self.dir.path(), config, &self.env(), &[file]);
        tinkery.wait_for_text("Ln 1, Col 1", START);
        tinkery
    }

    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.dir.path().join(name)).expect("read project file")
    }

    /// Every logged message, as `(pid, message)`.
    fn log(&self) -> Vec<(u64, Value)> {
        let Ok(text) = fs::read_to_string(self.log_path()) else {
            return Vec::new();
        };
        text.lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .map(|entry| (entry["pid"].as_u64().unwrap_or(0), entry["message"].clone()))
            .collect()
    }

    /// Whether the fake has received `method` for a URI ending in `/<file>`.
    fn received(&self, method: &str, file: &str) -> bool {
        self.log()
            .iter()
            .any(|(_, message)| is(message, method, file))
    }

    fn wait_for(&self, tinkery: &Tinkery, method: &str, file: &str) {
        tinkery.wait_for_files(&format!("{method} for {file}"), WAIT, || {
            self.received(method, file)
        });
    }
}

fn is(message: &Value, method: &str, file: &str) -> bool {
    message["method"] == method
        && message["params"]["textDocument"]["uri"]
            .as_str()
            .is_some_and(|uri| uri.ends_with(&format!("/{file}")))
}

fn method(message: &Value) -> &str {
    message["method"].as_str().unwrap_or("")
}

#[test]
fn syncs_document() {
    let project = Project::new(None);
    let mut tinkery = project.open(&rust_server(fake()), "a.rs");
    project.wait_for(&tinkery, "textDocument/didOpen", "a.rs");

    tinkery.type_text("x");
    project.wait_for(&tinkery, "textDocument/didChange", "a.rs");
    tinkery.send_keys("ctrl+s");
    tinkery.wait_for_text("saved", WAIT);
    project.wait_for(&tinkery, "textDocument/didSave", "a.rs");

    // A second Rust file in the same root goes to the same server.
    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text("cast · files · commands", WAIT);
    tinkery.type_text("b.rs");
    tinkery.wait_for_text("✦ b.rs", WAIT);
    // The query echoes before the file walk fills the list; Enter on an empty
    // list does nothing, so wait for b.rs's own row to be selected.
    tinkery.wait_for_screen("b.rs's row in the picker", WAIT, |lines| {
        lines.iter().any(|line| line.contains("› b.rs "))
    });
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone("cast · files · commands", WAIT);
    project.wait_for(&tinkery, "textDocument/didOpen", "b.rs");

    // Back to a.rs, which is saved, and close it.
    tinkery.send_keys("alt+,");
    tinkery.send_keys("ctrl+w");
    project.wait_for(&tinkery, "textDocument/didClose", "a.rs");

    let log = project.log();
    let pids: std::collections::BTreeSet<u64> = log.iter().map(|(pid, _)| *pid).collect();
    assert_eq!(pids.len(), 1, "one server process for both files: {log:#?}");
    let inits = log
        .iter()
        .filter(|(_, m)| method(m) == "initialize")
        .count();
    assert_eq!(inits, 1, "{log:#?}");

    // a.rs's life, in order, after the handshake.
    let mut seen: Vec<&str> = Vec::new();
    for (_, message) in &log {
        let name = method(message);
        let about_a = message["params"]["textDocument"]["uri"]
            .as_str()
            .is_some_and(|uri| uri.ends_with("/a.rs"));
        if name == "initialize" || name == "initialized" || about_a {
            seen.push(name);
        }
    }
    assert_eq!(
        seen,
        [
            "initialize",
            "initialized",
            "textDocument/didOpen",
            "textDocument/didChange",
            "textDocument/didSave",
            "textDocument/didClose",
        ],
        "{log:#?}"
    );

    // The fake advertises incremental sync, so the change is just the `x`.
    let (_, open) = log
        .iter()
        .find(|(_, m)| is(m, "textDocument/didOpen", "a.rs"))
        .expect("didOpen logged");
    assert_eq!(open["params"]["textDocument"]["text"], "fn a() {}\n");
    assert_eq!(open["params"]["textDocument"]["languageId"], "rust");
    let (_, change) = log
        .iter()
        .find(|(_, m)| is(m, "textDocument/didChange", "a.rs"))
        .expect("didChange logged");
    let edit = &change["params"]["contentChanges"][0];
    assert_eq!(edit["text"], "x");
    assert_eq!(edit["range"]["start"]["line"], 0);
    assert_eq!(edit["range"]["start"]["character"], 0);
    assert_eq!(change["params"]["textDocument"]["version"], 1);
    assert_eq!(project.read("a.rs"), "xfn a() {}\n");
}

#[test]
fn full_sync_sends_the_whole_text() {
    let script = r#"{"responses": {"initialize": {"capabilities": {"textDocumentSync": 1}}}}"#;
    let project = Project::new(Some(script));
    let mut tinkery = project.open(&rust_server(fake()), "a.rs");
    project.wait_for(&tinkery, "textDocument/didOpen", "a.rs");
    tinkery.type_text("y");
    project.wait_for(&tinkery, "textDocument/didChange", "a.rs");
    let log = project.log();
    let (_, change) = log
        .iter()
        .find(|(_, m)| is(m, "textDocument/didChange", "a.rs"))
        .expect("didChange logged");
    let edit = &change["params"]["contentChanges"][0];
    assert_eq!(edit["text"], "yfn a() {}\n");
    assert!(edit.get("range").is_none(), "{edit}");
}

#[test]
fn missing_server_keeps_editing() {
    let project = Project::new(None);
    let mut tinkery = project.open(&rust_server("tinkery-no-such-server"), "a.rs");
    let message = "rust: server not found (tinkery-no-such-server)";
    tinkery.wait_for_text(message, WAIT);

    tinkery.type_text("abc");
    tinkery.wait_for_text("a.rs •", WAIT);
    tinkery.send_keys("ctrl+s");
    tinkery.wait_for_text("saved a.rs", WAIT);
    assert_eq!(project.read("a.rs"), "abcfn a() {}\n");

    // Another Rust file doesn't try again or say it again.
    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text("cast · files · commands", WAIT);
    tinkery.type_text("b.rs");
    tinkery.wait_for_text("✦ b.rs", WAIT);
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone("cast · files · commands", WAIT);
    // The save's toast took the first "not found" down. Opening b.rs syncs
    // the server in the same frame, so a second one would be on screen now.
    assert_eq!(tinkery.text_col(ROWS - 1, "b.rs"), Some(20));
    let screen = tinkery.screen();
    assert!(
        !screen.join(" ").contains("server not found"),
        "{screen:#?}"
    );
    tinkery.type_text("z");
    tinkery.wait_for_text("b.rs •", WAIT);
}

/// npm installs servers as `.cmd` shims; Tinkery has to find one through
/// `PATHEXT` and start it, as `--health` already finds it.
#[cfg(windows)]
#[test]
fn a_cmd_server_on_path_starts() {
    let project = Project::new(None);
    let bin = tempfile::tempdir().expect("create shim dir");
    fs::write(
        bin.path().join("tinkery-cmd-server.cmd"),
        format!(
            "@\"{}\" %*
",
            fake()
        ),
    )
    .expect("write shim");
    let mut path = OsString::from(bin.path());
    if let Some(rest) = std::env::var_os("PATH") {
        path.push(";");
        path.push(rest);
    }
    let mut env = project.env();
    env.push(("PATH", path));
    let mut tinkery = Tinkery::spawn_in_with_config_and_env(
        project.dir.path(),
        &rust_server("tinkery-cmd-server"),
        &env,
        &["a.rs"],
    );
    tinkery.wait_for_text("Ln 1, Col 1", START);
    project.wait_for(&tinkery, "textDocument/didOpen", "a.rs");
    let log = project.log();
    assert!(
        log.iter().any(|(_, m)| method(m) == "initialize"),
        "{log:#?}"
    );
    let screen = tinkery.screen();
    assert!(
        !screen.join(" ").contains("server not found"),
        "{screen:#?}"
    );
    // Quit cleanly so the fake behind the shim exits too.
    tinkery.send_keys("ctrl+q");
    tinkery.wait_exit(WAIT);
}

#[test]
fn crashed_server_keeps_editing() {
    let project = Project::new(Some(r#"{"exit_on": "textDocument/didChange"}"#));
    let mut tinkery = project.open(&rust_server(fake()), "a.rs");
    project.wait_for(&tinkery, "textDocument/didOpen", "a.rs");

    tinkery.type_text("q");
    tinkery.wait_for_text("rust: server crashed (exit code 3)", WAIT);

    tinkery.type_text("rs");
    tinkery.wait_for_text("qrsfn a()", WAIT);
    tinkery.send_keys("ctrl+s");
    tinkery.wait_for_text("saved a.rs", WAIT);
    assert_eq!(project.read("a.rs"), "qrsfn a() {}\n");
    // Nothing more reached the dead server.
    let changes = project
        .log()
        .iter()
        .filter(|(_, m)| method(m) == "textDocument/didChange")
        .count();
    assert_eq!(changes, 1);
}

// aurora's roles, for the server segment's colours.
const AURORA_OK: vt100::Color = vt100::Color::Rgb(0x7f, 0xe3, 0xc9);
const AURORA_ERR: vt100::Color = vt100::Color::Rgb(0xff, 0x6f, 0x91);
const AURORA_MUTED: vt100::Color = vt100::Color::Rgb(0x67, 0x62, 0x7d);
const AURORA_TEXT: vt100::Color = vt100::Color::Rgb(0xa6, 0xa2, 0xbb);

/// `rust_server(command)` under the aurora theme. The theme key goes first, since
/// after `[lsp.rust]` it would belong to that table.
fn aurora_server(command: &str) -> String {
    format!("theme = \"aurora\"\n{}", rust_server(command))
}

/// The status line's column of `text`, which must be on it.
fn status_col(tinkery: &Tinkery, text: &str) -> u16 {
    tinkery
        .text_col(ROWS - 1, text)
        .unwrap_or_else(|| panic!("{text:?} on the status line: {:?}", status_line(tinkery)))
}

#[test]
fn a_ready_server_shows_after_the_language() {
    let project = Project::new(None);
    let tinkery = project.open(&aurora_server(fake()), "a.rs");
    project.wait_for(&tinkery, "textDocument/didOpen", "a.rs");
    tinkery.wait_for_text("Rust    ● fake_lsp", WAIT);

    let status = status_line(&tinkery);
    assert!(
        status.ends_with("Ln 1, Col 1    Rust    ● fake_lsp"),
        "{status:?}"
    );
    // It ends two cells from the right edge, like every right-hand segment.
    let name = status_col(&tinkery, "fake_lsp");
    assert_eq!(name + 8, harness::COLS - 2);
    assert_eq!(tinkery.fg_at(name - 2, ROWS - 1), AURORA_OK);
    assert_eq!(tinkery.fg_at(name, ROWS - 1), AURORA_TEXT);
    assert_eq!(
        tinkery.fg_at(status_col(&tinkery, "Rust"), ROWS - 1),
        AURORA_TEXT
    );
}

#[test]
fn a_missing_server_shows_no_server_in_muted() {
    let project = Project::new(None);
    let mut tinkery = project.open(&aurora_server("tinkery-no-such-server"), "a.rs");
    // The message is a toast, so the bar keeps its path and full right side.
    tinkery.wait_for_toast("⚠ rust: server not found (tinkery-no-such-server)", WAIT);
    tinkery.send_keys("right");
    tinkery.wait_for_text("Ln 1, Col 2    Rust    ○ no server", WAIT);
    let label = status_col(&tinkery, "no server");
    assert_eq!(tinkery.fg_at(label - 2, ROWS - 1), AURORA_MUTED);
    assert_eq!(tinkery.fg_at(label, ROWS - 1), AURORA_MUTED);
    assert!(!status_line(&tinkery).contains("tinkery-no-such-server"));
}

#[test]
fn a_crashed_server_shows_its_name_in_err() {
    let project = Project::new(Some(r#"{"exit_on": "textDocument/didChange"}"#));
    let mut tinkery = project.open(&aurora_server(fake()), "a.rs");
    project.wait_for(&tinkery, "textDocument/didOpen", "a.rs");
    tinkery.wait_for_text("● fake_lsp", WAIT);

    tinkery.type_text("q");
    tinkery.wait_for_text("rust: server crashed (exit code 3)", WAIT);
    tinkery.send_keys("left");
    tinkery.wait_for_text("Rust    ✕ fake_lsp", WAIT);
    let name = status_col(&tinkery, "fake_lsp");
    assert_eq!(tinkery.fg_at(name - 2, ROWS - 1), AURORA_ERR);
    assert_eq!(tinkery.fg_at(name, ROWS - 1), AURORA_ERR);
}

#[test]
fn a_file_without_a_language_says_plain_text_and_has_no_server() {
    let project = Project::new(None);
    fs::write(project.dir.path().join("notes.txt"), "hi\n").expect("write notes.txt");
    let tinkery = project.open(&aurora_server(fake()), "notes.txt");
    let status = status_line(&tinkery);
    assert!(status.ends_with("Ln 1, Col 1    Plain text"), "{status:?}");
    assert!(!status.contains("fake_lsp"), "{status:?}");
}

#[test]
fn quitting_shuts_the_server_down() {
    let project = Project::new(None);
    let mut tinkery = project.open(&rust_server(fake()), "a.rs");
    project.wait_for(&tinkery, "textDocument/didOpen", "a.rs");
    tinkery.send_keys("ctrl+q");
    tinkery.wait_exit(WAIT);
    tinkery.wait_for_files("shutdown then exit as the last messages", WAIT, || {
        let methods: Vec<String> = project
            .log()
            .iter()
            .map(|(_, m)| method(m).to_string())
            .collect();
        methods.ends_with(&["shutdown".to_string(), "exit".to_string()])
    });
}

#[test]
fn opening_another_folder_shuts_the_old_server_down_and_starts_a_new_one() {
    let project = Project::new(None);
    let other = tempfile::tempdir().expect("create the other project");
    let other_name = other
        .path()
        .file_name()
        .expect("temp folders have names")
        .to_string_lossy()
        .into_owned();
    fs::write(other.path().join("c.rs"), "fn c() {}\n").expect("write c.rs");
    let mut tinkery = project.open(&rust_server(fake()), "a.rs");
    project.wait_for(&tinkery, "textDocument/didOpen", "a.rs");
    let first = project
        .log()
        .first()
        .map(|(pid, _)| *pid)
        .expect("the first server logged");

    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text("cast · files · commands", WAIT);
    tinkery.type_text(">open directory");
    tinkery.wait_for_text("cast · commands", WAIT);
    tinkery.wait_for_text("Open directory", WAIT);
    tinkery.send_keys("enter");
    tinkery.wait_for_text("open · folders", WAIT);
    tinkery.type_text(&other.path().display().to_string());
    tinkery.send_keys("enter");
    // The project holds no folders either, so wait for the query to clear:
    // an Enter sent before then could arrive in the same paste-like burst.
    tinkery.wait_for_screen("the other folder in the browser", WAIT, |screen| {
        screen[8].contains(&other_name) && screen[9].contains("no folders here")
    });
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone("open · folders", WAIT);

    tinkery.wait_for_files("the old server told to shut down and exit", WAIT, || {
        let methods: Vec<String> = project
            .log()
            .iter()
            .filter(|(pid, _)| *pid == first)
            .map(|(_, m)| method(m).to_string())
            .collect();
        methods.ends_with(&["shutdown".to_string(), "exit".to_string()])
    });
    assert!(project.received("textDocument/didClose", "a.rs"));

    // A file in the new folder starts a server of its own, rooted there.
    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text("cast · files · commands", WAIT);
    tinkery.type_text("c.rs");
    tinkery.wait_for_text("✦ c.rs", WAIT);
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone("cast · files · commands", WAIT);
    project.wait_for(&tinkery, "textDocument/didOpen", "c.rs");
    let log = project.log();
    let (second, _) = log
        .iter()
        .find(|(_, m)| is(m, "textDocument/didOpen", "c.rs"))
        .expect("didOpen logged");
    assert_ne!(*second, first, "{log:#?}");
    let (_, init) = log
        .iter()
        .find(|(pid, m)| pid == second && method(m) == "initialize")
        .expect("the new server initialized");
    let root = init["params"]["rootUri"].as_str().unwrap_or_default();
    assert!(root.ends_with(&other_name), "{root}");
}

/// A Rust file with something to complain about on lines 2 and 3.
const DIAG_FILE: &str = "fn main() {\n    let x = 1;\n    let y = 2;\n}\n";

/// The theme with the severity colours pinned, so they're known.
fn diag_config() -> String {
    format!(
        "{}[theme_overrides]\nerr = \"#ff0000\"\nworking = \"#ffaa00\"\ninfo = \"#0000ff\"\n\
         err_soft = \"#880000\"\nwarn_soft = \"#885500\"\ninfo_soft = \"#000088\"\n",
        rust_server(fake())
    )
}

const ERR: vt100::Color = vt100::Color::Rgb(0xff, 0x00, 0x00);
const WORKING: vt100::Color = vt100::Color::Rgb(0xff, 0xaa, 0x00);
const INFO: vt100::Color = vt100::Color::Rgb(0x00, 0x00, 0xff);
const ERR_SOFT: vt100::Color = vt100::Color::Rgb(0x88, 0x00, 0x00);
const WARN_SOFT: vt100::Color = vt100::Color::Rgb(0x88, 0x55, 0x00);
const INFO_SOFT: vt100::Color = vt100::Color::Rgb(0x00, 0x00, 0x88);

/// A `publishDiagnostics` for the file just received, one entry per
/// `(line, start, end, severity, message)`.
fn publish(diagnostics: &[(u32, u32, u32, u8, &str)]) -> String {
    let list: Vec<String> = diagnostics
        .iter()
        .map(|(line, start, end, severity, message)| {
            format!(
                r#"{{"range": {{"start": {{"line": {line}, "character": {start}}},
                   "end": {{"line": {line}, "character": {end}}}}},
                   "severity": {severity}, "message": "{message}"}}"#
            )
        })
        .collect();
    format!(
        r#"{{"method": "textDocument/publishDiagnostics",
            "params": {{"uri": "$uri", "diagnostics": [{}]}}}}"#,
        list.join(", ")
    )
}

/// A warning on `x` (line 2) and an error on `y` (line 3).
fn both() -> String {
    publish(&[
        (1, 8, 9, 2, "unused variable: x"),
        (2, 8, 9, 1, "mismatched types"),
    ])
}

fn diag_project(script: &str) -> (Project, Tinkery) {
    let project = Project::new(Some(script));
    fs::write(project.dir.path().join("c.rs"), DIAG_FILE).expect("write c.rs");
    let tinkery = project.open(&diag_config(), "c.rs");
    (project, tinkery)
}

// Screen rows: the tab header takes rows 0-2, so buffer line n is row n + 2. The
// gutter "   1  " is 6 cells, so char column c is screen column 6 + c.
const X_ROW: u16 = 4;
const Y_ROW: u16 = 5;
const VAR_COL: u16 = 14;

#[test]
fn diagnostics_are_underlined_marked_and_counted() {
    let script = format!(r#"{{"notify": {{"textDocument/didOpen": [{}]}}}}"#, both());
    let (_project, tinkery) = diag_project(&script);
    tinkery.wait_for_text("✕ 1  ⚠ 1", WAIT);

    tinkery.wait_for_underlined(Y_ROW, "y", WAIT);
    tinkery.wait_for_underlined(X_ROW, "x", WAIT);
    // Nothing else is underlined.
    assert_eq!(tinkery.underlined_text(3), "");
    assert_eq!(tinkery.underlined_text(6), "");
    // The underline is curly and in the severity's colour. vt100 keeps neither,
    // so the bytes are checked. The Windows pseudo-console re-encodes the output
    // and drops both, so there only the straight underline above reaches us.
    if cfg!(not(windows)) {
        tinkery.wait_for_output(b"\x1b[58:2::255:0:0m\x1b[4:3my", WAIT);
        tinkery.wait_for_output(b"\x1b[58:2::255:170:0m\x1b[4:3mx", WAIT);
    }
    // The text keeps its own colour.
    assert_ne!(tinkery.fg_at(VAR_COL, Y_ROW), ERR);
    assert_ne!(tinkery.fg_at(VAR_COL, X_ROW), WORKING);

    // The gutter marks both lines in their colour, and only them.
    let screen = tinkery.screen();
    assert!(
        screen[usize::from(X_ROW)].starts_with("◆  2  "),
        "{screen:#?}"
    );
    assert!(
        screen[usize::from(Y_ROW)].starts_with("◆  3  "),
        "{screen:#?}"
    );
    assert!(screen[3].starts_with("   1  "), "{screen:#?}");
    assert_eq!(tinkery.fg_at(0, X_ROW), WORKING);
    assert_eq!(tinkery.fg_at(0, Y_ROW), ERR);

    // The counts on the status line are in the same colours.
    let status = status_line(&tinkery);
    assert!(status.contains("✕ 1  ⚠ 1"), "{status:?}");
    let status_row = ROWS - 1;
    let warn = tinkery.text_col(status_row, "⚠ 1").expect("warning count");
    let err = tinkery.text_col(status_row, "✕ 1").expect("error count");
    assert_eq!(tinkery.fg_at(warn, status_row), WORKING);
    assert_eq!(tinkery.fg_at(err, status_row), ERR);
}

/// `    let x = 1;` is 14 cells and the text starts at column 6, so a lens on it
/// starts 4 cells past its end.
const LENS_COL: u16 = 6 + 14 + 4;

#[test]
fn the_lens_shows_the_most_severe_message_after_the_line() {
    // Line 2 has a warning and then an error; line 1 an information.
    let script = format!(
        r#"{{"notify": {{"textDocument/didOpen": [{}]}}}}"#,
        publish(&[
            (0, 3, 7, 3, "main is fine"),
            (1, 8, 9, 2, "unused variable: x"),
            (1, 12, 13, 1, "expected bool"),
            (2, 8, 9, 2, "unused variable: y"),
        ])
    );
    let (_project, tinkery) = diag_project(&script);
    tinkery.wait_for_text("◈ expected bool", WAIT);

    // The error wins over the warning before it, for the lens and the mark.
    assert_eq!(tinkery.text_col(X_ROW, "◈ expected bool"), Some(LENS_COL));
    assert!(!tinkery.screen()[usize::from(X_ROW)].contains("unused"));
    assert_eq!(tinkery.fg_at(0, X_ROW), ERR);
    assert_eq!(tinkery.fg_at(LENS_COL, X_ROW), ERR);
    assert_eq!(tinkery.fg_at(LENS_COL + 2, X_ROW), ERR_SOFT);
    assert!(tinkery.italic_at(LENS_COL + 2, X_ROW));
    assert!(!tinkery.italic_at(LENS_COL, X_ROW));

    assert_eq!(
        tinkery.text_col(Y_ROW, "◈ unused variable: y"),
        Some(LENS_COL)
    );
    assert_eq!(tinkery.fg_at(LENS_COL, Y_ROW), WORKING);
    assert_eq!(tinkery.fg_at(LENS_COL + 2, Y_ROW), WARN_SOFT);

    // `fn main() {` is 11 cells.
    let info_col = 6 + 11 + 4;
    assert_eq!(tinkery.text_col(3, "◈ main is fine"), Some(info_col));
    assert_eq!(tinkery.fg_at(0, 3), INFO);
    assert_eq!(tinkery.fg_at(info_col, 3), INFO);
    assert_eq!(tinkery.fg_at(info_col + 2, 3), INFO_SOFT);
    assert!(tinkery.italic_at(info_col + 2, 3));
}

#[test]
fn a_long_message_is_cut_and_a_long_line_has_no_lens() {
    // Line 1 ends at column 76, so its lens starts at 80: 16 cells are left for
    // the message. Line 2 ends at 84, and a lens at 88 would be too close to the
    // right edge (100 - 12).
    let near = format!("let a = \"{}\";", "a".repeat(59));
    let far = format!("let b = \"{}\";", "b".repeat(67));
    assert_eq!((near.len(), far.len()), (70, 78));
    let message = "this message is much too long to fit";
    let script = format!(
        r#"{{"notify": {{"textDocument/didOpen": [{}]}}}}"#,
        publish(&[(0, 4, 5, 1, message), (1, 4, 5, 1, message)])
    );
    let project = Project::new(Some(&script));
    fs::write(project.dir.path().join("c.rs"), format!("{near}\n{far}\n")).expect("write c.rs");
    let tinkery = project.open(&diag_config(), "c.rs");
    tinkery.wait_for_text("◈ this message is…", WAIT);

    assert_eq!(tinkery.text_col(3, "◈ this message is…"), Some(80));
    let screen = tinkery.screen();
    assert!(
        screen[3].trim_end().ends_with("◈ this message is…"),
        "{screen:#?}"
    );
    // The long line is still marked and underlined, but has no lens.
    tinkery.wait_for_underlined(4, "b", WAIT);
    assert!(screen[4].starts_with("◆  2  "), "{screen:#?}");
    assert!(!screen[4].contains('◈'), "{screen:#?}");
}

#[test]
fn mono_draws_the_lens_message_in_muted() {
    let script = format!(r#"{{"notify": {{"textDocument/didOpen": [{}]}}}}"#, both());
    let project = Project::new(Some(&script));
    fs::write(project.dir.path().join("c.rs"), DIAG_FILE).expect("write c.rs");
    let config = format!("theme = \"mono\"\n{}", rust_server(fake()));
    let tinkery = project.open(&config, "c.rs");
    tinkery.wait_for_text("◈ mismatched types", WAIT);

    // Mono's `muted` is the terminal's dark grey; its `err` is red.
    let muted = vt100::Color::Idx(8);
    assert_eq!(tinkery.fg_at(LENS_COL, Y_ROW), vt100::Color::Idx(1));
    assert_eq!(tinkery.fg_at(LENS_COL + 2, Y_ROW), muted);
    assert!(tinkery.italic_at(LENS_COL + 2, Y_ROW));
    assert_eq!(tinkery.fg_at(LENS_COL + 2, X_ROW), muted);
}

#[test]
fn f8_and_shift_f8_cycle_through_diagnostics() {
    let script = format!(r#"{{"notify": {{"textDocument/didOpen": [{}]}}}}"#, both());
    let (_project, mut tinkery) = diag_project(&script);
    tinkery.wait_for_text("✕ 1  ⚠ 1", WAIT);

    tinkery.send_keys("f8");
    tinkery.wait_for_cursor(VAR_COL, X_ROW, WAIT);
    tinkery.wait_for_text("Ln 2, Col 9", WAIT);
    tinkery.send_keys("f8");
    tinkery.wait_for_cursor(VAR_COL, Y_ROW, WAIT);
    // Past the last one, back to the first.
    tinkery.send_keys("f8");
    tinkery.wait_for_cursor(VAR_COL, X_ROW, WAIT);
    // Before the first one, round to the last.
    tinkery.send_keys("shift+f8");
    tinkery.wait_for_cursor(VAR_COL, Y_ROW, WAIT);
    tinkery.send_keys("shift+f8");
    tinkery.wait_for_cursor(VAR_COL, X_ROW, WAIT);
}

#[test]
fn the_diagnostic_under_the_cursor_shows_its_message() {
    let script = format!(r#"{{"notify": {{"textDocument/didOpen": [{}]}}}}"#, both());
    let (_project, mut tinkery) = diag_project(&script);
    tinkery.wait_for_text("✕ 1  ⚠ 1", WAIT);
    assert!(!status_line(&tinkery).contains("unused variable"));

    tinkery.send_keys("f8");
    tinkery.wait_for_text("⚠ unused variable: x", WAIT);
    tinkery.send_keys("f8");
    tinkery.wait_for_text("✕ mismatched types", WAIT);
    // The lenses keep both messages in the text area; the status line drops one.
    tinkery.wait_for_screen("the warning gone from the status line", WAIT, |lines| {
        !lines[usize::from(ROWS - 1)].contains("unused variable")
    });
    // It holds the path slot, its glyph in the severity's colour: a
    // diagnostic is no toast.
    let status_row = ROWS - 1;
    assert_eq!(tinkery.text_col(status_row, "✕ mismatched types"), Some(20));
    assert_eq!(tinkery.fg_at(20, status_row), ERR);
    assert!(!tinkery.screen()[usize::from(harness::TOAST_ROW)].contains("mismatched types"));

    // Off the diagnostic, the message goes and the path comes back.
    tinkery.send_keys("right");
    tinkery.wait_for_text("Ln 3, Col 10", WAIT);
    tinkery.wait_for_screen("the error gone from the status line", WAIT, |lines| {
        !lines[usize::from(ROWS - 1)].contains("mismatched types")
    });
    assert_eq!(tinkery.text_col(status_row, "c.rs"), Some(20));
}

#[test]
fn a_new_publish_replaces_the_set_and_an_empty_one_clears_it() {
    let only_error = publish(&[(2, 8, 9, 1, "mismatched types")]);
    let script = format!(
        r#"{{"notify": {{
            "textDocument/didOpen": [{}],
            "textDocument/didChange": [{only_error}],
            "textDocument/didSave": [{}]
        }}}}"#,
        both(),
        publish(&[])
    );
    let (project, mut tinkery) = diag_project(&script);
    tinkery.wait_for_text("✕ 1  ⚠ 1", WAIT);
    tinkery.wait_for_underlined(X_ROW, "x", WAIT);

    // Typing on line 1 sends a change; the server's new set has only the error.
    tinkery.type_text("z");
    project.wait_for(&tinkery, "textDocument/didChange", "c.rs");
    tinkery.wait_for_text("✕ 1  ⚠ 0", WAIT);
    tinkery.wait_for_underlined(X_ROW, "", WAIT);
    tinkery.wait_for_underlined(Y_ROW, "y", WAIT);
    assert!(tinkery.screen()[usize::from(X_ROW)].starts_with("   2  "));

    // Saving gets an empty publish: everything goes.
    tinkery.send_keys("ctrl+s");
    tinkery.wait_for_text("saved c.rs", WAIT);
    tinkery.wait_for_text_gone("✕", WAIT);
    tinkery.wait_for_underlined(Y_ROW, "", WAIT);
    assert!(tinkery.screen()[usize::from(Y_ROW)].starts_with("   3  "));
    assert!(!status_line(&tinkery).contains("⚠"));
}

/// The fake's answer to every `textDocument/definition`: `greet` in `util.rs`.
const DEFINITION_IN_UTIL: &str = r#"{"responses": {"textDocument/definition": {
    "uri": "$dir/util.rs",
    "range": {"start": {"line": 2, "character": 7}, "end": {"line": 2, "character": 12}}
}}}"#;

/// A project holding the `tests/fixtures/definition` files, with `main.rs` open:
/// `util::greet()` on its line 4 calls `greet`, defined on `util.rs` line 3.
fn definition_project(script: &str) -> (Project, Tinkery) {
    let project = Project::new(Some(script));
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/definition");
    for name in ["main.rs", "util.rs"] {
        fs::copy(fixtures.join(name), project.dir.path().join(name)).expect("copy fixture");
    }
    let tinkery = project.open(&rust_server(fake()), "main.rs");
    project.wait_for(&tinkery, "textDocument/didOpen", "main.rs");
    (project, tinkery)
}

// `greet` in `    util::greet();` starts at char 10 of line 4, which is screen
// column 6 + 10 past the gutter, on row 6 below the tab header.
const GREET_COL: u16 = 16;
const GREET_ROW: u16 = 6;

/// The position of the last `textDocument/definition` the fake received, as
/// (line, character).
fn definition_position(project: &Project) -> (u64, u64) {
    let log = project.log();
    let (_, request) = log
        .iter()
        .rev()
        .find(|(_, m)| is(m, "textDocument/definition", "main.rs"))
        .expect("definition request logged");
    let position = &request["params"]["position"];
    (
        position["line"].as_u64().unwrap_or(u64::MAX),
        position["character"].as_u64().unwrap_or(u64::MAX),
    )
}

#[test]
fn f12_opens_the_definition_in_another_file_and_alt_left_comes_back() {
    let (project, mut tinkery) = definition_project(DEFINITION_IN_UTIL);
    // One key at a time: a burst of keys can read as a paste on Windows.
    for (key, at) in [
        ("down", "Ln 2, Col 1"),
        ("down", "Ln 3, Col 1"),
        ("down", "Ln 4, Col 1"),
        ("ctrl+right", "Ln 4, Col 9"),
        ("ctrl+right", "Ln 4, Col 11"),
    ] {
        tinkery.send_keys(key);
        tinkery.wait_for_text(at, WAIT);
    }

    tinkery.send_keys("f12");
    project.wait_for(&tinkery, "textDocument/definition", "main.rs");
    assert_eq!(definition_position(&project), (3, 10));
    // util.rs opens in a tab of its own, cursor on `greet`.
    tinkery.wait_for_text("Ln 3, Col 8", WAIT);
    let tabs = tinkery.screen()[1].clone();
    assert!(
        tabs.contains("main.rs") && tabs.contains("util.rs"),
        "{tabs:?}"
    );
    tinkery.wait_for_cursor(6 + 7, 5, WAIT);
    assert!(status_line(&tinkery).contains("util.rs"));

    tinkery.send_keys("alt+left");
    tinkery.wait_for_text("Ln 4, Col 11", WAIT);
    tinkery.wait_for_cursor(GREET_COL, GREET_ROW, WAIT);
    assert!(status_line(&tinkery).contains("main.rs"));

    // Nothing more to go back to: it stays put.
    tinkery.send_keys("alt+left");
    tinkery.send_keys("right");
    tinkery.wait_for_text("Ln 4, Col 12", WAIT);
    assert!(status_line(&tinkery).contains("main.rs"));
}

#[test]
fn ctrl_click_goes_to_the_definition() {
    let (project, mut tinkery) = definition_project(DEFINITION_IN_UTIL);
    tinkery.ctrl_click(GREET_COL + 2, GREET_ROW);
    project.wait_for(&tinkery, "textDocument/definition", "main.rs");
    assert_eq!(definition_position(&project), (3, 12));
    tinkery.wait_for_text("Ln 3, Col 8", WAIT);
    assert!(status_line(&tinkery).contains("util.rs"));

    // Back to where the click put the cursor.
    tinkery.send_keys("alt+left");
    tinkery.wait_for_text("Ln 4, Col 13", WAIT);
    assert!(status_line(&tinkery).contains("main.rs"));
}

#[test]
fn no_definition_says_so() {
    // Unscripted, the fake answers `null`.
    let (project, mut tinkery) = definition_project("{}");
    tinkery.send_keys("f12");
    project.wait_for(&tinkery, "textDocument/definition", "main.rs");
    tinkery.wait_for_text("No definition found", WAIT);
    // The message is a toast at the top right of the editor; the path keeps
    // its slot and the cursor stays where it was.
    assert_eq!(
        tinkery.text_col(harness::TOAST_ROW, "⚠ No definition found"),
        Some(harness::toast_col("No definition found"))
    );
    assert_eq!(tinkery.text_col(ROWS - 1, "main.rs"), Some(20));
    assert!(status_line(&tinkery).contains("Ln 1, Col 1"));
    assert!(!tinkery.screen()[1].contains("util.rs"));
}

/// The fake's answer to every `textDocument/definition`: the start of `main.rs`.
const DEFINITION_AT_TOP: &str = r#"{"responses": {"textDocument/definition": {
    "uri": "$dir/main.rs",
    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 1}}
}}}"#;

#[test]
fn the_jump_list_keeps_the_last_50_places() {
    let project = Project::new(Some(DEFINITION_AT_TOP));
    fs::write(
        project.dir.path().join("main.rs"),
        "abcdefghij\n".repeat(27),
    )
    .expect("write main.rs");
    let mut tinkery = project.open(&rust_server(fake()), "main.rs");
    project.wait_for(&tinkery, "textDocument/didOpen", "main.rs");

    let text = tinkery.text_col(3, "abcdefghij").expect("line 1 on screen");
    // 55 different places, each left by F12: lines 2 to 26 at columns 2, 4 and 6.
    let places: Vec<(u16, u16)> = (0..55u16).map(|i| (2 + i % 25, 2 + 2 * (i / 25))).collect();
    for &(line, col) in &places {
        // Line n is on screen row n + 2, below the tab header.
        tinkery.click(text + col - 1, line + 2);
        tinkery.wait_for_text(&format!("Ln {line}, Col {col}"), WAIT);
        tinkery.send_keys("f12");
        tinkery.wait_for_text("Ln 1, Col 1", WAIT);
    }

    // Back through the newest 50, newest first.
    for &(line, col) in places.iter().rev().take(50) {
        tinkery.send_keys("alt+left");
        tinkery.wait_for_text(&format!("Ln {line}, Col {col}"), WAIT);
    }
    // The 50th-newest is as far as it goes: the five before it were dropped.
    let (line, col) = places[5];
    for _ in 0..5 {
        tinkery.send_keys("alt+left");
    }
    tinkery.send_keys("right");
    tinkery.wait_for_text(&format!("Ln {line}, Col {}", col + 1), WAIT);
    assert!(status_line(&tinkery).contains(&format!("Ln {line}, Col {}", col + 1)));
}

// The popups are read in the default theme, tinkery, which a config naming no
// theme runs on. A popup's card is `raised`; its edge and the hover's rule are
// `line2`, and the edge sits on the editor's `bg`.
const CARD: vt100::Color = vt100::Color::Rgb(0x1f, 0x1b, 0x19);
const LINE2: vt100::Color = vt100::Color::Rgb(0x40, 0x36, 0x2f);
const BG: vt100::Color = vt100::Color::Rgb(0x11, 0x0f, 0x0e);
const HOV: vt100::Color = vt100::Color::Rgb(0x3a, 0x30, 0x2a);
const ACCENT: vt100::Color = vt100::Color::Rgb(0xff, 0x8a, 0x3d);
const FG: vt100::Color = vt100::Color::Rgb(0xdc, 0xd2, 0xc7);
const TEXT: vt100::Color = vt100::Color::Rgb(0xb5, 0xa8, 0x9d);
const STRONG: vt100::Color = vt100::Color::Rgb(0xf8, 0xf0, 0xe6);
const MUTED: vt100::Color = vt100::Color::Rgb(0x7d, 0x70, 0x67);
/// `[ui]` asking for a card's edge as rounded lines rather than flush blocks.
const ROUNDED: &str = "[ui]\ncorners = \"rounded\"\n";

/// The fake's answer to every `textDocument/hover`: markdown with a code fence
/// and a paragraph too long for one line of the popup.
const HOVER_MARKDOWN: &str = r#"{"responses": {"textDocument/hover": {"contents": {
    "kind": "markdown",
    "value": "```rust\npub fn greet()\n```\n\nGreets whoever is listening, then keeps on talking for quite a while so this line wraps."
}}}}"#;

/// The cells of `row` from column `from` up to `to`, blanks included: `screen`
/// drops the blanks a row ends in, and a flush card's corners are blank.
fn cells(screen: &[String], row: u16, from: u16, to: u16) -> String {
    let line = &screen[usize::from(row)];
    line.chars()
        .chain(std::iter::repeat(' '))
        .skip(usize::from(from))
        .take(usize::from(to - from))
        .collect()
}

/// A definition project (cursor still at the top) in tinkery, its cursor
/// moved onto `greet` on line 4.
fn hover_project(script: &str) -> (Project, Tinkery) {
    hover_project_with(script, "")
}

/// `hover_project` with `extra` added to its config.
fn hover_project_with(script: &str, extra: &str) -> (Project, Tinkery) {
    let project = Project::new(Some(script));
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/definition");
    for name in ["main.rs", "util.rs"] {
        fs::copy(fixtures.join(name), project.dir.path().join(name)).expect("copy fixture");
    }
    let config = format!("{}{extra}", rust_server(fake()));
    let mut tinkery = project.open(&config, "main.rs");
    project.wait_for(&tinkery, "textDocument/didOpen", "main.rs");
    for (key, at) in [
        ("down", "Ln 2, Col 1"),
        ("down", "Ln 3, Col 1"),
        ("down", "Ln 4, Col 1"),
        ("ctrl+right", "Ln 4, Col 9"),
        ("ctrl+right", "Ln 4, Col 11"),
    ] {
        tinkery.send_keys(key);
        tinkery.wait_for_text(at, WAIT);
    }
    (project, tinkery)
}

// Below the cursor on `greet` (16, 6): the card's edge on row 7, then the text
// one cell in from the edge, on rows 8 to 11.
const HOVER_TEXT_COL: u16 = 18;
const HOVER_FIRST_ROW: u16 = 8;

#[test]
fn alt_k_shows_the_hover_below_the_cursor() {
    let (project, mut tinkery) = hover_project(HOVER_MARKDOWN);
    tinkery.send_keys("alt+k");
    project.wait_for(&tinkery, "textDocument/hover", "main.rs");
    let (_, request) = project
        .log()
        .into_iter()
        .rfind(|(_, m)| is(m, "textDocument/hover", "main.rs"))
        .expect("hover request logged");
    assert_eq!(
        request["params"]["position"],
        serde_json::json!({"line": 3, "character": 10})
    );

    tinkery.wait_for_text("pub fn greet()", WAIT);
    let screen = tinkery.screen();
    assert!(!screen.iter().any(|row| row.contains("```")), "{screen:#?}");
    // Plain text, fence gone, the paragraph wrapped at the popup's 60 columns.
    for (row, text) in [
        (HOVER_FIRST_ROW, "pub fn greet()"),
        (
            HOVER_FIRST_ROW + 2,
            "Greets whoever is listening, then keeps on talking for quite",
        ),
        (HOVER_FIRST_ROW + 3, "a while so this line wraps."),
    ] {
        assert_eq!(
            tinkery.text_col(row, text),
            Some(HOVER_TEXT_COL),
            "{text:?} on row {row}: {screen:#?}"
        );
    }
    assert_eq!(tinkery.bg_at(HOVER_TEXT_COL, HOVER_FIRST_ROW), CARD);
    assert_eq!(tinkery.bg_at(HOVER_TEXT_COL + 40, HOVER_FIRST_ROW), CARD);
    // The code in `fg`, the docs in `text`.
    assert_eq!(tinkery.fg_at(HOVER_TEXT_COL, HOVER_FIRST_ROW), FG);
    assert_eq!(tinkery.fg_at(HOVER_TEXT_COL, HOVER_FIRST_ROW + 2), TEXT);
    assert_eq!(tinkery.fg_at(HOVER_TEXT_COL, HOVER_FIRST_ROW + 3), TEXT);
    // The cursor stays in the text, on `greet`.
    tinkery.wait_for_cursor(GREET_COL, GREET_ROW, WAIT);
    let status = status_line(&tinkery);
    assert!(
        status.contains("main.rs") && status.contains("Ln 4, Col 11"),
        "{status:?}"
    );
}

/// Opens the hover of a `hover_project` under `extra` config and checks its
/// card, from (16, 7) to (79, 12): 60 columns of text plus an edge and a cell
/// of padding each side; the code, the rule, two rows of docs. `top` and
/// `bottom` are the edge rows as drawn, `sides` the two side glyphs, and
/// `rule` the row between the code and the docs.
fn assert_hover_card(extra: &str, top: &str, bottom: &str, sides: (&str, &str), rule: &str) {
    let (_project, mut tinkery) = hover_project_with(HOVER_MARKDOWN, extra);
    // Every cell around the card, from the tab header down past its bottom:
    // the rows above and below it, and the columns either side of it.
    let (left, right, top_row, bottom_row) = (16, 79, HOVER_FIRST_ROW - 1, HOVER_FIRST_ROW + 4);
    let outside: Vec<(u16, u16)> = (3..14)
        .flat_map(|row| (0..100).map(move |col| (col, row)))
        .filter(|&(col, row)| {
            !((left..=right).contains(&col) && (top_row..=bottom_row).contains(&row))
        })
        .collect();
    let colours = |tinkery: &Tinkery| -> Vec<_> {
        outside
            .iter()
            .map(|&(col, row)| (tinkery.fg_at(col, row), tinkery.bg_at(col, row)))
            .collect()
    };
    let before = colours(&tinkery);
    let around = |tinkery: &Tinkery| -> Vec<String> {
        let screen = tinkery.screen();
        (3..14)
            .map(|row| {
                let beside = (top_row..=bottom_row).contains(&row);
                cells(&screen, row, 0, if beside { left } else { 100 })
            })
            .collect()
    };
    let text_before = around(&tinkery);

    tinkery.send_keys("alt+k");
    tinkery.wait_for_text("pub fn greet()", WAIT);
    let screen = tinkery.screen();
    let row_of = |row: u16| cells(&screen, row, left, right + 1);
    assert_eq!(row_of(top_row), top, "{screen:#?}");
    assert_eq!(row_of(bottom_row), bottom, "{screen:#?}");
    assert_eq!(row_of(HOVER_FIRST_ROW + 1), rule, "{screen:#?}");
    // The edge is `line2` on the editor's ground; nothing of the text it
    // covers is left in it.
    for row in [top_row, bottom_row] {
        for col in [left, left + 30, right] {
            assert_eq!(tinkery.fg_at(col, row), LINE2, "{col},{row}");
            assert_eq!(tinkery.bg_at(col, row), BG, "{col},{row}");
        }
    }
    for row in HOVER_FIRST_ROW..bottom_row {
        if row != HOVER_FIRST_ROW + 1 {
            let line = row_of(row);
            assert!(
                line.starts_with(sides.0) && line.ends_with(sides.1),
                "{line:?}"
            );
        }
        for col in [left, right] {
            assert_eq!(tinkery.fg_at(col, row), LINE2, "{col},{row}");
            assert_eq!(tinkery.bg_at(col, row), BG, "{col},{row}");
        }
    }
    // The rule takes the blank line between the code and the docs, on the card.
    assert_eq!(tinkery.fg_at(left + 30, HOVER_FIRST_ROW + 1), LINE2);
    assert_eq!(tinkery.bg_at(left + 30, HOVER_FIRST_ROW + 1), CARD);
    // Not a dialog: everything around the card keeps its text and colours.
    assert_eq!(colours(&tinkery), before);
    assert_eq!(around(&tinkery), text_before);
}

#[test]
fn the_hover_is_a_flush_card_with_a_rule_under_the_code_and_dims_nothing() {
    // Eighth blocks on the side of each cell that faces the card, with blank
    // corners; the rule runs between the two side edges.
    assert_hover_card(
        "",
        &format!(" {} ", "▁".repeat(62)),
        &format!(" {} ", "▔".repeat(62)),
        ("▕", "▏"),
        &format!("▕{}▏", "─".repeat(62)),
    );
}

#[test]
fn a_rounded_hover_card_joins_its_rule_to_the_side_edges() {
    assert_hover_card(
        ROUNDED,
        &format!("╭{}╮", "─".repeat(62)),
        &format!("╰{}╯", "─".repeat(62)),
        ("│", "│"),
        &format!("├{}┤", "─".repeat(62)),
    );
}

#[test]
fn esc_movement_and_typing_close_the_hover() {
    let (_project, mut tinkery) = hover_project(HOVER_MARKDOWN);

    tinkery.send_keys("alt+k");
    tinkery.wait_for_text("pub fn greet()", WAIT);
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone("pub fn greet()", WAIT);
    assert!(status_line(&tinkery).contains("Ln 4, Col 11"));

    tinkery.send_keys("alt+k");
    tinkery.wait_for_text("pub fn greet()", WAIT);
    tinkery.send_keys("right");
    tinkery.wait_for_text("Ln 4, Col 12", WAIT);
    tinkery.wait_for_text_gone("pub fn greet()", WAIT);

    tinkery.send_keys("alt+k");
    tinkery.wait_for_text("pub fn greet()", WAIT);
    tinkery.type_text("z");
    tinkery.wait_for_text("util::gzreet();", WAIT);
    tinkery.wait_for_text_gone("pub fn greet()", WAIT);
}

#[test]
fn an_empty_hover_shows_nothing() {
    // The hover reply is `null`; a publish sent right after it marks when the
    // reply has been handled.
    let script = format!(
        r#"{{"notify": {{"textDocument/hover": [{}]}}}}"#,
        publish(&[(0, 0, 2, 2, "after the hover")])
    );
    let (project, mut tinkery) = hover_project(&script);
    let before = tinkery.screen();
    tinkery.send_keys("alt+k");
    project.wait_for(&tinkery, "textDocument/hover", "main.rs");
    tinkery.wait_for_text("✕ 0  ⚠ 1", WAIT);

    // No card anywhere: the text area is as it was, but for the gutter mark and
    // the lens the publish put on line 1.
    let after = tinkery.screen();
    let line = after[3].replacen('◆', " ", 1);
    let lens = line.find('◈').expect("a lens on line 1");
    assert_eq!(line[..lens].trim_end(), before[3].trim_end(), "{after:#?}");
    assert_eq!(&after[..3], &before[..3]);
    assert_eq!(
        &after[4..usize::from(ROWS - 1)],
        &before[4..usize::from(ROWS - 1)]
    );
    let status = status_line(&tinkery);
    assert!(
        status.contains("main.rs") && status.contains("Ln 4, Col 11"),
        "{status:?}"
    );
}

#[test]
fn the_hover_flips_above_and_shifts_left_at_the_screen_edges() {
    let script = r#"{"responses": {"textDocument/hover": {"contents": {
        "kind": "plaintext",
        "value": "fn last() -> u32\n\nReturns the last value, counting back from the very end."
    }}}}"#;
    let project = Project::new(Some(script));
    // 40 lines, the last 85 characters long with no line break after it.
    let mut text = "// filler\n".repeat(39);
    text.push_str(&format!("fn last() -> u32 {{ 7 }} {}", "/".repeat(85 - 23)));
    assert_eq!(text.lines().last().map(str::len), Some(85));
    fs::write(project.dir.path().join("long.rs"), &text).expect("write long.rs");
    let mut tinkery = project.open(&rust_server(fake()), "long.rs");
    project.wait_for(&tinkery, "textDocument/didOpen", "long.rs");

    tinkery.send_keys("ctrl+end");
    tinkery.wait_for_text("Ln 40, Col 86", WAIT);
    // The gutter "  40  " is 6 cells; the last editor row is just above the status.
    let cursor = (6 + 85, ROWS - 2);
    tinkery.wait_for_cursor(cursor.0, cursor.1, WAIT);

    tinkery.send_keys("alt+k");
    let doc = "Returns the last value, counting back from the very end.";
    tinkery.wait_for_text(doc, WAIT);
    // 56 columns of text in a 60-wide card, pushed left to end at the right
    // edge, and flipped above: edge, three text rows, edge ending just over
    // the cursor's row.
    let row = cursor.1 - 2;
    let col = tinkery.text_col(row, doc);
    assert_eq!(col, Some(100 - 60 + 2), "{:#?}", tinkery.screen());
    assert_eq!(tinkery.text_col(row - 2, "fn last() -> u32"), Some(42));
    assert_eq!(tinkery.bg_at(42, row), CARD);
    // The card's own corners: (40, 24) to (99, 28).
    let screen = tinkery.screen();
    for (edge_row, glyph) in [(row - 3, "▁"), (row + 1, "▔")] {
        let line = cells(&screen, edge_row, 40, 100);
        assert_eq!(line, format!(" {} ", glyph.repeat(58)), "{screen:#?}");
    }
    // The cursor's line and the status line are untouched.
    let screen = tinkery.screen();
    assert!(
        screen[usize::from(cursor.1)].starts_with("  40  fn last()"),
        "{screen:#?}"
    );
    assert!(status_line(&tinkery).contains("Ln 40, Col 86"));
    tinkery.wait_for_cursor(cursor.0, cursor.1, WAIT);
}

/// Whether `row` of a card whose rows start `shift` cells right of the usual is
/// the selected one: a pill of `hov` from the card's inside, `›` a cell into
/// it in `accent`, and its label in `strong` bold from column `col`.
fn is_selected(tinkery: &Tinkery, row: u16, shift: u16, col: u16) -> bool {
    let pill = KIND_COL - 3 + shift;
    tinkery.text_col(row, CAP_L) == Some(pill)
        && tinkery.fg_at(pill, row) == HOV
        && tinkery.bg_at(pill, row) == CARD
        && tinkery.text_col(row, "›") == Some(pill + 1)
        && tinkery.fg_at(pill + 1, row) == ACCENT
        && tinkery.bg_at(pill + 1, row) == HOV
        && tinkery.bg_at(col, row) == HOV
        && tinkery.fg_at(col, row) == STRONG
        && tinkery.bold_at(col, row)
}

/// Initialize advertising `.` as a completion trigger, then 12 items for every
/// `textDocument/completion`, sorted by label: `len` carries a `textEdit` from
/// the cursor after `s.` on line 2 and a detail, `push` a snippet, `push_str` plain insert
/// text and `trim` only its label.
fn completion_script(extra: &str) -> String {
    let method = |label: &str| format!(r#"{{"label": "{label}", "kind": 2}}"#);
    let mut items: Vec<String> = [
        "capacity",
        "chars",
        "clear",
        "contains",
        "ends_with",
        "is_empty",
        "lines",
    ]
    .iter()
    .map(|l| method(l))
    .collect();
    items.extend([
        r#"{"label": "as_str", "kind": 5}"#.to_string(),
        r#"{"label": "len", "kind": 2, "detail": "fn(&self) -> usize", "textEdit": {"range": {
            "start": {"line": 1, "character": 6}, "end": {"line": 1, "character": 6}},
            "newText": "len()"}}"#
            .to_string(),
        r#"{"label": "push", "kind": 2, "insertTextFormat": 2,
            "insertText": "push(${1:ch})$0"}"#
            .to_string(),
        r#"{"label": "push_str", "kind": 2, "insertText": "push_str"}"#.to_string(),
        r#"{"label": "trim", "kind": 3}"#.to_string(),
    ]);
    format!(
        r#"{{"responses": {{
            "initialize": {{"capabilities": {{
                "textDocumentSync": {{"openClose": true, "change": 2, "save": {{"includeText": false}}}},
                "completionProvider": {{"triggerCharacters": ["."]}}}}}},
            "textDocument/completion": {{"isIncomplete": false, "items": [{}]}}
        }}{extra}}}"#,
        items.join(", ")
    )
}

/// A project with `comp.rs` open on the fake server, the cursor at the end of
/// its indented empty line 2.
fn completion_project(script: &str) -> (Project, Tinkery) {
    completion_project_with(script, &rust_server(fake()))
}

/// `completion_project` under `config` rather than tinkery's defaults.
fn completion_project_with(script: &str, config: &str) -> (Project, Tinkery) {
    let project = Project::new(Some(script));
    fs::write(project.dir.path().join("comp.rs"), "fn main() {\n    \n}\n").expect("write comp.rs");
    let mut tinkery = project.open(config, "comp.rs");
    project.wait_for(&tinkery, "textDocument/didOpen", "comp.rs");
    tinkery.send_keys("down");
    tinkery.wait_for_text("Ln 2, Col 1", WAIT);
    tinkery.send_keys("end");
    tinkery.wait_for_text("Ln 2, Col 5", WAIT);
    (project, tinkery)
}

fn completion_requests(project: &Project) -> Vec<Value> {
    project
        .log()
        .into_iter()
        .map(|(_, m)| m)
        .filter(|m| is(m, "textDocument/completion", "comp.rs"))
        .collect()
}

/// Types `s` then `.` on the current line and waits for the popup.
fn open_completion(project: &Project, tinkery: &mut Tinkery, line: u16) {
    let asked = completion_requests(project).len();
    tinkery.type_text("s");
    tinkery.wait_for_text(&format!("Ln {line}, Col 6"), WAIT);
    tinkery.type_text(".");
    // The `s` asks too, as any letter does; no item starts with it, so the
    // popup is the `.`'s.
    tinkery.wait_for_files("the completion requests", WAIT, || {
        completion_requests(project).len() >= asked + 2
    });
    tinkery.wait_for_text("capacity", WAIT);
}

// The gutter "   1  " is 6 cells, so after `    s.` on line 2 the cursor is at
// (12, 4). The card's edge is on row 5 and in column 12; its rows start on row
// 6. A row's pill starts at column 13 with `›` at 14, so the kind is three
// cells into the card's inside at column 16 and, after the 6-wide `method` and
// a space, the label at column 23.
const ITEM_ROW: u16 = 6;
const KIND_COL: u16 = 16;
const LABEL_COL: u16 = 23;

#[test]
fn a_trigger_character_or_alt_slash_shows_up_to_ten_items_with_kinds() {
    let (project, mut tinkery) = completion_project(&completion_script(""));
    open_completion(&project, &mut tinkery, 2);
    // One request for the `s`, as for any letter, then the `.`'s.
    let requests = completion_requests(&project);
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[1]["params"]["position"],
        serde_json::json!({"line": 1, "character": 6})
    );
    // The server heard about the `.` before being asked about the text after it.
    let log: Vec<Value> = project.log().into_iter().map(|(_, m)| m).collect();
    let asked = log
        .iter()
        .rposition(|m| method(m) == "textDocument/completion")
        .expect("completion logged");
    let last_change = log[..asked]
        .iter()
        .rfind(|m| method(m) == "textDocument/didChange")
        .expect("a change before the request");
    assert_eq!(last_change["params"]["contentChanges"][0]["text"], ".");

    tinkery.wait_for_cursor(12, 4, WAIT);
    let screen = tinkery.screen();
    let expected = [
        ("field ", "as_str"),
        ("method", "capacity"),
        ("method", "chars"),
        ("method", "clear"),
        ("method", "contains"),
        ("method", "ends_with"),
        ("method", "is_empty"),
        ("method", "len"),
        ("method", "lines"),
        ("method", "push"),
    ];
    for (row, (kind, label)) in (ITEM_ROW..).zip(expected) {
        assert_eq!(
            tinkery.text_col(row, &format!("{kind} {label}")),
            Some(KIND_COL),
            "{label} on row {row}: {screen:#?}"
        );
    }
    // Ten at most: the last two sorted items aren't shown.
    assert!(
        !screen.iter().any(|r| r.contains("push_str")),
        "{screen:#?}"
    );
    assert!(!screen.iter().any(|r| r.contains("trim")), "{screen:#?}");
    // The first row is selected, a pill with its kind in `text`; the rest are
    // on the card, kind `muted`, label `fg`.
    assert!(is_selected(&tinkery, ITEM_ROW, 0, LABEL_COL), "{screen:#?}");
    assert_eq!(tinkery.fg_at(KIND_COL, ITEM_ROW), TEXT);
    assert_eq!(tinkery.text_col(ITEM_ROW + 1, "›"), None, "{screen:#?}");
    assert_eq!(tinkery.bg_at(KIND_COL - 3, ITEM_ROW + 1), CARD);
    assert_eq!(tinkery.bg_at(LABEL_COL, ITEM_ROW + 1), CARD);
    assert_eq!(tinkery.fg_at(KIND_COL, ITEM_ROW + 1), MUTED);
    assert_eq!(tinkery.fg_at(LABEL_COL, ITEM_ROW + 1), FG);
    // `len`'s detail, right-aligned two cells in from the card's right edge,
    // where the row's pill ends: 6 + 1 + 9 + 2 + 18 cells of text, the three
    // before it and an edge each side make the card 42 wide, from column 12.
    let (left, right) = (12, 12 + 42 - 1);
    let len_row = ITEM_ROW + 7;
    let detail = "fn(&self) -> usize";
    assert_eq!(
        tinkery.text_col(len_row, detail),
        Some(right - 1 - 18),
        "{screen:#?}"
    );
    assert_eq!(tinkery.fg_at(right - 1 - 18, len_row), MUTED);
    // The selected row's pill ends in the cell before the edge.
    assert_eq!(tinkery.text_col(ITEM_ROW, CAP_R), Some(right - 1));
    // The same flush card as the hover's, its edge `line2` on the editor's
    // ground: nothing of the `}` it covers on row 5 shows.
    let row_of = |row: u16| cells(&screen, row, left, right + 1);
    assert_eq!(row_of(ITEM_ROW - 1), format!(" {} ", "▁".repeat(40)));
    assert_eq!(row_of(ITEM_ROW + 10), format!(" {} ", "▔".repeat(40)));
    for row in ITEM_ROW..ITEM_ROW + 10 {
        let line = row_of(row);
        assert!(line.starts_with('▕') && line.ends_with('▏'), "{line:?}");
        for col in [left, right] {
            assert_eq!(tinkery.fg_at(col, row), LINE2, "{col},{row}");
            assert_eq!(tinkery.bg_at(col, row), BG, "{col},{row}");
        }
    }
    assert_eq!(tinkery.fg_at(left + 5, ITEM_ROW - 1), LINE2);
    assert_eq!(tinkery.bg_at(left + 5, ITEM_ROW - 1), BG);
    // Not dimmed: the code beside it keeps its colours.
    assert!(screen[3].starts_with("   1  fn main() {"), "{screen:#?}");
    assert!(screen[4].starts_with("   2      s."), "{screen:#?}");
    assert!(status_line(&tinkery).contains("Ln 2, Col 7"));

    // Esc closes it; Alt+/ asks again from the same place.
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone("capacity", WAIT);
    tinkery.send_keys("alt+/");
    tinkery.wait_for_text("capacity", WAIT);
    let requests = completion_requests(&project);
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests[2]["params"]["position"],
        serde_json::json!({"line": 1, "character": 6})
    );
}

#[test]
fn mono_reverses_the_selected_completion_row() {
    let config = format!("theme = \"mono\"\n{}", rust_server(fake()));
    let (project, mut tinkery) = completion_project_with(&completion_script(""), &config);
    open_completion(&project, &mut tinkery, 2);
    tinkery.wait_for_cursor(12, 4, WAIT);
    let screen = tinkery.screen();
    // The selected row is reverse video across the card's inside, with its
    // mark, kind and label in it.
    let selected = tinkery.reversed_text(ITEM_ROW);
    assert!(
        selected.starts_with(" › field  as_str"),
        "{selected:?} {screen:#?}"
    );
    assert_eq!(selected.chars().count(), 42 - 2, "{selected:?}");
    // Its label keeps the row's own colours rather than `strong`, so it reads
    // as reversed like the rest of the row, and is told apart by weight.
    for col in [KIND_COL - 2, KIND_COL, LABEL_COL] {
        assert_eq!(
            (tinkery.fg_at(col, ITEM_ROW), tinkery.bg_at(col, ITEM_ROW)),
            (
                tinkery.fg_at(KIND_COL - 3, ITEM_ROW),
                tinkery.bg_at(KIND_COL - 3, ITEM_ROW)
            ),
            "{col}"
        );
    }
    assert!(tinkery.bold_at(LABEL_COL, ITEM_ROW));
    assert!(!tinkery.bold_at(KIND_COL, ITEM_ROW));
    // No Nerd Font glyph rounds the row's ends.
    assert!(
        !screen
            .iter()
            .any(|row| row.contains(CAP_L) || row.contains(CAP_R)),
        "{screen:#?}"
    );
    assert_ne!(
        tinkery.fg_at(LABEL_COL, ITEM_ROW),
        vt100::Color::Idx(15),
        "{screen:#?}"
    );
    // The next row isn't reversed: kind in `muted` (dark grey), label in `fg`
    // (the terminal's default).
    assert_eq!(tinkery.reversed_text(ITEM_ROW + 1), "", "{screen:#?}");
    assert_eq!(tinkery.fg_at(KIND_COL, ITEM_ROW + 1), vt100::Color::Idx(8));
    assert_eq!(
        tinkery.fg_at(LABEL_COL, ITEM_ROW + 1),
        vt100::Color::Default
    );
}

#[test]
fn the_pointer_lights_a_completion_row_and_enter_takes_the_selected_one() {
    let (project, mut tinkery) = completion_project(&completion_script(""));
    open_completion(&project, &mut tinkery, 2);
    tinkery.wait_for_cursor(12, 4, WAIT);
    tinkery.wait_for_bg(LABEL_COL, ITEM_ROW, HOV, WAIT);
    let row = ITEM_ROW + 1;
    assert_eq!(tinkery.bg_at(LABEL_COL, row), CARD);
    let label = |tinkery: &Tinkery, row: u16| -> String {
        tinkery.screen()[usize::from(row)]
            .chars()
            .skip(usize::from(LABEL_COL))
            .take_while(|c| !c.is_whitespace())
            .collect()
    };
    let (first, second) = (label(&tinkery, ITEM_ROW), label(&tinkery, row));
    assert_ne!(first, second);

    // The row under the pointer is the pill without the mark, its label as
    // it was; the selected row keeps its mark and the popup stays open.
    tinkery.move_to(LABEL_COL, row);
    tinkery.wait_for_bg(LABEL_COL, row, HOV, WAIT);
    assert_eq!(tinkery.text_col(row, "›"), None);
    assert!(!tinkery.bold_at(LABEL_COL, row));
    assert_eq!(tinkery.fg_at(LABEL_COL, row), FG);
    assert!(is_selected(&tinkery, ITEM_ROW, 0, LABEL_COL));
    // Off the card it rests again, and the popup is still there.
    tinkery.move_to(70, 22);
    tinkery.wait_for_bg(LABEL_COL, row, CARD, WAIT);
    assert!(is_selected(&tinkery, ITEM_ROW, 0, LABEL_COL));
    assert_eq!(label(&tinkery, row), second);

    // Enter takes the selected item, not the one the pointer is on.
    tinkery.move_to(LABEL_COL, row);
    tinkery.wait_for_bg(LABEL_COL, row, HOV, WAIT);
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone("capacity", WAIT);
    let screen = tinkery.screen();
    assert!(
        screen[4].contains(&format!("s.{first}")),
        "{first} {screen:#?}"
    );
    assert!(!screen[4].contains(&second), "{second} {screen:#?}");
}

#[test]
fn typing_filters_and_the_arrows_move_the_selection() {
    let (project, mut tinkery) = completion_project(&completion_script(""));
    open_completion(&project, &mut tinkery, 2);

    // Case doesn't matter: `P` keeps `push` and `push_str`.
    tinkery.type_text("P");
    tinkery.wait_for_text_gone("capacity", WAIT);
    tinkery.wait_for_text("s.P", WAIT);
    // The card follows the cursor, one cell right now.
    let screen = tinkery.screen();
    assert_eq!(
        tinkery.text_col(ITEM_ROW, "method push"),
        Some(KIND_COL + 1),
        "{screen:#?}"
    );
    assert_eq!(
        tinkery.text_col(ITEM_ROW + 1, "method push_str"),
        Some(KIND_COL + 1)
    );
    // The typed `p` is `accent` bold in every label; the rest of the selected
    // label, from column `LABEL_COL + 2`, is `strong` bold, and of the other
    // `fg`.
    assert!(
        is_selected(&tinkery, ITEM_ROW, 1, LABEL_COL + 2),
        "{screen:#?}"
    );
    for row in [ITEM_ROW, ITEM_ROW + 1] {
        assert_eq!(tinkery.fg_at(LABEL_COL + 1, row), ACCENT);
        assert!(tinkery.bold_at(LABEL_COL + 1, row));
    }
    assert_eq!(tinkery.fg_at(LABEL_COL + 2, ITEM_ROW + 1), FG);
    assert!(!tinkery.bold_at(LABEL_COL + 2, ITEM_ROW + 1));

    tinkery.send_keys("down");
    tinkery.wait_for_fg_at(LABEL_COL + 2, ITEM_ROW + 1, STRONG, WAIT);
    assert!(is_selected(&tinkery, ITEM_ROW + 1, 1, LABEL_COL + 2));
    assert_eq!(tinkery.bg_at(LABEL_COL + 1, ITEM_ROW), CARD);
    assert_eq!(tinkery.fg_at(LABEL_COL + 2, ITEM_ROW), FG);
    assert_eq!(tinkery.text_col(ITEM_ROW, "›"), None);
    // The selection stops at the last item, and the cursor never moves.
    tinkery.send_keys("down");
    tinkery.send_keys("up");
    tinkery.wait_for_fg_at(LABEL_COL + 2, ITEM_ROW, STRONG, WAIT);
    assert!(status_line(&tinkery).contains("Ln 2, Col 8"));

    // A filter matching nothing closes the popup.
    tinkery.type_text("z");
    tinkery.wait_for_text_gone("push", WAIT);
}

#[test]
fn the_arrows_scroll_the_popup_through_every_match() {
    let (project, mut tinkery) = completion_project(&completion_script(""));
    open_completion(&project, &mut tinkery, 2);
    tinkery.wait_for_cursor(12, 4, WAIT);
    let last = ITEM_ROW + 9;
    // The card as it opened: 42 wide from column 12.
    let (left, right) = (12, 12 + 42 - 1);
    let edges = |tinkery: &Tinkery| -> Vec<String> {
        let screen = tinkery.screen();
        (ITEM_ROW - 1..=last + 1)
            .map(|row| {
                let line = cells(&screen, row, left, right + 1);
                let mut inside = line.chars();
                let ends = (inside.next(), inside.next_back());
                // The edge rows whole, an item's row by its two ends.
                if row < ITEM_ROW || row > last {
                    line
                } else {
                    format!("{ends:?}")
                }
            })
            .collect()
    };
    let opened = edges(&tinkery);
    // What a row shows from the kind column on, and which rows have the mark.
    let at_kind = |tinkery: &Tinkery, row: u16, text: &str| -> bool {
        tinkery.text_col(row, text) == Some(KIND_COL)
    };
    let marked = |tinkery: &Tinkery| -> Vec<u16> {
        (ITEM_ROW..=last)
            .filter(|&row| tinkery.text_col(row, "›").is_some())
            .collect()
    };

    // Down to the tenth row scrolls nothing.
    for _ in 0..9 {
        tinkery.send_keys("down");
    }
    tinkery.wait_for_fg_at(LABEL_COL, last, STRONG, WAIT);
    let screen = tinkery.screen();
    assert!(at_kind(&tinkery, ITEM_ROW, "field  as_str"), "{screen:#?}");
    assert!(at_kind(&tinkery, last, "method push"), "{screen:#?}");
    assert!(
        !screen.iter().any(|r| r.contains("push_str")),
        "{screen:#?}"
    );

    // Down on the tenth row: every row moves up one and the eleventh match
    // is selected on the last row. The first row's kind stays in its column
    // and the card where it was.
    tinkery.send_keys("down");
    tinkery.wait_for_text("push_str", WAIT);
    tinkery.wait_for_text_gone("as_str", WAIT);
    let screen = tinkery.screen();
    assert!(
        at_kind(&tinkery, ITEM_ROW, "method capacity"),
        "{screen:#?}"
    );
    assert!(at_kind(&tinkery, last - 1, "method push "), "{screen:#?}");
    assert!(at_kind(&tinkery, last, "method push_str"), "{screen:#?}");
    assert!(is_selected(&tinkery, last, 0, LABEL_COL), "{screen:#?}");
    assert_eq!(marked(&tinkery), [last], "{screen:#?}");
    assert_eq!(tinkery.text_col(last, CAP_R), Some(right - 1));
    assert_eq!(edges(&tinkery), opened, "{screen:#?}");

    // And again, to the twelfth and last.
    tinkery.send_keys("down");
    tinkery.wait_for_text("trim", WAIT);
    tinkery.wait_for_text_gone("capacity", WAIT);
    let screen = tinkery.screen();
    assert!(at_kind(&tinkery, ITEM_ROW, "method chars"), "{screen:#?}");
    assert!(at_kind(&tinkery, last, "fn     trim"), "{screen:#?}");
    assert!(is_selected(&tinkery, last, 0, LABEL_COL), "{screen:#?}");
    assert_eq!(marked(&tinkery), [last], "{screen:#?}");
    assert_eq!(edges(&tinkery), opened, "{screen:#?}");
    // `len`'s detail came up with its row, still against the right edge.
    assert_eq!(
        tinkery.text_col(ITEM_ROW + 5, "fn(&self) -> usize"),
        Some(right - 1 - 18),
        "{screen:#?}"
    );

    // The pointer lights the row it is on, whichever match scrolled there,
    // and no other; the selection keeps its row.
    let under = ITEM_ROW + 1;
    tinkery.move_to(LABEL_COL, under);
    tinkery.wait_for_bg(LABEL_COL, under, HOV, WAIT);
    let screen = tinkery.screen();
    assert!(at_kind(&tinkery, under, "method clear"), "{screen:#?}");
    assert_eq!(tinkery.fg_at(LABEL_COL, under), FG);
    for row in ITEM_ROW..=last {
        let lit = tinkery.bg_at(LABEL_COL, row) == HOV;
        assert_eq!(lit, row == under || row == last, "{row} {screen:#?}");
    }
    assert_eq!(marked(&tinkery), [last], "{screen:#?}");
    tinkery.move_to(70, 22);
    tinkery.wait_for_bg(LABEL_COL, under, CARD, WAIT);

    // It stops at the last match: another Down changes nothing, so the Up
    // after it lands on the row above, over rows that stay where they are.
    tinkery.send_keys("down");
    tinkery.send_keys("up");
    tinkery.wait_for_fg_at(LABEL_COL, last - 1, STRONG, WAIT);
    let screen = tinkery.screen();
    assert!(is_selected(&tinkery, last - 1, 0, LABEL_COL), "{screen:#?}");
    assert_eq!(marked(&tinkery), [last - 1], "{screen:#?}");
    assert!(at_kind(&tinkery, ITEM_ROW, "method chars"), "{screen:#?}");
    assert!(at_kind(&tinkery, last, "fn     trim"), "{screen:#?}");

    // Up only scrolls back once the selection is on the first row.
    for _ in 0..8 {
        tinkery.send_keys("up");
    }
    tinkery.wait_for_fg_at(LABEL_COL, ITEM_ROW, STRONG, WAIT);
    let screen = tinkery.screen();
    assert!(is_selected(&tinkery, ITEM_ROW, 0, LABEL_COL), "{screen:#?}");
    assert!(at_kind(&tinkery, ITEM_ROW, "method chars"), "{screen:#?}");
    assert!(at_kind(&tinkery, last, "fn     trim"), "{screen:#?}");
    tinkery.send_keys("up");
    tinkery.wait_for_text("capacity", WAIT);
    tinkery.wait_for_text_gone("trim", WAIT);
    let screen = tinkery.screen();
    assert!(
        at_kind(&tinkery, ITEM_ROW, "method capacity"),
        "{screen:#?}"
    );
    assert!(at_kind(&tinkery, last, "method push_str"), "{screen:#?}");
    assert!(is_selected(&tinkery, ITEM_ROW, 0, LABEL_COL), "{screen:#?}");
    assert_eq!(marked(&tinkery), [ITEM_ROW], "{screen:#?}");
    assert_eq!(edges(&tinkery), opened, "{screen:#?}");
    tinkery.send_keys("up");
    tinkery.wait_for_text("as_str", WAIT);
    tinkery.wait_for_text_gone("push_str", WAIT);
    assert!(at_kind(&tinkery, ITEM_ROW, "field  as_str"));
    assert!(is_selected(&tinkery, ITEM_ROW, 0, LABEL_COL));
    // And it stops at the first match: another Up, then Down is the second.
    tinkery.send_keys("up");
    tinkery.send_keys("down");
    tinkery.wait_for_fg_at(LABEL_COL, ITEM_ROW + 1, STRONG, WAIT);
    let screen = tinkery.screen();
    assert!(at_kind(&tinkery, ITEM_ROW, "field  as_str"), "{screen:#?}");
    assert!(
        at_kind(&tinkery, ITEM_ROW + 1, "method capacity"),
        "{screen:#?}"
    );
    assert_eq!(marked(&tinkery), [ITEM_ROW + 1], "{screen:#?}");
    // The cursor never moved.
    assert!(status_line(&tinkery).contains("Ln 2, Col 7"));
}

#[test]
fn enter_and_tab_insert_an_item_that_was_scrolled_to() {
    let (project, mut tinkery) = completion_project(&completion_script(""));
    // `trim` is the twelfth match: on no row until the popup has scrolled.
    for (line, key) in [(2, "enter"), (3, "tab")] {
        open_completion(&project, &mut tinkery, line);
        let screen = tinkery.screen();
        assert!(
            !screen.iter().any(|r| r.contains("fn     trim")),
            "{screen:#?}"
        );
        for _ in 0..11 {
            tinkery.send_keys("down");
        }
        tinkery.wait_for_text("fn     trim", WAIT);
        // The cursor is on row `line + 2`, so the tenth row is eleven under.
        let last = line + 2 + 11;
        tinkery.wait_for_fg_at(LABEL_COL, last, STRONG, WAIT);
        assert!(is_selected(&tinkery, last, 0, LABEL_COL));
        tinkery.send_keys(key);
        tinkery.wait_for_text_gone("fn     trim", WAIT);
        tinkery.wait_for_text(&format!("Ln {line}, Col 11"), WAIT);
        let screen = tinkery.screen();
        assert!(
            screen[usize::from(line) + 2].ends_with("s.trim"),
            "{key} {screen:#?}"
        );
        if line == 2 {
            tinkery.send_keys("enter");
            tinkery.wait_for_text("Ln 3, Col 5", WAIT);
        }
    }
    tinkery.send_keys("ctrl+s");
    tinkery.wait_for_text("saved", WAIT);
    assert_eq!(
        project.read("comp.rs"),
        "fn main() {\n    s.trim\n    s.trim\n}\n"
    );
}

#[test]
fn enter_and_tab_insert_the_item_as_one_undo_step() {
    let (project, mut tinkery) = completion_project(&completion_script(""));

    // `len`'s textEdit, stretched over the `le` typed since.
    open_completion(&project, &mut tinkery, 2);
    tinkery.type_text("le");
    tinkery.wait_for_text_gone("capacity", WAIT);
    tinkery.wait_for_text("method len", WAIT);
    tinkery.send_keys("enter");
    tinkery.wait_for_text("s.len()", WAIT);
    tinkery.wait_for_text_gone("method len", WAIT);
    assert!(status_line(&tinkery).contains("Ln 2, Col 12"));
    // One undo takes back just the insertion.
    tinkery.send_keys("ctrl+z");
    tinkery.wait_for_text_gone("s.len()", WAIT);
    tinkery.wait_for_text("Ln 2, Col 9", WAIT);
    let screen = tinkery.screen();
    assert!(screen[4].ends_with("s.le"), "{screen:#?}");
    tinkery.send_keys("ctrl+y");
    tinkery.wait_for_text("s.len()", WAIT);

    // Tab takes `push`'s snippet with its placeholder as plain text.
    tinkery.send_keys("enter");
    tinkery.wait_for_text("Ln 3, Col 5", WAIT);
    open_completion(&project, &mut tinkery, 3);
    tinkery.type_text("pu");
    tinkery.wait_for_text_gone("capacity", WAIT);
    tinkery.send_keys("tab");
    tinkery.wait_for_text("s.push(ch)", WAIT);

    // `push_str`'s insert text, picked with Down. The cursor is on row 6, so the
    // card's rows start on row 8.
    tinkery.send_keys("enter");
    tinkery.wait_for_text("Ln 4, Col 5", WAIT);
    open_completion(&project, &mut tinkery, 4);
    tinkery.type_text("p");
    tinkery.wait_for_text_gone("capacity", WAIT);
    tinkery.send_keys("down");
    tinkery.wait_for_fg_at(LABEL_COL + 2, 9, STRONG, WAIT);
    tinkery.send_keys("enter");
    tinkery.wait_for_text("s.push_str", WAIT);

    // `trim` has only its label.
    tinkery.send_keys("enter");
    tinkery.wait_for_text("Ln 5, Col 5", WAIT);
    open_completion(&project, &mut tinkery, 5);
    tinkery.type_text("tr");
    tinkery.wait_for_text_gone("capacity", WAIT);
    tinkery.wait_for_text("fn trim", WAIT);
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone("fn trim", WAIT);
    tinkery.wait_for_text("s.trim", WAIT);

    tinkery.send_keys("ctrl+s");
    tinkery.wait_for_text("saved", WAIT);
    assert_eq!(
        project.read("comp.rs"),
        "fn main() {\n    s.len()\n    s.push(ch)\n    s.push_str\n    s.trim\n}\n"
    );
}

#[test]
fn esc_dismisses_the_popup_without_changing_the_buffer() {
    let (project, mut tinkery) = completion_project(&completion_script(""));
    open_completion(&project, &mut tinkery, 2);
    let before = tinkery.screen()[4].clone();
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone("capacity", WAIT);
    let screen = tinkery.screen();
    assert_eq!(screen[4], before);
    assert!(screen[4].ends_with("    s."), "{screen:#?}");
    assert!(status_line(&tinkery).contains("Ln 2, Col 7"));
    // Enter is the editor's again.
    tinkery.send_keys("enter");
    tinkery.wait_for_text("Ln 3, Col 5", WAIT);
    tinkery.send_keys("ctrl+s");
    tinkery.wait_for_text("saved", WAIT);
    assert_eq!(project.read("comp.rs"), "fn main() {\n    s.\n    \n}\n");
}

#[test]
fn a_late_answer_after_the_cursor_left_the_word_is_dropped() {
    // The answer comes 2 s late; a publish sent right after it marks when it
    // has been handled.
    let extra = format!(
        r#", "delay": {{"textDocument/completion": 2000}},
            "notify": {{"textDocument/completion": [{}]}}"#,
        publish(&[(0, 0, 2, 2, "after the completion")])
    );
    let (project, mut tinkery) = completion_project(&completion_script(&extra));
    // Only the `.`: a letter before it would ask too, and that request's publish
    // would come before the answer this test is about.
    tinkery.type_text(".");
    tinkery.wait_for_text("Ln 2, Col 6", WAIT);
    project.wait_for(&tinkery, "textDocument/completion", "comp.rs");
    assert_eq!(completion_requests(&project).len(), 1);
    tinkery.send_keys("down");
    tinkery.wait_for_text("Ln 3, Col 2", WAIT);

    tinkery.wait_for_text("✕ 0  ⚠ 1", WAIT);
    let screen = tinkery.screen();
    assert!(
        !screen.iter().any(|r| r.contains("capacity")),
        "{screen:#?}"
    );
    assert!(status_line(&tinkery).contains("Ln 3, Col 2"));
    // Back on the word, the dropped answer doesn't come back either.
    tinkery.send_keys("up");
    tinkery.wait_for_text("Ln 2, Col 6", WAIT);
    let screen = tinkery.screen();
    assert!(
        !screen.iter().any(|r| r.contains("capacity")),
        "{screen:#?}"
    );
}

/// The labels on the popup's rows from `ITEM_ROW` down, wherever its card sits.
fn completion_labels(tinkery: &Tinkery) -> Vec<String> {
    tinkery
        .screen()
        .iter()
        .skip(usize::from(ITEM_ROW))
        .map_while(|row| {
            let (_, item) = row.split_once("method ")?;
            // The selected row's pill ends in a cap right after a full-width label.
            let label: String = item
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            Some(label)
        })
        .collect()
}

#[test]
fn typing_a_letter_opens_the_popup_filtered_by_the_word() {
    let (project, mut tinkery) = completion_project(&completion_script(""));
    tinkery.type_text("c");
    tinkery.wait_for_text("capacity", WAIT);
    // Asked once, at the cursor after the `c`, which the server had heard of.
    let requests = completion_requests(&project);
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0]["params"]["position"],
        serde_json::json!({"line": 1, "character": 5})
    );
    let log: Vec<Value> = project.log().into_iter().map(|(_, m)| m).collect();
    let asked = log
        .iter()
        .position(|m| method(m) == "textDocument/completion")
        .expect("completion logged");
    let last_change = log[..asked]
        .iter()
        .rfind(|m| method(m) == "textDocument/didChange")
        .expect("a change before the request");
    assert_eq!(last_change["params"]["contentChanges"][0]["text"], "c");

    // Only the items starting with `c`, the typed letter `accent` bold in each.
    let screen = tinkery.screen();
    assert_eq!(
        completion_labels(&tinkery),
        ["capacity", "chars", "clear", "contains"],
        "{screen:#?}"
    );
    // The card is under the cursor, a cell left of where it is after `s.`.
    let label = LABEL_COL - 1;
    for (row, name) in (ITEM_ROW..).zip(["capacity", "chars", "clear", "contains"]) {
        assert_eq!(
            tinkery.text_col(row, &format!("method {name}")),
            Some(KIND_COL - 1),
            "{screen:#?}"
        );
        assert_eq!(tinkery.fg_at(label, row), ACCENT, "{name}");
        assert!(tinkery.bold_at(label, row), "{name}");
    }
    // The first row is the selected one, the rest of its label `strong` bold.
    assert_eq!(tinkery.text_col(ITEM_ROW, "›"), Some(KIND_COL - 3));
    assert_eq!(tinkery.bg_at(label + 1, ITEM_ROW), HOV);
    assert_eq!(tinkery.fg_at(label + 1, ITEM_ROW), STRONG);
    assert!(tinkery.bold_at(label + 1, ITEM_ROW));
    assert_eq!(tinkery.text_col(ITEM_ROW + 1, "›"), None);
    assert_eq!(tinkery.fg_at(label + 1, ITEM_ROW + 1), FG);
    assert!(!tinkery.bold_at(label + 1, ITEM_ROW + 1));
    assert!(screen[4].starts_with("   2      c"), "{screen:#?}");
    assert!(status_line(&tinkery).contains("Ln 2, Col 6"));
}

#[test]
fn more_letters_before_the_answer_ask_once_and_filter_it() {
    let extra = r#", "delay": {"textDocument/completion": 1500}"#;
    let (project, mut tinkery) = completion_project(&completion_script(extra));
    tinkery.type_text("c");
    project.wait_for(&tinkery, "textDocument/completion", "comp.rs");
    tinkery.type_text("a");
    tinkery.wait_for_text("Ln 2, Col 7", WAIT);
    let screen = tinkery.screen();
    assert!(
        !screen.iter().any(|r| r.contains("capacity")),
        "the answer came before the `a`: {screen:#?}"
    );

    tinkery.wait_for_text("capacity", WAIT);
    let screen = tinkery.screen();
    assert_eq!(completion_labels(&tinkery), ["capacity"], "{screen:#?}");
    assert!(screen[4].starts_with("   2      ca"), "{screen:#?}");
    // A third letter filters the open popup, which never asks.
    tinkery.type_text("p");
    tinkery.wait_for_text("Ln 2, Col 8", WAIT);
    assert_eq!(completion_labels(&tinkery), ["capacity"]);
    assert_eq!(completion_requests(&project).len(), 1);
}

#[test]
fn after_esc_the_next_letter_of_the_word_opens_the_popup_again() {
    let (project, mut tinkery) = completion_project(&completion_script(""));
    tinkery.type_text("c");
    tinkery.wait_for_text("capacity", WAIT);
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone("capacity", WAIT);
    assert_eq!(completion_requests(&project).len(), 1);

    tinkery.type_text("h");
    tinkery.wait_for_text("chars", WAIT);
    let screen = tinkery.screen();
    assert_eq!(completion_labels(&tinkery), ["chars"], "{screen:#?}");
    assert!(screen[4].starts_with("   2      ch"), "{screen:#?}");
    let requests = completion_requests(&project);
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[1]["params"]["position"],
        serde_json::json!({"line": 1, "character": 6})
    );
}

#[test]
fn an_answer_matching_nothing_typed_opens_no_popup() {
    // A publish sent right after the answer marks when it has been handled.
    let extra = format!(
        r#", "notify": {{"textDocument/completion": [{}]}}"#,
        publish(&[(0, 0, 2, 2, "after the completion")])
    );
    let (project, mut tinkery) = completion_project(&completion_script(&extra));
    tinkery.type_text("z");
    tinkery.wait_for_text("✕ 0  ⚠ 1", WAIT);
    assert_eq!(completion_requests(&project).len(), 1);
    assert!(completion_labels(&tinkery).is_empty());
    // Nothing is open, to take Esc or to filter: Backspace on a popup hidden
    // for want of a match would show every item.
    tinkery.send_keys("backspace");
    tinkery.wait_for_text("Ln 2, Col 5", WAIT);
    let screen = tinkery.screen();
    assert!(
        !screen.iter().any(|r| r.contains("capacity")),
        "{screen:#?}"
    );
    assert_eq!(completion_requests(&project).len(), 1);
    // The next letter asks as the first one did.
    tinkery.type_text("c");
    tinkery.wait_for_text("capacity", WAIT);
    assert_eq!(completion_requests(&project).len(), 2);
}

#[test]
fn a_letter_that_filters_every_item_away_closes_the_popup() {
    let (project, mut tinkery) = completion_project(&completion_script(""));
    tinkery.type_text("c");
    tinkery.wait_for_text("capacity", WAIT);
    tinkery.type_text("z");
    tinkery.wait_for_text_gone("capacity", WAIT);
    tinkery.wait_for_text("Ln 2, Col 7", WAIT);
    // Closed, not hidden: Backspace to a word that matches doesn't bring it
    // back, and no popup is left to take Esc.
    tinkery.send_keys("backspace");
    tinkery.wait_for_text("Ln 2, Col 6", WAIT);
    let screen = tinkery.screen();
    assert!(
        !screen.iter().any(|r| r.contains("capacity")),
        "{screen:#?}"
    );
    assert_eq!(completion_requests(&project).len(), 1);
    tinkery.send_keys("esc");
    // The next letter of the word asks again, as it does after Esc.
    tinkery.type_text("h");
    tinkery.wait_for_text("chars", WAIT);
    assert_eq!(completion_labels(&tinkery), ["chars"]);
    assert_eq!(completion_requests(&project).len(), 2);
}

#[test]
fn auto_complete_off_asks_on_a_trigger_character_and_alt_slash_only() {
    let config = format!("{}[editor]\nauto_complete = false\n", rust_server(fake()));
    let (project, mut tinkery) = completion_project_with(&completion_script(""), &config);
    tinkery.type_text("ca");
    tinkery.wait_for_text("Ln 2, Col 7", WAIT);
    // The trigger character still asks, and its request is the first the
    // server got: it reads its messages in order, so the letters sent none.
    tinkery.type_text(".");
    tinkery.wait_for_text("capacity", WAIT);
    let requests = completion_requests(&project);
    assert_eq!(requests.len(), 1, "{requests:#?}");
    assert_eq!(
        requests[0]["params"]["position"],
        serde_json::json!({"line": 1, "character": 7})
    );
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone("capacity", WAIT);

    // Letters with the popup closed ask nothing; Alt+/ does, and the popup
    // opens on the word typed so far.
    tinkery.type_text("le");
    tinkery.wait_for_text("Ln 2, Col 10", WAIT);
    let screen = tinkery.screen();
    assert!(!screen.iter().any(|r| r.contains("method")), "{screen:#?}");
    tinkery.send_keys("alt+/");
    tinkery.wait_for_text("method len", WAIT);
    let requests = completion_requests(&project);
    assert_eq!(requests.len(), 2, "{requests:#?}");
    assert_eq!(
        requests[1]["params"]["position"],
        serde_json::json!({"line": 1, "character": 9})
    );
    assert!(tinkery.screen()[4].starts_with("   2      ca.le"));
}

#[test]
fn a_digit_a_space_and_other_punctuation_ask_nothing() {
    let (project, mut tinkery) = completion_project(&completion_script(""));
    tinkery.type_text("7 ;");
    tinkery.wait_for_text("Ln 2, Col 8", WAIT);
    // The trigger character still asks, and its request is the only one: the
    // server reads its messages in order, so nothing came before it.
    tinkery.type_text(".");
    tinkery.wait_for_text("capacity", WAIT);
    let requests = completion_requests(&project);
    assert_eq!(requests.len(), 1, "{requests:#?}");
    assert_eq!(
        requests[0]["params"]["position"],
        serde_json::json!({"line": 1, "character": 8})
    );
}

#[test]
fn a_server_without_a_completion_provider_is_never_asked_by_typing() {
    // The fake's own `initialize` answer has no `completionProvider`.
    let script = r#"{"responses": {"textDocument/completion": {"isIncomplete": false,
        "items": [{"label": "capacity", "kind": 2}]}}}"#;
    let (project, mut tinkery) = completion_project(script);
    tinkery.type_text("ca");
    tinkery.wait_for_text("Ln 2, Col 7", WAIT);
    // Alt+/ asks whatever the server said, as it always has, and that request
    // is the first the server got.
    tinkery.send_keys("alt+/");
    tinkery.wait_for_text("capacity", WAIT);
    let requests = completion_requests(&project);
    assert_eq!(requests.len(), 1, "{requests:#?}");
    assert_eq!(
        requests[0]["params"]["position"],
        serde_json::json!({"line": 1, "character": 6})
    );
}

#[test]
fn typing_into_the_find_bar_a_prompt_or_the_palette_asks_nothing() {
    let (project, mut tinkery) = completion_project(&completion_script(""));
    tinkery.send_keys("ctrl+f");
    tinkery.wait_for_text("Find", WAIT);
    tinkery.type_text("cap");
    tinkery.wait_for_text("cap", WAIT);
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone("cap", WAIT);

    // Save as is a prompt; its bar holds what is typed.
    tinkery.send_keys("alt+s");
    tinkery.wait_for_text("Save as", WAIT);
    tinkery.type_text("cap");
    tinkery.wait_for_text("cap", WAIT);
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone("cap", WAIT);

    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text("cast · files · commands", WAIT);
    tinkery.type_text("cap");
    tinkery.wait_for_text("cap", WAIT);
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone("cast ·", WAIT);

    // Nothing was typed into the file, and Alt+/ in the editor sends the first
    // request the server got.
    let screen = tinkery.screen();
    assert_eq!(screen[4].trim_end(), "   2", "{screen:#?}");
    tinkery.send_keys("alt+/");
    tinkery.wait_for_text("capacity", WAIT);
    let requests = completion_requests(&project);
    assert_eq!(requests.len(), 1, "{requests:#?}");
    assert_eq!(
        requests[0]["params"]["position"],
        serde_json::json!({"line": 1, "character": 4})
    );
}

/// `config.toml` with format on save for Rust, and the editor's indentation the
/// request should carry.
fn format_config(on: bool) -> String {
    format!(
        "[editor]\ntab_width = 2\ninsert_spaces = false\n{}format_on_save = {on}\n",
        rust_server(fake())
    )
}

fn format_requests(project: &Project) -> Vec<Value> {
    project
        .log()
        .into_iter()
        .map(|(_, message)| message)
        .filter(|message| method(message) == "textDocument/formatting")
        .collect()
}

#[test]
fn ctrl_s_formats_through_the_server_then_saves() {
    let range = |from: u32, to: u32| {
        format!(
            r#"{{"start": {{"line": 0, "character": {from}}}, "end": {{"line": 0, "character": {to}}}}}"#
        )
    };
    let script = format!(
        r#"{{"responses": {{"textDocument/formatting": [
            {{"range": {}, "newText": " "}},
            {{"range": {}, "newText": " "}}
        ]}}}}"#,
        range(2, 4),
        range(7, 7)
    );
    let project = Project::new(Some(&script));
    fs::write(project.dir.path().join("a.rs"), "fn  a(){}\n").expect("write a.rs");
    let mut tinkery = project.open(&format_config(true), "a.rs");
    project.wait_for(&tinkery, "textDocument/didOpen", "a.rs");

    tinkery.send_keys("ctrl+s");
    project.wait_for(&tinkery, "textDocument/didSave", "a.rs");
    tinkery.wait_for_text("fn a() {}", WAIT);
    tinkery.wait_for_text("saved a.rs", WAIT);
    assert_eq!(project.read("a.rs"), "fn a() {}\n");
    let screen = tinkery.screen();
    assert!(!screen.join(" ").contains("unformatted"), "{screen:#?}");

    let requests = format_requests(&project);
    assert_eq!(requests.len(), 1, "{requests:#?}");
    assert_eq!(
        requests[0]["params"]["options"],
        serde_json::json!({"tabSize": 2, "insertSpaces": false})
    );
    // The request went out before the save the server heard of.
    let order: Vec<String> = project
        .log()
        .iter()
        .map(|(_, m)| method(m).to_string())
        .filter(|m| m == "textDocument/formatting" || m == "textDocument/didSave")
        .collect();
    assert_eq!(order, ["textDocument/formatting", "textDocument/didSave"]);

    // Both edits undo as one step.
    tinkery.send_keys("ctrl+z");
    tinkery.wait_for_text("fn  a(){}", WAIT);
    tinkery.wait_for_text("a.rs •", WAIT);
}

#[test]
fn a_server_that_never_answers_saves_unformatted_after_two_seconds() {
    let project = Project::new(Some(r#"{"silent": ["textDocument/formatting"]}"#));
    let mut tinkery = project.open(&format_config(true), "a.rs");
    project.wait_for(&tinkery, "textDocument/didOpen", "a.rs");
    tinkery.type_text("x");
    tinkery.wait_for_text("a.rs •", WAIT);

    tinkery.send_keys("ctrl+s");
    project.wait_for(&tinkery, "textDocument/formatting", "a.rs");
    tinkery.wait_for_text("saved a.rs unformatted (no answer in 2 s)", WAIT);
    assert_eq!(project.read("a.rs"), "xfn a() {}\n");
    tinkery.wait_for_text_gone("a.rs •", WAIT);
}

#[test]
fn a_formatting_error_saves_unformatted_and_says_so() {
    let script = r#"{"errors": {"textDocument/formatting": {"code": -32603, "message": "boom"}}}"#;
    let project = Project::new(Some(script));
    let mut tinkery = project.open(&format_config(true), "a.rs");
    project.wait_for(&tinkery, "textDocument/didOpen", "a.rs");
    tinkery.type_text("y");
    tinkery.wait_for_text("a.rs •", WAIT);

    tinkery.send_keys("ctrl+s");
    tinkery.wait_for_text("saved a.rs unformatted (server error: boom)", WAIT);
    assert_eq!(project.read("a.rs"), "yfn a() {}\n");
}

#[test]
fn without_format_on_save_saving_sends_no_formatting_request() {
    // The fake would format if asked, so silence proves it wasn't.
    let script = r#"{"responses": {"textDocument/formatting": [{"range":
        {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 2}},
        "newText": "FN"}]}}"#;
    let project = Project::new(Some(script));
    // The default: no `format_on_save` key at all.
    let mut tinkery = project.open(&rust_server(fake()), "a.rs");
    project.wait_for(&tinkery, "textDocument/didOpen", "a.rs");
    tinkery.type_text("z");
    tinkery.wait_for_text("a.rs •", WAIT);

    tinkery.send_keys("ctrl+s");
    project.wait_for(&tinkery, "textDocument/didSave", "a.rs");
    tinkery.wait_for_text("saved a.rs", WAIT);
    assert_eq!(project.read("a.rs"), "zfn a() {}\n");
    assert!(format_requests(&project).is_empty(), "{:#?}", project.log());

    // Explicitly off behaves the same.
    let project = Project::new(Some(script));
    let mut tinkery = project.open(&format_config(false), "a.rs");
    project.wait_for(&tinkery, "textDocument/didOpen", "a.rs");
    tinkery.send_keys("ctrl+s");
    project.wait_for(&tinkery, "textDocument/didSave", "a.rs");
    assert!(format_requests(&project).is_empty(), "{:#?}", project.log());
}

/// Tinkery on `a.rs` with its config file at `config`, holding `text`, so
/// `>settings` opens and saves that file.
fn open_with_config_file(project: &Project, config: &Path, text: &str) -> Tinkery {
    fs::write(config, text).expect("write config");
    let mut env = project.env();
    env.push(("TINKERY_CONFIG", config.as_os_str().to_owned()));
    let tinkery = Tinkery::spawn_in_with_env(project.dir.path(), &env, &["a.rs"]);
    tinkery.wait_for_text("Ln 1, Col 1", START);
    tinkery
}

/// Ctrl+P, `query`, and Enter once `first_row` shows.
fn cast(tinkery: &mut Tinkery, query: &str, first_row: &str) {
    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text("cast · files · commands", WAIT);
    tinkery.type_text(query);
    tinkery.wait_for_text(first_row, WAIT);
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone("cast ·", WAIT);
}

/// The pids of the servers sent `didOpen` for `file`, newest first.
fn servers_of(project: &Project, file: &str) -> Vec<u64> {
    let mut pids: Vec<u64> = project
        .log()
        .iter()
        .filter(|(_, m)| is(m, "textDocument/didOpen", file))
        .map(|(pid, _)| *pid)
        .collect();
    pids.reverse();
    pids
}

/// How many messages named `name` server `pid` received.
fn received_by(project: &Project, pid: u64, name: &str) -> usize {
    project
        .log()
        .iter()
        .filter(|(p, m)| *p == pid && method(m) == name)
        .count()
}

#[test]
fn saving_the_config_restarts_only_the_servers_whose_table_changed() {
    let project = Project::new(None);
    fs::write(project.dir.path().join("c.py"), "x = 1\n").expect("write c.py");
    let config = project.files.path().join("config.toml");
    let fake = fake();
    let text = format!(
        "[lsp.rust]\ncommand = '{fake}'\nargs = ['--xy']\n[lsp.python]\ncommand = '{fake}'\n"
    );
    let mut tinkery = open_with_config_file(&project, &config, &text);
    project.wait_for(&tinkery, "textDocument/didOpen", "a.rs");
    cast(&mut tinkery, "c.py", "✦ c.py");
    project.wait_for(&tinkery, "textDocument/didOpen", "c.py");
    let rust = servers_of(&project, "a.rs")[0];
    let python = servers_of(&project, "c.py")[0];
    assert_ne!(rust, python);

    // `--xy` becomes `--x`: Rust's table changed, Python's didn't.
    cast(&mut tinkery, ">settings", "Settings");
    tinkery.wait_for_text("args = ['--xy']", WAIT);
    tinkery.send_keys("ctrl+home");
    tinkery.send_keys("down");
    tinkery.send_keys("down");
    tinkery.send_keys("end");
    tinkery.send_keys("left");
    tinkery.send_keys("left");
    tinkery.send_keys("backspace");
    tinkery.wait_for_text("args = ['--x']", WAIT);
    tinkery.send_keys("ctrl+s");
    tinkery.wait_for_text("settings applied", WAIT);

    tinkery.wait_for_files("a.rs opened in a second Rust server", WAIT, || {
        servers_of(&project, "a.rs").first() != Some(&rust)
    });
    let restarted = servers_of(&project, "a.rs")[0];
    assert_eq!(received_by(&project, restarted, "initialize"), 1);
    tinkery.wait_for_files("the old Rust server told to exit", WAIT, || {
        received_by(&project, rust, "exit") == 1
    });
    // Python's server is the one it started with, and was never stopped.
    assert_eq!(servers_of(&project, "c.py"), [python]);
    assert_eq!(received_by(&project, python, "initialize"), 1);
    assert_eq!(
        received_by(&project, python, "shutdown"),
        0,
        "{:#?}",
        project.log()
    );
}

#[test]
fn a_config_naming_a_missing_server_shows_no_server_and_editing_carries_on() {
    let script = format!(r#"{{"notify": {{"textDocument/didOpen": [{}]}}}}"#, both());
    let project = Project::new(Some(&script));
    fs::write(project.dir.path().join("a.rs"), DIAG_FILE).expect("write a.rs");
    let config = project.files.path().join("config.toml");
    let mut tinkery = open_with_config_file(&project, &config, &rust_server(fake()));
    tinkery.wait_for_text("● fake_lsp", WAIT);
    tinkery.wait_for_text("✕ 1  ⚠ 1", WAIT);
    tinkery.wait_for_underlined(Y_ROW, "y", WAIT);
    let first = servers_of(&project, "a.rs");

    // The command's closing quote ends line 2; type just before it.
    cast(&mut tinkery, ">settings", "Settings");
    tinkery.wait_for_text("[lsp.rust]", WAIT);
    tinkery.send_keys("ctrl+home");
    tinkery.send_keys("down");
    tinkery.send_keys("end");
    tinkery.send_keys("left");
    tinkery.type_text("-missing");
    tinkery.wait_for_text("-missing'", WAIT);
    tinkery.send_keys("ctrl+s");
    tinkery.wait_for_text("server not found", WAIT);

    tinkery.send_keys("alt+,");
    tinkery.wait_for_text("○ no server", WAIT);
    // The stopped server's diagnostics went with it: no underlines, no
    // counts, and F8 has nowhere to go, so `abc` lands where the cursor was.
    tinkery.wait_for_underlined(Y_ROW, "", WAIT);
    assert_eq!(tinkery.underlined_text(X_ROW), "");
    let status = status_line(&tinkery);
    assert!(!status.contains("✕ 1"), "{status:?}");
    tinkery.send_keys("f8");
    tinkery.type_text("abc");
    tinkery.wait_for_text("a.rs •", WAIT);
    tinkery.send_keys("ctrl+s");
    tinkery.wait_for_text("saved a.rs", WAIT);
    assert_eq!(project.read("a.rs"), format!("abc{DIAG_FILE}"));
    assert_eq!(servers_of(&project, "a.rs"), first);
}
