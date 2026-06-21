//! `ConfigFormat` abstraction — Story 001 (FR24, FR25).
//!
//! Defines the format-agnostic seam that lets all IDE language-feature
//! handlers dispatch by capability rather than file name. Adding a new
//! format (e.g. YAML in unit 002) only requires:
//!   1. Implementing `ConfigFormat` for the new type.
//!   2. Registering the new recognizer predicate alongside the existing ones.
//!
//! Nothing in `src/lsp/` needs to change for step 2.

use std::collections::HashMap;

use tower_lsp::lsp_types::Range as LspRange;

/// Whether a format allows in-place edits (rename, format) via the
/// tempfile + atomic-rename path. Read-only formats (e.g. YAML in
/// unit 002) return `ReadOnly` and the rename/format handlers skip them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteCapability {
    /// The format supports in-place edits (rename, format).
    ReadWrite,
    /// The format is read-only for LSP purposes.
    ReadOnly,
}

/// Which "layer" a config entry comes from. Used by the resolution
/// engine and surfaced in hover popups.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SourceLayer {
    /// Spring Boot / Quarkus base: `application.properties`
    Base,
    /// Spring Boot profile: `application-{profile}.properties`
    Profile(String),
    /// `.env` (lowest precedence in the cascade)
    DotEnv,
    /// `.env.local`
    DotEnvLocal,
    /// `.env.{environment}` (e.g. `.env.staging`)
    DotEnvEnvironment(String),
    /// Source unknown / not provided.
    Unknown,
}

impl SourceLayer {
    /// Human-readable display label shown in hover popups.
    pub fn display(&self) -> String {
        match self {
            SourceLayer::Base => "base".to_string(),
            SourceLayer::Profile(p) => format!("profile:{}", p),
            SourceLayer::DotEnv => ".env".to_string(),
            SourceLayer::DotEnvLocal => ".env.local".to_string(),
            SourceLayer::DotEnvEnvironment(e) => format!(".env.{}", e),
            SourceLayer::Unknown => "unknown".to_string(),
        }
    }

    /// Numeric precedence — higher number wins when resolving conflicts.
    /// Within the `.env` cascade: `.env` < `.env.local` < `.env.{env}`.
    /// Within Spring profiles:   `base` < `profile`.
    pub fn precedence(&self) -> u8 {
        match self {
            SourceLayer::DotEnv => 1,
            SourceLayer::Base => 1,
            SourceLayer::DotEnvLocal => 2,
            SourceLayer::DotEnvEnvironment(_) => 3,
            SourceLayer::Profile(_) => 2,
            SourceLayer::Unknown => 0,
        }
    }
}

/// A single key/value pair parsed from a config file, with UTF-16
/// position ranges for LSP navigation.
#[derive(Debug, Clone)]
pub struct ConfigEntry {
    pub key: String,
    pub value: String,
    /// Range covering the key token (used for goto-def, rename, tokens).
    pub key_range: LspRange,
    /// Range covering the value token (used for diagnostics, tokens).
    pub value_range: LspRange,
    /// Zero-based line number (same as `key_range.start.line`).
    pub line: u32,
    /// Which layer/file this entry comes from.
    pub source_layer: SourceLayer,
}

/// The resolved effective value of a key across layers.
#[derive(Debug, Clone)]
pub struct ResolvedValue {
    /// The final resolved value (after interpolation).
    pub value: String,
    /// Which layer provided the winning value.
    pub winning_layer: SourceLayer,
    /// Whether the value was produced by interpolation (useful for
    /// deciding whether to redact sensitive interpolated values).
    pub interpolated: bool,
}

/// Format-agnostic abstraction over a config-file type (FR24).
///
/// Implement this trait to add a new format. The LSP handlers dispatch
/// through this interface so they never branch on file names.
pub trait ConfigFormat: Send + Sync {
    /// Parse `content` into a list of positioned entries.
    fn parse(&self, content: &str, layer: SourceLayer) -> Vec<ConfigEntry>;

