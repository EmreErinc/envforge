use std::collections::BTreeMap;

use crate::ops::listing::{EntryLocation, EnvEntry};

/// Supported export formats.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportFormat {
    /// Standard .env format (default)
    Dotenv,
    /// JSON object { "KEY": "VALUE" }
    Json,
    /// YAML mapping
    Yaml,
    /// TOML key-value pairs
    Toml,
    /// Docker --env-file format (KEY=VALUE, no quotes, no comments)
    Docker,
    /// Kubernetes Secret manifest (base64-encoded)
    K8s,
    /// Terraform tfvars format
    Tfvars,
    /// Docker Compose secrets - individual files in a directory
    DockerSecrets,
}

impl ExportFormat {
    /// Parse format name from string.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "dotenv" | "env" | ".env" => Ok(Self::Dotenv),
            "json" => Ok(Self::Json),
            "yaml" | "yml" => Ok(Self::Yaml),
            "toml" => Ok(Self::Toml),
            "docker" | "docker-env" => Ok(Self::Docker),
            "k8s" | "kubernetes" | "k8s-secret" => Ok(Self::K8s),
            "tfvars" | "terraform" | "tf" => Ok(Self::Tfvars),
            "docker-secrets" | "compose-secrets" => Ok(Self::DockerSecrets),
            _ => Err(format!(
                "Unknown format '{}'. Supported: dotenv, json, yaml, toml, docker, k8s, tfvars, docker-secrets",
                s
            )),
        }
    }

    /// File extension for this format.
    pub fn extension(&self) -> &str {
        match self {
            Self::Dotenv => ".env",
            Self::Json => ".json",
            Self::Yaml => ".yaml",
            Self::Toml => ".toml",
            Self::Docker => ".env",
            Self::K8s => ".yaml",
            Self::Tfvars => ".tfvars",
            Self::DockerSecrets => "",
        }
    }
}

/// Convert entries to ordered map, skipping commented.
fn entries_to_map(entries: &[EnvEntry]) -> BTreeMap<String, String> {
    entries
        .iter()
        .filter(|e| e.location != EntryLocation::Commented)
        .map(|e| (e.key.clone(), e.value.clone()))
        .collect()
}

/// Export entries in the specified format.
pub fn export_as(entries: &[EnvEntry], format: &ExportFormat, k8s_name: Option<&str>, k8s_namespace: Option<&str>) -> String {
    match format {
        ExportFormat::Dotenv => export_dotenv(entries),
        ExportFormat::Json => export_json(entries),
        ExportFormat::Yaml => export_yaml(entries),
        ExportFormat::Toml => export_toml(entries),
        ExportFormat::Docker => export_docker(entries),
        ExportFormat::K8s => export_k8s(entries, k8s_name, k8s_namespace),
        ExportFormat::Tfvars => export_tfvars(entries),
        ExportFormat::DockerSecrets => export_docker_secrets(entries),
    }
}

fn export_dotenv(entries: &[EnvEntry]) -> String {
    let map = entries_to_map(entries);
    let mut out = String::new();
    for (k, v) in &map {
        if v.contains(' ') || v.contains('#') || v.contains('"') || v.contains('\'') || v.contains('\n') || v.is_empty() {
            out.push_str(&format!("{}=\"{}\"\n", k, v.replace('"', "\\\"")));
        } else {
            out.push_str(&format!("{}={}\n", k, v));
        }
    }
    out
}

fn export_json(entries: &[EnvEntry]) -> String {
    let map = entries_to_map(entries);
    let json_map: serde_json::Map<String, serde_json::Value> = map
        .into_iter()
        .map(|(k, v)| (k, serde_json::Value::String(v)))
        .collect();
    serde_json::to_string_pretty(&json_map).unwrap_or_default() + "\n"
}

fn export_yaml(entries: &[EnvEntry]) -> String {
    let map = entries_to_map(entries);
    let mut out = String::new();
    for (k, v) in &map {
        // YAML: quote values that could be misinterpreted
        if needs_yaml_quoting(v) {
            out.push_str(&format!("{}: \"{}\"\n", k, v.replace('"', "\\\"")));
        } else {
            out.push_str(&format!("{}: {}\n", k, v));
        }
    }
    out
}

