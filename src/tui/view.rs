//! Rendering layer for the TUI.
//!
//! All drawing goes through [`render_to_buffer`], which splits the terminal
//! into four vertical sections (header, timeline, footer panels, controls bar)
//! and overlays any active modal or help screen. Everything writes directly to
//! a ratatui [`Buffer`].

use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget,
    Widget, Wrap,
};

use crate::tui::forms::Modal;
use crate::tui::state::AppState;

const UTC_DISPLAY_LABEL: &str = "Coordinated Universal Time (UTC)";

/// Compute the minimum terminal (width, height) required to render the TUI
/// in compact mode for the current state (slot count, label widths, etc.).
///
/// The width accounts for: zone_label_col + separator + offset_col + all compact slots
/// plus block borders and a frame-right-edge column.
///
/// The height accounts for: header + minimum timeline (inner >= 4) + footer + controls.
pub fn min_terminal_size(state: &AppState) -> (u16, u16) {
    let slot_count = state.model.timeline_slots.len() as u16;

    // Zone column width in compact mode (same logic as render_timeline)
    let utc_label_len = UTC_DISPLAY_LABEL.chars().count() as u16;
    let max_label_len = state
        .model
        .zones
        .iter()
        .map(|z| z.label.chars().count() as u16)
        .max()
        .unwrap_or(4)
        .max(utc_label_len);
    // In compact mode: zone_width = max_label_len.clamp(9, available.max(9))
    // At the minimum width, available is tight, so zone_width = 9 (the minimum clamp).
    // But we need at least enough width for labels to be readable.
    // Use the minimum clamp value of 9 for the threshold calculation.
    let zone_width: u16 = max_label_len.clamp(9, 32);

    // Inner width: zone_width + 1(sep) + offset_col(7) + slots*(slot_width(2)+1(sep)) + 1(frame right edge)
    // slot_x_positions[i] = zone_width + 1 + 7 + i * 3
    // Last slot rightmost pixel: slot_x_positions[last] + 2
    // Frame right edge: slot_x_positions[last] + 2 (needs < inner.width)
    // So min_inner_width = zone_width + 8 + (slot_count - 1) * 3 + 2 + 1
    //                    = zone_width + 8 + slot_count * 3
    let min_inner_width = zone_width + 8 + slot_count * 3;
    // Terminal width = inner_width + 2 (block left/right borders)
    let min_width = min_inner_width + 2;

    // Height: header + timeline_area + footer + controls
    // Timeline inner needs >= 4 (frame_top + header + utc_row + frame_bottom).
    // Timeline area = inner + 2 borders = 6.
    // But the layout uses Constraint::Min(10) for timeline — when space is short,
    // timeline gets area.height - header - footer - controls.
    // We need that remainder >= 6 for inner >= 4.
    //
    // header_height and controls_height depend on the terminal width, so we compute
    // them at min_width.
    let header_height = compute_header_height(state, min_width);
    let controls_height = compute_controls_height(state, min_width);
    let footer_height: u16 = 10;
    let min_timeline_area: u16 = 6; // inner 4 + 2 borders

    let min_height = header_height + min_timeline_area + footer_height + controls_height;

    (min_width, min_height)
}

/// Render the entire UI into `buffer`.
///
/// If the terminal is smaller than the minimum required size, shows a resize
/// prompt instead. Otherwise paints header, timeline grid, footer panels,
/// controls bar, then any open modal or help overlay.
pub fn render_to_buffer(buffer: &mut Buffer, area: Rect, state: &AppState) {
    let (min_w, min_h) = min_terminal_size(state);
    if area.width < min_w || area.height < min_h {
        Paragraph::new(format!("Resize terminal to at least {min_w}x{min_h}"))
            .block(Block::bordered().title("Zone Timeline"))
            .render(area, buffer);
        return;
    }

    let header_height = compute_header_height(state, area.width);
    let controls_height = compute_controls_height(state, area.width);

    let [header_area, timeline_area, footer_area, controls_area] = Layout::vertical([
        Constraint::Length(header_height),   // Header: dynamic (3-5 rows)
        Constraint::Min(10),                 // Timeline: remaining space
        Constraint::Length(10),              // Footer panels: fixed
        Constraint::Length(controls_height), // Controls bar: dynamic (1-2 rows)
    ])
    .areas(area);

    render_header(buffer, header_area, state);
    render_timeline(buffer, timeline_area, state);
    render_footer(buffer, footer_area, state);
    render_controls(buffer, controls_area, state);
    render_modal(buffer, area, state);

    if state.show_help {
        render_help(buffer, area);
    }
}

/// Compute the total header section height (content lines + 2 for borders).
/// Content lines are clamped to 1..=3.
pub fn compute_header_height(state: &AppState, terminal_width: u16) -> u16 {
    let inner_width = terminal_width.saturating_sub(2) as usize; // subtract left/right border
    if inner_width == 0 {
        return 3; // minimum: 1 content line + 2 borders
    }

    // Measure the summary spans total character width
    let mut total_chars: usize = 0;
    for (i, zone) in state.model.zones.iter().enumerate() {
        if i > 0 {
            total_chars += 5; // "  |  "
        }
        total_chars += zone.label.chars().count();
        total_chars += 1; // space
        total_chars += 5; // "HH:MM"
        let offset_secs = zone.handle.utc_offset_seconds(state.now_utc);
        let offset_str = format!(
            " ({})",
            crate::core::timezones::format_utc_offset_short(offset_secs)
        );
        total_chars += offset_str.chars().count();
    }

    let mut content_lines = if total_chars == 0 {
        1
    } else {
        total_chars.div_ceil(inner_width) as u16
    };

    // Add status line if present
    if state.status.is_some() {
        content_lines += 1;
    }

    let clamped = content_lines.clamp(1, 3);
    clamped + 2 // add top/bottom border
}

/// Compute the controls bar height: 1 if spans fit on one line, 2 otherwise.
/// The controls bar has no border, so inner width == terminal width.
pub fn compute_controls_height(state: &AppState, terminal_width: u16) -> u16 {
    let total_chars = compute_controls_char_width(state);
    if total_chars <= terminal_width as usize {
        1
    } else {
        2
    }
}

