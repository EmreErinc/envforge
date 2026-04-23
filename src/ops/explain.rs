use std::path::PathBuf;

use crate::config::{load_or_create_default, AppConfig};
use crate::model::{ExportStyle, QuoteStyle};
use crate::ops::encrypt::is_encrypted;
use crate::ops::listing::{collect_all_entries, EntryLocation, EnvEntry};
use crate::ops::schema::{find_schema, parse_schema};
use crate::ops::secrets::age::load_sources;
use crate::ops::secrets::cache::{is_reference, SecretRef};
use crate::ops::sync::marking::get_key_status;
use crate::ops::sync::{is_initialized, read_config, sync_dir, KeyStatus, CONFIG_FILE};
use crate::parser::parse_shell_file;

// ─── Data Structures ────────────────────────────────────────

/// All known information about a single environment variable key.
#[derive(Debug, Clone)]
pub struct KeyExplanation {
    pub key: String,
    pub found: bool,
    pub sources: Vec<SourceInfo>,
    pub profile: Option<ProfileInfo>,
    pub schema: Option<SchemaInfo>,
    pub encrypted: bool,
    pub reference: Option<ReferenceInfo>,
    pub sync_status: Option<String>,
    pub age: Option<AgeInfo>,
    pub value_preview: String,
    pub similar_keys: Vec<String>,
}

/// Where the key is defined.
#[derive(Debug, Clone)]
pub struct SourceInfo {
    pub file: PathBuf,
    pub line_number: usize,
    pub export_style: String,
    pub quote_style: String,
    pub status: String,
    pub value: String,
}

/// Profile context for the key.
#[derive(Debug, Clone)]
pub struct ProfileInfo {
    pub profile_name: String,
    pub scope: String, // "shared" or "profile-specific"
}

/// Schema metadata for the key.
#[derive(Debug, Clone)]
pub struct SchemaInfo {
    pub var_type: String,
    pub required: bool,
    pub description: Option<String>,
    pub sensitive: bool,
    pub default: Option<String>,
    pub example: Option<String>,
    pub pattern: Option<String>,
    pub values: Option<Vec<String>>,
}

/// Secret reference details.
#[derive(Debug, Clone)]
pub struct ReferenceInfo {
    pub provider: String,
    pub path: String,
}

/// Age/freshness information for the key.
#[derive(Debug, Clone)]
pub struct AgeInfo {
    pub provider: String,
    pub days: i64,
    pub stale: bool,
    pub updated_at: String,
}

// ─── Core Logic ─────────────────────────────────────────────

