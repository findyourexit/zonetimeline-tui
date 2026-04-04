use std::collections::BTreeMap;
use std::fs;

use chrono::NaiveTime;
use clap::Parser;
use tempfile::tempdir;
use zonetimeline_tui::cli::Cli;
use zonetimeline_tui::config::{
    ConfigRoots, ConfigSource, FileConfig, load_file_config, locate_config, merge_with_cli,
    save_session,
};
use zonetimeline_tui::core::model::{AnchorSpec, SessionConfig, SortMode};

#[test]
fn explicit_config_path_controls_load_and_save_target() {
    let root = tempdir().unwrap();
    let explicit = root.path().join("custom.toml");
    fs::write(&explicit, "[general]\nnhours = 8\n").unwrap();

    let source = locate_config(Some(explicit.clone()), &ConfigRoots::from_base(root.path()));

    assert_eq!(source.load_path, Some(explicit.clone()));
    assert_eq!(source.save_path, explicit);
}

#[test]
fn missing_new_config_falls_back_to_legacy_for_read_but_not_write() {
    let root = tempdir().unwrap();
    let legacy = root.path().join("zonetimeline").join("config");
    fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    fs::write(&legacy, "[general]\nnhours = 12\n").unwrap();

    let source = locate_config(None, &ConfigRoots::from_base(root.path()));

    assert_eq!(source.load_path, Some(legacy));
    assert_eq!(
        source.save_path,
        root.path().join("zonetimeline-tui").join("config.toml")
    );
}

#[test]
fn new_config_wins_over_legacy_when_both_exist() {
    let root = tempdir().unwrap();
    let roots = ConfigRoots::from_base(root.path());

    fs::create_dir_all(roots.new_config_path.parent().unwrap()).unwrap();
    fs::create_dir_all(roots.legacy_config_path.parent().unwrap()).unwrap();
    fs::write(&roots.new_config_path, "[general]\nnhours = 8\n").unwrap();
    fs::write(&roots.legacy_config_path, "[general]\nnhours = 12\n").unwrap();

    let source = locate_config(None, &roots);

    assert_eq!(source.load_path, Some(roots.new_config_path.clone()));
    assert_eq!(source.save_path, roots.new_config_path);
}

#[test]
fn missing_configs_default_save_target_to_new_path() {
    let root = tempdir().unwrap();
    let roots = ConfigRoots::from_base(root.path());

    let source = locate_config(None, &roots);

    assert_eq!(source.load_path, None);
    assert_eq!(source.save_path, roots.new_config_path);
}

#[test]
fn load_file_config_returns_default_when_file_is_missing() {
    let source = ConfigSource::new(None, std::env::temp_dir().join("config.toml"));

    let loaded = load_file_config(&source).unwrap();

    assert_eq!(loaded, FileConfig::default());
}

#[test]
fn load_file_config_parses_existing_config_and_work_hours() {
    let root = tempdir().unwrap();
    let path = root.path().join("config.toml");
    fs::write(
        &path,
        "[general]\nzones = [\"UTC\"]\nzone = [\"Europe/London\"]\nnhours = 10\n\n[overlap]\ndefault_window = \"08:00-16:00\"\n\n[overlap.work_hours]\nweekday = \"09:00-17:00\"\n",
    )
    .unwrap();

    let loaded = load_file_config(&ConfigSource::new(
        Some(path),
        std::env::temp_dir().join("save.toml"),
    ))
    .unwrap();

    let mut work_hours = BTreeMap::new();
    work_hours.insert("weekday".to_string(), "09:00-17:00".to_string());
    assert_eq!(
        loaded,
        FileConfig::from_parts(
            vec!["UTC".into()],
            vec!["Europe/London".into()],
            Some(10),
            "08:00-16:00".into(),
            work_hours,
        )
    );
}

#[test]
fn load_file_config_rejects_invalid_anchor_time() {
    let root = tempdir().unwrap();
    let path = root.path().join("config.toml");
    fs::write(&path, "[general]\nanchor_time = \"25:99\"\n").unwrap();

    let error = load_file_config(&ConfigSource::new(
        Some(path.clone()),
        root.path().join("save.toml"),
    ))
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains(&format!("failed to parse config file {}", path.display()))
    );
}

#[test]
fn config_value_wins_when_nhours_flag_is_omitted() {
    let cli = Cli::parse_from(["ztl"]);
    let file = FileConfig::from_parts(
        vec!["UTC".into()],
        vec![],
        Some(36),
        "09:00-17:00".into(),
        BTreeMap::new(),
    );

    let merged = merge_with_cli(
        &cli,
        file,
        ConfigSource::new(None, std::env::temp_dir().join("config.toml")),
    );

    assert_eq!(merged.nhours, 36);
}

