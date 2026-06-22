//! Cross-format schema unification — Intent 040, Stories 001–004.
//!
//! ## Summary
//!
//! This module provides a format-agnostic key normalization layer that maps
//! concrete keys from any recognized config format to a single *canonical key*,
//! and then looks that canonical key up in the unified `.env.schema`.
//!
//! ## Canonical key rules (Story 001 + 004)
//!
//! The canonical key is **lowercase** and uses **dots** as the only separator.
//! Before converting to lowercase the following transformations are applied:
//!
//! 1. **Separator unification** — `:`, `__`, and `-` are all treated as
//!    equivalent to `.` (dots).  A run of any combination of these separators
//!    collapses to a single `.`.
//! 2. **UPPER_SNAKE decomposition** — an all-uppercase sequence that contains
//!    `_` (the `.env` convention, e.g. `SPRING_DATASOURCE_URL`) is split on
//!    `_` and the parts are joined with `.`.
//! 3. **camelCase / PascalCase splitting** — `myKey` or `MyKey` is split at
//!    ASCII uppercase-letter boundaries so `myKey` → `my.key`.
//! 4. **Spring relaxed binding** — the Spring Boot relaxed-binding rules mean
//!    `spring.datasource.url`, `SPRING_DATASOURCE_URL`, `spring-datasource-url`,
//!    `spring__datasource__url`, and `springDatasourceUrl` all collapse to the
//!    same canonical key `spring.datasource.url`.
//!
//! ## Format-key conventions (informs the normalizer)
//!
//! | Format | Key convention | Example |
//! |--------|---------------|---------|
//! | `.env` / `.env.local` | UPPER_SNAKE | `SPRING_DATASOURCE_URL` |
//! | `.properties` / YAML | dotted lower | `spring.datasource.url` |
//! | TOML | dotted lower or `[table]` | `spring.datasource.url` |
//! | JSONC / appsettings | `:` path (PascalCase segments) | `Spring:Datasource:Url` |
//!
//! ## AI-safety note
//!
//! Sensitivity is preserved across formats: a key marked `sensitive = true` in
//! `.env.schema` is redacted in hover for *every* format that maps to the same
//! canonical key.
//!
//! ## No-panic guarantee
//!
//! All functions accept `&str` inputs and return `None` / empty collections
//! rather than panicking on malformed or empty keys.

use std::collections::HashMap;

use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticSeverity, Location, Position, Range as LspRange, Url,
};

use crate::ops::config_format::ConfigEntry;
use crate::ops::schema::{EnvSchema, SchemaVariable, VarType};

// ── Story 001 + 004: Key normalization ───────────────────────────────────────

