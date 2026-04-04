mod support;

use chrono::{FixedOffset, NaiveTime, TimeZone, Utc};
use zonetimeline_tui::core::model::{AnchorSpec, ComparisonModel, SortMode};
use zonetimeline_tui::core::timezones::{ZoneHandle, format_utc_offset, parse_zone};

#[test]
fn parse_zone_accepts_local_utc_offsets_and_iana_names() {
    assert!(parse_zone("local").is_ok());
    assert!(parse_zone("UTC").is_ok());
    assert!(parse_zone("GMT").is_ok());
    assert!(parse_zone("UTC+2").is_ok());
    assert!(parse_zone("GMT-3").is_ok());
    assert!(parse_zone("Europe/London").is_ok());
}

#[test]
fn explicit_time_uses_the_current_utc_calendar_date() {
    let anchor = AnchorSpec::Explicit(NaiveTime::from_hms_opt(8, 15, 0).unwrap())
        .resolve(support::fixed_now());

    assert_eq!(anchor.to_rfc3339(), "2026-04-01T08:15:00+00:00");
}

#[test]
fn duplicate_resolved_zones_are_removed_by_identity() {
    let mut seed = support::fixture_seed();
    seed.ordered_zones = vec!["UTC".into(), "GMT".into(), "Europe/London".into()];

    let model = ComparisonModel::build(seed, support::fixed_now()).unwrap();

    assert_eq!(model.zones.len(), 2);
    assert_eq!(model.zones[0].label, "UTC");
    assert_eq!(
        model.session().ordered_zones,
        vec!["UTC".to_string(), "Europe/London".to_string()]
    );
}

#[test]
fn odd_nhours_produces_exact_slot_count() {
    let mut seed = support::fixture_seed();
    seed.nhours = 5;

    let model = ComparisonModel::build(seed, support::fixed_now()).unwrap();

    assert_eq!(model.timeline_slots.len(), 5);
    assert_eq!(model.timeline_slots.first().unwrap().offset_hours, -2);
    assert_eq!(model.timeline_slots.last().unwrap().offset_hours, 2);
}

#[test]
fn overlap_segments_use_minute_precision_and_wraparound_windows() {
    let mut seed = support::fixture_seed();
    seed.nhours = 6;
    seed.anchor_time = Some(NaiveTime::from_hms_opt(23, 30, 0).unwrap());
    seed.work_hours.insert("UTC".into(), "22:00-06:00".into());
    seed.work_hours
        .insert("Europe/London".into(), "22:00-06:00".into());
    seed.work_hours
        .insert("America/New_York".into(), "17:00-01:00".into());

    let model = ComparisonModel::build(seed, support::fixed_now()).unwrap();

    assert!(!model.overlap_segments.is_empty());
    assert!(
        model
            .overlap_segments
            .iter()
            .all(|segment| segment.duration_minutes > 0)
    );
}

#[test]
fn overlap_segments_and_ranked_windows_stay_within_displayed_range() {
    let mut seed = support::fixture_seed();
    seed.ordered_zones = vec!["UTC".into()];
    seed.nhours = 2;
    seed.work_hours.insert("UTC".into(), "22:00-23:00".into());

    let model = ComparisonModel::build(seed, support::fixed_now()).unwrap();

    assert!(model.overlap_segments.is_empty());
    assert!(model.classified_windows().is_empty());
}

#[test]
fn rebuilding_from_session_preserves_custom_window_for_canonical_zone_inputs() {
    let mut seed = support::fixture_seed();
    seed.ordered_zones = vec!["utc+2".into()];
    seed.work_hours.insert("utc+2".into(), "14:00-14:15".into());

    let model = ComparisonModel::build(seed, support::fixed_now()).unwrap();
    let rebuilt =
        ComparisonModel::from_session(model.session().clone(), support::fixed_now()).unwrap();

    assert_eq!(model.session().ordered_zones, vec!["utc+2".to_string()]);
    assert_eq!(rebuilt.session().ordered_zones, vec!["utc+2".to_string()]);
    assert_eq!(rebuilt.zones[0].window, model.zones[0].window);
}

