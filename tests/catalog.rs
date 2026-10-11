//! `>language servers`: the catalog card listing each server and whether it's
//! installed (tinkery-catalog spec C1, C2).

mod harness;

use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::time::Duration;

use harness::{CAP_L, CAP_R, ROWS, Tinkery, hints_text};
use tempfile::TempDir;
use vt100::Color;

const START: Duration = Duration::from_secs(10);
const WAIT: Duration = Duration::from_secs(10);

/// At 100x30 the catalog is a panel the size of the cast's: 86 wide from
/// column 7, dropping from row 4, its title in the top edge. The header's strip
/// is row 6, row 7 is blank and the servers are rows 8 to 14, their text four
/// cells in; the states end four cells in from the card's right edge. A blank,
/// the `debuggers` heading on row 16 and the adapters on rows 17 to 19; a
/// blank, the keys on row 21 and the bottom edge. A row's pill runs the card's
/// inside, from column 8 to column 91, with `›` a cell into it.
const CARD_X: u16 = 7;
const CARD_Y: u16 = 4;
const CARD_WIDTH: u16 = 86;
const HEADER_ROW: u16 = 6;
const BLANK_ROW: u16 = 7;
const FIRST_ROW: u16 = 8;
const DEBUGGERS_ROW: u16 = 16;
const LAST_ROW: u16 = 19;
const PILL_X: u16 = CARD_X + 1;
const PILL_END: u16 = CARD_X + CARD_WIDTH - 2;
const MARK_X: u16 = CARD_X + 2;
const TEXT_X: u16 = 11;
const COMMAND_X: u16 = 36;
const RIGHT: u16 = 89;
const FOOTER_ROW: u16 = 21;
const TITLE: &str = "▁ Language servers ▁";
/// All the header's strip says: the edge above it names the list.
const MARK: &str = "✦";

/// The default theme, tinkery, which an empty config runs on: the roles the
/// catalog is drawn in.
const DEFAULT: &str = "";
const ACCENT: Color = Color::Rgb(0xff, 0x8a, 0x3d);
const RAISED: Color = Color::Rgb(0x1f, 0x1b, 0x19);
const RAISED2: Color = Color::Rgb(0x29, 0x23, 0x20);
const HOV: Color = Color::Rgb(0x3a, 0x30, 0x2a);
const BTN: Color = Color::Rgb(0x33, 0x2a, 0x25);
const MUTED: Color = Color::Rgb(0x7d, 0x70, 0x67);
const TEXT: Color = Color::Rgb(0xb5, 0xa8, 0x9d);
const STRONG: Color = Color::Rgb(0xf8, 0xf0, 0xe6);
const ERR: Color = Color::Rgb(0xff, 0x5d, 0x4d);

