<p align="center">
  <img src="assets/ztl-header.png" alt="Zone Timeline TUI" />
</p>

# Zone Timeline (TUI)

[![CI Status](https://github.com/findyourexit/zonetimeline-tui/workflows/CI/badge.svg)](https://github.com/findyourexit/zonetimeline-tui/actions)
[![Release](https://img.shields.io/github/v/release/findyourexit/zonetimeline-tui)](https://github.com/findyourexit/zonetimeline-tui/releases)
[![License](https://img.shields.io/github/license/findyourexit/zonetimeline-tui)](https://opensource.org/licenses/MIT)

A terminal tool for visually comparing time zones — built for distributed teams.

See at a glance where working hours overlap across zones, find meeting slots
without mental arithmetic, and manage your zone list interactively. Ships as a
single binary with both a rich TUI and a plain-text mode for scripts and pipes.

<p align="center">
  <img src="assets/demo.gif" alt="Zone Timeline TUI — live timeline with gradient ribbons, cursor, now marker and overlap histogram" width="800" />
</p>

## Quick Start

### Install It

<details>
<summary><strong>Homebrew (macOS)</strong></summary>

```bash
brew tap findyourexit/tap
brew install ztl
```

</details>

<details>
<summary><strong>Pre-Built Binaries</strong></summary>

Download the latest release for your platform from the
[GitHub Releases](https://github.com/findyourexit/zonetimeline-tui/releases) page.

Archives are provided for:
- macOS (Apple Silicon and Intel)
- Linux (`x86_64` and `aarch64`)
- Windows (`x86_64`)

</details>

<details>
<summary><strong>Build It Yourself</strong></summary>

### Source Build

```bash
cargo install --git https://github.com/findyourexit/zonetimeline-tui
```

### Local Build

```bash
git clone https://github.com/findyourexit/zonetimeline-tui.git
cd zonetimeline-tui
cargo build --release
# Binary is at target/release/ztl
```

</details>

### Run It

```bash
# Launch the TUI
ztl

# Plain text output
ztl --plain --zones UTC,Europe/London,America/New_York --time 07:30 --nhours 12

# List supported timezone names
ztl list
```

## Modes

| Flag / subcommand | Behaviour                               |
|-------------------|-----------------------------------------|
| *(none)*          | Launch the interactive TUI              |
| `--plain`         | Render a text timeline and exit         |
| `list`            | Print supported timezone names and exit |

- `--time` is interpreted in UTC and accepts both `HH` and `HH:MM`.
- An explicit time is applied to the current UTC date.
- `--width` sets the output width in columns.
- `--shoulder-hours` controls how many hours outside the work window are marked as shoulder time (default: 1).

### The Timeline View

<p align="center">
  <img src="assets/timeline.gif" alt="Moving the 30-minute cursor updates the At-cursor inspector; ▼ marks now" width="800" />
</p>

Each zone is shown as a continuous **availability ribbon** rather than a grid of
numbers:

- Every minute is one of three states: **Core** work hours, **shoulder** time
  (±`--shoulder-hours`), or **Off**-hours — each with its own colour (and a
  distinct shade character in monochrome). A one-line **legend** in the header
  spells out the colours and markers, so nothing needs memorising.
- An **Overlap** strip beneath the ribbons shows when people are mutually
  available, drawn as a braille histogram whose dot height reflects how many
  zones are reachable. A summary line names the
  best meeting window inline — its UTC time, length, reach (e.g. `4/6`), and tier
  (*ideal* / *feasible* / *fallback*).
- A movable **cursor** (`◆`, 30-minute steps) drives the **At cursor** inspector,
  which lists the exact local time and status for every zone at that instant; a
  separate `▼` marker tracks **now**.

Rendering adapts to your terminal's colour support: 24-bit truecolour, falling
back to 16-colour, then to monochrome block shading.

### Responsive Layout

The timeline **scales to fill the available width** — widen the pane and each
column covers a finer slice of time (the hour axis gains detail); narrow it and
each column covers more. At typical widths a column spans well under 30 minutes.
When there are more zones than vertical space, the rows **scroll** and a
scrollbar appears.

Below **80×24** the tool shows a resize prompt — that is the minimum size for a
legible render.

## TUI Controls

<p align="center">
  <img src="assets/manage.gif" alt="Adding a zone, focusing it, editing its work window, and removing it" width="800" />
</p>

| Key                                                             | Action                      |
|-----------------------------------------------------------------|-----------------------------|
| <kbd>Left</kbd> / <kbd>Right</kbd>, <kbd>h</kbd> / <kbd>l</kbd> | Move cursor (30-min steps)  |
| <kbd>Up</kbd> / <kbd>Down</kbd>, <kbd>j</kbd> / <kbd>k</kbd>    | Move zone focus             |
| <kbd>n</kbd>                                                    | Jump cursor to now          |
| <kbd>a</kbd>                                                    | Add zone                    |
| <kbd>x</kbd>                                                    | Remove zone                 |
| <kbd>J</kbd> / <kbd>K</kbd>                                     | Reorder zone                |
| <kbd>e</kbd>                                                    | Edit work window            |
| <kbd>o</kbd>                                                    | Cycle sort mode             |
| <kbd>Enter</kbd>                                                | Submit modal input          |
| <kbd>Esc</kbd>                                                  | Cancel modal input          |
| <kbd>Tab</kbd>                                                  | Switch pane (in edit modal) |
| <kbd>s</kbd>                                                    | Save config                 |
| <kbd>?</kbd>                                                    | Help                        |
| <kbd>q</kbd>                                                    | Quit                        |

## Configuration

The primary config path is `<platform-config-dir>/zonetimeline-tui/config.toml`.
A legacy fallback at `<platform-config-dir>/zonetimeline/config` is also
supported.

Config discovery is asymmetric by design:

- Reads check the new config first, then fall back to the legacy path.
- Writes always target the new config unless `--config PATH` is specified.
- `--config PATH` acts as both the load override and the save target.
- TUI edits persist only on explicit save (`s`).

If no zones are provided via CLI or config, a default set is used:
`local`, `America/New_York`, `Europe/London`. The UTC row is always shown.

Zone merging follows per-field precedence:

- `--zones` overrides config `zones` only.
- `--zone` overrides config `zone` only.
- The two zone lists are concatenated in that order.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

### Regenerating the demo GIFs

The demo GIFs are recorded with [VHS](https://github.com/charmbracelet/vhs)
from the tapes in [`tapes/`](tapes). With `vhs`, `ttyd` and `ffmpeg` on your
PATH, run from the repo root:

```bash
cargo build --release
vhs tapes/demo.tape      # hero            -> assets/demo.gif
vhs tapes/timeline.tape  # timeline view   -> assets/timeline.gif
vhs tapes/manage.tape    # zone management -> assets/manage.gif
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for more details.

## Motivation

This tool was born out of a practical need: managing a distributed team of
engineers spread across distinct time zones around the globe. Finding good
mutual time slots for meetings and synchronous work shouldn't require mental
arithmetic or a web app — it should be a quick terminal command away.

No existing CLI tool quite fit the bill. The closest was the original
[`zonetimeline`](https://github.com/jvrsantacruz/zonetimeline) by
[@jvrsantacruz](https://github.com/jvrsantacruz), a Python utility for
comparing time zones on the command line. It was a great starting point, but I
wanted something more portable, more powerful, and — crucially — with the option
of a rich interactive TUI. I also prefer Rust for CLI/TUI tools these days.

`zonetimeline-tui` is intended as a spiritual successor to that project, offering
a superset of the original's features and functions in a single, self-contained
binary.

## Acknowledgments

This project was inspired by
[zonetimeline](https://github.com/jvrsantacruz/zonetimeline) by
[Javier Santacruz](https://github.com/jvrsantacruz). Thank you for the original
idea and implementation.

## License

[MIT](LICENSE)
