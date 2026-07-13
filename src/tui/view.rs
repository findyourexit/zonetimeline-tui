//! Rendering layer for the TUI.
//!
//! All drawing goes through [`render_to_buffer`], which splits the terminal
//! into four vertical sections (header, timeline, footer panels, controls bar)
//! and overlays any active modal or help screen. Everything writes directly to
//! a ratatui [`Buffer`].

use chrono::Timelike;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget,
    Widget, Wrap,
};

use crate::core::model::MinuteClass;
use crate::tui::forms::Modal;
use crate::tui::palette::{Capability, Palette, Role};
use crate::tui::ribbon::{self, RibbonState};
use crate::tui::state::AppState;

/// The minimum terminal (width, height) for a legible render.
///
/// The ribbon layout scales to fill any width and scrolls vertically when there
/// are more zones than rows, so the only hard requirement is the absolute floor
/// below which the resize guard is shown.
pub fn min_terminal_size(_state: &AppState) -> (u16, u16) {
    (80, 24)
}

/// Render the entire UI into `buffer`.
///
/// Below the 80×24 floor a "Resize terminal" guard is shown. Above it, the full
/// layout (header, timeline, footer panels, controls) is rendered; the ribbon
/// timeline scales to fill the pane width and scrolls vertically when there are
/// more zones than rows. Modals and the help overlay are painted on top.
pub fn render_to_buffer(buffer: &mut Buffer, area: Rect, state: &AppState) {
    render_to_buffer_with_palette(buffer, area, state, &Palette::from_env());
}

/// Render the entire UI using an explicit [`Palette`].
///
/// Used by tests to inject a fixed capability; [`render_to_buffer`] delegates
/// here after resolving the palette from the environment.
pub fn render_to_buffer_with_palette(
    buffer: &mut Buffer,
    area: Rect,
    state: &AppState,
    palette: &Palette,
) {
    // Absolute floor: below 80x24 nothing legible fits — show the resize guard.
    let (min_w, min_h) = min_terminal_size(state);
    if area.width < min_w || area.height < min_h {
        Paragraph::new("Resize terminal to at least 80x24")
            .block(Block::bordered().title("Zone Timeline"))
            .render(area, buffer);
        return;
    }

    // Above the floor the ribbon layout scales to fill the pane width and scrolls
    // vertically when there are more zones than rows.
    let header_height = compute_header_height(state, area.width);
    let controls_height = compute_controls_height(state, area.width);

    let [header_area, timeline_area, footer_area, controls_area] = Layout::vertical([
        Constraint::Length(header_height),
        Constraint::Min(10),
        Constraint::Length(10),
        Constraint::Length(controls_height),
    ])
    .areas(area);

    render_header(buffer, header_area, state, palette);
    render_timeline(buffer, timeline_area, state, palette);
    render_footer(buffer, footer_area, state, palette);
    render_controls(buffer, controls_area, state, palette);

    render_modal(buffer, area, state, palette);

    if state.show_help {
        render_help(buffer, area, palette);
    }
}

/// Header height: one legend line (plus one for an active status line) and borders.
pub fn compute_header_height(state: &AppState, _terminal_width: u16) -> u16 {
    let content_lines = 1 + u16::from(state.status.is_some());
    content_lines.clamp(1, 3) + 2
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

/// Total character width of the controls bar (sum of all segment contents).
fn compute_controls_char_width(state: &AppState) -> usize {
    controls_segments(state)
        .iter()
        .map(|(text, _)| text.chars().count())
        .sum()
}

/// Ordered controls-bar segments as `(text, role)` pairs, grouped Navigate /
/// Zones / General. Palette-free so width measurement and styled rendering
/// share one content source and never drift.
fn controls_segments(state: &AppState) -> Vec<(String, Role)> {
    let key = Role::KeyHint;
    let desc = Role::Muted;
    vec![
        (" h/l".into(), key),
        (" cursor".into(), desc),
        ("  ".into(), desc),
        ("j/k".into(), key),
        (" zones".into(), desc),
        ("  ".into(), desc),
        ("n".into(), key),
        (" now".into(), desc),
        ("  │  ".into(), desc),
        ("a".into(), key),
        (" add".into(), desc),
        ("  ".into(), desc),
        ("x".into(), key),
        (" del".into(), desc),
        ("  ".into(), desc),
        ("J/K".into(), key),
        (" move".into(), desc),
        ("  ".into(), desc),
        ("e".into(), key),
        (" edit".into(), desc),
        ("  ".into(), desc),
        ("o".into(), key),
        (format!(" sort:{}", state.sort_mode.label()), desc),
        ("  │  ".into(), desc),
        ("s".into(), key),
        (" save".into(), desc),
        ("  ".into(), desc),
        ("?".into(), key),
        (" help".into(), desc),
        ("  ".into(), desc),
        ("q".into(), key),
        (" quit".into(), desc),
    ]
}

/// Styled controls-bar spans, resolving each segment's role through `palette`.
fn controls_spans(state: &AppState, palette: &Palette) -> Vec<Span<'static>> {
    controls_segments(state)
        .into_iter()
        .map(|(text, role)| Span::styled(text, palette.style(role)))
        .collect()
}

