//! Profile/cascade layer precedence + `${VAR:default}` interpolation engine
//! — Story 003 (FR15, FR16, NFR16).
//!
//! Two pure (no I/O, no LSP types) functions:
//!
//! 1. `resolve_layers` — picks the winning `ConfigEntry` for a given key from
//!    multiple layers, following the documented precedence rules:
//!    - Spring: `base` < `profile`
//!    - `.env` cascade: `.env` < `.env.local` < `.env.{environment}`
//!
//! 2. `interpolate_value` — expands `${VAR}`, `${VAR:default}` references
//!    inside a value string using a flat key→value map. Detects cycles
//!    (stops after visiting each variable once) so circular references
//!    never loop forever.

use std::collections::{HashMap, HashSet};

use super::config_format::{resolve_ref, ConfigEntry, ResolvedValue};

// ── Layer resolution ─────────────────────────────────────────────────────────

/// Return the effective `ConfigEntry` for `key` across all `layers`.
///
/// Each element of `layers` is a parsed file (a `Vec<ConfigEntry>`). The
/// entry with the highest `SourceLayer::precedence()` wins. When two layers
/// have equal precedence, the later one in the slice wins (last-write wins,
/// matching shell semantics).
///
/// Returns `None` when the key is absent from all layers.
pub fn resolve_layers<'a>(key: &str, layers: &'a [Vec<ConfigEntry>]) -> Option<&'a ConfigEntry> {
    let mut winner: Option<&'a ConfigEntry> = None;
    let mut best_prec: u8 = 0;
    let mut best_idx: usize = 0; // layer index for tie-breaking

    for (layer_idx, layer_entries) in layers.iter().enumerate() {
        for entry in layer_entries {
            if entry.key != key {
                continue;
            }
            let prec = entry.source_layer.precedence();
            // Higher precedence always wins; equal precedence → later layer wins.
            if winner.is_none() || prec > best_prec || (prec == best_prec && layer_idx >= best_idx)
            {
                winner = Some(entry);
                best_prec = prec;
                best_idx = layer_idx;
            }
        }
    }

    winner
}

/// Resolve the effective value for `key` across `layers` AND apply
/// `${VAR:default}` interpolation using all keys visible in `layers` as the
/// environment. Returns `None` when the key is absent from all layers.
pub fn resolve_effective_value(key: &str, layers: &[Vec<ConfigEntry>]) -> Option<ResolvedValue> {
    let entry = resolve_layers(key, layers)?;

    // Build a flat key→value map from all layers (lower precedence layers
    // first so higher-precedence entries overwrite them).
    let mut env: HashMap<String, String> = HashMap::new();
    for layer in layers {
        for e in layer {
            if !e.key.is_empty() {
                env.insert(e.key.clone(), e.value.clone());
            }
        }
    }

    let (resolved, interpolated) = interpolate_value(&entry.value, &env, &mut HashSet::new());

    Some(ResolvedValue {
        value: resolved,
        winning_layer: entry.source_layer.clone(),
        interpolated,
    })
}

// ── Interpolation ─────────────────────────────────────────────────────────────

/// Maximum recursion depth for `interpolate_value`. Beyond this depth the raw
/// string is returned unresolved to prevent stack overflow on long
/// `${A}->${B}->...` chains (M-C fix).
const MAX_INTERPOLATION_DEPTH: usize = 100;

/// Expand all `${VAR}` and `${VAR:default}` references in `raw` using `env`.
///
/// Cycle detection: `visited` tracks the set of variable names currently
/// being expanded. When a cycle is detected (a variable references itself
/// directly or transitively) the expansion stops and the original reference
/// is left in the output unchanged.
///
/// Depth cap: when `visited.len() >= MAX_INTERPOLATION_DEPTH` the raw string
/// is returned immediately (M-C). This prevents stack overflow on pathological
/// long chains that are not cycles but are deeply nested.
///
/// Returns `(expanded_string, any_expansion_occurred)`.
pub fn interpolate_value(
    raw: &str,
    env: &HashMap<String, String>,
    visited: &mut HashSet<String>,
) -> (String, bool) {
    // M-C: depth cap — return raw string unresolved if too deep.
    if visited.len() >= MAX_INTERPOLATION_DEPTH {
        return (raw.to_string(), false);
    }
    let mut result = String::with_capacity(raw.len());
    let mut any_expanded = false;
    let mut remaining = raw;

    while let Some(start) = remaining.find("${") {
        result.push_str(&remaining[..start]);
        remaining = &remaining[start + 2..]; // skip `${`

        // Find the closing `}`.
        let Some(end) = remaining.find('}') else {
            // Unterminated `${` — leave as-is (diagnostics will flag it).
            result.push_str("${");
            // Continue scanning for more references after the `${`.
            continue;
        };

        let inner = &remaining[..end];
        remaining = &remaining[end + 1..]; // skip `}`

        // Split on `:` to extract default.
        let (var_name, default) = if let Some(colon) = inner.find(':') {
            (&inner[..colon], Some(&inner[colon + 1..]))
        } else {
            (inner, None)
        };

        if var_name.is_empty() {
            // `${}` — leave as-is.
            result.push_str("${}");
            continue;
        }

        // Cycle guard.
        if visited.contains(var_name) {
            // Reconstruct the original reference.
            let original = if let Some(d) = default {
                format!("${{{}:{}}}", var_name, d)
            } else {
                format!("${{{}}}", var_name)
            };
            result.push_str(&original);
            continue;
        }

        let (val, resolved) = resolve_ref(var_name, default, env);

        if resolved {
            // Recursively expand the resolved value in case it also contains
            // `${...}` references, with the current var added to visited.
            visited.insert(var_name.to_string());
            let (expanded, _) = interpolate_value(&val, env, visited);
            visited.remove(var_name);
            result.push_str(&expanded);
            any_expanded = true;
        } else {
            // Unresolved — emit as-is.
            result.push_str(&val);
        }
    }

    result.push_str(remaining);
    (result, any_expanded)
}

/// Check whether `raw` contains any `${` that is not properly terminated.
/// Returns a list of character positions (UTF-16 offsets) where unterminated
/// references start — used by the diagnostics engine (Story 008).
///
/// The position returned is the column of the `$` character (0-indexed UTF-16).
pub fn find_unterminated_refs(raw: &str) -> Vec<u32> {
    let mut positions = Vec::new();
    let mut chars = raw.char_indices().peekable();
    let mut col: u32 = 0;

    while let Some((byte_pos, ch)) = chars.next() {
        if ch == '$' {
            if let Some((_, '{')) = chars.peek() {
                // Consume the `{`.
                chars.next();
                // `start_col` is the column of `$` — correct (no off-by-one).
                let start_col = col;
                col += 1; // for `{`
                let rest = &raw[byte_pos + 2..]; // after `${`
                if !rest.contains('}') {
                    // Return the column of `$` directly (was: saturating_sub(1)+1
                    // which produced wrong results at col 0).
                    positions.push(start_col);
                }
            }
        }
        col += ch.len_utf16() as u32;
    }

    positions
}

// ── Diagnostics helpers ───────────────────────────────────────────────────────

/// Returns `true` if `value` contains at least one `${VAR}` interpolation
/// reference (terminated or not). Used by the diagnostics engine.
pub fn contains_interpolation(value: &str) -> bool {
    value.contains("${")
}