/// Normalize any concrete config key to a canonical dot-separated lowercase key.
///
/// The canonical key is the format-agnostic identity used for cross-format
/// goto-definition and find-references navigation (Spring relaxed-binding rules).
///
/// # Rules (applied in order)
///
/// 1. Trim whitespace.
/// 2. Replace `:` (JSONC / .NET path separator) with `.`.
/// 3. Replace `__` (Spring double-underscore) with `.`.
/// 4. If the key is entirely uppercase ASCII + `_` (UPPER_SNAKE) then replace
///    `_` with `.`.
/// 5. Otherwise split camelCase / PascalCase on uppercase-to-lowercase
///    boundaries and insert `.`.
/// 6. Replace remaining `-` with `.`.
/// 7. Collapse runs of `.` to a single `.`.
/// 8. Strip leading/trailing `.`.
/// 9. Convert to lowercase.
///
/// Returns `None` when the resulting canonical key is empty.
///
/// # WARNING: over-collapse
///
/// This function is intentionally **relaxed** for goto-definition / find-references
/// where extra cross-format matches are low-harm. Do **NOT** use it for
/// sensitivity checks or unknown-key diagnostics — use [`canonical_key_strict`]
/// instead to avoid false positives (e.g. `line-length` ≢ `LINE_LENGTH`).
///
/// # Examples
///
/// ```
/// use envforge::ops::schema_unification::canonical_key;
///
/// assert_eq!(canonical_key("SPRING_DATASOURCE_URL").as_deref(), Some("spring.datasource.url"));
/// assert_eq!(canonical_key("spring.datasource.url").as_deref(), Some("spring.datasource.url"));
/// assert_eq!(canonical_key("Spring:Datasource:Url").as_deref(), Some("spring.datasource.url"));
/// assert_eq!(canonical_key("spring-datasource-url").as_deref(), Some("spring.datasource.url"));
/// assert_eq!(canonical_key("springDatasourceUrl").as_deref(), Some("spring.datasource.url"));
/// assert_eq!(canonical_key("spring__datasource__url").as_deref(), Some("spring.datasource.url"));
/// assert_eq!(canonical_key("").as_deref(), None);
/// ```
pub fn canonical_key(key: &str) -> Option<String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Step 1: replace `:` with `.`.
    let s = trimmed.replace(':', ".");

    // Step 2: replace `__` with `.` (Spring double-underscore binding).
    let s = replace_double_underscore(&s);

    // Step 3: determine if this is UPPER_SNAKE (all uppercase + digits + `_`).
    let is_upper_snake = s
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_' || c == '.');

    let s = if is_upper_snake {
        // Replace `_` with `.` (UPPER_SNAKE → dotted).
        s.replace('_', ".")
    } else {
        // Split camelCase / PascalCase at uppercase boundaries.
        split_camel_case(&s)
    };

    // Step 4: replace `-` with `.`.
    let s = s.replace('-', ".");

    // Step 5: collapse runs of `.` to a single `.`.
    let s = collapse_dots(&s);

    // Step 6: strip leading/trailing `.` and convert to lowercase.
    let s = s.trim_matches('.').to_lowercase();

    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Strict canonical key normalization for **sensitivity checks and unknown-key
