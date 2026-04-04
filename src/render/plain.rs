//! Plain-text renderer for `--plain` mode.
//!
//! Produces a fixed-width, human-readable table showing the current time
//! in each configured timezone followed by an hour-by-hour timeline grid.

use crate::core::model::ComparisonModel;
use crate::core::timezones::format_utc_offset;

/// Render the comparison model as a plain-text string.
///
/// The output contains a header section with the current local time for
/// each zone, then a timeline grid with one row per zone aligned to
/// `width` columns. `display_order` controls the row ordering.
pub fn render_plain(model: &ComparisonModel, width: u16, display_order: &[usize]) -> String {
    let utc_label = "UTC";
    let header_width = model
        .zones
        .iter()
        .map(|zone| zone.label.len())
        .max()
        .unwrap_or(0)
        .max(utc_label.len())
        + 5;
    let slot_count = model.timeline_slots.len().max(1);
    let offset_width: usize = 8;
    let body_width = usize::from(width)
        .saturating_sub(header_width)
        .saturating_sub(offset_width);
    let cell_width = (body_width / slot_count).max(3);
    let mut lines = Vec::new();

    // Fixed UTC time summary
    let utc_handle =
        crate::core::timezones::ZoneHandle::Fixed(chrono::FixedOffset::east_opt(0).unwrap());
    let utc_local = utc_handle.local_time(model.anchor);
    lines.push(format!(
        "{label:<header_width$}{timestamp}",
        label = format!("{utc_label}:"),
        timestamp = utc_local.format("%Y-%m-%d %H:%M:%S"),
    ));

    // User zones in display order
    for &model_idx in display_order {
        let zone = &model.zones[model_idx];
        let local_time = zone.handle.local_time(model.anchor);
        lines.push(format!(
            "{label:<header_width$}{timestamp}",
            label = format!("{}:", zone.label),
            timestamp = local_time.format("%Y-%m-%d %H:%M:%S"),
        ));
    }

    lines.push(String::new());
    lines.push(format!("{:<header_width$}{:<offset_width$}↓↓", "", ""));

    // Fixed UTC timeline row
    {
        let mut row = format!(
            "{:<header_width$}{:<offset_width$}",
            format!("{utc_label}:"),
            format_utc_offset(0)
        );
        for (index, slot) in model.timeline_slots.iter().enumerate() {
            let local_time = utc_handle.local_time(slot.start_utc);
            let hour = local_time.format("%H").to_string();
            if index + 1 == model.timeline_slots.len() {
                row.push_str(&hour);
            } else {
                row.push_str(&format!("{hour:<cell_width$}"));
            }
        }
        lines.push(row.trim_end().to_string());
    }

    // User zones in display order
    for &model_idx in display_order {
        let zone = &model.zones[model_idx];
        let offset_secs = zone.handle.utc_offset_seconds(model.anchor);
        let offset_str = format_utc_offset(offset_secs);
        let mut row = format!(
            "{:<header_width$}{:<offset_width$}",
            format!("{}:", zone.label),
            offset_str
        );
        for (index, slot) in model.timeline_slots.iter().enumerate() {
            let local_time = zone.handle.local_time(slot.start_utc);
            let hour = local_time.format("%H").to_string();
            if index + 1 == model.timeline_slots.len() {
                row.push_str(&hour);
            } else {
                row.push_str(&format!("{hour:<cell_width$}"));
            }
        }
        lines.push(row.trim_end().to_string());
    }

    lines.push(format!("{:<header_width$}{:<offset_width$}↑↑", "", ""));
    lines.push(String::new());
    lines.join("\n")
}
