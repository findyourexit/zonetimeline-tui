//! Domain model for multi-timezone overlap analysis.
//!
//! The central abstraction is a **minute-level bitmap**: for every minute in
//! the visible timeline window we classify whether that instant falls inside
//! every participant's work window (`Ideal`), within shoulder hours for all
//! (`Feasible`), partially reachable (`Partial`), or outside all windows
//! (`None`).  Contiguous runs of the same class are then extracted as
//! [`ClassifiedWindow`]s and ranked so the UI can highlight the best meeting
//! times.

use std::cmp::Reverse;
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use anyhow::Result;
use chrono::{DateTime, Duration, NaiveTime, Timelike, Utc};

use serde::{Deserialize, Serialize};

use crate::config::SessionSeed;

use super::timezones::{ZoneHandle, parse_zone};
use super::windows::WorkWindow;

/// Controls the display order of timezone rows in the TUI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SortMode {
    /// Ascending UTC offset (east-to-west), default.
    #[default]
    UtcOffsetAsc,
    /// Descending UTC offset (west-to-east).
    UtcOffsetDesc,
    /// Alphabetical by label, A-Z.
    LabelAsc,
    /// Alphabetical by label, Z-A.
    LabelDesc,
    /// User-defined drag-and-drop order.
    Manual,
}

impl SortMode {
    /// Cycle to the next sort mode in a fixed ring order.
    pub fn next(self) -> Self {
        match self {
            Self::UtcOffsetAsc => Self::UtcOffsetDesc,
            Self::UtcOffsetDesc => Self::LabelAsc,
            Self::LabelAsc => Self::LabelDesc,
            Self::LabelDesc => Self::Manual,
            Self::Manual => Self::UtcOffsetAsc,
        }
    }

    /// Short human-readable label for the current mode (shown in the status bar).
    pub fn label(self) -> &'static str {
        match self {
            Self::UtcOffsetAsc => "UTC+",
            Self::UtcOffsetDesc => "UTC-",
            Self::LabelAsc => "A-Z",
            Self::LabelDesc => "Z-A",
            Self::Manual => "Manual",
        }
    }
}

/// Specifies where the timeline is anchored: live (`Now`) or a fixed time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnchorSpec {
    /// Track the current wall-clock time (updates each tick).
    Now,
    /// Pin the anchor to a specific time-of-day on the current UTC date.
    Explicit(NaiveTime),
}

impl AnchorSpec {
    /// Resolve to an absolute UTC instant given the current time.
    pub fn resolve(&self, now_utc: DateTime<Utc>) -> DateTime<Utc> {
        match self {
            Self::Now => now_utc,
            Self::Explicit(time) => now_utc.date_naive().and_time(*time).and_utc(),
        }
    }
}

/// Fully resolved configuration for a single comparison session.
///
/// Combines CLI flags, config-file defaults, and any runtime mutations
/// (e.g. adding/removing zones) into one snapshot that can rebuild the model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionConfig {
    /// Zones loaded from the base config file.
    pub base_zones: Vec<String>,
    /// Zones added interactively during this session.
    pub extra_zones: Vec<String>,
    /// Canonical display order (base + extra, after dedup).
    pub ordered_zones: Vec<String>,
    /// Number of hours shown on the timeline (centered on anchor).
    pub nhours: u16,
    /// Timeline anchor point.
    pub anchor: AnchorSpec,
    /// Optional explicit terminal width override.
    pub width: Option<u16>,
    /// If true, disable colors/styles for piping.
    pub plain: bool,
    /// Path to the session save file (TOML).
    pub save_path: PathBuf,
    /// Fallback work-window spec applied when a zone has no per-zone override.
    pub default_window: String,
    /// Per-zone work-window overrides, keyed by zone input name.
    pub work_hours: BTreeMap<String, String>,
    /// Minutes added before/after each work window as a "shoulder" period.
    pub shoulder_hours: u16,
    /// Current row sort order.
    pub sort_mode: SortMode,
}

