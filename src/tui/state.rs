//! TUI application state machine.
//!
//! [`AppState`] is the single source of truth for the interactive session.
//! It owns the current [`ComparisonModel`], focus cursors, sort order, and any
//! open modal dialog. Every user action maps to a method on `AppState` that
//! mutates it in place; the view layer then reads the state to render.
//!
//! ## Unified index space
//!
//! Zone rows use a *unified index* where **0 = the fixed UTC row** and
//! **1..=N = user zones** (mapped through [`display_order`](AppState::display_order)).
//! This keeps the UTC reference row always at index 0 regardless of sort mode,
//! simplifying focus tracking and boundary checks.

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};

use crate::config::save_session;
use crate::core::model::{
    AnchorSpec, ComparisonModel, SessionConfig, SortMode, compute_display_order,
};
use crate::core::timezones::parse_zone;
use crate::tui::forms::{Modal, Pane, TIME_SLOTS, build_picker_entries, refilter};

/// Central application state for the TUI session.
///
/// Holds the comparison model, cursor positions (hour and zone), the current
/// sort mode, and any active modal dialog. Methods on this struct are the sole
/// way the event loop mutates UI state.
#[derive(Debug, Clone)]
pub struct AppState {
    pub session: SessionConfig,
    pub model: ComparisonModel,
    pub now_utc: DateTime<Utc>,
    pub focused_hour: usize,
    pub selected_zone: usize,
    pub show_help: bool,
    pub status: Option<String>,
    pub modal: Option<Modal>,
    /// Runtime source of truth for sort mode. May differ from `session.sort_mode`
    /// until `save()` is called, which patches the session before writing.
    pub sort_mode: SortMode,
    pub display_order: Vec<usize>,
}

impl AppState {
    /// Create a new `AppState` from a freshly-built model.
    ///
    /// Initializes `focused_hour` to the column whose offset is 0 (the anchor
    /// slot), and sets the zone cursor to the first row (UTC).
    pub fn new(model: ComparisonModel, now_utc: DateTime<Utc>) -> Self {
        let focused_hour = model
            .timeline_slots
            .iter()
            .position(|slot| slot.offset_hours == 0)
            .unwrap_or(0);

        let sort_mode = model.session().sort_mode;
        let display_order = compute_display_order(&model.zones, sort_mode, now_utc);

        Self {
            session: model.session().clone(),
            model,
            now_utc,
            focused_hour,
            selected_zone: 0,
            show_help: false,
            status: None,
            modal: None,
            sort_mode,
            display_order,
        }
    }

    /// Update the wall-clock time, rebuilding the model when necessary.
    ///
    /// A rebuild is triggered every minute when the anchor is `Now`, or once
    /// per calendar-day change when an explicit anchor is set.
    pub fn refresh_now(&mut self, now_utc: DateTime<Utc>) -> Result<()> {
        if self.now_utc.timestamp() / 60 == now_utc.timestamp() / 60 {
            return Ok(());
        }

        let should_rebuild = match self.session.anchor {
            AnchorSpec::Now => true,
            AnchorSpec::Explicit(_) => self.now_utc.date_naive() != now_utc.date_naive(),
        };
        self.now_utc = now_utc;

        if should_rebuild {
            self.rebuild()?;
        }

        Ok(())
    }

    /// Move the hour cursor one slot to the left (clamped at 0).
    pub fn focus_left(&mut self) {
        self.focused_hour = self.focused_hour.saturating_sub(1);
    }

    /// Move the hour cursor one slot to the right (clamped at the last slot).
    pub fn focus_right(&mut self) {
        let last = self.model.timeline_slots.len().saturating_sub(1);
        self.focused_hour = self.focused_hour.min(last).saturating_add(1).min(last);
    }

    /// Unified index space: 0 = UTC row, 1..=N = user zones via display_order
    pub fn focus_up(&mut self) {
        self.selected_zone = self.selected_zone.saturating_sub(1);
    }

    /// Unified index space: 0 = UTC row, 1..=N = user zones via display_order
    pub fn focus_down(&mut self) {
        // max unified index = display_order.len() (0=UTC + N user zones)
        let last = self.display_order.len();
        self.selected_zone = self.selected_zone.min(last).saturating_add(1).min(last);
    }

