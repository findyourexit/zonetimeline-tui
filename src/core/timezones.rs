//! Timezone parsing, resolution, and display utilities.
//!
//! Accepts IANA names (`America/New_York`), the special token `local`,
//! bare `UTC`/`GMT`, and UTC-offset strings like `UTC+5:30` or `GMT-4`.

use chrono::{DateTime, FixedOffset, Local, Offset, Timelike, Utc};
use chrono_tz::Tz;
use thiserror::Error;

/// An opaque handle to a resolved timezone — either an IANA name or a fixed offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneHandle {
    /// A named IANA timezone (e.g. `America/New_York`). DST-aware.
    Named(Tz),
    /// A fixed UTC offset (e.g. `UTC+5:30`). No DST transitions.
    Fixed(FixedOffset),
}

/// Errors produced when a timezone string cannot be resolved.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TimezoneError {
    #[error("unknown timezone: {0}")]
    Unknown(String),
}

impl ZoneHandle {
    /// Return a deduplication key that is stable across input aliases.
    ///
    /// Named zones key on the IANA identifier; fixed offsets key on their
    /// seconds-east-of-UTC value so that e.g. `UTC+0` and `GMT` collapse.
    pub fn identity_key(&self) -> String {
        match self {
            Self::Named(tz) => format!("named:{tz}"),
            Self::Fixed(offset) => format!("fixed:{}", offset.local_minus_utc()),
        }
    }

    /// Derive a human-friendly label from the raw input string.
    ///
    /// `"local"` resolves to `"<IANA name> (Local)"`, UTC-offset inputs are
    /// uppercased, and IANA region/city strings are passed through as-is.
    pub fn display_label(input: &str) -> String {
        let trimmed = input.trim();
        if trimmed.eq_ignore_ascii_case("utc") || trimmed.eq_ignore_ascii_case("gmt") {
            return trimmed.to_ascii_uppercase();
        }
        if trimmed.eq_ignore_ascii_case("local") {
            // Resolve the actual IANA timezone name and append "(Local)" suffix
            if let Ok(name) = iana_time_zone::get_timezone() {
                return format!("{name} (Local)");
            }
            // Fallback if IANA resolution fails
            return "Local Timezone".to_string();
        }
        if has_utc_prefix(trimmed) {
            return trimmed.to_ascii_uppercase();
        }
        trimmed.to_string()
    }

    /// Convert a UTC instant to local time in this zone.
    pub fn local_time(&self, anchor: DateTime<Utc>) -> DateTime<FixedOffset> {
        match self {
            Self::Named(tz) => anchor.with_timezone(tz).fixed_offset(),
            Self::Fixed(offset) => anchor.with_timezone(offset),
        }
    }

    /// Return the local minute-of-day (0-1439) for a given UTC instant.
    pub fn minute_of_day(&self, instant: DateTime<Utc>) -> u16 {
        let local = self.local_time(instant);
        local.hour() as u16 * 60 + local.minute() as u16
    }

    /// Current UTC offset in seconds (positive = east of UTC).
    ///
    /// For named zones this is DST-aware at `now_utc`.
    pub fn utc_offset_seconds(&self, now_utc: DateTime<Utc>) -> i32 {
        match self {
            Self::Named(tz) => now_utc.with_timezone(tz).offset().fix().local_minus_utc(),
            Self::Fixed(offset) => offset.local_minus_utc(),
        }
    }
}

