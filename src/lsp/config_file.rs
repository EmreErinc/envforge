//! File-type recognition and `ConfigFormat` dispatch — Story 001 (FR1–FR5, FR24, FR25).
//!
//! Adds predicates for JVM config files (`.properties`) and the `.env`
//! cascade ALONGSIDE the existing `is_env_file`/`is_schema_file` predicates
//! in `server.rs`. Nothing here replaces existing behaviour.
//!
//! The LSP language-feature handlers route through `format_for_uri` so they
//! dispatch by capability rather than by file name — satisfying FR25
//! (add-format-without-touching-handlers).
//!
//! # Recognition rules
//!
//! JVM config (`is_jvm_config_file`):
//! - `application.properties`
//! - `application-{profile}.properties` (non-empty profile)
//! - `microprofile-config.properties`
//!
//! `.env` cascade (`is_env_cascade_file`):
//! - `.env.local`
//! - `.env.{environment}` (anything starting with `.env.` that is NOT `.env.schema*`)
//!
//! **Exclusion**: Plain `.env` stays on the existing env handler path (the
//! `documents` store) so it keeps ALL its existing handlers (AI-guard scan,
//! code_lens canary annotations, inlay hints, code_actions, compute_diagnostics,
//! managed-var hover, republish_all) UNCHANGED.
//!
//! **Exclusion**: `.env.schema` / `.env.schema.*` (json/yaml/toml) are handled
//! by the existing schema handler and are NOT routed through the new path.

use tower_lsp::lsp_types::Url;

use crate::ops::config_format::{
    source_layer_for_dotenv, source_layer_for_properties, source_layer_for_yaml, ConfigEntry,
    ConfigFormat, ResolvedValue, SourceLayer, WriteCapability,
};
use crate::ops::config_resolution::resolve_layers;
use crate::ops::properties_parser::{parse_dotenv_cascade, parse_properties};
use crate::parser::yaml_config_parser::parse_yaml_config;

// ── Predicates ───────────────────────────────────────────────────────────────

/// Returns `true` when `uri` refers to a JVM properties config file that the
/// new handler should own. Keeps the existing `is_env_file` / `is_schema_file`
/// predicates untouched.
///
/// Only the following filenames are recognized (FR3 scope, mirroring
/// `is_yaml_config_file` to avoid false-positive diagnostics on
/// `log4j.properties`, `pom.properties`, etc.):
/// - `application.properties`
/// - `application-{non-empty-profile}.properties`
/// - `microprofile-config.properties`
pub fn is_jvm_config_file(uri: &Url) -> bool {
    let path = uri.path();
    let fname = path.rsplit('/').next().unwrap_or("");
    if !fname.ends_with(".properties") {
        return false;
    }
    // Strip `.properties` suffix and validate the stem.
    let stem = &fname[..fname.len() - ".properties".len()];
    if stem == "application" || stem == "microprofile-config" {
        return true;
    }
    // application-{non-empty-profile}.properties
    if let Some(profile) = stem.strip_prefix("application-") {
        return !profile.is_empty();
    }
    false
}

/// Returns `true` when `uri` is a `.env` cascade sibling that should route
/// through the config path — i.e. `.env.local` or `.env.{environment}`.
///
/// Plain `.env` is explicitly excluded: it must stay on the existing env
/// handler path (the `documents` store) so it keeps all its existing handlers
/// (AI-guard scan, code_lens canary annotations, inlay hints, code_actions,
/// compute_diagnostics, managed-var hover, republish_all) UNCHANGED.
///
/// Schema files (`.env.schema`, `.env.schema.json`, `.env.schema.yaml`,
/// `.env.schema.toml`) are also excluded — they have their own handler.
pub fn is_env_cascade_file(uri: &Url) -> bool {
    let path = uri.path();
    let fname = path.rsplit('/').next().unwrap_or("");
    // Plain .env stays on the existing handler — never route through config path.
    if fname == ".env" {
        return false;
    }
    // Exclude schema files (all variants: .env.schema, .env.schema.json, etc.).
    if fname == ".env.schema" || fname.starts_with(".env.schema.") {
        return false;
    }
    // Accept .env.local and .env.{environment} (anything .env.<something>).
    fname == ".env.local" || fname.starts_with(".env.")
}

