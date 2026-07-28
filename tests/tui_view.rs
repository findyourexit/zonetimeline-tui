//! Rendering tests for the heat-ribbon timeline view.
//!
//! Layout/structure is locked with `insta` symbol snapshots (palette-agnostic,
//! deterministic via an explicit anchor); colour wiring that snapshots can't
//! see is asserted directly against an injected truecolor palette.

mod support;

use chrono::NaiveTime;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use zonetimeline_tui::core::model::{ComparisonModel, ViewMode};
use zonetimeline_tui::tui::palette::{Capability, Palette, Role};
use zonetimeline_tui::tui::state::AppState;
use zonetimeline_tui::tui::view::render_to_buffer_with_palette;

/// A deterministic 6-zone, globe-spanning state with an explicit anchor (so no
/// live "now" leaks into the render).
fn fixed_state(nhours: u16) -> AppState {
    let mut seed = support::fixture_seed();
    seed.nhours = nhours;
    seed.anchor_time = Some(NaiveTime::from_hms_opt(12, 0, 0).unwrap());
    seed.ordered_zones = vec![
        "America/Los_Angeles".to_string(),
        "America/New_York".to_string(),
        "Europe/London".to_string(),
        "Europe/Berlin".to_string(),
        "Asia/Kolkata".to_string(),
        "Australia/Sydney".to_string(),
    ];
    seed.base_zones = seed.ordered_zones.clone();
    let model = ComparisonModel::build(seed, support::fixed_now()).unwrap();
    AppState::new(model, support::fixed_now())
}

/// Render into a fixed buffer and return the symbol grid.
///
/// Pinned to the monochrome palette so the structural snapshot is deterministic
/// across environments and renders the ribbon *shape* via shade characters
/// (█/▓/░); colour wiring is asserted separately against a truecolor palette.
fn render_symbols(state: &AppState, w: u16, h: u16) -> String {
    let area = Rect::new(0, 0, w, h);
    let mut buf = Buffer::empty(area);
    render_to_buffer_with_palette(&mut buf, area, state, &Palette::new(Capability::Monochrome));
    let mut out = String::new();
    for y in 0..h {
        for x in 0..w {
            out.push_str(buf.cell((x, y)).unwrap().symbol());
        }
        out.push('\n');
    }
    out
}

#[test]
fn normal_layout_snapshot() {
    let state = fixed_state(24);
    insta::assert_snapshot!(render_symbols(&state, 120, 28));
}

#[test]
fn ribbon_core_cell_uses_palette_bg_in_truecolor() {
    // Over a 24h window, at least one globe-spanning zone is in core hours, so
    // the ribbon must contain at least one cell painted with the core bg colour.
    let state = fixed_state(24);
    let area = Rect::new(0, 0, 120, 28);
    let mut buf = Buffer::empty(area);
    let palette = Palette::new(Capability::Truecolor);
    render_to_buffer_with_palette(&mut buf, area, &state, &palette);

    let core_bg = palette.style(Role::Core).bg.expect("core role has a bg");
    let found =
        (0..area.width).any(|x| (0..area.height).any(|y| buf.cell((x, y)).unwrap().bg == core_bg));
    assert!(found, "expected at least one core-coloured ribbon cell");
}

#[test]
fn narrow_layout_snapshot() {
    // A 36-hour timeline at the 80-column floor: the ribbon scales down (more
    // time per column, half-blocks for sub-column boundaries).
    let state = fixed_state(36);
    insta::assert_snapshot!(render_symbols(&state, 80, 28));
}

#[test]
fn wide_layout_snapshot() {
    // A 24-hour timeline on a wide pane: the ribbon stretches to fill the width.
    let state = fixed_state(24);
    insta::assert_snapshot!(render_symbols(&state, 200, 30));
}

