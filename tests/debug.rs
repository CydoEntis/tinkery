//! Starting and stopping a debug session against `fake_dap`, the scripted
//! adapter in `src/bin/fake_dap.rs`, driven through the real binary
//! (tinkery-debugger spec D4, D5, D6, D7).

mod harness;

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use harness::{CAP_L, CAP_R, ROWS, Tinkery, hints_text};
use serde_json::{Value, json};
use tempfile::TempDir;

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(10);

/// Aurora's `accent2`, which the paused line is marked in.
const ACCENT2: vt100::Color = vt100::Color::Rgb(0x6e, 0xe7, 0xd8);
/// The paused line's flat fill: Aurora's `bg` (`#0b0a10`) mixed 0.12 toward
/// `accent2` (Tinkery Forge F13).
const PAUSED_BG: vt100::Color = vt100::Color::Rgb(0x17, 0x25, 0x28);

fn fake() -> &'static str {
    env!("CARGO_BIN_EXE_fake_dap")
}

/// The screen row of buffer line `line` (1-based) while the view is at the top:
/// the editor starts below the three rows of the tab header.
fn row(line: u16) -> u16 {
    line + 2
}

fn status_line(screen: &[String]) -> &str {
    &screen[usize::from(ROWS - 1)]
}

/// A Python project holding `main.py` and `other.py`, with the fake's log and
/// script outside it so the tree never shows them.
struct Project {
    dir: TempDir,
    files: TempDir,
}

