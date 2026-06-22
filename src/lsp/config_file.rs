//! File-type recognition and `ConfigFormat` dispatch — Story 001 (FR1–FR5, FR24, FR25)
//! extended by Intent 037 (TOML support) Story 001 (FR1, FR6).
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
//! TOML config (`is_toml_config_file`) — scoped canonical names only (FR3 lesson):
//! - `Cargo.toml`
//! - `pyproject.toml`
//! - `config.toml`
//! - `.cargo/config.toml` (path form)
//!
//! **Exclusion**: Plain `.env` stays on the existing env handler path (the
//! `documents` store) so it keeps ALL its existing handlers (AI-guard scan,
//! code_lens canary annotations, inlay hints, code_actions, compute_diagnostics,
//! managed-var hover, republish_all) UNCHANGED.
//!
//! **Exclusion**: `.env.schema` / `.env.schema.*` (json/yaml/toml) are handled
//! by the existing schema handler and are NOT routed through the new path.
//!
//! **Exclusion**: Arbitrary `*.toml` files not in the canonical list — NOT
//! recognized (avoids the over-broad false-positive lesson from 036 FR3).

use tower_lsp::lsp_types::Url;

use crate::ops::config_format::{
    source_layer_for_appsettings, source_layer_for_dotenv, source_layer_for_properties,
    source_layer_for_toml, source_layer_for_yaml, ConfigEntry, ConfigFormat, ResolvedValue,
    SourceLayer, WriteCapability,
};
use crate::ops::config_resolution::resolve_layers;
use crate::ops::properties_parser::{parse_dotenv_cascade, parse_properties};
use crate::parser::jsonc_config_parser::parse_jsonc_config;
use crate::parser::toml_config_parser::parse_toml_config;
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

/// Returns `true` when `uri` refers to a canonical TOML config file that the
/// TOML ReadWrite handler should own.
///
/// **Scoped to canonical names only** (FR3 lesson — avoids false-positive
/// diagnostics on arbitrary project `*.toml` files):
/// - `Cargo.toml`
/// - `pyproject.toml`
/// - `config.toml` (covers both standalone and `.cargo/config.toml` path form)
///
/// Every other `*.toml` file (e.g. `foo.toml`, `Gemfile.toml`) is NOT
/// recognized. This is intentional: canonical names are well-defined
/// contexts where env-var intelligence adds value.
pub fn is_toml_config_file(uri: &Url) -> bool {
    let path = uri.path();
    let fname = path.rsplit('/').next().unwrap_or("");
    matches!(fname, "Cargo.toml" | "pyproject.toml" | "config.toml")
}

/// Returns `true` when `uri` refers to a .NET `appsettings*` JSON file that
/// the JSONC handler should own.
///
/// **Scoped to exact names only** (FR1 / scope lesson from 036/037):
/// - `appsettings.json`
/// - `appsettings.{non-empty-environment}.json` (e.g. `appsettings.Production.json`)
///
/// Intentionally excluded to avoid false-positive handling:
/// - `mcp.json`, `.mcp.json`, `.vscode/mcp.json`, `.cursor/mcp.json` — MCP config (checked first)
/// - `package.json`, `tsconfig.json`, `.eslintrc.json`, etc. — unrelated JSON
/// - `appsettings..json` (empty environment) — treated as not recognized
///
/// The `is_mcp_config_file` check in `server.rs` is evaluated **before** this
/// predicate in the routing logic, so MCP config files are never claimed here.
pub fn is_appsettings_file(uri: &Url) -> bool {
    let path = uri.path();
    let fname = path.rsplit('/').next().unwrap_or("");
    // Exact base name match only.
    if fname == "appsettings.json" {
        return true;
    }
    // appsettings.{non-empty-Environment}.json
    // M-3 fix: restrict env to a single segment — no embedded `.` allowed.
    // `appsettings.Production.json` is valid; `appsettings.foo.bar.json` is not
    // (it would match `appsettings.foo.bar.json` with env = `foo.bar` which
    // contains a `.` and could clash with other file patterns).
    if let Some(rest) = fname.strip_prefix("appsettings.") {
        if let Some(env) = rest.strip_suffix(".json") {
            // env must be non-empty and must not contain `.` (single segment only).
            return !env.is_empty() && !env.contains('.');
        }
    }
    false
}