fn render_header(buffer: &mut Buffer, area: Rect, state: &AppState, palette: &Palette) {
    let clock = state.model.anchor.format("%Y-%m-%d %H:%M");
    let anchor_label = match state.session.anchor {
        crate::core::model::AnchorSpec::Now => "Now",
        crate::core::model::AnchorSpec::Explicit(_) => "Date",
    };
    let title = format!(" {anchor_label} · {clock} UTC ");

    let mut spans = legend_spans(palette);
    let inner_width = area.width.saturating_sub(2) as usize;
    let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    if inner_width > 0 && total > inner_width {
        spans = truncate_spans_with_ellipsis(&spans, inner_width, palette);
    }

    let mut lines: Vec<Line<'static>> = vec![Line::from(spans)];
    if let Some(status) = &state.status {
        lines.push(Line::from(Span::styled(
            status.clone(),
            palette.style(Role::Caution),
        )));
    }

    Paragraph::new(lines)
        .block(Block::bordered().title(title))
        .render(area, buffer);
}

/// A representative, legible foreground colour for a legend swatch.
fn legend_color(palette: &Palette, role: Role) -> Color {
    match palette.capability() {
        Capability::Truecolor => match role {
            Role::Core => Color::Rgb(0x2e, 0xcc, 0x71),
            Role::Shoulder => Color::Rgb(0xd0, 0x8a, 0x3a),
            _ => Color::Rgb(0x6a, 0x6f, 0x7e),
        },
        _ => match role {
            Role::Core => Color::Green,
            Role::Shoulder => Color::Yellow,
            _ => Color::DarkGray,
        },
    }
}

/// Build the availability + marker legend shown in the header.
///
/// In monochrome the swatches fall back to the same shade characters the ribbon
/// uses, so the key stays meaningful without colour.
fn legend_spans(palette: &Palette) -> Vec<Span<'static>> {
    let mono = palette.capability() == Capability::Monochrome;
    let swatch = |role: Role, shade: &'static str| -> Span<'static> {
        if mono {
            Span::raw(shade)
        } else {
            Span::styled("██", Style::new().fg(legend_color(palette, role)))
        }
    };
    let label = |t: &'static str| Span::styled(t, palette.style(Role::Muted));
    let marker = |glyph: &'static str, role: Role, t: &'static str| -> [Span<'static>; 2] {
        [
            Span::styled(glyph, palette.style(role).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" {t}"), palette.style(Role::Muted)),
        ]
    };
    let mut spans = vec![
        swatch(Role::Core, "██"),
        label(" Core   "),
        swatch(Role::Shoulder, "▓▓"),
        label(" Shoulder   "),
        swatch(Role::Off, "░░"),
        label(" Off     "),
    ];
    spans.extend(marker("▼", Role::MarkerNow, "Now"));
    spans.push(label("  "));
    spans.extend(marker("◆", Role::MarkerCursor, "Cursor"));
    spans
}