/// diagnostics** — high-precision, no false positives.
///
/// Only the *path-separator class* equivalences are applied:
/// - `:` (JSONC / .NET) → `.`
/// - `__` (Spring double-underscore) → `.`
/// - UPPER_SNAKE `_` → `.` (only when the entire key is UPPER_SNAKE)
/// - Case-folded to lowercase
///
/// The **dangerous** collapses that `canonical_key` applies are deliberately
/// **omitted** here:
/// - `-` is NOT treated as a separator (so `line-length` ≢ `line.length`).
/// - Single `_` in mixed-case keys is NOT collapsed.
/// - camelCase / PascalCase splitting is NOT applied.
///
/// This means `LINE_LENGTH` (UPPER_SNAKE) still maps to `line.length`, and
/// `Logging:LogLevel` (JSONC path) still maps to `logging.loglevel`, but
/// `line-length` (TOML bare key) stays as `line-length` (lowercase) and does
/// NOT alias `LINE_LENGTH`.
///
/// Returns `None` when the resulting canonical key is empty.
///
/// # Examples
///
/// ```
/// use envforge::ops::schema_unification::canonical_key_strict;
///
/// // Path-separator equivalence is preserved:
/// assert_eq!(canonical_key_strict("LOGGING__LOGLEVEL__DEFAULT").as_deref(),
///            Some("logging.loglevel.default"));
/// assert_eq!(canonical_key_strict("Logging:LogLevel:Default").as_deref(),
///            Some("logging.loglevel.default"));
/// assert_eq!(canonical_key_strict("LOGGING_LOGLEVEL_DEFAULT").as_deref(),
///            Some("logging.loglevel.default"));
///
/// // Hyphens are NOT collapsed (H-2 fix):
/// assert_ne!(canonical_key_strict("line-length").as_deref(),
///            canonical_key_strict("LINE_LENGTH").as_deref());
/// assert_eq!(canonical_key_strict("line-length").as_deref(), Some("line-length"));
/// assert_eq!(canonical_key_strict("LINE_LENGTH").as_deref(), Some("line.length"));
/// ```
pub fn canonical_key_strict(key: &str) -> Option<String> {
    let trimmed = key.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Step 1: replace `:` with `.`.
    let s = trimmed.replace(':', ".");

    // Step 2: replace `__` with `.` (Spring double-underscore binding).
    let s = replace_double_underscore(&s);

    // Step 3: determine if this is UPPER_SNAKE (all uppercase + digits + `_`).
    let is_upper_snake = s
        .chars()
        .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_' || c == '.');

    // Only split UPPER_SNAKE `_` into path separators. Do NOT apply
    // camelCase splitting, `-` collapsing, or single-`_` collapsing for
    // mixed-case keys — those cause false sensitivity/unknown-key matches.
    let s = if is_upper_snake {
        s.replace('_', ".")
    } else {
        s
    };

    // Step 4: collapse runs of `.` to a single `.`.
    let s = collapse_dots(&s);

    // Step 5: strip leading/trailing `.` and convert to lowercase.
    let s = s.trim_matches('.').to_lowercase();

    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Replace all occurrences of `__` with `.` iteratively until stable.
fn replace_double_underscore(s: &str) -> String {
    let mut result = s.to_string();
    while result.contains("__") {
        result = result.replace("__", ".");
    }
    result
}

/// Split a camelCase / PascalCase string at uppercase-to-lowercase boundaries,
/// inserting `.` before each run that starts with an uppercase letter that is
/// preceded by a lowercase letter or digit.
///
/// Examples:
/// - `springDatasourceUrl` → `spring.Datasource.Url`
/// - `SpringDatasourceUrl` → `Spring.Datasource.Url`
/// - `myHTTPClient` → `my.H.T.T.P.Client` (each uppercase run is a segment)
///
/// NOTE: after this function the caller converts to lowercase, so the result
/// need not be pretty — it just needs to produce the right segments.
fn split_camel_case(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(s.len() + 8);
    out.push(chars[0]);
    for i in 1..chars.len() {
        let prev = chars[i - 1];
        let cur = chars[i];
        // Insert `.` when transitioning from a lowercase/digit to an uppercase letter.
        if cur.is_ascii_uppercase() && (prev.is_ascii_lowercase() || prev.is_ascii_digit()) {
            out.push('.');
        }
        out.push(cur);
    }
    out
}

/// Collapse consecutive `.` characters into a single `.`.
fn collapse_dots(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_was_dot = false;
    for c in s.chars() {
        if c == '.' {
            if !last_was_dot {
                out.push('.');
            }
            last_was_dot = true;
        } else {
            out.push(c);
            last_was_dot = false;
        }
    }
    out
}

// ── Story 001: UnifiedSchema ──────────────────────────────────────────────────

/// Format-agnostic view over a `.env.schema`.
///
/// Provides O(1) lookup from a concrete key in any format to its
/// `SchemaVariable`.
///
/// Two indices are maintained (H-2 fix):
///
/// - **`strict_index`**: built with [`canonical_key_strict`] (path-separator +
///   UPPER_SNAKE only). Used for **sensitivity checks** and **unknown-key
///   diagnostics** where false positives are harmful (e.g. `line-length` must
///   not alias `LINE_LENGTH`).
///
/// - **`relaxed_index`**: built with [`canonical_key`] (Spring relaxed-binding,
///   includes camelCase split and `-` collapse). Used for **goto-definition**
///   and **find-references** where extra cross-format matches are low-harm.
///
/// `lookup` uses the strict index; `lookup_relaxed` uses the relaxed index.
#[derive(Debug, Clone)]
pub struct UnifiedSchema {
    /// The raw schema (keyed by UPPER_SNAKE names as stored in `.env.schema`).
    inner: EnvSchema,
    /// Map from *strict canonical key* → UPPER_SNAKE schema key.
    /// Used for sensitivity checks and unknown-key diagnostics (H-2 fix).
    strict_index: HashMap<String, String>,
    /// Map from *relaxed canonical key* → UPPER_SNAKE schema key.
    /// Used for goto-definition and find-references.
    relaxed_index: HashMap<String, String>,
}

impl UnifiedSchema {
    /// Build a `UnifiedSchema` from a parsed `EnvSchema`.
    ///
    /// Both the strict and relaxed indices are built at construction time.
    /// If two schema variable names happen to produce the same canonical key the
    /// last one wins (both map to the same logical configuration concept).
    pub fn new(schema: EnvSchema) -> Self {
        let mut strict_index: HashMap<String, String> = HashMap::new();
        let mut relaxed_index: HashMap<String, String> = HashMap::new();
        for name in schema.variables.keys() {
            if let Some(ck) = canonical_key_strict(name) {
                strict_index.insert(ck, name.clone());
            }
            if let Some(ck) = canonical_key(name) {
                relaxed_index.insert(ck, name.clone());
            }
        }
        Self {
            inner: schema,
            strict_index,
            relaxed_index,
        }
    }

    /// Look up a `SchemaVariable` by a concrete key from any format.
    ///
    /// Uses the **strict** canonical key (path-separator equivalence only).
    /// This is the correct lookup for sensitivity checks and unknown-key
    /// diagnostics — it avoids false positives from camelCase / `-` collapse.
    ///
    /// Returns `None` when:
    /// - The key cannot be normalized (empty or all-whitespace).
    /// - No schema variable maps to the resulting canonical key.
    pub fn lookup(&self, concrete_key: &str) -> Option<&SchemaVariable> {
        let ck = canonical_key_strict(concrete_key)?;
        let schema_key = self.strict_index.get(&ck)?;
        self.inner.variables.get(schema_key)
    }

    /// Look up a `SchemaVariable` using the **relaxed** canonical key.
    ///
    /// Uses Spring relaxed-binding rules (camelCase split, `-` collapse).
    /// Intended for goto-definition and find-references where extra matches
    /// are acceptable. Use [`lookup`][Self::lookup] for sensitivity / diagnostics.
    pub fn lookup_relaxed(&self, concrete_key: &str) -> Option<&SchemaVariable> {
        let ck = canonical_key(concrete_key)?;
        let schema_key = self.relaxed_index.get(&ck)?;
        self.inner.variables.get(schema_key)
    }

    /// Look up the schema variable name (UPPER_SNAKE) for a concrete key.
    ///
    /// Uses the strict index.
    pub fn schema_key_for(&self, concrete_key: &str) -> Option<&str> {
        let ck = canonical_key_strict(concrete_key)?;
        self.strict_index.get(&ck).map(String::as_str)
    }

    /// Return the underlying `EnvSchema`.
    pub fn inner(&self) -> &EnvSchema {
        &self.inner
    }

    /// Iterate all (schema_key, SchemaVariable) pairs.
    pub fn variables(&self) -> impl Iterator<Item = (&str, &SchemaVariable)> {
        self.inner.variables.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Check whether this schema is empty (no variables defined).
    pub fn is_empty(&self) -> bool {
        self.inner.variables.is_empty()
    }
}

impl From<EnvSchema> for UnifiedSchema {
    fn from(schema: EnvSchema) -> Self {
        Self::new(schema)
    }
}

// ── Story 002: Cross-format diagnostics ──────────────────────────────────────

/// Kind of a cross-format schema diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrossFormatDiagnosticKind {
    /// The key is not present in the schema.
    UnknownKey,
    /// The value's type contradicts the schema's declared type.
    TypeMismatch {
        expected: String,
        actual_value: String,
    },
    /// A required key is absent from all formats in the provided entry set.
    MissingRequired,
}

/// A single cross-format diagnostic finding.
#[derive(Debug, Clone)]
pub struct CrossFormatDiagnostic {
    /// The concrete key as it appears in the config file.
    pub concrete_key: String,
    /// The canonical key that was looked up in the schema.
    pub canonical: String,
    /// The kind of diagnostic.
    pub kind: CrossFormatDiagnosticKind,
    /// The LSP range covering the key (or value for type-mismatch).
    pub range: LspRange,
}

/// Compute unknown-key and type-mismatch diagnostics for a slice of `ConfigEntry`
/// values against the unified schema.
///
/// Returns one `CrossFormatDiagnostic` per offending entry. Call this once per
/// open document; call `missing_required_diagnostics` once across all documents.
///
/// # Behaviour
///
/// - **Unknown key**: key is present in the file but has no matching schema entry
///   (after strict canonical normalization — H-1/H-2 fix). Emits
///   `CrossFormatDiagnosticKind::UnknownKey` with the entry's `key_range`.
/// - **Type mismatch**: the schema entry has a declared `var_type` and the value
///   in the file is demonstrably incompatible (e.g. `"hello"` for a `number`
///   field). Emits `CrossFormatDiagnosticKind::TypeMismatch` with the entry's
///   `value_range`.
///
/// Uses [`canonical_key_strict`] so that TOML `line-length` is NOT aliased to
/// schema key `LINE_LENGTH` (H-2 fix), and JSONC `Logging:LogLevel:Default`
/// correctly matches schema `LOGGING__LOGLEVEL__DEFAULT` via path-separator
/// unification (H-1 fix).
///
/// Empty keys (blank lines, comments) are silently skipped.
/// No panic on any input.
pub fn cross_format_entry_diagnostics(
    entries: &[ConfigEntry],
    schema: &UnifiedSchema,
) -> Vec<CrossFormatDiagnostic> {
    let mut diags = Vec::new();

    for entry in entries {
        if entry.key.is_empty() {
            continue;
        }

        // H-1/H-2: use strict canonical key for unknown-key / type-mismatch
        // diagnostics (path-separator equivalence only, no `-`/camelCase collapse).
        let ck = match canonical_key_strict(&entry.key) {
            Some(k) => k,
            None => continue,
        };

        match schema.lookup(&entry.key) {
            None => {
                diags.push(CrossFormatDiagnostic {
                    concrete_key: entry.key.clone(),
                    canonical: ck,
                    kind: CrossFormatDiagnosticKind::UnknownKey,
                    range: entry.key_range,
                });
            }
            Some(var_def) => {
                if !check_value_type_vs_schema(&entry.value, &var_def.var_type) {
                    diags.push(CrossFormatDiagnostic {
                        concrete_key: entry.key.clone(),
                        canonical: ck,
                        kind: CrossFormatDiagnosticKind::TypeMismatch {
                            expected: var_def.var_type.display().to_string(),
                            actual_value: entry.value.clone(),
                        },
                        range: entry.value_range,
                    });
                }
            }
        }
    }

    diags
}

/// Compute missing-required diagnostics across *all* provided entry sets.
///
/// A required schema key is *not* considered missing when at least one entry
/// across any of the provided sets maps to it via canonical normalization.
/// This implements the FR requirement: "Required key present in `.env` but not
/// YAML → not missing-required (satisfied by any format)".
///
/// Returns one diagnostic per missing-required schema variable. The diagnostic
/// has a zero-width range at position (0, 0) (document-level); callers may
/// attach it to the document or display it as a file-level warning.
///
/// No panic on any input; returns empty when schema has no `required` variables.
pub fn missing_required_diagnostics(
    all_entries: &[&[ConfigEntry]],
    schema: &UnifiedSchema,
) -> Vec<CrossFormatDiagnostic> {
    // Build the set of strict canonical keys present across all formats.
    // Use canonical_key_strict so the presence check is consistent with the
    // unknown-key / sensitivity checks (H-2 fix: no false positives from
    // `-`/camelCase collapse).
    let mut present_canonicals: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    for entries in all_entries {
        for entry in *entries {
            if entry.key.is_empty() {
                continue;
            }
            if let Some(ck) = canonical_key_strict(&entry.key) {
                present_canonicals.insert(ck);
            }
        }
    }

    let mut diags = Vec::new();
    for (schema_key, var_def) in schema.variables() {
        if !var_def.required {
            continue;
        }
        // Check if the schema key itself (UPPER_SNAKE) or any of its canonical
        // variants is present.
        let schema_ck = match canonical_key_strict(schema_key) {
            Some(k) => k,
            None => continue,
        };
        if !present_canonicals.contains(&schema_ck) {
            diags.push(CrossFormatDiagnostic {
                concrete_key: schema_key.to_string(),
                canonical: schema_ck,
                kind: CrossFormatDiagnosticKind::MissingRequired,
                range: LspRange {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 0,
                        character: 0,
                    },
                },
            });
        }
    }

    // Sort for determinism (by schema_key).
    diags.sort_by(|a, b| a.concrete_key.cmp(&b.concrete_key));
    diags
}

