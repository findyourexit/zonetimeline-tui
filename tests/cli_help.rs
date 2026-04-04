use assert_cmd::Command;
use predicates::prelude::*;

fn temp_config_path() -> (tempfile::TempDir, std::path::PathBuf) {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    (temp_dir, config_path)
}

#[test]
fn help_mentions_plain_mode_and_original_flags() {
    Command::cargo_bin("ztl")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("--time"))
        .stdout(predicate::str::contains("--plain"))
        .stdout(predicate::str::contains("--nhours"))
        .stdout(predicate::str::contains("--zone"))
        .stdout(predicate::str::contains("--zones"))
        .stdout(predicate::str::contains("--config"))
        .stdout(predicate::str::contains("--width"))
        .stdout(predicate::str::contains("list"));
}

#[test]
fn time_flag_accepts_single_digit_hour() {
    let (_temp_dir, config_path) = temp_config_path();

    Command::cargo_bin("ztl")
        .unwrap()
        .arg("--config")
        .arg(&config_path)
        .args(["--plain", "--time", "7"])
        .assert()
        .success();
}

#[test]
fn time_flag_accepts_zero_padded_hour() {
    let (_temp_dir, config_path) = temp_config_path();

    Command::cargo_bin("ztl")
        .unwrap()
        .arg("--config")
        .arg(&config_path)
        .args(["--plain", "--time", "07"])
        .assert()
        .success();
}

#[test]
fn time_flag_accepts_hour_and_minutes() {
    let (_temp_dir, config_path) = temp_config_path();

    Command::cargo_bin("ztl")
        .unwrap()
        .arg("--config")
        .arg(&config_path)
        .args(["--plain", "--time", "07:30"])
        .assert()
        .success();
}

#[test]
fn time_flag_rejects_invalid_hour() {
    Command::cargo_bin("ztl")
        .unwrap()
        .args(["--time", "24"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("24 is not valid time HH[:MM]"));
}
