//! Work-window definitions and shoulder-hour logic.
//!
//! A [`WorkWindow`] represents the daily time range during which a participant
//! is available, specified as `HH:MM-HH:MM`.  Windows that wrap past midnight
//! (e.g. `22:00-06:00`) are supported.  Shoulder hours extend the range by a
//! configurable number of minutes on each side for "feasible but not ideal"
//! scheduling.

use thiserror::Error;

/// Total minutes in a day (24 × 60 = 1440).
const MINUTES_PER_DAY: u16 = 24 * 60;

/// A daily work window defined by start and end minutes-of-day.
///
/// Both bounds are in `0..1440`.  If `start_minute > end_minute` the window
/// wraps past midnight (e.g. 22:00-06:00 → 1320..360).  If they are equal
/// the window covers the full 24 hours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkWindow {
    /// Start of the work window as minutes since midnight (0-1439).
    pub start_minute: u16,
    /// End of the work window as minutes since midnight (0-1439).
    pub end_minute: u16,
}

/// Errors produced when a work-window string cannot be parsed.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WindowError {
    #[error("invalid work window: {0}")]
    Invalid(String),
}

impl WorkWindow {
    /// Parse a `"HH:MM-HH:MM"` string into a [`WorkWindow`].
    pub fn parse(input: &str) -> Result<Self, WindowError> {
        let Some((start, end)) = input.trim().split_once('-') else {
            return Err(WindowError::Invalid(input.to_string()));
        };

        Ok(Self {
            start_minute: parse_time(start, input)?,
            end_minute: parse_time(end, input)?,
        })
    }

    /// Returns `true` if `minute_of_day` falls inside this work window.
    ///
    /// Handles both normal (start < end) and wrap-around (start > end) windows.
    /// When start == end the window is treated as 24h and always returns `true`.
    pub fn contains(&self, minute_of_day: u16) -> bool {
        if self.start_minute == self.end_minute {
            return true;
        }

        if self.start_minute < self.end_minute {
            (self.start_minute..self.end_minute).contains(&minute_of_day)
        } else {
            minute_of_day >= self.start_minute || minute_of_day < self.end_minute
        }
    }

    /// Returns `true` if `minute_of_day` is within `shoulder_minutes` of the
    /// window's start or end, but NOT inside the window itself.
    /// Handles wrap-around windows (e.g. 22:00-06:00).
    pub fn shoulder_contains(&self, minute_of_day: u16, shoulder_minutes: u16) -> bool {
        if shoulder_minutes == 0 || self.contains(minute_of_day) {
            return false;
        }

        let m = minute_of_day as i32;
        let start = self.start_minute as i32;
        let end = self.end_minute as i32;
        let sh = shoulder_minutes as i32;
        let day = MINUTES_PER_DAY as i32;

        // Distance before start (circular)
        let before_start = ((start - m) % day + day) % day;
        // Distance after end (circular)
        let after_end = ((m - end) % day + day) % day;

        (before_start > 0 && before_start <= sh) || (after_end > 0 && after_end <= sh)
    }
}

/// Parse a `"HH:MM"` time string into minutes since midnight.
fn parse_time(input: &str, whole: &str) -> Result<u16, WindowError> {
    let Some((hours, minutes)) = input.trim().split_once(':') else {
        return Err(WindowError::Invalid(whole.to_string()));
    };

    let Ok(hours) = hours.parse::<u16>() else {
        return Err(WindowError::Invalid(whole.to_string()));
    };
    let Ok(minutes) = minutes.parse::<u16>() else {
        return Err(WindowError::Invalid(whole.to_string()));
    };

    if hours >= 24 || minutes >= 60 {
        return Err(WindowError::Invalid(whole.to_string()));
    }

    let minute = hours * 60 + minutes;
    if minute >= MINUTES_PER_DAY {
        return Err(WindowError::Invalid(whole.to_string()));
    }

    Ok(minute)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Normal window 09:00-17:00 (540-1020)
    fn normal_window() -> WorkWindow {
        WorkWindow {
            start_minute: 540,
            end_minute: 1020,
        }
    }

    // Wraparound window 22:00-06:00 (1320-360)
    fn wrap_window() -> WorkWindow {
        WorkWindow {
            start_minute: 1320,
            end_minute: 360,
        }
    }

    #[test]
    fn shoulder_before_normal_window() {
        assert!(normal_window().shoulder_contains(480, 60));
    }

    #[test]
    fn shoulder_after_normal_window() {
        assert!(normal_window().shoulder_contains(1050, 60));
    }

    #[test]
    fn inside_window_is_not_shoulder() {
        assert!(!normal_window().shoulder_contains(720, 60));
    }

    #[test]
    fn outside_both_window_and_shoulder() {
        assert!(!normal_window().shoulder_contains(360, 60));
    }

    #[test]
    fn exact_shoulder_boundary_is_included() {
        assert!(normal_window().shoulder_contains(480, 60));
        assert!(!normal_window().shoulder_contains(479, 60));
    }

    #[test]
    fn zero_shoulder_means_no_shoulder() {
        assert!(!normal_window().shoulder_contains(480, 0));
        assert!(!normal_window().shoulder_contains(1050, 0));
    }

    #[test]
    fn shoulder_before_wrap_window() {
        assert!(wrap_window().shoulder_contains(1260, 60));
    }

    #[test]
    fn shoulder_after_wrap_window() {
        assert!(wrap_window().shoulder_contains(390, 60));
    }

    #[test]
    fn inside_wrap_window_is_not_shoulder() {
        assert!(!wrap_window().shoulder_contains(1380, 60));
        assert!(!wrap_window().shoulder_contains(180, 60));
    }

    #[test]
    fn outside_wrap_window_and_shoulder() {
        assert!(!wrap_window().shoulder_contains(720, 60));
    }

    #[test]
    fn shoulder_with_large_width() {
        assert!(normal_window().shoulder_contains(420, 120));
        assert!(!normal_window().shoulder_contains(419, 120));
    }
}
