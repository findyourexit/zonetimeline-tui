mod support;

use chrono::{Duration as ChronoDuration, Timelike};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use tempfile::tempdir;
use zonetimeline_tui::core::model::{AnchorSpec, ComparisonModel, SessionConfig, SortMode};
use zonetimeline_tui::tui::state::AppState;
use zonetimeline_tui::tui::view::render_to_buffer;

#[test]
fn timeline_view_renders_grid_overlap_panels_and_cursor_details() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());
    let area = Rect::new(0, 0, 120, 36);
    let mut buffer = Buffer::empty(area);

    render_to_buffer(&mut buffer, area, &state);
    let text = buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    assert!(text.contains("Zone Timeline"));
    assert!(text.contains("Working Windows"));
    assert!(text.contains("Zones"));
    assert!(text.contains("Details"));
    // Now row removed — verify box-drawing frame instead
    assert!(text.contains("┌"), "should render box-drawing frame");
}

#[test]
fn tiny_term_shows_resize_guard() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());
    let area = Rect::new(0, 0, 60, 12);
    let mut buffer = Buffer::empty(area);

    render_to_buffer(&mut buffer, area, &state);
    let text = buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();

    assert!(text.contains("Resize terminal"));
}

#[test]
fn resize_guard_triggers_one_column_below_dynamic_minimum() {
    // The dynamic minimum width for the fixture (2 zones, 12 slots, UTC label 32 chars)
    // should be less than the old hardcoded 80. At (min_w - 1) the guard must trigger.
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());

    let (min_w, min_h) = zonetimeline_tui::tui::view::min_terminal_size(&state);
    assert!(
        min_w < 80,
        "dynamic minimum width ({min_w}) should be less than old hardcoded 80 for 12-slot fixture"
    );

    let area = Rect::new(0, 0, min_w - 1, min_h + 10);
    let mut buffer = Buffer::empty(area);
    render_to_buffer(&mut buffer, area, &state);
    let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();

    assert!(
        text.contains("Resize terminal"),
        "resize guard should trigger at width {} (one below min {min_w})",
        min_w - 1
    );
}

#[test]
fn resize_guard_does_not_trigger_at_exact_dynamic_minimum() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());

    let (min_w, min_h) = zonetimeline_tui::tui::view::min_terminal_size(&state);

    // Give enough height so only width matters
    let area = Rect::new(0, 0, min_w, min_h + 10);
    let mut buffer = Buffer::empty(area);
    render_to_buffer(&mut buffer, area, &state);
    let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();

    assert!(
        !text.contains("Resize terminal"),
        "resize guard should NOT trigger at exact dynamic minimum ({min_w}x{})",
        min_h + 10
    );
    assert!(
        text.contains("Zone Timeline"),
        "should render the timeline panel at exact minimum size"
    );
}

#[test]
fn resize_guard_message_shows_dynamic_dimensions() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());

    let (min_w, min_h) = zonetimeline_tui::tui::view::min_terminal_size(&state);

    let area = Rect::new(0, 0, min_w - 1, 12);
    let mut buffer = Buffer::empty(area);
    render_to_buffer(&mut buffer, area, &state);
    let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();

    let expected_dims = format!("{}x{}", min_w, min_h);
    assert!(
        text.contains(&expected_dims),
        "resize message should include dynamic dimensions '{expected_dims}', got: {text}"
    );
}

#[test]
fn resize_guard_triggers_one_row_below_dynamic_minimum_height() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());

    let (min_w, min_h) = zonetimeline_tui::tui::view::min_terminal_size(&state);

    let area = Rect::new(0, 0, min_w + 20, min_h - 1);
    let mut buffer = Buffer::empty(area);
    render_to_buffer(&mut buffer, area, &state);
    let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();

    assert!(
        text.contains("Resize terminal"),
        "resize guard should trigger at height {} (one below min {min_h})",
        min_h - 1
    );
}

#[test]
fn timeline_view_compacts_24_hour_grid_on_narrow_terminal() {
    let mut seed = support::fixture_seed();
    seed.ordered_zones = vec!["UTC".to_string()];
    seed.nhours = 24;
    seed.width = Some(120);

    let model = ComparisonModel::build(seed, support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());
    let area = Rect::new(0, 0, 120, 36);
    let mut buffer = Buffer::empty(area);

    render_to_buffer(&mut buffer, area, &state);

    // UTC data row is one row below the header row (row 6: header=3 + border=1 + header_row=1 + first_zone=1)
    let utc_data_row = (0..area.width)
        .map(|x| buffer.cell((x, 6)).unwrap().symbol())
        .collect::<String>();

    assert!(
        utc_data_row.contains("23"),
        "UTC data row was: {utc_data_row:?}"
    );
    assert!(
        utc_data_row.contains("+00:00"),
        "UTC data row should contain offset: {utc_data_row:?}"
    );
}

#[test]
fn timeline_view_compacts_default_grid_at_minimum_width() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());

    // Use the dynamic minimum size so the resize guard does not trigger
    let (min_w, min_h) = zonetimeline_tui::tui::view::min_terminal_size(&state);
    let area = Rect::new(0, 0, min_w, min_h);
    let mut buffer = Buffer::empty(area);

    render_to_buffer(&mut buffer, area, &state);

    // Find the UTC data row by scanning for "+00:00" (offset column)
    let utc_data_row = (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buffer.cell((x, y)).unwrap().symbol())
                .collect::<String>()
        })
        .find(|row| row.contains("+00:00"))
        .expect("should find a row containing +00:00");

    // In compact mode, time slots use HH format; count colons — should only have 2 from "+00:00"
    let colon_count = utc_data_row.matches(':').count();
    assert!(
        colon_count <= 2,
        "Expected at most 2 colons (from offset), got {colon_count} — UTC data row was: {utc_data_row:?}"
    );
    assert!(
        utc_data_row.contains("17"),
        "UTC data row was: {utc_data_row:?}"
    );
}

#[test]
fn refresh_now_updates_wall_clock_and_live_anchor_model() {
    let now = support::fixed_now();
    let mut state = AppState::new(
        ComparisonModel::from_session(
            SessionConfig {
                base_zones: vec!["UTC".to_string()],
                extra_zones: Vec::new(),
                ordered_zones: vec!["UTC".to_string()],
                nhours: 12,
                anchor: AnchorSpec::Now,
                width: Some(96),
                plain: true,
                save_path: std::env::temp_dir().join("ztl-refresh-now.toml"),
                default_window: "09:00-17:00".to_string(),
                work_hours: Default::default(),
                shoulder_hours: 1,
                sort_mode: SortMode::default(),
            },
            now,
        )
        .unwrap(),
        now,
    );

    let advanced = now + ChronoDuration::minutes(61);
    state.refresh_now(advanced).unwrap();

    assert_eq!(state.now_utc, advanced);
    assert_eq!(state.model.anchor, advanced);
    assert_eq!(
        state
            .model
            .timeline_slots
            .iter()
            .find(|slot| slot.offset_hours == 0)
            .and_then(|slot| slot.current_minute_offset),
        Some(advanced.minute())
    );
}