#[test]
fn cli_nhours_overrides_config_when_provided() {
    let cli = Cli::parse_from(["ztl", "--nhours", "48"]);
    let file = FileConfig::from_parts(
        vec!["UTC".into()],
        vec![],
        Some(36),
        "09:00-17:00".into(),
        BTreeMap::new(),
    );

    let merged = merge_with_cli(
        &cli,
        file,
        ConfigSource::new(None, std::env::temp_dir().join("config.toml")),
    );

    assert_eq!(merged.nhours, 48);
}

#[test]
fn missing_nhours_in_cli_and_config_uses_default_24() {
    let merged = merge_with_cli(
        &Cli::parse_from(["ztl"]),
        FileConfig::default(),
        ConfigSource::new(None, std::env::temp_dir().join("config.toml")),
    );

    assert_eq!(merged.nhours, 24);
}

#[test]
fn cli_zone_lists_override_per_field_and_seed_defaults_when_missing() {
    let cli = Cli::parse_from([
        "ztl",
        "--zones",
        "UTC,Europe/London",
        "--zone",
        "America/New_York",
    ]);
    let merged = merge_with_cli(
        &cli,
        FileConfig::default(),
        ConfigSource::new(None, std::env::temp_dir().join("config.toml")),
    );

    assert_eq!(
        merged.ordered_zones,
        vec!["Europe/London".to_string(), "America/New_York".to_string(),]
    );
    assert_eq!(merged.base_zones, vec!["Europe/London".to_string()]);
    assert_eq!(merged.extra_zones, vec!["America/New_York".to_string()]);

    let defaults = merge_with_cli(
        &Cli::parse_from(["ztl"]),
        FileConfig::default(),
        ConfigSource::new(None, std::env::temp_dir().join("config.toml")),
    );

    assert_eq!(
        defaults.ordered_zones,
        vec![
            "local".to_string(),
            "America/New_York".to_string(),
            "Europe/London".to_string(),
        ]
    );
}

#[test]
fn cli_zones_override_config_zones_but_keep_config_zone_entries() {
    let file = FileConfig::from_parts(
        vec!["Asia/Tokyo".into()],
        vec!["Europe/London".into()],
        None,
        "09:00-17:00".into(),
        BTreeMap::new(),
    );
    let merged = merge_with_cli(
        &Cli::parse_from(["ztl", "--zones", "UTC,America/New_York"]),
        file,
        ConfigSource::new(None, std::env::temp_dir().join("config.toml")),
    );

    assert_eq!(
        merged.ordered_zones,
        vec!["America/New_York".to_string(), "Europe/London".to_string(),]
    );
    assert_eq!(merged.base_zones, vec!["America/New_York".to_string()]);
    assert_eq!(merged.extra_zones, vec!["Europe/London".to_string()]);
}

#[test]
fn cli_zone_overrides_config_zone_but_keeps_config_zones_entries() {
    let file = FileConfig::from_parts(
        vec!["UTC".into(), "Europe/London".into()],
        vec!["Asia/Tokyo".into()],
        None,
        "09:00-17:00".into(),
        BTreeMap::new(),
    );
    let merged = merge_with_cli(
        &Cli::parse_from(["ztl", "--zone", "America/New_York"]),
        file,
        ConfigSource::new(None, std::env::temp_dir().join("config.toml")),
    );

    assert_eq!(
        merged.ordered_zones,
        vec!["Europe/London".to_string(), "America/New_York".to_string(),]
    );
    assert_eq!(merged.base_zones, vec!["Europe/London".to_string()]);
    assert_eq!(merged.extra_zones, vec!["America/New_York".to_string()]);
}

#[test]
fn merge_with_cli_carries_time_width_and_plain_into_session_seed() {
    let cli = Cli::parse_from(["ztl", "--time", "07:30", "--width", "120", "--plain"]);
    let merged = merge_with_cli(
        &cli,
        FileConfig::default(),
        ConfigSource::new(None, std::env::temp_dir().join("config.toml")),
    );

    assert_eq!(
        merged.anchor_time,
        Some(NaiveTime::from_hms_opt(7, 30, 0).unwrap())
    );
    assert_eq!(merged.width, Some(120));
    assert!(merged.plain);
}