    /// Advance to the next sort mode (Manual -> ByOffset -> ByName -> Manual …).
    ///
    /// Preserves the identity of the currently-selected zone so the cursor
    /// follows it to its new position in the reordered list.
    pub fn cycle_sort_mode(&mut self) {
        let prev_key = self.selected_zone_identity_key();
        self.sort_mode = self.sort_mode.next();
        self.recompute_display_order();
        self.restore_cursor(prev_key);
        self.status = None;
    }

    /// Open the "Add Zone" picker modal, pre-populated with all IANA timezones.
    pub fn open_add_zone(&mut self) {
        self.status = None;
        self.show_help = false;
        let entries = build_picker_entries(self.now_utc);
        let filtered = refilter(&entries, "");
        self.modal = Some(Modal::AddZone {
            input: String::new(),
            entries,
            filtered,
            selected: 0,
            scroll_offset: 0,
        });
    }

    /// Move the selection cursor up in the zone picker list.
    pub fn picker_up(&mut self) {
        if let Some(Modal::AddZone { selected, .. }) = &mut self.modal {
            *selected = selected.saturating_sub(1);
        }
    }

    /// Move the selection cursor down in the zone picker list.
    pub fn picker_down(&mut self) {
        if let Some(Modal::AddZone {
            filtered, selected, ..
        }) = &mut self.modal
        {
            let last = filtered.len().saturating_sub(1);
            *selected = (*selected + 1).min(last);
        }
    }

    /// Move the active pane's selection up in the Edit Window modal (wraps around).
    pub fn edit_window_up(&mut self) {
        if let Some(Modal::EditWindow {
            active_pane,
            start_selected,
            end_selected,
            ..
        }) = &mut self.modal
        {
            let selected = match active_pane {
                Pane::Start => start_selected,
                Pane::End => end_selected,
            };
            *selected = if *selected == 0 {
                TIME_SLOTS.len() - 1
            } else {
                *selected - 1
            };
        }
    }

    /// Move the active pane's selection down in the Edit Window modal (wraps around).
    pub fn edit_window_down(&mut self) {
        if let Some(Modal::EditWindow {
            active_pane,
            start_selected,
            end_selected,
            ..
        }) = &mut self.modal
        {
            let selected = match active_pane {
                Pane::Start => start_selected,
                Pane::End => end_selected,
            };
            *selected = if *selected >= TIME_SLOTS.len() - 1 {
                0
            } else {
                *selected + 1
            };
        }
    }

    /// Toggle focus between the Start and End panes in the Edit Window modal.
    pub fn edit_window_switch_pane(&mut self) {
        if let Some(Modal::EditWindow { active_pane, .. }) = &mut self.modal {
            *active_pane = match active_pane {
                Pane::Start => Pane::End,
                Pane::End => Pane::Start,
            };
        }
    }

    /// Open the Edit Window modal for the currently-selected zone.
    ///
    /// The UTC row (unified index 0) cannot be edited. The modal is
    /// pre-populated with the zone's existing work window or the session
    /// default.
    pub fn open_edit_window(&mut self) {
        use crate::core::windows::WorkWindow;
        use crate::tui::forms::{Pane, time_slot_index_for_time};

        // Unified index 0 = UTC row, can't edit
        if self.selected_zone == 0 {
            self.status = Some("Cannot edit UTC row".to_string());
            return;
        }

        let Some(zone_name) = self.current_zone_name().map(str::to_string) else {
            self.status = Some("No zone selected".to_string());
            return;
        };

        self.status = None;
        self.show_help = false;

        // Translate unified index to ordered_zones index via display_order
        let display_idx = self.selected_zone - 1;
        let model_idx = self.display_order[display_idx];
        let ordered_idx = self
            .session
            .ordered_zones
            .iter()
            .position(|z| z == &self.model.zones[model_idx].input_name)
            .unwrap_or(display_idx);

        let raw_window = self
            .session
            .work_hours
            .get(&zone_name)
            .cloned()
            .unwrap_or_else(|| self.session.default_window.clone());

        let (start_idx, end_idx) = if let Ok(ww) = WorkWindow::parse(&raw_window) {
            let sh = (ww.start_minute / 60) as u8;
            let sm = (ww.start_minute % 60) as u8;
            let eh = (ww.end_minute / 60) as u8;
            let em = (ww.end_minute % 60) as u8;
            (
                time_slot_index_for_time(sh, sm),
                time_slot_index_for_time(eh, em),
            )
        } else {
            (18, 34) // fallback: 09:00-17:00
        };

        self.modal = Some(Modal::EditWindow {
            zone_index: ordered_idx,
            active_pane: Pane::Start,
            start_selected: start_idx,
            start_scroll_offset: start_idx.saturating_sub(3),
            end_selected: end_idx,
            end_scroll_offset: end_idx.saturating_sub(3),
        });
    }