/// Gather all known information about a single environment variable key.
pub fn explain_key(key: &str) -> KeyExplanation {
    let mut explanation = KeyExplanation {
        key: key.to_string(),
        found: false,
        sources: Vec::new(),
        profile: None,
        schema: None,
        encrypted: false,
        reference: None,
        sync_status: None,
        age: None,
        value_preview: String::new(),
        similar_keys: Vec::new(),
    };

    // Load context (config + shell files)
    let (config, shell_files) = match load_context() {
        Ok(ctx) => ctx,
        Err(_) => return explanation,
    };

    let entries = collect_all_entries(&shell_files);

    // 1. Source info — find all entries matching this key
    let matching: Vec<&EnvEntry> = entries.iter().filter(|e| e.key == key).collect();

    if matching.is_empty() {
        // Suggest similar keys
        explanation.similar_keys = find_similar_keys(key, &entries);
        return explanation;
    }

    explanation.found = true;

    for entry in &matching {
        let status = match entry.location {
            EntryLocation::InFile => "active",
            EntryLocation::InReference => "active (reference file)",
            EntryLocation::Commented => "commented",
        };
        let export_style = match entry.export_style {
            ExportStyle::Export => "export",
            ExportStyle::Bare => "bare",
        };
        let quote_style = match entry.quote_style {
            QuoteStyle::Double => "double-quoted",
            QuoteStyle::Single => "single-quoted",
            QuoteStyle::None => "unquoted",
        };

        explanation.sources.push(SourceInfo {
            file: entry.source_file.clone(),
            line_number: entry.line_number,
            export_style: export_style.to_string(),
            quote_style: quote_style.to_string(),
            status: status.to_string(),
            value: entry.value.clone(),
        });
    }

    // Use the first active entry for value-based checks
    let active_entry = matching
        .iter()
        .find(|e| e.location != EntryLocation::Commented)
        .or(matching.first())
        .unwrap();
    let value = &active_entry.value;

    // 2. Profile context
    explanation.profile = detect_profile_context(&config, active_entry);

    // 3. Schema info
    explanation.schema = load_schema_info(key);

    // 4. Encryption status
    explanation.encrypted = is_encrypted(value);

    // 5. Secret reference
    if is_reference(value) {
        if let Some(secret_ref) = SecretRef::parse(value) {
            explanation.reference = Some(ReferenceInfo {
                provider: secret_ref.provider,
                path: if secret_ref.path.is_empty() {
                    secret_ref.key
                } else {
                    format!("{}/{}", secret_ref.path, secret_ref.key)
                },
            });
        }
    }

    // 6. Sync status
    explanation.sync_status = load_sync_status(key);

    // 7. Age info
    explanation.age = load_age_info(key);

    // 8. Value preview (mask if sensitive)
    let is_sensitive = explanation
        .schema
        .as_ref()
        .map(|s| s.sensitive)
        .unwrap_or(false);
    explanation.value_preview = if explanation.encrypted {
        "ENC[age:...]".to_string()
    } else if is_sensitive {
        mask_value(value)
    } else {
        value.to_string()
    };

    explanation
}

/// Format explanation for terminal output with ANSI colors.
pub fn format_explanation(exp: &KeyExplanation) -> String {
    let mut out = String::new();

    // Header
    let header = format!(" KEY: {} ", exp.key);
    let bar_len = 50usize.saturating_sub(header.len());
    out.push_str(&format!(
        "\x1b[1m\x1b[36m──{}{}\x1b[0m\n\n",
        header,
        "─".repeat(bar_len)
    ));

    if !exp.found {
        out.push_str(&format!(
            "\x1b[31mKey '{}' not found in any managed file.\x1b[0m\n",
            exp.key
        ));
        if !exp.similar_keys.is_empty() {
            out.push_str("\nDid you mean:\n");
            for k in &exp.similar_keys {
                out.push_str(&format!("  - {}\n", k));
            }
        }
        return out;
    }

    // Source section
    out.push_str("\x1b[1mSource\x1b[0m\n");
    for (i, src) in exp.sources.iter().enumerate() {
        if exp.sources.len() > 1 {
            out.push_str(&format!("  [{}]\n", i + 1));
        }
        out.push_str(&format!(
            "  File:     {}:{}\n",
            src.file.display(),
            src.line_number
        ));
        out.push_str(&format!(
            "  Style:    {} {}\n",
            src.export_style, src.quote_style
        ));
        out.push_str(&format!("  Status:   {}\n", src.status));
    }

    // Profile section
    if let Some(ref profile) = exp.profile {
        out.push_str(&format!(
            "\n\x1b[1mProfile\x1b[0m\n  Profile:  {} ({})\n",
            profile.profile_name, profile.scope
        ));
    }

    // Schema section
    if let Some(ref schema) = exp.schema {
        out.push_str(&format!(
            "\n\x1b[1mSchema\x1b[0m\n  Type:     {}\n  Required: {}\n",
            schema.var_type,
            if schema.required { "yes" } else { "no" }
        ));
        if let Some(ref desc) = schema.description {
            out.push_str(&format!("  Desc:     {}\n", desc));
        }
        if schema.sensitive {
            out.push_str("  Sensitive: yes\n");
        }
        if let Some(ref default) = schema.default {
            out.push_str(&format!("  Default:  {}\n", default));
        }
        if let Some(ref example) = schema.example {
            out.push_str(&format!("  Example:  {}\n", example));
        }
        if let Some(ref pattern) = schema.pattern {
            out.push_str(&format!("  Pattern:  {}\n", pattern));
        }
        if let Some(ref values) = schema.values {
            out.push_str(&format!("  Values:   {}\n", values.join(", ")));
        }
    }

    // Encryption
    let enc_label = if exp.encrypted {
        "\x1b[33mencrypted (age)\x1b[0m"
    } else {
        "plaintext"
    };
    out.push_str(&format!("\n\x1b[1mEncryption:\x1b[0m {}\n", enc_label));

    // Reference
    let ref_label = if let Some(ref r) = exp.reference {
        format!("{} ({})", r.provider, r.path)
    } else {
        "none".to_string()
    };
    out.push_str(&format!("\x1b[1mReference:\x1b[0m  {}\n", ref_label));

    // Sync status
    let sync_label = exp.sync_status.as_deref().unwrap_or("N/A");
    out.push_str(&format!("\x1b[1mSync:\x1b[0m       {}\n", sync_label));

    // Age info
    if let Some(ref age) = exp.age {
        let stale_marker = if age.stale {
            "\x1b[31m(stale)\x1b[0m"
        } else {
            "\x1b[32m(ok)\x1b[0m"
        };
        out.push_str(&format!(
            "\x1b[1mAge:\x1b[0m        {} days {} — from {}\n",
            age.days, stale_marker, age.provider
        ));
    }

    // Value preview
    out.push_str(&format!(
        "\n\x1b[1mValue:\x1b[0m      {}\n",
        exp.value_preview
    ));

    out
}

