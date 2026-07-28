//! Terminal UI layer for Zone Timeline.
//!
//! Organizes the TUI into seven sub-modules: the event loop (`app`), application
//! state machine (`state`), modal/form data types (`forms`), the color palette
//! (`palette`), the ribbon timeline logic (`ribbon`), the world map (`map`), and
//! the rendering layer (`view`).

/// Event loop: terminal setup, input dispatch, and frame drawing.
pub mod app;
/// Modal dialog data types, timezone picker entries, and time-slot helpers.
pub mod forms;
/// World map view: braille Web-Mercator globe with day/night and zone markers.
pub mod map;
/// Color-capability detection and semantic role → style mapping.
pub mod palette;
/// Pure ribbon logic: state classification, overlap aggregation, cell packing.
pub mod ribbon;
/// Application state machine managing focus, zones, sort order, and modals.
pub mod state;
/// Rendering functions that paint the UI into a ratatui `Buffer`.
pub mod view;
