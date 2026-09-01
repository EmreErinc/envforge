#![allow(deprecated)]
use std::path::Path;

use crate::ops::secrets::credentials::read_all_credentials;

use super::OpError;
use crate::ops::secrets::provider::ProviderRegistry;

/// A parsed secret URI.
#[derive(Debug, Clone, PartialEq)]
pub struct SecretUri {
    pub provider: String, // "vault", "aws-ssm", etc.
    pub path: String,     // full path including key
}

/// Known provider schemes that are valid in secret URIs.
const VALID_PROVIDERS: &[&str] = &[
    "vault",
    "aws-ssm",
    "1password",
    "doppler",
    "infisical",
    "gcp",
    "azure",
];

/// Parse a secret URI string.
/// Format: provider://path
pub fn parse_secret_uri(uri: &str) -> Option<SecretUri> {
    let parts: Vec<&str> = uri.splitn(2, "://").collect();
    if parts.len() != 2 {
        return None;
    }
    let provider = parts[0];
    let path = parts[1].trim_start_matches('/');

    if !VALID_PROVIDERS.contains(&provider) {
        return None;
    }

    if path.is_empty() {
        return None;
    }

    Some(SecretUri {
        provider: provider.to_string(),
        path: path.to_string(),
    })
}

/// Check if a string looks like a secret URI.
pub fn is_secret_uri(value: &str) -> bool {
    value.contains("://") && parse_secret_uri(value).is_some()
}

/// Parse a key=value file where values may be secret URIs.
/// Returns (key, value_or_uri) pairs.
pub fn parse_uri_file(path: &Path) -> Result<Vec<(String, String)>, OpError> {
    let content = std::fs::read_to_string(path)?;
    Ok(parse_uri_content(&content))
}

/// Parse key=value content (testable without filesystem).
pub fn parse_uri_content(content: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(eq_pos) = trimmed.find('=') {
            let key = trimmed[..eq_pos].trim().to_string();
            let value = trimmed[eq_pos + 1..].trim().to_string();
            let value = if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                value[1..value.len() - 1].to_string()
            } else {
                value
            };
            if !key.is_empty() {
                entries.push((key, value));
            }
        }
    }

    entries
}

/// Result of resolving a single entry.
///
/// **`Debug` is implemented manually** to mask the secret `value` field.
/// Without this, any `format!("{:?}", entry)`, panic message, `dbg!()`,
/// or log statement would leak the resolved plaintext. Callers that
/// truly need the value access it through the public field directly.
#[derive(Clone)]
pub struct ResolvedEntry {
    pub key: String,
    pub value: String,
    pub was_uri: bool,
    pub error: Option<String>,
}

impl std::fmt::Debug for ResolvedEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value_preview = if self.value.is_empty() {
            "<empty>".to_string()
        } else {
            format!("***({} chars)", self.value.chars().count())
        };
        f.debug_struct("ResolvedEntry")
            .field("key", &self.key)
            .field("value", &value_preview)
            .field("was_uri", &self.was_uri)
            .field("error", &self.error)
            .finish()
    }
}

/// Resolve all secret URIs in a list of entries.
/// Returns resolved entries (URIs replaced with actual values).
pub fn resolve_uris(
    entries: &[(String, String)],
    registry: &ProviderRegistry,
) -> Vec<ResolvedEntry> {
    let mut results = Vec::new();

    for (key, value) in entries {
        if let Some(uri) = parse_secret_uri(value) {
            let registry_name = map_provider_name(&uri.provider);

            match resolve_single_uri(&uri, registry_name, registry) {
                Ok(resolved) => {
                    results.push(ResolvedEntry {
                        key: key.clone(),
                        value: resolved,
                        was_uri: true,
                        error: None,
                    });
                }
                Err(e) => {
                    results.push(ResolvedEntry {
                        key: key.clone(),
                        value: value.clone(),
                        was_uri: true,
                        error: Some(e),
                    });
                }
            }
        } else {
            results.push(ResolvedEntry {
                key: key.clone(),
                value: value.clone(),
                was_uri: false,
                error: None,
            });
        }
    }

    results
}

fn map_provider_name(scheme: &str) -> &str {
    scheme
}