/// The keys on the catalog's last row, as keycaps with round ends: Enter does
/// `enter` to the selected row.
fn hints(enter: &str) -> String {
    hints_text(&[("⏎", enter), ("c", "copy command"), ("Esc", "close")])
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

/// Waits for row `y` to be the selected row and asserts it's drawn as one: a
/// pill of `hov` on the card's `raised` across its inside, `›` in the accent a
/// cell into it, and the name in `strong` bold.
fn assert_selected(tinkery: &Tinkery, y: u16) {
    tinkery.wait_for_bg(MARK_X, y, HOV, WAIT);
    assert_eq!(cell(tinkery, MARK_X, y), "›");
    assert_eq!(tinkery.fg_at(MARK_X, y), ACCENT);
    for (x, cap) in [(PILL_X, CAP_L), (PILL_END, CAP_R)] {
        assert_eq!(cell(tinkery, x, y), cap, "{x}");
        assert_eq!(tinkery.fg_at(x, y), HOV, "{x}");
        assert_eq!(tinkery.bg_at(x, y), RAISED, "{x}");
    }
    assert_eq!(tinkery.fg_at(TEXT_X, y), STRONG);
    assert!(tinkery.bold_at(TEXT_X, y));
    assert_eq!(tinkery.bg_at(RIGHT - 1, y), HOV);
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

/// A folder of empty programs named `names`, runnable as the PATH lookup
/// counts them: with `.exe` on Windows, executable elsewhere.
fn programs(names: &[&str]) -> TempDir {
    let dir = tempfile::tempdir().expect("create PATH folder");
    for name in names {
        let file = if cfg!(windows) {
            format!("{name}.exe")
        } else {
            (*name).to_string()
        };
        let path = dir.path().join(file);
        fs::write(&path, "").expect("write fake program");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
                .expect("make fake program executable");
        }
    }
    dir
}

/// Tinkery on an empty project with only `path` on its PATH.
fn with_path(project: &Path, path: &Path) -> Tinkery {
    let env = [("PATH", OsString::from(path))];
    let tinkery = Tinkery::spawn_in_with_config_and_env(project, DEFAULT, &env, &["."]);
    tinkery.wait_for_text("Open directory", START);
    tinkery
}

/// Ctrl+P, `>language servers`, Enter.
fn open_catalog(tinkery: &mut Tinkery) {
    tinkery.send_keys("ctrl+p");
    tinkery.wait_for_text("cast · files · commands", WAIT);
    tinkery.type_text(">language servers");
    tinkery.wait_for_text("cast · commands", WAIT);
    tinkery.wait_for_screen("Language servers listed", WAIT, |screen| {
        // The cast's first command row, under its `COMMANDS` label.
        screen[9].contains("Language servers")
    });
    tinkery.send_keys("enter");
    // The header can arrive before the rest of the card (ConPTY streams the
    // redraw in pieces), so wait for the keys on its last row too.
    tinkery.wait_for_screen("the catalog", WAIT, |screen| {
        screen[usize::from(CARD_Y)].contains(TITLE)
            && screen
                .iter()
                .any(|row| row.contains("copy command") && row.contains("close"))
    });
}

/// Column where `state` starts on a server row: it ends at `RIGHT`.
fn state_x(state: &str) -> u16 {
    RIGHT - u16::try_from(state.chars().count()).expect("short state")
}

#[test]
fn language_servers_in_the_cast_lists_each_server_and_its_state() {
    let project = tempfile::tempdir().expect("create project");
    let path = programs(&["rust-analyzer", "npm", "vscode-css-language-server"]);
    let mut tinkery = with_path(project.path(), path.path());
    open_catalog(&mut tinkery);

    // A panel: the title set into its accent edge, then the header's strip,
    // which holds the mark and doesn't say the title again, and a blank row.
    assert_eq!(tinkery.text_col(CARD_Y, TITLE), Some(CARD_X + 1));
    assert_eq!(tinkery.fg_at(CARD_X + 1, CARD_Y), ACCENT);
    assert!(tinkery.bold_at(CARD_X + 3, CARD_Y));
    assert_eq!(cell(&tinkery, CARD_X, HEADER_ROW), "▕");
    assert_eq!(cell(&tinkery, CARD_X + CARD_WIDTH - 1, HEADER_ROW), "▏");
    assert_eq!(
        inside(&tinkery, HEADER_ROW),
        format!(
            "{CAP_L}  {MARK}{}{CAP_R}",
            " ".repeat(usize::from(PILL_END - TEXT_X - 1))
        )
    );
    assert_eq!(tinkery.text_col(HEADER_ROW, MARK), Some(TEXT_X));
    assert_eq!(cell(&tinkery, PILL_X, HEADER_ROW), CAP_L);
    assert_eq!(cell(&tinkery, PILL_END, HEADER_ROW), CAP_R);
    assert_eq!(tinkery.fg_at(PILL_X, HEADER_ROW), RAISED2);
    assert_eq!(tinkery.bg_at(TEXT_X + 2, HEADER_ROW), RAISED2);
    assert_eq!(inside(&tinkery, BLANK_ROW), "");
    assert_eq!(
        row(&tinkery, FOOTER_ROW + 1)
            .chars()
            .skip(usize::from(PILL_X))
            .take(usize::from(CARD_WIDTH - 2))
            .collect::<String>(),
        "▔".repeat(usize::from(CARD_WIDTH - 2))
    );

    let expect = [
        ("Rust", "rust-analyzer", "installed"),
        ("Go", "gopls", "needs go"),
        (
            "TypeScript / JavaScript",
            "typescript-language-server",
            "missing",
        ),
        ("Python", "pyright-langserver", "missing"),
        ("HTML", "vscode-html-language-server", "missing"),
        ("CSS", "vscode-css-language-server", "installed"),
        ("SQL", "sqls", "needs go"),
    ];
    for (i, (name, command, state)) in expect.into_iter().enumerate() {
        let y = FIRST_ROW + u16::try_from(i).expect("seven rows");
        assert_eq!(tinkery.text_col(y, name), Some(TEXT_X), "{name}");
        assert_eq!(tinkery.text_col(y, command), Some(COMMAND_X), "{name}");
        assert_eq!(tinkery.text_col(y, state), Some(state_x(state)), "{name}");
    }
    // Each state has its own colour: `ok`, `warn` and `muted`.
    let installed = tinkery.fg_at(state_x("installed"), FIRST_ROW);
    let needs = tinkery.fg_at(state_x("needs go"), FIRST_ROW + 1);
    let missing = tinkery.fg_at(state_x("missing"), FIRST_ROW + 2);
    assert_ne!(installed, needs);
    assert_ne!(installed, missing);
    assert_ne!(needs, missing);
    assert_eq!(
        tinkery.fg_at(state_x("installed"), FIRST_ROW + 5),
        installed
    );

    // The first row is the selected one; its command is in `text`, which
    // reads on the pill where `muted` wouldn't. The others are left alone.
    assert_selected(&tinkery, FIRST_ROW);
    assert_eq!(tinkery.fg_at(COMMAND_X, FIRST_ROW), TEXT);
    assert_not_selected(&tinkery, FIRST_ROW + 1);
    assert_eq!(tinkery.fg_at(COMMAND_X, FIRST_ROW + 1), MUTED);
    assert_eq!(missing, MUTED);

    // A blank, then the keys as keycaps: `strong` bold on `btn`, what each
    // does in `muted`.
    assert_eq!(inside(&tinkery, FOOTER_ROW - 1), "");
    assert_eq!(inside(&tinkery, FOOTER_ROW), hints("install"));
    assert_eq!(
        tinkery.text_col(FOOTER_ROW, &hints("install")),
        Some(TEXT_X)
    );
    assert_eq!(tinkery.bg_text(FOOTER_ROW, BTN), "⏎cEsc");
    assert_eq!(tinkery.fg_at(TEXT_X + 1, FOOTER_ROW), STRONG);
    assert!(tinkery.bold_at(TEXT_X + 1, FOOTER_ROW));
    assert_eq!(tinkery.fg_at(TEXT_X, FOOTER_ROW), BTN);
    assert_eq!(tinkery.bg_at(TEXT_X, FOOTER_ROW), RAISED);
    assert_eq!(tinkery.fg_at(TEXT_X + 4, FOOTER_ROW), MUTED);
}

#[test]
fn a_running_server_says_so() {
    let project = tempfile::tempdir().expect("create project");
    fs::write(project.path().join("a.rs"), "fn a() {}\n").expect("write a.rs");
    let log = tempfile::tempdir().expect("create log dir");
    let config = format!(
        "[lsp.rust]\ncommand = '{}'\n",
        env!("CARGO_BIN_EXE_fake_lsp")
    );
    let env = [(
        "FAKE_LSP_LOG",
        log.path().join("log.jsonl").into_os_string(),
    )];
    let mut tinkery =
        Tinkery::spawn_in_with_config_and_env(project.path(), &config, &env, &["a.rs"]);
    tinkery.wait_for_text("Ln 1, Col 1", START);
    open_catalog(&mut tinkery);
    tinkery.wait_for_screen("Rust running", WAIT, |screen| {
        screen[usize::from(FIRST_ROW)].contains("running")
    });
    assert_eq!(
        tinkery.text_col(FIRST_ROW, "running"),
        Some(state_x("running"))
    );
}

#[test]
fn arrows_move_the_selection_and_esc_or_a_click_outside_close() {
    let project = tempfile::tempdir().expect("create project");
    let path = programs(&[]);
    let mut tinkery = with_path(project.path(), path.path());
    open_catalog(&mut tinkery);

    // The first row starts selected: a pill with its mark.
    assert_selected(&tinkery, FIRST_ROW);
    assert_not_selected(&tinkery, FIRST_ROW + 1);
    tinkery.send_keys("down");
    assert_selected(&tinkery, FIRST_ROW + 1);
    assert_not_selected(&tinkery, FIRST_ROW);
    // ↓ stops at the last row, ↑ at the first.
    for _ in 0..10 {
        tinkery.send_keys("down");
    }
    tinkery.wait_for_bg(MARK_X, LAST_ROW, HOV, WAIT);
    for _ in 0..10 {
        tinkery.send_keys("up");
    }
    tinkery.wait_for_bg(MARK_X, FIRST_ROW, HOV, WAIT);

    // Typed letters go nowhere; the catalog stays.
    tinkery.type_text("x");
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(TITLE, WAIT);

    // A click outside the card closes it too.
    open_catalog(&mut tinkery);
    tinkery.click(2, ROWS - 3);
    tinkery.wait_for_text_gone(TITLE, WAIT);
}

#[test]
fn mono_reverses_the_selected_row() {
    let project = tempfile::tempdir().expect("create project");
    let path = programs(&[]);
    let env = [("PATH", OsString::from(path.path()))];
    let mut tinkery =
        Tinkery::spawn_in_with_config_and_env(project.path(), "theme = \"mono\"\n", &env, &["."]);
    tinkery.wait_for_text("Open directory", START);
    open_catalog(&mut tinkery);
    // The card's whole inside, the mark a cell in and the state four cells in
    // from the card's right edge.
    let rust = format!(" › {:<25}{:<41}needs rustup   ", "Rust", "rust-analyzer");
    tinkery.wait_for_reversed(FIRST_ROW, &rust, WAIT);
    // No Nerd Font glyph in `mono`: the pill's ends are blanks, and so are
    // the keycaps', which are bold.
    let screen = tinkery.screen().join("\n");
    assert!(
        !screen.contains(CAP_L) && !screen.contains(CAP_R),
        "{screen}"
    );
    assert_eq!(
        inside(&tinkery, FOOTER_ROW),
        "⏎  install    c  copy command    Esc  close"
    );
    // The panel's lit edge is bold too.
    assert_eq!(tinkery.bold_text(FOOTER_ROW), "▕⏎cEsc▏");
    tinkery.send_keys("down");
    let go = format!(" › {:<25}{:<45}needs go   ", "Go", "gopls");
    tinkery.wait_for_reversed(FIRST_ROW + 1, &go, WAIT);
    assert_eq!(tinkery.reversed_text(FIRST_ROW), "");
}

/// `command` as the platform shell runs it, so the install's first word (the
/// tool the catalog looks for) is a program on PATH: `cmd` or `sh`.
fn shell(command: &str) -> String {
    if cfg!(windows) {
        format!("cmd /C {command}")
    } else {
        format!("sh -c '{command}'")
    }
}

/// `config.toml` giving Rust the server `command` and the install `install`.
/// Triple-quoted literals, so backslashes and quotes stay as they are. The
/// missing server is called `nope` so the messages naming it fit the 100-cell
/// status line beside the cursor position.
fn rust_install(command: &str, install: &str) -> String {
    format!("[lsp.rust]\ncommand = '''{command}'''\ninstall = '''{install}'''\n")
}

/// Tinkery on an empty project with `config`, the PATH it inherits, and its
/// clipboard in the file `clipboard`.
fn with_config(project: &Path, config: &str, clipboard: &Path) -> Tinkery {
    let env = [("TINKERY_CLIPBOARD_FILE", OsString::from(clipboard))];
    let tinkery = Tinkery::spawn_in_with_config_and_env(project, config, &env, &["."]);
    tinkery.wait_for_text("Open directory", START);
    tinkery
}

/// At 30 rows the run panel's title is row 20 and its output starts on row 21.
const RUN_TITLE_ROW: u16 = 20;
const RUN_OUTPUT_ROW: u16 = 21;

/// Waits for the first server row to say `missing`.
fn wait_missing(tinkery: &Tinkery) {
    tinkery.wait_for_screen("Rust missing", WAIT, |screen| {
        screen[usize::from(FIRST_ROW)].contains("missing")
    });
}

#[test]
fn enter_on_a_missing_row_runs_its_install_in_the_run_panel() {
    let project = tempfile::tempdir().expect("create project");
    let clip = tempfile::tempdir().expect("create clipboard dir");
    let config = rust_install("nope", &shell("echo installing rust"));
    let mut tinkery = with_config(project.path(), &config, &clip.path().join("clip"));
    open_catalog(&mut tinkery);
    wait_missing(&tinkery);
    tinkery.send_keys("enter");

    // The card closes and the panel, hidden until now, shows the install.
    tinkery.wait_for_text_gone(TITLE, WAIT);
    tinkery.wait_for_screen("the install's title and output", WAIT, |screen| {
        screen[usize::from(RUN_TITLE_ROW)].contains("install Rust")
            && screen[usize::from(RUN_OUTPUT_ROW)].contains("installing rust")
    });
    // It exited 0, but the server is still nowhere on PATH.
    tinkery.wait_for_text(
        "installed, but nope isn't on PATH; restart your terminal",
        WAIT,
    );
}

#[test]
fn a_failed_install_says_its_exit_code() {
    let project = tempfile::tempdir().expect("create project");
    let clip = tempfile::tempdir().expect("create clipboard dir");
    let config = rust_install("nope", &shell("exit 3"));
    let mut tinkery = with_config(project.path(), &config, &clip.path().join("clip"));
    open_catalog(&mut tinkery);
    wait_missing(&tinkery);
    tinkery.send_keys("enter");
    tinkery.wait_for_text("install failed (exit 3); see the run panel", WAIT);
    assert!(row(&tinkery, RUN_TITLE_ROW).contains("install Rust"));
}

#[test]
fn an_install_waits_for_the_running_command() {
    let project = tempfile::tempdir().expect("create project");
    let clip = tempfile::tempdir().expect("create clipboard dir");
    // Long enough to still be running when Enter is pressed again; it's
    // stopped below, never waited out.
    let long = if cfg!(windows) {
        shell("\"echo started & ping -n 60 127.0.0.1 >nul\"")
    } else {
        shell("echo started; sleep 60")
    };
    let config = rust_install("nope", &long);
    let mut tinkery = with_config(project.path(), &config, &clip.path().join("clip"));
    open_catalog(&mut tinkery);
    wait_missing(&tinkery);
    tinkery.send_keys("enter");
    tinkery.wait_for_screen("the install running", WAIT, |screen| {
        screen[usize::from(RUN_OUTPUT_ROW)].contains("started")
    });

    open_catalog(&mut tinkery);
    wait_missing(&tinkery);
    tinkery.send_keys("enter");
    tinkery.wait_for_text("install Rust is running; stop it first", WAIT);
    // The catalog stays open under the message.
    assert_eq!(tinkery.text_col(CARD_Y, TITLE), Some(CARD_X + 1));

    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(TITLE, WAIT);
    tinkery.send_keys("shift+f5");
    tinkery.wait_for_screen("the install stopped", WAIT, |screen| {
        screen[usize::from(RUN_TITLE_ROW)].contains("stopped")
    });
}

#[test]
fn enter_copies_a_command_it_cant_run() {
    let project = tempfile::tempdir().expect("create project");
    let clip = tempfile::tempdir().expect("create clipboard dir");
    let clipboard = clip.path().join("clip");

    // `needs rustup`: the PATH holds nothing.
    let path = programs(&[]);
    let env = [
        ("PATH", OsString::from(path.path())),
        ("TINKERY_CLIPBOARD_FILE", OsString::from(&clipboard)),
    ];
    let mut tinkery = Tinkery::spawn_in_with_config_and_env(project.path(), DEFAULT, &env, &["."]);
    tinkery.wait_for_text("Open directory", START);
    open_catalog(&mut tinkery);
    tinkery.send_keys("enter");
    tinkery.wait_for_text("copied: rustup component add rust-analyzer", WAIT);
    assert_eq!(
        fs::read_to_string(&clipboard).expect("read clipboard"),
        "rustup component add rust-analyzer"
    );
    // The catalog stays, and nothing ran.
    assert_eq!(tinkery.text_col(CARD_Y, TITLE), Some(CARD_X + 1));
    assert!(!tinkery.screen().iter().any(|l| l.contains("install Rust")));
    drop(tinkery);

    // `sudo` would ask for a password the run panel can't take: copied too.
    let sudo = format!("sudo {}", shell("echo hi"));
    let config = rust_install("nope", &sudo);
    let mut tinkery = with_config(project.path(), &config, &clipboard);
    open_catalog(&mut tinkery);
    wait_missing(&tinkery);
    tinkery.send_keys("enter");
    tinkery.wait_for_text(&format!("copied: {sudo}"), WAIT);
    assert_eq!(
        fs::read_to_string(&clipboard).expect("read clipboard"),
        sudo
    );
}

/// Presses Enter on the selected row, then ↓: keys are handled in order, so
/// once the selection has moved, Enter has been handled too.
fn enter_then_down(tinkery: &mut Tinkery) {
    tinkery.wait_for_bg(MARK_X, FIRST_ROW, HOV, WAIT);
    tinkery.send_keys("enter");
    tinkery.send_keys("down");
    tinkery.wait_for_bg(MARK_X, FIRST_ROW + 1, HOV, WAIT);
}

#[test]
fn enter_on_an_installed_row_does_nothing() {
    let project = tempfile::tempdir().expect("create project");
    let path = programs(&["rust-analyzer"]);
    let mut tinkery = with_path(project.path(), path.path());
    open_catalog(&mut tinkery);
    enter_then_down(&mut tinkery);
    assert_eq!(tinkery.text_col(CARD_Y, TITLE), Some(CARD_X + 1));
    let screen = tinkery.screen();
    assert!(
        !screen.iter().any(|l| l.contains("install Rust")),
        "{screen:#?}"
    );
    assert!(!screen.iter().any(|l| l.contains("copied")), "{screen:#?}");
}

#[test]
fn enter_on_a_running_row_does_nothing() {
    let project = tempfile::tempdir().expect("create project");
    fs::write(project.path().join("a.rs"), "fn a() {}\n").expect("write a.rs");
    let log = tempfile::tempdir().expect("create log dir");
    let config = rust_install(env!("CARGO_BIN_EXE_fake_lsp"), &shell("echo again"));
    let env = [(
        "FAKE_LSP_LOG",
        log.path().join("log.jsonl").into_os_string(),
    )];
    let mut tinkery =
        Tinkery::spawn_in_with_config_and_env(project.path(), &config, &env, &["a.rs"]);
    tinkery.wait_for_text("Ln 1, Col 1", START);
    open_catalog(&mut tinkery);
    tinkery.wait_for_screen("Rust running", WAIT, |screen| {
        screen[usize::from(FIRST_ROW)].contains("running")
    });
    enter_then_down(&mut tinkery);
    assert_eq!(tinkery.text_col(CARD_Y, TITLE), Some(CARD_X + 1));
    assert!(!tinkery.screen().iter().any(|l| l.contains("install Rust")));
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(TITLE, WAIT);
    // Quit cleanly so the fake server exits too.
    tinkery.send_keys("ctrl+q");
    tinkery.wait_exit(WAIT);
}

#[test]
fn a_server_found_after_its_install_starts_for_the_open_files() {
    let project = tempfile::tempdir().expect("create project");
    fs::write(project.path().join("a.rs"), "fn a() {}\n").expect("write a.rs");
    let files = tempfile::tempdir().expect("create server dir");
    let log = files.path().join("log.jsonl");
    // The fake server, copied into place by the install: absent until then.
    let server = files.path().join(if cfg!(windows) {
        "tinkery-fake-server.exe"
    } else {
        "tinkery-fake-server"
    });
    let fake = env!("CARGO_BIN_EXE_fake_lsp");
    let install = if cfg!(windows) {
        shell(&format!("copy /Y \"{fake}\" \"{}\"", server.display()))
    } else {
        format!("cp '{fake}' '{}'", server.display())
    };
    let config = rust_install(&server.display().to_string(), &install);
    let env = [("FAKE_LSP_LOG", log.clone().into_os_string())];
    let mut tinkery =
        Tinkery::spawn_in_with_config_and_env(project.path(), &config, &env, &["a.rs"]);
    tinkery.wait_for_text("rust: server not found", START);

    open_catalog(&mut tinkery);
    wait_missing(&tinkery);
    tinkery.send_keys("enter");
    tinkery.wait_for_text("Rust installed", WAIT);
    // Without restarting Tinkery, the server now follows a.rs.
    tinkery.wait_for_files("the server to open a.rs", WAIT, || {
        fs::read_to_string(&log)
            .is_ok_and(|log| log.contains("textDocument/didOpen") && log.contains("a.rs"))
    });
    // Quit cleanly so the server exits and its folder can go.
    tinkery.send_keys("ctrl+q");
    tinkery.wait_exit(WAIT);
}

#[test]
fn server_not_found_says_the_catalog_installs_it() {
    let project = tempfile::tempdir().expect("create project");
    fs::write(project.path().join("a.rs"), "fn a() {}\n").expect("write a.rs");
    let config = "[lsp.rust]\ncommand = 'nope'\n";
    let tinkery = Tinkery::spawn_in_with_config(project.path(), config, &["a.rs"]);
    tinkery.wait_for_text(
        "rust: server not found (nope) · >language servers installs it",
        START,
    );
}

/// The tool `lldb-dap`'s install runs on this platform.
const LLDB_TOOL: &str = if cfg!(windows) { "winget" } else { "apt" };
const LLDB_INSTALL: &str = if cfg!(windows) {
    "winget install --id LLVM.LLVM -e --accept-source-agreements --accept-package-agreements"
} else {
    "sudo apt install lldb"
};

/// Writes the script `name` into `dir`: `windows` as `name.cmd`, `unix` as an
/// executable `name`. The catalog finds either as the program `name`.
fn script(dir: &Path, name: &str, windows: &str, unix: &str) {
    if cfg!(windows) {
        fs::write(dir.join(format!("{name}.cmd")), windows).expect("write fake script");
    } else {
        let path = dir.join(name);
        fs::write(&path, unix).expect("write fake script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
                .expect("make fake script executable");
        }
    }
}

/// `dir` on PATH ahead of the folders the platform shell and the scripts'
/// commands come from, but none of the user's own programs.
fn path_with_shell(dir: &Path) -> OsString {
    let system: Vec<std::path::PathBuf> = if cfg!(windows) {
        let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
        vec![Path::new(&root).join("System32")]
    } else {
        vec!["/usr/bin".into(), "/bin".into()]
    };
    std::env::join_paths(std::iter::once(dir.to_path_buf()).chain(system)).expect("join PATH")
}

/// Tinkery on an empty project with `path` as its PATH, an empty LLVM folder, and
/// its clipboard in the file `clipboard`. Windows can still hand Tinkery the real
/// `ProgramFiles` (CI's windows-latest has LLVM there), so a test that asserts
/// lldb-dap's state asks `lldb_state` what to expect.
fn with_adapters_path(project: &Path, path: OsString, clipboard: &Path) -> (Tinkery, TempDir) {
    let program_files = tempfile::tempdir().expect("create Program Files");
    let env = [
        ("PATH", path),
        ("ProgramFiles", program_files.path().as_os_str().to_owned()),
        ("TINKERY_CLIPBOARD_FILE", clipboard.as_os_str().to_owned()),
    ];
    let tinkery = Tinkery::spawn_in_with_config_and_env(project, DEFAULT, &env, &["."]);
    tinkery.wait_for_text("Open directory", START);
    (tinkery, program_files)
}

/// lldb-dap's state when it isn't on PATH: `installed` when LLVM's own folder
/// on this machine has it, as on CI's Windows runner, else `missing`.
fn lldb_state() -> &'static str {
    let program_files =
        std::env::var_os("ProgramFiles").unwrap_or_else(|| r"C:\Program Files".into());
    let lldb = Path::new(&program_files)
        .join("LLVM")
        .join("bin")
        .join("lldb-dap.exe");
    if cfg!(windows) && lldb.is_file() {
        "installed"
    } else {
        "missing"
    }
}