/// Truncate a list of spans to fit within `char_budget` characters,
/// replacing the last 3 characters with "...".
fn truncate_spans_with_ellipsis(
    spans: &[Span<'static>],
    char_budget: usize,
    palette: &Palette,
) -> Vec<Span<'static>> {
    if char_budget < 3 {
        return vec![Span::styled("...", palette.style(Role::Muted))];
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

    result.push(Span::styled("...", palette.style(Role::Muted)));
    result
}

fn render_timeline(buffer: &mut Buffer, area: Rect, state: &AppState, palette: &Palette) {
    let block = Block::bordered().title("Zone Timeline");
    let inner = block.inner(area);
    block.render(area, buffer);

    if inner.height < 4 || inner.width < 20 {
        return;
    }

    let slot_count = state.model.timeline_slots.len();
    if slot_count == 0 {
        return;
    }

    // Zone label gutter (longest label or the "Overlap" header, clamped).
    let max_label = state
        .model
        .zones
        .iter()
        .map(|z| z.label.chars().count())
        .max()
        .unwrap_or(3)
        .max("Overlap".len());
    let zone_width = (max_label as u16).clamp(7, 20);

    let ribbon_x0 = zone_width + 1; // gutter + separator
    let ribbon_w = inner.width.saturating_sub(ribbon_x0);
    if ribbon_w < 2 {
        return;
    }
    let ribbon_w_usize = ribbon_w as usize;

    // The ribbon fills the full available width: every column maps to a slice of
    // the visible time range, so the timeline scales to the pane.
    let total_minutes = (slot_count as i64) * 60;
    let m_per_col = total_minutes as f64 / ribbon_w as f64;
    let timeline_start = state.model.timeline_slots[0].start_utc;
    let shoulder_minutes = state.session.shoulder_hours * 60;

    // Map a minute offset from the timeline start to a ribbon column.
    let col_of = |minute: i64| -> usize {
        ((minute as f64 / m_per_col) as i64).clamp(0, ribbon_w as i64 - 1) as usize
    };

    // Row layout: a merged UTC axis/reference row on top (hour scale + "UTC"
    // gutter label), then the user zone ribbons, the OVERLAP strip, and a
    // one-line best-window summary beneath it.
    let axis_row = 0u16;
    let first_zone_row = 1u16;
    let user_zone_count = state.display_order.len();
    // Reserve three non-zone rows: the UTC axis + strip + best-window summary.
    let available_user_rows = inner.height.saturating_sub(3) as usize;
    let visible_user_count = user_zone_count.min(available_user_rows);
    let needs_scroll = user_zone_count > available_user_rows;
    let scroll_offset = if needs_scroll && state.selected_zone > 0 {
        let display_idx = state.selected_zone - 1;
        if display_idx >= available_user_rows {
            display_idx - available_user_rows + 1
        } else {
            0
        }
    } else {
        0
    };

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

    // Resolve marker columns up front so axis labels can dodge the carets.
    let now_col = state
        .model
        .timeline_slots
        .iter()
        .enumerate()
        .find(|(_, s)| s.current_minute_offset.is_some())
        .map(|(idx, s)| {
            col_of((idx as i64) * 60 + i64::from(s.current_minute_offset.unwrap_or(0)))
        });
    let cursor_col = col_of(state.cursor_minutes);

    // --- Time axis (UTC hours), labelled at a density that fits the width ---
    let cols_per_hour = ribbon_w as f64 / slot_count as f64;
    let label_every = ((6.0 / cols_per_hour).ceil() as usize).max(1);
    for (off, hour) in ribbon::hour_ticks(
        timeline_start.hour(),
        timeline_start.minute(),
        total_minutes,
        label_every,
    ) {
        let col = col_of(off);
        // A 2-char hour label collides with a marker if either of its cells
        // lands on a marker column; skip it so the caret reads cleanly.
        let collides = [col, col + 1]
            .iter()
            .any(|&c| now_col == Some(c) || cursor_col == c);
        if collides {
            continue;
        }
        write_text(
            buffer,
            ribbon_x0 + col as u16,
            axis_row,
            &format!("{hour:02}"),
            palette.style(Role::Axis),
        );
    }

    // --- UTC label in the axis-row gutter (the hour scale is itself UTC) ---
    {
        let utc_selected = state.selected_zone == 0;
        let label_style = if utc_selected {
            palette.style(Role::LabelSelected).dim()
        } else {
            Style::new().dim()
        };
        let label = fit_label("UTC", zone_width as usize);
        write_text(buffer, 0, axis_row, &label, label_style);
    }

    // --- User zone ribbons (fill the full width; no day glyphs) ---
    for (visible_idx, display_idx) in
        (scroll_offset..scroll_offset + visible_user_count).enumerate()
    {
        let model_idx = state.display_order[display_idx];
        let zone = &state.model.zones[model_idx];
        let row_y = first_zone_row + visible_idx as u16;
        let zone_selected = state.selected_zone == display_idx + 1;

        let label_style = if zone_selected {
            palette.style(Role::LabelSelected)
        } else {
            palette.style(Role::Label)
        };
        let label = fit_label(&zone.label, zone_width as usize);
        write_text(buffer, 0, row_y, &label, label_style);

        let coverage =
            ribbon::column_coverage(ribbon_w_usize, total_minutes, RIBBON_BLUR_CELLS, |m| {
                let instant = timeline_start + chrono::Duration::minutes(m);
                ribbon::classify(
                    zone.handle.minute_of_day(instant),
                    &zone.window,
                    shoulder_minutes,
                )
            });
        for (i, counts) in coverage.iter().enumerate() {
            draw_ribbon_cell(
                buffer,
                inner.x + ribbon_x0 + i as u16,
                inner.y + row_y,
                *counts,
                palette,
            );
        }
    }

    // --- OVERLAP strip: one aggregate sample per column (coarse — a column may
    // land a minute off the exact "Best" summary below, which scans every minute) ---
    let strip_row = first_zone_row + visible_user_count as u16;
    let total_zones = state.model.zones.len();
    {
        let lbl = fit_label("Overlap", zone_width as usize);
        write_text(buffer, 0, strip_row, &lbl, palette.style(Role::PanelTitle));
        for c in 0..ribbon_w_usize {
            let m = ((c as f64 + 0.5) * m_per_col) as i64;
            let instant = timeline_start + chrono::Duration::minutes(m);
            let (in_count, reach_count) =
                crate::core::model::reach_counts(&state.model.zones, instant, shoulder_minutes);
            let class = ribbon::overlap_class(in_count, reach_count, total_zones);
            draw_overlap_cell(
                buffer,
                inner.x + ribbon_x0 + c as u16,
                inner.y + strip_row,
                class,
                reach_count,
                total_zones,
                palette,
            );
        }
    }

    // --- Best-window summary line (turns former dead space into the answer) ---
    let summary_row = strip_row + 1;
    if summary_row < inner.height {
        let (text, style) = best_window_summary(state, total_zones, palette);
        write_text(buffer, 0, summary_row, &text, style);
    }

    // --- NOW + cursor markers: overlay a caret + bg-preserving vertical rule ---
    let draw_marker = |buf: &mut Buffer, col: usize, role: Role, caret: char| {
        let fg = palette.style(role).fg;
        let x = inner.x + ribbon_x0 + col as u16;
        if let Some(cell) = buf.cell_mut((x, inner.y + axis_row)) {
            cell.set_char(caret);
            if let Some(f) = fg {
                cell.set_fg(f);
            }
            cell.modifier.insert(Modifier::BOLD);
        }
        // Vertical rule through the ribbon rows; keep each cell's background so
        // the cursor reads as an overlay, not a gap torn in the ribbon.
        for ry in first_zone_row..=strip_row {
            if let Some(cell) = buf.cell_mut((x, inner.y + ry)) {
                cell.set_char('│');
                if let Some(f) = fg {
                    cell.set_fg(f);
                }
            }
        }
    };
    if let Some(col) = now_col {
        draw_marker(buffer, col, Role::MarkerNow, '▼');
    }
    // Cursor drawn last so it wins on overlap.
    draw_marker(buffer, cursor_col, Role::MarkerCursor, '◆');

    // Scrollbar (only when content overflows vertically)
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

/// Truncate a zone label to `width` columns, appending `…` when it overflows.
fn fit_label(label: &str, width: usize) -> String {
    let count = label.chars().count();
    if count <= width {
        return label.chars().collect();
    }
    if width <= 1 {
        return label.chars().take(width).collect();
    }
    let mut s: String = label.chars().take(width - 1).collect();
    s.push('…');
    s
}

/// Build the inline best-window summary shown beneath the OVERLAP strip.
/// Times are UTC to match the strip's frame.
fn best_window_summary(state: &AppState, total_zones: usize, palette: &Palette) -> (String, Style) {
    use crate::core::model::WindowTier;
    match state.model.classified_windows().first() {
        None => (
            "Best  no shared window".to_string(),
            palette.style(Role::Muted),
        ),
        Some(w) => {
            let (tag, role) = match w.tier {
                WindowTier::Ideal => ("ideal", Role::Good),
                WindowTier::Feasible => ("feasible", Role::Caution),
                WindowTier::LeastBad => ("fallback", Role::Muted),
            };
            let reach = if w.zones_in_window >= total_zones {
                format!("all {total_zones}")
            } else {
                format!("{}/{}", w.zones_in_window, total_zones)
            };
            (
                format!(
                    "Best  {}–{} UTC · {}m · {reach} · {tag}",
                    w.start_utc.format("%H:%M"),
                    w.end_utc.format("%H:%M"),
                    w.duration_minutes,
                ),
                palette.style(role),
            )
        }
    }
}

/// Map a [`RibbonState`] to its palette [`Role`].
fn role_for(state: RibbonState) -> Role {
    match state {
        RibbonState::Core => Role::Core,
        RibbonState::Shoulder => Role::Shoulder,
        RibbonState::Off => Role::Off,
    }
}

/// Blur radius, in ribbon columns, applied when sampling state coverage: a
/// work-window boundary ramps over roughly `2 * this + 1` columns.
const RIBBON_BLUR_CELLS: f64 = 0.75;

/// Paint one ribbon cell from its per-state coverage counts (Core=0,
/// Shoulder=1, Off=2).
///
/// Truecolor blends the state colours by coverage, so a boundary reads as a
/// smooth multi-cell gradient. Ansi16 can't blend 16 colours, so it fills with
/// the dominant state; monochrome maps the mix onto a shade ramp (`░▒▓█`).
fn draw_ribbon_cell(buffer: &mut Buffer, x: u16, y: u16, counts: [u16; 3], palette: &Palette) {
    let Some(target) = buffer.cell_mut((x, y)) else {
        return;
    };
    match palette.capability() {
        Capability::Monochrome => {
            target.set_char(mono_shade(counts));
            target.set_style(Style::reset());
        }
        Capability::Truecolor => {
            let (r, g, b) = blend_counts(counts, palette);
            target.set_char(' ');
            target.set_style(Style::new().bg(Color::Rgb(r, g, b)));
        }
        Capability::Ansi16 => {
            let st = dominant_state(counts);
            let named = palette.style(role_for(st)).bg.unwrap_or(Color::Reset);
            target.set_char(' ');
            target.set_style(Style::new().bg(named));
        }
    }
}

/// Blend a column's per-state coverage counts into one RGB fill (truecolor).
fn blend_counts(counts: [u16; 3], palette: &Palette) -> (u8, u8, u8) {
    let total: f32 = counts.iter().map(|c| f32::from(*c)).sum();
    if total == 0.0 {
        return (0, 0, 0);
    }
    let mut acc = (0.0f32, 0.0f32, 0.0f32);
    for st in [RibbonState::Core, RibbonState::Shoulder, RibbonState::Off] {
        let w = f32::from(counts[st.index()]) / total;
        let (r, g, b) = palette.role_rgb(role_for(st)).unwrap_or((0, 0, 0));
        acc.0 += f32::from(r) * w;
        acc.1 += f32::from(g) * w;
        acc.2 += f32::from(b) * w;
    }
    (acc.0 as u8, acc.1 as u8, acc.2 as u8)
}

/// The state holding the most coverage (ties resolve Core > Shoulder > Off).
fn dominant_state(counts: [u16; 3]) -> RibbonState {
    [RibbonState::Core, RibbonState::Shoulder, RibbonState::Off]
        .into_iter()
        .max_by_key(|st| counts[st.index()])
        .unwrap_or(RibbonState::Off)
}

/// Monochrome shade for a column: pure states map to `░`/`▓`/`█`, mixes fall on
/// the ramp (weighting Off=0, Shoulder=1, Core=2) with `▒` as the light step.
fn mono_shade(counts: [u16; 3]) -> char {
    let total = f32::from(counts[0] + counts[1] + counts[2]).max(1.0);
    let level = (f32::from(counts[RibbonState::Shoulder.index()])
        + f32::from(counts[RibbonState::Core.index()]) * 2.0)
        / total;
    if level < 0.34 {
        '░'
    } else if level < 0.9 {
        '▒'
    } else if level < 1.5 {
        '▓'
    } else {
        '█'
    }
}

/// Braille dot height (0..=4) for a column's reach fraction; any reach shows at
/// least one dot row so the strip never reads empty where a slot is reachable.
fn braille_height(reach: usize, total: usize) -> u8 {
    if total == 0 || reach == 0 {
        return 0;
    }
    (reach * 4).div_ceil(total).clamp(1, 4) as u8
}

/// A braille glyph filled from the bottom up to `height` rows (both dot cols).
fn braille_bar(height: u8) -> char {
    let bits: u32 = match height {
        0 => 0x00,
        1 => 0xC0,
        2 => 0xE4,
        3 => 0xF6,
        _ => 0xFF,
    };
    char::from_u32(0x2800 + bits).unwrap_or(' ')
}

/// Paint one OVERLAP strip cell as a braille reach-histogram column: dot height
/// encodes how many zones are reachable, colour encodes the meeting tier.
fn draw_overlap_cell(
    buffer: &mut Buffer,
    x: u16,
    y: u16,
    class: MinuteClass,
    reach_count: usize,
    total: usize,
    palette: &Palette,
) {
    let Some(target) = buffer.cell_mut((x, y)) else {
        return;
    };
    let height = braille_height(reach_count, total);
    target.set_char(braille_bar(height));
    match palette.capability() {
        Capability::Monochrome => {
            // No colour: dot height alone carries reach; dim the empty baseline.
            let style = if height == 0 {
                Style::new().add_modifier(Modifier::DIM)
            } else {
                Style::reset()
            };
            target.set_style(style);
        }
        _ => {
            target.set_style(palette.overlap_fg(class, total));
        }
    }
}

fn render_footer(buffer: &mut Buffer, area: Rect, state: &AppState, palette: &Palette) {
    let [windows_area, inspector_area] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(area);

    // --- Best Windows (ranked meeting windows) ---
    let best_windows_lines: Vec<Line<'static>> = {
        let windows = state.model.classified_windows();
        let mut lines: Vec<Line<'static>> = Vec::new();
        let zone_label = selected_zone_label(state);
        lines.push(Line::from(Span::styled(
            format!("Times shown for {zone_label}"),
            palette.style(Role::Muted),
        )));
        if windows.is_empty() {
            lines.push(Line::from(Span::styled(
                "No shared windows found",
                palette.style(Role::Muted),
            )));
        }
        let panel_height = windows_area.height.saturating_sub(2) as usize;
        let visible_rows = panel_height.saturating_sub(lines.len());
        for window in windows.iter().take(visible_rows) {
            let (tier_label, tier_role) = match window.tier {
                crate::core::model::WindowTier::Ideal => ("● Ideal   ", Role::Good),
                crate::core::model::WindowTier::Feasible => ("● Feasible", Role::Caution),
                crate::core::model::WindowTier::LeastBad => ("● Fallback", Role::Muted),
            };
            let (start_str, end_str) = format_window_times(state, window);
            let time_str = if window.tier == crate::core::model::WindowTier::LeastBad {
                format!(
                    "{}-{} ({}m, {}/{})",
                    start_str,
                    end_str,
                    window.duration_minutes,
                    window.zones_in_window,
                    window.total_zones
                )
            } else {
                format!("{}-{} ({}m)", start_str, end_str, window.duration_minutes)
            };
            lines.push(Line::from(vec![
                Span::styled(format!("{tier_label} "), palette.style(tier_role)),
                Span::raw(time_str),
            ]));
        }
        lines
    };
    Paragraph::new(best_windows_lines)
        .block(panel_block("Best Windows", false, palette))
        .wrap(Wrap { trim: true })
        .render(windows_area, buffer);

    // --- Cursor inspector ---
    let lines = inspector_lines(state, palette);
    let block = panel_block("Cursor Position", false, palette);
    let inner = block.inner(inspector_area);
    let panel_inner_height = inner.height as usize;
    let total = lines.len();
    let target_row = state.selected_zone + 1; // +1 for the header line
    let scroll_offset = if panel_inner_height > 0 && target_row >= panel_inner_height {
        target_row - panel_inner_height + 1
    } else {
        0
    };
    let visible_end = (scroll_offset + panel_inner_height).min(total);
    let visible: Vec<Line<'static>> = lines
        .into_iter()
        .skip(scroll_offset)
        .take(visible_end.saturating_sub(scroll_offset))
        .collect();
    Paragraph::new(visible)
        .block(block)
        .render(inspector_area, buffer);
    if total > panel_inner_height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None);
        let mut sb = ScrollbarState::new(total)
            .position(target_row)
            .viewport_content_length(panel_inner_height);
        StatefulWidget::render(scrollbar, inner, buffer, &mut sb);
    }
}