/// Convert `CrossFormatDiagnostic` to `tower_lsp::lsp_types::Diagnostic`.
///
/// The `source` field is always `"envforge"` for consistency with all other
/// diagnostic producers in this codebase.
pub fn cross_format_diagnostics_to_lsp(diags: &[CrossFormatDiagnostic]) -> Vec<Diagnostic> {
    diags
        .iter()
        .map(|d| {
            let (severity, message) = match &d.kind {
                CrossFormatDiagnosticKind::UnknownKey => (
                    DiagnosticSeverity::WARNING,
                    format!(
                        "Unknown key '{}' (not in schema; canonical: '{}')",
                        d.concrete_key, d.canonical
                    ),
                ),
                CrossFormatDiagnosticKind::TypeMismatch {
                    expected,
                    actual_value,
                } => (
                    DiagnosticSeverity::WARNING,
                    format!(
                        "Type mismatch for '{}': schema expects '{}', got '{}'",
                        d.concrete_key, expected, actual_value
                    ),
                ),
                CrossFormatDiagnosticKind::MissingRequired => (
                    DiagnosticSeverity::WARNING,
                    format!(
                        "Required key '{}' is absent from all config files",
                        d.concrete_key
                    ),
                ),
            };
            Diagnostic {
                range: d.range,
                severity: Some(severity),
                source: Some("envforge".into()),
                message,
                ..Default::default()
            }
        })
        .collect()
}