#[test]
fn refresh_now_updates_wall_clock_without_moving_explicit_anchor() {
    let now = support::fixed_now();
    let mut state = AppState::new(
        ComparisonModel::from_session(
            SessionConfig {
                base_zones: vec!["UTC".to_string()],
                extra_zones: Vec::new(),
                ordered_zones: vec!["UTC".to_string()],
                nhours: 12,
                anchor: AnchorSpec::Explicit(now.time()),
                width: Some(96),
                plain: true,
                save_path: std::env::temp_dir().join("ztl-refresh-explicit.toml"),
                default_window: "09:00-17:00".to_string(),
                work_hours: Default::default(),
                shoulder_hours: 1,
                sort_mode: SortMode::default(),
            },
            now,
        )
        .unwrap(),
        now,
    );

    let advanced = now + ChronoDuration::minutes(61);
    let anchor = state.model.anchor;
    state.refresh_now(advanced).unwrap();

    assert_eq!(state.now_utc, advanced);
    assert_eq!(state.model.anchor, anchor);
}

#[test]
fn refresh_now_rebuilds_explicit_anchor_when_utc_date_rolls_over() {
    let now = chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2026, 4, 1, 23, 30, 0).unwrap();
    let mut state = AppState::new(
        ComparisonModel::from_session(
            SessionConfig {
                base_zones: vec!["UTC".to_string()],
                extra_zones: Vec::new(),
                ordered_zones: vec!["UTC".to_string()],
                nhours: 12,
                anchor: AnchorSpec::Explicit(chrono::NaiveTime::from_hms_opt(23, 30, 0).unwrap()),
                width: Some(96),
                plain: true,
                save_path: std::env::temp_dir().join("ztl-refresh-explicit-midnight.toml"),
                default_window: "09:00-17:00".to_string(),
                work_hours: Default::default(),
                shoulder_hours: 1,
                sort_mode: SortMode::default(),
            },
            now,
        )
        .unwrap(),
        now,
    );

    let advanced = chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2026, 4, 2, 0, 5, 0).unwrap();
    state.refresh_now(advanced).unwrap();

    assert_eq!(state.now_utc, advanced);
    assert_eq!(state.model.anchor.date_naive(), advanced.date_naive());
    assert_eq!(
        state.model.anchor.time(),
        chrono::NaiveTime::from_hms_opt(23, 30, 0).unwrap()
    );
}

#[test]
fn add_remove_reorder_and_edit_window_rebuild_the_model() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    state.add_zone("Asia/Tokyo".to_string()).unwrap();
    state.move_zone_up(state.selected_zone);
    state.update_window(0, "08:00-16:00").unwrap();
    state.remove_zone(1).unwrap();

    assert_eq!(state.model.zones[0].input_name, "Asia/Tokyo");
    assert!(
        state
            .model
            .zones
            .iter()
            .any(|zone| zone.input_name == "Asia/Tokyo")
    );
    assert_eq!(state.model.zones[0].window.start_minute, 8 * 60);
    assert_eq!(state.model.zones[0].window.end_minute, 16 * 60);
    assert_eq!(
        state
            .session
            .work_hours
            .get("Asia/Tokyo")
            .map(String::as_str),
        Some("08:00-16:00")
    );
}

#[test]
fn explicit_save_writes_to_the_session_save_target() {
    let dir = tempdir().unwrap();
    let mut seed = support::fixture_seed();
    seed.save_path = dir.path().join("nested").join("custom.toml");
    let model = ComparisonModel::build(seed, support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    state.add_zone("Asia/Tokyo".to_string()).unwrap();
    state.save().unwrap();

    let saved = std::fs::read_to_string(dir.path().join("nested").join("custom.toml")).unwrap();
    assert!(saved.contains("Asia/Tokyo"));
}

#[test]
fn modal_text_entry_supports_submit_and_cancel() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    state.open_add_zone();
    for ch in "Asia/Tokyo".chars() {
        state.push_modal_char(ch);
    }
    state.submit_modal().unwrap();
    assert!(
        state
            .model
            .zones
            .iter()
            .any(|zone| zone.input_name == "Asia/Tokyo")
    );

    // EditWindow no longer uses text input; test open + cancel only
    state.open_edit_window();
    assert!(state.modal.is_some());
    state.cancel_modal();
    assert!(state.modal.is_none());
}

#[test]
fn edit_window_submit_always_produces_valid_window_from_slots() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    // Select first user zone (unified index 1), not UTC row (0)
    state.selected_zone = 1;

    state.open_edit_window();

    // Submit with default slot selections — should always succeed
    state.submit_modal().unwrap();
    assert!(state.modal.is_none());
}

#[test]
fn removing_a_zone_cleans_up_its_work_hours_override() {
    let mut seed = support::fixture_seed();
    seed.work_hours
        .insert("Europe/London".to_string(), "08:00-16:00".to_string());
    let model = ComparisonModel::build(seed, support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    // In UtcOffsetAsc, America/New_York (-4) is display_order[0], Europe/London (+1) is display_order[1]
    // Unified index 2 = Europe/London
    let london_unified = state
        .display_order
        .iter()
        .position(|&idx| state.model.zones[idx].input_name == "Europe/London")
        .map(|i| i + 1) // +1 for unified space (0=UTC)
        .unwrap();
    state.remove_zone(london_unified).unwrap();

    assert!(!state.session.work_hours.contains_key("Europe/London"));
}

#[test]
fn duplicate_alias_zone_submit_errors_and_keeps_modal_open() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    let original_session = state.session.clone();

    state.open_add_zone();
    for ch in "Europe/London".chars() {
        state.push_modal_char(ch);
    }

    let error = state.submit_modal().unwrap_err();

    assert!(error.to_string().contains("already present"));
    assert_eq!(state.session, original_session);
    assert!(matches!(
        state.modal,
        Some(zonetimeline_tui::tui::forms::Modal::AddZone { .. })
    ));
}

#[test]
fn modal_recovery_paths_clear_stale_status() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    state.status = Some("old error".to_string());
    state.open_add_zone();
    assert_eq!(state.status, None);

    state.cancel_modal();
    assert_eq!(state.status, None);

    state.status = Some("old error".to_string());
    state.open_add_zone();
    for ch in "Asia/Tokyo".chars() {
        state.push_modal_char(ch);
    }
    state.submit_modal().unwrap();

    assert_eq!(state.status, None);
    assert!(state.modal.is_none());
}