/// Moves the selection from the first row down to row `y`.
fn select(tinkery: &mut Tinkery, y: u16) {
    tinkery.wait_for_bg(MARK_X, FIRST_ROW, HOV, WAIT);
    // The heading isn't a row: the adapters follow SQL a ↓ each, so this
    // stops short of `y` by the heading and its blank.
    for _ in FIRST_ROW..y - 2 {
        tinkery.send_keys("down");
    }
    tinkery.wait_for_bg(MARK_X, y, HOV, WAIT);
}

#[test]
fn debuggers_are_listed_under_their_heading_and_copy_their_install() {
    let project = tempfile::tempdir().expect("create project");
    let clip = tempfile::tempdir().expect("create clipboard dir");
    let clipboard = clip.path().join("clip");
    // lldb-dap's install tool and a Python that can't import anything, but
    // no Go.
    let path = programs(&[LLDB_TOOL]);
    script(
        path.path(),
        "python",
        "@exit /b 1\r\n",
        "#!/bin/sh\nexit 1\n",
    );
    let (mut tinkery, _llvm) =
        with_adapters_path(project.path(), OsString::from(path.path()), &clipboard);
    open_catalog(&mut tinkery);
    // The header can arrive before the rest of the card (ConPTY streams the
    // redraw in pieces), so wait for every adapter's state and the footer.
    tinkery.wait_for_screen("the debuggers' states", WAIT, |screen| {
        let at = |y: u16| &screen[usize::from(y)];
        at(DEBUGGERS_ROW + 1).contains(lldb_state())
            && at(DEBUGGERS_ROW + 2).contains("missing")
            && at(LAST_ROW).contains("needs go")
            && at(FOOTER_ROW).contains(&hints("install"))
    });

    assert_eq!(inside(&tinkery, DEBUGGERS_ROW - 1), "");
    assert_eq!(inside(&tinkery, DEBUGGERS_ROW), "debuggers");
    assert_eq!(tinkery.fg_at(TEXT_X, DEBUGGERS_ROW), MUTED);
    assert_eq!(tinkery.text_col(DEBUGGERS_ROW, "debuggers"), Some(TEXT_X));
    assert!(tinkery.bold_at(TEXT_X, DEBUGGERS_ROW));
    let expect = [
        ("lldb-dap", "lldb-dap", lldb_state()),
        ("debugpy", "python -m debugpy.adapter", "missing"),
        ("dlv", "dlv dap", "needs go"),
    ];
    for (i, (name, command, state)) in expect.into_iter().enumerate() {
        let y = DEBUGGERS_ROW + 1 + u16::try_from(i).expect("three rows");
        assert_eq!(tinkery.text_col(y, name), Some(TEXT_X), "{name}");
        // lldb-dap's command is its name, so look from the command column.
        let from_command: String = row(&tinkery, y)
            .chars()
            .skip(usize::from(COMMAND_X))
            .collect();
        assert!(from_command.starts_with(command), "{from_command:?}");
        assert_eq!(tinkery.text_col(y, state), Some(state_x(state)), "{name}");
    }
    // The states take the servers' colours.
    let missing = tinkery.fg_at(state_x("missing"), DEBUGGERS_ROW + 2);
    let needs = tinkery.fg_at(state_x("needs go"), LAST_ROW);
    assert_ne!(missing, needs);
    assert_eq!(tinkery.fg_at(state_x("needs go"), FIRST_ROW + 6), needs);
    assert_eq!(inside(&tinkery, FOOTER_ROW - 1), "");
    assert_eq!(inside(&tinkery, FOOTER_ROW), hints("install"));

    // `c` on lldb-dap copies its install command.
    select(&mut tinkery, DEBUGGERS_ROW + 1);
    tinkery.send_keys("c");
    tinkery.wait_for_text("copied: ", WAIT);
    assert_eq!(
        fs::read_to_string(&clipboard).expect("read clipboard"),
        LLDB_INSTALL
    );
    // Enter on dlv, whose install needs Go, copies too.
    tinkery.send_keys("down");
    tinkery.send_keys("down");
    tinkery.wait_for_bg(MARK_X, LAST_ROW, HOV, WAIT);
    tinkery.send_keys("enter");
    tinkery.wait_for_text(
        "copied: go install github.com/go-delve/delve/cmd/dlv@latest",
        WAIT,
    );
    // Off Windows lldb-dap's install needs `sudo`: Enter copies it rather
    // than running it.
    if !cfg!(windows) {
        fs::remove_file(&clipboard).expect("clear clipboard");
        tinkery.send_keys("up");
        tinkery.send_keys("up");
        tinkery.wait_for_bg(MARK_X, DEBUGGERS_ROW + 1, HOV, WAIT);
        tinkery.send_keys("enter");
        tinkery.wait_for_text("copied: sudo apt install lldb", WAIT);
        assert_eq!(
            fs::read_to_string(&clipboard).expect("read clipboard"),
            LLDB_INSTALL
        );
    }
    assert!(!tinkery.screen().iter().any(|l| l.contains("install dlv")));
}

