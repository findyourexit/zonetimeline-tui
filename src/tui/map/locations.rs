//! Resolve a zone to a point on the map.
//!
//! Named IANA zones look up their representative coordinate in the vendored
//! [`ZONE_COORDS`] table. Fixed-offset zones (and the rare named zone missing
//! from the table) have no real location, so they are placed on the equator at
//! the longitude matching their current UTC offset (15° per hour) and tagged
//! [`Placement::Offset`] so the renderer can mark them distinctly.

use chrono::{DateTime, Utc};

use crate::core::timezones::ZoneHandle;
use crate::tui::map::projection::wrap_lon;
use crate::tui::map::zone_coords::ZONE_COORDS;

/// How a zone's map position was derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// A real geographic coordinate from the IANA table.
    Geographic,
    /// A synthetic equator position derived from the zone's UTC offset.
    Offset,
}

/// A resolved map position for a zone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoneLocation {
    pub lat: f64,
    pub lon: f64,
    pub placement: Placement,
}

/// Resolve a zone handle to a map position at `now_utc` (needed for the offset
/// fallback, whose longitude follows the zone's DST-aware offset).
pub fn locate(handle: &ZoneHandle, now_utc: DateTime<Utc>) -> ZoneLocation {
    if let ZoneHandle::Named(tz) = handle {
        let name = tz.name();
        if let Ok(idx) = ZONE_COORDS.binary_search_by(|(candidate, _, _)| (*candidate).cmp(name)) {
            let (_, lat, lon) = ZONE_COORDS[idx];
            return ZoneLocation {
                lat: lat as f64,
                lon: lon as f64,
                placement: Placement::Geographic,
            };
        }
    }

    // Fixed offset, or a named zone absent from the table: place on the equator
    // at the longitude matching the current UTC offset (15°/hour = 1°/240s).
    let offset_secs = handle.utc_offset_seconds(now_utc) as f64;
    ZoneLocation {
        lat: 0.0,
        lon: wrap_lon(offset_secs / 240.0),
        placement: Placement::Offset,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::timezones::parse_zone;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 28, 12, 0, 0).unwrap()
    }

    #[test]
    fn named_zone_resolves_to_a_real_coordinate() {
        let loc = locate(&parse_zone("America/New_York").unwrap(), now());
        assert_eq!(loc.placement, Placement::Geographic);
        assert!((loc.lat - 40.71).abs() < 0.1);
        assert!((loc.lon + 74.0).abs() < 0.1);
    }

    #[test]
    fn utc_lands_on_the_prime_meridian_equator() {
        let loc = locate(&parse_zone("UTC").unwrap(), now());
        assert_eq!(loc.placement, Placement::Offset);
        assert!(loc.lat.abs() < 1e-9);
        assert!(loc.lon.abs() < 1e-9);
    }

    #[test]
    fn fixed_offset_lands_at_its_offset_longitude() {
        let loc = locate(&parse_zone("UTC+5:30").unwrap(), now());
        assert_eq!(loc.placement, Placement::Offset);
        assert!(loc.lat.abs() < 1e-9);
        // 5.5 hours × 15° = 82.5°E.
        assert!((loc.lon - 82.5).abs() < 0.01, "got {}", loc.lon);
    }

    #[test]
    fn every_vendored_coordinate_is_in_range() {
        for (name, lat, lon) in ZONE_COORDS {
            assert!((-90.0..=90.0).contains(&(*lat as f64)), "{name} lat {lat}");
            assert!(
                (-180.0..=180.0).contains(&(*lon as f64)),
                "{name} lon {lon}"
            );
        }
    }
}
