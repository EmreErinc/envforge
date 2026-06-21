//! Read-only YAML config parser — Unit 002 (FR17, NFR14).
//!
//! Parses `application.yml`/`.yaml` (and profile variants) into the
//! format-agnostic [`ConfigEntry`] model used by the LSP language-feature
//! handlers. Nested YAML keys are flattened to dotted paths so
//! `spring.datasource.url` behaves like a flat key.
//!
//! # Design constraints
//! - **Read-only**: no serialisation/write path exists in this module.
//! - **No panics**: malformed YAML returns an error; callers degrade to
//!   diagnostics rather than crashing.
//! - **UTF-16-correct positions**: all `position.character` values are
//!   UTF-16 code-unit offsets (LSP spec), not byte offsets.
//!
//! # Position strategy
//! `yaml-rust2` exposes per-event `Marker`s. `Marker::line()` is 1-indexed,
//! `Marker::col()` is 1-indexed char-column (increments by one per Unicode
//! scalar, not per byte). We convert the 0-indexed char-column to a UTF-16
//! column by walking the source line and accumulating `ch.len_utf16()` counts.

use std::collections::HashMap;

use tower_lsp::lsp_types::{Position, Range as LspRange};
use yaml_rust2::parser::{Event, MarkedEventReceiver, Parser};
use yaml_rust2::scanner::{Marker, ScanError};

use crate::ops::config_format::{ConfigEntry, SourceLayer};

// ── Error type ───────────────────────────────────────────────────────────────

/// Error returned when YAML content cannot be parsed.
#[derive(Debug, Clone)]
pub struct YamlParseError {
    /// Human-readable message suitable for an LSP diagnostic.
    pub message: String,
    /// 0-based line number, if available.
    pub line: Option<u32>,
}

impl std::fmt::Display for YamlParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(l) = self.line {
            write!(f, "YAML parse error on line {}: {}", l + 1, self.message)
        } else {
            write!(f, "YAML parse error: {}", self.message)
        }
    }
}

impl From<ScanError> for YamlParseError {
    fn from(e: ScanError) -> Self {
        let line = e.marker().line().saturating_sub(1) as u32;
        YamlParseError {
            message: e.to_string(),
            line: Some(line),
        }
    }
}

// ── Position helpers ──────────────────────────────────────────────────────────

/// Convert a 0-indexed **char** column (as produced by `Marker::col() - 1`) to
/// a UTF-16 code-unit column (0-indexed, as required by LSP `Position::character`).
///
/// `line_text` is the raw source line (no trailing newline).
/// `char_col` is the 0-indexed character index from the start of `line_text`.
///
/// Returns the UTF-16 column clamped to the number of code units in the line.
fn char_col_to_utf16_col(line_text: &str, char_col: usize) -> u32 {
    let mut units: u32 = 0;
    for (char_idx, ch) in line_text.chars().enumerate() {
        if char_idx >= char_col {
            break;
        }
        units += ch.len_utf16() as u32;
    }
    units
}

// ── Event-based collector ────────────────────────────────────────────────────

/// A raw key→value record captured by the event receiver before flattening.
#[derive(Debug, Clone)]
struct RawEntry {
    /// The dotted key path built by the receiver.
    key: String,
    /// The scalar value (as string).
    value: String,
    /// Marker for the key scalar (start position).
    key_mark: Marker,
    /// Marker for the value scalar (start position).
    value_mark: Marker,
}

/// State machine for the event receiver.
#[derive(Debug, Clone, PartialEq)]
enum RecvState {
    /// Not inside any document structure yet.
    Idle,
    /// Expecting a key scalar (inside a mapping).
    ExpectKey,
    /// Expecting a value scalar / nested structure for `pending_key`.
    ExpectValue,
}

