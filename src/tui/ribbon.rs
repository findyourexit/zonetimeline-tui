//! Pure rendering logic for the heat-ribbon timeline.
//!
//! No ratatui drawing here — only classification and layout math, so it can be
//! exhaustively unit-tested.

use crate::core::model::MinuteClass;
use crate::core::windows::WorkWindow;

/// Per-instant availability state of a single zone, for ribbon colouring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RibbonState {
    /// Inside the work window.
    Core,
    /// Within shoulder hours of the window.
    Shoulder,
    /// Outside the window and its shoulder (off-hours / asleep).
    Off,
}

impl RibbonState {
    /// Stable index for the three states: `Core` = 0, `Shoulder` = 1, `Off` = 2.
    /// Keys per-state arrays such as a blended column's sub-sample counts.
    pub const fn index(self) -> usize {
        match self {
            RibbonState::Core => 0,
            RibbonState::Shoulder => 1,
            RibbonState::Off => 2,
        }
    }
}

/// Classify a local minute-of-day against a zone's window + shoulder.
///
/// Returns [`RibbonState::Core`] inside the window, [`RibbonState::Shoulder`]
/// within `shoulder_minutes` of it, and [`RibbonState::Off`] everywhere else.
pub fn classify(minute_of_day: u16, window: &WorkWindow, shoulder_minutes: u16) -> RibbonState {
    if window.contains(minute_of_day) {
        RibbonState::Core
    } else if window.shoulder_contains(minute_of_day, shoulder_minutes) {
        RibbonState::Shoulder
    } else {
        RibbonState::Off
    }
}

/// Aggregate per-column zone counts into the model's `MinuteClass` tiers.
/// Thin wrapper over [`crate::core::model::classify_reach`] — the single source
/// of truth — so the ribbon and the minute bitmap never disagree.
pub fn overlap_class(in_count: usize, reach_count: usize, total: usize) -> MinuteClass {
    crate::core::model::classify_reach(in_count, reach_count, total)
}

/// Sub-samples taken per column-width when averaging state coverage. Higher
/// values give a smoother colour ramp across a transition.
pub const SUBCELLS: usize = 8;

/// Per-column state coverage, sampled over a window that overlaps `radius`
/// columns into each neighbour so a work-window boundary renders as a
/// multi-cell colour gradient rather than a hard step.
///
/// Returns per-column counts indexed by [`RibbonState::index`] (Core=0,
/// Shoulder=1, Off=2); every column's counts sum to the same total. Columns far
/// from any boundary sample a single state and stay pure; only columns within
/// `radius` of a boundary pick up a blend, so solid bands stay crisp. `state_at`
/// maps a minute offset from the start of the range to a [`RibbonState`] and may
/// be queried just outside `0..total_minutes`.
pub fn column_coverage<F>(
    width: usize,
    total_minutes: i64,
    radius: f64,
    mut state_at: F,
) -> Vec<[u16; 3]>
where
    F: FnMut(i64) -> RibbonState,
{
    if width == 0 || total_minutes <= 0 {
        return Vec::new();
    }
    let m_per_col = total_minutes as f64 / width as f64;
    // Half the sampling window, in minutes: the column itself plus `radius`
    // columns of overlap on each side.
    let half = (radius + 0.5) * m_per_col;
    let step = m_per_col / SUBCELLS as f64;
    let n = ((2.0 * half / step).round() as i64).max(1);
    (0..width)
        .map(|c| {
            let center = (c as f64 + 0.5) * m_per_col;
            let mut counts = [0u16; 3];
            for s in 0..n {
                let frac = if n == 1 {
                    0.5
                } else {
                    s as f64 / (n - 1) as f64
                };
                let minute = center - half + frac * (2.0 * half);
                counts[state_at(minute.round() as i64).index()] += 1;
            }
            counts
        })
        .collect()
}