/// A fake `go` that says what it was asked and, when `creates`, leaves an
/// empty `dlv` beside itself, as `go install` would in `~/go/bin`.
fn fake_go(dir: &Path, creates: bool) {
    let (windows, unix) = if creates {
        (
            "@echo off\r\necho fake go %*\r\ntype nul > \"%~dp0dlv.exe\"\r\n",
            "#!/bin/sh\necho \"fake go $*\"\nd=$(dirname \"$0\")\n: > \"$d/dlv\"\nchmod +x \"$d/dlv\"\n",
        )
    } else {
        (
            "@echo off\r\necho fake go %*\r\n",
            "#!/bin/sh\necho \"fake go $*\"\n",
        )
    };
    script(dir, "go", windows, unix);
}

#[test]
fn enter_on_dlv_runs_its_install_and_finds_it_after() {
    let project = tempfile::tempdir().expect("create project");
    let clip = tempfile::tempdir().expect("create clipboard dir");
    let bin = tempfile::tempdir().expect("create PATH folder");
    fake_go(bin.path(), true);
    let (mut tinkery, _llvm) = with_adapters_path(
        project.path(),
        path_with_shell(bin.path()),
        &clip.path().join("clip"),
    );
    open_catalog(&mut tinkery);
    tinkery.wait_for_screen("dlv missing", WAIT, |screen| {
        screen[usize::from(LAST_ROW)].contains("missing")
    });
    select(&mut tinkery, LAST_ROW);
    tinkery.send_keys("enter");

    tinkery.wait_for_text_gone(TITLE, WAIT);
    tinkery.wait_for_screen("the install's title and output", WAIT, |screen| {
        screen[usize::from(RUN_TITLE_ROW)].contains("install dlv")
            && screen[usize::from(RUN_OUTPUT_ROW)]
                .contains("fake go install github.com/go-delve/delve/cmd/dlv@latest")
    });
    tinkery.wait_for_text("dlv installed", WAIT);

    // Opened again, the catalog finds it.
    open_catalog(&mut tinkery);
    tinkery.wait_for_screen("dlv installed", WAIT, |screen| {
        screen[usize::from(LAST_ROW)].contains("installed")
    });
    assert_eq!(
        tinkery.text_col(LAST_ROW, "installed"),
        Some(state_x("installed"))
    );
}