/// Heuristic type check for a string value against a schema `VarType`.
///
/// Returns `true` when the value is *compatible* with the expected type.
/// Returns `false` only on clear mismatches. When in doubt, returns `true`
/// (conservative — avoids false positives on partial / placeholder values).
fn check_value_type_vs_schema(value: &str, var_type: &VarType) -> bool {
    match var_type {
        VarType::Bool => matches!(
            value.to_lowercase().as_str(),
            "true" | "false" | "1" | "0" | "yes" | "no"
        ),
        VarType::Number => value.parse::<f64>().is_ok(),
        VarType::Port => value.parse::<u16>().map(|p| p > 0).unwrap_or(false),
        VarType::Url => {
            value.contains("://")
                || value.starts_with("http")
                || value.starts_with("postgres")
                || value.starts_with("mysql")
                || value.starts_with("redis")
        }
        VarType::Email => value.contains('@') && value.contains('.'),
        // Enum, Regex, String — always compatible (conservative).
        VarType::Enum | VarType::Regex | VarType::String => true,
    }
}

// ── Story 003: Cross-format go-to-definition & find-references ────────────────

/// Resolve a concrete key (in any format) to its schema definition location
/// AND to all concrete definitions of the same logical key across all open
/// config documents.
///
/// # Priority order (matching the existing single-format behaviour)
///
/// 1. Schema file location (via `schema_line_map`).
/// 2. All concrete definitions across `open_docs`, sorted by (URI, line) for
///    determinism.
///
/// # Cache reuse
///
/// `open_docs` is the already-parsed `config_documents` map from `Backend`.
/// This function never re-parses files — it only iterates the cached entries,
/// satisfying NFR3 (no per-keystroke re-scan).
///
/// M-4: a per-request `HashMap<String, Option<String>>` caches canonical key
/// computations so each unique key string is normalized at most once per call.
///
/// Returns an empty `Vec` when the key cannot be normalized or no locations are
/// found.
pub fn cross_format_goto_definition(
    concrete_key: &str,
    schema_uri: Option<&Url>,
    schema_line_map: &HashMap<String, u32>,
    open_docs: &HashMap<Url, Vec<ConfigEntry>>,
) -> Vec<Location> {
    let ck = match canonical_key(concrete_key) {
        Some(k) => k,
        None => return Vec::new(),
    };

    // M-4: per-request canonical-key memo cache — avoids repeated allocation
    // for the same key string when scanning large open_docs maps.
    let mut ck_cache: HashMap<String, Option<String>> = HashMap::new();
    let cached_ck = |key: &str, cache: &mut HashMap<String, Option<String>>| -> Option<String> {
        if let Some(cached) = cache.get(key) {
            return cached.clone();
        }
        let result = canonical_key(key);
        cache.insert(key.to_string(), result.clone());
        result
    };

    let mut locations: Vec<Location> = Vec::new();

    // 1. Schema definition: look up by the canonical key against the
    //    schema_line_map which is keyed by UPPER_SNAKE names.
    // Try all schema_line_map keys whose canonical form matches.
    if let Some(uri) = schema_uri {
        // First try exact match (UPPER_SNAKE key coming from .env).
        if let Some(&line) = schema_line_map.get(concrete_key) {
            let key_utf16_len: u32 = concrete_key.chars().map(|c| c.len_utf16() as u32).sum();
            locations.push(Location {
                uri: uri.clone(),
                range: LspRange {
                    start: Position { line, character: 0 },
                    end: Position {
                        line,
                        character: key_utf16_len + 2,
                    },
                },
            });
        } else {
            // Try all schema entries — find one whose canonical key matches ours.
            let mut schema_match: Vec<(&String, &u32)> = schema_line_map
                .iter()
                .filter(|(k, _)| canonical_key(k).as_deref() == Some(ck.as_str()))
                .collect();
            schema_match.sort_by_key(|(k, _)| k.as_str());
            // Take only the first matching schema entry.
            if let Some((schema_key, &line)) = schema_match.into_iter().next() {
                let key_utf16_len: u32 = schema_key.chars().map(|c| c.len_utf16() as u32).sum();
                locations.push(Location {
                    uri: uri.clone(),
                    range: LspRange {
                        start: Position { line, character: 0 },
                        end: Position {
                            line,
                            character: key_utf16_len + 2,
                        },
                    },
                });
            }
        }
    }

    // 2. Concrete definitions across all open documents.
    // Sort URIs for determinism.
    let mut sorted_docs: Vec<(&Url, &Vec<ConfigEntry>)> = open_docs.iter().collect();
    sorted_docs.sort_by_key(|(u, _)| u.as_str());

    for (uri, entries) in &sorted_docs {
        for entry in *entries {
            if entry.key.is_empty() {
                continue;
            }
            // M-4: use per-request cache to avoid recomputing canonical_key.
            if cached_ck(&entry.key, &mut ck_cache).as_deref() == Some(ck.as_str()) {
                locations.push(Location {
                    uri: (*uri).clone(),
                    range: entry.key_range,
                });
            }
        }
    }

    // Final sort by (uri, line) for determinism (dedup schema entry if already
    // present in open_docs).
    locations.sort_by(|a, b| {
        a.uri
            .as_str()
            .cmp(b.uri.as_str())
            .then_with(|| a.range.start.line.cmp(&b.range.start.line))
    });
    locations.dedup_by(|a, b| a.uri == b.uri && a.range.start.line == b.range.start.line);

    locations
}

