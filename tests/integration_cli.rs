use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn plain_mode_honors_explicit_utc_time() {
    Command::cargo_bin("ztl")
        .unwrap()
        .args([
            "--plain",
            "--zones",
            "Europe/London,America/New_York",
            "--time",
            "08:15",
            "--nhours",
            "8",
            "--width",
            "72",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Europe/London"))
        .stdout(predicate::str::contains("America/New_York"))
        .stdout(predicate::str::contains("09:15"))
        .stdout(predicate::str::contains("04:15"));
}

#[test]
fn invalid_startup_input_exits_before_entering_tui_mode() {
    Command::cargo_bin("ztl")
        .unwrap()
        .args(["--zone", "Mars/Olympus"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown timezone"));
}