    /// Append a character to the AddZone modal's filter input and refilter.
    pub fn push_modal_char(&mut self, ch: char) {
        if let Some(Modal::AddZone {
            input,
            entries,
            filtered,
            selected,
            scroll_offset,
        }) = &mut self.modal
        {
            input.push(ch);
            *filtered = refilter(entries, input);
            *selected = 0;
            *scroll_offset = 0;
        }
    }

    /// Delete the last character from the AddZone modal's filter input and refilter.
    pub fn pop_modal_char(&mut self) {
        if let Some(Modal::AddZone {
            input,
            entries,
            filtered,
            selected,
            scroll_offset,
        }) = &mut self.modal
        {
            input.pop();
            *filtered = refilter(entries, input);
            *selected = 0;
            *scroll_offset = 0;
        }
    }

    /// Dismiss the current modal without applying changes.
    pub fn cancel_modal(&mut self) {
        self.status = None;
        self.modal = None;
    }

    /// Confirm and apply the current modal's selection.
    ///
    /// For `AddZone`, adds the selected timezone. For `EditWindow`, writes the
    /// chosen start/end times. On success the modal is closed; on error it
    /// remains open so the user can retry.
    pub fn submit_modal(&mut self) -> Result<()> {
        let modal = self.modal.clone().ok_or_else(|| anyhow!("no modal open"))?;
        let result = match &modal {
            Modal::AddZone {
                input,
                entries,
                filtered,
                selected,
                ..
            } => {
                let zone_name = filtered
                    .get(*selected)
                    .and_then(|&i| entries.get(i))
                    .map(|entry| entry.name.clone())
                    .unwrap_or_else(|| input.clone());
                self.add_zone(zone_name)
            }
            Modal::EditWindow {
                zone_index,
                start_selected,
                end_selected,
                ..
            } => {
                let raw = format!(
                    "{}-{}",
                    crate::tui::forms::format_time_slot(*start_selected),
                    crate::tui::forms::format_time_slot(*end_selected),
                );
                self.update_window(*zone_index, &raw)
            }
        };

        if result.is_ok() {
            self.status = None;
            self.modal = None;
        } else {
            self.modal = Some(modal);
        }

        result
    }

    /// Add a new timezone to the session.
    ///
    /// Validates the zone string, rejects duplicates and UTC/GMT (always shown
    /// as the fixed row), inserts it after the currently-selected zone, and
    /// rebuilds the model. The cursor moves to the newly-added zone.
    pub fn add_zone(&mut self, zone: String) -> Result<()> {
        let zone = zone.trim();
        if zone.is_empty() {
            return Err(anyhow!("zone cannot be empty"));
        }

        let handle = parse_zone(zone)?;

        // Reject UTC/GMT — the UTC row is always present as a fixed row
        if handle.identity_key() == "fixed:0" {
            return Err(anyhow!("UTC is always shown; cannot add {zone} as a zone"));
        }

        if self
            .model
            .zones
            .iter()
            .any(|existing| existing.handle.identity_key() == handle.identity_key())
        {
            return Err(anyhow!("zone already present: {zone}"));
        }

        // Compute insertion point in ordered_zones based on unified index
        let insert_at = if self.selected_zone == 0 {
            // After UTC row = beginning of user zones
            0
        } else {
            let display_idx = self.selected_zone - 1;
            if display_idx < self.display_order.len() {
                let model_idx = self.display_order[display_idx];
                // Find position of this zone in ordered_zones, insert after it
                self.session
                    .ordered_zones
                    .iter()
                    .position(|z| z == &self.model.zones[model_idx].input_name)
                    .map(|p| p + 1)
                    .unwrap_or(self.session.ordered_zones.len())
            } else {
                self.session.ordered_zones.len()
            }
        };

        let mut session = self.session.clone();
        session.ordered_zones.insert(insert_at, zone.to_string());
        session.extra_zones.push(zone.to_string());
        self.apply_session(session)?;

        // Select the newly added zone in unified space
        // Find it in the new display_order
        let new_model_idx = self
            .model
            .zones
            .iter()
            .position(|z| z.input_name == zone)
            .unwrap_or(0);
        let new_display_idx = self
            .display_order
            .iter()
            .position(|&idx| idx == new_model_idx)
            .unwrap_or(0);
        self.selected_zone = new_display_idx + 1; // +1 for unified space (0=UTC)

        self.status = None;
        Ok(())
    }