#[test]
fn dirty_edit_actions_clear_stale_status() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    state.status = Some("Saved config".to_string());
    state.add_zone("Asia/Tokyo".to_string()).unwrap();
    assert_eq!(state.status, None);

    // move_zone_up clears status even in non-Manual mode (no-op but status cleared)
    state.status = Some("Saved config".to_string());
    state.move_zone_up(state.selected_zone);
    assert_eq!(state.status, None);

    // update_window takes an ordered_zones index, not unified index
    state.status = Some("Saved config".to_string());
    state.update_window(0, "08:00-16:00").unwrap();
    assert_eq!(state.status, None);

    // remove_zone takes a unified index; use 1 to remove first user zone
    state.status = Some("Saved config".to_string());
    state.selected_zone = 1;
    state.remove_zone(state.selected_zone).unwrap();
    assert_eq!(state.status, None);
}

#[test]
fn opening_a_modal_hides_help_and_keeps_modal_state_active() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    state.show_help = true;
    state.open_add_zone();

    assert!(!state.show_help);
    assert!(matches!(
        state.modal,
        Some(zonetimeline_tui::tui::forms::Modal::AddZone { .. })
    ));
}

#[test]
fn noop_edit_attempts_clear_stale_status() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    state.status = Some("Saved config".to_string());
    state.move_zone_up(0);
    assert_eq!(state.status, None);

    state.status = Some("Saved config".to_string());
    state.move_zone_down(state.model.zones.len().saturating_sub(1));
    assert_eq!(state.status, None);

    let single_zone_model = ComparisonModel::from_session(
        SessionConfig {
            base_zones: vec!["UTC".to_string()],
            extra_zones: Vec::new(),
            ordered_zones: vec!["UTC".to_string()],
            nhours: 12,
            anchor: AnchorSpec::Now,
            width: Some(96),
            plain: true,
            save_path: std::env::temp_dir().join("ztl-noop-remove.toml"),
            default_window: "09:00-17:00".to_string(),
            work_hours: Default::default(),
            shoulder_hours: 1,
            sort_mode: SortMode::default(),
        },
        support::fixed_now(),
    )
    .unwrap();
    let mut single_zone_state = AppState::new(single_zone_model, support::fixed_now());

    single_zone_state.status = Some("Saved config".to_string());
    single_zone_state.remove_zone(0).unwrap();
    assert_eq!(single_zone_state.status, None);
}

#[test]
fn picker_opens_with_all_entries_and_empty_filter() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    state.open_add_zone();

    match &state.modal {
        Some(zonetimeline_tui::tui::forms::Modal::AddZone {
            input,
            entries,
            filtered,
            selected,
            scroll_offset,
        }) => {
            assert!(input.is_empty());
            assert!(
                entries.len() > 500,
                "should have ~593 entries, got {}",
                entries.len()
            );
            assert_eq!(filtered.len(), entries.len(), "empty filter shows all");
            assert_eq!(*selected, 0);
            assert_eq!(*scroll_offset, 0);
        }
        other => panic!("expected AddZone modal, got {:?}", other),
    }
}

#[test]
fn picker_filter_narrows_results_with_substring_match() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    state.open_add_zone();
    for ch in "tokyo".chars() {
        state.push_modal_char(ch);
    }

    match &state.modal {
        Some(zonetimeline_tui::tui::forms::Modal::AddZone {
            entries,
            filtered,
            selected,
            ..
        }) => {
            assert!(
                filtered.len() < entries.len(),
                "filtering should reduce the list"
            );
            assert_eq!(*selected, 0, "selected resets to 0 after typing");
            for &i in filtered {
                assert!(
                    entries[i].search_key.contains("tokyo"),
                    "entry '{}' should contain 'tokyo'",
                    entries[i].name
                );
            }
            assert!(
                filtered.iter().any(|&i| entries[i].name == "Asia/Tokyo"),
                "Asia/Tokyo should be in the filtered results"
            );
        }
        other => panic!("expected AddZone modal, got {:?}", other),
    }
}

#[test]
fn picker_backspace_widens_results() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    state.open_add_zone();
    for ch in "asia/tok".chars() {
        state.push_modal_char(ch);
    }

    let narrow_count = match &state.modal {
        Some(zonetimeline_tui::tui::forms::Modal::AddZone { filtered, .. }) => filtered.len(),
        _ => panic!("expected AddZone"),
    };

    state.pop_modal_char();
    state.pop_modal_char();
    state.pop_modal_char();

    let wider_count = match &state.modal {
        Some(zonetimeline_tui::tui::forms::Modal::AddZone { filtered, .. }) => filtered.len(),
        _ => panic!("expected AddZone"),
    };

    assert!(
        wider_count > narrow_count,
        "backspace should widen results: {} > {}",
        wider_count,
        narrow_count
    );
}

#[test]
fn picker_up_down_navigates_selection() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    state.open_add_zone();

    state.picker_down();
    match &state.modal {
        Some(zonetimeline_tui::tui::forms::Modal::AddZone { selected, .. }) => {
            assert_eq!(*selected, 1);
        }
        _ => panic!("expected AddZone"),
    }

    state.picker_down();
    match &state.modal {
        Some(zonetimeline_tui::tui::forms::Modal::AddZone { selected, .. }) => {
            assert_eq!(*selected, 2);
        }
        _ => panic!("expected AddZone"),
    }

    state.picker_up();
    match &state.modal {
        Some(zonetimeline_tui::tui::forms::Modal::AddZone { selected, .. }) => {
            assert_eq!(*selected, 1);
        }
        _ => panic!("expected AddZone"),
    }

    state.picker_up();
    state.picker_up();
    match &state.modal {
        Some(zonetimeline_tui::tui::forms::Modal::AddZone { selected, .. }) => {
            assert_eq!(*selected, 0);
        }
        _ => panic!("expected AddZone"),
    }
}

#[test]
fn picker_submit_uses_selected_entry() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    state.open_add_zone();
    for ch in "Asia/Tokyo".chars() {
        state.push_modal_char(ch);
    }

    state.submit_modal().unwrap();

    assert!(
        state
            .model
            .zones
            .iter()
            .any(|zone| zone.input_name == "Asia/Tokyo"),
        "Asia/Tokyo should be added"
    );
    assert!(state.modal.is_none());
}

#[test]
fn picker_submit_fallback_to_raw_text_for_custom_offset() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    state.open_add_zone();
    for ch in "UTC+5:30".chars() {
        state.push_modal_char(ch);
    }

    state.submit_modal().unwrap();

    assert!(
        state
            .model
            .zones
            .iter()
            .any(|zone| zone.input_name == "UTC+5:30"),
        "UTC+5:30 should be added via raw text fallback"
    );
}

#[test]
fn timeline_shows_all_hours_without_dot_placeholders() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());
    let area = Rect::new(0, 0, 120, 36);
    let mut buffer = Buffer::empty(area);

    render_to_buffer(&mut buffer, area, &state);
    let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();

    assert!(
        !text.contains(".."),
        "buffer should not contain '..' placeholders, but found them"
    );
}