#[test]
fn overlap_segments_respect_the_actual_anchor_instant_at_range_edges() {
    let mut seed = support::fixture_seed();
    seed.ordered_zones = vec!["UTC".into()];
    seed.nhours = 2;
    seed.anchor_time = Some(NaiveTime::from_hms_opt(12, 30, 0).unwrap());
    seed.work_hours.insert("UTC".into(), "13:00-13:30".into());

    let model = ComparisonModel::build(seed, support::fixed_now()).unwrap();
    let segments = model
        .overlap_segments
        .iter()
        .map(|segment| {
            (
                segment.start_utc.to_rfc3339(),
                segment.end_utc.to_rfc3339(),
                segment.duration_minutes,
            )
        })
        .collect::<Vec<_>>();

    assert!(segments.contains(&(
        "2026-04-01T13:00:00+00:00".to_string(),
        "2026-04-01T13:30:00+00:00".to_string(),
        30,
    )));
}

#[test]
fn classified_windows_sort_ideal_first_then_feasible() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let windows = model.classified_windows();

    // Verify Ideal windows come before Feasible windows
    let mut seen_feasible = false;
    for w in windows {
        if w.tier == WindowTier::Feasible {
            seen_feasible = true;
        }
        if w.tier == WindowTier::Ideal && seen_feasible {
            panic!("Ideal window found after Feasible window");
        }
    }
}

#[test]
fn sort_mode_next_cycles_through_all_modes() {
    let mut mode = SortMode::UtcOffsetAsc;
    mode = mode.next();
    assert_eq!(mode, SortMode::UtcOffsetDesc);
    mode = mode.next();
    assert_eq!(mode, SortMode::LabelAsc);
    mode = mode.next();
    assert_eq!(mode, SortMode::LabelDesc);
    mode = mode.next();
    assert_eq!(mode, SortMode::Manual);
    mode = mode.next();
    assert_eq!(mode, SortMode::UtcOffsetAsc);
}

#[test]
fn sort_mode_label_returns_short_display_strings() {
    assert_eq!(SortMode::UtcOffsetAsc.label(), "UTC+");
    assert_eq!(SortMode::UtcOffsetDesc.label(), "UTC-");
    assert_eq!(SortMode::LabelAsc.label(), "A-Z");
    assert_eq!(SortMode::LabelDesc.label(), "Z-A");
    assert_eq!(SortMode::Manual.label(), "Manual");
}

#[test]
fn sort_mode_default_is_utc_offset_asc() {
    assert_eq!(SortMode::default(), SortMode::UtcOffsetAsc);
}

#[test]
fn utc_offset_seconds_returns_zero_for_utc() {
    let handle = ZoneHandle::Fixed(FixedOffset::east_opt(0).unwrap());
    let now = Utc.with_ymd_and_hms(2026, 4, 1, 12, 0, 0).unwrap();
    assert_eq!(handle.utc_offset_seconds(now), 0);
}

#[test]
fn utc_offset_seconds_returns_correct_value_for_fixed_offset() {
    let handle = ZoneHandle::Fixed(FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap());
    let now = Utc.with_ymd_and_hms(2026, 4, 1, 12, 0, 0).unwrap();
    assert_eq!(handle.utc_offset_seconds(now), 5 * 3600 + 30 * 60);
}

#[test]
fn utc_offset_seconds_resolves_dst_for_named_timezone() {
    let handle = parse_zone("America/New_York").unwrap();
    // April 1 2026 — EDT is active (UTC-4)
    let summer = Utc.with_ymd_and_hms(2026, 4, 1, 12, 0, 0).unwrap();
    assert_eq!(handle.utc_offset_seconds(summer), -4 * 3600);

    // January 15 2026 — EST is active (UTC-5)
    let winter = Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap();
    assert_eq!(handle.utc_offset_seconds(winter), -5 * 3600);
}

#[test]
fn display_label_for_local_resolves_iana_name_with_suffix() {
    let label = ZoneHandle::display_label("local");
    // Should NOT be the literal string "local"
    assert_ne!(label, "local");
    // Should end with "(Local)"
    assert!(
        label.ends_with("(Local)"),
        "expected label to end with '(Local)', got: {label}"
    );
    // Should contain a slash (IANA names like "Australia/Sydney")
    // OR be a valid timezone name — at minimum, not "local"
    assert!(label.len() > 10, "label too short: {label}");
}