/// A timezone entry fully resolved and ready for bitmap computation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedZone {
    /// Raw user-supplied zone string (e.g. `"America/New_York"`, `"UTC+5:30"`).
    pub input_name: String,
    /// Human-friendly display label derived from `input_name`.
    pub label: String,
    /// Parsed timezone handle for time conversions.
    pub handle: ZoneHandle,
    /// Work window applicable to this zone.
    pub window: WorkWindow,
}

/// One hour-wide column in the timeline grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineSlot {
    /// Signed offset from the anchor hour (e.g. -12..+11 for a 24h window).
    pub offset_hours: i32,
    /// Absolute UTC instant at the start of this slot.
    pub start_utc: DateTime<Utc>,
    /// If this slot contains "now", the minute offset within the hour (0-59).
    /// `None` for all other slots.
    pub current_minute_offset: Option<u32>,
}

/// A contiguous run of minutes where all zones are in their work window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlapSegment {
    /// UTC start of the overlap run (inclusive).
    pub start_utc: DateTime<Utc>,
    /// UTC end of the overlap run (exclusive).
    pub end_utc: DateTime<Utc>,
    /// Length in minutes.
    pub duration_minutes: i64,
}

/// Classification of a single minute in the bitmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinuteClass {
    /// All zones are within their core work window.
    Ideal,
    /// All zones are reachable (core + shoulder), but not all are in core.
    Feasible,
    /// Only `n` zones are reachable (core + shoulder).
    Partial(u8),
    /// No zone is reachable at this minute.
    None,
}

/// Quality tier for a classified meeting window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowTier {
    /// All participants in core work hours.
    Ideal,
    /// All participants reachable (core or shoulder).
    Feasible,
    /// Best partial overlap found when no Ideal/Feasible window exists.
    LeastBad,
}

/// A contiguous time range sharing the same [`WindowTier`], scored and ranked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedWindow {
    /// Quality tier of this window.
    pub tier: WindowTier,
    /// UTC start (inclusive).
    pub start_utc: DateTime<Utc>,
    /// UTC end (exclusive).
    pub end_utc: DateTime<Utc>,
    /// Duration in minutes.
    pub duration_minutes: i64,
    /// How many zones are reachable during this window.
    pub zones_in_window: usize,
    /// Total zones in the session (for context).
    pub total_zones: usize,
}

/// Top-level model that owns the full result of a comparison computation.
///
/// Constructed via [`ComparisonModel::build`] or [`ComparisonModel::rebuild`],
/// it resolves zones, generates the minute bitmap, and extracts overlap
/// segments and classified windows in one pass.
#[derive(Debug, Clone)]
pub struct ComparisonModel {
    session: SessionConfig,
    /// The resolved UTC anchor instant.
    pub anchor: DateTime<Utc>,
    /// Resolved timezone entries in canonical order.
    pub zones: Vec<ResolvedZone>,
    /// Hour-wide timeline columns.
    pub timeline_slots: Vec<TimelineSlot>,
    /// Contiguous ideal-overlap segments (used by the summary bar).
    pub overlap_segments: Vec<OverlapSegment>,
    classified_windows: Vec<ClassifiedWindow>,
}

impl ComparisonModel {
    /// Build a fresh model from a [`SessionSeed`] (initial config load).
    pub fn build(seed: SessionSeed, now_utc: DateTime<Utc>) -> Result<Self> {
        Self::from_session(SessionConfig::from(seed), now_utc)
    }

    /// Rebuild the model from a mutated [`SessionConfig`] (e.g. after adding a zone).
    pub fn rebuild(session: SessionConfig, now_utc: DateTime<Utc>) -> Result<Self> {
        Self::from_session(session, now_utc)
    }