#[test]
fn picker_renders_without_panic_at_various_sizes() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    state.open_add_zone();

    let (min_w, min_h) = zonetimeline_tui::tui::view::min_terminal_size(&state);
    for (w, h) in [(120, 36), (min_w, min_h), (min_w, min_h + 5)] {
        let area = Rect::new(0, 0, w, h);
        let mut buffer = Buffer::empty(area);
        render_to_buffer(&mut buffer, area, &state);
        let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        assert!(text.contains("Add Zone"), "should render picker at {w}x{h}");
    }
}

#[test]
fn timeline_cells_use_green_foreground_for_in_window_slots() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());
    let area = Rect::new(0, 0, 120, 36);
    let mut buffer = Buffer::empty(area);

    render_to_buffer(&mut buffer, area, &state);

    let has_green = (0..area.width).any(|x| {
        (0..area.height).any(|y| {
            let cell = buffer.cell((x, y)).unwrap();
            cell.fg == Color::Green && !cell.symbol().trim().is_empty()
        })
    });
    assert!(has_green, "should have green cells for in-window slots");
}

#[test]
fn timeline_cells_use_red_foreground_for_outside_window_slots() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());
    let area = Rect::new(0, 0, 120, 36);
    let mut buffer = Buffer::empty(area);

    render_to_buffer(&mut buffer, area, &state);

    let has_red = (0..area.width).any(|x| {
        (0..area.height).any(|y| {
            let cell = buffer.cell((x, y)).unwrap();
            cell.fg == Color::Red && !cell.symbol().trim().is_empty()
        })
    });
    assert!(has_red, "should have red cells for outside-window slots");
}

#[test]
fn timeline_overlap_columns_have_underline_modifier() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());
    let area = Rect::new(0, 0, 120, 36);
    let mut buffer = Buffer::empty(area);

    render_to_buffer(&mut buffer, area, &state);

    let has_underline = (0..area.width).any(|x| {
        (0..area.height).any(|y| {
            let cell = buffer.cell((x, y)).unwrap();
            cell.modifier.contains(Modifier::UNDERLINED) && !cell.symbol().trim().is_empty()
        })
    });
    assert!(
        has_underline,
        "should have underlined cells in overlap columns"
    );
}

#[test]
fn timeline_has_no_now_caret_row() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());
    let area = Rect::new(0, 0, 120, 36);
    let mut buffer = Buffer::empty(area);

    render_to_buffer(&mut buffer, area, &state);
    let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();

    assert!(
        !text.contains("Now"),
        "Now row should be removed from timeline"
    );
}

#[test]
fn timeline_draws_box_frame_around_selected_column() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());
    let area = Rect::new(0, 0, 120, 36);
    let mut buffer = Buffer::empty(area);

    render_to_buffer(&mut buffer, area, &state);
    let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();

    assert!(text.contains('┌'), "should have top-left corner");
    assert!(text.contains('┐'), "should have top-right corner");
    assert!(text.contains('└'), "should have bottom-left corner");
    assert!(text.contains('┘'), "should have bottom-right corner");
    assert!(text.contains('│'), "should have vertical edges");
    assert!(text.contains('─'), "should have horizontal edges");
}

#[test]
fn timeline_frame_at_first_slot_does_not_panic() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    // Move cursor to first slot
    for _ in 0..50 {
        state.focus_left();
    }
    let area = Rect::new(0, 0, 120, 36);
    let mut buffer = Buffer::empty(area);
    render_to_buffer(&mut buffer, area, &state);
    // Just verify no panic occurred
}

#[test]
fn timeline_frame_at_last_slot_does_not_panic() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    // Move cursor to last slot
    for _ in 0..50 {
        state.focus_right();
    }
    let area = Rect::new(0, 0, 120, 36);
    let mut buffer = Buffer::empty(area);
    render_to_buffer(&mut buffer, area, &state);
    // Just verify no panic occurred
}

#[test]
fn display_order_sorts_by_utc_offset_ascending_by_default() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());

    assert_eq!(state.sort_mode, SortMode::UtcOffsetAsc);
    // America/New_York (UTC-4) should come before Europe/London (UTC+1)
    let first_zone = &state.model.zones[state.display_order[0]];
    let second_zone = &state.model.zones[state.display_order[1]];
    assert_eq!(first_zone.input_name, "America/New_York");
    assert_eq!(second_zone.input_name, "Europe/London");
}

#[test]
fn display_order_sorts_by_utc_offset_descending() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    state.cycle_sort_mode();

    assert_eq!(state.sort_mode, SortMode::UtcOffsetDesc);
    let first_zone = &state.model.zones[state.display_order[0]];
    let second_zone = &state.model.zones[state.display_order[1]];
    assert_eq!(first_zone.input_name, "Europe/London");
    assert_eq!(second_zone.input_name, "America/New_York");
}

#[test]
fn display_order_sorts_by_label_ascending() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    state.cycle_sort_mode();
    state.cycle_sort_mode();

    assert_eq!(state.sort_mode, SortMode::LabelAsc);
    let first_zone = &state.model.zones[state.display_order[0]];
    let second_zone = &state.model.zones[state.display_order[1]];
    assert_eq!(first_zone.input_name, "America/New_York");
    assert_eq!(second_zone.input_name, "Europe/London");
}

#[test]
fn display_order_sorts_by_label_descending() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    state.cycle_sort_mode();
    state.cycle_sort_mode();
    state.cycle_sort_mode();

    assert_eq!(state.sort_mode, SortMode::LabelDesc);
    let first_zone = &state.model.zones[state.display_order[0]];
    assert_eq!(first_zone.input_name, "Europe/London");
}

#[test]
fn display_order_uses_ordered_zones_in_manual_mode() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    for _ in 0..4 {
        state.cycle_sort_mode();
    }

    assert_eq!(state.sort_mode, SortMode::Manual);
    let first_zone = &state.model.zones[state.display_order[0]];
    assert_eq!(first_zone.input_name, "Europe/London");
}

#[test]
fn cursor_follows_zone_when_sort_mode_changes() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    state.selected_zone = 1; // first user zone in unified space
    let selected_key = state.model.zones[state.display_order[0]]
        .handle
        .identity_key();

    state.cycle_sort_mode();

    let new_display_idx = state.selected_zone.saturating_sub(1);
    let new_key = state.model.zones[state.display_order[new_display_idx]]
        .handle
        .identity_key();
    assert_eq!(selected_key, new_key);
}

#[test]
fn j_k_reorder_only_works_in_manual_mode() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    let order_before = state.display_order.clone();
    state.selected_zone = 1;
    state.move_zone_down(state.selected_zone);
    assert_eq!(state.display_order, order_before);

    for _ in 0..4 {
        state.cycle_sort_mode();
    }
    assert_eq!(state.sort_mode, SortMode::Manual);

    state.selected_zone = 1;
    let first_zone_before = state.model.zones[state.display_order[0]].input_name.clone();
    state.move_zone_down(state.selected_zone);
    let first_zone_after = state.model.zones[state.display_order[0]].input_name.clone();
    assert_ne!(first_zone_before, first_zone_after);
}

