//! Configuration loading, saving, and merging with CLI overrides.
//!
//! Resolution order (highest priority first):
//!
//! 1. **CLI flags** -- always win when provided
//! 2. **Explicit `--config` path** -- used verbatim for both read and write
//! 3. **New-style path** -- `<config_dir>/zonetimeline-tui/config.toml`
//! 4. **Legacy path** -- `<config_dir>/zonetimeline/config` (read-only fallback)
//! 5. **Built-in defaults**
//!
//! Writes always target the new-style path (or the explicit path), so a
//! legacy config is silently migrated on the first save.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::NaiveTime;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::cli::Cli;
use crate::core::model::SortMode;

/// Fallback zone list when neither CLI nor config supplies one.
pub const DEFAULT_ZONES: [&str; 3] = ["local", "America/New_York", "Europe/London"];
/// Default number of hours shown on the timeline.
pub const DEFAULT_NHOURS: u16 = 24;
/// Default work-window range used for overlap highlighting.
pub const DEFAULT_WINDOW: &str = "09:00-17:00";

/// Pair of filesystem paths used to locate config files.
///
/// `new_config_path` is the canonical TOML location; `legacy_config_path`
/// points to the older plain-text format for backwards compatibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigRoots {
    /// Canonical path: `<config_dir>/zonetimeline-tui/config.toml`.
    pub new_config_path: PathBuf,
    /// Legacy path: `<config_dir>/zonetimeline/config`.
    pub legacy_config_path: PathBuf,
}

impl ConfigRoots {
    /// Derive config roots from the platform-standard directories.
    pub fn from_project_dirs() -> Result<Self> {
        let dirs = ProjectDirs::from("", "", "zonetimeline-tui")
            .ok_or_else(|| anyhow!("could not determine config directories"))?;
        let config_dir = dirs.config_dir();
        let base_dir = config_dir
            .parent()
            .ok_or_else(|| anyhow!("could not determine legacy config directory"))?;

        Ok(Self {
            new_config_path: config_dir.join("config.toml"),
            legacy_config_path: base_dir.join("zonetimeline").join("config"),
        })
    }

    /// Build config roots under an arbitrary base directory (useful for tests).
    pub fn from_base(base: impl AsRef<Path>) -> Self {
        let base = base.as_ref();
        Self {
            new_config_path: base.join("zonetimeline-tui").join("config.toml"),
            legacy_config_path: base.join("zonetimeline").join("config"),
        }
    }
}

/// Resolved read/write paths for the current session.
///
/// `load_path` may differ from `save_path` when reading from the legacy
/// location -- saves always go to the new-style path so that legacy configs
/// are migrated automatically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSource {
    /// Where to read from (`None` when no config file exists yet).
    pub load_path: Option<PathBuf>,
    /// Where to write on save (always the new-style or explicit path).
    pub save_path: PathBuf,
}

impl ConfigSource {
    /// Create a `ConfigSource` with explicit paths.
    pub fn new(load_path: Option<PathBuf>, save_path: PathBuf) -> Self {
        Self {
            load_path,
            save_path,
        }
    }
}

/// On-disk TOML representation of the configuration file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FileConfig {
    /// `[general]` table.
    pub general: GeneralConfig,
    /// `[overlap]` table.
    pub overlap: OverlapConfig,
}

impl FileConfig {
    /// Construct a `FileConfig` from individual field groups.
    pub fn from_parts(
        zones: Vec<String>,
        extra: Vec<String>,
        nhours: Option<u16>,
        window: String,
        work_hours: BTreeMap<String, String>,
    ) -> Self {
        Self {
            general: GeneralConfig {
                zones,
                zone: extra,
                nhours,
                ..GeneralConfig::default()
            },
            overlap: OverlapConfig {
                default_window: window,
                work_hours,
                shoulder_hours: default_shoulder_hours(),
            },
        }
    }
}

