use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ─── Schema Types ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EnvSchema {
    pub variables: HashMap<String, SchemaVariable>,
}

#[derive(Debug, Clone)]
pub struct SchemaVariable {
    pub var_type: VarType,
    pub required: bool,
    pub default: Option<String>,
    pub description: Option<String>,
    pub example: Option<String>,
    pub sensitive: bool,
    pub pattern: Option<String>,
    pub values: Option<Vec<String>>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub env_overrides: HashMap<String, SchemaOverride>,
}

#[derive(Debug, Clone)]
pub struct SchemaOverride {
    pub var_type: Option<VarType>,
    pub required: Option<bool>,
    pub default: Option<String>,
    pub pattern: Option<String>,
    pub values: Option<Vec<String>>,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VarType {
    String,
    Number,
    Bool,
    Url,
    Email,
    Enum,
    Regex,
    Port,
}

impl VarType {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "string" => Some(VarType::String),
            "number" => Some(VarType::Number),
            "bool" => Some(VarType::Bool),
            "url" => Some(VarType::Url),
            "email" => Some(VarType::Email),
            "enum" => Some(VarType::Enum),
            "regex" => Some(VarType::Regex),
            "port" => Some(VarType::Port),
            _ => None,
        }
    }

    pub fn display(&self) -> &str {
        match self {
            VarType::String => "string",
            VarType::Number => "number",
            VarType::Bool => "bool",
            VarType::Url => "url",
            VarType::Email => "email",
            VarType::Enum => "enum",
            VarType::Regex => "regex",
            VarType::Port => "port",
        }
    }
}

impl Default for SchemaVariable {
    fn default() -> Self {
        Self {
            var_type: VarType::String,
            required: false,
            default: None,
            description: None,
            example: None,
            sensitive: false,
            pattern: None,
            values: None,
            min: None,
            max: None,
            env_overrides: HashMap::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    #[error("schema file not found: {}", .0.display())]
    FileNotFound(PathBuf),

    #[error("schema parse error: {0}")]
    ParseError(String),

    #[error("unknown type '{typ}' for variable '{var}'")]
    UnknownType { var: String, typ: String },
}

// ─── Schema Parsing ──────────────────────────────────────────

/// Parse a .env.schema TOML file.
pub fn parse_schema(path: &Path) -> Result<EnvSchema, SchemaError> {
    if !path.exists() {
        return Err(SchemaError::FileNotFound(path.to_path_buf()));
    }
    let content = std::fs::read_to_string(path)
        .map_err(|e| SchemaError::ParseError(format!("{}: {}", path.display(), e)))?;
    parse_schema_content(&content)
}

/// Parse schema from string content.
pub fn parse_schema_content(content: &str) -> Result<EnvSchema, SchemaError> {
    let table: toml::Table = content
        .parse()
        .map_err(|e: toml::de::Error| SchemaError::ParseError(e.to_string()))?;

    let mut variables = HashMap::new();

    for (key, value) in &table {
        if let Some(tbl) = value.as_table() {
            // Check if this is an env override like [DB_URL.production]
            if key.contains('.') {
                continue; // Handled below with parent
            }
            let var = parse_variable(key, tbl)?;
            variables.insert(key.clone(), var);
        }
    }

    // Parse env overrides [VAR.environment]
    for (key, value) in &table {
        if let Some(dot_pos) = key.find('.') {
            let var_name = &key[..dot_pos];
            let env_name = &key[dot_pos + 1..];
            if let Some(tbl) = value.as_table() {
                if let Some(var) = variables.get_mut(var_name) {
                    let override_val = parse_override(tbl);
                    var.env_overrides.insert(env_name.to_string(), override_val);
                }
            }
        }
    }

    Ok(EnvSchema { variables })
}

fn parse_variable(name: &str, table: &toml::Table) -> Result<SchemaVariable, SchemaError> {
    let type_str = table
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("string");

    let var_type = VarType::parse(type_str).ok_or_else(|| SchemaError::UnknownType {
        var: name.to_string(),
        typ: type_str.to_string(),
    })?;

    Ok(SchemaVariable {
        var_type,
        required: table
            .get("required")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        default: table
            .get("default")
            .and_then(|v| v.as_str())
            .map(String::from),
        description: table
            .get("description")
            .and_then(|v| v.as_str())
            .map(String::from),
        example: table
            .get("example")
            .and_then(|v| v.as_str())
            .map(String::from),
        sensitive: table
            .get("sensitive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        pattern: table
            .get("pattern")
            .and_then(|v| v.as_str())
            .map(String::from),
        values: table.get("values").and_then(|v| {
            v.as_array().map(|arr| {
                arr.iter()
                    .filter_map(|item| item.as_str().map(String::from))
                    .collect()
            })
        }),
        min: table
            .get("min")
            .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))),
        max: table
            .get("max")
            .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))),
        env_overrides: HashMap::new(),
    })
}

