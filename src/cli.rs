//! CLI argument parsing via `clap` derive macros.
//!
//! Defines the top-level [`Cli`] struct and the optional [`Command`] subcommand.

use std::path::PathBuf;

use chrono::NaiveTime;
use clap::{Parser, Subcommand};

/// Top-level CLI arguments for `ztl`.
#[derive(Debug, Parser)]
#[command(name = "ztl", about = "zone time line", version)]
pub struct Cli {
    /// Anchor time in `HH` or `HH:MM` format (defaults to now).
    #[arg(short = 't', long, value_parser = parse_hhmm)]
    pub time: Option<NaiveTime>,

    /// Number of hours to display in the timeline.
    #[arg(short = 'n', long)]
    pub nhours: Option<u16>,

    /// Add a single timezone (repeatable, e.g. `-z US/Eastern -z Europe/London`).
    #[arg(short = 'z', long)]
    pub zone: Vec<String>,

    /// Add multiple timezones at once, comma-separated (e.g. `-Z US/Eastern,Europe/London`).
    #[arg(short = 'Z', long, value_delimiter = ',')]
    pub zones: Vec<String>,

    /// Path to a TOML config file (overrides default search paths).
    #[arg(short = 'c', long)]
    pub config: Option<PathBuf>,

    /// Output width in columns.
    #[arg(short = 'w', long)]
    pub width: Option<u16>,

    /// Render plain text to stdout instead of launching the TUI.
    #[arg(long)]
    pub plain: bool,

    /// Launch directly into the interactive world map view.
    #[arg(long, conflicts_with = "plain")]
    pub map: bool,

    #[arg(
        long,
        help = "Hours outside work window to mark as shoulder (default: 1)"
    )]
    /// Hours outside the work window to mark as shoulder time.
    pub shoulder_hours: Option<u16>,

    /// Optional subcommand (e.g. `list`).
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Available subcommands.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// Print all known timezone identifiers and exit.
    List,
}

// Parse a time string in `HH` or `HH:MM` format into a `NaiveTime`.
fn parse_hhmm(input: &str) -> Result<NaiveTime, String> {
    NaiveTime::parse_from_str(input, "%H:%M")
        .or_else(|_| NaiveTime::parse_from_str(&format!("{input}:00"), "%H:%M"))
        .map_err(|_| format!("{input} is not valid time HH[:MM]"))
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::Parser;

    #[test]
    fn map_conflicts_with_plain() {
        let err = Cli::try_parse_from(["ztl", "--map", "--plain"])
            .expect_err("--map and --plain must be mutually exclusive");
        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn map_alone_parses() {
        let cli = Cli::try_parse_from(["ztl", "--map"]).expect("--map alone is valid");
        assert!(cli.map && !cli.plain);
    }
}