/// `[general]` section of the config file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    /// Primary zone list (`zones = [...]` in TOML).
    pub zones: Vec<String>,
    /// Additional zones appended via `[[zone]]` entries.
    pub zone: Vec<String>,
    /// Persisted display order (may diverge from `zones + zone` after user reorder).
    pub ordered_zones: Vec<String>,
    /// Number of hours visible on the timeline.
    pub nhours: Option<u16>,
    /// Fixed anchor time (`HH:MM`); `None` means "now".
    pub anchor_time: Option<String>,
    /// Terminal column width override.
    pub width: Option<u16>,
    /// If `true`, skip the TUI and use plain-text output.
    pub plain: bool,
    /// Zone sort strategy.
    pub sort_mode: Option<SortMode>,
}

/// `[overlap]` section -- controls work-window highlighting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OverlapConfig {
    /// Fallback work window applied to zones without a per-zone override.
    #[serde(default = "default_window")]
    pub default_window: String,
    /// Per-zone work-window overrides keyed by IANA timezone name.
    pub work_hours: BTreeMap<String, String>,
    /// Hours of "shoulder" time rendered around each work window.
    #[serde(default = "default_shoulder_hours")]
    pub shoulder_hours: u16,
}

/// Default shoulder hours (1 hour on each side of the work window).
pub fn default_shoulder_hours() -> u16 {
    1
}

impl Default for OverlapConfig {
    fn default() -> Self {
        Self {
            default_window: default_window(),
            work_hours: BTreeMap::new(),
            shoulder_hours: default_shoulder_hours(),
        }
    }
}

/// Fully resolved, ready-to-use configuration produced by merging file +
/// CLI. This is the value handed to the app/TUI layer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionSeed {
    /// Primary zone list (from `--zones` or config `zones`).
    pub base_zones: Vec<String>,
    /// Extra zones appended via `--zone` or config `[[zone]]`.
    pub extra_zones: Vec<String>,
    /// Display order after merge and dedup.
    pub ordered_zones: Vec<String>,
    /// Timeline span in hours.
    pub nhours: u16,
    /// Fixed anchor time; `None` means "now".
    pub anchor_time: Option<NaiveTime>,
    /// Terminal column width override.
    pub width: Option<u16>,
    /// Skip the TUI and render plain text.
    pub plain: bool,
    /// Path where config will be saved on exit.
    pub save_path: PathBuf,
    /// Fallback work window for overlap highlighting.
    pub default_window: String,
    /// Per-zone work-window overrides.
    pub work_hours: BTreeMap<String, String>,
    /// Shoulder hours around work windows.
    pub shoulder_hours: u16,
    /// How zones are sorted in the display.
    pub sort_mode: SortMode,
}

/// Return the default work-window string.
pub fn default_window() -> String {
    DEFAULT_WINDOW.to_string()
}

/// Determine where to read and write configuration.
///
/// If an explicit path is given (via `--config`), it is used for both read
/// and write. Otherwise the new-style path is preferred for reading, with a
/// fallback to the legacy path. Writes always target the new-style path so
/// that legacy configs are transparently migrated on save.
pub fn locate_config(explicit: Option<PathBuf>, roots: &ConfigRoots) -> ConfigSource {
    // Explicit --config flag: use the same path for both read and write.
    if let Some(path) = explicit {
        return ConfigSource::new(Some(path.clone()), path);
    }

    // Prefer new path; fall back to legacy path for reading only.
    let load_path = if roots.new_config_path.exists() {
        Some(roots.new_config_path.clone())
    } else if roots.legacy_config_path.exists() {
        Some(roots.legacy_config_path.clone())
    } else {
        None
    };

    // Save always goes to the new-style path (migrates legacy on first write).
    ConfigSource::new(load_path, roots.new_config_path.clone())
}

/// Read and parse the TOML config file. Returns defaults when no file exists.
pub fn load_file_config(source: &ConfigSource) -> Result<FileConfig> {
    let Some(path) = source.load_path.as_ref() else {
        return Ok(FileConfig::default());
    };

    if !path.exists() {
        return Ok(FileConfig::default());
    }

    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read config file {}", path.display()))?;
    let file: FileConfig = toml::from_str(&raw)
        .with_context(|| format!("failed to parse config file {}", path.display()))?;

    if let Some(anchor_time) = file.general.anchor_time.as_deref() {
        NaiveTime::parse_from_str(anchor_time, "%H:%M")
            .with_context(|| format!("failed to parse config file {}", path.display()))?;
    }

    Ok(file)
}