fn parse_override(table: &toml::Table) -> SchemaOverride {
    SchemaOverride {
        var_type: table
            .get("type")
            .and_then(|v| v.as_str())
            .and_then(VarType::parse),
        required: table.get("required").and_then(|v| v.as_bool()),
        default: table
            .get("default")
            .and_then(|v| v.as_str())
            .map(String::from),
        pattern: table
            .get("pattern")
            .and_then(|v| v.as_str())
            .map(String::from),
        values: table.get("values").and_then(|v| {
            v.as_array().map(|arr| {
                arr.iter()
                    .filter_map(|item| item.as_str().map(String::from))
                    .collect()
            })
        }),
        min: table
            .get("min")
            .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))),
        max: table
            .get("max")
            .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64))),
    }
}

// ─── Validation ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SchemaValidationError {
    pub key: String,
    pub message: String,
    pub expected: String,
    pub actual: String,
}

/// Validate ENV vars against a schema, merged with config.toml validation rules.
pub fn validate_against_schema(
    env: &HashMap<String, String>,
    schema: &EnvSchema,
    environment: Option<&str>,
    config_rules: &HashMap<String, String>,
) -> Vec<SchemaValidationError> {
    let mut errors = Vec::new();

    // Check schema variables
    for (var_name, var_def) in &schema.variables {
        let effective = resolve_effective(var_def, environment);

        match env.get(var_name) {
            Some(value) => {
                if let Some(err) = validate_value(var_name, value, &effective) {
                    errors.push(err);
                }
            }
            None => {
                if effective.required && effective.default.is_none() {
                    errors.push(SchemaValidationError {
                        key: var_name.clone(),
                        message: "required variable is missing".into(),
                        expected: "a value".into(),
                        actual: "(not set)".into(),
                    });
                }
            }
        }
    }

    // Check config.toml rules for vars NOT in schema
    for (key, rule) in config_rules {
        if schema.variables.contains_key(key) {
            continue; // Schema takes priority
        }
        if let Some(value) = env.get(key) {
            if let Some(msg) = crate::ops::validation::validate_value(value, rule) {
                errors.push(SchemaValidationError {
                    key: key.clone(),
                    message: msg,
                    expected: rule.clone(),
                    actual: value.clone(),
                });
            }
        }
    }

    errors.sort_by(|a, b| a.key.cmp(&b.key));
    errors
}

pub(crate) struct EffectiveVar {
    pub(crate) var_type: VarType,
    pub(crate) required: bool,
    pub(crate) default: Option<String>,
    pub(crate) pattern: Option<String>,
    pub(crate) values: Option<Vec<String>>,
    pub(crate) min: Option<f64>,
    pub(crate) max: Option<f64>,
}

pub(crate) fn resolve_effective(var: &SchemaVariable, environment: Option<&str>) -> EffectiveVar {
    let mut eff = EffectiveVar {
        var_type: var.var_type.clone(),
        required: var.required,
        default: var.default.clone(),
        pattern: var.pattern.clone(),
        values: var.values.clone(),
        min: var.min,
        max: var.max,
    };

    if let Some(env_name) = environment {
        if let Some(ov) = var.env_overrides.get(env_name) {
            if let Some(ref t) = ov.var_type {
                eff.var_type = t.clone();
            }
            if let Some(r) = ov.required {
                eff.required = r;
            }
            if ov.default.is_some() {
                eff.default = ov.default.clone();
            }
            if ov.pattern.is_some() {
                eff.pattern = ov.pattern.clone();
            }
            if ov.values.is_some() {
                eff.values = ov.values.clone();
            }
            if ov.min.is_some() {
                eff.min = ov.min;
            }
            if ov.max.is_some() {
                eff.max = ov.max;
            }
        }
    }

    eff
}