/// Resolve a single URI to its secret value.
fn resolve_single_uri(
    uri: &SecretUri,
    registry_name: &str,
    registry: &ProviderRegistry,
) -> Result<String, String> {
    validate_uri_path(&uri.path)?;

    let provider = registry.get(registry_name).map_err(|e| format!("{}", e))?;

    let credentials = read_all_credentials(registry_name)
        .map_err(|e| format!("credentials for '{}': {}", registry_name, e))?;

    // Split path into dir + key at the last slash
    let (dir, secret_key) = if let Some(last_slash) = uri.path.rfind('/') {
        (&uri.path[..last_slash], &uri.path[last_slash + 1..])
    } else {
        ("", uri.path.as_str())
    };

    provider
        .get(&credentials, dir, secret_key)
        .map_err(|e| format!("{}", e))
}

/// Reject `..` segments, leading `/`, double slashes, and control chars
/// in a URI path before it's handed to a provider backend. Some providers
/// (e.g. raw Vault KVv2 calls) do not collapse `..`, so a path of
/// `secret/../../sys/` could escape the intended scope.
fn validate_uri_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("URI path cannot be empty".to_string());
    }
    if path.starts_with('/') {
        return Err("URI path must not be absolute".to_string());
    }
    if path.contains("//") {
        return Err("URI path must not contain '//'".to_string());
    }
    for segment in path.split('/') {
        if segment == ".." || segment == "." {
            return Err(format!(
                "URI path must not contain '{}' segments (path traversal)",
                segment
            ));
        }
    }
    for ch in path.chars() {
        if ch == '\0' || ch == '\n' || ch == '\r' || (ch as u32) < 0x20 {
            return Err("URI path contains control character".to_string());
        }
    }
    Ok(())
}

/// Format resolved entries as export statements.
pub fn format_as_export(entries: &[ResolvedEntry]) -> String {
    let mut output = String::new();
    for entry in entries {
        if entry.error.is_none() {
            output.push_str(&format!(
                "export {}='{}'\n",
                entry.key,
                escape_single_quote(&entry.value)
            ));
        }
    }
    output
}

/// Format resolved entries as .env format.
pub fn format_as_env(entries: &[ResolvedEntry]) -> String {
    let mut output = String::new();
    for entry in entries {
        if entry.error.is_none() {
            output.push_str(&format!("{}={}\n", entry.key, entry.value));
        }
    }
    output
}

/// Escape single quotes for shell export statements.
fn escape_single_quote(s: &str) -> String {
    s.replace('\'', "'\\''")
}