#[test]
fn utc_row_cannot_be_deleted() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    let zone_count_before = state.model.zones.len();

    state.selected_zone = 0;
    let result = state.remove_zone(state.selected_zone);

    assert_eq!(state.model.zones.len(), zone_count_before);
    assert!(result.is_ok());
}

#[test]
fn adding_utc_zone_is_rejected() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    let result = state.add_zone("UTC".to_string());
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("UTC"));
}

#[test]
fn adding_gmt_zone_is_rejected() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    let result = state.add_zone("GMT".to_string());
    assert!(result.is_err());
}

#[test]
fn timeline_panel_title_is_zone_timeline() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());
    let area = Rect::new(0, 0, 120, 36);
    let mut buffer = Buffer::empty(area);

    render_to_buffer(&mut buffer, area, &state);
    let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();

    assert!(
        text.contains("Zone Timeline"),
        "panel should be titled 'Zone Timeline', got: look for it in rendered text"
    );
}

#[test]
fn timeline_column_header_shows_utc_reference() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());
    let area = Rect::new(0, 0, 120, 36);
    let mut buffer = Buffer::empty(area);

    render_to_buffer(&mut buffer, area, &state);
    let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();

    assert!(
        text.contains("Coordinated Universal Time (UTC)") || text.contains("UTC"),
        "column header should reference UTC"
    );
}

#[test]
fn sort_indicator_shows_current_sort_mode() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());
    let area = Rect::new(0, 0, 120, 36);
    let mut buffer = Buffer::empty(area);

    render_to_buffer(&mut buffer, area, &state);
    let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();

    assert!(
        text.contains("Sort: UTC+"),
        "should show sort indicator for default mode"
    );
}

#[test]
fn fixed_utc_row_is_rendered_with_dim_style() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());
    let area = Rect::new(0, 0, 120, 36);
    let mut buffer = Buffer::empty(area);

    render_to_buffer(&mut buffer, area, &state);

    // Find a cell in the UTC row that has DIM modifier
    let has_dim = (0..area.width).any(|x| {
        (0..area.height).any(|y| {
            let cell = buffer.cell((x, y)).unwrap();
            cell.modifier.contains(Modifier::DIM) && cell.symbol() == "U"
        })
    });
    assert!(has_dim, "UTC row should have DIM modifier");
}

#[test]
fn focus_up_down_navigates_unified_index_space() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    assert_eq!(state.selected_zone, 0);
    state.focus_down();
    assert_eq!(state.selected_zone, 1);
    state.focus_down();
    assert_eq!(state.selected_zone, 2);
    state.focus_down();
    assert_eq!(state.selected_zone, 2);
    state.focus_up();
    assert_eq!(state.selected_zone, 1);
    state.focus_up();
    assert_eq!(state.selected_zone, 0);
    state.focus_up();
    assert_eq!(state.selected_zone, 0);
}

#[test]
fn zone_picker_excludes_utc_and_gmt_entries() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    state.open_add_zone();

    match &state.modal {
        Some(zonetimeline_tui::tui::forms::Modal::AddZone { entries, .. }) => {
            let has_utc = entries.iter().any(|e| e.name.eq_ignore_ascii_case("utc"));
            let has_gmt = entries.iter().any(|e| e.name.eq_ignore_ascii_case("gmt"));
            let has_etc_utc = entries.iter().any(|e| e.name == "Etc/UTC");
            let has_etc_gmt = entries.iter().any(|e| e.name == "Etc/GMT");
            let has_etc_greenwich = entries.iter().any(|e| e.name == "Etc/Greenwich");
            assert!(!has_utc, "picker should not contain UTC");
            assert!(!has_gmt, "picker should not contain GMT");
            assert!(!has_etc_utc, "picker should not contain Etc/UTC");
            assert!(!has_etc_gmt, "picker should not contain Etc/GMT");
            assert!(
                !has_etc_greenwich,
                "picker should not contain Etc/Greenwich"
            );
        }
        other => panic!("expected AddZone modal, got {:?}", other),
    }
}

#[test]
fn working_windows_panel_shows_tier_labels() {
    let mut seed = support::fixture_seed();
    seed.nhours = 24;
    seed.anchor_time = Some(chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap());
    seed.shoulder_hours = 1;

    let model = ComparisonModel::build(seed, support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());
    let area = Rect::new(0, 0, 120, 36);
    let mut buffer = Buffer::empty(area);

    render_to_buffer(&mut buffer, area, &state);
    let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();

    assert!(
        text.contains("Working Windows"),
        "panel title should be Working Windows"
    );
    // With default 09:00-17:00 windows, 1hr shoulder, London+NYC, there should be overlap
    assert!(
        text.contains("Ideal") || text.contains("Feasible"),
        "should show at least one tier label in the panel"
    );
}

#[test]
fn working_windows_shows_times_for_selected_zone() {
    let mut seed = support::fixture_seed();
    seed.nhours = 24;
    seed.anchor_time = Some(chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap());

    let model = ComparisonModel::build(seed, support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    // Select first user zone (not UTC)
    state.focus_down();
    assert!(state.selected_zone > 0);

    let area = Rect::new(0, 0, 120, 36);
    let mut buffer = Buffer::empty(area);
    render_to_buffer(&mut buffer, area, &state);
    let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();

    assert!(
        text.contains("Times shown for"),
        "should show timezone reference line"
    );
}

#[test]
fn compute_controls_height_fits_one_line_at_120_width() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());
    let height = zonetimeline_tui::tui::view::compute_controls_height(&state, 120);
    assert_eq!(height, 1);
}

#[test]
fn compute_controls_height_wraps_to_two_at_narrow_width() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());
    // The controls text is roughly 90-100 chars depending on anchor date.
    // At width 80, the inner width (80) should still fit, but test with a smaller hypothetical.
    // We need to find a width where the controls definitely wrap.
    // The controls bar has ~95 chars of content. At width 80, it exceeds 80 -> wraps.
    let height = zonetimeline_tui::tui::view::compute_controls_height(&state, 80);
    // Controls bar has no border, so inner_width == terminal_width.
    // At 80 cols with ~95 chars of content, it wraps to 2 lines.
    assert_eq!(height, 2);
}

#[test]
fn compute_header_height_single_zone_fits_one_line() {
    // With 2 zones at width 120, the summary spans fit in one line.
    // Expected: 1 content line + 2 borders = 3
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());
    // fixture_seed has 2 zones (Europe/London, America/New_York)
    // At width 120, "London 13:30 (UTC+1)  |  New York 08:30 (UTC-4)" easily fits in one line
    let height = zonetimeline_tui::tui::view::compute_header_height(&state, 120);
    assert_eq!(height, 3, "1 content line + 2 borders");
}

