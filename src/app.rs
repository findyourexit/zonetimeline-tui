//! Top-level application entry point.
//!
//! Parses CLI arguments, loads and merges configuration, then dispatches
//! to either the interactive TUI or plain-text output mode.

use anyhow::Result;
use chrono::Utc;
use clap::Parser;

use crate::cli::{Cli, Command};
use crate::config::{ConfigRoots, load_file_config, locate_config, merge_with_cli};
use crate::core::model::{ComparisonModel, compute_display_order};
use crate::core::timezones::all_timezones;
use crate::render::plain::render_plain;

/// Run the application.
///
/// Handles the `list` subcommand directly, otherwise builds a
/// [`ComparisonModel`] from config + CLI and renders output in
/// plain or TUI mode.
pub fn run() -> Result<()> {
    let cli = Cli::parse();

    if matches!(cli.command, Some(Command::List)) {
        for timezone in all_timezones() {
            println!("{timezone}");
        }
        return Ok(());
    }

    let roots = ConfigRoots::from_project_dirs()?;
    let source = locate_config(cli.config.clone(), &roots);
    let file = load_file_config(&source)?;
    let seed = merge_with_cli(&cli, file, source);
    let model = ComparisonModel::build(seed, Utc::now())?;

    if model.session().plain {
        let sort_mode = model.session().sort_mode;
        let display_order = compute_display_order(&model.zones, sort_mode, Utc::now());
        print!(
            "{}",
            render_plain(&model, model.session().width.unwrap_or(96), &display_order)
        );
        return Ok(());
    }

    crate::tui::app::run(model)
}
