//! Modal dialog data types and timezone picker helpers.
//!
//! Defines the [`Modal`] enum (variants for the zone picker and the work-window
//! editor), the 48 half-hour [`TIME_SLOTS`] used by the window editor, and
//! functions for building and filtering the timezone picker list.

use chrono::{DateTime, Utc};

use crate::core::timezones::{ZoneHandle, all_timezones};

/// Which pane of the Edit Window modal currently has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    /// The work-window start time pane.
    Start,
    /// The work-window end time pane.
    End,
}

/// 48 time slots at 30-minute intervals: (hour, minute).
pub const TIME_SLOTS: [(u8, u8); 48] = {
    let mut slots = [(0u8, 0u8); 48];
    let mut i = 0;
    while i < 48 {
        slots[i] = ((i / 2) as u8, if i % 2 == 0 { 0 } else { 30 });
        i += 1;
    }
    slots
};

/// Find the TIME_SLOTS index closest to the given hour and minute.
/// Snaps to the nearest 30-minute boundary (rounding down).
pub fn time_slot_index_for_time(hour: u8, minute: u8) -> usize {
    let idx = (hour as usize) * 2 + if minute >= 30 { 1 } else { 0 };
    idx.min(TIME_SLOTS.len() - 1)
}

/// Format a TIME_SLOTS entry as "HH:MM".
pub fn format_time_slot(index: usize) -> String {
    let (h, m) = TIME_SLOTS[index.min(TIME_SLOTS.len() - 1)];
    format!("{:02}:{:02}", h, m)
}

/// A single entry in the timezone picker list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZonePickerEntry {
    /// Canonical zone name used when adding to the session (e.g. `"America/New_York"`).
    pub name: String,
    /// Human-readable display string including offset and current local time.
    pub display: String,
    /// Lowercased search key for fuzzy filtering.
    pub search_key: String,
}

/// Active modal dialog state.
///
/// Each variant carries all the transient UI state for its dialog
/// (filter text, cursor position, scroll offset, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modal {
    /// Fuzzy-filter timezone picker for adding a new zone.
    AddZone {
        input: String,
        entries: Vec<ZonePickerEntry>,
        filtered: Vec<usize>,
        selected: usize,
        scroll_offset: usize,
    },
    /// Dual-pane time picker for editing a zone's work window.
    EditWindow {
        zone_index: usize,
        active_pane: Pane,
        start_selected: usize,
        start_scroll_offset: usize,
        end_selected: usize,
        end_scroll_offset: usize,
    },
}

impl Modal {
    /// Return a mutable reference to the text input buffer (AddZone only).
    ///
    /// # Panics
    /// Panics if called on `EditWindow`.
    pub fn input_mut(&mut self) -> &mut String {
        match self {
            Self::AddZone { input, .. } => input,
            Self::EditWindow { .. } => {
                panic!("EditWindow does not have text input")
            }
        }
    }
}

/// Build the full list of timezone picker entries, including a "local" entry
/// (the host's system timezone) followed by every IANA zone except UTC/GMT.
pub fn build_picker_entries(now_utc: DateTime<Utc>) -> Vec<ZonePickerEntry> {
    let mut entries = Vec::with_capacity(600);

    // "local" entry
    if let Ok(name) = iana_time_zone::get_timezone()
        && let Ok(tz) = name.parse::<chrono_tz::Tz>()
    {
        let handle = ZoneHandle::Named(tz);
        let local = handle.local_time(now_utc);
        let offset_secs = local.offset().local_minus_utc();
        entries.push(ZonePickerEntry {
            name: "local".to_string(),
            display: format!(
                "local ({})  ({}, {})",
                name,
                format_offset(offset_secs),
                local.format("%H:%M")
            ),
            search_key: format!("local {}", name.to_lowercase()),
        });
    }

    // All IANA timezones (excluding UTC/GMT since they are fixed rows)
    for tz in all_timezones() {
        let name = tz.name().to_string();
        if matches!(
            name.as_str(),
            "UTC" | "GMT" | "Etc/UTC" | "Etc/GMT" | "Etc/Greenwich"
        ) {
            continue;
        }
        let handle = ZoneHandle::Named(*tz);
        let local = handle.local_time(now_utc);
        let offset_secs = local.offset().local_minus_utc();
        entries.push(ZonePickerEntry {
            display: format!(
                "{}  ({}, {})",
                name,
                format_offset(offset_secs),
                local.format("%H:%M")
            ),
            search_key: name.to_lowercase(),
            name,
        });
    }

    entries
}

/// Return indices into `entries` whose `search_key` contains `filter` (case-insensitive).
/// An empty filter returns all indices.
pub fn refilter(entries: &[ZonePickerEntry], filter: &str) -> Vec<usize> {
    if filter.is_empty() {
        return (0..entries.len()).collect();
    }
    let lower = filter.to_lowercase();
    entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.search_key.contains(&lower))
        .map(|(i, _)| i)
        .collect()
}

/// Format a UTC offset in seconds as a compact string (e.g. `"UTC+5"` or `"UTC+5:30"`).
fn format_offset(total_seconds: i32) -> String {
    let sign = if total_seconds < 0 { '-' } else { '+' };
    let abs = total_seconds.unsigned_abs();
    let hours = abs / 3600;
    let minutes = (abs % 3600) / 60;
    if minutes == 0 {
        format!("UTC{sign}{hours}")
    } else {
        format!("UTC{sign}{hours}:{minutes:02}")
    }
}