/// Gathers raw key→value pairs from the YAML event stream.
struct YamlCollector {
    /// Accumulated entries (depth-first, flattened).
    entries: Vec<RawEntry>,
    /// Per-depth key path segments. Each element is the **leaf** segment for
    /// that nesting level so `current_prefix()` joins them correctly.
    path_stack: Vec<String>,
    /// The current pending key (full dotted path) waiting for its value.
    pending_key: Option<String>,
    /// The leaf portion of the current pending key (as written in source).
    pending_leaf: Option<String>,
    /// Marker for the current pending key.
    pending_key_mark: Option<Marker>,
    /// Receiver state at the current mapping depth.
    state_stack: Vec<RecvState>,
    /// Current overall state.
    current_state: RecvState,
    /// First error encountered (if any).
    error: Option<YamlParseError>,
    /// Duplicate key detection: dotted path → first-seen 0-based line.
    seen_keys: HashMap<String, u32>,
    /// Duplicate key diagnostics (populated during collection).
    duplicate_diags: Vec<(String, u32)>,
    /// Whether we are inside a sequence (sequences are skipped for flat model).
    seq_depth: u32,
}

impl YamlCollector {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            path_stack: Vec::new(),
            pending_key: None,
            pending_leaf: None,
            pending_key_mark: None,
            state_stack: Vec::new(),
            current_state: RecvState::Idle,
            error: None,
            seen_keys: HashMap::new(),
            duplicate_diags: Vec::new(),
            seq_depth: 0,
        }
    }

    /// Build the current dotted prefix from the path stack.
    fn current_prefix(&self) -> String {
        let parts: Vec<&str> = self.path_stack.iter().map(String::as_str).collect();
        parts.join(".")
    }

    /// Build the full dotted key for a leaf under the current prefix.
    fn full_key(&self, leaf: &str) -> String {
        let prefix = self.current_prefix();
        if prefix.is_empty() {
            leaf.to_string()
        } else {
            format!("{}.{}", prefix, leaf)
        }
    }
}

impl MarkedEventReceiver for YamlCollector {
    fn on_event(&mut self, ev: Event, mark: Marker) {
        if self.error.is_some() {
            return;
        }
        // Inside a sequence: skip everything except nested structure tracking.
        match &ev {
            Event::SequenceStart(_, _) => {
                self.seq_depth += 1;
                return;
            }
            Event::SequenceEnd => {
                self.seq_depth = self.seq_depth.saturating_sub(1);
                if self.seq_depth == 0 && self.current_state == RecvState::ExpectValue {
                    // The sequence was the value for the pending key — skip this entry.
                    self.pending_key = None;
                    self.pending_leaf = None;
                    self.pending_key_mark = None;
                    self.current_state = RecvState::ExpectKey;
                }
                return;
            }
            _ if self.seq_depth > 0 => {
                // Inside a sequence — track nested mappings for depth but skip entries.
                match &ev {
                    Event::MappingStart(_, _) => self.path_stack.push(String::new()),
                    Event::MappingEnd => {
                        self.path_stack.pop();
                    }
                    _ => {}
                }
                return;
            }
            _ => {}
        }

        match ev {
            Event::StreamStart
            | Event::StreamEnd
            | Event::DocumentStart
            | Event::DocumentEnd
            | Event::Nothing
            // SequenceStart/SequenceEnd are fully handled in the pre-match above
            // (they return early). These arms are unreachable at runtime but
            // required for exhaustiveness.
            | Event::SequenceStart(_, _)
            | Event::SequenceEnd => {}

            Event::MappingStart(_, _) => {
                // When we encounter a mapping as a value, push the LEAF segment of
                // the pending key onto the path stack so that `current_prefix()`
                // correctly concatenates leaf segments rather than full dotted keys.
                if self.current_state == RecvState::ExpectValue {
                    // Push only the leaf (not the full dotted pending_key).
                    let leaf = self.pending_leaf.take().unwrap_or_default();
                    self.pending_key = None;
                    self.pending_key_mark = None;
                    self.path_stack.push(leaf);
                    self.state_stack.push(RecvState::ExpectKey);
                    self.current_state = RecvState::ExpectKey;
                } else {
                    // Top-level or nested mapping start when idle/expecting key.
                    self.state_stack.push(self.current_state.clone());
                    self.current_state = RecvState::ExpectKey;
                }
            }

            Event::MappingEnd => {
                // Pop path segment and state.
                self.path_stack.pop();
                self.current_state = self.state_stack.pop().unwrap_or(RecvState::Idle);
            }

            Event::Scalar(value, _style, _anchor, _tag) => {
                match self.current_state {
                    RecvState::ExpectKey => {
                        let full_key = self.full_key(&value);
                        // Track duplicates.
                        let key_line = mark.line().saturating_sub(1) as u32;
                        if let Some(&first_line) = self.seen_keys.get(&full_key) {
                            self.duplicate_diags.push((full_key.clone(), first_line));
                        } else {
                            self.seen_keys.insert(full_key.clone(), key_line);
                        }
                        // Store both full key and leaf so that if a MappingStart
                        // follows, we push only the leaf onto path_stack.
                        self.pending_leaf = Some(value);
                        self.pending_key = Some(full_key);
                        self.pending_key_mark = Some(mark);
                        self.current_state = RecvState::ExpectValue;
                    }
                    RecvState::ExpectValue => {
                        if let (Some(key), Some(key_mark)) =
                            (self.pending_key.take(), self.pending_key_mark.take())
                        {
                            self.pending_leaf = None;
                            self.entries.push(RawEntry {
                                key,
                                value,
                                key_mark,
                                value_mark: mark,
                            });
                        }
                        self.current_state = RecvState::ExpectKey;
                    }
                    RecvState::Idle => {}
                }
            }

            Event::Alias(_) => {
                // Treat aliases as empty value for the pending key (skip).
                if self.current_state == RecvState::ExpectValue {
                    self.pending_key = None;
                    self.pending_leaf = None;
                    self.pending_key_mark = None;
                    self.current_state = RecvState::ExpectKey;
                }
            }
        }
    }
}