/// Compute the total character width of all control bar spans.
fn compute_controls_char_width(state: &AppState) -> usize {
    // Reproduce the span content character counts from render_controls
    let anchor_str = format!("{} UTC", state.model.anchor.format("%Y-%m-%d %H:%M"));
    let sort_str = format!(" {}", state.sort_mode.label());

    let parts: &[&str] = &[
        " Anchor ",
        &anchor_str,
        "  ",
        "Sort:",
        &sort_str,
        "  ",
        "h/l",
        " scroll",
        "  ",
        "j/k",
        " zones",
        "  ",
        "o",
        " order",
        "  ",
        "a/x",
        " +/-",
        "  ",
        "e",
        " edit",
        "  ",
        "s",
        " save",
        "  ",
        "?",
        " help",
        "  ",
        "q",
        " quit",
    ];
    parts.iter().map(|s| s.chars().count()).sum()
}

fn render_header(buffer: &mut Buffer, area: Rect, state: &AppState) {
    let mut summary_spans: Vec<Span<'static>> = Vec::new();
    for (i, zone) in state.model.zones.iter().enumerate() {
        if i > 0 {
            summary_spans.push("  |  ".dark_gray());
        }
        let local = zone.handle.local_time(state.now_utc);
        let offset_secs = zone.handle.utc_offset_seconds(state.now_utc);
        summary_spans.push(Span::styled(zone.label.clone(), Style::new().cyan()));
        summary_spans.push(Span::raw(" "));
        summary_spans.push(Span::styled(
            format!("{}", local.format("%H:%M")),
            Style::new().bold(),
        ));
        summary_spans.push(Span::styled(
            format!(
                " ({})",
                crate::core::timezones::format_utc_offset_short(offset_secs)
            ),
            Style::new().dark_gray(),
        ));
    }

    let inner_width = area.width.saturating_sub(2) as usize;
    let total_chars: usize = summary_spans
        .iter()
        .map(|s| s.content.chars().count())
        .sum();
    let max_content_lines = (area.height.saturating_sub(2)) as usize; // area already sized by compute_header_height

    // Check if summary overflows the available content lines
    let summary_lines_needed = if inner_width > 0 {
        total_chars.div_ceil(inner_width)
    } else {
        1
    };
    let overflows = summary_lines_needed > max_content_lines;

    let mut lines: Vec<Line<'static>> = Vec::new();

    if overflows && max_content_lines > 0 {
        // Truncate: fit into max_content_lines, replace last 3 chars with "..."
        let char_budget = max_content_lines * inner_width;
        let truncated_spans = truncate_spans_with_ellipsis(&summary_spans, char_budget);
        lines.push(Line::from(truncated_spans));
    } else {
        lines.push(Line::from(summary_spans));
    }

    // Only add status line if present (suppress empty status)
    if let Some(status) = &state.status {
        lines.push(Line::from(status.clone()));
    }

    Paragraph::new(lines)
        .block(Block::bordered().title("Current Times"))
        .wrap(Wrap { trim: true })
        .render(area, buffer);
}