#[test]
fn format_utc_offset_positive() {
    assert_eq!(format_utc_offset(36000), "+10:00");
}

#[test]
fn format_utc_offset_negative() {
    assert_eq!(format_utc_offset(-14400), "-04:00");
}

#[test]
fn format_utc_offset_zero() {
    assert_eq!(format_utc_offset(0), "+00:00");
}

#[test]
fn format_utc_offset_half_hour() {
    assert_eq!(format_utc_offset(19800), "+05:30");
}

use zonetimeline_tui::core::model::{MinuteClass, WindowTier};

#[test]
fn minute_class_variants_are_distinct() {
    assert_ne!(MinuteClass::Ideal, MinuteClass::Feasible);
    assert_ne!(MinuteClass::Feasible, MinuteClass::Partial(2));
    assert_ne!(MinuteClass::Partial(2), MinuteClass::None);
    assert_eq!(MinuteClass::Partial(3), MinuteClass::Partial(3));
}

#[test]
fn window_tier_variants_are_distinct() {
    assert_ne!(WindowTier::Ideal, WindowTier::Feasible);
    assert_ne!(WindowTier::Feasible, WindowTier::LeastBad);
}

#[test]
fn bitmap_all_ideal_when_zones_share_same_window() {
    let mut seed = support::fixture_seed();
    seed.ordered_zones = vec!["Europe/London".to_string()];
    seed.nhours = 2;
    seed.anchor_time = Some(NaiveTime::from_hms_opt(14, 0, 0).unwrap());
    seed.work_hours
        .insert("Europe/London".to_string(), "14:00-16:00".to_string());

    let model = ComparisonModel::build(seed, support::fixed_now()).unwrap();
    let windows = model.classified_windows();

    assert!(windows.iter().any(|w| w.tier == WindowTier::Ideal));
    assert!(!windows.iter().any(|w| w.tier == WindowTier::LeastBad));
}

#[test]
fn bitmap_single_zone_has_ideal_and_feasible() {
    let mut seed = support::fixture_seed();
    seed.ordered_zones = vec!["Europe/London".to_string()];
    seed.nhours = 6;
    seed.anchor_time = Some(NaiveTime::from_hms_opt(14, 0, 0).unwrap());
    seed.work_hours
        .insert("Europe/London".to_string(), "14:00-16:00".to_string());
    seed.shoulder_hours = 1;

    let model = ComparisonModel::build(seed, support::fixed_now()).unwrap();
    let windows = model.classified_windows();

    let has_ideal = windows.iter().any(|w| w.tier == WindowTier::Ideal);
    let has_feasible = windows.iter().any(|w| w.tier == WindowTier::Feasible);
    assert!(has_ideal, "single zone should have Ideal windows");
    assert!(
        has_feasible,
        "single zone with shoulder should have Feasible windows"
    );
    assert!(!windows.iter().any(|w| w.tier == WindowTier::LeastBad));
}

#[test]
fn bitmap_no_overlap_produces_least_bad() {
    let mut seed = support::fixture_seed();
    seed.ordered_zones = vec!["Europe/London".to_string(), "America/New_York".to_string()];
    seed.nhours = 6;
    seed.anchor_time = Some(NaiveTime::from_hms_opt(12, 0, 0).unwrap());
    // At 12:00 UTC: London is BST (UTC+1) = 13:00 local, NY is EDT (UTC-4) = 08:00 local
    // Timeline spans 09:00-15:00 UTC
    // London 13:00-14:00 local = 12:00-13:00 UTC (in range, only London active)
    // NY 08:00-09:00 local = 12:00-13:00 UTC... but wait, that overlaps with London!
    // Use non-overlapping windows instead:
    // London 15:00-16:00 local = 14:00-15:00 UTC (in range)
    // NY 05:00-06:00 local = 09:00-10:00 UTC (in range)
    // These don't overlap at all in UTC, so we get Partial (1 zone each) but never 2 zones
    seed.work_hours
        .insert("Europe/London".to_string(), "15:00-16:00".to_string());
    seed.work_hours
        .insert("America/New_York".to_string(), "05:00-06:00".to_string());
    seed.shoulder_hours = 0;

    let model = ComparisonModel::build(seed, support::fixed_now()).unwrap();
    let windows = model.classified_windows();

    assert!(!windows.iter().any(|w| w.tier == WindowTier::Ideal));
    assert!(!windows.iter().any(|w| w.tier == WindowTier::Feasible));
    assert!(windows.iter().any(|w| w.tier == WindowTier::LeastBad));
}