// ── Public parse function ────────────────────────────────────────────────────

/// Parse YAML `content` into a list of [`ConfigEntry`] values with UTF-16
/// positions, plus a list of parse/duplicate-key errors.
///
/// - Nested maps are flattened to dotted paths (`spring.datasource.url`).
/// - Sequences are skipped (no indexed entries).
/// - Malformed YAML returns `Err(YamlParseError)` — never panics.
/// - Duplicate dotted keys are returned in `errors` as additional diagnostics.
pub fn parse_yaml_config(
    content: &str,
    layer: SourceLayer,
) -> Result<(Vec<ConfigEntry>, Vec<YamlParseError>), YamlParseError> {
    // Collect source lines for position conversion (UTF-16 column mapping).
    let source_lines: Vec<&str> = content.lines().collect();

    let mut collector = YamlCollector::new();
    let mut parser = Parser::new_from_str(content);
    parser
        .load(&mut collector, true)
        .map_err(YamlParseError::from)?;

    // Build duplicate-key diagnostics from collector.
    // Each element in duplicate_diags is (key, first_line_0). A key appearing N
    // times generates N-1 entries (one per duplicate occurrence after the first).
    // We immediately record which line each duplicate is on so attribution is
    // correct for 3+ occurrences. We use a per-key occurrence counter to skip
    // the first occurrence and assign each subsequent one its own line.
    let mut key_occurrence: HashMap<&str, u32> = HashMap::new();
    let mut errors: Vec<YamlParseError> = Vec::new();
    for (dup_key, first_line) in &collector.duplicate_diags {
        // Count how many times we've seen this duplicate already (to skip the
        // first duplicate occurrence being attributed twice).
        let occ = key_occurrence.entry(dup_key.as_str()).or_insert(0);
        *occ += 1;
        let skip = *occ - 1; // how many duplicates we've already emitted for this key
                             // Find the (skip+1)-th occurrence that is not on first_line.
        let dup_line = collector
            .entries
            .iter()
            .filter(|e| {
                &e.key == dup_key && e.key_mark.line().saturating_sub(1) as u32 != *first_line
            })
            .nth(skip as usize)
            .map(|e| e.key_mark.line().saturating_sub(1) as u32);
        errors.push(YamlParseError {
            message: format!(
                "Duplicate key '{}' (first defined on line {})",
                dup_key,
                first_line + 1
            ),
            line: dup_line,
        });
    }

    // Convert raw entries to ConfigEntry with UTF-16 positions.
    let mut entries: Vec<ConfigEntry> = Vec::with_capacity(collector.entries.len());

    // Also scan for unterminated `${` in values.
    for raw in &collector.entries {
        // Key position — Marker::col() is a 0-indexed char-column (the scanner
        // stores 0-indexed and displays col+1; the struct comment "1-indexed" is
        // the display convention). Convert the char-column to UTF-16 units by
        // walking the line. Do NOT use Marker::index() here: it is a char count
        // from document start, NOT a byte count, so subtracting a byte offset
        // produces mixed-unit garbage.
        let key_line_0 = raw.key_mark.line().saturating_sub(1);
        let key_line_str = source_lines.get(key_line_0).copied().unwrap_or("");
        let key_char_col = raw.key_mark.col();
        let key_start_utf16 = char_col_to_utf16_col(key_line_str, key_char_col);

        // The leaf key (last dotted segment) length for the range end.
        // The key mark points to the leaf scalar in source, not the dotted path.
        let leaf_key = raw.key.rsplit('.').next().unwrap_or(raw.key.as_str());
        // UTF-16 length of the leaf key token.
        let leaf_utf16_len: u32 = leaf_key.chars().map(|c| c.len_utf16() as u32).sum();
        let key_end_utf16 = key_start_utf16 + leaf_utf16_len;

        // Value position — same char-column approach for UTF-16 correctness.
        let val_line_0 = raw.value_mark.line().saturating_sub(1);
        let val_line_str = source_lines.get(val_line_0).copied().unwrap_or("");
        let val_char_col = raw.value_mark.col();
        let val_start_utf16 = char_col_to_utf16_col(val_line_str, val_char_col);
        let val_utf16_len: u32 = raw.value.chars().map(|c| c.len_utf16() as u32).sum();
        let val_end_utf16 = val_start_utf16 + val_utf16_len;

        // Check for unterminated `${` in the value.
        if crate::ops::config_resolution::contains_interpolation(&raw.value) {
            let unterm = crate::ops::config_resolution::find_unterminated_refs(&raw.value);
            if !unterm.is_empty() {
                errors.push(YamlParseError {
                    message: format!("Unterminated interpolation `${{` in value of '{}'", raw.key),
                    line: Some(val_line_0 as u32),
                });
            }
        }

        entries.push(ConfigEntry {
            key: raw.key.clone(),
            value: raw.value.clone(),
            key_range: LspRange {
                start: Position {
                    line: key_line_0 as u32,
                    character: key_start_utf16,
                },
                end: Position {
                    line: key_line_0 as u32,
                    character: key_end_utf16,
                },
            },
            value_range: LspRange {
                start: Position {
                    line: val_line_0 as u32,
                    character: val_start_utf16,
                },
                end: Position {
                    line: val_line_0 as u32,
                    character: val_end_utf16,
                },
            },
            line: key_line_0 as u32,
            source_layer: layer.clone(),
        });
    }

    Ok((entries, errors))
}