/// Build the cursor-inspector lines: a header plus one row per zone (UTC first)
/// showing the exact local time and availability badge at the cursor instant.
fn inspector_lines(state: &AppState, palette: &Palette) -> Vec<Line<'static>> {
    let cursor_instant = state
        .model
        .timeline_slots
        .first()
        .map(|s| s.start_utc + chrono::Duration::minutes(state.cursor_minutes))
        .unwrap_or(state.model.anchor);
    let shoulder_minutes = state.session.shoulder_hours * 60;
    let total = state.model.zones.len();
    let in_core = state
        .model
        .zones
        .iter()
        .filter(|z| z.window.contains(z.handle.minute_of_day(cursor_instant)))
        .count();

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        format!(
            "\u{25c6} Cursor {} UTC \u{b7} {in_core}/{total} core",
            cursor_instant.format("%H:%M")
        ),
        palette.style(Role::Heading),
    )));

    // UTC reference row.
    let utc_label_style = if state.selected_zone == 0 {
        palette.style(Role::Selected)
    } else {
        Style::new()
    };
    lines.push(Line::from(vec![
        Span::styled(format!("{:<18}", "UTC"), utc_label_style),
        Span::styled(
            format!(" {}", cursor_instant.format("%H:%M")),
            Style::new().bold(),
        ),
    ]));

    for (display_idx, &model_idx) in state.display_order.iter().enumerate() {
        let zone = &state.model.zones[model_idx];
        let local = zone.handle.local_time(cursor_instant);
        let m = zone.handle.minute_of_day(cursor_instant);
        let (badge, badge_role) = match ribbon::classify(m, &zone.window, shoulder_minutes) {
            RibbonState::Core => ("core", Role::Good),
            RibbonState::Shoulder => ("shldr", Role::Caution),
            RibbonState::Off => ("off", Role::Muted),
        };
        let is_selected = state.selected_zone == display_idx + 1;
        let label = fit_label(&zone.label, 18);
        let label_style = if is_selected {
            palette.style(Role::Selected)
        } else {
            Style::new()
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{label:<18}"), label_style),
            Span::styled(format!(" {} ", local.format("%H:%M")), Style::new().bold()),
            Span::styled(badge.to_string(), palette.style(badge_role)),
        ]));
    }
    lines
}