pub(crate) fn validate_value(
    key: &str,
    value: &str,
    eff: &EffectiveVar,
) -> Option<SchemaValidationError> {
    match eff.var_type {
        VarType::Number => match value.parse::<f64>() {
            Ok(n) => {
                if let Some(min) = eff.min {
                    if n < min {
                        return Some(SchemaValidationError {
                            key: key.into(),
                            message: format!("value {} is below minimum {}", n, min),
                            expected: format!("number >= {}", min),
                            actual: value.into(),
                        });
                    }
                }
                if let Some(max) = eff.max {
                    if n > max {
                        return Some(SchemaValidationError {
                            key: key.into(),
                            message: format!("value {} exceeds maximum {}", n, max),
                            expected: format!("number <= {}", max),
                            actual: value.into(),
                        });
                    }
                }
                None
            }
            Err(_) => Some(SchemaValidationError {
                key: key.into(),
                message: format!("'{}' is not a valid number", value),
                expected: "number".into(),
                actual: value.into(),
            }),
        },
        VarType::Bool => {
            if crate::ops::validation_utils::is_valid_bool(value) {
                None
            } else {
                Some(SchemaValidationError {
                    key: key.into(),
                    message: format!("'{}' is not a valid boolean", value),
                    expected: "true/false/1/0/yes/no".into(),
                    actual: value.into(),
                })
            }
        }
        VarType::Url => {
            if crate::ops::validation_utils::is_valid_url(value) {
                if let Some(ref pattern) = eff.pattern {
                    return check_pattern(key, value, pattern);
                }
                None
            } else {
                Some(SchemaValidationError {
                    key: key.into(),
                    message: format!("'{}' is not a valid URL", value),
                    expected: "URL (e.g., http://, https://, postgres://)".into(),
                    actual: value.into(),
                })
            }
        }
        VarType::Email => {
            if crate::ops::validation_utils::is_valid_email(value) {
                None
            } else {
                Some(SchemaValidationError {
                    key: key.into(),
                    message: format!("'{}' is not a valid email", value),
                    expected: "email@domain.com".into(),
                    actual: value.into(),
                })
            }
        }
        VarType::Port => {
            if crate::ops::validation_utils::is_valid_port(value) {
                None
            } else {
                Some(SchemaValidationError {
                    key: key.into(),
                    message: format!("'{}' is not a valid port (1-65535)", value),
                    expected: "1-65535".into(),
                    actual: value.into(),
                })
            }
        }
        VarType::Enum => {
            if let Some(ref allowed) = eff.values {
                if allowed.contains(&value.to_string()) {
                    None
                } else {
                    Some(SchemaValidationError {
                        key: key.into(),
                        message: format!("'{}' is not one of: {}", value, allowed.join(", ")),
                        expected: allowed.join(", "),
                        actual: value.into(),
                    })
                }
            } else {
                None
            }
        }
        VarType::Regex => {
            if let Some(ref pattern) = eff.pattern {
                check_pattern(key, value, pattern)
            } else {
                None
            }
        }
        VarType::String => {
            if let Some(ref pattern) = eff.pattern {
                check_pattern(key, value, pattern)
            } else {
                None
            }
        }
    }
}

fn check_pattern(key: &str, value: &str, pattern: &str) -> Option<SchemaValidationError> {
    match regex::Regex::new(pattern) {
        Ok(re) => {
            if re.is_match(value) {
                None
            } else {
                Some(SchemaValidationError {
                    key: key.into(),
                    message: format!("'{}' does not match pattern '{}'", value, pattern),
                    expected: format!("matches /{}/", pattern),
                    actual: value.into(),
                })
            }
        }
        Err(e) => Some(SchemaValidationError {
            key: key.into(),
            message: format!("invalid regex pattern '{}': {}", pattern, e),
            expected: "valid regex".into(),
            actual: pattern.into(),
        }),
    }
}

// ─── Schema Generation ───────────────────────────────────────