    /// Core constructor: resolve zones, build bitmap, extract windows.
    pub fn from_session(mut session: SessionConfig, now_utc: DateTime<Utc>) -> Result<Self> {
        let anchor = session.anchor.resolve(now_utc);
        let zones = resolve_zones(&mut session)?;
        let timeline_slots = build_timeline_slots(session.nhours, anchor, &session.anchor);
        let shoulder_minutes = session.shoulder_hours * 60;
        let bitmap = build_minute_bitmap(&zones, &timeline_slots, shoulder_minutes);
        let start_utc = timeline_slots
            .first()
            .map(|s| s.start_utc)
            .unwrap_or(anchor);
        let overlap_segments = derive_overlap_segments(&bitmap, start_utc);
        let classified_windows =
            extract_classified_windows(&bitmap, start_utc, anchor, zones.len());

        Ok(Self {
            session,
            anchor,
            zones,
            timeline_slots,
            overlap_segments,
            classified_windows,
        })
    }

    /// Ranked meeting-window suggestions (Ideal first, then Feasible, then LeastBad).
    pub fn classified_windows(&self) -> &[ClassifiedWindow] {
        &self.classified_windows
    }

    /// Borrow the session config (e.g. for saving or rebuilding).
    pub fn session(&self) -> &SessionConfig {
        &self.session
    }
}

impl From<SessionSeed> for SessionConfig {
    fn from(seed: SessionSeed) -> Self {
        Self {
            base_zones: seed.base_zones,
            extra_zones: seed.extra_zones,
            ordered_zones: seed.ordered_zones,
            nhours: seed.nhours,
            anchor: seed
                .anchor_time
                .map_or(AnchorSpec::Now, AnchorSpec::Explicit),
            width: seed.width,
            plain: seed.plain,
            save_path: seed.save_path,
            default_window: seed.default_window,
            work_hours: seed.work_hours,
            shoulder_hours: seed.shoulder_hours,
            sort_mode: seed.sort_mode,
        }
    }
}

/// Parse and deduplicate zones from the session config.
///
/// Zones appearing more than once (by identity key) are silently dropped.
/// The session's `ordered_zones`, `base_zones`, and `extra_zones` vectors
/// are updated in-place to reflect the deduplicated set.
fn resolve_zones(session: &mut SessionConfig) -> Result<Vec<ResolvedZone>> {
    let mut zones = Vec::new();
    let mut seen = HashSet::new();
    let mut deduped_inputs = Vec::new();
    let mut deduped_base = Vec::new();
    let mut deduped_extra = Vec::new();

    for input in session.ordered_zones.clone() {
        let handle = parse_zone(&input)?;
        // Skip duplicate timezone identities (e.g. "UTC" and "GMT" resolve to the same offset)
        if !seen.insert(handle.identity_key()) {
            continue;
        }

        let label = ZoneHandle::display_label(&input);
        let window_spec = session
            .work_hours
            .get(&input)
            .cloned()
            .unwrap_or_else(|| session.default_window.clone());
        let window = WorkWindow::parse(&window_spec)?;

        deduped_inputs.push(input);
        let input = deduped_inputs.last().cloned().unwrap();
        if session.base_zones.contains(&input) {
            deduped_base.push(input.clone());
        }
        if session.extra_zones.contains(&input) {
            deduped_extra.push(input.clone());
        }
        zones.push(ResolvedZone {
            input_name: input,
            label,
            handle,
            window,
        });
    }

    // Reflect the deduplication back into the session
    session.ordered_zones = deduped_inputs;
    session.base_zones = deduped_base;
    session.extra_zones = deduped_extra;
    Ok(zones)
}

