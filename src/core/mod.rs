//! Core domain logic for timezone comparison.
//!
//! This module contains the data model, timezone resolution, and work-window
//! definitions used to compute overlap between participants in different zones.

/// Domain model: session config, minute-level bitmap, overlap analysis.
pub mod model;
/// Timezone parsing, IANA lookup, and UTC-offset helpers.
pub mod timezones;
/// Work-window (e.g. `09:00-17:00`) parsing and shoulder-hour logic.
pub mod windows;