/// Generate a schema from existing ENV vars using type heuristics.
pub fn generate_schema(env: &HashMap<String, String>) -> String {
    let sensitive_patterns = [
        "SECRET",
        "TOKEN",
        "PASSWORD",
        "KEY",
        "CREDENTIAL",
        "PRIVATE",
    ];
    let mut keys: Vec<&String> = env.keys().collect();
    keys.sort();

    let mut output = String::new();
    output.push_str("# Generated .env.schema — review and adjust types/requirements\n\n");

    for key in keys {
        let value = &env[key];
        let inferred_type = infer_type(value);
        let is_sensitive = sensitive_patterns
            .iter()
            .any(|p| key.to_uppercase().contains(p));

        output.push_str(&format!("[{}]\n", key));
        output.push_str(&format!("type = \"{}\"\n", inferred_type));
        output.push_str("required = false\n");
        if is_sensitive {
            output.push_str("sensitive = true\n");
        }
        output.push('\n');
    }

    output
}

fn infer_type(value: &str) -> &str {
    if value.parse::<f64>().is_ok() {
        return "number";
    }
    match value.to_lowercase().as_str() {
        "true" | "false" | "1" | "0" | "yes" | "no" => return "bool",
        _ => {}
    }
    if value.starts_with("http://") || value.starts_with("https://") {
        return "url";
    }
    if value.contains('@') && value.contains('.') && !value.contains(' ') {
        return "email";
    }
    if let Ok(port) = value.parse::<u16>() {
        if (1024..=65535).contains(&port) {
            return "port";
        }
    }
    "string"
}

// ─── Documentation Generation ────────────────────────────────

/// Generate Markdown documentation from a schema.
pub fn generate_docs(schema: &EnvSchema) -> String {
    let mut required: Vec<(&String, &SchemaVariable)> = schema
        .variables
        .iter()
        .filter(|(_, v)| v.required)
        .collect();
    let mut optional: Vec<(&String, &SchemaVariable)> = schema
        .variables
        .iter()
        .filter(|(_, v)| !v.required)
        .collect();

    required.sort_by_key(|(k, _)| k.to_string());
    optional.sort_by_key(|(k, _)| k.to_string());

    let mut output = String::new();
    output.push_str("# Environment Variables\n\n");
    output.push_str("| Variable | Type | Required | Default | Description |\n");
    output.push_str("|----------|------|----------|---------|-------------|\n");

    for (name, var) in required.iter().chain(optional.iter()) {
        let sensitive_marker = if var.sensitive { " [sensitive]" } else { "" };
        let default = var.default.as_deref().unwrap_or("—");
        let desc = var.description.as_deref().unwrap_or("");
        let req = if var.required { "Yes" } else { "No" };
        output.push_str(&format!(
            "| {}{} | {} | {} | {} | {} |\n",
            name,
            sensitive_marker,
            var.var_type.display(),
            req,
            default,
            desc
        ));
    }

    output
}

// ─── Drift Detection ─────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct DriftEntry {
    pub key: String,
    pub values: HashMap<String, Option<String>>,
    pub status: DriftStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DriftStatus {
    Same,
    Differs,
    Missing,
}

/// Compare multiple .env files against each other and optionally a schema.
pub fn detect_drift(
    env_files: &[(String, HashMap<String, String>)],
    schema: Option<&EnvSchema>,
) -> Vec<DriftEntry> {
    // Collect all keys
    let mut all_keys: Vec<String> = Vec::new();
    for (_, env) in env_files {
        for key in env.keys() {
            if !all_keys.contains(key) {
                all_keys.push(key.clone());
            }
        }
    }
    if let Some(s) = schema {
        for key in s.variables.keys() {
            if !all_keys.contains(key) {
                all_keys.push(key.clone());
            }
        }
    }
    all_keys.sort();

    let mut entries = Vec::new();
    for key in &all_keys {
        let mut values = HashMap::new();
        let mut found_values: Vec<Option<&str>> = Vec::new();

        for (env_name, env) in env_files {
            let val = env.get(key).map(|s| s.as_str());
            values.insert(env_name.clone(), val.map(String::from));
            found_values.push(val);
        }

        let status = if found_values.iter().any(|v| v.is_none()) {
            DriftStatus::Missing
        } else {
            let first = found_values[0];
            if found_values.iter().all(|v| *v == first) {
                DriftStatus::Same
            } else {
                DriftStatus::Differs
            }
        };

        entries.push(DriftEntry {
            key: key.clone(),
            values,
            status,
        });
    }

    entries
}

