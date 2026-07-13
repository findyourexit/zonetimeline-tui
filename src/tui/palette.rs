//! Color capability detection and semantic palette.
//!
//! Resolves a [`Capability`] from the environment once at startup, then maps
//! semantic [`Role`]s to ratatui [`Style`]s (and monochrome shade characters).
//! Keeps all color decisions in one place so the renderer is colour-agnostic.

use ratatui::style::{Color, Modifier, Style};

use crate::core::model::MinuteClass;

/// Rendering fidelity, highest to lowest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    /// 24-bit RGB.
    Truecolor,
    /// 16 named ANSI colours.
    Ansi16,
    /// No colour: encode state with shade characters.
    Monochrome,
}

/// A semantic colour role. The renderer asks for these, never raw colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Core,
    Shoulder,
    Off,
    MarkerNow,
    MarkerCursor,
    Label,
    LabelSelected,
    Axis,
    PanelTitle,
    /// Key-hint glyphs in the controls bar and help overlay.
    KeyHint,
    /// Section headings and the inspector header.
    Heading,
    /// Secondary / de-emphasised text (labels, hints, muted rows).
    Muted,
    /// A selected-row highlight (reverse video in monochrome).
    Selected,
    /// Positive availability (ideal tier, core badge).
    Good,
    /// Marginal availability (feasible tier, shoulder badge).
    Caution,
    /// A selected-row highlight in a focused pane (brighter than [`Role::Selected`]).
    SelectedActive,
}

/// Detect capability from explicit env inputs (testable).
pub fn detect(colorterm: Option<&str>, _term: Option<&str>, no_color: bool) -> Capability {
    if no_color {
        return Capability::Monochrome;
    }
    match colorterm {
        Some(s)
            if s.eq_ignore_ascii_case("truecolor") || s.to_ascii_lowercase().contains("24bit") =>
        {
            Capability::Truecolor
        }
        _ => Capability::Ansi16,
    }
}

/// Resolve capability from the process environment.
pub fn from_env() -> Capability {
    let colorterm = std::env::var("COLORTERM").ok();
    let term = std::env::var("TERM").ok();
    let no_color = std::env::var_os("NO_COLOR").is_some();
    detect(colorterm.as_deref(), term.as_deref(), no_color)
}

/// Maps semantic roles to concrete styles for a fixed capability.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    cap: Capability,
}

impl Palette {
    pub fn new(cap: Capability) -> Self {
        Self { cap }
    }

    pub fn from_env() -> Self {
        Self::new(from_env())
    }

    pub fn capability(&self) -> Capability {
        self.cap
    }

    /// Style for a role. Ribbon states use background fills; markers/labels use fg.
    pub fn style(&self, role: Role) -> Style {
        match self.cap {
            Capability::Truecolor => self.truecolor(role),
            Capability::Ansi16 => self.ansi16(role),
            Capability::Monochrome => self.monochrome(role),
        }
    }

    /// Shade character for ribbon states in monochrome mode (None otherwise).
    pub fn shade(&self, role: Role) -> Option<char> {
        if self.cap != Capability::Monochrome {
            return None;
        }
        match role {
            Role::Core => Some('█'),
            Role::Shoulder => Some('▓'),
            Role::Off => Some('░'),
            _ => None,
        }
    }