/// Returns `true` when `uri` refers to a Spring/Quarkus/MicroProfile YAML
/// config file that the read-only YAML handler should own.
///
/// Recognized patterns (spec scope — NFR11):
/// - `application.yml` / `application.yaml`
/// - `application-{profile}.yml` / `application-{profile}.yaml` (non-empty profile)
///
/// Intentionally excluded to avoid false-positive diagnostics on unrelated YAML:
/// `docker-compose.yml`, `k8s/*.yaml`, `.github/workflows/*.yml`, etc.
///
/// YAML files are classified `WriteCapability::ReadOnly` — no write path exists.
pub fn is_yaml_config_file(uri: &Url) -> bool {
    let path = uri.path();
    let fname = path.rsplit('/').next().unwrap_or("");
    // Strip either .yml or .yaml suffix; everything else is excluded.
    let stem = if let Some(s) = fname.strip_suffix(".yml") {
        s
    } else if let Some(s) = fname.strip_suffix(".yaml") {
        s
    } else {
        return false;
    };
    // Must be exactly "application" or "application-{non-empty-profile}".
    if stem == "application" {
        return true;
    }
    if let Some(profile) = stem.strip_prefix("application-") {
        return !profile.is_empty();
    }
    false
}

/// Combined predicate: is this URI handled by the new format dispatch layer?
pub fn is_config_format_file(uri: &Url) -> bool {
    is_jvm_config_file(uri) || is_env_cascade_file(uri) || is_yaml_config_file(uri)
}

// ── ConfigFormat impls ────────────────────────────────────────────────────────

/// Format implementation for Java `.properties` files.
pub struct PropertiesFormat;

impl ConfigFormat for PropertiesFormat {
    fn parse(&self, content: &str, layer: SourceLayer) -> Vec<ConfigEntry> {
        parse_properties(content, layer)
    }

    fn resolve<'a>(&self, key: &str, layers: &'a [Vec<ConfigEntry>]) -> Option<ResolvedValue> {
        let entry = resolve_layers(key, layers)?;
        Some(ResolvedValue {
            value: entry.value.clone(),
            winning_layer: entry.source_layer.clone(),
            interpolated: false,
        })
    }

    fn write_capability(&self) -> WriteCapability {
        WriteCapability::ReadWrite
    }
}

/// Format implementation for `.env`-cascade files.
pub struct DotEnvCascadeFormat;

impl ConfigFormat for DotEnvCascadeFormat {
    fn parse(&self, content: &str, layer: SourceLayer) -> Vec<ConfigEntry> {
        parse_dotenv_cascade(content, layer)
    }

    fn resolve<'a>(&self, key: &str, layers: &'a [Vec<ConfigEntry>]) -> Option<ResolvedValue> {
        let entry = resolve_layers(key, layers)?;
        Some(ResolvedValue {
            value: entry.value.clone(),
            winning_layer: entry.source_layer.clone(),
            interpolated: false,
        })
    }

    fn write_capability(&self) -> WriteCapability {
        WriteCapability::ReadWrite
    }
}

/// Format implementation for YAML config files (`application.yml`/`.yaml` +
/// `application-{profile}.yml`/`.yaml`).
///
/// **Write-capability: `ReadOnly`**. This format never produces write edits.
/// The parse path uses the read-only `yaml-rust2` crate; there is no
/// serialisation path in this implementation.
pub struct YamlFormat;

impl ConfigFormat for YamlFormat {
    fn parse(&self, content: &str, layer: SourceLayer) -> Vec<ConfigEntry> {
        // Ignore diagnostics at parse time; they are surfaced separately via
        // `publish_config_diagnostics_for` which calls `config_diagnostics`.
        match parse_yaml_config(content, layer) {
            Ok((entries, _errs)) => entries,
            Err(_e) => Vec::new(),
        }
    }

    fn resolve<'a>(&self, key: &str, layers: &'a [Vec<ConfigEntry>]) -> Option<ResolvedValue> {
        let entry = resolve_layers(key, layers)?;
        Some(ResolvedValue {
            value: entry.value.clone(),
            winning_layer: entry.source_layer.clone(),
            interpolated: false,
        })
    }

    /// YAML files are read-only — no language feature may write or reformat them.
    fn write_capability(&self) -> WriteCapability {
        WriteCapability::ReadOnly
    }
}

/// Return the appropriate `ConfigFormat` implementation for a URI, plus the
/// `SourceLayer` inferred from the file name.
///
/// Returns `None` for URIs that are not recognised config-format files.
pub fn format_for_uri(uri: &Url) -> Option<(Box<dyn ConfigFormat>, SourceLayer)> {
    let path = uri.path();
    let fname = path.rsplit('/').next().unwrap_or("");

    if is_jvm_config_file(uri) {
        let layer = source_layer_for_properties(fname);
        return Some((Box::new(PropertiesFormat), layer));
    }
    if is_env_cascade_file(uri) {
        let layer = source_layer_for_dotenv(fname);
        return Some((Box::new(DotEnvCascadeFormat), layer));
    }
    if is_yaml_config_file(uri) {
        let layer = source_layer_for_yaml(fname);
        return Some((Box::new(YamlFormat), layer));
    }
    None
}

// Tests for this module live in tests/properties_env_intelligence_tests.rs
// per the CLAUDE.md convention: "All tests live in tests/ (no in-module tests)".
