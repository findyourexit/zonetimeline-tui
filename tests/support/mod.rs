use std::collections::BTreeMap;

use chrono::{TimeZone, Utc};
use zonetimeline_tui::config::SessionSeed;
use zonetimeline_tui::core::model::SortMode;

pub fn fixed_now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 4, 1, 12, 30, 0).unwrap()
}

pub fn fixture_seed() -> SessionSeed {
    SessionSeed {
        base_zones: vec!["Europe/London".to_string(), "America/New_York".to_string()],
        extra_zones: Vec::new(),
        ordered_zones: vec!["Europe/London".to_string(), "America/New_York".to_string()],
        nhours: 12,
        anchor_time: None,
        width: Some(96),
        plain: true,
        save_path: std::env::temp_dir().join("ztl-test.toml"),
        default_window: "09:00-17:00".to_string(),
        work_hours: BTreeMap::new(),
        shoulder_hours: 1,
        sort_mode: SortMode::default(),
    }
}