    /// Remove the zone at the given unified index.
    ///
    /// The UTC row (index 0) and the last remaining zone cannot be removed.
    /// Cleans up both `ordered_zones` and the work-hours map, then rebuilds.
    pub fn remove_zone(&mut self, unified_index: usize) -> Result<()> {
        // Unified index 0 = UTC row, can't remove
        if unified_index == 0 {
            self.status = None;
            return Ok(());
        }

        let display_idx = unified_index - 1;
        if display_idx >= self.display_order.len() {
            self.status = None;
            return Ok(());
        }

        // Can't remove if only one zone left
        if self.session.ordered_zones.len() <= 1 {
            self.status = None;
            return Ok(());
        }

        let model_idx = self.display_order[display_idx];
        let zone_name = self.model.zones[model_idx].input_name.clone();

        // Find the zone in ordered_zones and remove it
        let ordered_idx = self
            .session
            .ordered_zones
            .iter()
            .position(|z| z == &zone_name);
        let Some(ordered_idx) = ordered_idx else {
            self.status = None;
            return Ok(());
        };

        let mut session = self.session.clone();
        session.ordered_zones.remove(ordered_idx);
        session.base_zones.retain(|z| z != &zone_name);
        session.extra_zones.retain(|z| z != &zone_name);
        session.work_hours.remove(&zone_name);
        self.apply_session(session)?;

        // Clamp selected_zone to valid range
        let max_unified = self.display_order.len(); // 0=UTC + N user zones
        self.selected_zone = self.selected_zone.min(max_unified);
        self.status = None;
        Ok(())
    }

    /// Swap the zone at `unified_index` with the one above it (Manual sort only).
    pub fn move_zone_up(&mut self, unified_index: usize) {
        // Only works in Manual mode
        if self.sort_mode != SortMode::Manual {
            self.status = None;
            return;
        }

        // Can't move UTC row (0), or first user zone (1) up
        if unified_index <= 1 {
            self.status = None;
            return;
        }

        let display_idx = unified_index - 1;
        if display_idx >= self.display_order.len() {
            self.status = None;
            return;
        }

        // In Manual mode, display_order matches ordered_zones order
        let ordered_idx = display_idx;
        if ordered_idx == 0 || ordered_idx >= self.session.ordered_zones.len() {
            self.status = None;
            return;
        }

        self.session
            .ordered_zones
            .swap(ordered_idx, ordered_idx - 1);
        sync_zone_buckets(&mut self.session);
        self.selected_zone = unified_index - 1;
        let _ = self.rebuild();
        self.status = None;
    }

    /// Swap the zone at `unified_index` with the one below it (Manual sort only).
    pub fn move_zone_down(&mut self, unified_index: usize) {
        // Only works in Manual mode
        if self.sort_mode != SortMode::Manual {
            self.status = None;
            return;
        }

        // Can't move UTC row (0)
        if unified_index == 0 {
            self.status = None;
            return;
        }

        let display_idx = unified_index - 1;
        if display_idx >= self.display_order.len() {
            self.status = None;
            return;
        }

        // In Manual mode, display_order matches ordered_zones order
        let ordered_idx = display_idx;
        if ordered_idx + 1 >= self.session.ordered_zones.len() {
            self.status = None;
            return;
        }

        self.session
            .ordered_zones
            .swap(ordered_idx, ordered_idx + 1);
        sync_zone_buckets(&mut self.session);
        self.selected_zone = unified_index + 1;
        let _ = self.rebuild();
        self.status = None;
    }