#[test]
fn ribbon_fills_full_width() {
    // The ribbon must stretch across the pane, not sit at a fixed ~2-char/hour
    // width: on a 200-col pane a far-right column still carries a ribbon fill.
    let state = fixed_state(24);
    let area = Rect::new(0, 0, 200, 30);
    let mut buf = Buffer::empty(area);
    let palette = Palette::new(Capability::Truecolor);
    render_to_buffer_with_palette(&mut buf, area, &state, &palette);

    let fills = [
        palette.style(Role::Core).bg,
        palette.style(Role::Shoulder).bg,
        palette.style(Role::Off).bg,
    ];
    let x = area.width - 4; // near the right edge, inside the border
    let has_fill = (0..area.height).any(|y| fills.contains(&Some(buf.cell((x, y)).unwrap().bg)));
    assert!(
        has_fill,
        "ribbon should fill to the right edge of a wide pane"
    );
}

#[test]
fn renders_in_monochrome_without_panic() {
    let state = fixed_state(24);
    let area = Rect::new(0, 0, 120, 28);
    let mut buf = Buffer::empty(area);
    render_to_buffer_with_palette(
        &mut buf,
        area,
        &state,
        &Palette::new(Capability::Monochrome),
    );
    let text: String = (0..area.height)
        .flat_map(|y| (0..area.width).map(move |x| (x, y)))
        .map(|(x, y)| buf.cell((x, y)).unwrap().symbol().to_string())
        .collect();
    assert!(text.contains("Zone Timeline"));
    assert!(
        ['█', '▓', '▒', '░'].iter().any(|c| text.contains(*c)),
        "monochrome ribbons should use shade characters"
    );
}

#[test]
fn renders_in_ansi16_without_panic() {
    let state = fixed_state(24);
    let area = Rect::new(0, 0, 120, 28);
    let mut buf = Buffer::empty(area);
    let palette = Palette::new(Capability::Ansi16);
    render_to_buffer_with_palette(&mut buf, area, &state, &palette);
    let core_bg = palette.style(Role::Core).bg.expect("core role has a bg");
    let found =
        (0..area.width).any(|x| (0..area.height).any(|y| buf.cell((x, y)).unwrap().bg == core_bg));
    assert!(
        found,
        "ansi16 ribbons should paint core cells with the named green bg"
    );
}

/// Symbols of each rendered row for a given palette (whole buffer, one row per
/// string) — used to assert glyph-level behaviour the mono snapshot can't see.
fn rendered_rows(cap: Capability, w: u16, h: u16) -> Vec<String> {
    let state = fixed_state(24);
    let area = Rect::new(0, 0, w, h);
    let mut buf = Buffer::empty(area);
    render_to_buffer_with_palette(&mut buf, area, &state, &Palette::new(cap));
    (0..h)
        .map(|y| (0..w).map(|x| buf.cell((x, y)).unwrap().symbol()).collect())
        .collect()
}

#[test]
fn truecolor_ribbon_blends_across_boundaries() {
    use ratatui::style::Color;
    let state = fixed_state(24);
    let area = Rect::new(0, 0, 120, 28);
    let mut buf = Buffer::empty(area);
    render_to_buffer_with_palette(&mut buf, area, &state, &Palette::new(Capability::Truecolor));

    // Ribbons are flat colour fills — never half/eighth-block glyphs.
    let mut syms = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            syms.push_str(buf.cell((x, y)).unwrap().symbol());
        }
    }
    for g in ['▀', '▄', '▏', '▎', '▍', '▌', '▋', '▊', '▉'] {
        assert!(!syms.contains(g), "ribbon must not use block glyph {g:?}");
    }

    // A coverage blend yields many intermediate background colours — far more
    // than the handful of pure state/reference fills — i.e. a real gradient.
    let mut bgs = std::collections::BTreeSet::new();
    for y in 0..area.height {
        for x in 0..area.width {
            if let Color::Rgb(r, g, b) = buf.cell((x, y)).unwrap().bg {
                bgs.insert((r, g, b));
            }
        }
    }
    assert!(
        bgs.len() > 8,
        "expected many blended ribbon fills (a gradient), got {}",
        bgs.len()
    );
}

#[test]
fn overlap_strip_is_a_braille_histogram() {
    let rows = rendered_rows(Capability::Truecolor, 120, 28);
    let overlap = rows
        .iter()
        .find(|r| r.contains("Overlap"))
        .expect("overlap row present");
    // Dot height encodes reach, so a globe-spanning day must show >1 height.
    let heights: std::collections::BTreeSet<char> = overlap
        .chars()
        .filter(|c| ('\u{2800}'..='\u{28ff}').contains(c))
        .collect();
    assert!(
        heights.len() >= 2,
        "braille overlap histogram should vary in dot height, got {heights:?}"
    );
}

