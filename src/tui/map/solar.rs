//! Solar geometry for the day/night terminator.
//!
//! Computes the subsolar point (the spot on Earth where the sun is directly
//! overhead) for an instant, then classifies any `(lat, lon)` as day or night
//! by the sign of the solar elevation. The approximation is NOAA's low-order
//! series — accurate to a fraction of a degree, far finer than one map cell,
//! and dependency-free.

use chrono::{DateTime, Datelike, Timelike, Utc};
use std::f64::consts::PI;

use crate::tui::map::projection::wrap_lon;

/// The subsolar point for an instant: the latitude/longitude (degrees) at which
/// the sun is at the zenith.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SunPosition {
    /// Subsolar latitude in degrees (equals the solar declination).
    pub lat: f64,
    /// Subsolar longitude in degrees, normalized to `[-180, 180)`.
    pub lon: f64,
}

/// Compute the subsolar point for `instant` (NOAA low-order approximation).
pub fn subsolar(instant: DateTime<Utc>) -> SunPosition {
    let day = instant.ordinal() as f64; // 1..=366
    let hour =
        instant.hour() as f64 + instant.minute() as f64 / 60.0 + instant.second() as f64 / 3600.0;

    // Fractional year (radians).
    let gamma = 2.0 * PI / 365.0 * (day - 1.0 + (hour - 12.0) / 24.0);

    // Equation of time (minutes).
    let eqtime = 229.18
        * (0.000075 + 0.001868 * gamma.cos()
            - 0.032077 * gamma.sin()
            - 0.014615 * (2.0 * gamma).cos()
            - 0.040849 * (2.0 * gamma).sin());

    // Solar declination (radians).
    let decl = 0.006918 - 0.399912 * gamma.cos() + 0.070257 * gamma.sin()
        - 0.006758 * (2.0 * gamma).cos()
        + 0.000907 * (2.0 * gamma).sin()
        - 0.002697 * (3.0 * gamma).cos()
        + 0.001480 * (3.0 * gamma).sin();

    // The sun stands over the meridian where apparent solar time is noon.
    let utc_minutes = hour * 60.0;
    let lon = wrap_lon((720.0 - utc_minutes - eqtime) / 4.0);

    SunPosition {
        lat: decl.to_degrees(),
        lon,
    }
}

/// Cosine of the solar zenith distance at `(lat, lon)`: positive when the sun is
/// above the horizon, negative below, zero on the terminator.
pub fn zenith_cos(sun: &SunPosition, lat: f64, lon: f64) -> f64 {
    let sslat = sun.lat.to_radians();
    let latr = lat.to_radians();
    let dlon = (lon - sun.lon).to_radians();
    latr.sin() * sslat.sin() + latr.cos() * sslat.cos() * dlon.cos()
}

/// Whether `(lat, lon)` is on the night side of the terminator for this instant.
pub fn is_night(sun: &SunPosition, lat: f64, lon: f64) -> bool {
    zenith_cos(sun, lat, lon) < 0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, 0, 0).unwrap()
    }

    #[test]
    fn subsolar_latitude_tracks_the_seasons() {
        // Solstices: sun overhead near the tropics (±23.44°).
        assert!((subsolar(at(2026, 6, 21, 12)).lat - 23.44).abs() < 1.0);
        assert!((subsolar(at(2026, 12, 21, 12)).lat + 23.44).abs() < 1.0);
        // Equinox: sun over the equator.
        assert!(subsolar(at(2026, 3, 20, 12)).lat.abs() < 2.0);
    }

    #[test]
    fn subsolar_longitude_follows_utc() {
        // At 12:00 UTC the sun is near the prime meridian.
        assert!(subsolar(at(2026, 3, 20, 12)).lon.abs() < 5.0);
        // At 00:00 UTC it is near the antimeridian.
        assert!(subsolar(at(2026, 3, 20, 0)).lon.abs() > 175.0);
    }

    #[test]
    fn noon_greenwich_is_day_antimeridian_is_night() {
        let sun = subsolar(at(2026, 3, 20, 12));
        assert!(!is_night(&sun, 0.0, 0.0), "Greenwich at solar noon is day");
        assert!(
            is_night(&sun, 0.0, 180.0),
            "antimeridian at UTC noon is night"
        );
    }

    #[test]
    fn terminator_separates_day_from_night() {
        let sun = subsolar(at(2026, 6, 21, 12));
        // 90° of longitude east of the subsolar meridian on the equator is
        // right at sunrise/sunset — zenith cosine near zero.
        let c = zenith_cos(&sun, 0.0, sun.lon + 90.0);
        assert!(c.abs() < 0.15, "expected near-terminator, got {c}");
    }
}
