//! A braille sub-cell canvas.
//!
//! Each terminal cell holds a 2×4 grid of dots, so a `cols × rows` cell area
//! addresses a `2·cols × 4·rows` monochrome bitmap. The coastline is drawn into
//! this bitmap and flushed as Unicode braille glyphs, giving a fine outline that
//! reflows with the terminal — the same technique the overlap histogram uses.

use crate::tui::map::projection::{lat_to_norm, lon_to_norm};

/// Braille bit for each `(col, row)` sub-cell (col ∈ 0..2, row ∈ 0..4).
///
/// Braille dot numbering is `1 4 / 2 5 / 3 6 / 7 8`, so the left column carries
/// dots 1,2,3,7 and the right column dots 4,5,6,8.
const DOT_BITS: [[u8; 4]; 2] = [[0x01, 0x02, 0x04, 0x40], [0x08, 0x10, 0x20, 0x80]];

/// A monochrome braille bitmap sized to a terminal cell area.
#[derive(Debug, Clone)]
pub struct BrailleCanvas {
    cols: u16,
    rows: u16,
    cells: Vec<u8>,
}

impl BrailleCanvas {
    /// Allocate a blank canvas covering `cols × rows` terminal cells.
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            cells: vec![0u8; cols as usize * rows as usize],
        }
    }

    /// Canvas width in dots (`2 · cols`).
    pub fn dots_w(&self) -> u32 {
        self.cols as u32 * 2
    }

    /// Canvas height in dots (`4 · rows`).
    pub fn dots_h(&self) -> u32 {
        self.rows as u32 * 4
    }

    /// Set the dot at absolute dot coordinates `(dx, dy)`, origin top-left.
    /// Out-of-bounds coordinates are silently ignored.
    pub fn set_dot(&mut self, dx: i32, dy: i32) {
        if dx < 0 || dy < 0 {
            return;
        }
        let (dx, dy) = (dx as u32, dy as u32);
        if dx >= self.dots_w() || dy >= self.dots_h() {
            return;
        }
        let cx = (dx / 2) as usize;
        let cy = (dy / 4) as usize;
        let bit = DOT_BITS[(dx % 2) as usize][(dy % 4) as usize];
        self.cells[cy * self.cols as usize + cx] |= bit;
    }

    /// Rasterize a straight line between two dot coordinates (Bresenham).
    pub fn stroke(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) {
        let mut x0 = x0.round() as i32;
        let mut y0 = y0.round() as i32;
        let x1 = x1.round() as i32;
        let y1 = y1.round() as i32;
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.set_dot(x0, y0);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    /// Project a `(lon, lat)` degree pair to fractional dot coordinates.
    pub fn project_dot(&self, lon: f64, lat: f64) -> (f64, f64) {
        let x = lon_to_norm(lon) * (self.dots_w() as f64 - 1.0);
        let y = lat_to_norm(lat) * (self.dots_h() as f64 - 1.0);
        (x, y)
    }

    /// Stroke a geographic segment, skipping segments that wrap the antimeridian
    /// (endpoints more than half a world apart in longitude) so they don't smear
    /// a horizontal line straight across the map.
    pub fn stroke_geo(&mut self, lon0: f64, lat0: f64, lon1: f64, lat1: f64) {
        if (lon_to_norm(lon0) - lon_to_norm(lon1)).abs() > 0.5 {
            return;
        }
        let (x0, y0) = self.project_dot(lon0, lat0);
        let (x1, y1) = self.project_dot(lon1, lat1);
        self.stroke(x0, y0, x1, y1);
    }

    /// The braille glyph for cell `(cx, cy)`, or `None` if no dots are set.
    pub fn glyph(&self, cx: u16, cy: u16) -> Option<char> {
        if cx >= self.cols || cy >= self.rows {
            return None;
        }
        let mask = self.cells[cy as usize * self.cols as usize + cx as usize];
        if mask == 0 {
            None
        } else {
            char::from_u32(0x2800 + mask as u32)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_dot_becomes_the_expected_braille_glyph() {
        let mut c = BrailleCanvas::new(1, 1);
        c.set_dot(0, 0); // top-left = dot 1 = 0x01
        assert_eq!(c.glyph(0, 0), Some('\u{2801}'));
    }

    #[test]
    fn all_eight_dots_form_a_full_cell() {
        let mut c = BrailleCanvas::new(1, 1);
        for dy in 0..4 {
            for dx in 0..2 {
                c.set_dot(dx, dy);
            }
        }
        assert_eq!(c.glyph(0, 0), Some('\u{28FF}'));
    }

    #[test]
    fn empty_cells_have_no_glyph() {
        let c = BrailleCanvas::new(2, 2);
        assert_eq!(c.glyph(0, 0), None);
        assert_eq!(c.glyph(5, 5), None); // out of bounds
    }

    #[test]
    fn out_of_bounds_dots_are_ignored() {
        let mut c = BrailleCanvas::new(1, 1);
        c.set_dot(-1, 0);
        c.set_dot(0, -1);
        c.set_dot(100, 0);
        c.set_dot(0, 100);
        assert_eq!(c.glyph(0, 0), None);
    }

    #[test]
    fn stroke_draws_a_connected_line() {
        let mut c = BrailleCanvas::new(4, 1); // 8 dots wide
        c.stroke(0.0, 0.0, 7.0, 0.0);
        // Every cell along the top row should light up.
        for cx in 0..4 {
            assert!(c.glyph(cx, 0).is_some(), "cell {cx} should be set");
        }
    }

    #[test]
    fn antimeridian_segments_are_skipped() {
        let mut c = BrailleCanvas::new(40, 10);
        c.stroke_geo(179.0, 0.0, -179.0, 0.0);
        let lit = (0..40).any(|cx| c.glyph(cx, 5).is_some() || c.glyph(cx, 4).is_some());
        assert!(
            !lit,
            "a segment wrapping the antimeridian must not be drawn"
        );
    }
}
