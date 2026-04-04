//! # zonetimeline-tui
//!
//! A terminal UI for comparing multiple time zones side by side.
//!
//! Run the binary `ztl` to launch the interactive TUI, or pass `--plain`
//! for a non-interactive text rendering suitable for piping and scripts.
//!
//! ## Crate layout
//!
//! - [`app`] -- top-level entry point that dispatches to TUI or plain mode
//! - [`cli`] -- command-line argument parsing via clap
//! - [`config`] -- configuration loading/saving (TOML) with legacy fallback
//! - [`core`] -- domain model: timezone entries, work windows, minute-level bitmaps
//! - [`render`] -- plain-text renderer for `--plain` mode
//! - [`tui`] -- interactive terminal UI built on ratatui

/// Top-level entry point; dispatches to the TUI or plain-text renderer.
pub mod app;
/// Command-line argument parsing (clap).
pub mod cli;
/// Configuration loading, saving, and CLI-merge logic.
pub mod config;
/// Domain model: timezone entries, work windows, and overlap bitmaps.
pub mod core;
/// Plain-text rendering for non-interactive (`--plain`) output.
pub mod render;
/// Interactive terminal UI built on ratatui.
pub mod tui;