#[test]
fn an_adapter_still_missing_after_its_install_says_so() {
    let project = tempfile::tempdir().expect("create project");
    let clip = tempfile::tempdir().expect("create clipboard dir");
    let bin = tempfile::tempdir().expect("create PATH folder");
    fake_go(bin.path(), false);
    let (mut tinkery, _llvm) = with_adapters_path(
        project.path(),
        path_with_shell(bin.path()),
        &clip.path().join("clip"),
    );
    open_catalog(&mut tinkery);
    tinkery.wait_for_screen("dlv missing", WAIT, |screen| {
        screen[usize::from(LAST_ROW)].contains("missing")
    });
    select(&mut tinkery, LAST_ROW);
    tinkery.send_keys("enter");
    tinkery.wait_for_text(
        "installed, but dlv isn't on PATH; restart your terminal",
        WAIT,
    );
}

#[test]
fn debugpy_is_installed_once_python_can_import_it() {
    let project = tempfile::tempdir().expect("create project");
    let clip = tempfile::tempdir().expect("create clipboard dir");
    let bin = tempfile::tempdir().expect("create PATH folder");
    // A fake Python: `-c` (the import check) succeeds once its pip has run.
    script(
        bin.path(),
        "python",
        "@echo off\r\n\
         if \"%~1\"==\"-c\" goto import\r\n\
         echo fake pip %*\r\n\
         type nul > \"%~dp0debugpy-installed\"\r\n\
         exit /b 0\r\n\
         :import\r\n\
         if exist \"%~dp0debugpy-installed\" exit /b 0\r\n\
         exit /b 1\r\n",
        "#!/bin/sh\n\
         d=$(dirname \"$0\")\n\
         if [ \"$1\" = \"-c\" ]; then\n\
         test -f \"$d/debugpy-installed\"\n\
         exit $?\n\
         fi\n\
         echo \"fake pip $*\"\n\
         : > \"$d/debugpy-installed\"\n",
    );
    let (mut tinkery, _llvm) = with_adapters_path(
        project.path(),
        path_with_shell(bin.path()),
        &clip.path().join("clip"),
    );
    open_catalog(&mut tinkery);
    // Python is there, but it can't import debugpy yet.
    let debugpy = DEBUGGERS_ROW + 2;
    tinkery.wait_for_screen("debugpy missing", WAIT, |screen| {
        screen[usize::from(debugpy)].contains("missing")
    });
    select(&mut tinkery, debugpy);
    tinkery.send_keys("enter");
    tinkery.wait_for_screen("the install's title and output", WAIT, |screen| {
        screen[usize::from(RUN_TITLE_ROW)].contains("install debugpy")
            && screen[usize::from(RUN_OUTPUT_ROW)].contains("fake pip -m pip install debugpy")
    });
    tinkery.wait_for_text("debugpy installed", WAIT);

    open_catalog(&mut tinkery);
    tinkery.wait_for_screen("debugpy installed", WAIT, |screen| {
        screen[usize::from(debugpy)].contains("installed")
    });
}