/// Combined predicate: is this URI handled by the new format dispatch layer?
pub fn is_config_format_file(uri: &Url) -> bool {
    is_jvm_config_file(uri)
        || is_env_cascade_file(uri)
        || is_yaml_config_file(uri)
        || is_toml_config_file(uri)
        || is_appsettings_file(uri)
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
/// **Write-capability: `ReadWrite`** (upgraded from `ReadOnly` in Intent 038).
/// Rename is supported via surgical byte-range splice (`SurgicalEdit` +
/// `yamlpath`); format is a guaranteed no-op (see `config_yaml_format_text_edits`).
///
/// # Round-trip safety
/// Rename edits are produced by [`config_yaml_rename`] which locates the exact
/// key byte-range and splices only that range — every other byte is untouched
/// *by construction*. No YAML serializer is invoked; comments, indentation,
/// and block-scalar bodies are preserved.
///
/// # Anchors/aliases (documented gap)
/// If the document contains YAML anchors the rename is declined (`None`
/// returned) rather than silently emitting an edit at the wrong location.
/// This is documented in Intent 038 requirements §"Acknowledged Gaps".
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

    /// YAML files are now read-write — rename is supported via surgical splice.
    /// Format is a guaranteed no-op (see Intent 038, Open decision 1).
    fn write_capability(&self) -> WriteCapability {
        WriteCapability::ReadWrite
    }
}

/// Format implementation for canonical TOML config files
/// (`Cargo.toml`, `pyproject.toml`, `config.toml`, `.cargo/config.toml`).
///
/// **Write-capability: `ReadWrite`**. This format supports in-place rename
/// and format via `toml_edit` lossless mutation.
pub struct TomlFormat;

impl ConfigFormat for TomlFormat {
    fn parse(&self, content: &str, layer: SourceLayer) -> Vec<ConfigEntry> {
        match parse_toml_config(content, layer) {
            Ok((entries, _diags, _doc)) => entries,
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

    /// TOML files are read-write — rename and format are both supported.
    fn write_capability(&self) -> WriteCapability {
        WriteCapability::ReadWrite
    }
}

/// Format implementation for .NET `appsettings.json` / `appsettings.{Env}.json`
/// JSONC files.
///
/// **Write-capability: `ReadWrite`**. Rename is supported via `SurgicalEdit`
/// on the exact key-string byte span, preserving comments and trailing commas.
///
/// # Round-trip safety
/// Rename edits are produced by [`config_jsonc_rename`] (in `config_features.rs`)
/// which resolves the exact key byte-range via
/// [`resolve_jsonc_key_span`][crate::parser::jsonc_config_parser::resolve_jsonc_key_span]
/// and splices only that range. Every other byte is unchanged by construction.
///
/// # .NET environment cascade
/// `appsettings.json` maps to `SourceLayer::DotNetBase` (precedence 1).
/// `appsettings.{Env}.json` maps to `SourceLayer::DotNetEnvironment(env)` (precedence 2).
/// The resolution engine uses the `SourceLayer::precedence()` ordering, so
/// environment-specific values override the base.
pub struct JsoncFormat;

impl ConfigFormat for JsoncFormat {
    fn parse(&self, content: &str, layer: SourceLayer) -> Vec<ConfigEntry> {
        let (entries, _diags) = parse_jsonc_config(content, layer);
        entries
    }

    fn resolve<'a>(&self, key: &str, layers: &'a [Vec<ConfigEntry>]) -> Option<ResolvedValue> {
        let entry = resolve_layers(key, layers)?;
        Some(ResolvedValue {
            value: entry.value.clone(),
            winning_layer: entry.source_layer.clone(),
            interpolated: false,
        })
    }

    /// JSONC files are read-write — rename is supported via surgical byte splice.
    fn write_capability(&self) -> WriteCapability {
        WriteCapability::ReadWrite
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
    if is_toml_config_file(uri) {
        let layer = source_layer_for_toml(fname);
        return Some((Box::new(TomlFormat), layer));
    }
    // .NET appsettings — checked after TOML/YAML/properties so it can never
    // claim those formats; `is_mcp_config_file` is checked in server.rs before
    // `is_config_format_file` so MCP config files never reach this branch.
    if is_appsettings_file(uri) {
        let layer = source_layer_for_appsettings(fname);
        return Some((Box::new(JsoncFormat), layer));
    }
    None
}

// Tests for this module live in tests/properties_env_intelligence_tests.rs
// per the CLAUDE.md convention: "All tests live in tests/ (no in-module tests)".
