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

struct EffectiveVar {
    var_type: VarType,
    required: bool,
    default: Option<String>,
    pattern: Option<String>,
    values: Option<Vec<String>>,
    min: Option<f64>,
    max: Option<f64>,
}

fn resolve_effective(var: &SchemaVariable, environment: Option<&str>) -> EffectiveVar {
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

fn validate_value(key: &str, value: &str, eff: &EffectiveVar) -> Option<SchemaValidationError> {
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
        VarType::Bool => match value.to_lowercase().as_str() {
            "true" | "false" | "1" | "0" | "yes" | "no" => None,
            _ => Some(SchemaValidationError {
                key: key.into(),
                message: format!("'{}' is not a valid boolean", value),
                expected: "true/false/1/0/yes/no".into(),
                actual: value.into(),
            }),
        },
        VarType::Url => {
            if value.starts_with("http://") || value.starts_with("https://") {
                if let Some(ref pattern) = eff.pattern {
                    return check_pattern(key, value, pattern);
                }
                None
            } else {
                Some(SchemaValidationError {
                    key: key.into(),
                    message: format!("'{}' is not a valid URL", value),
                    expected: "http:// or https://".into(),
                    actual: value.into(),
                })
            }
        }
        VarType::Email => {
            if value.contains('@') && value.contains('.') {
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
        VarType::Port => match value.parse::<u16>() {
            Ok(_) => None,
            _ => Some(SchemaValidationError {
                key: key.into(),
                message: format!("'{}' is not a valid port (1-65535)", value),
                expected: "1-65535".into(),
                actual: value.into(),
            }),
        },
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
