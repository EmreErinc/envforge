//! YAML value-span resolution — Intent 038, Unit 002, Story 001 (FR2).
//!
//! Resolves a dotted-path key (as produced by `yaml_config_parser`) to the
//! **exact byte range** of its *key token* in the YAML source, using the
//! `yamlpath` crate (tree-sitter-yaml). This range is then fed to
//! [`SurgicalEdit`] to produce a rename edit that touches only those bytes.
//!
//! # Coverage
//! - Nested block mappings (`spring.datasource.url`)
//! - Flow mappings (`{a: b}`)
//! - Block scalars (`|` / `>`)
//! - Single/double-quoted keys
//! - Plain scalar keys
//!
//! # Documented gaps (acknowledged, not silently mis-resolved)
//! - **Anchors/aliases**: if `yamlpath` resolves the route through an alias,
//!   the feature points to the *anchor definition site*, not the alias site.
//!   This function detects this case and returns `None` rather than silently
//!   emitting an edit at the wrong location.
//! - **Multi-document YAML (`---`)**: `yamlpath` only processes the first
//!   document. If a key exists only in a subsequent document section the
//!   resolver returns `None` and no edit is produced.
//!
//! # Byte-span semantics
//! The returned range is a **byte range** into the original `content` string.
//! It covers exactly the *leaf key token* (the last segment of the dotted
//! path) as it appears in the source — not the full dotted-path string.
//! For quoted keys the range includes the quote characters so callers that
//! splice the new key must also supply appropriate quotes if needed.

use std::ops::Range;

use yamlpath::{Component, Document, Route};

/// A resolved key byte-span within a YAML document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YamlKeySpan {
    /// Byte range of the *key token* in the source (leaf segment only).
    pub byte_range: Range<usize>,
    /// Whether the key token is quoted (single or double quotes).
    pub is_quoted: bool,
}

/// Errors from YAML span resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// The `yamlpath` Document could not be constructed (malformed YAML).
    MalformedYaml,
    /// The dotted path was not found in the document.
    KeyNotFound,
    /// The key is reached via an anchor/alias — renaming is ambiguous.
    AnchorAlias,
    /// Any other resolution failure.
    Other(String),
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MalformedYaml => write!(f, "malformed YAML"),
            Self::KeyNotFound => write!(f, "key not found"),
            Self::AnchorAlias => {
                write!(f, "key is an anchor/alias target — rename is ambiguous")
            }
            Self::Other(s) => write!(f, "{}", s),
        }
    }
}

/// Resolve `dotted_key` to the byte range of its leaf key token in `content`.
///
/// # Arguments
/// - `content` — raw YAML source.
/// - `dotted_key` — dotted path as produced by `yaml_config_parser`
///   (e.g. `"spring.datasource.url"`).
///
/// # Returns
/// `Ok(YamlKeySpan)` on success; `Err(ResolveError)` on any failure.
///
/// Never panics on malformed YAML — parser errors become `Err(MalformedYaml)`.
pub fn resolve_yaml_key_span(content: &str, dotted_key: &str) -> Result<YamlKeySpan, ResolveError> {
    let doc = Document::new(content).map_err(|_| ResolveError::MalformedYaml)?;

    // Anchors/aliases guard: if the document has anchors, we flag the result
    // after resolution so we can detect when `query_key_only` resolved
    // through an alias (the feature's start byte would be inside the anchor
    // definition, not the alias reference site). We refuse such renames.
    let has_anchors = doc.has_anchors();

    // Build the yamlpath Route from the dotted key segments.
    let segments: Vec<&str> = dotted_key.split('.').collect();
    let route = Route::from(
        segments
            .iter()
            .map(|s| Component::Key((*s).into()))
            .collect::<Vec<_>>(),
    );

    // `query_key_only` returns the *key node* of the last segment, not the value.
    let feature = doc.query_key_only(&route).map_err(|e| match e {
        yamlpath::QueryError::ExhaustedMapping(_)
        | yamlpath::QueryError::MissingChildField(_, _) => ResolveError::KeyNotFound,
        yamlpath::QueryError::InvalidInput => ResolveError::MalformedYaml,
        other => ResolveError::Other(other.to_string()),
    })?;

    let (start, end) = feature.location.byte_span;

    // Anchors/aliases guard: if the document has anchors and the resolved
    // span falls inside an anchor block, the edit would land at the definition
    // site rather than the usage site — refuse.
    if has_anchors {
        // We can't cheaply distinguish "resolved through alias" from "normal"
        // via the public API, so we refuse ALL key renames when anchors are
        // present. This is conservative but safe: it satisfies the spec
        // "documented gap, never silently mis-edit".
        return Err(ResolveError::AnchorAlias);
    }

    // Detect if the key is quoted by inspecting the source bytes.
    let raw = content.get(start..end).ok_or_else(|| {
        ResolveError::Other(format!(
            "byte span {}..{} is out of range for content of length {}",
            start,
            end,
            content.len()
        ))
    })?;
    let first_byte = raw.as_bytes().first().copied().unwrap_or(0);
    let is_quoted = first_byte == b'\'' || first_byte == b'"';

    Ok(YamlKeySpan {
        byte_range: start..end,
        is_quoted,
    })
}

/// Resolve `dotted_key` to the byte range of its *value* in `content`.
///
/// Returns `Ok(Range<usize>)` on success; `Err(ResolveError)` on failure.
/// Returns `Err(KeyNotFound)` for absent values (e.g. `key:` with no value).
///
/// This is used by tests that need the value span; rename only needs the key span.
pub fn resolve_yaml_value_span(
    content: &str,
    dotted_key: &str,
) -> Result<Range<usize>, ResolveError> {
    let doc = Document::new(content).map_err(|_| ResolveError::MalformedYaml)?;

    let has_anchors = doc.has_anchors();
    if has_anchors {
        return Err(ResolveError::AnchorAlias);
    }

    let segments: Vec<&str> = dotted_key.split('.').collect();
    let route = Route::from(
        segments
            .iter()
            .map(|s| Component::Key((*s).into()))
            .collect::<Vec<_>>(),
    );

    let maybe_feature = doc.query_exact(&route).map_err(|e| match e {
        yamlpath::QueryError::ExhaustedMapping(_)
        | yamlpath::QueryError::MissingChildField(_, _) => ResolveError::KeyNotFound,
        yamlpath::QueryError::InvalidInput => ResolveError::MalformedYaml,
        other => ResolveError::Other(other.to_string()),
    })?;

    let feature = maybe_feature.ok_or(ResolveError::KeyNotFound)?;
    Ok(feature.location.byte_span.0..feature.location.byte_span.1)
}