/// Truncate a list of spans to fit within `char_budget` characters,
/// replacing the last 3 characters with "...".
fn truncate_spans_with_ellipsis(spans: &[Span<'static>], char_budget: usize) -> Vec<Span<'static>> {
    if char_budget < 3 {
        return vec![Span::raw("...")];
    }
    let budget = char_budget - 3; // reserve space for "..."
    let mut result: Vec<Span<'static>> = Vec::new();
    let mut remaining = budget;

    for span in spans {
        let span_len = span.content.chars().count();
        if span_len <= remaining {
            result.push(span.clone());
            remaining -= span_len;
        } else if remaining > 0 {
            let truncated: String = span.content.chars().take(remaining).collect();
            result.push(Span::styled(truncated, span.style));
            break;
        } else {
            break;
        }
    }

    result.push("...".dark_gray());
    result
}

fn render_timeline(buffer: &mut Buffer, area: Rect, state: &AppState) {
    let block = Block::bordered().title("Zone Timeline");
    let inner = block.inner(area);
    block.render(area, buffer);

    if inner.height < 4 || inner.width < 20 {
        return;
    }

    let slot_count = state.model.timeline_slots.len();

    // Compute zone column width based on the longest label (including the UTC display label for fixed row)
    let utc_label_len = UTC_DISPLAY_LABEL.chars().count() as u16;
    let max_label_len = state
        .model
        .zones
        .iter()
        .map(|z| z.label.chars().count() as u16)
        .max()
        .unwrap_or(4)
        .max(utc_label_len);

    let offset_col_width: u16 = 7; // "+00:00 " with trailing space
    let wide_zone_width = max_label_len.max(18);
    let wide_table_width = wide_zone_width
        + offset_col_width
        + (slot_count as u16).saturating_mul(5)
        + slot_count as u16;
    let compact = inner.width < wide_table_width;

    let (zone_width, slot_width) = if compact {
        let compact_slot_space = (slot_count as u16) * 3;
        let available = inner.width.saturating_sub(compact_slot_space);
        (max_label_len.clamp(9, available.max(9)), 2u16)
    } else {
        (wide_zone_width, 5u16)
    };

    let frame_top_row = 0u16;
    let header_row = 1u16;
    let first_zone_row = 2u16;
    let user_zone_count = state.display_order.len();
    // Available rows for user zones: inner.height - 4 (frame_top + header + utc_row + frame_bottom)
    let available_user_rows = inner.height.saturating_sub(4) as usize;
    let visible_user_count = user_zone_count.min(available_user_rows);
    let needs_scroll = user_zone_count > available_user_rows;

    // Compute scroll offset to keep selected zone visible
    let timeline_scroll_offset = if needs_scroll && state.selected_zone > 0 {
        let display_idx = state.selected_zone - 1;
        if display_idx >= available_user_rows {
            display_idx - available_user_rows + 1
        } else {
            0
        }
    } else {
        0
    };

    let frame_bottom_row = first_zone_row + 1 + visible_user_count as u16;

    let slot_x_positions: Vec<u16> = (0..slot_count)
        .map(|i| zone_width + 1 + offset_col_width + (i as u16) * (slot_width + 1))
        .collect();

    let now_col: Option<usize> = state
        .model
        .timeline_slots
        .iter()
        .position(|slot| slot.current_minute_offset.is_some());
    let selected_col: usize = state.focused_hour;

    let shoulder_minutes = state.session.shoulder_hours * 60;

    let write_text = |buf: &mut Buffer, x: u16, y: u16, text: &str, style: Style| {
        for (col, ch) in (x..).zip(text.chars()) {
            if col >= inner.width {
                break;
            }
            if let Some(cell) = buf.cell_mut((inner.x + col, inner.y + y)) {
                cell.set_char(ch);
                cell.set_style(style);
            }
        }
    };

    // Render header row — column header
    let col_header = format!("{:<width$}", "Zone", width = zone_width as usize);
    write_text(buffer, 0, header_row, &col_header, Style::new().bold());
    write_text(
        buffer,
        zone_width + 1,
        header_row,
        "Offset",
        Style::new().bold(),
    );

    // Render fixed UTC row (always first zone row)
    {
        let row_y = first_zone_row;
        let utc_selected = state.selected_zone == 0;
        let dim_style = if utc_selected {
            Style::new().cyan().bold().dim()
        } else {
            Style::new().dim()
        };

        let label: String = UTC_DISPLAY_LABEL
            .chars()
            .take(zone_width as usize)
            .collect();
        write_text(buffer, 0, row_y, &label, dim_style);
        write_text(
            buffer,
            zone_width + 1,
            row_y,
            &crate::core::timezones::format_utc_offset(0),
            dim_style,
        );

        let utc_handle =
            crate::core::timezones::ZoneHandle::Fixed(chrono::FixedOffset::east_opt(0).unwrap());

        for (slot_idx, slot) in state.model.timeline_slots.iter().enumerate() {
            let local = utc_handle.local_time(slot.start_utc);
            let text = if compact {
                local.format("%H").to_string()
            } else {
                local.format("%H:%M").to_string()
            };
            let is_overlap = overlaps_slot(state, slot_idx);
            let mut style = Style::new().dim();
            if is_overlap {
                style = style.underlined();
            }
            if utc_selected {
                style = style.bold();
            }
            write_text(buffer, slot_x_positions[slot_idx], row_y, &text, style);
        }
    }

    // Render user zone data rows (using display_order, with scroll offset)
    for (visible_idx, display_idx) in
        (timeline_scroll_offset..timeline_scroll_offset + visible_user_count).enumerate()
    {
        let &model_idx = &state.display_order[display_idx];
        let zone = &state.model.zones[model_idx];
        let row_y = first_zone_row + 1 + visible_idx as u16; // +1 for UTC row
        let zone_selected = state.selected_zone == display_idx + 1; // +1 for UTC row

        let label_style = if zone_selected {
            Style::new().cyan().bold()
        } else {
            Style::new()
        };

        let label: String = zone.label.chars().take(zone_width as usize).collect();
        write_text(buffer, 0, row_y, &label, label_style);

        let offset_secs = zone.handle.utc_offset_seconds(state.now_utc);
        let offset_str = crate::core::timezones::format_utc_offset(offset_secs);
        write_text(buffer, zone_width + 1, row_y, &offset_str, label_style);

        for (slot_idx, slot) in state.model.timeline_slots.iter().enumerate() {
            let local = zone.handle.local_time(slot.start_utc);
            let minute_of_day = zone.handle.minute_of_day(slot.start_utc);
            let in_window = zone.window.contains(minute_of_day);
            let in_shoulder = zone
                .window
                .shoulder_contains(minute_of_day, shoulder_minutes);
            let is_overlap = overlaps_slot(state, slot_idx);

            let style = cell_style(&CellStyleInput {
                in_window,
                in_shoulder,
                is_overlap,
                zone_selected,
                is_header: false,
            });

            let text = if compact {
                local.format("%H").to_string()
            } else {
                local.format("%H:%M").to_string()
            };
            write_text(buffer, slot_x_positions[slot_idx], row_y, &text, style);
        }
    }

    // Draw box-drawing frames
    // Each entry: (column_index, style, top_row)
    let frames: Vec<(usize, Style, u16)> = {
        let selected_style = Style::new().white();
        let now_style = Style::new().dark_gray();
        let mut frames = Vec::new();
        if let Some(nc) = now_col {
            if nc == selected_col {
                // Overlap: show now frame in white (selected frame hidden)
                frames.push((nc, selected_style, frame_top_row));
            } else {
                // Different columns: now frame full-height, selected frame excludes header
                frames.push((nc, now_style, frame_top_row));
                frames.push((selected_col, selected_style, header_row));
            }
        } else {
            // No now column: only selected frame (excludes header)
            frames.push((selected_col, selected_style, header_row));
        }
        frames
    };

    for (col_idx, frame_style, top_row) in &frames {
        if *col_idx >= slot_x_positions.len() {
            continue;
        }
        draw_column_frame(
            buffer,
            inner,
            slot_x_positions[*col_idx],
            slot_width,
            *top_row,
            frame_bottom_row,
            *frame_style,
        );
    }

    // Draw "NOW" label in the header cell of the current-time column
    if let Some(nc) = now_col
        && nc < slot_x_positions.len()
    {
        let now_text = "NOW";
        let text_len = now_text.len() as u16;
        if slot_width >= text_len {
            let padding = (slot_width - text_len) / 2;
            let now_label_style = if nc == selected_col {
                Style::new().white()
            } else {
                Style::new().dark_gray()
            };
            write_text(
                buffer,
                slot_x_positions[nc] + padding,
                header_row,
                now_text,
                now_label_style,
            );
        }
    }

    // Scrollbar (only when content overflows)
    if needs_scroll {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None);
        let mut scrollbar_state = ScrollbarState::new(user_zone_count)
            .position(state.selected_zone.saturating_sub(1))
            .viewport_content_length(available_user_rows);
        StatefulWidget::render(scrollbar, inner, buffer, &mut scrollbar_state);
    }
}

/// Draw a box-drawing frame around a single timeline column.
fn draw_column_frame(
    buffer: &mut Buffer,
    inner: Rect,
    slot_x_offset: u16,
    slot_width: u16,
    frame_top_row: u16,
    frame_bottom_row: u16,
    style: Style,
) {
    let col_x = inner.x + slot_x_offset;
    let left_gap_x = col_x.saturating_sub(1);
    let right_gap_x = col_x + slot_width;
    let header_row = frame_top_row + 1;

    // Top border row
    let top_y = inner.y + frame_top_row;
    if let Some(cell) = buffer.cell_mut((left_gap_x, top_y)) {
        cell.set_symbol("┌");
        cell.set_style(style);
    }
    for dx in 0..slot_width {
        if let Some(cell) = buffer.cell_mut((col_x + dx, top_y)) {
            cell.set_symbol("─");
            cell.set_style(style);
        }
    }
    if right_gap_x < inner.x + inner.width
        && let Some(cell) = buffer.cell_mut((right_gap_x, top_y))
    {
        cell.set_symbol("┐");
        cell.set_style(style);
    }

    // Vertical edges on data rows (header + zone rows)
    for row_y in header_row..=frame_bottom_row.saturating_sub(1) {
        let y = inner.y + row_y;
        if let Some(cell) = buffer.cell_mut((left_gap_x, y)) {
            cell.set_symbol("│");
            cell.set_style(style);
        }
        if right_gap_x < inner.x + inner.width
            && let Some(cell) = buffer.cell_mut((right_gap_x, y))
        {
            cell.set_symbol("│");
            cell.set_style(style);
        }
    }

    // Bottom border row
    let bottom_y = inner.y + frame_bottom_row;
    if let Some(cell) = buffer.cell_mut((left_gap_x, bottom_y)) {
        cell.set_symbol("└");
        cell.set_style(style);
    }
    for dx in 0..slot_width {
        if let Some(cell) = buffer.cell_mut((col_x + dx, bottom_y)) {
            cell.set_symbol("─");
            cell.set_style(style);
        }
    }
    if right_gap_x < inner.x + inner.width
        && let Some(cell) = buffer.cell_mut((right_gap_x, bottom_y))
    {
        cell.set_symbol("┘");
        cell.set_style(style);
    }
}

fn render_footer(buffer: &mut Buffer, area: Rect, state: &AppState) {
    let [windows_area, zones_area, details_area] = Layout::horizontal([
        Constraint::Percentage(34),
        Constraint::Percentage(33),
        Constraint::Percentage(33),
    ])
    .areas(area);

    let working_window_lines: Vec<Line<'static>> = {
        let windows = &state.model.classified_windows();
        let has_ideal_or_feasible = windows.iter().any(|w| {
            w.tier == crate::core::model::WindowTier::Ideal
                || w.tier == crate::core::model::WindowTier::Feasible
        });

        let mut lines: Vec<Line<'static>> = Vec::new();

        // Header text
        if has_ideal_or_feasible {
            let zone_label = selected_zone_label(state);
            lines.push(Line::from(Span::styled(
                format!("Times shown for {zone_label}"),
                Style::new().dark_gray(),
            )));
        } else if !windows.is_empty() {
            lines.push(Line::from(Span::styled(
                "No ideal or feasible windows",
                Style::new().dark_gray(),
            )));
            lines.push(Line::from(Span::styled(
                "shared by the selected zones.",
                Style::new().dark_gray(),
            )));
            let zone_label = selected_zone_label(state);
            lines.push(Line::from(Span::styled(
                format!("Times shown for {zone_label}"),
                Style::new().dark_gray(),
            )));
        }

        // Table rows
        let header_lines = lines.len();
        let panel_height = windows_area.height.saturating_sub(2) as usize; // minus border
        let visible_rows = panel_height.saturating_sub(header_lines);

        for window in windows.iter().take(visible_rows) {
            let (tier_label, tier_color) = match window.tier {
                crate::core::model::WindowTier::Ideal => ("Ideal    ", Color::Green),
                crate::core::model::WindowTier::Feasible => ("Feasible ", Color::Yellow),
                crate::core::model::WindowTier::LeastBad => ("Least Bad", Color::Red),
            };

            let (start_str, end_str) = format_window_times(state, window);

            let time_str = if window.tier == crate::core::model::WindowTier::LeastBad {
                format!(
                    "{}-{} ({}m, {}/{} zones)",
                    start_str,
                    end_str,
                    window.duration_minutes,
                    window.zones_in_window,
                    window.total_zones,
                )
            } else {
                format!("{}-{} ({}m)", start_str, end_str, window.duration_minutes,)
            };

            lines.push(Line::from(vec![
                Span::styled(format!("  {tier_label} "), Style::new().fg(tier_color)),
                Span::raw(time_str),
            ]));
        }

        lines
    };

    let zone_lines: Vec<Line<'static>> = {
        let mut lines = Vec::new();
        // UTC fixed row
        let utc_offset_label = " (UTC+0)";
        let is_selected = state.selected_zone == 0;
        if is_selected {
            lines.push(Line::from(vec![
                Span::styled(
                    UTC_DISPLAY_LABEL.to_string(),
                    Style::new().on_dark_gray().white(),
                ),
                Span::styled(
                    utc_offset_label.to_string(),
                    Style::new().on_dark_gray().white().dim(),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::raw(UTC_DISPLAY_LABEL.to_string()),
                Span::styled(utc_offset_label.to_string(), Style::new().dim()),
            ]));
        }
        // User zones in display order
        for (display_idx, &model_idx) in state.display_order.iter().enumerate() {
            let zone = &state.model.zones[model_idx];
            let offset_secs = zone.handle.utc_offset_seconds(state.now_utc);
            let offset_label = format!(
                " ({})",
                crate::core::timezones::format_utc_offset_short(offset_secs)
            );
            let is_selected = state.selected_zone == display_idx + 1;
            if is_selected {
                lines.push(Line::from(vec![
                    Span::styled(zone.label.clone(), Style::new().on_dark_gray().white()),
                    Span::styled(offset_label, Style::new().on_dark_gray().white().dim()),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::raw(zone.label.clone()),
                    Span::styled(offset_label, Style::new().dim()),
                ]));
            }
        }
        lines
    };

    let details_lines = details_lines(state);

    Paragraph::new(working_window_lines)
        .block(panel_block("Working Windows", false))
        .wrap(Wrap { trim: true })
        .render(windows_area, buffer);

    {
        let zones_block = panel_block("Zones", false);
        let zones_inner = zones_block.inner(zones_area);
        let panel_inner_height = zones_inner.height as usize;
        let total_zone_lines = 1 + state.display_order.len(); // UTC + user zones

        // Compute scroll offset to keep selected zone visible
        let scroll_offset = if state.selected_zone >= panel_inner_height {
            state.selected_zone - panel_inner_height + 1
        } else {
            0
        };

        let visible_end = (scroll_offset + panel_inner_height).min(zone_lines.len());
        let visible_lines: Vec<Line<'static>> = zone_lines
            .into_iter()
            .skip(scroll_offset)
            .take(visible_end - scroll_offset)
            .collect();

        Paragraph::new(visible_lines)
            .block(zones_block)
            .render(zones_area, buffer);

        // Scrollbar (only when content overflows)
        if total_zone_lines > panel_inner_height {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None);
            let mut scrollbar_state = ScrollbarState::new(total_zone_lines)
                .position(state.selected_zone)
                .viewport_content_length(panel_inner_height);
            StatefulWidget::render(scrollbar, zones_inner, buffer, &mut scrollbar_state);
        }
    }

    Paragraph::new(details_lines)
        .block(panel_block("Details", false))
        .wrap(Wrap { trim: true })
        .render(details_area, buffer);
}