/// Assert no cell carries a fg/bg colour: monochrome conveys everything through
/// shade characters and modifiers (bold/dim/reverse) only, honouring NO_COLOR.
fn assert_no_colour(state: &AppState, w: u16, h: u16) {
    use ratatui::style::Color;
    let area = Rect::new(0, 0, w, h);
    let mut buf = Buffer::empty(area);
    render_to_buffer_with_palette(&mut buf, area, state, &Palette::new(Capability::Monochrome));
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = buf.cell((x, y)).unwrap();
            assert_eq!(
                cell.fg,
                Color::Reset,
                "cell ({x},{y}) {:?} set a fg in monochrome",
                cell.symbol()
            );
            assert_eq!(
                cell.bg,
                Color::Reset,
                "cell ({x},{y}) {:?} set a bg in monochrome",
                cell.symbol()
            );
        }
    }
}

#[test]
fn monochrome_render_emits_no_colour() {
    // The palette is the single owner of colour: the entire base render (header,
    // ribbon, footer, controls) must be colour-free in monochrome.
    assert_no_colour(&fixed_state(24), 120, 28);
}

#[test]
fn monochrome_overlays_emit_no_colour() {
    // The help overlay and both modals route through the palette too, so they
    // must render colour-free in monochrome as well.
    let mut help = fixed_state(24);
    help.show_help = true;
    assert_no_colour(&help, 120, 28);

    let mut add = fixed_state(24);
    add.open_add_zone();
    assert_no_colour(&add, 120, 28);

    let mut edit = fixed_state(24);
    edit.selected_zone = 1;
    edit.open_edit_window();
    assert_no_colour(&edit, 120, 28);
}

// ============================ World map view =============================
//
// The braille coastline is projected with trig, whose last-ULP results can
// differ across platforms and shift a dot into a neighbouring cell — so a
// pixel-exact snapshot would be flaky in CI. These tests assert the render's
// observable contract instead: the frame, a marker per zone, the day/night
// terminator, availability colour wiring, and monochrome colour-safety.

/// The 6-zone fixed state, switched to the map view.
fn fixed_map_state() -> AppState {
    let mut state = fixed_state(24);
    state.view = ViewMode::Map;
    state
}

/// Render the map view for a palette and return one string per row.
fn map_rows(cap: Capability, w: u16, h: u16) -> Vec<String> {
    let state = fixed_map_state();
    let area = Rect::new(0, 0, w, h);
    let mut buf = Buffer::empty(area);
    render_to_buffer_with_palette(&mut buf, area, &state, &Palette::new(cap));
    (0..h)
        .map(|y| (0..w).map(|x| buf.cell((x, y)).unwrap().symbol()).collect())
        .collect()
}

#[test]
fn map_view_draws_frame_markers_and_coastline() {
    let all = map_rows(Capability::Truecolor, 120, 28).join("\n");
    assert!(all.contains("World Map"), "map frame title present");
    assert!(all.contains('◉'), "selected zone uses the highlight marker");
    let markers = all
        .chars()
        .filter(|&c| matches!(c, '●' | '○' | '◉'))
        .count();
    assert!(
        markers >= 6,
        "expected a marker per zone (6 + UTC), got {markers}"
    );
    let braille = all
        .chars()
        .filter(|c| ('\u{2800}'..='\u{28ff}').contains(c))
        .count();
    assert!(braille > 100, "expected a braille coastline, got {braille}");
}

#[test]
fn map_view_shades_day_and_night_in_truecolor() {
    use ratatui::style::Color;
    let state = fixed_map_state();
    let area = Rect::new(0, 0, 120, 28);
    let mut buf = Buffer::empty(area);
    render_to_buffer_with_palette(&mut buf, area, &state, &Palette::new(Capability::Truecolor));
    let (mut day, mut night) = (false, false);
    for y in 0..area.height {
        for x in 0..area.width {
            match buf.cell((x, y)).unwrap().bg {
                Color::Rgb(0x14, 0x1d, 0x33) => day = true,
                Color::Rgb(0x07, 0x0a, 0x16) => night = true,
                _ => {}
            }
        }
    }
    assert!(day, "expected a lit day hemisphere");
    assert!(night, "expected a shaded night hemisphere");
}

