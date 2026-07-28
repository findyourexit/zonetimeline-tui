//! Web-Mercator projection helpers for the world map.
//!
//! Longitude/latitude in degrees map to normalized `[0, 1]` coordinates where
//! x runs west→east and y runs north→south (so `y = 0` is the top of the map).
//! Latitude is clamped to ±85.05° — the standard Web-Mercator limit beyond
//! which `tan` diverges and the poles smear to infinity.

use std::f64::consts::PI;

/// Maximum absolute latitude representable in Web Mercator (degrees).
pub const LAT_MAX: f64 = 85.051_128_78;

/// Clamp a latitude to the Web-Mercator valid range.
pub fn clamp_lat(lat: f64) -> f64 {
    lat.clamp(-LAT_MAX, LAT_MAX)
}

/// Normalize any longitude into the `[-180, 180)` half-open range.
pub fn wrap_lon(lon: f64) -> f64 {
    let wrapped = (lon + 180.0).rem_euclid(360.0) - 180.0;
    // f64 rem_euclid can round up to exactly 360.0 for tiny-negative inputs,
    // yielding wrapped == 180.0; fold that back into the half-open range.
    if wrapped >= 180.0 {
        wrapped - 360.0
    } else {
        wrapped
    }
}

/// Project a longitude to normalized x in `[0, 1)` (0 = 180°W; approaches 1
/// toward 180°E, which folds back to 0 at the antimeridian seam).
pub fn lon_to_norm(lon: f64) -> f64 {
    (wrap_lon(lon) + 180.0) / 360.0
}

/// Project a latitude to normalized y in `[0, 1]` (0 = north edge, 1 = south edge).
pub fn lat_to_norm(lat: f64) -> f64 {
    let rad = clamp_lat(lat).to_radians();
    0.5 - (PI / 4.0 + rad / 2.0).tan().ln() / (2.0 * PI)
}

/// Inverse of [`lon_to_norm`]: normalized x → longitude in degrees.
pub fn norm_to_lon(nx: f64) -> f64 {
    nx * 360.0 - 180.0
}

/// Inverse of [`lat_to_norm`]: normalized y → latitude in degrees.
pub fn norm_to_lat(ny: f64) -> f64 {
    let t = (0.5 - ny) * 2.0 * PI;
    (2.0 * t.exp().atan() - PI / 2.0).to_degrees()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn longitude_maps_to_full_width() {
        assert!(close(lon_to_norm(-180.0), 0.0, 1e-9));
        assert!(close(lon_to_norm(0.0), 0.5, 1e-9));
        assert!(close(lon_to_norm(179.999), 1.0, 1e-3));
    }

    #[test]
    fn equator_is_vertical_center() {
        assert!(close(lat_to_norm(0.0), 0.5, 1e-9));
    }

    #[test]
    fn northern_latitudes_sit_above_the_equator() {
        assert!(lat_to_norm(45.0) < 0.5);
        assert!(lat_to_norm(-45.0) > 0.5);
    }

    #[test]
    fn latitude_is_clamped_to_mercator_limit() {
        assert_eq!(clamp_lat(90.0), LAT_MAX);
        assert_eq!(clamp_lat(-90.0), -LAT_MAX);
        // Beyond the limit the projection must stay finite.
        assert!(lat_to_norm(90.0).is_finite());
        assert!(lat_to_norm(-90.0).is_finite());
    }

    #[test]
    fn round_trips_within_the_valid_range() {
        for lat in [-80.0, -45.0, -10.0, 0.0, 23.4, 51.5, 80.0] {
            assert!(close(norm_to_lat(lat_to_norm(lat)), lat, 1e-6));
        }
        for lon in [-179.0, -73.0, 0.0, 55.5, 139.7] {
            assert!(close(norm_to_lon(lon_to_norm(lon)), lon, 1e-6));
        }
    }

    #[test]
    fn longitude_wrapping_is_stable() {
        assert!(close(wrap_lon(190.0), -170.0, 1e-9));
        assert!(close(wrap_lon(-190.0), 170.0, 1e-9));
        assert!(wrap_lon(180.0) < 180.0);
        assert!(wrap_lon(360.0).abs() < 1e-9);
    }
}