/// Parse a user-supplied timezone string into a [`ZoneHandle`].
///
/// Accepted formats:
/// - `"local"` — system timezone (IANA lookup, fixed-offset fallback)
/// - `"UTC"` / `"GMT"` — zero offset
/// - `"UTC+5:30"`, `"GMT-4"` — explicit offset
/// - Any valid IANA name (e.g. `"Europe/London"`)
pub fn parse_zone(input: &str) -> Result<ZoneHandle, TimezoneError> {
    let trimmed = input.trim();

    if trimmed.eq_ignore_ascii_case("local") {
        if let Ok(name) = iana_time_zone::get_timezone()
            && let Ok(tz) = name.parse::<Tz>()
        {
            return Ok(ZoneHandle::Named(tz));
        }

        return Ok(ZoneHandle::Fixed(Local::now().offset().fix()));
    }

    if trimmed.eq_ignore_ascii_case("utc") || trimmed.eq_ignore_ascii_case("gmt") {
        return Ok(ZoneHandle::Fixed(FixedOffset::east_opt(0).unwrap()));
    }

    if has_utc_prefix(trimmed) {
        let prefix_len = 3;
        let offset = parse_utc_offset(&trimmed[prefix_len..], trimmed)?;
        return Ok(ZoneHandle::Fixed(offset));
    }

    trimmed
        .parse::<Tz>()
        .map(ZoneHandle::Named)
        .map_err(|_| TimezoneError::Unknown(trimmed.to_string()))
}

/// Return a static slice of all known IANA timezones.
pub fn all_timezones() -> &'static [Tz] {
    &chrono_tz::TZ_VARIANTS
}

/// Check if a string starts with a `UTC` or `GMT` prefix followed by more characters.
fn has_utc_prefix(input: &str) -> bool {
    input.len() > 3
        && (input[..3].eq_ignore_ascii_case("utc") || input[..3].eq_ignore_ascii_case("gmt"))
}

/// Format a UTC offset in seconds as `±HH:MM` (e.g., `+10:00`, `-04:00`, `+00:00`).
pub fn format_utc_offset(offset_seconds: i32) -> String {
    let sign = if offset_seconds < 0 { '-' } else { '+' };
    let abs = offset_seconds.unsigned_abs();
    let hours = abs / 3600;
    let minutes = (abs % 3600) / 60;
    format!("{sign}{hours:02}:{minutes:02}")
}

/// Format a UTC offset in seconds as a short label: `UTC+N` or `UTC+N:MM`.
/// Examples: `UTC+0`, `UTC+10`, `UTC-4`, `UTC+5:30`.
pub fn format_utc_offset_short(offset_seconds: i32) -> String {
    let sign = if offset_seconds < 0 { '-' } else { '+' };
    let abs = offset_seconds.unsigned_abs();
    let hours = abs / 3600;
    let minutes = (abs % 3600) / 60;
    if minutes == 0 {
        format!("UTC{sign}{hours}")
    } else {
        format!("UTC{sign}{hours}:{minutes:02}")
    }
}

/// Parse the numeric portion of a `UTC±H[:MM]` string into a [`FixedOffset`].
fn parse_utc_offset(input: &str, whole: &str) -> Result<FixedOffset, TimezoneError> {
    let Some(sign) = input.chars().next() else {
        return Err(TimezoneError::Unknown(whole.to_string()));
    };
    if sign != '+' && sign != '-' {
        return Err(TimezoneError::Unknown(whole.to_string()));
    }

    let rest = &input[1..];
    let (hours, minutes) = if let Some((hours, minutes)) = rest.split_once(':') {
        (hours, minutes)
    } else if rest.len() > 2 {
        (&rest[..2], &rest[2..])
    } else {
        (rest, "0")
    };

    let Ok(hours) = hours.parse::<i32>() else {
        return Err(TimezoneError::Unknown(whole.to_string()));
    };
    let Ok(minutes) = minutes.parse::<i32>() else {
        return Err(TimezoneError::Unknown(whole.to_string()));
    };

    if hours > 23 || minutes > 59 {
        return Err(TimezoneError::Unknown(whole.to_string()));
    }

    let seconds = hours * 3600 + minutes * 60;
    let seconds = if sign == '-' { -seconds } else { seconds };

    FixedOffset::east_opt(seconds).ok_or_else(|| TimezoneError::Unknown(whole.to_string()))
}