    /// Set a zone's work-window string (e.g. `"09:00-17:00"`) by `ordered_zones` index.
    pub fn update_window(&mut self, index: usize, raw: &str) -> Result<()> {
        let Some(zone_name) = self.session.ordered_zones.get(index).cloned() else {
            return Ok(());
        };

        let mut session = self.session.clone();
        session.work_hours.insert(zone_name, raw.trim().to_string());
        self.apply_session(session)?;
        self.status = None;
        Ok(())
    }

    /// Persist the current session to disk, syncing the runtime sort mode first.
    pub fn save(&self) -> Result<()> {
        let mut session = self.session.clone();
        session.sort_mode = self.sort_mode;
        save_session(&session)
    }

    /// Rebuild the model from the current session (convenience wrapper).
    fn rebuild(&mut self) -> Result<()> {
        self.apply_session(self.session.clone())
    }

    /// Replace the session, rebuild the model, and re-clamp cursors.
    fn apply_session(&mut self, session: SessionConfig) -> Result<()> {
        let model = ComparisonModel::rebuild(session, self.now_utc)?;
        self.session = model.session().clone();
        self.model = model;
        self.focused_hour = self
            .focused_hour
            .min(self.model.timeline_slots.len().saturating_sub(1));
        self.recompute_display_order();
        // Clamp selected_zone to valid unified range
        let max_unified = self.display_order.len();
        self.selected_zone = self.selected_zone.min(max_unified);
        Ok(())
    }

    /// Return the input name of the zone at the current unified cursor position.
    fn current_zone_name(&self) -> Option<&str> {
        if self.selected_zone == 0 {
            // UTC row
            return Some("UTC");
        }
        let display_idx = self.selected_zone - 1;
        self.display_order
            .get(display_idx)
            .and_then(|&model_idx| self.model.zones.get(model_idx))
            .map(|z| z.input_name.as_str())
    }

    /// Recompute `display_order` from the current zones, sort mode, and time.
    fn recompute_display_order(&mut self) {
        self.display_order = compute_display_order(&self.model.zones, self.sort_mode, self.now_utc);
    }

    /// Return the stable identity key of the currently-selected zone.
    ///
    /// Used before a re-sort so the cursor can be restored afterwards.
    fn selected_zone_identity_key(&self) -> Option<String> {
        if self.selected_zone == 0 {
            return Some("fixed:0".to_string()); // UTC
        }
        let display_idx = self.selected_zone - 1;
        self.display_order
            .get(display_idx)
            .and_then(|&model_idx| self.model.zones.get(model_idx))
            .map(|z| z.handle.identity_key())
    }

    /// Restore the zone cursor to the zone identified by `prev_key` after a re-sort.
    fn restore_cursor(&mut self, prev_key: Option<String>) {
        let Some(key) = prev_key else {
            return;
        };
        if key == "fixed:0" {
            self.selected_zone = 0;
            return;
        }
        // Find the zone in the new display_order
        for (i, &model_idx) in self.display_order.iter().enumerate() {
            if self.model.zones[model_idx].handle.identity_key() == key {
                self.selected_zone = i + 1; // +1 for unified space
                return;
            }
        }
        // Fallback: clamp
        let max = self.display_order.len();
        self.selected_zone = self.selected_zone.min(max);
    }
}

/// Re-order `base_zones` and `extra_zones` to match the canonical `ordered_zones`
/// sequence after a manual swap, keeping each zone in its original bucket.
fn sync_zone_buckets(session: &mut SessionConfig) {
    session.base_zones = session
        .ordered_zones
        .iter()
        .filter(|zone| session.base_zones.contains(*zone))
        .cloned()
        .collect();
    session.extra_zones = session
        .ordered_zones
        .iter()
        .filter(|zone| session.extra_zones.contains(*zone))
        .cloned()
        .collect();
}