/// Find every reference to a concrete key (in any format) across all open
/// config documents.
///
/// Uses canonical-key matching so `SPRING_DATASOURCE_URL`, `spring.datasource.url`,
/// and `Spring:Datasource:Url` are all considered references to the same logical
/// key.
///
/// When `include_declaration` is `true`, the schema file entry is included.
///
/// # Cache reuse
///
/// Same as `cross_format_goto_definition`: iterates cached entries from
/// `open_docs` — no file I/O, no re-parsing.
///
/// M-4: a per-request `HashMap<String, Option<String>>` caches canonical key
/// computations so each unique key string is normalized at most once per call.
///
/// Returns a sorted (by URI, then line) `Vec<Location>`.
pub fn cross_format_find_references(
    concrete_key: &str,
    schema_uri: Option<&Url>,
    schema_line_map: &HashMap<String, u32>,
    open_docs: &HashMap<Url, Vec<ConfigEntry>>,
    include_declaration: bool,
) -> Vec<Location> {
    let ck = match canonical_key(concrete_key) {
        Some(k) => k,
        None => return Vec::new(),
    };

    // M-4: per-request canonical-key memo cache.
    let mut ck_cache: HashMap<String, Option<String>> = HashMap::new();
    let cached_ck = |key: &str, cache: &mut HashMap<String, Option<String>>| -> Option<String> {
        if let Some(cached) = cache.get(key) {
            return cached.clone();
        }
        let result = canonical_key(key);
        cache.insert(key.to_string(), result.clone());
        result
    };

    let mut locations: Vec<Location> = Vec::new();

    // Schema declaration.
    if include_declaration {
        if let Some(uri) = schema_uri {
            // Find schema entries whose canonical form matches.
            let mut schema_matches: Vec<(&String, &u32)> = schema_line_map
                .iter()
                .filter(|(k, _)| canonical_key(k).as_deref() == Some(ck.as_str()))
                .collect();
            schema_matches.sort_by_key(|(k, _)| k.as_str());
            for (schema_key, &line) in schema_matches {
                let key_utf16_len: u32 = schema_key.chars().map(|c| c.len_utf16() as u32).sum();
                locations.push(Location {
                    uri: uri.clone(),
                    range: LspRange {
                        start: Position { line, character: 0 },
                        end: Position {
                            line,
                            character: key_utf16_len + 2,
                        },
                    },
                });
            }
        }
    }

    // Concrete references across all open documents.
    let mut sorted_docs: Vec<(&Url, &Vec<ConfigEntry>)> = open_docs.iter().collect();
    sorted_docs.sort_by_key(|(u, _)| u.as_str());

    for (uri, entries) in &sorted_docs {
        for entry in *entries {
            if entry.key.is_empty() {
                continue;
            }
            // M-4: use per-request cache to avoid recomputing canonical_key.
            if cached_ck(&entry.key, &mut ck_cache).as_deref() == Some(ck.as_str()) {
                locations.push(Location {
                    uri: (*uri).clone(),
                    range: entry.key_range,
                });
            }
        }
    }

    // Sort by (uri, line) for determinism.
    locations.sort_by(|a, b| {
        a.uri
            .as_str()
            .cmp(b.uri.as_str())
            .then_with(|| a.range.start.line.cmp(&b.range.start.line))
    });

    locations
}