#[test]
fn dynamic_layout_header_is_three_rows_with_two_zones() {
    // With 2 zones at width 120, header should be 3 rows (1 content + 2 border)
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());
    let area = Rect::new(0, 0, 120, 36);
    let mut buffer = Buffer::empty(area);
    render_to_buffer(&mut buffer, area, &state);

    // Row 0: top border of header
    let row0 = (0..area.width)
        .map(|x| buffer.cell((x, 0)).unwrap().symbol())
        .collect::<String>();
    assert!(
        row0.contains("Current Times"),
        "Row 0 should be header border with title"
    );

    // Row 1: content line (zone summaries)
    let row1 = (0..area.width)
        .map(|x| buffer.cell((x, 1)).unwrap().symbol())
        .collect::<String>();
    assert!(
        row1.contains("London") || row1.contains("13:30"),
        "Row 1 should contain zone summary"
    );

    // Row 2: bottom border of header
    let row2 = (0..area.width)
        .map(|x| buffer.cell((x, 2)).unwrap().symbol())
        .collect::<String>();
    // Bottom border contains horizontal line characters
    assert!(row2.contains("─"), "Row 2 should be bottom border");

    // Row 3: should be timeline section (top border of timeline)
    let row3 = (0..area.width)
        .map(|x| buffer.cell((x, 3)).unwrap().symbol())
        .collect::<String>();
    assert!(
        row3.contains("Zone Timeline"),
        "Row 3 should be timeline border"
    );
}

#[test]
fn header_suppresses_empty_status_line() {
    // When state.status is None, the header should NOT render an empty second line
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());
    assert!(
        state.status.is_none(),
        "fixture state should have no status"
    );
    let area = Rect::new(0, 0, 120, 36);
    let mut buffer = Buffer::empty(area);
    render_to_buffer(&mut buffer, area, &state);

    // Header is 3 rows (1 content + 2 borders). Row 1 has zone info, row 2 is bottom border.
    // If status were rendered as empty line, it would push content or waste space.
    // Verify row 2 is the bottom border (contains ─), not a blank content line.
    let row2 = (0..area.width)
        .map(|x| buffer.cell((x, 2)).unwrap().symbol())
        .collect::<String>();
    assert!(
        row2.contains("─"),
        "Row 2 should be bottom border when no status"
    );
}

#[test]
fn compute_header_height_clamps_to_max_five() {
    // Even with many zones that would wrap to 10+ lines, height is clamped at 3+2=5
    use zonetimeline_tui::config::SessionSeed;
    let seed = SessionSeed {
        base_zones: vec!["UTC".to_string()],
        extra_zones: (0..20).map(|i| format!("Etc/GMT+{}", i % 12)).collect(),
        ordered_zones: std::iter::once("UTC".to_string())
            .chain((0..20).map(|i| format!("Etc/GMT+{}", i % 12)))
            .collect(),
        nhours: 12,
        anchor_time: None,
        width: Some(80),
        plain: true,
        save_path: std::env::temp_dir().join("ztl-header-height-test.toml"),
        default_window: "09:00-17:00".to_string(),
        work_hours: Default::default(),
        shoulder_hours: 1,
        sort_mode: Default::default(),
    };
    let model = ComparisonModel::build(seed, support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());
    let height = zonetimeline_tui::tui::view::compute_header_height(&state, 80);
    assert_eq!(height, 5, "clamped to 3 content lines + 2 borders");
}

#[test]
fn controls_bar_wraps_to_two_lines_at_narrow_width() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let state = AppState::new(model, support::fixed_now());

    // Use dynamic minimum width (80 cols) but ensure enough height to pass resize guard
    let (_, min_h) = zonetimeline_tui::tui::view::min_terminal_size(&state);
    let area = Rect::new(0, 0, 80, min_h);
    let mut buffer = Buffer::empty(area);
    render_to_buffer(&mut buffer, area, &state);

    // At width 80, controls should wrap to 2 lines.
    // The last 2 rows of the buffer should contain control text.
    let last_row = area.height - 1;
    let second_last_row = area.height - 2;

    let row_last = (0..area.width)
        .map(|x| buffer.cell((x, last_row)).unwrap().symbol())
        .collect::<String>();
    let row_second_last = (0..area.width)
        .map(|x| buffer.cell((x, second_last_row)).unwrap().symbol())
        .collect::<String>();

    // Both rows should contain some control text
    assert!(
        row_second_last.contains("Anchor") || row_second_last.contains("scroll"),
        "Second-to-last row should have controls: {row_second_last:?}"
    );
    assert!(
        row_last.contains("help") || row_last.contains("quit") || row_last.contains("save"),
        "Last row should have wrapped controls: {row_last:?}"
    );
}

