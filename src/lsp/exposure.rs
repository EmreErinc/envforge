use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::ops::canary::list_canaries;
use crate::ops::config_format::ConfigEntry;
use crate::ops::dotenv::is_sensitive_key;
use crate::ops::schema::EnvSchema;

use super::document::{EnvDocEntry, EnvLineType};

/// Per-line AI-exposure classification surfaced to IDE clients so they
/// can render a colored gutter glyph next to each env-var entry. The
/// three levels map to the visual contract: **green** = blocked from AI
/// agents by an active fence; **amber** = sensitive value, would be
/// redacted by AI-guard if exfiltrated through a hooked tool; **red** =
/// plaintext value, fully readable by any AI agent or scanning tool
/// that opens the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExposureLevel {
    Red,
    Amber,
    Green,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExposureEntry {
    pub line: u32,
    pub key: String,
    pub level: ExposureLevel,
    pub reason: String,
    /// True when this key currently has a registered canary. Plugins
    /// can render a distinct gutter glyph (e.g. a shield) for canary
    /// entries to make the tripwire visible.
    #[serde(default)]
    pub canary: bool,
}

/// Build a per-line exposure map for an `.env*` document. Only emits
/// entries for `EnvLineType::EnvVar` lines — comments, blanks, and
/// `Other` lines are skipped (no plausible AI-readable content there).
///
/// Classification precedence:
/// 1. `fence_active` is true (every fence file present and configured)
///    → `Green`. The reason explains that the workspace fence has
///    instructed all known AI agents to refuse reading `.env*`.
/// 2. Otherwise, if the entry is `sensitive` (per schema or
///    `is_sensitive_key` heuristic) → `Amber`. AI-guard will redact
///    these in tool inputs, but the file content itself is still on
///    disk in plaintext.
/// 3. Otherwise → `Red`. No protection. Any AI agent with read access
///    to the file sees the raw value.
pub fn compute_exposure_map(
    entries: &[EnvDocEntry],
    schema: Option<&EnvSchema>,
    fence_active: bool,
) -> Vec<ExposureEntry> {
    // Query the canary store once per request rather than per entry.
    // The store hits disk, so a single open is cheaper than N opens
    // even when no entries match.
    let canary_keys: HashSet<String> = list_canaries()
        .map(|cs| cs.into_iter().map(|c| c.key).collect())
        .unwrap_or_default();

    let mut out = Vec::new();
    for entry in entries {
        if entry.line_type != EnvLineType::EnvVar {
            continue;
        }
        let sensitive = schema
            .and_then(|s| s.variables.get(&entry.key))
            .map(|v| v.sensitive)
            .unwrap_or(false)
            || is_sensitive_key(&entry.key);

        let (level, mut reason) = if fence_active {
            (
                ExposureLevel::Green,
                "Fence active — AI agents instructed to refuse reads of this file.".to_string(),
            )
        } else if sensitive {
            (
                ExposureLevel::Amber,
                "Sensitive value — AI-guard will redact this in tool inputs, but plaintext lives on disk."
                    .to_string(),
            )
        } else {
            (
                ExposureLevel::Red,
                "Plaintext value, no fence — any AI agent reading this file sees the raw value."
                    .to_string(),
            )
        };

        let canary = canary_keys.contains(&entry.key);
        if canary {
            reason.push_str(
                " Canary tripwire registered — any exfiltration of this value triggers an alert.",
            );
        }

        out.push(ExposureEntry {
            line: entry.line,
            key: entry.key.clone(),
            level,
            reason,
            canary,
        });
    }
    out
}

/// Build a per-line exposure map for a recognized **framework config** document
/// (`.properties`, `.env`-cascade, `application.yml`, etc.).
///
/// Mirrors [`compute_exposure_map`] but accepts [`ConfigEntry`] slices produced
/// by the config-format parsers (unit 001/002). All three classification levels
/// (Green/Amber/Red) and canary annotation apply identically so config files
/// have **parity** with `.env` surfaces (FR20/AR7).
///
/// Only entries with a non-empty key are emitted; entries whose key is the
/// empty string (e.g. YAML comment pseudo-entries) are skipped.
pub fn compute_config_exposure_map(
    entries: &[ConfigEntry],
    schema: Option<&EnvSchema>,
    fence_active: bool,
) -> Vec<ExposureEntry> {
    // Query the canary store once per request, same as the .env path.
    let canary_keys: HashSet<String> = list_canaries()
        .map(|cs| cs.into_iter().map(|c| c.key).collect())
        .unwrap_or_default();

    let mut out = Vec::new();
    for entry in entries {
        if entry.key.is_empty() {
            continue;
        }

        let sensitive = schema
            .and_then(|s| s.variables.get(&entry.key))
            .map(|v| v.sensitive)
            .unwrap_or(false)
            || is_sensitive_key(&entry.key);

        let (level, mut reason) = if fence_active {
            (
                ExposureLevel::Green,
                "Fence active — AI agents instructed to refuse reads of this file.".to_string(),
            )
        } else if sensitive {
            (
                ExposureLevel::Amber,
                "Sensitive value — AI-guard will redact this in tool inputs, but plaintext lives on disk."
                    .to_string(),
            )
        } else {
            (
                ExposureLevel::Red,
                "Plaintext value, no fence — any AI agent reading this file sees the raw value."
                    .to_string(),
            )
        };

        let canary = canary_keys.contains(&entry.key);
        if canary {
            reason.push_str(
                " Canary tripwire registered — any exfiltration of this value triggers an alert.",
            );
        }

        out.push(ExposureEntry {
            line: entry.line,
            key: entry.key.clone(),
            level,
            reason,
            canary,
        });
    }
    out
}