// ── Hover helper: cross-format sensitive flag ─────────────────────────────────

/// Return `true` when the given concrete key is marked `sensitive` in the
/// unified schema, or when the key matches a sensitive-key heuristic.
///
/// This ensures that a key marked `sensitive = true` in `.env.schema` is
/// redacted in hover for *every* format (TOML, YAML, JSONC, etc.) that maps to
/// the same canonical key — AI-safety requirement from the spec.
///
/// H-2 fix: uses `UnifiedSchema::lookup` which internally uses `canonical_key_strict`
/// so that TOML `line-length` is NOT considered sensitive just because schema
/// has `LINE_LENGTH = sensitive`. Path-separator equivalences (`:` ≡ `__` ≡ `.`)
/// and UPPER_SNAKE decomposition are still applied, so `SPRING_DATASOURCE_PASSWORD`
/// correctly redacts `spring.datasource.password` and `Spring:Datasource:Password`.
pub fn is_key_sensitive(concrete_key: &str, schema: &UnifiedSchema) -> bool {
    if crate::ops::dotenv::is_sensitive_key(concrete_key) {
        return true;
    }
    // Use strict lookup (H-2 fix) — no false sensitivity from `-`/camelCase collapse.
    schema
        .lookup(concrete_key)
        .map(|v| v.sensitive)
        .unwrap_or(false)
}