#[test]
fn map_markers_carry_availability_colour_in_truecolor() {
    use ratatui::style::Color;
    let state = fixed_map_state();
    let area = Rect::new(0, 0, 120, 28);
    let mut buf = Buffer::empty(area);
    render_to_buffer_with_palette(&mut buf, area, &state, &Palette::new(Capability::Truecolor));
    // At 12:00 UTC, London/Berlin are inside core hours → green markers.
    let mut core_marker = false;
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = buf.cell((x, y)).unwrap();
            if matches!(cell.symbol(), "●" | "○" | "◉") && cell.fg == Color::Rgb(0x2e, 0xcc, 0x71)
            {
                core_marker = true;
            }
        }
    }
    assert!(
        core_marker,
        "expected at least one core (green) availability marker"
    );
}

#[test]
fn map_view_is_colour_free_in_monochrome() {
    assert_no_colour(&fixed_map_state(), 120, 28);
}

#[test]
fn map_view_renders_without_panic_across_palettes_and_sizes() {
    for cap in [
        Capability::Truecolor,
        Capability::Ansi16,
        Capability::Monochrome,
    ] {
        for (w, h) in [(80, 24), (120, 28), (200, 50)] {
            let _ = map_rows(cap, w, h);
        }
    }
}

#[test]
fn map_legend_labels_are_title_cased() {
    let all = map_rows(Capability::Truecolor, 120, 28).join("\n");
    for label in [
        "Core", "Shoulder", "Off", "Offset", "Selected", "Day", "Night",
    ] {
        assert!(
            all.contains(label),
            "legend should contain title-cased {label:?}"
        );
    }
    // The lowercase forms must be gone.
    assert!(
        !all.contains("offset"),
        "legend still shows lowercase 'offset'"
    );
    assert!(
        !all.contains("selected"),
        "legend still shows lowercase 'selected'"
    );
}

#[test]
fn map_tiles_horizontally_on_wide_terminals() {
    use ratatui::style::Color;
    let state = fixed_map_state();
    // A wide, short terminal has surplus width: the world tiles to fill it, so
    // there is no off-map void and the selected marker repeats across copies.
    let area = Rect::new(0, 0, 200, 24);
    let mut buf = Buffer::empty(area);
    render_to_buffer_with_palette(&mut buf, area, &state, &Palette::new(Capability::Truecolor));
    let void = Color::Rgb(0x04, 0x06, 0x0c);
    let void_cells = (0..area.height)
        .flat_map(|y| (0..area.width).map(move |x| (x, y)))
        .filter(|&(x, y)| buf.cell((x, y)).unwrap().bg == void)
        .count();
    assert_eq!(void_cells, 0, "a tiled wide map should leave no void");
    let selected = (0..area.height)
        .flat_map(|y| (0..area.width).map(move |x| (x, y)))
        .filter(|&(x, y)| buf.cell((x, y)).unwrap().symbol() == "◉")
        .count();
    assert!(
        selected >= 2,
        "the selected marker should repeat across tiles, got {selected}"
    );
}

#[test]
fn map_letterboxes_vertically_on_tall_terminals() {
    use ratatui::style::Color;
    let state = fixed_map_state();
    // A tall, narrow terminal has surplus height: no tiling — the single world
    // is centred vertically with an off-map void above and below.
    let area = Rect::new(0, 0, 90, 60);
    let mut buf = Buffer::empty(area);
    render_to_buffer_with_palette(&mut buf, area, &state, &Palette::new(Capability::Truecolor));
    let void = Color::Rgb(0x04, 0x06, 0x0c);
    let void_cells = (0..area.height)
        .flat_map(|y| (0..area.width).map(move |x| (x, y)))
        .filter(|&(x, y)| buf.cell((x, y)).unwrap().bg == void)
        .count();
    assert!(
        void_cells > 0,
        "a tall terminal should letterbox vertically"
    );
}