fn render_controls(buffer: &mut Buffer, area: Rect, state: &AppState) {
    let key_style = Style::new().cyan().dim();
    let desc_style = Style::new().dark_gray();
    let sep = Span::styled("  ", desc_style);

    let spans = vec![
        Span::styled(" Anchor ", key_style),
        Span::styled(
            format!("{} UTC", state.model.anchor.format("%Y-%m-%d %H:%M")),
            desc_style,
        ),
        sep.clone(),
        Span::styled("Sort:", key_style),
        Span::styled(format!(" {}", state.sort_mode.label()), desc_style),
        sep.clone(),
        Span::styled("h/l", key_style),
        Span::styled(" scroll", desc_style),
        sep.clone(),
        Span::styled("j/k", key_style),
        Span::styled(" zones", desc_style),
        sep.clone(),
        Span::styled("o", key_style),
        Span::styled(" order", desc_style),
        sep.clone(),
        Span::styled("a/x", key_style),
        Span::styled(" +/-", desc_style),
        sep.clone(),
        Span::styled("e", key_style),
        Span::styled(" edit", desc_style),
        sep.clone(),
        Span::styled("s", key_style),
        Span::styled(" save", desc_style),
        sep.clone(),
        Span::styled("?", key_style),
        Span::styled(" help", desc_style),
        sep.clone(),
        Span::styled("q", key_style),
        Span::styled(" quit", desc_style),
    ];

    if area.height >= 2 {
        // Wrapping mode: use Paragraph with Wrap
        let total_chars: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        let char_budget = (area.width as usize) * 2;

        let final_spans = if total_chars > char_budget {
            truncate_spans_with_ellipsis(&spans, char_budget)
        } else {
            spans
        };

        Paragraph::new(Line::from(final_spans))
            .wrap(Wrap { trim: true })
            .render(area, buffer);
    } else {
        // Single line mode
        Paragraph::new(Line::from(spans)).render(area, buffer);
    }
}