#[test]
fn bitmap_shoulder_creates_feasible_between_zones() {
    let mut seed = support::fixture_seed();
    seed.ordered_zones = vec!["Europe/London".to_string(), "America/New_York".to_string()];
    seed.nhours = 24;
    seed.anchor_time = Some(NaiveTime::from_hms_opt(12, 0, 0).unwrap());
    // Default 09:00-17:00 windows with 1hr shoulder
    seed.shoulder_hours = 1;

    let model = ComparisonModel::build(seed, support::fixed_now()).unwrap();
    let windows = model.classified_windows();

    let has_ideal = windows.iter().any(|w| w.tier == WindowTier::Ideal);
    let has_feasible = windows.iter().any(|w| w.tier == WindowTier::Feasible);
    assert!(has_ideal, "should have ideal overlap");
    assert!(has_feasible, "shoulder should create feasible extensions");
}

#[test]
fn bitmap_feasible_segments_exclude_ideal_minutes() {
    let mut seed = support::fixture_seed();
    seed.ordered_zones = vec!["Europe/London".to_string(), "America/New_York".to_string()];
    seed.nhours = 24;
    seed.anchor_time = Some(NaiveTime::from_hms_opt(12, 0, 0).unwrap());
    seed.shoulder_hours = 1;

    let model = ComparisonModel::build(seed, support::fixed_now()).unwrap();
    let windows = model.classified_windows();

    // Check that no Feasible window overlaps any Ideal window
    let ideal_windows: Vec<_> = windows
        .iter()
        .filter(|w| w.tier == WindowTier::Ideal)
        .collect();
    let feasible_windows: Vec<_> = windows
        .iter()
        .filter(|w| w.tier == WindowTier::Feasible)
        .collect();

    for f in &feasible_windows {
        for i in &ideal_windows {
            let overlaps = f.start_utc < i.end_utc && f.end_utc > i.start_utc;
            assert!(
                !overlaps,
                "Feasible {:?}-{:?} overlaps Ideal {:?}-{:?}",
                f.start_utc, f.end_utc, i.start_utc, i.end_utc
            );
        }
    }
}

#[test]
fn bitmap_empty_zones_produces_no_windows() {
    let mut seed = support::fixture_seed();
    seed.ordered_zones = vec!["UTC".into()];
    seed.nhours = 2;
    seed.work_hours
        .insert("UTC".to_string(), "22:00-23:00".to_string());

    let model = ComparisonModel::build(seed, support::fixed_now()).unwrap();
    let windows = model.classified_windows();

    assert!(
        windows.is_empty(),
        "no windows when zone's window is entirely outside visible range"
    );
}

#[test]
fn derive_overlap_segments_matches_ideal_windows() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();

    let mut ideal_windows: Vec<_> = model
        .classified_windows()
        .iter()
        .filter(|w| w.tier == WindowTier::Ideal)
        .cloned()
        .collect();

    // Sort ideal windows chronologically to match overlap_segments order
    // (classified_windows() sorts by duration-first, overlap_segments are chronological)
    ideal_windows.sort_by_key(|w| w.start_utc);

    // Each overlap segment should correspond to an Ideal window
    assert_eq!(
        model.overlap_segments.len(),
        ideal_windows.len(),
        "overlap_segments count should match Ideal windows count"
    );

    for (seg, ideal) in model.overlap_segments.iter().zip(ideal_windows.iter()) {
        assert_eq!(seg.start_utc, ideal.start_utc);
        assert_eq!(seg.end_utc, ideal.end_utc);
        assert_eq!(seg.duration_minutes, ideal.duration_minutes);
    }
}
