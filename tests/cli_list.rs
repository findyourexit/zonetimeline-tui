use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn list_command_prints_common_iana_zone_names() {
    Command::cargo_bin("ztl")
        .unwrap()
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("UTC"))
        .stdout(predicate::str::contains("Europe/London"));
}