/// Whole-hour axis ticks for a visible range that may begin mid-hour.
///
/// Returns `(minute_offset_from_start, hour)` for every whole UTC hour inside
/// `0..total_minutes` whose hour is a multiple of `label_every`. Labelling true
/// hours — rather than evenly-spaced grid slots — keeps the axis aligned with
/// the ribbon's work-window boundaries even when the grid starts mid-hour for an
/// explicit `--time HH:MM` anchor (a live "now" anchor snaps to the top of the
/// hour; an explicit 13:30 would otherwise label its slot "13" and skew every
/// boundary half an hour).
pub fn hour_ticks(
    start_hour: u32,
    start_minute: u32,
    total_minutes: i64,
    label_every: usize,
) -> Vec<(i64, u32)> {
    let label_every = label_every.max(1) as i64;
    // Minutes from the start until the first whole hour at/after it.
    let first = (60 - i64::from(start_minute)) % 60;
    let mut ticks = Vec::new();
    let mut off = first;
    while off < total_minutes {
        let hours_since = (i64::from(start_minute) + off) / 60;
        let hour = (i64::from(start_hour) + hours_since).rem_euclid(24);
        if hour % label_every == 0 {
            ticks.push((off, hour as u32));
        }
        off += 60;
    }
    ticks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(s: u16, e: u16) -> WorkWindow {
        WorkWindow {
            start_minute: s,
            end_minute: e,
        }
    }

    #[test]
    fn core_inside_window() {
        assert_eq!(classify(600, &win(540, 1020), 60), RibbonState::Core);
    }

    #[test]
    fn shoulder_before_and_after() {
        assert_eq!(classify(490, &win(540, 1020), 60), RibbonState::Shoulder);
        assert_eq!(classify(1070, &win(540, 1020), 60), RibbonState::Shoulder);
    }

    #[test]
    fn off_before_and_after_work() {
        assert_eq!(classify(420, &win(540, 1020), 60), RibbonState::Off); // 07:00
        assert_eq!(classify(1140, &win(540, 1020), 60), RibbonState::Off); // 19:00
    }

    #[test]
    fn overnight_window_off_outside() {
        // window 22:00-06:00 -> off gap [360,1320)
        let w = win(1320, 360);
        assert_eq!(classify(1380, &w, 0), RibbonState::Core); // 23:00 inside
        assert_eq!(classify(600, &w, 0), RibbonState::Off); // 10:00 off
        assert_eq!(classify(1000, &w, 0), RibbonState::Off); // 16:40 off
    }

    #[test]
    fn overlap_all_core_is_ideal() {
        assert_eq!(overlap_class(6, 6, 6), MinuteClass::Ideal);
    }

    #[test]
    fn overlap_all_reachable_is_feasible() {
        assert_eq!(overlap_class(4, 6, 6), MinuteClass::Feasible);
    }

    #[test]
    fn overlap_some_reachable_is_partial() {
        assert_eq!(overlap_class(1, 3, 6), MinuteClass::Partial(3));
    }

    #[test]
    fn overlap_none_reachable_is_none() {
        assert_eq!(overlap_class(0, 0, 6), MinuteClass::None);
    }

    #[test]
    fn column_coverage_pure_when_uniform() {
        let cov = column_coverage(100, 1440, 1.0, |_| RibbonState::Core);
        assert_eq!(cov.len(), 100);
        // Core=0; every column is entirely core, nothing in shoulder/off.
        assert!(cov.iter().all(|c| c[0] > 0 && c[1] == 0 && c[2] == 0));
    }

    #[test]
    fn column_coverage_is_pure_far_from_boundaries() {
        // Off for the first half, Core for the second.
        let cov = column_coverage(100, 6000, 1.0, |m| {
            if m < 3000 {
                RibbonState::Off
            } else {
                RibbonState::Core
            }
        });
        // The far-left column is entirely Off, the far-right entirely Core.
        assert!(cov[0][0] == 0 && cov[0][1] == 0 && cov[0][2] > 0);
        let last = cov.len() - 1;
        assert!(cov[last][0] > 0 && cov[last][1] == 0 && cov[last][2] == 0);
    }

    #[test]
    fn column_coverage_blends_monotonically_across_a_boundary() {
        // Off → Core at the midpoint ramps smoothly: core coverage never drops
        // left→right, and at least one column is a genuine two-state blend.
        let cov = column_coverage(60, 6000, 1.0, |m| {
            if m < 3000 {
                RibbonState::Off
            } else {
                RibbonState::Core
            }
        });
        let core: Vec<u16> = cov.iter().map(|c| c[0]).collect();
        assert!(
            core.windows(2).all(|w| w[0] <= w[1]),
            "core coverage must be non-decreasing across the boundary"
        );
        assert!(
            cov.iter().any(|c| c[0] > 0 && c[2] > 0),
            "expected a blended transition column"
        );
    }

    #[test]
    fn column_coverage_empty_for_zero_width() {
        assert!(column_coverage(0, 120, 1.0, |_| RibbonState::Core).is_empty());
    }

    #[test]
    fn hour_ticks_label_true_hours_when_grid_starts_on_the_hour() {
        // 24h from 00:00, every 2nd hour: offsets are exact multiples of 120.
        let ticks = hour_ticks(0, 0, 1440, 2);
        assert_eq!(ticks.first(), Some(&(0, 0)));
        assert_eq!(ticks[1], (120, 2));
        assert!(ticks.iter().all(|(off, h)| off % 120 == 0 && h % 2 == 0));
    }

    #[test]
    fn hour_ticks_track_whole_hours_when_grid_starts_mid_hour() {
        // Grid begins 00:30 (an explicit --time 00:30 anchor). The first whole
        // hour (01:00) is 30 minutes in, and every label sits a true hour later.
        let ticks = hour_ticks(0, 30, 120, 1);
        assert_eq!(ticks, vec![(30, 1), (90, 2)]);
    }

    #[test]
    fn hour_ticks_wrap_past_midnight() {
        // Start 23:30, 2 hours: whole hours at 00:00 (+30) and 01:00 (+90).
        let ticks = hour_ticks(23, 30, 120, 1);
        assert_eq!(ticks, vec![(30, 0), (90, 1)]);
    }
}