// ─── AI-Safe Context Emission ───────────────────────────────

/// Infer type and sensitivity from a key name and value (for entries without schema).
fn infer_type_for_ai(key: &str, value: &str) -> (&'static str, bool) {
    let lower_key = key.to_lowercase();
    let is_sensitive = crate::ops::dotenv::is_sensitive_key(key);

    let var_type = if value.parse::<bool>().is_ok() || value == "0" || value == "1" {
        "bool"
    } else if value.parse::<u16>().is_ok()
        && value.parse::<u16>().unwrap() > 0
        && lower_key.contains("port")
    {
        "port"
    } else if value.contains("://") {
        "url"
    } else if value.contains('@') && value.contains('.') {
        "email"
    } else if value.parse::<f64>().is_ok() {
        "number"
    } else {
        "string"
    };

    (var_type, is_sensitive)
}

/// Generate AI-safe context file from schema and/or current env vars.
/// Contains names, types, descriptions, sensitivity — NO values.
pub fn emit_ai_context(schema: Option<&EnvSchema>, entries: &[(String, String)]) -> String {
    let mut output = String::new();
    output.push_str("# Environment Variables (AI Context)\n");
    output.push_str("# Safe for AI tools — no secret values included.\n");
    output.push_str("# Generated by EnvForge\n");

    // Collect all variable names, schema first then inferred
    let mut seen = std::collections::HashSet::new();

    // Schema-defined variables
    if let Some(s) = schema {
        let mut schema_keys: Vec<&String> = s.variables.keys().collect();
        schema_keys.sort();

        for key in schema_keys {
            let var = &s.variables[key];
            seen.insert(key.clone());
            output.push('\n');
            output.push_str(&format!("## {}\n", key));
            output.push_str(&format!("- **Type**: {}\n", var.var_type.display()));
            if var.required {
                output.push_str("- **Required**: yes\n");
            }
            if let Some(ref desc) = var.description {
                output.push_str(&format!("- **Description**: {}\n", desc));
            }
            let is_sensitive = var.sensitive || crate::ops::dotenv::is_sensitive_key(key);
            if is_sensitive {
                output.push_str("- **Sensitive**: YES — do not hardcode or log\n");
            } else {
                output.push_str("- **Sensitive**: no\n");
            }
            if let Some(ref default) = var.default {
                output.push_str(&format!("- **Default**: {}\n", default));
            }
            if let Some(ref pattern) = var.pattern {
                output.push_str(&format!("- **Pattern**: {}\n", pattern));
            }
            if let Some(ref values) = var.values {
                output.push_str(&format!("- **Values**: {}\n", values.join(", ")));
            }
        }
    }

    // Inferred variables (entries not already covered by schema)
    let mut inferred: Vec<&(String, String)> =
        entries.iter().filter(|(k, _)| !seen.contains(k)).collect();
    inferred.sort_by_key(|(k, _)| k.clone());

    for (key, value) in inferred {
        let (var_type, is_sensitive) = infer_type_for_ai(key, value);
        output.push('\n');
        output.push_str(&format!("## {}\n", key));
        output.push_str(&format!("- **Type**: {}\n", var_type));
        if is_sensitive {
            output.push_str("- **Sensitive**: YES — do not hardcode or log\n");
        } else {
            output.push_str("- **Sensitive**: no\n");
        }
    }

    output
}

// ─── AI Context Auto-Update ─────────────────────────────────

/// Auto-update .env.ai.md in current directory if it exists.
/// Called after modifications to env vars.
pub fn auto_update_ai_context() {
    let cwd = std::env::current_dir().unwrap_or_default();
    let ai_file = cwd.join(".env.ai.md");
    if !ai_file.exists() {
        return; // Only update if file already exists
    }

    // Load schema if available
    let schema = find_schema().and_then(|p| parse_schema(&p).ok());

    // Load entries for inference
    if let Ok(config) = crate::config::load_or_create_default() {
        let mut shell_files = Vec::new();
        let primary = shellexpand_path(&config.files.primary);
        if primary.exists() {
            if let Ok(sf) = crate::parser::parse_shell_file(&primary) {
                shell_files.push(sf);
            }
        }
        let entries: Vec<(String, String)> = crate::ops::collect_all_entries(&shell_files)
            .into_iter()
            .filter(|e| e.location != crate::ops::EntryLocation::Commented)
            .map(|e| (e.key, e.value))
            .collect();

        let content = emit_ai_context(schema.as_ref(), &entries);
        let _ = std::fs::write(&ai_file, content);
    }
}

