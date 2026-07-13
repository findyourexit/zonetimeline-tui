# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0] - 2026-07-13

### Changed

- Refreshed all dependencies to their latest stable releases and raised the minimum supported Rust version to 1.88 to match.
- Merged the timeline's UTC reference row into the hour-axis row: the axis is itself the UTC scale, so the `UTC` label now sits in that row's gutter rather than on a separate, empty reference bar — reclaiming a row for zone ribbons.
- Polished the ribbon rendering: adjacent availability sections (core / shoulder / off) now blend into each other with a smooth multi-cell colour gradient in truecolour instead of a hard edge — each column is filled by the time-weighted mix of the states it spans, and monochrome ramps the same transition across its `░▒▓█` shades. The **Overlap** strip is now a braille reach-histogram whose dot height encodes how many zones are reachable.
- Redesigned the timeline as a width-scaling **heat-ribbon grid**: each zone is a continuous availability ribbon (core / shoulder / off) with an **Overlap** strip that plots mutual availability and a summary line naming the best meeting window inline, plus an **At cursor** inspector showing exact local times per zone.
- The timeline now scales to fill the terminal width — each column covers a slice of the visible range (well under 30 minutes at typical widths) — and adapts to terminal colour support (truecolour → 16-colour → monochrome).
- Added a one-line **legend** in the header (availability colours + now / cursor markers) so the view is self-documenting, and moved the anchor clock into the header title.
- Simplified off-hours to a single neutral tone (dropping the before/after split), removed the per-zone ○ / ● noon-midnight glyphs that cluttered each ribbon, and unified the availability vocabulary across the ribbon, inspector, and overlap strip.
- Grouped the controls bar (navigate / zones / general) and renamed the weakest meeting tier from *LeastBad* to *fallback*.

### Fixed

- The cursor caret no longer overwrites an axis hour label (previously rendered as e.g. `◆2`), and the cursor rule now overlays the ribbon instead of erasing its colour.
- Hour-aligned the live "now" timeline so work-window boundaries, axis labels and the cursor's 30-minute steps land on whole hours instead of being skewed by the current minute-of-hour; the exact "now" instant is still pinpointed by the ▼ marker. Explicit `--time` anchors remain precise to the minute.
- The ▼ "now" marker and the *jump-to-now* cursor now land on the true current instant (they were previously offset by the anchor's minute-of-hour).
- Axis hour labels now track true UTC hours even when the visible range starts mid-hour, so they no longer drift away from the ribbon's boundaries.
- Fixed a one-minute classification hole at each work-window's end: the exact closing hour (e.g. 17:00) was counted as neither core nor shoulder, which both punched a single-character "gap" between the core and shoulder bands on the ribbon (at whole hours, intermittently) and clipped overlap windows by a minute (e.g. a 60-minute window reported as `15:01–16:00 · 59m`). Shoulders are now half-open `[end, end + shoulder)`, flush against core.
- Monochrome / `NO_COLOR` rendering now covers the whole UI. The header legend, the best-windows and cursor-inspector panels, the controls bar and the help overlay previously still emitted ANSI colour; every colour decision now flows through the palette, so a monochrome terminal renders colour-free throughout.

### Added

- `n` keybinding to jump the cursor to the current time (the cursor moves in 30-minute steps). With a fixed `--time` anchor there is no live "now", so it surfaces a brief status message instead of silently doing nothing.
- VHS tapes in [`tapes/`](tapes) for recording the README demo GIFs, and a refreshed hero `assets/demo.gif` plus new `assets/timeline.gif` and `assets/manage.gif` capturing the current gradient ribbons, overlap histogram and interactive zone management.

### Removed

- Micro Mode and the fixed Compact layout — superseded by the single responsive layout that scales to fill the pane and scrolls vertically. Below 80×24 a resize prompt is shown.

## [0.2.0] - 2026-04-07

### Added

- Micro Mode! A new display mode, intended for super size-constrained terminal clients (inc. those running on mobile handsets)

### Fixed

- Broken Renovate GitHub Actions workflow

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