/// Parse YAML and convert parse errors into LSP `Diagnostic` shapes.
///
/// Returns flattened entries and any diagnostics (parse errors, duplicate
/// keys, unterminated interpolations).
pub fn parse_yaml_to_entries(
    content: &str,
    layer: SourceLayer,
) -> (
    Vec<ConfigEntry>,
    Vec<crate::lsp::config_features::YamlDiagnostic>,
) {
    use crate::lsp::config_features::YamlDiagnostic;
    use tower_lsp::lsp_types::{DiagnosticSeverity, Range as LspRange};

    match parse_yaml_config(content, layer) {
        Err(e) => {
            // Malformed YAML — produce a single parse-error diagnostic.
            let line = e.line.unwrap_or(0);
            let diag = YamlDiagnostic {
                range: LspRange {
                    start: Position { line, character: 0 },
                    end: Position { line, character: 0 },
                },
                severity: DiagnosticSeverity::ERROR,
                message: e.message,
            };
            (Vec::new(), vec![diag])
        }
        Ok((entries, errs)) => {
            let diags: Vec<YamlDiagnostic> = errs
                .into_iter()
                .map(|e| {
                    let line = e.line.unwrap_or(0);
                    // Find the entry with this line to get a precise range.
                    let range = entries
                        .iter()
                        .find(|en| en.line == line)
                        .map(|en| en.key_range)
                        .unwrap_or_else(|| LspRange {
                            start: Position { line, character: 0 },
                            end: Position { line, character: 0 },
                        });
                    let severity = if e.message.contains("Duplicate") {
                        DiagnosticSeverity::WARNING
                    } else {
                        DiagnosticSeverity::ERROR
                    };
                    YamlDiagnostic {
                        range,
                        severity,
                        message: e.message,
                    }
                })
                .collect();
            (entries, diags)
        }
    }
}