/// Why the fake server below refuses to initialize: longer than the card is
/// wide, as a real server's error often is.
const REASON: &str = "Could not find a TypeScript install: typescript@7 ships no tsserver.js; \
                      install typescript 5 next to the project and try again";
/// The cells the reason gets: from two in under the names to where the states end.
const REASON_X: u16 = TEXT_X + 2;
const REASON_WIDTH: usize = (RIGHT - REASON_X) as usize;
const RUST_INSTALL: &str = "rustup component add rust-analyzer";

/// Tinkery editing `a.rs` in `project`, its Rust server the fake one refusing
/// `initialize` with `REASON` and logging to `files/log.jsonl`, its clipboard
/// `files/clip`, and `top` (a theme, say) at the top of its config. Waits for
/// the failure to reach the status line.
fn with_failing_rust(project: &Path, files: &Path, top: &str) -> Tinkery {
    fs::write(project.join("a.rs"), "fn a() {}\n").expect("write a.rs");
    let script = files.join("script.json");
    let error = serde_json::json!({"errors": {"initialize": {"code": -32603, "message": REASON}}});
    fs::write(&script, error.to_string()).expect("write script");
    let config = format!(
        "{top}[lsp.rust]\ncommand = '''{}'''\nargs = ['--log', '''{}''', '--script', '''{}''']\n\
         install = '''{RUST_INSTALL}'''\n",
        env!("CARGO_BIN_EXE_fake_lsp"),
        files.join("log.jsonl").display(),
        script.display(),
    );
    let env = [(
        "TINKERY_CLIPBOARD_FILE",
        files.join("clip").into_os_string(),
    )];
    let tinkery = Tinkery::spawn_in_with_config_and_env(project, &config, &env, &["a.rs"]);
    tinkery.wait_for_text("rust: server failed to start", START);
    tinkery
}

