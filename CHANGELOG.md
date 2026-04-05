# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-04-05

### Added

- Interactive TUI mode with side-by-side timezone comparison
- Plain text output mode (`--plain`) for scripting and piping
- `list` subcommand to print supported IANA timezone names
- Work window highlighting with configurable shoulder hours
- Overlap detection showing ideal, feasible, and least-bad meeting windows
- Zone picker with substring filtering for adding timezones
- Reorderable zone list with manual and automatic sort modes
- Editable per-zone work windows via dual-pane time slot selector
- Persistent TOML configuration with legacy config fallback
- Homebrew tap installation support
- Cross-platform release binaries (macOS, Linux, Windows)