/// Convert explanation to JSON.
pub fn explanation_to_json(exp: &KeyExplanation) -> serde_json::Value {
    let sources: Vec<serde_json::Value> = exp
        .sources
        .iter()
        .map(|s| {
            serde_json::json!({
                "file": s.file.to_string_lossy(),
                "line_number": s.line_number,
                "export_style": s.export_style,
                "quote_style": s.quote_style,
                "status": s.status,
            })
        })
        .collect();

    let mut json = serde_json::json!({
        "key": exp.key,
        "found": exp.found,
        "sources": sources,
        "encrypted": exp.encrypted,
        "value_preview": exp.value_preview,
    });

    let obj = json.as_object_mut().unwrap();

    if let Some(ref profile) = exp.profile {
        obj.insert(
            "profile".to_string(),
            serde_json::json!({
                "name": profile.profile_name,
                "scope": profile.scope,
            }),
        );
    }

    if let Some(ref schema) = exp.schema {
        let mut schema_json = serde_json::json!({
            "type": schema.var_type,
            "required": schema.required,
            "sensitive": schema.sensitive,
        });
        let s_obj = schema_json.as_object_mut().unwrap();
        if let Some(ref desc) = schema.description {
            s_obj.insert("description".to_string(), serde_json::json!(desc));
        }
        if let Some(ref default) = schema.default {
            s_obj.insert("default".to_string(), serde_json::json!(default));
        }
        if let Some(ref example) = schema.example {
            s_obj.insert("example".to_string(), serde_json::json!(example));
        }
        if let Some(ref pattern) = schema.pattern {
            s_obj.insert("pattern".to_string(), serde_json::json!(pattern));
        }
        if let Some(ref values) = schema.values {
            s_obj.insert("values".to_string(), serde_json::json!(values));
        }
        obj.insert("schema".to_string(), schema_json);
    }

    if let Some(ref r) = exp.reference {
        obj.insert(
            "reference".to_string(),
            serde_json::json!({
                "provider": r.provider,
                "path": r.path,
            }),
        );
    }

    if let Some(ref sync) = exp.sync_status {
        obj.insert("sync_status".to_string(), serde_json::json!(sync));
    }

    if let Some(ref age) = exp.age {
        obj.insert(
            "age".to_string(),
            serde_json::json!({
                "provider": age.provider,
                "days": age.days,
                "stale": age.stale,
                "updated_at": age.updated_at,
            }),
        );
    }

    if !exp.similar_keys.is_empty() {
        obj.insert(
            "similar_keys".to_string(),
            serde_json::json!(exp.similar_keys),
        );
    }

    json
}