fn shellexpand_path(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

// ─── Find Schema ─────────────────────────────────────────────

/// Search for .env.schema in current directory and parents.
pub fn find_schema() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join(".env.schema");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emit_ai_context_generates_content() {
        let entries = vec![
            ("API_KEY".to_string(), "sk-proj-abc123".to_string()),
            ("PORT".to_string(), "3000".to_string()),
            (
                "DATABASE_URL".to_string(),
                "postgres://localhost/mydb".to_string(),
            ),
        ];
        let content = emit_ai_context(None, &entries);
        assert!(content.contains("# Environment Variables (AI Context)"));
        assert!(content.contains("## API_KEY"));
        assert!(content.contains("## PORT"));
        assert!(content.contains("## DATABASE_URL"));
        // Should not contain actual values
        assert!(!content.contains("sk-proj-abc123"));
        assert!(!content.contains("postgres://localhost/mydb"));
    }

    #[test]
    fn test_emit_ai_context_with_schema() {
        let mut variables = HashMap::new();
        variables.insert(
            "DB_HOST".to_string(),
            SchemaVariable {
                var_type: VarType::String,
                required: true,
                description: Some("Database hostname".to_string()),
                ..Default::default()
            },
        );
        let schema = EnvSchema { variables };
        let entries = vec![
            ("DB_HOST".to_string(), "localhost".to_string()),
            ("PORT".to_string(), "8080".to_string()),
        ];
        let content = emit_ai_context(Some(&schema), &entries);
        assert!(content.contains("## DB_HOST"));
        assert!(content.contains("**Required**: yes"));
        assert!(content.contains("Database hostname"));
        assert!(content.contains("## PORT"));
    }

    #[test]
    fn test_shellexpand_path_tilde() {
        let path = shellexpand_path("~/test/file");
        assert!(path.to_string_lossy().contains("test/file"));
        assert!(!path.to_string_lossy().starts_with("~/"));
    }

    #[test]
    fn test_shellexpand_path_no_tilde() {
        let path = shellexpand_path("/absolute/path");
        assert_eq!(path, PathBuf::from("/absolute/path"));
    }

    // ─── parse_schema_content ─────────────────────────────────

    #[test]
    fn test_parse_schema_basic() {
        let content = r#"
[DB_HOST]
type = "string"
required = true
description = "Database hostname"
"#;
        let schema = parse_schema_content(content).unwrap();
        assert_eq!(schema.variables.len(), 1);
        let var = schema.variables.get("DB_HOST").unwrap();
        assert_eq!(var.var_type, VarType::String);
        assert!(var.required);
        assert_eq!(var.description.as_deref(), Some("Database hostname"));
    }

    #[test]
    fn test_parse_schema_with_enum() {
        let content = r#"
[NODE_ENV]
type = "enum"
values = ["development", "staging", "production"]
required = true
"#;
        let schema = parse_schema_content(content).unwrap();
        let var = schema.variables.get("NODE_ENV").unwrap();
        assert_eq!(var.var_type, VarType::Enum);
        assert_eq!(
            var.values.as_ref().unwrap(),
            &vec!["development", "staging", "production"]
        );
    }

    // ─── VarType::parse ───────────────────────────────────────

    #[test]
    fn test_vartype_parse_all_known() {
        assert_eq!(VarType::parse("string"), Some(VarType::String));
        assert_eq!(VarType::parse("number"), Some(VarType::Number));
        assert_eq!(VarType::parse("bool"), Some(VarType::Bool));
        assert_eq!(VarType::parse("url"), Some(VarType::Url));
        assert_eq!(VarType::parse("email"), Some(VarType::Email));
        assert_eq!(VarType::parse("enum"), Some(VarType::Enum));
        assert_eq!(VarType::parse("port"), Some(VarType::Port));
    }

    #[test]
    fn test_vartype_parse_unknown() {
        assert_eq!(VarType::parse("unknown_type"), None);
    }

    // ─── validate_against_schema ──────────────────────────────

    #[test]
    fn test_validate_missing_required() {
        let content = r#"
[API_KEY]
type = "string"
required = true
"#;
        let schema = parse_schema_content(content).unwrap();
        let env: HashMap<String, String> = HashMap::new(); // no API_KEY set
        let errors = validate_against_schema(&env, &schema, None, &HashMap::new());
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.key == "API_KEY"));
    }

    #[test]
    fn test_validate_number_type() {
        let content = r#"
[PORT]
type = "number"
"#;
        let schema = parse_schema_content(content).unwrap();
        let mut env = HashMap::new();
        env.insert("PORT".to_string(), "not_a_number".to_string());
        let errors = validate_against_schema(&env, &schema, None, &HashMap::new());
        assert!(errors.iter().any(|e| e.key == "PORT"));
    }

    #[test]
    fn test_validate_bool_type() {
        let content = r#"
[DEBUG]
type = "bool"
"#;
        let schema = parse_schema_content(content).unwrap();
        // Valid bools should pass
        let mut env = HashMap::new();
        env.insert("DEBUG".to_string(), "true".to_string());
        let errors = validate_against_schema(&env, &schema, None, &HashMap::new());
        assert!(errors.is_empty());

        // Invalid bool should fail
        env.insert("DEBUG".to_string(), "maybe".to_string());
        let errors = validate_against_schema(&env, &schema, None, &HashMap::new());
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_validate_enum_type() {
        let content = r#"
[NODE_ENV]
type = "enum"
values = ["dev", "prod"]
"#;
        let schema = parse_schema_content(content).unwrap();
        let mut env = HashMap::new();
        env.insert("NODE_ENV".to_string(), "invalid".to_string());
        let errors = validate_against_schema(&env, &schema, None, &HashMap::new());
        assert!(errors.iter().any(|e| e.key == "NODE_ENV"));
    }

    // ─── generate_schema ──────────────────────────────────────

    #[test]
    fn test_generate_schema_infers_types() {
        let mut env = HashMap::new();
        env.insert("PORT".to_string(), "3000".to_string());
        env.insert("DEBUG".to_string(), "true".to_string());
        env.insert("DB_URL".to_string(), "https://db.example.com".to_string());
        let schema_str = generate_schema(&env);
        assert!(schema_str.contains("type = \"number\"") || schema_str.contains("type = \"port\""));
        assert!(schema_str.contains("type = \"bool\""));
        assert!(schema_str.contains("type = \"url\""));
    }

    #[test]
    fn test_generate_schema_detects_sensitive() {
        let mut env = HashMap::new();
        env.insert("SECRET_KEY".to_string(), "abc123".to_string());
        let schema_str = generate_schema(&env);
        assert!(schema_str.contains("sensitive = true"));
    }

    // ─── detect_drift ─────────────────────────────────────────

    #[test]
    fn test_drift_same_values() {
        let mut env1 = HashMap::new();
        env1.insert("A".to_string(), "1".to_string());
        let mut env2 = HashMap::new();
        env2.insert("A".to_string(), "1".to_string());
        let drift = detect_drift(
            &[("env1".to_string(), env1), ("env2".to_string(), env2)],
            None,
        );
        assert!(drift.iter().all(|d| matches!(d.status, DriftStatus::Same)));
    }

    #[test]
    fn test_drift_missing_and_differs() {
        let mut env1 = HashMap::new();
        env1.insert("A".to_string(), "1".to_string());
        env1.insert("B".to_string(), "old".to_string());
        let mut env2 = HashMap::new();
        env2.insert("B".to_string(), "new".to_string());
        env2.insert("C".to_string(), "3".to_string());
        let drift = detect_drift(
            &[("env1".to_string(), env1), ("env2".to_string(), env2)],
            None,
        );
        // A: only in env1 (missing from env2)
        // B: differs
        // C: only in env2 (missing from env1)
        assert!(drift.len() >= 2);
        assert!(drift
            .iter()
            .any(|d| matches!(d.status, DriftStatus::Differs)));
    }
}