fn render_controls(buffer: &mut Buffer, area: Rect, state: &AppState, palette: &Palette) {
    let spans = controls_spans(state, palette);

    if area.height >= 2 {
        let total_chars: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        let char_budget = (area.width as usize) * 2;
        let final_spans = if total_chars > char_budget {
            truncate_spans_with_ellipsis(&spans, char_budget, palette)
        } else {
            spans
        };
        Paragraph::new(Line::from(final_spans))
            .wrap(Wrap { trim: true })
            .render(area, buffer);
    } else {
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

fn render_help(buffer: &mut Buffer, area: Rect, palette: &Palette) {
    let key_style = palette.style(Role::KeyHint);
    let desc_style = palette.style(Role::Label);

    let key_col: usize = 14; // width for the key column
    let indent = "  ";

    let mut lines: Vec<Line> = Vec::new();

    // --- Navigation ---
    lines.push(Line::from(Span::styled(
        " Navigation",
        palette.style(Role::Heading),
    )));
    lines.push(Line::from(vec![
        Span::raw(indent),
        Span::styled(format!("{:<key_col$}", "\u{2190}/\u{2192}  h/l"), key_style),
        Span::styled("Move cursor (\u{00bd} hour)", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::raw(indent),
        Span::styled(format!("{:<key_col$}", "\u{2191}/\u{2193}  j/k"), key_style),
        Span::styled("Move zone cursor", desc_style),
    ]));
    lines.push(Line::from(vec![
        Span::raw(indent),
        Span::styled(format!("{:<key_col$}", "n"), key_style),
        Span::styled("Jump cursor to now", desc_style),
    ]));
    lines.push(Line::from(""));

    // --- Zone Management ---
    lines.push(Line::from(Span::styled(
        " Zone Management",
        palette.style(Role::Heading),
    )));
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
    lines.push(Line::from(Span::styled(
        " General",
        palette.style(Role::Heading),
    )));
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
    lines.push(Line::from(Span::styled(
        "                          Esc or ? to close ",
        palette.style(Role::Muted),
    )));

    let content_height = lines.len() as u16 + 2; // +2 for borders
    let popup = centered_rect(area, 48, content_height);
    Clear.render(popup, buffer);
    Paragraph::new(lines)
        .block(Block::bordered().title(Span::styled(" Help ", palette.style(Role::Heading))))
        .render(popup, buffer);
}

fn render_modal(buffer: &mut Buffer, area: Rect, state: &AppState, palette: &Palette) {
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
            palette,
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
            palette,
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
    palette: &Palette,
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
        (palette.style(Role::Heading), palette.style(Role::Muted));
    let (active_highlight, inactive_highlight) = (
        palette.style(Role::SelectedActive),
        palette.style(Role::Selected),
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
        Paragraph::new(Span::styled(summary, palette.style(Role::Caution)))
            .render(summary_area, buffer);
    }

    // Hints line
    let hints_y = summary_y + 1;
    if hints_y < inner.y + inner.height {
        let hints_area = Rect::new(inner.x + 1, hints_y, inner.width.saturating_sub(2), 1);
        Paragraph::new(Line::from(vec![
            Span::styled("Tab", palette.style(Role::KeyHint)),
            Span::styled(" switch pane  ", palette.style(Role::Muted)),
            Span::styled("\u{2191}\u{2193}/jk", palette.style(Role::KeyHint)),
            Span::styled(" scroll  ", palette.style(Role::Muted)),
            Span::styled("Enter", palette.style(Role::KeyHint)),
            Span::styled(" submit  ", palette.style(Role::Muted)),
            Span::styled("Esc", palette.style(Role::KeyHint)),
            Span::styled(" cancel", palette.style(Role::Muted)),
        ]))
        .render(hints_area, buffer);
    }
}

#[allow(clippy::too_many_arguments)]
fn render_add_zone_picker(
    buffer: &mut Buffer,
    area: Rect,
    input: &str,
    entries: &[crate::tui::forms::ZonePickerEntry],
    filtered: &[usize],
    selected: usize,
    mut scroll_offset: usize,
    palette: &Palette,
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
        Span::styled("> ", palette.style(Role::KeyHint)),
        Span::raw(input),
        Span::styled(
            if placeholder.is_empty() {
                "_"
            } else {
                placeholder
            },
            palette.style(Role::Muted),
        ),
    ]))
    .render(filter_area, buffer);

    // Separator line
    let sep_area = Rect::new(inner.x, inner.y + 1, inner.width, 1);
    let sep: String = "─".repeat(inner.width as usize);
    Paragraph::new(Span::styled(&*sep, palette.style(Role::Muted))).render(sep_area, buffer);

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
            palette.style(Role::SelectedActive)
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
            Span::styled(
                format!("{match_count} matches  "),
                palette.style(Role::Muted),
            ),
            Span::styled(
                "↑↓ navigate  Enter select  Esc cancel",
                palette.style(Role::Muted),
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

fn panel_block(title: &'static str, focused: bool, palette: &Palette) -> Block<'static> {
    let style = if focused {
        palette.style(Role::Heading)
    } else {
        Style::new()
    };
    Block::bordered().title(Span::styled(title, style))
}