#[test]
fn saved_anchor_width_and_plain_round_trip_when_cli_omits_them() {
    let root = tempdir().unwrap();
    let path = root.path().join("config.toml");
    save_session(&SessionConfig {
        base_zones: vec!["UTC".to_string()],
        extra_zones: Vec::new(),
        ordered_zones: vec!["UTC".to_string()],
        nhours: 12,
        anchor: AnchorSpec::Explicit(NaiveTime::from_hms_opt(7, 30, 0).unwrap()),
        width: Some(120),
        plain: true,
        save_path: path.clone(),
        default_window: "09:00-17:00".to_string(),
        work_hours: BTreeMap::new(),
        shoulder_hours: 1,
        sort_mode: SortMode::default(),
    })
    .unwrap();

    let loaded = load_file_config(&ConfigSource::new(
        Some(path),
        root.path().join("save.toml"),
    ))
    .unwrap();
    let merged = merge_with_cli(
        &Cli::parse_from(["ztl"]),
        loaded,
        ConfigSource::new(None, std::env::temp_dir().join("config.toml")),
    );

    assert_eq!(
        merged.anchor_time,
        Some(NaiveTime::from_hms_opt(7, 30, 0).unwrap())
    );
    assert_eq!(merged.width, Some(120));
    assert!(merged.plain);
}

#[test]
fn save_session_accepts_relative_explicit_config_path() {
    let root = tempdir().unwrap();
    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(root.path()).unwrap();

    let result = save_session(&SessionConfig {
        base_zones: vec!["UTC".to_string()],
        extra_zones: Vec::new(),
        ordered_zones: vec!["UTC".to_string()],
        nhours: 12,
        anchor: AnchorSpec::Now,
        width: None,
        plain: false,
        save_path: std::path::PathBuf::from("config.toml"),
        default_window: "09:00-17:00".to_string(),
        work_hours: BTreeMap::new(),
        shoulder_hours: 1,
        sort_mode: SortMode::default(),
    });

    std::env::set_current_dir(original_dir).unwrap();

    result.unwrap();
    assert!(root.path().join("config.toml").exists());
}

#[test]
fn save_reload_preserves_zones_and_additive_zone_semantics() {
    let root = tempdir().unwrap();
    let path = root.path().join("config.toml");
    fs::write(
        &path,
        "[general]\nzones = [\"UTC\", \"Europe/London\"]\nzone = [\"Asia/Tokyo\"]\nnhours = 12\n",
    )
    .unwrap();

    let loaded = load_file_config(&ConfigSource::new(
        Some(path.clone()),
        root.path().join("save.toml"),
    ))
    .unwrap();
    let merged = merge_with_cli(
        &Cli::parse_from(["ztl"]),
        loaded,
        ConfigSource::new(None, path.clone()),
    );

    save_session(&SessionConfig::from(merged)).unwrap();

    let reloaded = load_file_config(&ConfigSource::new(
        Some(path),
        root.path().join("save.toml"),
    ))
    .unwrap();
    let remerged = merge_with_cli(
        &Cli::parse_from(["ztl", "--zones", "America/New_York"]),
        reloaded,
        ConfigSource::new(None, std::env::temp_dir().join("config.toml")),
    );

    assert_eq!(
        remerged.ordered_zones,
        vec!["America/New_York".to_string(), "Asia/Tokyo".to_string(),]
    );
}

#[test]
fn save_reload_preserves_reordered_mixed_bucket_visible_order_and_additive_semantics() {
    let root = tempdir().unwrap();
    let path = root.path().join("config.toml");
    fs::write(
        &path,
        "[general]\nzones = [\"UTC\", \"Europe/London\"]\nzone = [\"Asia/Tokyo\"]\nnhours = 12\n",
    )
    .unwrap();

    let loaded = load_file_config(&ConfigSource::new(
        Some(path.clone()),
        root.path().join("save.toml"),
    ))
    .unwrap();
    let mut merged = merge_with_cli(
        &Cli::parse_from(["ztl"]),
        loaded,
        ConfigSource::new(None, path.clone()),
    );
    merged.ordered_zones = vec![
        "Asia/Tokyo".to_string(),
        "UTC".to_string(),
        "Europe/London".to_string(),
    ];

    save_session(&SessionConfig::from(merged)).unwrap();

    let reloaded = load_file_config(&ConfigSource::new(
        Some(path.clone()),
        root.path().join("save.toml"),
    ))
    .unwrap();
    let restart = merge_with_cli(
        &Cli::parse_from(["ztl"]),
        reloaded.clone(),
        ConfigSource::new(None, std::env::temp_dir().join("config.toml")),
    );
    let remerged = merge_with_cli(
        &Cli::parse_from(["ztl", "--zones", "America/New_York"]),
        reloaded,
        ConfigSource::new(None, std::env::temp_dir().join("config.toml")),
    );

    assert_eq!(
        restart.ordered_zones,
        vec!["Asia/Tokyo".to_string(), "Europe/London".to_string(),]
    );
    assert_eq!(
        remerged.ordered_zones,
        vec!["America/New_York".to_string(), "Asia/Tokyo".to_string(),]
    );
}