/// Build the hour-wide timeline columns centered on the anchor.
///
/// The window spans `nhours` columns starting at `anchor - nhours/2`.
/// Only the slot containing the live anchor (when `AnchorSpec::Now`) gets a
/// non-`None` `current_minute_offset`.
fn build_timeline_slots(
    nhours: u16,
    anchor: DateTime<Utc>,
    anchor_spec: &AnchorSpec,
) -> Vec<TimelineSlot> {
    let start_offset = -(i32::from(nhours) / 2);
    // Truncate anchor to the start of its minute for clean alignment
    let anchor_instant = anchor
        .with_second(0)
        .and_then(|value| value.with_nanosecond(0))
        .unwrap();

    (0..usize::from(nhours))
        .map(|index| {
            let offset_hours = start_offset + index as i32;
            TimelineSlot {
                offset_hours,
                start_utc: anchor_instant + Duration::hours(offset_hours as i64),
                current_minute_offset: match anchor_spec {
                    AnchorSpec::Now if offset_hours == 0 => Some(anchor.minute()),
                    _ => None,
                },
            }
        })
        .collect()
}

/// Build a minute-resolution bitmap classifying every minute in the timeline.
///
/// For each minute, we count how many zones have that instant inside their
/// core work window (`in_count`) and how many are reachable via core + shoulder
/// (`reach_count`).  The classification follows:
///
/// - `Ideal`:      `in_count == total_zones`
/// - `Feasible`:   `reach_count == total_zones` (but not all in core)
/// - `Partial(n)`: `reach_count == n` where `0 < n < total_zones`
/// - `None`:       `reach_count == 0`
fn build_minute_bitmap(
    zones: &[ResolvedZone],
    timeline_slots: &[TimelineSlot],
    shoulder_minutes: u16,
) -> Vec<MinuteClass> {
    let Some(first_slot) = timeline_slots.first() else {
        return Vec::new();
    };

    let start_utc = first_slot.start_utc;
    let total_minutes = timeline_slots.len() as i64 * 60;
    let total_zones = zones.len();
    let mut bitmap = Vec::with_capacity(total_minutes as usize);

    for minute in 0..total_minutes {
        let instant = start_utc + Duration::minutes(minute);

        if total_zones == 0 {
            bitmap.push(MinuteClass::None);
            continue;
        }

        // Count zones whose work window or shoulder contains this minute
        let mut in_count = 0usize;
        let mut reach_count = 0usize;

        for zone in zones {
            let m = zone.handle.minute_of_day(instant);
            let in_window = zone.window.contains(m);
            let in_shoulder = zone.window.shoulder_contains(m, shoulder_minutes);

            if in_window {
                in_count += 1;
                reach_count += 1;
            } else if in_shoulder {
                reach_count += 1;
            }
        }

        // Classify based on how many zones are in-window vs reachable
        let class = if in_count == total_zones {
            MinuteClass::Ideal
        } else if reach_count == total_zones {
            MinuteClass::Feasible
        } else if reach_count > 0 {
            MinuteClass::Partial(reach_count as u8)
        } else {
            MinuteClass::None
        };

        bitmap.push(class);
    }

    bitmap
}