// ─── Helpers ────────────────────────────────────────────────

fn load_context() -> Result<(AppConfig, Vec<crate::model::ShellFile>), super::OpError> {
    let config = load_or_create_default()?;
    let mut shell_files = Vec::new();

    let primary = shellexpand(&config.files.primary);
    if primary.exists() {
        shell_files.push(parse_shell_file(&primary)?);
    }

    let ref_path = shellexpand(&config.files.reference);
    if config.files.use_reference_file && ref_path.exists() {
        shell_files.push(parse_shell_file(&ref_path)?);
    }

    // Also load profile-specific and shared files
    let shared_path = shellexpand(&config.profiles.shared_file);
    if shared_path.exists() && shared_path != primary && shared_path != ref_path {
        if let Ok(sf) = parse_shell_file(&shared_path) {
            shell_files.push(sf);
        }
    }

    let active = &config.profiles.active;
    if let Some(entry) = config.profiles.entries.get(active) {
        let profile_path = shellexpand(&entry.file);
        if profile_path.exists()
            && profile_path != primary
            && profile_path != ref_path
            && profile_path != shared_path
        {
            if let Ok(sf) = parse_shell_file(&profile_path) {
                shell_files.push(sf);
            }
        }
    }

    Ok((config, shell_files))
}

fn shellexpand(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

fn detect_profile_context(config: &AppConfig, entry: &EnvEntry) -> Option<ProfileInfo> {
    let shared_path = shellexpand(&config.profiles.shared_file);
    if entry.source_file == shared_path {
        return Some(ProfileInfo {
            profile_name: "shared".to_string(),
            scope: "shared".to_string(),
        });
    }

    // Check each profile's file
    for (name, profile_entry) in &config.profiles.entries {
        let profile_path = shellexpand(&profile_entry.file);
        if entry.source_file == profile_path {
            return Some(ProfileInfo {
                profile_name: name.clone(),
                scope: "profile-specific".to_string(),
            });
        }
    }

    // If it's in the primary file, associate with active profile
    let primary_path = shellexpand(&config.files.primary);
    if entry.source_file == primary_path {
        return Some(ProfileInfo {
            profile_name: config.profiles.active.clone(),
            scope: "primary file".to_string(),
        });
    }

    let ref_path = shellexpand(&config.files.reference);
    if entry.source_file == ref_path {
        return Some(ProfileInfo {
            profile_name: config.profiles.active.clone(),
            scope: "reference file".to_string(),
        });
    }

    None
}

fn load_schema_info(key: &str) -> Option<SchemaInfo> {
    let schema_path = find_schema()?;
    let schema = parse_schema(&schema_path).ok()?;
    let var = schema.variables.get(key)?;

    Some(SchemaInfo {
        var_type: var.var_type.display().to_string(),
        required: var.required,
        description: var.description.clone(),
        sensitive: var.sensitive,
        default: var.default.clone(),
        example: var.example.clone(),
        pattern: var.pattern.clone(),
        values: var.values.clone(),
    })
}

fn load_sync_status(key: &str) -> Option<String> {
    let base_path = sync_dir().ok()?;
    if !is_initialized(&base_path) {
        return None;
    }
    let config_path = base_path.join(CONFIG_FILE);
    let config = read_config(&config_path).ok()?;
    let status = get_key_status(key, &config);
    Some(match status {
        KeyStatus::Synced => "synced".to_string(),
        KeyStatus::LocalOnly => "local-only".to_string(),
        KeyStatus::Unset => "untracked".to_string(),
    })
}

fn load_age_info(key: &str) -> Option<AgeInfo> {
    let sources = load_sources().ok()?;
    let secret_age = sources.secrets.get(key)?;

    let now = chrono::Utc::now();
    let days = if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&secret_age.updated_at) {
        now.signed_duration_since(dt).num_days()
    } else {
        -1
    };

    Some(AgeInfo {
        provider: secret_age.provider.clone(),
        days,
        stale: days >= 90,
        updated_at: secret_age.updated_at.clone(),
    })
}