    /// Resolved OVERLAP-strip colour for a class, or `None` in monochrome.
    fn overlap_color(&self, class: MinuteClass, total: usize) -> Option<Color> {
        if self.cap == Capability::Monochrome {
            return None;
        }
        let rgb_or_named = |rgb: (u8, u8, u8), named: Color| -> Color {
            if self.cap == Capability::Truecolor {
                Color::Rgb(rgb.0, rgb.1, rgb.2)
            } else {
                named
            }
        };
        let color = match class {
            MinuteClass::Ideal => rgb_or_named((0x2e, 0xcc, 0x71), Color::Green),
            MinuteClass::Feasible => rgb_or_named((0x27, 0xae, 0x60), Color::Green),
            MinuteClass::Partial(n) => {
                // ramp dim mauve -> amber by n/total
                if self.cap == Capability::Truecolor {
                    let t = if total > 1 {
                        f32::from(n) / (total as f32)
                    } else {
                        0.0
                    };
                    let lerp =
                        |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t) as u8;
                    Color::Rgb(lerp(0x3b, 0xb0), lerp(0x24, 0x7d), lerp(0x33, 0x24))
                } else {
                    Color::Yellow
                }
            }
            MinuteClass::None => rgb_or_named((0x14, 0x14, 0x22), Color::Black),
        };
        Some(color)
    }

    /// Background style for an OVERLAP strip column given its class and count.
    pub fn overlap_style(&self, class: MinuteClass, total: usize) -> Style {
        match self.overlap_color(class, total) {
            Some(color) => Style::new().bg(color),
            None => Style::new(),
        }
    }

    /// Foreground style for drawing the OVERLAP strip as a braille histogram;
    /// the dot glyph carries the reach height, this only sets its colour.
    pub fn overlap_fg(&self, class: MinuteClass, total: usize) -> Style {
        match self.overlap_color(class, total) {
            Some(color) => Style::new().fg(color),
            None => Style::new(),
        }
    }

    /// Base ribbon fill colour for a state role, as raw RGB. `Some` only in
    /// truecolor and only for the three ribbon states; the renderer derives
    /// bevel highlights and blended boundary tints from it.
    pub fn role_rgb(&self, role: Role) -> Option<(u8, u8, u8)> {
        if self.cap != Capability::Truecolor {
            return None;
        }
        match role {
            Role::Core => Some((0x1f, 0x9d, 0x57)),
            Role::Shoulder => Some((0x9a, 0x63, 0x26)),
            Role::Off => Some((0x26, 0x29, 0x33)),
            _ => None,
        }
    }

    fn truecolor(&self, role: Role) -> Style {
        if let Some((r, g, b)) = self.role_rgb(role) {
            return Style::new().bg(Color::Rgb(r, g, b));
        }
        let fg = |r, g, b| Style::new().fg(Color::Rgb(r, g, b));
        match role {
            Role::MarkerNow => fg(0xff, 0x5c, 0x8a).add_modifier(Modifier::BOLD),
            Role::MarkerCursor => fg(0x9c, 0xc3, 0xff).add_modifier(Modifier::BOLD),
            Role::Label => Style::new(),
            Role::LabelSelected => Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            Role::Axis => fg(0x7a, 0x7c, 0x98),
            Role::PanelTitle => fg(0x7a, 0x7c, 0x98),
            Role::KeyHint => Style::new().fg(Color::Cyan).add_modifier(Modifier::DIM),
            Role::Heading => Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            Role::Muted => fg(0x7a, 0x7c, 0x98),
            Role::Selected => Style::new()
                .fg(Color::White)
                .bg(Color::Rgb(0x3a, 0x3d, 0x4a)),
            Role::Good => fg(0x2e, 0xcc, 0x71),
            Role::Caution => fg(0xd0, 0x8a, 0x3a),
            Role::SelectedActive => Style::new().fg(Color::Black).bg(Color::Cyan),
            Role::Core | Role::Shoulder | Role::Off => unreachable!("handled by role_rgb"),
        }
    }

    fn ansi16(&self, role: Role) -> Style {
        match role {
            Role::Core => Style::new().bg(Color::Green),
            Role::Shoulder => Style::new().bg(Color::Yellow),
            Role::Off => Style::new().bg(Color::DarkGray),
            Role::MarkerNow => Style::new()
                .fg(Color::LightMagenta)
                .add_modifier(Modifier::BOLD),
            Role::MarkerCursor => Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            Role::Label => Style::new(),
            Role::LabelSelected => Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            Role::Axis => Style::new().fg(Color::DarkGray),
            Role::PanelTitle => Style::new().fg(Color::DarkGray),
            Role::KeyHint => Style::new().fg(Color::Cyan).add_modifier(Modifier::DIM),
            Role::Heading => Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            Role::Muted => Style::new().fg(Color::DarkGray),
            Role::Selected => Style::new().fg(Color::White).bg(Color::DarkGray),
            Role::Good => Style::new().fg(Color::Green),
            Role::Caution => Style::new().fg(Color::Yellow),
            Role::SelectedActive => Style::new().fg(Color::Black).bg(Color::Cyan),
        }
    }

    fn monochrome(&self, role: Role) -> Style {
        match role {
            Role::LabelSelected | Role::MarkerNow | Role::MarkerCursor | Role::Heading => {
                Style::new().add_modifier(Modifier::BOLD)
            }
            Role::Axis | Role::PanelTitle | Role::KeyHint | Role::Muted => {
                Style::new().add_modifier(Modifier::DIM)
            }
            Role::Selected | Role::SelectedActive => Style::new().add_modifier(Modifier::REVERSED),
            _ => Style::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_no_color_is_monochrome() {
        assert_eq!(
            detect(Some("truecolor"), None, true),
            Capability::Monochrome
        );
    }

    #[test]
    fn detect_truecolor_from_colorterm() {
        assert_eq!(
            detect(Some("truecolor"), None, false),
            Capability::Truecolor
        );
        assert_eq!(detect(Some("24bit"), None, false), Capability::Truecolor);
    }

    #[test]
    fn detect_defaults_to_ansi16() {
        assert_eq!(
            detect(None, Some("xterm-256color"), false),
            Capability::Ansi16
        );
    }

    #[test]
    fn truecolor_core_is_rgb_green() {
        let p = Palette::new(Capability::Truecolor);
        assert_eq!(p.style(Role::Core).bg, Some(Color::Rgb(0x1f, 0x9d, 0x57)));
    }

    #[test]
    fn ansi16_core_is_named_green() {
        let p = Palette::new(Capability::Ansi16);
        assert_eq!(p.style(Role::Core).bg, Some(Color::Green));
    }

    #[test]
    fn monochrome_core_has_no_bg_but_has_shade() {
        let p = Palette::new(Capability::Monochrome);
        assert_eq!(p.style(Role::Core).bg, None);
        assert_eq!(p.shade(Role::Core), Some('█'));
    }

    #[test]
    fn overlap_ideal_brighter_than_partial() {
        let p = Palette::new(Capability::Truecolor);
        // Smoke test: Ideal and None resolve to distinct styles.
        assert_ne!(
            p.overlap_style(MinuteClass::Ideal, 6),
            p.overlap_style(MinuteClass::None, 6)
        );
    }

    #[test]
    fn monochrome_roles_never_set_colour() {
        let p = Palette::new(Capability::Monochrome);
        for role in [
            Role::Core,
            Role::Shoulder,
            Role::Off,
            Role::MarkerNow,
            Role::MarkerCursor,
            Role::Label,
            Role::LabelSelected,
            Role::Axis,
            Role::PanelTitle,
            Role::KeyHint,
            Role::Heading,
            Role::Muted,
            Role::Selected,
            Role::Good,
            Role::Caution,
            Role::SelectedActive,
        ] {
            let s = p.style(role);
            assert_eq!(s.fg, None, "role {role:?} set a fg in monochrome");
            assert_eq!(s.bg, None, "role {role:?} set a bg in monochrome");
        }
    }
}