fn needs_yaml_quoting(v: &str) -> bool {
    if v.is_empty() {
        return true;
    }
    // YAML special values
    let lower = v.to_lowercase();
    if matches!(lower.as_str(), "true" | "false" | "yes" | "no" | "null" | "~" | "on" | "off") {
        return true;
    }
    // Numeric strings
    if v.parse::<f64>().is_ok() {
        return true;
    }
    // Contains special chars
    v.contains(':') || v.contains('#') || v.contains('{') || v.contains('}')
        || v.contains('[') || v.contains(']') || v.contains(',') || v.contains('&')
        || v.contains('*') || v.contains('!') || v.contains('|') || v.contains('>')
        || v.contains('\'') || v.contains('"') || v.contains('%') || v.contains('@')
        || v.contains('`') || v.contains('\n') || v.starts_with(' ') || v.ends_with(' ')
}

fn export_toml(entries: &[EnvEntry]) -> String {
    let map = entries_to_map(entries);
    let mut out = String::new();
    for (k, v) in &map {
        // TOML: all string values must be quoted
        out.push_str(&format!("{} = \"{}\"\n", k, v.replace('\\', "\\\\").replace('"', "\\\"")));
    }
    out
}

fn export_docker(entries: &[EnvEntry]) -> String {
    // Docker env-file: KEY=VALUE, no quotes, no comments, no export
    let map = entries_to_map(entries);
    let mut out = String::new();
    for (k, v) in &map {
        out.push_str(&format!("{}={}\n", k, v));
    }
    out
}

fn export_k8s(entries: &[EnvEntry], name: Option<&str>, namespace: Option<&str>) -> String {
    use base64::Engine;
    let map = entries_to_map(entries);

    let secret_name = name.unwrap_or("envforge-secrets");
    let ns = namespace.unwrap_or("default");

    let mut out = String::new();
    out.push_str("apiVersion: v1\n");
    out.push_str("kind: Secret\n");
    out.push_str("metadata:\n");
    out.push_str(&format!("  name: {}\n", secret_name));
    out.push_str(&format!("  namespace: {}\n", ns));
    out.push_str("type: Opaque\n");
    out.push_str("data:\n");

    for (k, v) in &map {
        let encoded = base64::engine::general_purpose::STANDARD.encode(v.as_bytes());
        out.push_str(&format!("  {}: {}\n", k, encoded));
    }

    out
}

fn export_tfvars(entries: &[EnvEntry]) -> String {
    let map = entries_to_map(entries);
    let mut out = String::new();
    for (k, v) in &map {
        // Terraform: variable_name = "value" (lowercase convention)
        let tf_key = k.to_lowercase();
        out.push_str(&format!("{} = \"{}\"\n", tf_key, v.replace('\\', "\\\\").replace('"', "\\\"")));
    }
    out
}

