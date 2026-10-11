mod harness;

use std::time::Duration;

use harness::{ROWS, Tinkery};

#[test]
fn starts_draws_the_status_line_and_quits_on_ctrl_q() {
    let mut tinkery = Tinkery::spawn(&[]);

    tinkery.wait_for_text("tinkery", Duration::from_secs(10));
    let screen = tinkery.screen();
    assert_eq!(screen.len(), usize::from(ROWS));
    assert!(
        screen[screen.len() - 1].starts_with(" ✦ tinkery"),
        "status line should be on the last row: {screen:#?}"
    );
    let (col, row) = tinkery.cursor();
    assert!(col < harness::COLS && row < ROWS);

    tinkery.send_keys("ctrl+q");
    let status = tinkery.wait_exit(Duration::from_secs(5));
    assert!(status.success(), "tinkery exited with {status:?}");
}