#[test]
fn shoulder_hours_round_trips_through_save_and_reload() {
    let root = tempdir().unwrap();
    let path = root.path().join("config.toml");
    save_session(&SessionConfig {
        base_zones: vec!["UTC".to_string()],
        extra_zones: Vec::new(),
        ordered_zones: vec!["UTC".to_string()],
        nhours: 12,
        anchor: AnchorSpec::Now,
        width: None,
        plain: false,
        save_path: path.clone(),
        default_window: "09:00-17:00".to_string(),
        work_hours: BTreeMap::new(),
        shoulder_hours: 2,
        sort_mode: SortMode::default(),
    })
    .unwrap();

    let loaded = load_file_config(&ConfigSource::new(
        Some(path),
        root.path().join("save.toml"),
    ))
    .unwrap();
    let merged = merge_with_cli(
        &Cli::parse_from(["ztl"]),
        loaded,
        ConfigSource::new(None, std::env::temp_dir().join("config.toml")),
    );

    assert_eq!(merged.shoulder_hours, 2);
}

#[test]
fn cli_shoulder_hours_overrides_config() {
    let file = FileConfig::from_parts(
        vec!["UTC".into()],
        vec![],
        None,
        "09:00-17:00".into(),
        BTreeMap::new(),
    );
    let merged = merge_with_cli(
        &Cli::parse_from(["ztl", "--shoulder-hours", "3"]),
        file,
        ConfigSource::new(None, std::env::temp_dir().join("config.toml")),
    );

    assert_eq!(merged.shoulder_hours, 3);
}

#[test]
fn default_shoulder_hours_is_1() {
    let merged = merge_with_cli(
        &Cli::parse_from(["ztl"]),
        FileConfig::default(),
        ConfigSource::new(None, std::env::temp_dir().join("config.toml")),
    );

    assert_eq!(merged.shoulder_hours, 1);
}

#[test]
fn sort_mode_round_trips_through_save_and_reload() {
    let root = tempdir().unwrap();
    let path = root.path().join("config.toml");
    save_session(&SessionConfig {
        base_zones: vec!["Europe/London".to_string()],
        extra_zones: Vec::new(),
        ordered_zones: vec!["Europe/London".to_string()],
        nhours: 12,
        anchor: AnchorSpec::Now,
        width: None,
        plain: false,
        save_path: path.clone(),
        default_window: "09:00-17:00".to_string(),
        work_hours: BTreeMap::new(),
        shoulder_hours: 1,
        sort_mode: SortMode::LabelDesc,
    })
    .unwrap();

    let loaded = load_file_config(&ConfigSource::new(
        Some(path),
        root.path().join("save.toml"),
    ))
    .unwrap();
    let merged = merge_with_cli(
        &Cli::parse_from(["ztl"]),
        loaded,
        ConfigSource::new(None, std::env::temp_dir().join("config.toml")),
    );

    assert_eq!(merged.sort_mode, SortMode::LabelDesc);
}

#[test]
fn missing_sort_mode_in_config_defaults_to_utc_offset_asc() {
    let root = tempdir().unwrap();
    let path = root.path().join("config.toml");
    fs::write(&path, "[general]\nzones = [\"Europe/London\"]\n").unwrap();

    let loaded = load_file_config(&ConfigSource::new(
        Some(path),
        root.path().join("save.toml"),
    ))
    .unwrap();
    let merged = merge_with_cli(
        &Cli::parse_from(["ztl"]),
        loaded,
        ConfigSource::new(None, std::env::temp_dir().join("config.toml")),
    );

    assert_eq!(merged.sort_mode, SortMode::UtcOffsetAsc);
}

#[test]
fn utc_in_saved_zones_is_filtered_on_load() {
    let root = tempdir().unwrap();
    let path = root.path().join("config.toml");
    fs::write(
        &path,
        "[general]\nzones = [\"UTC\", \"Europe/London\"]\nordered_zones = [\"UTC\", \"Europe/London\"]\n",
    )
    .unwrap();

    let loaded = load_file_config(&ConfigSource::new(
        Some(path),
        root.path().join("save.toml"),
    ))
    .unwrap();
    let merged = merge_with_cli(
        &Cli::parse_from(["ztl"]),
        loaded,
        ConfigSource::new(None, std::env::temp_dir().join("config.toml")),
    );

    assert!(!merged.ordered_zones.contains(&"UTC".to_string()));
    assert!(merged.ordered_zones.contains(&"Europe/London".to_string()));
}

#[test]
fn default_zones_excludes_utc() {
    use zonetimeline_tui::config::DEFAULT_ZONES;
    for zone in &DEFAULT_ZONES {
        assert_ne!(
            zone.to_ascii_uppercase(),
            "UTC",
            "DEFAULT_ZONES should not contain UTC"
        );
    }
}