fn export_docker_secrets(entries: &[EnvEntry]) -> String {
    let map = entries_to_map(entries);
    let mut out = String::new();
    out.push_str("#!/bin/bash\n");
    out.push_str("# Generated by EnvForge - Docker Compose secrets\n");
    out.push_str("mkdir -p ./secrets\n\n");
    for (k, v) in &map {
        // Escape single quotes in value
        let escaped = v.replace('\'', "'\\''");
        out.push_str(&format!("echo '{}' > ./secrets/{}\n", escaped, k));
    }
    out.push_str("\n# docker-compose.yml snippet:\n");
    out.push_str("# services:\n");
    out.push_str("#   app:\n");
    out.push_str("#     secrets:\n");
    for k in map.keys() {
        out.push_str(&format!("#       - {}\n", k));
    }
    out.push_str("# secrets:\n");
    for k in map.keys() {
        out.push_str(&format!("#   {}:\n#     file: ./secrets/{}\n", k, k));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use crate::model::{ExportStyle, QuoteStyle};

    fn make_entries() -> Vec<EnvEntry> {
        vec![
            EnvEntry {
                key: "DB_HOST".to_string(),
                value: "localhost".to_string(),
                source_file: PathBuf::from("/test"),
                line_number: 1,
                location: EntryLocation::InFile,
                export_style: ExportStyle::Export,
                quote_style: QuoteStyle::Double,
                is_dirty: false,
            },
            EnvEntry {
                key: "API_KEY".to_string(),
                value: "sk-abc123".to_string(),
                source_file: PathBuf::from("/test"),
                line_number: 2,
                location: EntryLocation::InFile,
                export_style: ExportStyle::Export,
                quote_style: QuoteStyle::Double,
                is_dirty: false,
            },
            EnvEntry {
                key: "DEBUG".to_string(),
                value: "true".to_string(),
                source_file: PathBuf::from("/test"),
                line_number: 3,
                location: EntryLocation::InFile,
                export_style: ExportStyle::Export,
                quote_style: QuoteStyle::Double,
                is_dirty: false,
            },
            EnvEntry {
                key: "DELETED".to_string(),
                value: "gone".to_string(),
                source_file: PathBuf::from("/test"),
                line_number: 4,
                location: EntryLocation::Commented,
                export_style: ExportStyle::Export,
                quote_style: QuoteStyle::Double,
                is_dirty: false,
            },
        ]
    }

    #[test]
    fn test_format_parse() {
        assert_eq!(ExportFormat::parse("json").unwrap(), ExportFormat::Json);
        assert_eq!(ExportFormat::parse("yaml").unwrap(), ExportFormat::Yaml);
        assert_eq!(ExportFormat::parse("yml").unwrap(), ExportFormat::Yaml);
        assert_eq!(ExportFormat::parse("toml").unwrap(), ExportFormat::Toml);
        assert_eq!(ExportFormat::parse("docker").unwrap(), ExportFormat::Docker);
        assert_eq!(ExportFormat::parse("k8s").unwrap(), ExportFormat::K8s);
        assert_eq!(ExportFormat::parse("kubernetes").unwrap(), ExportFormat::K8s);
        assert_eq!(ExportFormat::parse("tfvars").unwrap(), ExportFormat::Tfvars);
        assert_eq!(ExportFormat::parse("terraform").unwrap(), ExportFormat::Tfvars);
        assert_eq!(ExportFormat::parse("tf").unwrap(), ExportFormat::Tfvars);
        assert!(ExportFormat::parse("xml").is_err());
    }

    #[test]
    fn test_export_json() {
        let entries = make_entries();
        let output = export_json(&entries);
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["DB_HOST"], "localhost");
        assert_eq!(parsed["API_KEY"], "sk-abc123");
        assert_eq!(parsed["DEBUG"], "true");
        // Commented entries should be excluded
        assert!(parsed.get("DELETED").is_none());
    }

    #[test]
    fn test_export_yaml() {
        let entries = make_entries();
        let output = export_yaml(&entries);
        assert!(output.contains("DB_HOST: localhost"));
        assert!(output.contains("API_KEY: sk-abc123"));
        // "true" should be quoted in YAML to avoid bool interpretation
        assert!(output.contains("DEBUG: \"true\""));
        assert!(!output.contains("DELETED"));
    }

    #[test]
    fn test_export_toml() {
        let entries = make_entries();
        let output = export_toml(&entries);
        assert!(output.contains("DB_HOST = \"localhost\""));
        assert!(output.contains("API_KEY = \"sk-abc123\""));
        assert!(!output.contains("DELETED"));
    }

    #[test]
    fn test_export_docker() {
        let entries = make_entries();
        let output = export_docker(&entries);
        assert!(output.contains("DB_HOST=localhost"));
        assert!(output.contains("API_KEY=sk-abc123"));
        // No quotes, no comments
        assert!(!output.contains('"'));
        assert!(!output.contains('#'));
        assert!(!output.contains("DELETED"));
    }

    #[test]
    fn test_export_k8s() {
        let entries = make_entries();
        let output = export_k8s(&entries, Some("my-secrets"), Some("production"));
        assert!(output.contains("apiVersion: v1"));
        assert!(output.contains("kind: Secret"));
        assert!(output.contains("name: my-secrets"));
        assert!(output.contains("namespace: production"));
        assert!(output.contains("type: Opaque"));
        // Values should be base64
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(b"localhost");
        assert!(output.contains(&format!("DB_HOST: {}", encoded)));
        assert!(!output.contains("DELETED"));
    }

    #[test]
    fn test_export_k8s_defaults() {
        let entries = make_entries();
        let output = export_k8s(&entries, None, None);
        assert!(output.contains("name: envforge-secrets"));
        assert!(output.contains("namespace: default"));
    }

    #[test]
    fn test_export_tfvars() {
        let entries = make_entries();
        let output = export_tfvars(&entries);
        // Keys should be lowercase
        assert!(output.contains("db_host = \"localhost\""));
        assert!(output.contains("api_key = \"sk-abc123\""));
        assert!(!output.contains("DELETED"));
    }

    #[test]
    fn test_export_dotenv() {
        let entries = make_entries();
        let output = export_dotenv(&entries);
        assert!(output.contains("DB_HOST=localhost"));
        assert!(!output.contains("DELETED"));
    }

    #[test]
    fn test_export_value_with_spaces() {
        let entries = vec![EnvEntry {
            key: "MSG".to_string(),
            value: "hello world".to_string(),
            source_file: PathBuf::from("/test"),
            line_number: 1,
            location: EntryLocation::InFile,
            export_style: ExportStyle::Export,
            quote_style: QuoteStyle::Double,
            is_dirty: false,
        }];
        let dotenv = export_dotenv(&entries);
        assert!(dotenv.contains("MSG=\"hello world\""));

        let docker = export_docker(&entries);
        assert!(docker.contains("MSG=hello world"));
    }

    #[test]
    fn test_export_docker_secrets() {
        let entries = make_entries();
        let output = export_docker_secrets(&entries);
        // Verify script header
        assert!(output.starts_with("#!/bin/bash\n"));
        assert!(output.contains("mkdir -p ./secrets"));
        // Verify secret files are created
        assert!(output.contains("echo 'localhost' > ./secrets/DB_HOST"));
        assert!(output.contains("echo 'sk-abc123' > ./secrets/API_KEY"));
        assert!(output.contains("echo 'true' > ./secrets/DEBUG"));
        // Commented entries should be excluded
        assert!(!output.contains("DELETED"));
        // Verify compose snippet present
        assert!(output.contains("# docker-compose.yml snippet:"));
        assert!(output.contains("#       - DB_HOST"));
        assert!(output.contains("#   DB_HOST:\n#     file: ./secrets/DB_HOST"));
    }

    #[test]
    fn test_export_docker_secrets_escapes_quotes() {
        let entries = vec![EnvEntry {
            key: "MSG".to_string(),
            value: "it's a test".to_string(),
            source_file: PathBuf::from("/test"),
            line_number: 1,
            location: EntryLocation::InFile,
            export_style: ExportStyle::Export,
            quote_style: QuoteStyle::Double,
            is_dirty: false,
        }];
        let output = export_docker_secrets(&entries);
        assert!(output.contains("echo 'it'\\''s a test' > ./secrets/MSG"));
    }

    #[test]
    fn test_format_parse_docker_secrets() {
        assert_eq!(ExportFormat::parse("docker-secrets").unwrap(), ExportFormat::DockerSecrets);
        assert_eq!(ExportFormat::parse("compose-secrets").unwrap(), ExportFormat::DockerSecrets);
    }

    #[test]
    fn test_export_value_with_quotes() {
        let entries = vec![EnvEntry {
            key: "MSG".to_string(),
            value: "say \"hello\"".to_string(),
            source_file: PathBuf::from("/test"),
            line_number: 1,
            location: EntryLocation::InFile,
            export_style: ExportStyle::Export,
            quote_style: QuoteStyle::Double,
            is_dirty: false,
        }];
        let json = export_json(&entries);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["MSG"], "say \"hello\"");

        let toml_out = export_toml(&entries);
        assert!(toml_out.contains("MSG = \"say \\\"hello\\\"\""));
    }
}