#[test]
fn zones_panel_scrolls_to_follow_selected_zone() {
    // Create a state with many zones, select the last one, verify it's visible
    use zonetimeline_tui::config::SessionSeed;
    let zones: Vec<String> = vec![
        "Europe/London",
        "America/New_York",
        "Asia/Tokyo",
        "Australia/Sydney",
        "Europe/Berlin",
        "America/Los_Angeles",
        "Asia/Shanghai",
        "Europe/Paris",
        "America/Chicago",
        "Asia/Kolkata",
        "Africa/Cairo",
        "Pacific/Auckland",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    let seed = SessionSeed {
        base_zones: zones.clone(),
        extra_zones: Vec::new(),
        ordered_zones: zones.clone(),
        nhours: 12,
        anchor_time: None,
        width: Some(120),
        plain: true,
        save_path: std::env::temp_dir().join("ztl-scroll-test.toml"),
        default_window: "09:00-17:00".to_string(),
        work_hours: Default::default(),
        shoulder_hours: 1,
        sort_mode: SortMode::Manual, // preserve insertion order so zone positions are predictable
    };
    let model = ComparisonModel::build(seed, support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    // Select the last zone (index = 12: UTC + 12 user zones, select last user zone)
    for _ in 0..12 {
        state.focus_down();
    }
    assert_eq!(state.selected_zone, 12);

    let area = Rect::new(0, 0, 120, 36);
    let mut buffer = Buffer::empty(area);
    render_to_buffer(&mut buffer, area, &state);

    // Footer is at the bottom. The zones panel is the middle third of the footer.
    // The selected zone (Pacific/Auckland) should be visible somewhere in the zones panel.
    // The footer is a 10-row section. With 13 zones and 8 inner rows, Auckland (zone 13)
    // is off-screen without scroll-follow.
    //
    // We identify the zones panel by finding the row with the "Zones" panel title,
    // then checking rows below it in the middle third of the terminal width.
    let panel_width = area.width / 3;
    let zones_col_start = panel_width; // middle third starts here approximately
    let zones_col_end = zones_col_start + panel_width;

    // Find the footer start row (the row with "Zones" title)
    let mut footer_start: u16 = 0;
    for y in 0..area.height {
        let row: String = (zones_col_start..zones_col_end)
            .map(|x| buffer.cell((x, y)).unwrap().symbol())
            .collect();
        if row.contains("Zones") {
            footer_start = y;
            break;
        }
    }
    assert!(footer_start > 0, "Should find Zones panel title row");

    // Now scan only the zones panel column area in the footer rows for "Auckland"
    let mut found_auckland_in_zones_panel = false;
    for y in (footer_start + 1)..area.height.min(footer_start + 10) {
        let row: String = (zones_col_start..zones_col_end)
            .map(|x| buffer.cell((x, y)).unwrap().symbol())
            .collect();
        if row.contains("Auckland") {
            found_auckland_in_zones_panel = true;
            break;
        }
    }
    assert!(
        found_auckland_in_zones_panel,
        "Selected zone 'Auckland' should be visible in the Zones panel after scroll-follow"
    );
}

#[test]
fn time_slots_has_48_entries_covering_full_day_in_30_min_intervals() {
    use zonetimeline_tui::tui::forms::TIME_SLOTS;
    assert_eq!(TIME_SLOTS.len(), 48);
    assert_eq!(TIME_SLOTS[0], (0, 0));
    assert_eq!(TIME_SLOTS[1], (0, 30));
    assert_eq!(TIME_SLOTS[47], (23, 30));
    // Verify all entries are valid HH:MM
    for &(h, m) in TIME_SLOTS.iter() {
        assert!(h < 24, "hour {h} out of range");
        assert!(m == 0 || m == 30, "minute {m} not 0 or 30");
    }
}

#[test]
fn time_slot_index_for_time_finds_exact_matches() {
    use zonetimeline_tui::tui::forms::time_slot_index_for_time;
    assert_eq!(time_slot_index_for_time(0, 0), 0);
    assert_eq!(time_slot_index_for_time(9, 0), 18); // 9*2
    assert_eq!(time_slot_index_for_time(17, 0), 34); // 17*2
    assert_eq!(time_slot_index_for_time(23, 30), 47);
}

#[test]
fn time_slot_index_for_time_snaps_non_30min_boundaries() {
    use zonetimeline_tui::tui::forms::time_slot_index_for_time;
    // 9:15 -> snaps to 9:00 (index 18)
    assert_eq!(time_slot_index_for_time(9, 15), 18);
    // 9:45 -> snaps to 9:30 (index 19)
    assert_eq!(time_slot_index_for_time(9, 45), 19);
    // 17:29 -> snaps to 17:00 (index 34)
    assert_eq!(time_slot_index_for_time(17, 29), 34);
}

#[test]
fn edit_window_modal_stores_pane_and_selection_state() {
    use zonetimeline_tui::tui::forms::{Modal, Pane};
    let modal = Modal::EditWindow {
        zone_index: 0,
        active_pane: Pane::Start,
        start_selected: 18,
        start_scroll_offset: 0,
        end_selected: 34,
        end_scroll_offset: 0,
    };
    match &modal {
        Modal::EditWindow {
            active_pane,
            start_selected,
            end_selected,
            ..
        } => {
            assert_eq!(*active_pane, Pane::Start);
            assert_eq!(*start_selected, 18);
            assert_eq!(*end_selected, 34);
        }
        _ => panic!("expected EditWindow"),
    }
}

#[test]
fn open_edit_window_parses_existing_window_into_slot_indices() {
    use zonetimeline_tui::tui::forms::{Modal, Pane};
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    state.selected_zone = 1; // first user zone

    state.open_edit_window();

    match &state.modal {
        Some(Modal::EditWindow {
            active_pane,
            start_selected,
            end_selected,
            ..
        }) => {
            assert_eq!(*active_pane, Pane::Start);
            // Default window is 09:00-17:00 -> indices 18 and 34
            assert_eq!(*start_selected, 18);
            assert_eq!(*end_selected, 34);
        }
        other => panic!("expected EditWindow, got {:?}", other),
    }
}

#[test]
fn open_edit_window_with_custom_work_hours_uses_those_values() {
    use zonetimeline_tui::tui::forms::Modal;
    let mut seed = support::fixture_seed();
    seed.work_hours
        .insert("Europe/London".to_string(), "08:30-16:30".to_string());
    let model = ComparisonModel::build(seed, support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());

    // Find Europe/London in display order
    let london_unified = state
        .display_order
        .iter()
        .enumerate()
        .find(|&(_, &mi)| state.model.zones[mi].input_name == "Europe/London")
        .map(|(di, _)| di + 1)
        .unwrap();
    state.selected_zone = london_unified;

    state.open_edit_window();

    match &state.modal {
        Some(Modal::EditWindow {
            start_selected,
            end_selected,
            ..
        }) => {
            // 08:30 -> index 17, 16:30 -> index 33
            assert_eq!(*start_selected, 17);
            assert_eq!(*end_selected, 33);
        }
        other => panic!("expected EditWindow, got {:?}", other),
    }
}

#[test]
fn submit_edit_window_formats_selected_slots_and_updates_model() {
    use zonetimeline_tui::tui::forms::Modal;
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    state.selected_zone = 1;

    state.open_edit_window();

    // Change start to 08:00 (index 16) and end to 18:00 (index 36)
    if let Some(Modal::EditWindow {
        start_selected,
        end_selected,
        ..
    }) = &mut state.modal
    {
        *start_selected = 16;
        *end_selected = 36;
    }

    state.submit_modal().unwrap();
    assert!(state.modal.is_none());

    // Verify the model was updated
    let display_idx = state.selected_zone - 1;
    let model_idx = state.display_order[display_idx];
    let zone = &state.model.zones[model_idx];
    assert_eq!(zone.window.start_minute, 8 * 60);
    assert_eq!(zone.window.end_minute, 18 * 60);
}

#[test]
fn edit_window_down_increments_selected_in_active_pane() {
    use zonetimeline_tui::tui::forms::Modal;
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    state.selected_zone = 1;
    state.open_edit_window();

    let initial_start = match &state.modal {
        Some(Modal::EditWindow { start_selected, .. }) => *start_selected,
        _ => panic!("expected EditWindow"),
    };

    state.edit_window_down();

    match &state.modal {
        Some(Modal::EditWindow { start_selected, .. }) => {
            assert_eq!(*start_selected, initial_start + 1);
        }
        _ => panic!("expected EditWindow"),
    }
}

#[test]
fn edit_window_up_decrements_selected_in_active_pane() {
    use zonetimeline_tui::tui::forms::Modal;
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    state.selected_zone = 1;
    state.open_edit_window();

    let initial_start = match &state.modal {
        Some(Modal::EditWindow { start_selected, .. }) => *start_selected,
        _ => panic!("expected EditWindow"),
    };

    state.edit_window_up();

    match &state.modal {
        Some(Modal::EditWindow { start_selected, .. }) => {
            assert_eq!(*start_selected, initial_start - 1);
        }
        _ => panic!("expected EditWindow"),
    }
}

#[test]
fn edit_window_down_wraps_from_47_to_0() {
    use zonetimeline_tui::tui::forms::Modal;
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    state.selected_zone = 1;
    state.open_edit_window();

    // Set start_selected to 47
    if let Some(Modal::EditWindow { start_selected, .. }) = &mut state.modal {
        *start_selected = 47;
    }

    state.edit_window_down();

    match &state.modal {
        Some(Modal::EditWindow { start_selected, .. }) => {
            assert_eq!(*start_selected, 0);
        }
        _ => panic!("expected EditWindow"),
    }
}

#[test]
fn edit_window_up_wraps_from_0_to_47() {
    use zonetimeline_tui::tui::forms::Modal;
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    state.selected_zone = 1;
    state.open_edit_window();

    // Set start_selected to 0
    if let Some(Modal::EditWindow { start_selected, .. }) = &mut state.modal {
        *start_selected = 0;
    }

    state.edit_window_up();

    match &state.modal {
        Some(Modal::EditWindow { start_selected, .. }) => {
            assert_eq!(*start_selected, 47);
        }
        _ => panic!("expected EditWindow"),
    }
}

#[test]
fn edit_window_switch_pane_toggles_between_start_and_end() {
    use zonetimeline_tui::tui::forms::{Modal, Pane};
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    state.selected_zone = 1;
    state.open_edit_window();

    // Starts on Start pane
    match &state.modal {
        Some(Modal::EditWindow { active_pane, .. }) => assert_eq!(*active_pane, Pane::Start),
        _ => panic!("expected EditWindow"),
    }

    state.edit_window_switch_pane();

    match &state.modal {
        Some(Modal::EditWindow { active_pane, .. }) => assert_eq!(*active_pane, Pane::End),
        _ => panic!("expected EditWindow"),
    }

    state.edit_window_switch_pane();

    match &state.modal {
        Some(Modal::EditWindow { active_pane, .. }) => assert_eq!(*active_pane, Pane::Start),
        _ => panic!("expected EditWindow"),
    }
}

#[test]
fn edit_window_navigation_affects_correct_pane_after_switch() {
    use zonetimeline_tui::tui::forms::Modal;
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    state.selected_zone = 1;
    state.open_edit_window();

    let initial_end = match &state.modal {
        Some(Modal::EditWindow { end_selected, .. }) => *end_selected,
        _ => panic!("expected EditWindow"),
    };

    // Switch to End pane and navigate
    state.edit_window_switch_pane();
    state.edit_window_down();

    match &state.modal {
        Some(Modal::EditWindow {
            end_selected,
            start_selected,
            ..
        }) => {
            assert_eq!(*end_selected, initial_end + 1, "end should have moved");
            // start should be unchanged (still 18 for 09:00)
            assert_eq!(*start_selected, 18, "start should not have moved");
        }
        _ => panic!("expected EditWindow"),
    }
}

#[test]
fn edit_window_modal_renders_twin_pane_layout() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    state.selected_zone = 1;
    state.open_edit_window();

    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);
    render_to_buffer(&mut buffer, area, &state);
    let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();

    assert!(
        text.contains("Edit Working Window"),
        "should show dialog title"
    );
    assert!(text.contains("Start"), "should show Start pane label");
    assert!(text.contains("End"), "should show End pane label");
    assert!(text.contains("09:00"), "should show default start time");
    assert!(text.contains("17:00"), "should show default end time");
    assert!(text.contains("Tab"), "should show Tab hint");
    assert!(text.contains("Enter"), "should show Enter hint");
    assert!(text.contains("Esc"), "should show Esc hint");
}

