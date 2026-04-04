mod support;

use assert_cmd::Command;
use insta::assert_snapshot;
use predicates::prelude::*;
use zonetimeline_tui::core::model::ComparisonModel;
use zonetimeline_tui::render::plain::render_plain;

fn temp_config_path() -> (tempfile::TempDir, std::path::PathBuf) {
    let temp_dir = tempfile::tempdir().unwrap();
    let config_path = temp_dir.path().join("config.toml");
    (temp_dir, config_path)
}

#[test]
fn plain_output_matches_the_original_shape() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let display_order: Vec<usize> = {
        let mut indices: Vec<usize> = (0..model.zones.len()).collect();
        indices.sort_by(|&a, &b| {
            let oa = model.zones[a]
                .handle
                .utc_offset_seconds(support::fixed_now());
            let ob = model.zones[b]
                .handle
                .utc_offset_seconds(support::fixed_now());
            oa.cmp(&ob)
        });
        indices
    };
    let output = render_plain(&model, 60, &display_order);

    assert_snapshot!(output, @r###"
    UTC:                 2026-04-01 12:30:00
    America/New_York:    2026-04-01 08:30:00
    Europe/London:       2026-04-01 13:30:00

                                 ↓↓
    UTC:                 +00:00  06 07 08 09 10 11 12 13 14 15 16 17
    America/New_York:    -04:00  02 03 04 05 06 07 08 09 10 11 12 13
    Europe/London:       +01:00  07 08 09 10 11 12 13 14 15 16 17 18
                                 ↑↑
    "###);
}

#[test]
fn plain_output_respects_requested_width() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let display_order: Vec<usize> = (0..model.zones.len()).collect();
    let wide = render_plain(&model, 96, &display_order);
    let narrow = render_plain(&model, 72, &display_order);

    assert!(wide.lines().nth(6).unwrap().len() > narrow.lines().nth(6).unwrap().len());
}

#[test]
fn cli_plain_mode_prints_timeline_output() {
    let (_temp_dir, config_path) = temp_config_path();

    Command::cargo_bin("ztl")
        .unwrap()
        .arg("--config")
        .arg(&config_path)
        .args([
            "--plain",
            "--time",
            "07:30",
            "--zones",
            "Europe/London,America/New_York",
            "--nhours",
            "12",
            "--width",
            "60",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("UTC:"))
        .stdout(predicate::str::contains("Europe/London:"))
        .stdout(predicate::str::contains("America/New_York:"))
        .stdout(predicate::str::contains("↓↓"))
        .stdout(predicate::str::contains("07 08 09"));
}

#[test]
fn config_driven_plain_mode_and_width_affect_runtime_when_cli_omits_them() {
    let (_temp_dir, config_path) = temp_config_path();
    std::fs::write(
        &config_path,
        "[general]\nplain = true\nwidth = 60\nzones = [\"Europe/London\", \"America/New_York\"]\nnhours = 12\nanchor_time = \"07:30\"\n",
    )
    .unwrap();

    Command::cargo_bin("ztl")
        .unwrap()
        .arg("--config")
        .arg(&config_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("UTC:"))
        .stdout(predicate::str::contains("Europe/London:"))
        .stdout(predicate::str::contains("America/New_York:"))
        .stdout(predicate::str::contains("↓↓"))
        .stdout(predicate::str::contains("07 08 09"));
}