    /// Resolve the effective value for `key` across all provided layers.
    fn resolve<'a>(&self, key: &str, layers: &'a [Vec<ConfigEntry>]) -> Option<ResolvedValue>;

    /// Whether this format supports in-place edits.
    fn write_capability(&self) -> WriteCapability;
}

/// Detect the `SourceLayer` for a JVM properties file from its file name.
///
/// Rules:
/// - `application.properties`           → `SourceLayer::Base`
/// - `application-{profile}.properties` → `SourceLayer::Profile(profile)`
/// - `microprofile-config.properties`   → `SourceLayer::Base`
/// - Any other `.properties` file       → `SourceLayer::Base`
pub fn source_layer_for_properties(file_name: &str) -> SourceLayer {
    if file_name == "application.properties" || file_name == "microprofile-config.properties" {
        return SourceLayer::Base;
    }
    // application-{profile}.properties
    if let Some(rest) = file_name.strip_prefix("application-") {
        if let Some(profile) = rest.strip_suffix(".properties") {
            if !profile.is_empty() {
                return SourceLayer::Profile(profile.to_string());
            }
        }
    }
    SourceLayer::Base
}

/// Detect the `SourceLayer` for a YAML config file from its file name.
///
/// Rules:
/// - `application.yml` / `application.yaml`              → `SourceLayer::Base`
/// - `application-{profile}.yml` / `.yaml`               → `SourceLayer::Profile(profile)`
/// - Any other `.yml` / `.yaml` file                      → `SourceLayer::Base`
pub fn source_layer_for_yaml(file_name: &str) -> SourceLayer {
    // application.yml / application.yaml
    if file_name == "application.yml" || file_name == "application.yaml" {
        return SourceLayer::Base;
    }
    // application-{profile}.yml / application-{profile}.yaml
    if let Some(rest) = file_name.strip_prefix("application-") {
        if let Some(profile) = rest
            .strip_suffix(".yml")
            .or_else(|| rest.strip_suffix(".yaml"))
        {
            if !profile.is_empty() {
                return SourceLayer::Profile(profile.to_string());
            }
        }
    }
    SourceLayer::Base
}

/// Detect the `SourceLayer` for a `.env`-cascade file from its file name.
///
/// Rules:
/// - `.env`              → `SourceLayer::DotEnv`
/// - `.env.local`        → `SourceLayer::DotEnvLocal`
/// - `.env.{env}`        → `SourceLayer::DotEnvEnvironment(env)`
/// - Fallback            → `SourceLayer::DotEnv`
pub fn source_layer_for_dotenv(file_name: &str) -> SourceLayer {
    match file_name {
        ".env" => SourceLayer::DotEnv,
        ".env.local" => SourceLayer::DotEnvLocal,
        _ => {
            if let Some(env) = file_name.strip_prefix(".env.") {
                if !env.is_empty() {
                    return SourceLayer::DotEnvEnvironment(env.to_string());
                }
            }
            SourceLayer::DotEnv
        }
    }
}

/// Resolve a `${VAR}` or `${VAR:default}` interpolation reference from a
/// lookup map. Returns `(resolved_value, was_substituted)`.
///
/// - If `VAR` is present in `env` → returns `(env[VAR].clone(), true)`.
/// - If not present and a default is provided → returns `(default, true)`.
/// - If not present and no default → returns `("${VAR}", false)` (unresolved).
///
/// The cycle-guard set is maintained by the caller (`interpolate_value`).
pub fn resolve_ref(
    var: &str,
    default: Option<&str>,
    env: &HashMap<String, String>,
) -> (String, bool) {
    if let Some(val) = env.get(var) {
        return (val.clone(), true);
    }
    if let Some(def) = default {
        return (def.to_string(), true);
    }
    // Unresolved — leave as-is so diagnostics can flag it.
    let raw = if let Some(d) = default {
        format!("${{{}:{}}}", var, d)
    } else {
        format!("${{{}}}", var)
    };
    (raw, false)
}