#[test]
fn edit_window_modal_shows_duration_summary() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    state.selected_zone = 1;
    state.open_edit_window();

    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);
    render_to_buffer(&mut buffer, area, &state);
    let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();

    // Default 09:00-17:00 = 8h 0m
    assert!(
        text.contains("8h 0m"),
        "should show duration for 09:00-17:00"
    );
}

#[test]
fn edit_window_modal_shows_overnight_indicator() {
    use zonetimeline_tui::tui::forms::Modal;
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    state.selected_zone = 1;
    state.open_edit_window();

    // Set start=22:00 (index 44), end=06:00 (index 12)
    if let Some(Modal::EditWindow {
        start_selected,
        end_selected,
        ..
    }) = &mut state.modal
    {
        *start_selected = 44;
        *end_selected = 12;
    }

    let area = Rect::new(0, 0, 120, 40);
    let mut buffer = Buffer::empty(area);
    render_to_buffer(&mut buffer, area, &state);
    let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();

    assert!(
        text.contains("overnight"),
        "should show overnight indicator"
    );
    assert!(
        text.contains("8h 0m"),
        "should show correct overnight duration"
    );
}

#[test]
fn edit_window_full_workflow_open_navigate_switch_submit() {
    use zonetimeline_tui::tui::forms::{Modal, Pane};
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    state.selected_zone = 1;

    // Open the editor
    state.open_edit_window();
    assert!(matches!(state.modal, Some(Modal::EditWindow { .. })));

    // Navigate start down twice (09:00 -> 09:30 -> 10:00)
    state.edit_window_down();
    state.edit_window_down();
    match &state.modal {
        Some(Modal::EditWindow { start_selected, .. }) => assert_eq!(*start_selected, 20), // 10:00
        _ => panic!("expected EditWindow"),
    }

    // Switch to end pane and navigate up once (17:00 -> 16:30)
    state.edit_window_switch_pane();
    state.edit_window_up();
    match &state.modal {
        Some(Modal::EditWindow {
            active_pane,
            end_selected,
            ..
        }) => {
            assert_eq!(*active_pane, Pane::End);
            assert_eq!(*end_selected, 33); // 16:30
        }
        _ => panic!("expected EditWindow"),
    }

    // Submit
    state.submit_modal().unwrap();
    assert!(state.modal.is_none());

    // Verify result: 10:00-16:30
    let display_idx = state.selected_zone - 1;
    let model_idx = state.display_order[display_idx];
    let zone = &state.model.zones[model_idx];
    assert_eq!(zone.window.start_minute, 10 * 60);
    assert_eq!(zone.window.end_minute, 16 * 60 + 30);
}

#[test]
fn edit_window_cancel_does_not_change_model() {
    let model = ComparisonModel::build(support::fixture_seed(), support::fixed_now()).unwrap();
    let mut state = AppState::new(model, support::fixed_now());
    state.selected_zone = 1;
    let display_idx = state.selected_zone - 1;
    let model_idx = state.display_order[display_idx];
    let original_window = state.model.zones[model_idx].window;

    state.open_edit_window();
    state.edit_window_down();
    state.edit_window_down();
    state.edit_window_down();
    state.cancel_modal();

    assert!(state.modal.is_none());
    assert_eq!(state.model.zones[model_idx].window, original_window);
}