fn selected_zone_label(state: &AppState) -> String {
    if state.selected_zone == 0 {
        "UTC".to_string()
    } else {
        let display_idx = state.selected_zone - 1;
        state
            .display_order
            .get(display_idx)
            .and_then(|&model_idx| state.model.zones.get(model_idx))
            .map(|z| z.label.clone())
            .unwrap_or_else(|| "UTC".to_string())
    }
}

fn format_window_times(
    state: &AppState,
    window: &crate::core::model::ClassifiedWindow,
) -> (String, String) {
    if state.selected_zone == 0 {
        // UTC row selected — show times in UTC
        (
            window.start_utc.format("%H:%M").to_string(),
            window.end_utc.format("%H:%M").to_string(),
        )
    } else {
        let display_idx = state.selected_zone - 1;
        if let Some(&model_idx) = state.display_order.get(display_idx)
            && let Some(zone) = state.model.zones.get(model_idx)
        {
            let start_local = zone.handle.local_time(window.start_utc);
            let end_local = zone.handle.local_time(window.end_utc);
            return (
                start_local.format("%H:%M").to_string(),
                end_local.format("%H:%M").to_string(),
            );
        }
        // Fallback to UTC
        (
            window.start_utc.format("%H:%M").to_string(),
            window.end_utc.format("%H:%M").to_string(),
        )
    }
}

fn details_lines(state: &AppState) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let label_style = Style::new().dark_gray().bold();
    let label_width = 8;

    let zone = if state.selected_zone == 0 {
        None
    } else {
        let display_idx = state.selected_zone - 1;
        state
            .display_order
            .get(display_idx)
            .and_then(|&model_idx| state.model.zones.get(model_idx))
    };

    if let Some(zone) = zone {
        lines.push(Line::from(vec![
            Span::styled(format!("{:<label_width$}", "Zone"), label_style),
            Span::raw(zone.label.clone()),
        ]));
        lines.push(Line::from(vec![
            Span::styled(format!("{:<label_width$}", "Window"), label_style),
            Span::raw(format!(
                "{:02}:{:02}-{:02}:{:02}",
                zone.window.start_minute / 60,
                zone.window.start_minute % 60,
                zone.window.end_minute / 60,
                zone.window.end_minute % 60
            )),
        ]));
    } else if state.selected_zone == 0 {
        lines.push(Line::from(vec![
            Span::styled(format!("{:<label_width$}", "Zone"), label_style),
            Span::raw(UTC_DISPLAY_LABEL.to_string()),
        ]));
    }

    if let Some(slot) = state.model.timeline_slots.get(state.focused_hour) {
        lines.push(Line::from(vec![
            Span::styled(format!("{:<label_width$}", "UTC"), label_style),
            Span::raw(format!("{}", slot.start_utc.format("%Y-%m-%d %H:%M"))),
        ]));
        lines.push(Line::from(vec![
            Span::styled(format!("{:<label_width$}", "Offset"), label_style),
            Span::raw(format!("{:+}", slot.offset_hours)),
        ]));
    }

    if let Some(slot) = state.model.timeline_slots.get(state.focused_hour) {
        if state.selected_zone == 0 {
            lines.push(Line::from(vec![
                Span::styled(format!("{:<label_width$}", "Local"), label_style),
                Span::raw(format!("{}", slot.start_utc.format("%Y-%m-%d %H:%M"))),
            ]));
        } else if let Some(zone) = state
            .display_order
            .get(state.selected_zone - 1)
            .and_then(|&mi| state.model.zones.get(mi))
        {
            let local = zone.handle.local_time(slot.start_utc);
            lines.push(Line::from(vec![
                Span::styled(format!("{:<label_width$}", "Local"), label_style),
                Span::raw(format!("{}", local.format("%Y-%m-%d %H:%M"))),
            ]));
        }
    }

    lines
}

