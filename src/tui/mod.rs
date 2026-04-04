//! Terminal UI layer for Zone Timeline.
//!
//! Organizes the TUI into four sub-modules: the event loop (`app`), application
//! state machine (`state`), modal/form data types (`forms`), and the rendering
//! layer (`view`).

/// Event loop: terminal setup, input dispatch, and frame drawing.
pub mod app;
/// Modal dialog data types, timezone picker entries, and time-slot helpers.
pub mod forms;
/// Application state machine managing focus, zones, sort order, and modals.
pub mod state;
/// Rendering functions that paint the UI into a ratatui `Buffer`.
pub mod view;