/// How many `initialize` requests the fake servers logged: one per start.
fn starts(files: &Path) -> usize {
    fs::read_to_string(files.join("log.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter(|line| line.contains(r#""method":"initialize""#))
        .count()
}

#[test]
fn a_failed_server_says_why_and_enter_retries_it() {
    let project = tempfile::tempdir().expect("create project");
    let files = tempfile::tempdir().expect("create server dir");
    let mut tinkery = with_failing_rust(project.path(), files.path(), DEFAULT);
    open_catalog(&mut tinkery);
    tinkery.wait_for_screen("Rust failed", WAIT, |screen| {
        screen[usize::from(FIRST_ROW)].contains("failed")
    });
    assert_eq!(
        tinkery.text_col(FIRST_ROW, "failed"),
        Some(state_x("failed"))
    );
    // `err` on the selected row's pill.
    assert_selected(&tinkery, FIRST_ROW);
    assert_eq!(tinkery.fg_at(state_x("failed"), FIRST_ROW), ERR);
    assert_eq!(tinkery.bg_at(state_x("failed"), FIRST_ROW), HOV);

    // The reason on the line below, in `muted`, cut where the states end.
    let shown: String = REASON.chars().take(REASON_WIDTH).collect();
    let line = row(&tinkery, FIRST_ROW + 1);
    assert_eq!(
        tinkery.text_col(FIRST_ROW + 1, &shown),
        Some(REASON_X),
        "{line:?}"
    );
    assert_eq!(inside(&tinkery, FIRST_ROW + 1), shown, "{line:?}");
    // It isn't a row: `muted` on the card, with no pill behind it.
    assert_eq!(tinkery.fg_at(REASON_X, FIRST_ROW + 1), MUTED);
    assert_not_selected(&tinkery, FIRST_ROW + 1);
    // The rows below make room for it, and Enter offers a retry.
    assert_eq!(tinkery.text_col(FIRST_ROW + 2, "Go"), Some(TEXT_X));
    assert_eq!(inside(&tinkery, FOOTER_ROW + 1), hints("retry"));
    assert_eq!(
        tinkery.text_col(FOOTER_ROW + 1, &hints("retry")),
        Some(TEXT_X)
    );

    // Off the failed row, the reason and the retry go.
    tinkery.send_keys("down");
    tinkery.wait_for_text_gone(&shown, WAIT);
    assert_eq!(tinkery.text_col(FIRST_ROW + 1, "Go"), Some(TEXT_X));
    assert_eq!(inside(&tinkery, FOOTER_ROW), hints("install"));
    tinkery.send_keys("up");
    tinkery.wait_for_text(&hints("retry"), WAIT);

    assert_eq!(starts(files.path()), 1);
    tinkery.send_keys("enter");
    tinkery.wait_for_text_gone(TITLE, WAIT);
    // Started again for the open file, without reopening it.
    tinkery.wait_for_files("a second start", WAIT, || starts(files.path()) == 2);
    tinkery.send_keys("ctrl+q");
    tinkery.wait_exit(WAIT);
}

#[test]
fn retrying_with_no_file_open_waits_for_one_and_stops_saying_failed() {
    let project = tempfile::tempdir().expect("create project");
    let files = tempfile::tempdir().expect("create server dir");
    let mut tinkery = with_failing_rust(project.path(), files.path(), DEFAULT);
    tinkery.send_keys("ctrl+w");
    tinkery.wait_for_screen("a.rs closed", WAIT, |screen| !screen[1].contains("a.rs"));
    open_catalog(&mut tinkery);
    tinkery.wait_for_screen("Rust failed", WAIT, |screen| {
        screen[usize::from(FIRST_ROW)].contains("failed")
    });
    tinkery.send_keys("enter");
    tinkery.wait_for_text("Rust will start when you open a file", WAIT);
    assert!(!tinkery.screen().iter().any(|l| l.contains("retrying")));
    assert_eq!(starts(files.path()), 1);

    // The reason is gone with nothing left to retry.
    open_catalog(&mut tinkery);
    tinkery.wait_for_screen("Rust no longer failed", WAIT, |screen| {
        let rust = &screen[usize::from(FIRST_ROW)];
        rust.contains("Rust") && !rust.contains("failed")
    });
    assert_eq!(inside(&tinkery, FOOTER_ROW), hints("install"));
}

#[test]
fn c_on_a_failed_row_copies_its_install_and_mono_reverses_it() {
    let project = tempfile::tempdir().expect("create project");
    let files = tempfile::tempdir().expect("create server dir");
    let mut tinkery = with_failing_rust(project.path(), files.path(), "theme = \"mono\"\n");
    open_catalog(&mut tinkery);
    tinkery.wait_for_screen("Rust failed", WAIT, |screen| {
        screen[usize::from(FIRST_ROW)].contains("failed")
    });
    // The whole row across the card's inside, not the reason under it.
    let selected = tinkery.reversed_text(FIRST_ROW);
    assert!(selected.starts_with(" › Rust "), "{selected:?}");
    assert!(selected.ends_with(" failed   "), "{selected:?}");
    assert_eq!(selected.chars().count(), 84, "{selected:?}");
    assert_eq!(tinkery.reversed_text(FIRST_ROW + 1), "");

    tinkery.send_keys("c");
    tinkery.wait_for_text(&format!("copied: {RUST_INSTALL}"), WAIT);
    assert_eq!(
        fs::read_to_string(files.path().join("clip")).expect("read clipboard"),
        RUST_INSTALL
    );
    // Still open, still failed: copying isn't retrying.
    assert_eq!(tinkery.text_col(CARD_Y, TITLE), Some(CARD_X + 1));
    assert_eq!(starts(files.path()), 1);
    tinkery.send_keys("esc");
    tinkery.wait_for_text_gone(TITLE, WAIT);
    tinkery.send_keys("ctrl+q");
    tinkery.wait_exit(WAIT);
}