/// Persist the current session back to `session.save_path` as TOML.
///
/// UTC/GMT zones are stripped before writing because they are always
/// available implicitly.
pub fn save_session(session: &crate::core::model::SessionConfig) -> Result<()> {
    let filter_utc = |zones: &[String]| -> Vec<String> {
        zones
            .iter()
            .filter(|z| {
                !z.trim().eq_ignore_ascii_case("utc") && !z.trim().eq_ignore_ascii_case("gmt")
            })
            .cloned()
            .collect()
    };

    let file = FileConfig {
        general: GeneralConfig {
            zones: filter_utc(&session.base_zones),
            zone: filter_utc(&session.extra_zones),
            ordered_zones: filter_utc(&session.ordered_zones),
            nhours: Some(session.nhours),
            anchor_time: match session.anchor {
                crate::core::model::AnchorSpec::Now => None,
                crate::core::model::AnchorSpec::Explicit(time) => {
                    Some(time.format("%H:%M").to_string())
                }
            },
            width: session.width,
            plain: session.plain,
            sort_mode: Some(session.sort_mode),
        },
        overlap: OverlapConfig {
            default_window: session.default_window.clone(),
            work_hours: session.work_hours.clone(),
            shoulder_hours: session.shoulder_hours,
        },
    };

    if let Some(parent) = session.save_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config directory {}", parent.display()))?;
    }

    fs::write(&session.save_path, toml::to_string_pretty(&file)?).with_context(|| {
        format!(
            "failed to write config file {}",
            session.save_path.display()
        )
    })?;
    Ok(())
}

/// Merge file-based config with CLI overrides to produce a [`SessionSeed`].
///
/// CLI values take precedence; file values fill in whatever the CLI didn't
/// specify; built-in defaults cover the rest.
pub fn merge_with_cli(cli: &Cli, file: FileConfig, source: ConfigSource) -> SessionSeed {
    let anchor_time = cli.time.or_else(|| {
        file.general
            .anchor_time
            .as_deref()
            .and_then(|value| NaiveTime::parse_from_str(value, "%H:%M").ok())
    });
    let zones = if cli.zones.is_empty() {
        file.general.zones
    } else {
        cli.zones.clone()
    };
    let extra = if cli.zone.is_empty() {
        file.general.zone
    } else {
        cli.zone.clone()
    };

    // Preserve the persisted display order only when the user hasn't
    // overridden zones on the CLI; otherwise rebuild from the new lists.
    let mut ordered_zones =
        if cli.zones.is_empty() && cli.zone.is_empty() && !file.general.ordered_zones.is_empty() {
            file.general.ordered_zones.clone()
        } else {
            let mut ordered_zones = zones.clone();
            ordered_zones.extend(extra.clone());
            ordered_zones
        };
    if ordered_zones.is_empty() {
        ordered_zones = DEFAULT_ZONES
            .iter()
            .map(|zone| (*zone).to_string())
            .collect();
    }

    // UTC/GMT are always synthesised by the TUI, so strip them from all
    // zone lists to avoid duplicates.
    let filter_utc = |v: &mut Vec<String>| {
        v.retain(|z| !z.trim().eq_ignore_ascii_case("utc") && !z.trim().eq_ignore_ascii_case("gmt"))
    };
    filter_utc(&mut ordered_zones);

    let mut base_zones = zones;
    let mut extra_zones = extra;
    filter_utc(&mut base_zones);
    filter_utc(&mut extra_zones);

    let nhours = cli.nhours.or(file.general.nhours).unwrap_or(DEFAULT_NHOURS);

    SessionSeed {
        base_zones,
        extra_zones,
        ordered_zones,
        nhours,
        anchor_time,
        width: cli.width.or(file.general.width),
        plain: cli.plain || file.general.plain,
        save_path: source.save_path,
        default_window: if file.overlap.default_window.is_empty() {
            default_window()
        } else {
            file.overlap.default_window
        },
        work_hours: file.overlap.work_hours,
        shoulder_hours: cli.shoulder_hours.unwrap_or(file.overlap.shoulder_hours),
        sort_mode: file.general.sort_mode.unwrap_or_default(),
    }
}