/// Extract ranked meeting-window suggestions from the bitmap.
///
/// Strategy:
/// 1. Scan for contiguous `Ideal` runs and contiguous `Feasible` runs.
/// 2. Sort each list by (longest first, closest midpoint to anchor, earliest).
/// 3. If any Ideal or Feasible windows exist, return Ideal then Feasible.
/// 4. Otherwise, find the single best `Partial` run (highest zone count,
///    longest duration) and return it as a `LeastBad` window.
fn extract_classified_windows(
    bitmap: &[MinuteClass],
    start_utc: DateTime<Utc>,
    anchor: DateTime<Utc>,
    total_zones: usize,
) -> Vec<ClassifiedWindow> {
    let mut ideal_windows = Vec::new();
    let mut feasible_windows = Vec::new();

    // --- Pass 1: Extract contiguous Ideal runs ---
    let mut seg_start: Option<usize> = None;
    for (i, class) in bitmap.iter().enumerate() {
        match (seg_start, class) {
            (None, MinuteClass::Ideal) => seg_start = Some(i),
            (Some(_), MinuteClass::Ideal) => {}
            (Some(s), _) => {
                let seg_start_utc = start_utc + Duration::minutes(s as i64);
                let seg_end_utc = start_utc + Duration::minutes(i as i64);
                ideal_windows.push(ClassifiedWindow {
                    tier: WindowTier::Ideal,
                    start_utc: seg_start_utc,
                    end_utc: seg_end_utc,
                    duration_minutes: (i - s) as i64,
                    zones_in_window: total_zones,
                    total_zones,
                });
                seg_start = None;
            }
            _ => {}
        }
    }
    // Handle a run that extends to the end of the bitmap
    if let Some(s) = seg_start {
        let seg_start_utc = start_utc + Duration::minutes(s as i64);
        let seg_end_utc = start_utc + Duration::minutes(bitmap.len() as i64);
        ideal_windows.push(ClassifiedWindow {
            tier: WindowTier::Ideal,
            start_utc: seg_start_utc,
            end_utc: seg_end_utc,
            duration_minutes: (bitmap.len() - s) as i64,
            zones_in_window: total_zones,
            total_zones,
        });
    }

    // --- Pass 2: Extract contiguous Feasible runs ---
    seg_start = None;
    for (i, class) in bitmap.iter().enumerate() {
        match (seg_start, class) {
            (None, MinuteClass::Feasible) => seg_start = Some(i),
            (Some(_), MinuteClass::Feasible) => {}
            (Some(s), _) => {
                let seg_start_utc = start_utc + Duration::minutes(s as i64);
                let seg_end_utc = start_utc + Duration::minutes(i as i64);
                feasible_windows.push(ClassifiedWindow {
                    tier: WindowTier::Feasible,
                    start_utc: seg_start_utc,
                    end_utc: seg_end_utc,
                    duration_minutes: (i - s) as i64,
                    zones_in_window: total_zones,
                    total_zones,
                });
                seg_start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = seg_start {
        let seg_start_utc = start_utc + Duration::minutes(s as i64);
        let seg_end_utc = start_utc + Duration::minutes(bitmap.len() as i64);
        feasible_windows.push(ClassifiedWindow {
            tier: WindowTier::Feasible,
            start_utc: seg_start_utc,
            end_utc: seg_end_utc,
            duration_minutes: (bitmap.len() - s) as i64,
            zones_in_window: total_zones,
            total_zones,
        });
    }

    // --- Ranking: longest duration first, then closest midpoint to anchor, then earliest ---
    let sort_key = |w: &ClassifiedWindow| {
        let midpoint = w.start_utc + Duration::minutes(w.duration_minutes / 2);
        let distance = midpoint.signed_duration_since(anchor).num_minutes().abs();
        (Reverse(w.duration_minutes), distance, w.start_utc)
    };
    ideal_windows.sort_by_key(|w| sort_key(w));
    feasible_windows.sort_by_key(|w| sort_key(w));

    // Return Ideal + Feasible if any exist
    if !ideal_windows.is_empty() || !feasible_windows.is_empty() {
        let mut result = ideal_windows;
        result.extend(feasible_windows);
        return result;
    }

    // --- Fallback: find the single best Partial run as "LeastBad" ---
    // Track the best segment seen so far (highest zone count, then longest)
    let mut best_count: u8 = 0;
    let mut best_seg: Option<(usize, usize, u8)> = None; // (start_idx, end_idx, zone_count)
    let mut partial_start: Option<(usize, u8)> = None;

    // Append a sentinel None to flush any trailing segment
    for (i, class) in bitmap
        .iter()
        .chain(std::iter::once(&MinuteClass::None))
        .enumerate()
    {
        match (partial_start, class) {
            (None, MinuteClass::Partial(n)) => {
                partial_start = Some((i, *n));
            }
            (Some((_s, c)), MinuteClass::Partial(n)) if *n == c => {
                // Continue same-count segment
            }
            (Some((s, c)), _) => {
                // Segment ended — keep if it's the best so far
                if c > best_count
                    || (c == best_count && best_seg.is_none_or(|(bs, be, _)| (i - s) > (be - bs)))
                {
                    best_count = c;
                    best_seg = Some((s, i, c));
                }
                // Start new segment if current minute is also Partial
                if let MinuteClass::Partial(n) = class {
                    partial_start = Some((i, *n));
                } else {
                    partial_start = None;
                }
            }
            _ => {}
        }
    }

    if let Some((s, e, c)) = best_seg {
        let seg_start_utc = start_utc + Duration::minutes(s as i64);
        let seg_end_utc = start_utc + Duration::minutes(e as i64);
        return vec![ClassifiedWindow {
            tier: WindowTier::LeastBad,
            start_utc: seg_start_utc,
            end_utc: seg_end_utc,
            duration_minutes: (e - s) as i64,
            zones_in_window: c as usize,
            total_zones,
        }];
    }

    Vec::new()
}

/// Extract contiguous `Ideal` runs from the bitmap as [`OverlapSegment`]s.
///
/// Used by the summary bar to show "N hours of full overlap" indicators.
fn derive_overlap_segments(
    bitmap: &[MinuteClass],
    start_utc: DateTime<Utc>,
) -> Vec<OverlapSegment> {
    let mut segments = Vec::new();
    let mut seg_start: Option<usize> = None;

    for (i, class) in bitmap.iter().enumerate() {
        match (seg_start, class) {
            (None, MinuteClass::Ideal) => seg_start = Some(i),
            (Some(_), MinuteClass::Ideal) => {}
            (Some(s), _) => {
                let seg_start_utc = start_utc + Duration::minutes(s as i64);
                let seg_end_utc = start_utc + Duration::minutes(i as i64);
                segments.push(OverlapSegment {
                    start_utc: seg_start_utc,
                    end_utc: seg_end_utc,
                    duration_minutes: (i - s) as i64,
                });
                seg_start = None;
            }
            _ => {}
        }
    }

    // Flush trailing segment
    if let Some(s) = seg_start {
        let seg_start_utc = start_utc + Duration::minutes(s as i64);
        let seg_end_utc = start_utc + Duration::minutes(bitmap.len() as i64);
        segments.push(OverlapSegment {
            start_utc: seg_start_utc,
            end_utc: seg_end_utc,
            duration_minutes: (bitmap.len() - s) as i64,
        });
    }

    segments
}

/// Compute a display-order permutation for the zone rows.
///
/// Returns a `Vec<usize>` of indices into `zones` reflecting the requested
/// [`SortMode`].  For `Manual`, the original insertion order is preserved.
pub fn compute_display_order(
    zones: &[ResolvedZone],
    sort_mode: SortMode,
    now_utc: DateTime<Utc>,
) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..zones.len()).collect();
    match sort_mode {
        SortMode::UtcOffsetAsc => {
            indices.sort_by(|&a, &b| {
                let oa = zones[a].handle.utc_offset_seconds(now_utc);
                let ob = zones[b].handle.utc_offset_seconds(now_utc);
                oa.cmp(&ob).then_with(|| {
                    zones[a]
                        .label
                        .to_lowercase()
                        .cmp(&zones[b].label.to_lowercase())
                })
            });
        }
        SortMode::UtcOffsetDesc => {
            indices.sort_by(|&a, &b| {
                let oa = zones[a].handle.utc_offset_seconds(now_utc);
                let ob = zones[b].handle.utc_offset_seconds(now_utc);
                ob.cmp(&oa).then_with(|| {
                    zones[a]
                        .label
                        .to_lowercase()
                        .cmp(&zones[b].label.to_lowercase())
                })
            });
        }
        SortMode::LabelAsc => {
            indices.sort_by(|&a, &b| {
                zones[a]
                    .label
                    .to_lowercase()
                    .cmp(&zones[b].label.to_lowercase())
            });
        }
        SortMode::LabelDesc => {
            indices.sort_by(|&a, &b| {
                zones[b]
                    .label
                    .to_lowercase()
                    .cmp(&zones[a].label.to_lowercase())
            });
        }
        SortMode::Manual => {}
    }
    indices
}