fn render_help(buffer: &mut Buffer, area: Rect) {
    let key_style = Style::new().cyan().dim();
    let desc_style = Style::new().gray();

    let key_col: usize = 14; // width for the key column
    let indent = "  ";

    let mut lines: Vec<Line> = Vec::new();

    // --- Navigation ---
    lines.push(Line::from(" Navigation".cyan().bold()));
    lines.push(Line::from(vec![
        Span::raw(indent),
        Span::styled(format!("{:<key_col$}", "\u{2190}/\u{2192}  h/l"), key_style),
        Span::styled("Move hour cursor", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::raw(indent),
        Span::styled(format!("{:<key_col$}", "\u{2191}/\u{2193}  j/k"), key_style),
        Span::styled("Move zone cursor", desc_style),
    ]));
    lines.push(Line::from(""));

    // --- Zone Management ---
    lines.push(Line::from(" Zone Management".cyan().bold()));
    lines.push(Line::from(vec![
        Span::raw(indent),
        Span::styled(format!("{:<key_col$}", "a"), key_style),
        Span::styled("Add zone", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::raw(indent),
        Span::styled(format!("{:<key_col$}", "x"), key_style),
        Span::styled("Remove zone", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::raw(indent),
        Span::styled(format!("{:<key_col$}", "e"), key_style),
        Span::styled("Edit work window", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::raw(indent),
        Span::styled(format!("{:<key_col$}", "o"), key_style),
        Span::styled("Cycle sort order", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::raw(indent),
        Span::styled(format!("{:<key_col$}", "J / K"), key_style),
        Span::styled("Move zone up/down (Manual sort)", desc_style),
    ]));
    lines.push(Line::from(""));

    // --- General ---
    lines.push(Line::from(" General".cyan().bold()));
    lines.push(Line::from(vec![
        Span::raw(indent),
        Span::styled(format!("{:<key_col$}", "s"), key_style),
        Span::styled("Save config", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::raw(indent),
        Span::styled(format!("{:<key_col$}", "?"), key_style),
        Span::styled("Toggle help", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::raw(indent),
        Span::styled(format!("{:<key_col$}", "q"), key_style),
        Span::styled("Quit", desc_style),
    ]));
    lines.push(Line::from(""));

    // --- Dismiss hint ---
    lines.push(Line::from(
        "                          Esc or ? to close "
            .dark_gray()
            .dim(),
    ));

    let content_height = lines.len() as u16 + 2; // +2 for borders
    let popup = centered_rect(area, 48, content_height);
    Clear.render(popup, buffer);
    Paragraph::new(lines)
        .block(Block::bordered().title(" Help ".cyan()))
        .render(popup, buffer);
}

fn render_modal(buffer: &mut Buffer, area: Rect, state: &AppState) {
    let Some(modal) = &state.modal else {
        return;
    };

    match modal {
        Modal::AddZone {
            input,
            entries,
            filtered,
            selected,
            scroll_offset,
        } => render_add_zone_picker(
            buffer,
            area,
            input,
            entries,
            filtered,
            *selected,
            *scroll_offset,
        ),
        Modal::EditWindow {
            zone_index,
            active_pane,
            start_selected,
            start_scroll_offset,
            end_selected,
            end_scroll_offset,
        } => render_edit_window(
            buffer,
            area,
            state,
            *zone_index,
            active_pane,
            *start_selected,
            *start_scroll_offset,
            *end_selected,
            *end_scroll_offset,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn render_edit_window(
    buffer: &mut Buffer,
    area: Rect,
    state: &AppState,
    zone_index: usize,
    active_pane: &crate::tui::forms::Pane,
    start_selected: usize,
    start_scroll_offset: usize,
    end_selected: usize,
    end_scroll_offset: usize,
) {
    use crate::tui::forms::{Pane, TIME_SLOTS, format_time_slot};

    let popup_width: u16 = 60;
    let list_visible_rows: u16 = 7;
    // height: 1 top border + 1 blank + list_visible_rows + 1 blank + 1 summary + 1 hints + 1 bottom border
    let popup_height: u16 = list_visible_rows + 6;
    let popup = centered_rect(area, popup_width, popup_height);
    Clear.render(popup, buffer);

    // Determine zone name for title
    let zone_name = state
        .session
        .ordered_zones
        .get(zone_index)
        .cloned()
        .unwrap_or_else(|| "Unknown".to_string());
    let title = format!("Edit Working Window: {}", zone_name);

    let block = Block::bordered().title(title);
    let inner = block.inner(popup);
    block.render(popup, buffer);

    if inner.height < 4 || inner.width < 30 {
        return;
    }

    let list_height = list_visible_rows as usize;
    let total_slots = TIME_SLOTS.len();

    // Pane widths: each pane is ~14 chars wide, with gap between
    let pane_width: u16 = 14;
    let gap: u16 = 4;
    let total_pane_width = pane_width * 2 + gap;
    let pane_x_offset = (inner.width.saturating_sub(total_pane_width)) / 2;
    let start_pane_x = pane_x_offset;
    let end_pane_x = start_pane_x + pane_width + gap;

    let (active_border_style, inactive_border_style) =
        (Style::new().cyan(), Style::new().dark_gray());
    let (active_highlight, inactive_highlight) = (
        Style::new().on_cyan().black(),
        Style::new().on_dark_gray().white(),
    );

    let is_start_active = *active_pane == Pane::Start;

    // Helper to render one pane
    let render_pane = |buf: &mut Buffer,
                       pane_x: u16,
                       label: &str,
                       selected: usize,
                       mut scroll_off: usize,
                       is_active: bool| {
        let border_style = if is_active {
            active_border_style
        } else {
            inactive_border_style
        };
        let highlight = if is_active {
            active_highlight
        } else {
            inactive_highlight
        };

        // Adjust scroll offset to keep selected centered
        if selected < scroll_off {
            scroll_off = selected;
        }
        if list_height > 0 && selected >= scroll_off + list_height {
            scroll_off = selected - list_height + 1;
        }
        // Try to center
        if list_height > 0 {
            let ideal = selected.saturating_sub(list_height / 2);
            let max_offset = total_slots.saturating_sub(list_height);
            scroll_off = ideal.min(max_offset);
        }

        // Draw pane border using Block
        let pane_rect = Rect::new(
            inner.x + pane_x,
            inner.y,
            pane_width,
            list_height as u16 + 2, // +2 for top/bottom border
        );
        let pane_block = Block::bordered()
            .title(Span::styled(format!(" {label} "), border_style))
            .border_style(border_style);
        let pane_inner = pane_block.inner(pane_rect);
        pane_block.render(pane_rect, buf);

        // Draw list items
        for (vis_row, slot_idx) in (scroll_off..total_slots).take(list_height).enumerate() {
            let time_str = format_time_slot(slot_idx);
            let is_sel = slot_idx == selected;
            let prefix = if is_sel { "\u{25b8} " } else { "  " };
            let text = format!("{prefix}{time_str}");
            let style = if is_sel { highlight } else { Style::new() };

            let row_y = pane_inner.y + vis_row as u16;
            let row_area = Rect::new(pane_inner.x, row_y, pane_inner.width, 1);

            // Fill background for selected row
            if is_sel {
                for x in row_area.x..row_area.x + row_area.width {
                    if let Some(cell) = buf.cell_mut((x, row_y)) {
                        cell.set_style(style);
                    }
                }
            }

            Paragraph::new(Span::styled(text, style)).render(row_area, buf);
        }

        // Scrollbar
        if total_slots > list_height {
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None);
            let scrollbar_area = Rect::new(
                pane_rect.x + pane_rect.width - 1,
                pane_inner.y,
                1,
                list_height as u16,
            );
            let mut scrollbar_state = ScrollbarState::new(total_slots)
                .position(selected)
                .viewport_content_length(list_height);
            StatefulWidget::render(scrollbar, scrollbar_area, buf, &mut scrollbar_state);
        }
    };

    render_pane(
        buffer,
        start_pane_x,
        "Start",
        start_selected,
        start_scroll_offset,
        is_start_active,
    );
    render_pane(
        buffer,
        end_pane_x,
        "End",
        end_selected,
        end_scroll_offset,
        !is_start_active,
    );

    // Summary line
    let summary_y = inner.y + list_visible_rows + 2; // after pane borders
    if summary_y < inner.y + inner.height {
        let (sh, sm) = TIME_SLOTS[start_selected.min(total_slots - 1)];
        let (eh, em) = TIME_SLOTS[end_selected.min(total_slots - 1)];
        let start_mins = sh as u16 * 60 + sm as u16;
        let end_mins = eh as u16 * 60 + em as u16;
        let duration_mins = if end_mins > start_mins {
            end_mins - start_mins
        } else {
            (24 * 60 - start_mins) + end_mins
        };
        let dur_h = duration_mins / 60;
        let dur_m = duration_mins % 60;

        let overnight = if end_mins <= start_mins {
            " \u{25d1} overnight"
        } else {
            ""
        };

        let summary = format!(
            "Window: {} \u{2014} {}  ({}h {}m){}",
            format_time_slot(start_selected),
            format_time_slot(end_selected),
            dur_h,
            dur_m,
            overnight,
        );
        let summary_area = Rect::new(inner.x + 1, summary_y, inner.width.saturating_sub(2), 1);
        Paragraph::new(Span::styled(summary, Style::new().yellow())).render(summary_area, buffer);
    }

    // Hints line
    let hints_y = summary_y + 1;
    if hints_y < inner.y + inner.height {
        let hints_area = Rect::new(inner.x + 1, hints_y, inner.width.saturating_sub(2), 1);
        Paragraph::new(Line::from(vec![
            "Tab".cyan(),
            " switch pane  ".dark_gray(),
            "\u{2191}\u{2193}/jk".cyan(),
            " scroll  ".dark_gray(),
            "Enter".cyan(),
            " submit  ".dark_gray(),
            "Esc".cyan(),
            " cancel".dark_gray(),
        ]))
        .render(hints_area, buffer);
    }
}

fn render_add_zone_picker(
    buffer: &mut Buffer,
    area: Rect,
    input: &str,
    entries: &[crate::tui::forms::ZonePickerEntry],
    filtered: &[usize],
    selected: usize,
    mut scroll_offset: usize,
) {
    let popup_height = (area.height * 70 / 100)
        .max(12)
        .min(area.height.saturating_sub(2));
    let popup = centered_rect(area, 64, popup_height);
    Clear.render(popup, buffer);

    let block = Block::bordered().title("Add Zone");
    let inner = block.inner(popup);
    block.render(popup, buffer);

    if inner.height < 4 || inner.width < 10 {
        return;
    }

    // Layout: filter line (1) + separator (1) + list (variable) + hint line (1)
    let list_height = inner.height.saturating_sub(3) as usize;

    // Adjust scroll_offset to keep selected visible
    if selected < scroll_offset {
        scroll_offset = selected;
    }
    if list_height > 0 && selected >= scroll_offset + list_height {
        scroll_offset = selected - list_height + 1;
    }

    // Filter input line
    let filter_area = Rect::new(inner.x, inner.y, inner.width, 1);
    let placeholder = if input.is_empty() {
        "type to filter..."
    } else {
        ""
    };
    Paragraph::new(Line::from(vec![
        "> ".cyan(),
        Span::raw(input),
        Span::styled(
            if placeholder.is_empty() {
                "_"
            } else {
                placeholder
            },
            Style::new().dark_gray(),
        ),
    ]))
    .render(filter_area, buffer);

    // Separator line
    let sep_area = Rect::new(inner.x, inner.y + 1, inner.width, 1);
    let sep: String = "─".repeat(inner.width as usize);
    Paragraph::new(Span::styled(&*sep, Style::new().dark_gray())).render(sep_area, buffer);

    // List area
    let list_area_y = inner.y + 2;
    for (visual_row, list_index) in (scroll_offset..filtered.len())
        .take(list_height)
        .enumerate()
    {
        let entry_index = filtered[list_index];
        let entry = &entries[entry_index];
        let is_selected = list_index == selected;

        let style = if is_selected {
            Style::new().on_cyan().black()
        } else {
            Style::new()
        };

        let prefix = if is_selected { "▸ " } else { "  " };
        let text = format!("{prefix}{}", entry.display);
        let truncated: String = text.chars().take(inner.width as usize).collect();

        let row_area = Rect::new(inner.x, list_area_y + visual_row as u16, inner.width, 1);

        // Fill background for selected row
        if is_selected {
            for x in row_area.x..row_area.x + row_area.width {
                if let Some(cell) = buffer.cell_mut((x, row_area.y)) {
                    cell.set_style(style);
                }
            }
        }

        Paragraph::new(Span::styled(truncated, style)).render(row_area, buffer);
    }

    // Match count and hint line
    let hint_y = inner.y + 2 + list_height as u16;
    if hint_y < inner.y + inner.height {
        let hint_area = Rect::new(inner.x, hint_y, inner.width, 1);
        let match_count = filtered.len();
        Paragraph::new(Line::from(vec![
            Span::styled(format!("{match_count} matches  "), Style::new().dark_gray()),
            Span::styled(
                "↑↓ navigate  Enter select  Esc cancel",
                Style::new().dark_gray(),
            ),
        ]))
        .render(hint_area, buffer);
    }

    // Scrollbar (only when content overflows)
    if filtered.len() > list_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None);
        let scrollbar_area = Rect::new(
            popup.x + popup.width - 1,
            inner.y + 2,
            1,
            list_height as u16,
        );
        let mut scrollbar_state = ScrollbarState::new(filtered.len())
            .position(selected)
            .viewport_content_length(list_height);
        StatefulWidget::render(scrollbar, scrollbar_area, buffer, &mut scrollbar_state);
    }
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let popup_width = width.min(area.width.saturating_sub(2)).max(10);
    let popup_height = height.min(area.height.saturating_sub(2)).max(5);
    area.centered(
        Constraint::Length(popup_width),
        Constraint::Length(popup_height),
    )
}

fn panel_block(title: &'static str, focused: bool) -> Block<'static> {
    let style = if focused {
        Style::new().cyan()
    } else {
        Style::new()
    };
    Block::bordered().title(Span::styled(title, style))
}

fn overlaps_slot(state: &AppState, slot_index: usize) -> bool {
    let Some(slot) = state.model.timeline_slots.get(slot_index) else {
        return false;
    };

    state
        .model
        .overlap_segments
        .iter()
        .any(|segment| slot.start_utc >= segment.start_utc && slot.start_utc < segment.end_utc)
}

/// Input for computing a timeline cell's style.
///
/// `in_window` takes priority over `in_shoulder` — if both are true, the cell
/// is treated as in-window (green). Callers should set at most one of these,
/// but the precedence is defined here for safety.
#[derive(Clone, Copy, Default)]
struct CellStyleInput {
    /// True if the slot's local time falls inside the zone's work window.
    in_window: bool,
    /// True if the slot's local time falls in the shoulder zone.
    in_shoulder: bool,
    /// True if this column is a mutual-overlap column (all zones' windows overlap).
    is_overlap: bool,
    /// True if this cell is in the selected zone's row.
    zone_selected: bool,
    /// True if this cell is in the UTC header row (no color coding).
    is_header: bool,
}

/// Compute the `Style` for a single timeline cell.
///
/// Style layering:
/// 1. Work-window foreground (green/amber/red) — skipped for header
/// 2. Overlap underline
/// 3. Selected-zone bold (or header bold)
fn cell_style(input: &CellStyleInput) -> Style {
    let mut style = Style::new();

    // Layer 1: work-window foreground (not for headers)
    if !input.is_header {
        style = style.fg(if input.in_window {
            Color::Green
        } else if input.in_shoulder {
            Color::Yellow
        } else {
            Color::Red
        });
    }

    // Layer 2: overlap underline
    if input.is_overlap {
        style = style.underlined();
    }

    // Layer 3: selected zone bold OR header bold
    if input.zone_selected || input.is_header {
        style = style.bold();
    }

    style
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::{Color, Modifier};

    #[test]
    fn cell_style_green_for_in_window() {
        let style = cell_style(&CellStyleInput {
            in_window: true,
            ..Default::default()
        });
        assert_eq!(style.fg, Some(Color::Green));
    }

    #[test]
    fn cell_style_yellow_for_shoulder() {
        let style = cell_style(&CellStyleInput {
            in_shoulder: true,
            ..Default::default()
        });
        assert_eq!(style.fg, Some(Color::Yellow));
    }

    #[test]
    fn cell_style_red_for_outside() {
        let style = cell_style(&CellStyleInput::default());
        assert_eq!(style.fg, Some(Color::Red));
    }

    #[test]
    fn cell_style_overlap_adds_underline() {
        let style = cell_style(&CellStyleInput {
            in_window: true,
            is_overlap: true,
            ..Default::default()
        });
        assert!(style.add_modifier.contains(Modifier::UNDERLINED));
        assert_eq!(style.fg, Some(Color::Green));
    }

    #[test]
    fn cell_style_selected_zone_adds_bold() {
        let style = cell_style(&CellStyleInput {
            in_window: true,
            zone_selected: true,
            ..Default::default()
        });
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn cell_style_header_has_no_color_but_keeps_bold() {
        let style = cell_style(&CellStyleInput {
            is_header: true,
            ..Default::default()
        });
        assert_eq!(style.fg, None);
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn cell_style_header_overlap_gets_underline() {
        let style = cell_style(&CellStyleInput {
            is_header: true,
            is_overlap: true,
            ..Default::default()
        });
        assert!(style.add_modifier.contains(Modifier::UNDERLINED));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn cell_style_all_modifiers_combine() {
        let style = cell_style(&CellStyleInput {
            in_window: true,
            is_overlap: true,
            zone_selected: true,
            ..Default::default()
        });
        assert_eq!(style.fg, Some(Color::Green));
        assert!(style.add_modifier.contains(Modifier::UNDERLINED));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }
}
