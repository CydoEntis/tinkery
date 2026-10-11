mod harness;

use std::time::Duration;

use harness::Tinkery;

const START: Duration = Duration::from_secs(10);
const EXIT: Duration = Duration::from_secs(5);

#[test]
fn remapped_quit() {
    let mut tinkery = Tinkery::spawn_with_config("[keys]\nquit = \"alt+q\"\n", &[]);
    tinkery.wait_for_text("tinkery", START);

    tinkery.send_keys("ctrl+q");
    tinkery.assert_running_for(Duration::from_millis(750));

    tinkery.send_keys("alt+q");
    let status = tinkery.wait_exit(EXIT);
    assert!(status.success(), "tinkery exited with {status:?}");
}

#[test]
fn bad_config_shows_error() {
    let mut tinkery = Tinkery::spawn_with_config("[keys]\nquit = = \"alt+q\"\n", &[]);
    tinkery.wait_for_text("config error:", START);
    let screen = tinkery.screen();
    let toast = &screen[usize::from(harness::TOAST_ROW)];
    assert!(
        toast.contains("✕ config error: line 2:"),
        "a toast should carry the error: {screen:#?}"
    );

    // Defaults still apply, so the default quit key works.
    tinkery.send_keys("ctrl+q");
    let status = tinkery.wait_exit(EXIT);
    assert!(status.success(), "tinkery exited with {status:?}");
}

#[test]
fn unknown_action_shows_error_and_keeps_defaults() {
    let mut tinkery = Tinkery::spawn_with_config("[keys]\nfly = \"ctrl+k\"\n", &[]);
    tinkery.wait_for_text("config error: [keys]: unknown action \"fly\"", START);

    tinkery.send_keys("ctrl+q");
    let status = tinkery.wait_exit(EXIT);
    assert!(status.success(), "tinkery exited with {status:?}");
}