impl Project {
    /// `script` is the fake's script; `tinkery_toml` the project's `.tinkery.toml`.
    fn new(script: Value, tinkery_toml: &str) -> Self {
        let dir = tempfile::tempdir().expect("create project dir");
        fs::write(dir.path().join("main.py"), "a = 1\nb = 2\nc = 3\n").expect("write main.py");
        fs::write(dir.path().join("other.py"), "x = 0\n").expect("write other.py");
        if !tinkery_toml.is_empty() {
            fs::write(dir.path().join(".tinkery.toml"), tinkery_toml).expect("write .tinkery.toml");
        }
        let files = tempfile::tempdir().expect("create log dir");
        fs::write(files.path().join("script.json"), script.to_string()).expect("write script");
        Self { dir, files }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn log_path(&self) -> PathBuf {
        self.files.path().join("log.jsonl")
    }

    /// `config.toml` pointing Python at `adapter`, with the fake's log and
    /// script as its arguments. Literal strings, so Windows backslashes stay.
    fn config(&self, adapter: &str) -> String {
        let log = self.log_path();
        let script = self.files.path().join("script.json");
        format!(
            "theme = \"aurora\"\n[debug.python]\nadapter = '{adapter}'\nargs = ['--log', '{}', '--script', '{}']\n",
            log.display(),
            script.display()
        )
    }

    /// Tinkery on `file` with the fake as Python's adapter.
    fn open(&self, file: &str) -> Tinkery {
        self.open_with(fake(), file)
    }

    fn open_with(&self, adapter: &str, file: &str) -> Tinkery {
        let tinkery =
            Tinkery::spawn_in_with_config(self.dir.path(), &self.config(adapter), &[file]);
        tinkery.wait_for_text("Ln 1, Col 1", START);
        tinkery
    }

    /// Every request the fake received, in order.
    fn requests(&self) -> Vec<Value> {
        let Ok(text) = fs::read_to_string(self.log_path()) else {
            return Vec::new();
        };
        text.lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .map(|entry| entry["message"].clone())
            .filter(|message| message["type"] == "request")
            .collect()
    }

    fn commands(&self) -> Vec<String> {
        self.requests()
            .iter()
            .filter_map(|r| r["command"].as_str().map(str::to_string))
            .collect()
    }

    /// Waits until the fake has received `count` requests named `command`.
    fn wait_for(&self, tinkery: &Tinkery, command: &str, count: usize) {
        tinkery.wait_for_files(&format!("{count} × {command}"), WAIT, || {
            self.commands().iter().filter(|c| *c == command).count() >= count
        });
    }
}

/// Lines of `setBreakpoints` request `request`, and the file it names.
fn breakpoints(request: &Value) -> (String, Vec<u64>) {
    let path = request["arguments"]["source"]["path"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let lines = request["arguments"]["breakpoints"]
        .as_array()
        .map(|list| list.iter().filter_map(|b| b["line"].as_u64()).collect())
        .unwrap_or_default();
    (path, lines)
}

fn wait_for_status(tinkery: &Tinkery, text: &str) {
    tinkery.wait_for_screen(&format!("{text:?} in the status line"), WAIT, |screen| {
        status_line(screen).contains(text)
    });
}

#[test]
fn alt_f5_builds_then_launches_with_every_breakpoint_in_order() {
    let project = Project::new(json!({}), "[debug]\nbuild = \"echo built\"\n");
    let mut tinkery = project.open("main.py");
    tinkery.send_keys("down");
    tinkery.wait_for_text("Ln 2, Col 1", WAIT);
    tinkery.send_keys("f9");
    // A second file, so the breakpoints go file by file.
    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text("cast", WAIT);
    tinkery.type_text("other.py");
    tinkery.wait_for_text("other.py  ", WAIT);
    tinkery.send_keys("enter");
    tinkery.wait_for_text("1  x = 0", WAIT);
    tinkery.send_keys("f9");

    tinkery.send_keys("alt+f5");
    wait_for_status(&tinkery, "● debugging");
    tinkery.wait_for_text("debug other.py", WAIT);
    project.wait_for(&tinkery, "configurationDone", 1);
    assert_eq!(
        project.commands(),
        [
            "initialize",
            "launch",
            "setBreakpoints",
            "setBreakpoints",
            "configurationDone"
        ]
    );
    let requests = project.requests();
    assert_eq!(
        requests[1]["arguments"]["program"],
        json!(project.path("other.py"))
    );
    let sent: Vec<(String, Vec<u64>)> = requests[2..4].iter().map(breakpoints).collect();
    let file = |name: &str| project.path(name).display().to_string();
    assert_eq!(
        sent,
        [(file("main.py"), vec![2]), (file("other.py"), vec![1])]
    );
}

#[test]
fn a_failed_build_stops_there() {
    let project = Project::new(json!({}), "[debug]\nbuild = \"exit 3\"\n");
    let mut tinkery = project.open("main.py");
    tinkery.send_keys("alt+f5");
    tinkery.wait_for_toast("build failed", WAIT);
    // The run panel keeps the build's own result.
    tinkery.wait_for_text("exited 3", WAIT);
    assert!(project.commands().is_empty(), "{:?}", project.commands());
    assert!(!status_line(&tinkery.screen()).contains("debugging"));
}

#[test]
fn program_output_goes_to_the_run_panel_titled_debug_program() {
    let output = json!({"event": "output", "body": {"category": "stdout", "output": "hello from the program\n"}});
    let project = Project::new(json!({"events": {"configurationDone": [output]}}), "");
    let mut tinkery = project.open("main.py");
    tinkery.send_keys("alt+f5");
    tinkery.wait_for_text("debug main.py", WAIT);
    tinkery.wait_for_text("hello from the program", WAIT);
    wait_for_status(&tinkery, "● debugging");
}

#[test]
fn a_stop_opens_the_top_frames_file_and_marks_the_paused_line() {
    let project = Project::new(json!({}), "");
    // The script names the project's own main.py, so it's written once the
    // project exists.
    let script = json!({
        "events": {"configurationDone": [
            {"event": "stopped", "body": {"reason": "breakpoint", "threadId": 1}}
        ]},
        "responses": {"stackTrace": {"stackFrames": [
            {"id": 1, "name": "<module>", "source": {"path": project.path("main.py")}, "line": 3, "column": 1}
        ]}},
    });
    fs::write(project.files.path().join("script.json"), script.to_string()).expect("write script");

    let mut tinkery = project.open("other.py");
    tinkery.send_keys("alt+f5");
    wait_for_status(&tinkery, "‖ paused main.py:3");
    wait_for_status(&tinkery, "Ln 3, Col 1");
    tinkery.wait_for_text("3  c = 3", WAIT);
    tinkery.wait_for_screen("▶ on line 3", WAIT, |screen| {
        screen[usize::from(row(3))].starts_with('▶')
    });
    tinkery.wait_for_fg_at(0, row(3), ACCENT2, WAIT);
    // The line is one flat fill from the mark cell on, not a ramp.
    tinkery.wait_for_bg(0, row(3), PAUSED_BG, WAIT);
    assert_eq!(tinkery.bg_at(8, row(3)), PAUSED_BG);
    assert_eq!(tinkery.bg_at(20, row(3)), PAUSED_BG);
    // Only the paused line is marked.
    let screen = tinkery.screen();
    assert!(!screen[usize::from(row(2))].starts_with('▶'));
    assert_ne!(tinkery.bg_at(8, row(2)), PAUSED_BG);
    // The stack was asked of the thread that stopped.
    let requests = project.requests();
    let stack = requests
        .iter()
        .find(|r| r["command"] == "stackTrace")
        .expect("a stackTrace request");
    assert_eq!(stack["arguments"]["threadId"], 1);
}

#[test]
fn alt_f6_disconnects_and_ends_the_session() {
    let project = Project::new(json!({}), "");
    let mut tinkery = project.open("main.py");
    tinkery.send_keys("alt+f5");
    wait_for_status(&tinkery, "● debugging");
    project.wait_for(&tinkery, "configurationDone", 1);

    tinkery.send_keys("alt+f6");
    tinkery.wait_for_toast("debugging stopped", WAIT);
    project.wait_for(&tinkery, "disconnect", 1);
    let requests = project.requests();
    let disconnect = requests
        .iter()
        .find(|r| r["command"] == "disconnect")
        .expect("a disconnect request");
    assert_eq!(disconnect["arguments"]["terminateDebuggee"], true);
    tinkery.wait_for_text("stopped", WAIT);
    assert!(!status_line(&tinkery.screen()).contains("● debugging"));

    // Nothing is left to stop.
    tinkery.send_keys("alt+f6");
    tinkery.wait_for_toast("not debugging", WAIT);
}

#[test]
fn the_program_exiting_ends_the_session_with_its_code() {
    let script = json!({"events": {"configurationDone": [
        {"event": "exited", "body": {"exitCode": 4}},
        {"event": "terminated"}
    ]}});
    let project = Project::new(script, "");
    let mut tinkery = project.open("main.py");
    tinkery.send_keys("alt+f5");
    tinkery.wait_for_toast("exited 4", WAIT);
    assert!(!status_line(&tinkery.screen()).contains("debugging"));
    // The session is over: the adapter was let go.
    project.wait_for(&tinkery, "disconnect", 1);
}

#[test]
fn a_lone_terminated_ends_the_session_without_calling_it_a_failure() {
    // Some adapters end with `terminated` and never send an exit code.
    let script = json!({"events": {"configurationDone": [
        {"event": "terminated"}
    ]}});
    let project = Project::new(script, "");
    let mut tinkery = project.open("main.py");
    tinkery.send_keys("alt+f5");
    tinkery.wait_for_toast("program ended", WAIT);
    tinkery.wait_for_text("debug main.py  ended", WAIT);
    let screen = tinkery.screen();
    assert!(!status_line(&screen).contains("debugging"));
    assert!(!screen.iter().any(|line| line.contains('✕')), "{screen:#?}");
    project.wait_for(&tinkery, "disconnect", 1);
}

#[test]
fn breakpoints_toggled_during_a_session_are_sent() {
    let project = Project::new(json!({}), "");
    let mut tinkery = project.open("main.py");
    tinkery.send_keys("alt+f5");
    wait_for_status(&tinkery, "● debugging");
    project.wait_for(&tinkery, "configurationDone", 1);
    // No breakpoints at the start, so none were sent.
    assert!(!project.commands().contains(&"setBreakpoints".to_string()));

    tinkery.send_keys("f9");
    project.wait_for(&tinkery, "setBreakpoints", 1);
    tinkery.send_keys("f9");
    project.wait_for(&tinkery, "setBreakpoints", 2);
    let sent: Vec<(String, Vec<u64>)> = project
        .requests()
        .iter()
        .filter(|r| r["command"] == "setBreakpoints")
        .map(breakpoints)
        .collect();
    let main = project.path("main.py").display().to_string();
    assert_eq!(sent, [(main.clone(), vec![1]), (main, vec![])]);
}

#[test]
fn a_language_without_an_adapter_has_no_debugger() {
    let project = Project::new(json!({}), "");
    fs::write(project.path("app.ts"), "let a = 1;\n").expect("write app.ts");
    let mut tinkery = project.open("app.ts");
    tinkery.send_keys("alt+f5");
    tinkery.wait_for_toast("no debugger for typescript", WAIT);
}

#[test]
fn a_missing_adapter_is_one_message_and_editing_carries_on() {
    let project = Project::new(json!({}), "");
    let missing = project.files.path().join("no-such-adapter");
    let mut tinkery = project.open_with(&missing.display().to_string(), "main.py");
    tinkery.send_keys("alt+f5");
    // The message names the adapter by its path, too long to show whole here.
    tinkery.wait_for_toast("✕ can't start", WAIT);
    tinkery.wait_for_screen("the session gone", WAIT, |screen| {
        !status_line(screen).contains("debugging")
    });
    tinkery.type_text("z");
    tinkery.wait_for_text("1  za = 1", WAIT);
}

#[test]
fn a_crashed_adapter_is_one_message_and_editing_carries_on() {
    let project = Project::new(json!({"exit_on": "launch"}), "");
    let mut tinkery = project.open("main.py");
    tinkery.send_keys("alt+f5");
    tinkery.wait_for_toast("debug adapter exited (3)", WAIT);
    assert!(!status_line(&tinkery.screen()).contains("debugging"));
    tinkery.type_text("z");
    tinkery.wait_for_text("1  za = 1", WAIT);
}

/// A project paused in `inner` (main.py:3), called from `caller` in other.py:1.
/// `inner` has `n` and `items` in its locals, `items` holding one value; the
/// caller has `x` in its globals.
fn paused_project() -> Project {
    let project = Project::new(json!({}), "");
    let script = json!({
        "events": {"configurationDone": [
            {"event": "stopped", "body": {"reason": "breakpoint", "threadId": 1}}
        ]},
        "responses": {"stackTrace": {"stackFrames": [
            {"id": 1, "name": "inner", "source": {"path": project.path("main.py")}, "line": 3, "column": 1},
            {"id": 2, "name": "caller", "source": {"path": project.path("other.py")}, "line": 1, "column": 1}
        ]}},
        "scopes": {
            "1": [{"name": "Locals", "variablesReference": 10}],
            "2": [{"name": "Globals", "variablesReference": 20}]
        },
        "variables": {
            "10": [
                {"name": "n", "value": "3", "type": "int", "variablesReference": 0},
                {"name": "items", "value": "[7]", "type": "list", "variablesReference": 11}
            ],
            "11": [{"name": "0", "value": "7", "type": "int", "variablesReference": 0}],
            "20": [{"name": "x", "value": "0", "type": "int", "variablesReference": 0}]
        }
    });
    fs::write(project.files.path().join("script.json"), script.to_string()).expect("write script");
    project
}

/// At 100x30 the panel's title is row 20, its sides' labels row 21 and their
/// lists start on row 22; the stack's side is columns 0 to 39.
const PANEL_ROW: u16 = 20;
const STACK_END: u16 = 39;
/// The keys the paused panel's title offers, drawn as keycaps.
const PAUSED_HINTS: &[(&str, &str)] =
    &[("Tab", "stack/variables"), ("F6", "focus"), ("F4", "hide")];

/// Requests named `command` whose `key` argument is `value`.
fn asked(project: &Project, command: &str, key: &str, value: i64) -> usize {
    project
        .requests()
        .iter()
        .filter(|r| r["command"] == command && r["arguments"][key] == value)
        .count()
}

#[test]
fn the_debug_panel_takes_the_run_panels_slot_while_debugging() {
    let project = paused_project();
    let mut tinkery = project.open("main.py");
    tinkery.send_keys("alt+f5");
    wait_for_status(&tinkery, "‖ paused main.py:3");
    tinkery.wait_for_text("call stack", WAIT);
    tinkery.wait_for_text("debug main.py  paused", WAIT);
    tinkery.wait_for_text("▶ inner  main.py:3", WAIT);
    tinkery.wait_for_text("caller  other.py:1", WAIT);
    tinkery.wait_for_text("▾ Locals", WAIT);
    tinkery.wait_for_text("n = 3  int", WAIT);
    tinkery.wait_for_text("▸ items = [7]  list", WAIT);
    // The title's keys are keycaps, ending a cell short of the right edge.
    let hints = hints_text(PAUSED_HINTS);
    let title = &tinkery.screen()[usize::from(PANEL_ROW)];
    assert!(title.starts_with(" ‖ debug main.py  paused "), "{title:?}");
    assert!(title.ends_with(&format!("   {hints}")), "{title:?}");
    assert_eq!(
        tinkery.text_col(PANEL_ROW, &hints),
        Some(harness::COLS - 1 - 46)
    );
    assert!(tinkery.bold_at(harness::COLS - 1 - 46 + 1, PANEL_ROW));
    // The selected frame is a pill across the stack's side with `›` before
    // its text; the frame under it isn't.
    let stack = &tinkery.screen()[usize::from(PANEL_ROW) + 2];
    assert!(
        stack.starts_with(&format!("{CAP_L}› ▶ inner  main.py:3")),
        "{stack:?}"
    );
    assert_eq!(tinkery.text_col(PANEL_ROW + 2, CAP_R), Some(STACK_END));
    assert_ne!(
        tinkery.bg_at(1, PANEL_ROW + 2),
        tinkery.bg_at(1, PANEL_ROW + 3)
    );
    assert_eq!(
        tinkery.bg_at(STACK_END - 1, PANEL_ROW + 2),
        tinkery.bg_at(1, PANEL_ROW + 2)
    );
    assert!(
        tinkery.bold_at(5, PANEL_ROW + 2),
        "the selected frame's name"
    );
    assert!(!tinkery.bold_at(5, PANEL_ROW + 3));
    // Without the keys the variables side shows no selection.
    let lists = tinkery.screen()[usize::from(PANEL_ROW) + 2..usize::from(ROWS) - 1].concat();
    assert_eq!(lists.matches('›').count(), 1, "{lists:?}");

    // F4 hides it and shows it again.
    tinkery.send_keys("f4");
    tinkery.wait_for_text_gone("call stack", WAIT);
    tinkery.send_keys("f4");
    tinkery.wait_for_text("call stack", WAIT);

    // The session over, the run panel is back with the program's output.
    tinkery.send_keys("alt+f6");
    tinkery.wait_for_text("debug main.py  stopped", WAIT);
    tinkery.wait_for_text_gone("call stack", WAIT);
    tinkery.wait_for_text(
        &hints_text(&[("Alt+F5", "debug again"), ("F4", "hide")]),
        WAIT,
    );
}

#[test]
fn in_mono_the_selected_rows_are_reverse_video_and_the_keycaps_bold_text() {
    let project = paused_project();
    let config = project.config(fake()).replace("aurora", "mono");
    let mut tinkery = Tinkery::spawn_in_with_config(project.dir.path(), &config, &["main.py"]);
    tinkery.wait_for_text("Ln 1, Col 1", START);
    tinkery.send_keys("alt+f5");
    tinkery.wait_for_text("▸ items = [7]  list", WAIT);
    // The selected frame, edge to edge of the stack's side, and nothing else.
    let frame = format!("{:<40}", " › ▶ inner  main.py:3");
    tinkery.wait_for_reversed(PANEL_ROW + 2, &frame, WAIT);
    for row in [PANEL_ROW, PANEL_ROW + 1, PANEL_ROW + 3, PANEL_ROW + 4] {
        assert_eq!(tinkery.reversed_text(row), "", "{row}");
    }
    let title = &tinkery.screen()[usize::from(PANEL_ROW)];
    assert!(
        title.ends_with("    Tab  stack/variables    F6  focus    F4  hide"),
        "{title:?}"
    );
    assert_eq!(title.chars().count(), usize::from(harness::COLS) - 1);
    assert_eq!(
        tinkery.bold_text(PANEL_ROW).replace(' ', ""),
        "debugmain.pyTabF6F4"
    );

    // With the keys on the variables side, its selected row is reversed too.
    tinkery.send_keys("f6");
    tinkery.send_keys("tab");
    tinkery.send_keys("down");
    tinkery.send_keys("down");
    let variable = format!("{:<59}", " ›   ▸ items = [7]  list");
    tinkery.wait_for_reversed(PANEL_ROW + 4, &variable, WAIT);
    assert_eq!(tinkery.reversed_text(PANEL_ROW + 2), frame);
    for row in PANEL_ROW..ROWS - 1 {
        let text = &tinkery.screen()[usize::from(row)];
        assert!(!text.contains(CAP_L) && !text.contains(CAP_R), "{text:?}");
    }
}

#[test]
fn f6_enters_the_panel_where_the_arrows_expand_and_collapse_values() {
    let project = paused_project();
    let mut tinkery = project.open("main.py");
    tinkery.send_keys("alt+f5");
    tinkery.wait_for_text("▸ items = [7]  list", WAIT);

    // Tree, editor, then the panel.
    tinkery.send_keys("f6");
    tinkery.send_keys("tab");
    tinkery.send_keys("down");
    tinkery.send_keys("down");
    // With the keys, the variable they are on is a pill with `›` before it.
    tinkery.wait_for_text(&format!("│{CAP_L}›   ▸ items = [7]  list"), WAIT);
    assert_eq!(
        tinkery.text_col(PANEL_ROW + 4, CAP_R),
        Some(harness::COLS - 1)
    );
    tinkery.send_keys("right");
    tinkery.wait_for_text(&format!("│{CAP_L}›   ▾ items = [7]  list"), WAIT);
    tinkery.wait_for_text("0 = 7  int", WAIT);
    assert_eq!(asked(&project, "variables", "variablesReference", 11), 1);

    tinkery.send_keys("left");
    tinkery.wait_for_text("▸ items = [7]  list", WAIT);
    tinkery.wait_for_text_gone("0 = 7  int", WAIT);
    // Enter expands too; what was loaded isn't asked for again.
    tinkery.send_keys("enter");
    tinkery.wait_for_text("0 = 7  int", WAIT);
    assert_eq!(asked(&project, "variables", "variablesReference", 11), 1);

    // Nothing typed reaches the buffer while the panel has the keys; F6 goes
    // on round to the editor, the tree being hidden.
    tinkery.type_text("q");
    tinkery.send_keys("f6");
    tinkery.type_text("z");
    tinkery.wait_for_text("3  zc = 3", WAIT);
}

#[test]
fn enter_on_a_frame_opens_its_file_at_its_line_and_shows_its_variables() {
    let project = paused_project();
    let mut tinkery = project.open("main.py");
    tinkery.send_keys("alt+f5");
    tinkery.wait_for_text("n = 3  int", WAIT);

    // ↓ then ↑ is back on the top frame, which Enter opens again. Opening a
    // frame takes the keys to its code, so F6 comes back to the panel.
    tinkery.send_keys("f6");
    tinkery.send_keys("down");
    tinkery.send_keys("up");
    tinkery.send_keys("enter");
    project.wait_for(&tinkery, "scopes", 2);
    assert_eq!(asked(&project, "scopes", "frameId", 1), 2);
    assert_eq!(asked(&project, "scopes", "frameId", 2), 0);
    tinkery.wait_for_text("n = 3  int", WAIT);

    tinkery.send_keys("f6");
    tinkery.send_keys("down");
    tinkery.send_keys("enter");
    tinkery.wait_for_text("▾ Globals", WAIT);
    tinkery.wait_for_text("x = 0  int", WAIT);
    tinkery.wait_for_text_gone("n = 3  int", WAIT);
    tinkery.wait_for_text("1  x = 0", WAIT);
    wait_for_status(&tinkery, "Ln 1, Col 1");
    assert_eq!(asked(&project, "scopes", "frameId", 2), 1);
    // The frame shown is the one marked.
    tinkery.wait_for_text("▶ caller  other.py:1", WAIT);
}

/// A stop in `main.py` on thread `thread`, as the fake sends it.
fn stopped(thread: i64) -> Value {
    json!({"event": "stopped", "body": {"reason": "step", "threadId": thread}})
}

/// The `stackTrace` answers for the stops in turn: each on `main.py` at one of
/// `lines`.
fn stacks(project: &Project, lines: &[u64]) -> Value {
    let main = project.path("main.py");
    lines
        .iter()
        .map(|line| {
            json!({"stackFrames": [
                {"id": 1, "name": "<module>", "source": {"path": main}, "line": line, "column": 1}
            ]})
        })
        .collect()
}

fn write_script(project: &Project, script: &Value) {
    fs::write(project.files.path().join("script.json"), script.to_string()).expect("write script");
}

/// Waits until the `▶` marker is on buffer line `line` and nowhere else.
fn wait_for_marker(tinkery: &Tinkery, line: u16) {
    tinkery.wait_for_screen(&format!("▶ on line {line} alone"), WAIT, |screen| {
        (1..=3).all(|l| screen[usize::from(row(l))].starts_with('▶') == (l == line))
    });
}

fn wait_for_no_marker(tinkery: &Tinkery) {
    tinkery.wait_for_screen("no ▶", WAIT, |screen| {
        (1..=3).all(|l| !screen[usize::from(row(l))].starts_with('▶'))
    });
}

#[test]
fn stepping_and_continuing_go_to_the_stopped_thread() {
    let project = Project::new(json!({}), "");
    let script = json!({
        "events": {
            "configurationDone": [stopped(7)],
            "next": [stopped(7)],
            "stepIn": [stopped(7)],
            "stepOut": [stopped(7)],
        },
        "responses": {"stackTrace": stacks(&project, &[3, 1, 2, 3])},
    });
    write_script(&project, &script);

    let mut tinkery = project.open("main.py");
    tinkery.send_keys("alt+f5");
    wait_for_status(&tinkery, "‖ paused main.py:3");
    wait_for_marker(&tinkery, 3);

    tinkery.send_keys("f10");
    wait_for_status(&tinkery, "‖ paused main.py:1");
    wait_for_marker(&tinkery, 1);
    tinkery.send_keys("alt+f10");
    wait_for_status(&tinkery, "‖ paused main.py:2");
    wait_for_marker(&tinkery, 2);
    tinkery.send_keys("shift+f10");
    wait_for_status(&tinkery, "‖ paused main.py:3");
    wait_for_marker(&tinkery, 3);

    // `continue` is answered with no stop: the program runs on.
    tinkery.send_keys("alt+f5");
    project.wait_for(&tinkery, "continue", 1);
    wait_for_status(&tinkery, "● debugging");
    wait_for_no_marker(&tinkery);

    let requests = project.requests();
    let steps: Vec<(String, Value)> = requests
        .iter()
        .filter(|r| {
            ["next", "stepIn", "stepOut", "continue"]
                .contains(&r["command"].as_str().unwrap_or_default())
        })
        .map(|r| {
            (
                r["command"].as_str().unwrap_or_default().to_string(),
                r["arguments"]["threadId"].clone(),
            )
        })
        .collect();
    assert_eq!(
        steps,
        [
            ("next".to_string(), json!(7)),
            ("stepIn".to_string(), json!(7)),
            ("stepOut".to_string(), json!(7)),
            ("continue".to_string(), json!(7)),
        ]
    );
}

#[test]
fn stepping_while_running_does_nothing_and_the_marker_returns_at_the_next_stop() {
    let project = Project::new(json!({}), "");
    // Toggling a breakpoint while the program runs is what stops it here, as
    // the fake has no program of its own to hit one.
    let script = json!({
        "events": {"setBreakpoints": [stopped(1)]},
        "responses": {"stackTrace": stacks(&project, &[3, 2])},
    });
    write_script(&project, &script);

    let mut tinkery = project.open("main.py");
    tinkery.send_keys("alt+f5");
    wait_for_status(&tinkery, "● debugging");
    project.wait_for(&tinkery, "configurationDone", 1);
    for key in ["f10", "alt+f10", "shift+f10", "alt+f5"] {
        tinkery.send_keys(key);
    }
    // Keys are handled in order, so by the time this breakpoint is sent the
    // four before it were already handled.
    tinkery.send_keys("f9");
    project.wait_for(&tinkery, "setBreakpoints", 1);
    wait_for_status(&tinkery, "‖ paused main.py:3");
    wait_for_marker(&tinkery, 3);
    let commands = project.commands();
    for step in ["next", "stepIn", "stepOut", "continue"] {
        assert!(!commands.iter().any(|c| c == step), "{commands:?}");
    }

    tinkery.send_keys("alt+f5");
    project.wait_for(&tinkery, "continue", 1);
    wait_for_status(&tinkery, "● debugging");
    wait_for_no_marker(&tinkery);

    tinkery.send_keys("f9");
    wait_for_status(&tinkery, "‖ paused main.py:2");
    wait_for_marker(&tinkery, 2);
}

#[test]
fn the_program_ending_after_continue_says_exited_and_clears_the_marker() {
    let project = Project::new(json!({}), "");
    let script = json!({
        "events": {
            "configurationDone": [stopped(1)],
            "continue": [
                {"event": "exited", "body": {"exitCode": 0}},
                {"event": "terminated"}
            ],
        },
        "responses": {"stackTrace": stacks(&project, &[2])},
    });
    write_script(&project, &script);

    let mut tinkery = project.open("main.py");
    tinkery.send_keys("alt+f5");
    wait_for_status(&tinkery, "‖ paused main.py:2");
    wait_for_marker(&tinkery, 2);

    tinkery.send_keys("alt+f5");
    tinkery.wait_for_toast("exited 0", WAIT);
    wait_for_no_marker(&tinkery);
    let screen = tinkery.screen();
    assert!(!status_line(&screen).contains("debugging"), "{screen:#?}");
    assert!(!status_line(&screen).contains("paused"), "{screen:#?}");
    project.wait_for(&tinkery, "disconnect", 1);
}

#[test]
fn a_refused_step_keeps_the_program_paused_where_it_was() {
    let project = Project::new(json!({}), "");
    let script = json!({
        "events": {
            "configurationDone": [stopped(7)],
            "stepIn": [stopped(7)],
        },
        "errors": {"next": "busy"},
        "responses": {"stackTrace": stacks(&project, &[3, 1])},
    });
    write_script(&project, &script);

    let mut tinkery = project.open("main.py");
    tinkery.send_keys("alt+f5");
    wait_for_status(&tinkery, "‖ paused main.py:3");
    wait_for_marker(&tinkery, 3);

    tinkery.send_keys("f10");
    tinkery.wait_for_toast("debugger: next failed: busy", WAIT);
    wait_for_marker(&tinkery, 3);

    // The stopped thread came back with the pause, so stepping still works.
    tinkery.send_keys("f10");
    project.wait_for(&tinkery, "next", 2);
    wait_for_marker(&tinkery, 3);
    tinkery.send_keys("alt+f10");
    wait_for_status(&tinkery, "‖ paused main.py:1");
    wait_for_marker(&tinkery, 1);

    let threads: Vec<Value> = project
        .requests()
        .iter()
        .filter(|r| ["next", "stepIn"].contains(&r["command"].as_str().unwrap_or_default()))
        .map(|r| r["arguments"]["threadId"].clone())
        .collect();
    assert_eq!(threads, [json!(7), json!(7), json!(7)]);
}

#[test]
fn alt_f5_continues_a_paused_session_with_no_file_open() {
    let project = Project::new(json!({}), "");
    let script = json!({
        "events": {"configurationDone": [stopped(7)]},
        "responses": {"stackTrace": stacks(&project, &[3])},
    });
    write_script(&project, &script);

    let mut tinkery = project.open("main.py");
    tinkery.send_keys("alt+f5");
    wait_for_status(&tinkery, "‖ paused main.py:3");

    // Closing the last tab leaves the key list, but the session is still
    // paused and Alt+F5 still continues it.
    tinkery.send_keys("ctrl+w");
    tinkery.wait_for_text("no file open", WAIT);
    tinkery.send_keys("alt+f5");
    project.wait_for(&tinkery, "continue", 1);

    let threads: Vec<Value> = project
        .requests()
        .iter()
        .filter(|r| r["command"] == "continue")
        .map(|r| r["arguments"]["threadId"].clone())
        .collect();
    assert_eq!(threads, [json!(7)]);
}
