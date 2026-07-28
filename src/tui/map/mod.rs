//! World map view: a braille Web-Mercator globe that plots each configured
//! zone with a day/night terminator, driven by the shared timeline cursor.
//!
//! The pieces:
//! - [`projection`] — Web-Mercator lon/lat ↔ normalized coordinates.
//! - [`canvas`] — a braille sub-cell bitmap for the coastline outline.
//! - [`coastline`] / [`zone_coords`] — vendored, generated geometry + zone points.
//! - [`solar`] — subsolar point and day/night classification.
//! - [`locations`] — resolve a zone to a point (real coordinate or offset).

pub mod canvas;
pub mod coastline;
pub mod locations;
pub mod projection;
pub mod solar;
pub mod zone_coords;