fn mask_value(value: &str) -> String {
    let len = value.len();
    if len <= 4 {
        "*".repeat(len)
    } else {
        let visible = &value[..2];
        format!("{}{}...{}", visible, "*".repeat(4), &value[len - 2..])
    }
}

fn find_similar_keys(key: &str, entries: &[EnvEntry]) -> Vec<String> {
    let key_lower = key.to_lowercase();
    let mut seen = std::collections::HashSet::new();
    let mut similar: Vec<String> = entries
        .iter()
        .filter(|e| e.location != EntryLocation::Commented)
        .filter(|e| {
            let k = e.key.to_lowercase();
            k.contains(&key_lower) || key_lower.contains(&k) || levenshtein_close(&key_lower, &k)
        })
        .filter(|e| seen.insert(e.key.clone()))
        .map(|e| e.key.clone())
        .collect();
    similar.truncate(5);
    similar
}

/// Simple check: are two strings within edit distance 2?
fn levenshtein_close(a: &str, b: &str) -> bool {
    let len_diff = (a.len() as i32 - b.len() as i32).unsigned_abs() as usize;
    if len_diff > 2 {
        return false;
    }
    if a.len() > 20 || b.len() > 20 {
        return false; // skip long strings
    }
    levenshtein_distance(a, b) <= 2
}

fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let n = a.len();
    let m = b.len();
    let mut dp = vec![vec![0usize; m + 1]; n + 1];

    for i in 0..=n {
        for j in 0..=m {
            match (i, j) {
                (0, _) => dp[0][j] = j,
                (_, 0) => dp[i][0] = i,
                _ => {
                    let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
                    dp[i][j] = (dp[i - 1][j] + 1)
                        .min(dp[i][j - 1] + 1)
                        .min(dp[i - 1][j - 1] + cost);
                }
            }
        }
    }
    dp[n][m]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_value_short() {
        assert_eq!(mask_value("ab"), "**");
        assert_eq!(mask_value("abcd"), "****");
    }

    #[test]
    fn test_mask_value_long() {
        let masked = mask_value("secret_password_123");
        assert!(masked.starts_with("se"));
        assert!(masked.ends_with("23"));
        assert!(masked.contains("****"));
    }

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(levenshtein_distance("abc", "abc"), 0);
        assert_eq!(levenshtein_distance("abc", "abd"), 1);
    }

    #[test]
    fn test_levenshtein_close() {
        assert!(levenshtein_close("api_key", "api_ky"));
        assert!(levenshtein_close("database", "databse"));
        assert!(!levenshtein_close("short", "completely_different"));
    }

    #[test]
    fn test_explanation_to_json_not_found() {
        let exp = KeyExplanation {
            key: "MISSING".to_string(),
            found: false,
            sources: Vec::new(),
            profile: None,
            schema: None,
            encrypted: false,
            reference: None,
            sync_status: None,
            age: None,
            value_preview: String::new(),
            similar_keys: vec!["MISSING_KEY".to_string()],
        };
        let json = explanation_to_json(&exp);
        assert_eq!(json["key"], "MISSING");
        assert_eq!(json["found"], false);
        assert!(json["similar_keys"].is_array());
    }

    #[test]
    fn test_explanation_to_json_found() {
        let exp = KeyExplanation {
            key: "DB_URL".to_string(),
            found: true,
            sources: vec![SourceInfo {
                file: PathBuf::from("/home/user/.zshrc"),
                line_number: 15,
                export_style: "export".to_string(),
                quote_style: "double-quoted".to_string(),
                status: "active".to_string(),
                value: "postgres://localhost/db".to_string(),
            }],
            profile: Some(ProfileInfo {
                profile_name: "dev".to_string(),
                scope: "profile-specific".to_string(),
            }),
            schema: Some(SchemaInfo {
                var_type: "url".to_string(),
                required: true,
                description: Some("PostgreSQL connection string".to_string()),
                sensitive: false,
                default: None,
                example: Some("postgres://localhost:5432/mydb".to_string()),
                pattern: None,
                values: None,
            }),
            encrypted: false,
            reference: None,
            sync_status: Some("synced".to_string()),
            age: Some(AgeInfo {
                provider: "vault".to_string(),
                days: 45,
                stale: false,
                updated_at: "2026-03-01T00:00:00Z".to_string(),
            }),
            value_preview: "postgres://localhost/db".to_string(),
            similar_keys: Vec::new(),
        };
        let json = explanation_to_json(&exp);
        assert_eq!(json["key"], "DB_URL");
        assert_eq!(json["found"], true);
        assert_eq!(json["sync_status"], "synced");
        assert!(json["schema"]["required"].as_bool().unwrap());
        assert_eq!(json["age"]["days"], 45);
        assert_eq!(json["profile"]["name"], "dev");
    }

    #[test]
    fn test_format_explanation_not_found() {
        let exp = KeyExplanation {
            key: "MISSING".to_string(),
            found: false,
            sources: Vec::new(),
            profile: None,
            schema: None,
            encrypted: false,
            reference: None,
            sync_status: None,
            age: None,
            value_preview: String::new(),
            similar_keys: vec!["MISSING_VAR".to_string()],
        };
        let output = format_explanation(&exp);
        assert!(output.contains("MISSING"));
        assert!(output.contains("not found"));
        assert!(output.contains("MISSING_VAR"));
    }

    #[test]
    fn test_format_explanation_found() {
        let exp = KeyExplanation {
            key: "API_KEY".to_string(),
            found: true,
            sources: vec![SourceInfo {
                file: PathBuf::from("/home/user/.zshrc"),
                line_number: 10,
                export_style: "export".to_string(),
                quote_style: "double-quoted".to_string(),
                status: "active".to_string(),
                value: "sk-12345".to_string(),
            }],
            profile: None,
            schema: None,
            encrypted: false,
            reference: None,
            sync_status: None,
            age: None,
            value_preview: "sk-12345".to_string(),
            similar_keys: Vec::new(),
        };
        let output = format_explanation(&exp);
        assert!(output.contains("API_KEY"));
        assert!(output.contains("Source"));
        assert!(output.contains(".zshrc:10"));
        assert!(output.contains("plaintext"));
    }

    #[test]
    fn test_mask_value_empty() {
        assert_eq!(mask_value(""), "");
    }

    #[test]
    fn test_mask_value_single_char() {
        assert_eq!(mask_value("x"), "*");
    }

    #[test]
    fn test_mask_value_exactly_four() {
        assert_eq!(mask_value("abcd"), "****");
    }

    #[test]
    fn test_mask_value_five_chars() {
        let masked = mask_value("abcde");
        assert!(masked.starts_with("ab"));
        assert!(masked.ends_with("de"));
        assert!(masked.contains("****"));
    }

    #[test]
    fn test_levenshtein_distance_empty_strings() {
        assert_eq!(levenshtein_distance("", ""), 0);
        assert_eq!(levenshtein_distance("abc", ""), 3);
        assert_eq!(levenshtein_distance("", "xyz"), 3);
    }

    #[test]
    fn test_levenshtein_distance_identical() {
        assert_eq!(levenshtein_distance("hello", "hello"), 0);
    }

    #[test]
    fn test_levenshtein_distance_single_edit() {
        assert_eq!(levenshtein_distance("cat", "car"), 1);
        assert_eq!(levenshtein_distance("cat", "cats"), 1);
        assert_eq!(levenshtein_distance("cats", "cat"), 1);
    }

    #[test]
    fn test_levenshtein_close_long_strings_skipped() {
        // Strings > 20 chars always return false
        let long_a = "a".repeat(25);
        let long_b = "a".repeat(25);
        assert!(!levenshtein_close(&long_a, &long_b));
    }

    #[test]
    fn test_levenshtein_close_big_length_diff() {
        assert!(!levenshtein_close("ab", "abcdef"));
    }

    #[test]
    fn test_format_explanation_encrypted() {
        let exp = KeyExplanation {
            key: "SECRET".to_string(),
            found: true,
            sources: vec![SourceInfo {
                file: PathBuf::from("/test"),
                line_number: 1,
                export_style: "export".to_string(),
                quote_style: "double-quoted".to_string(),
                status: "active".to_string(),
                value: "ENC[age:...]".to_string(),
            }],
            profile: None,
            schema: Some(SchemaInfo {
                var_type: "string".to_string(),
                required: true,
                description: Some("A secret key".to_string()),
                sensitive: true,
                default: Some("none".to_string()),
                example: Some("sk-xxx".to_string()),
                pattern: Some("^sk-".to_string()),
                values: Some(vec!["a".to_string(), "b".to_string()]),
            }),
            encrypted: true,
            reference: Some(ReferenceInfo {
                provider: "vault".to_string(),
                path: "secret/data/mykey".to_string(),
            }),
            sync_status: Some("synced".to_string()),
            age: Some(AgeInfo {
                provider: "vault".to_string(),
                days: 100,
                stale: true,
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            }),
            value_preview: "ENC[age:...]".to_string(),
            similar_keys: Vec::new(),
        };
        let output = format_explanation(&exp);
        assert!(output.contains("encrypted"));
        assert!(output.contains("vault"));
        assert!(output.contains("synced"));
        assert!(output.contains("stale"));
        assert!(output.contains("100 days"));
        assert!(output.contains("Schema"));
        assert!(output.contains("Sensitive"));
        assert!(output.contains("Pattern"));
        assert!(output.contains("Values"));
        assert!(output.contains("Default"));
        assert!(output.contains("Example"));
    }

    #[test]
    fn test_explanation_to_json_with_reference() {
        let exp = KeyExplanation {
            key: "REF_KEY".to_string(),
            found: true,
            sources: vec![],
            profile: None,
            schema: None,
            encrypted: false,
            reference: Some(ReferenceInfo {
                provider: "aws-ssm".to_string(),
                path: "/prod/api-key".to_string(),
            }),
            sync_status: None,
            age: None,
            value_preview: "ref:aws-ssm:/prod/api-key".to_string(),
            similar_keys: Vec::new(),
        };
        let json = explanation_to_json(&exp);
        assert_eq!(json["reference"]["provider"], "aws-ssm");
        assert_eq!(json["reference"]["path"], "/prod/api-key");
    }

    #[test]
    fn test_format_explanation_multiple_sources() {
        let exp = KeyExplanation {
            key: "MULTI".to_string(),
            found: true,
            sources: vec![
                SourceInfo {
                    file: PathBuf::from("/a"),
                    line_number: 1,
                    export_style: "export".to_string(),
                    quote_style: "double-quoted".to_string(),
                    status: "active".to_string(),
                    value: "val1".to_string(),
                },
                SourceInfo {
                    file: PathBuf::from("/b"),
                    line_number: 5,
                    export_style: "bare".to_string(),
                    quote_style: "unquoted".to_string(),
                    status: "active".to_string(),
                    value: "val2".to_string(),
                },
            ],
            profile: Some(ProfileInfo {
                profile_name: "dev".to_string(),
                scope: "shared".to_string(),
            }),
            schema: None,
            encrypted: false,
            reference: None,
            sync_status: None,
            age: None,
            value_preview: "val1".to_string(),
            similar_keys: Vec::new(),
        };
        let output = format_explanation(&exp);
        // Multiple sources should show numbered entries
        assert!(output.contains("[1]"));
        assert!(output.contains("[2]"));
        assert!(output.contains("Profile"));
        assert!(output.contains("shared"));
    }
}