/// Generate a summary line for the resolve operation.
pub fn format_summary(entries: &[ResolvedEntry]) -> String {
    let total = entries.len();
    let uri_count = entries.iter().filter(|e| e.was_uri).count();
    let resolved_count = entries
        .iter()
        .filter(|e| e.was_uri && e.error.is_none())
        .count();
    let error_count = entries.iter().filter(|e| e.error.is_some()).count();
    let plain_count = total - uri_count;

    if error_count > 0 {
        format!(
            "Resolved {}/{} URIs ({} plain values, {} errors)",
            resolved_count, uri_count, plain_count, error_count
        )
    } else {
        format!(
            "Resolved {}/{} URIs ({} plain values)",
            resolved_count, uri_count, plain_count
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_secret_uri ───────────────────────────────────────

    #[test]
    fn test_parse_vault_uri() {
        let uri = parse_secret_uri("vault://secret/myapp/DB_URL").unwrap();
        assert_eq!(uri.provider, "vault");
        assert_eq!(uri.path, "secret/myapp/DB_URL");
    }

    #[test]
    fn test_parse_aws_ssm_uri() {
        let uri = parse_secret_uri("aws-ssm:///prod/api-key").unwrap();
        assert_eq!(uri.provider, "aws-ssm");
        assert_eq!(uri.path, "prod/api-key");
    }

    #[test]
    fn test_parse_onepassword_uri() {
        let uri = parse_secret_uri("1password://vault/item/field").unwrap();
        assert_eq!(uri.provider, "1password");
        assert_eq!(uri.path, "vault/item/field");
    }

    #[test]
    fn test_parse_doppler_uri() {
        let uri = parse_secret_uri("doppler://project/config/KEY").unwrap();
        assert_eq!(uri.provider, "doppler");
        assert_eq!(uri.path, "project/config/KEY");
    }

    #[test]
    fn test_parse_gcp_uri() {
        let uri = parse_secret_uri("gcp://projects/myproject/secrets/KEY").unwrap();
        assert_eq!(uri.provider, "gcp");
        assert_eq!(uri.path, "projects/myproject/secrets/KEY");
    }

    #[test]
    fn test_parse_azure_uri() {
        let uri = parse_secret_uri("azure://myvault/KEY").unwrap();
        assert_eq!(uri.provider, "azure");
        assert_eq!(uri.path, "myvault/KEY");
    }

    #[test]
    fn test_parse_infisical_uri() {
        let uri = parse_secret_uri("infisical://project/env/KEY").unwrap();
        assert_eq!(uri.provider, "infisical");
        assert_eq!(uri.path, "project/env/KEY");
    }

    // ── parse_secret_uri invalid formats ───────────────────────

    #[test]
    fn test_parse_https_not_secret_uri() {
        assert!(parse_secret_uri("https://example.com/path").is_none());
    }

    #[test]
    fn test_parse_http_not_secret_uri() {
        assert!(parse_secret_uri("http://localhost:8080").is_none());
    }

    #[test]
    fn test_parse_ftp_not_secret_uri() {
        assert!(parse_secret_uri("ftp://server/file").is_none());
    }

    #[test]
    fn test_parse_no_scheme() {
        assert!(parse_secret_uri("just-a-string").is_none());
    }

    #[test]
    fn test_parse_empty_path() {
        assert!(parse_secret_uri("vault://").is_none());
    }

    #[test]
    fn test_parse_unknown_provider() {
        assert!(parse_secret_uri("unknown://path/key").is_none());
    }

    #[test]
    fn test_parse_empty_string() {
        assert!(parse_secret_uri("").is_none());
    }

    // ── is_secret_uri ──────────────────────────────────────────

    #[test]
    fn test_is_secret_uri_true() {
        assert!(is_secret_uri("vault://secret/myapp/DB_URL"));
        assert!(is_secret_uri("aws-ssm:///prod/api-key"));
        assert!(is_secret_uri("1password://vault/item/field"));
    }

    #[test]
    fn test_is_secret_uri_false() {
        assert!(!is_secret_uri("https://example.com"));
        assert!(!is_secret_uri("some_plain_value"));
        assert!(!is_secret_uri("DATABASE_URL=postgres://host/db"));
        assert!(!is_secret_uri(""));
    }

    #[test]
    fn test_is_secret_uri_regular_url_not_detected() {
        assert!(!is_secret_uri("https://api.github.com/repos"));
        assert!(!is_secret_uri("http://localhost:3000"));
        assert!(!is_secret_uri("ftp://files.example.com/data"));
        assert!(!is_secret_uri("ssh://git@github.com/repo.git"));
        assert!(!is_secret_uri("postgres://user:pass@host/db"));
        assert!(!is_secret_uri("redis://localhost:6379"));
        assert!(!is_secret_uri("mongodb://cluster.mongodb.net/mydb"));
    }

    // ── parse_uri_content ──────────────────────────────────────

    #[test]
    fn test_parse_uri_content_mixed() {
        let content = r#"
# Database config
DB_HOST=localhost
DB_PORT=5432
DB_PASSWORD=vault://secret/myapp/DB_PASSWORD
API_KEY=aws-ssm:///prod/api-key

# Empty lines and comments are skipped
PLAIN_VALUE="hello world"
QUOTED_SINGLE='single quoted'
"#;
        let entries = parse_uri_content(content);
        assert_eq!(entries.len(), 6);

        assert_eq!(entries[0], ("DB_HOST".to_string(), "localhost".to_string()));
        assert_eq!(entries[1], ("DB_PORT".to_string(), "5432".to_string()));
        assert_eq!(
            entries[2],
            (
                "DB_PASSWORD".to_string(),
                "vault://secret/myapp/DB_PASSWORD".to_string()
            )
        );
        assert_eq!(
            entries[3],
            ("API_KEY".to_string(), "aws-ssm:///prod/api-key".to_string())
        );
        assert_eq!(
            entries[4],
            ("PLAIN_VALUE".to_string(), "hello world".to_string())
        );
        assert_eq!(
            entries[5],
            ("QUOTED_SINGLE".to_string(), "single quoted".to_string())
        );
    }

    #[test]
    fn test_parse_uri_content_empty() {
        let entries = parse_uri_content("");
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_uri_content_comments_only() {
        let content = "# just comments\n# nothing else\n";
        let entries = parse_uri_content(content);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_uri_content_with_url_values() {
        let content = "CALLBACK_URL=https://example.com/callback\nSECRET=vault://path/key\n";
        let entries = parse_uri_content(content);
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0],
            (
                "CALLBACK_URL".to_string(),
                "https://example.com/callback".to_string()
            )
        );
        assert_eq!(
            entries[1],
            ("SECRET".to_string(), "vault://path/key".to_string())
        );
    }

    // ── format helpers ─────────────────────────────────────────

    #[test]
    fn test_format_as_export() {
        let entries = vec![
            ResolvedEntry {
                key: "A".to_string(),
                value: "hello".to_string(),
                was_uri: false,
                error: None,
            },
            ResolvedEntry {
                key: "B".to_string(),
                value: "world".to_string(),
                was_uri: true,
                error: None,
            },
        ];
        let output = format_as_export(&entries);
        assert_eq!(output, "export A='hello'\nexport B='world'\n");
    }

    #[test]
    fn test_format_as_env() {
        let entries = vec![
            ResolvedEntry {
                key: "A".to_string(),
                value: "hello".to_string(),
                was_uri: false,
                error: None,
            },
            ResolvedEntry {
                key: "B".to_string(),
                value: "world".to_string(),
                was_uri: true,
                error: None,
            },
        ];
        let output = format_as_env(&entries);
        assert_eq!(output, "A=hello\nB=world\n");
    }

    #[test]
    fn test_format_as_export_skips_errors() {
        let entries = vec![
            ResolvedEntry {
                key: "OK".to_string(),
                value: "val".to_string(),
                was_uri: false,
                error: None,
            },
            ResolvedEntry {
                key: "FAIL".to_string(),
                value: "vault://path".to_string(),
                was_uri: true,
                error: Some("provider not configured".to_string()),
            },
        ];
        let output = format_as_export(&entries);
        assert_eq!(output, "export OK='val'\n");
    }

    #[test]
    fn test_format_as_export_escapes_single_quotes() {
        let entries = vec![ResolvedEntry {
            key: "A".to_string(),
            value: "it's a test".to_string(),
            was_uri: false,
            error: None,
        }];
        let output = format_as_export(&entries);
        assert_eq!(output, "export A='it'\\''s a test'\n");
    }

    // ── format_summary ─────────────────────────────────────────

    #[test]
    fn test_format_summary_all_resolved() {
        let entries = vec![
            ResolvedEntry {
                key: "A".to_string(),
                value: "v".to_string(),
                was_uri: true,
                error: None,
            },
            ResolvedEntry {
                key: "B".to_string(),
                value: "v".to_string(),
                was_uri: true,
                error: None,
            },
            ResolvedEntry {
                key: "C".to_string(),
                value: "v".to_string(),
                was_uri: false,
                error: None,
            },
        ];
        let summary = format_summary(&entries);
        assert_eq!(summary, "Resolved 2/2 URIs (1 plain values)");
    }

    #[test]
    fn test_format_summary_with_errors() {
        let entries = vec![
            ResolvedEntry {
                key: "A".to_string(),
                value: "v".to_string(),
                was_uri: true,
                error: None,
            },
            ResolvedEntry {
                key: "B".to_string(),
                value: "v".to_string(),
                was_uri: true,
                error: Some("fail".to_string()),
            },
            ResolvedEntry {
                key: "C".to_string(),
                value: "v".to_string(),
                was_uri: false,
                error: None,
            },
        ];
        let summary = format_summary(&entries);
        assert_eq!(summary, "Resolved 1/2 URIs (1 plain values, 1 errors)");
    }

    // ── escape_single_quote ────────────────────────────────────

    #[test]
    fn test_escape_single_quote_no_quotes() {
        assert_eq!(escape_single_quote("hello"), "hello");
    }

    #[test]
    fn test_escape_single_quote_with_quotes() {
        assert_eq!(escape_single_quote("it's"), "it'\\''s");
    }
}
