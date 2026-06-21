// ─── LSP Security Guards ──────────────────────────────────
//
// Reusable security pipeline for LSP handlers. Each guard is a zero-cost
// check that can be composed at the top of any handler to enforce
// invariants consistently. New handlers get protection by default.
//
// Message-boundary validation (G6): every string crossing the LSP→CLI
// boundary MUST pass these guards before touching filesystem or subprocess
// operations. This is the front door — reject here, don't sanitize later.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Local;
use serde_json::json;

use crate::ops::fence;

pub const MAX_MESSAGE_LENGTH: usize = 1024;
pub const MAX_KEY_LENGTH: usize = 128;

/// Result of a security guard check.
pub type GuardResult = Result<(), String>;

/// Security policy for a workspace-scoped LSP session.
#[derive(Debug, Clone)]
pub struct LspSecurityPolicy {
    /// Maximum `reveal.value` calls allowed in a 60-second window.
    pub max_reveals_per_minute: u32,
    /// Refuse `reveal.value` when the workspace fence is active.
    pub fence_required_for_reveal: bool,
    /// Maximum tracked open documents.
    pub max_tracked_documents: usize,
}

impl Default for LspSecurityPolicy {
    fn default() -> Self {
        Self {
            max_reveals_per_minute: 10,
            fence_required_for_reveal: true,
            max_tracked_documents: 256,
        }
    }
}

pub struct LspAuditLogger {
    log_path: PathBuf,
}

impl LspAuditLogger {
    pub fn new() -> Result<Self, std::io::Error> {
        let config_dir = dirs::config_dir().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "Config directory not found")
        })?;

        let envforge_dir = config_dir.join("envforge");
        std::fs::create_dir_all(&envforge_dir)?;

        let log_path = envforge_dir.join("lsp-audit.log");
        Ok(LspAuditLogger { log_path })
    }

    pub fn log_operation(
        &self,
        operation: &str,
        file_path: &str,
        keys_accessed: &[String],
        status: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let entry = json!({
            "timestamp": Local::now().to_rfc3339(),
            "operation": operation,
            "file_path": file_path,
            "keys_accessed": keys_accessed,
            "status": status,
        });

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;

        writeln!(file, "{}", entry)?;
        Ok(())
    }
}

/// Reject the operation if the workspace fence is active.
/// Used by `reveal.value` and any operation that exposes secret values.
pub fn guard_fence_check(workspace_root: Option<&Path>, operation: &str) -> GuardResult {
    let Some(root) = workspace_root else {
        return Ok(()); // no workspace → skip fence check
    };
    match fence::check_fence_status(root) {
        Ok(status) if status.all_fenced => Err(format!(
            "fence is active; {} blocked. Disable the fence first.",
            operation
        )),
        _ => Ok(()),
    }
}

/// Verify a file path stays within the workspace root. Used by
/// `canary.scan` and any handler that opens files from client input.
pub fn guard_workspace_containment(
    workspace_root: Option<&Path>,
    file_name: &str,
) -> Result<std::path::PathBuf, String> {
    let root =
        workspace_root.ok_or_else(|| "workspace root required for file access".to_string())?;
    let resolved = root.join(file_name);
    let canonical = std::fs::canonicalize(&resolved)
        .map_err(|e| format!("cannot resolve path '{}': {}", file_name, e))?;
    let canonical_root =
        std::fs::canonicalize(root).map_err(|e| format!("cannot resolve workspace root: {}", e))?;
    if !canonical.starts_with(&canonical_root) {
        return Err(format!(
            "file '{}' is outside the workspace root",
            canonical.display()
        ));
    }
    Ok(canonical)
}

/// Verify an **absolute** file path stays within the workspace root.
///
/// Unlike [`guard_workspace_containment`], which joins `workspace_root + file_name`,
/// this variant treats `file_path` as an absolute path (caller-supplied, so it
/// might be `../outside/file` or a symlink to `/etc/passwd`). We canonicalize it
/// and require the result is still under the canonicalized workspace root.
///
/// Used by `canary.plant` (security fix: arbitrary-file-write guard).
pub fn guard_workspace_containment_absolute(
    workspace_root: Option<&Path>,
    file_path: &str,
) -> Result<std::path::PathBuf, String> {
    let root =
        workspace_root.ok_or_else(|| "workspace root required for file access".to_string())?;
    let canonical_root =
        std::fs::canonicalize(root).map_err(|e| format!("cannot resolve workspace root: {}", e))?;
    let path = Path::new(file_path);
    // For absolute paths: canonicalize directly.
    // For relative paths: resolve relative to workspace root.
    let resolved = if path.is_absolute() {
        std::fs::canonicalize(path)
            .map_err(|e| format!("cannot resolve path '{}': {}", file_path, e))?
    } else {
        std::fs::canonicalize(root.join(path))
            .map_err(|e| format!("cannot resolve path '{}': {}", file_path, e))?
    };
    if !resolved.starts_with(&canonical_root) {
        return Err(format!(
            "file '{}' is outside the workspace root",
            resolved.display()
        ));
    }
    Ok(resolved)
}

/// Validate that an env-var key name contains only safe characters.
/// Matches POSIX shell identifier rules: `[A-Za-z_][A-Za-z0-9_]*`.
pub fn guard_key_pattern(key: &str) -> GuardResult {
    use std::sync::OnceLock;
    static KEY_RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = KEY_RE
        .get_or_init(|| regex::Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*$").expect("static regex"));
    if !re.is_match(key) {
        return Err(format!(
            "invalid key '{}': must match [A-Za-z_][A-Za-z0-9_]*",
            key
        ));
    }
    Ok(())
}

/// Validate a string payload crossing the LSP→CLI boundary for shell
/// metacharacters and control characters. Blocks injection before any
/// subprocess spawn or filesystem I/O. Used by every handler that
/// forwards client-provided strings to `Command::new()` or file paths.
pub fn guard_payload_safety(label: &str, payload: &str) -> GuardResult {
    // Length cap — prevents buffer-bloat attacks and limits audit-log
    // verbosity for free-form fields like `reveal.reason`.
    if payload.len() > MAX_MESSAGE_LENGTH {
        return Err(format!(
            "{} exceeds max length ({} > {})",
            label,
            payload.len(),
            MAX_MESSAGE_LENGTH
        ));
    }

    // Shell metacharacter filter — same regex used by the CLI-side
    // volatile.run guard. Reject, don't sanitize; sanitizing invites
    // bypass through encoding tricks.
    use std::sync::OnceLock;
    static META_RE: OnceLock<regex::Regex> = OnceLock::new();
    let meta_re = META_RE
        .get_or_init(|| regex::Regex::new(r"[];&|`$(){}\[<>!#~*?\n\r]").expect("static regex"));
    if meta_re.is_match(payload) {
        return Err(format!("{} contains forbidden shell metacharacters", label));
    }

    // Control-character check — catches null bytes, backspaces, and
    // other non-printable characters that could be smuggled past the
    // main regex through encoding tricks.
    if payload.chars().any(|c| c.is_control() && c != '\t') {
        return Err(format!("{} contains control characters", label));
    }

    Ok(())
}

/// Validate message/payload length only (for fields passed as CLI args
/// where the shell metacharacter filter already runs in the subprocess
/// path). This is a lighter check for fields that don't need the full
/// `guard_payload_safety` treatment because the downstream code already
/// handles injection.
pub fn guard_message_length(label: &str, msg: &str) -> GuardResult {
    if msg.len() > MAX_MESSAGE_LENGTH {
        return Err(format!(
            "{} exceeds max length ({} > {})",
            label,
            msg.len(),
            MAX_MESSAGE_LENGTH
        ));
    }
    Ok(())
}

/// Validate a key name against the safe pattern AND length limits.
/// Combines `guard_key_pattern` with a max length of `MAX_KEY_LENGTH`.
/// Use for all key-name inputs from LSP clients.
pub fn guard_key_pattern_with_length(key: &str) -> GuardResult {
    guard_key_pattern(key)?;
    if key.len() > MAX_KEY_LENGTH {
        return Err(format!(
            "key '{}' exceeds max length ({} > {})",
            key,
            key.len(),
            MAX_KEY_LENGTH
        ));
    }
    Ok(())
}

/// Restrict file extensions for scanning operations. Returns Err if the
/// extension is not in the allowlist.
pub fn guard_scan_extension(path: &Path) -> GuardResult {
    match path.extension().and_then(|e| e.to_str()) {
        Some("log") | Some("txt") | Some("md") | Some("json") | Some("jsonl") | Some("yml")
        | Some("yaml") | Some("csv") | Some("toml") | Some("properties") => Ok(()),
        Some(other) => Err(format!(
            "file extension '{}' is not allowed for scanning",
            other
        )),
        None => Err("file has no extension; cannot verify safety".to_string()),
    }
}

/// Cap a file size for reading operations. Returns the metadata if
/// file size is within limits.
pub fn guard_file_size(path: &Path, max_bytes: u64) -> GuardResult {
    let meta =
        std::fs::metadata(path).map_err(|e| format!("cannot stat '{}': {}", path.display(), e))?;
    if meta.len() > max_bytes {
        return Err(format!(
            "file too large ({} bytes, max {})",
            meta.len(),
            max_bytes
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_log_creation() {
        let logger = LspAuditLogger::new().unwrap();
        logger
            .log_operation("hover", ".env", &["DATABASE_URL".to_string()], "success")
            .unwrap();

        // Verify log file exists
        assert!(logger.log_path.exists());
    }

    #[test]
    fn test_guard_key_pattern_valid() {
        assert!(guard_key_pattern("API_KEY").is_ok());
        assert!(guard_key_pattern("DB_HOST").is_ok());
        assert!(guard_key_pattern("A").is_ok());
        assert!(guard_key_pattern("_private").is_ok());
    }

    #[test]
    fn test_guard_key_pattern_invalid() {
        assert!(guard_key_pattern("../../../etc/passwd").is_err());
        assert!(guard_key_pattern("FOO=bar").is_err());
        assert!(guard_key_pattern("hello world").is_err());
        assert!(guard_key_pattern("123abc").is_err());
    }

    #[test]
    fn test_guard_fence_check_no_workspace() {
        assert!(guard_fence_check(None, "test").is_ok());
    }

    #[test]
    fn test_guard_fence_check_clean_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(guard_fence_check(Some(tmp.path()), "reveal").is_ok());
    }

    #[test]
    fn test_guard_fence_check_active() {
        let tmp = tempfile::TempDir::new().unwrap();
        crate::ops::fence::create_fence(tmp.path(), false).unwrap();
        let result = guard_fence_check(Some(tmp.path()), "reveal");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("fence is active"));
    }

    #[test]
    fn test_guard_workspace_containment_rejects_traversal() {
        let tmp = tempfile::TempDir::new().unwrap();
        let result = guard_workspace_containment(Some(tmp.path()), "../../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_guard_workspace_containment_allows_safe_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let safe = tmp.path().join("data.log");
        std::fs::write(&safe, "test").unwrap();
        let result = guard_workspace_containment(Some(tmp.path()), "data.log");
        assert!(result.is_ok());
    }

    #[test]
    fn test_guard_scan_extension_allows_log() {
        assert!(guard_scan_extension(Path::new("test.log")).is_ok());
        assert!(guard_scan_extension(Path::new("test.json")).is_ok());
        assert!(guard_scan_extension(Path::new("test.toml")).is_ok());
    }

    #[test]
    fn test_guard_scan_extension_rejects_bin() {
        assert!(guard_scan_extension(Path::new("test.bin")).is_err());
        assert!(guard_scan_extension(Path::new("test.exe")).is_err());
    }

    #[test]
    fn test_guard_scan_extension_rejects_no_extension() {
        assert!(guard_scan_extension(Path::new("noext")).is_err());
    }

    #[test]
    fn test_guard_file_size_ok() {
        let tmp = tempfile::TempDir::new().unwrap();
        let f = tmp.path().join("small.txt");
        std::fs::write(&f, "hello").unwrap();
        assert!(guard_file_size(&f, 1024).is_ok());
    }

    #[test]
    fn test_guard_file_size_too_large() {
        let tmp = tempfile::TempDir::new().unwrap();
        let f = tmp.path().join("big.txt");
        std::fs::write(&f, "too big").unwrap();
        assert!(guard_file_size(&f, 5).is_err());
    }

    #[test]
    fn test_security_policy_defaults() {
        let policy = LspSecurityPolicy::default();
        assert_eq!(policy.max_reveals_per_minute, 10);
        assert!(policy.fence_required_for_reveal);
        assert_eq!(policy.max_tracked_documents, 256);
    }

    #[test]
    fn test_guard_payload_safety_rejects_shell_metacharacters() {
        assert!(guard_payload_safety("test", "safe-value").is_ok());
        assert!(guard_payload_safety("test", "FOO=bar").is_ok());
        assert!(guard_payload_safety("test", "hello world").is_ok());
        assert!(guard_payload_safety("test", "$(whoami)").is_err());
        assert!(guard_payload_safety("test", "`whoami`").is_err());
        assert!(guard_payload_safety("test", "a|b").is_err());
        assert!(guard_payload_safety("test", "a;b").is_err());
        assert!(guard_payload_safety("test", "a&b").is_err());
        assert!(guard_payload_safety("test", "a>b").is_err());
        assert!(guard_payload_safety("test", "ls *").is_err());
        assert!(guard_payload_safety("test", "echo #{var}").is_err());
    }

    #[test]
    fn test_guard_payload_safety_rejects_control_characters() {
        assert!(guard_payload_safety("test", "\x00").is_err());
        assert!(guard_payload_safety("test", "\x08").is_err());
        assert!(guard_payload_safety("test", "\x1b").is_err());
        assert!(guard_payload_safety("test", "hello\tworld").is_ok());
    }

    #[test]
    fn test_guard_payload_safety_rejects_overlength() {
        let long = "a".repeat(MAX_MESSAGE_LENGTH + 1);
        assert!(guard_payload_safety("test", &long).is_err());
    }

    #[test]
    fn test_guard_key_pattern_with_length_valid() {
        assert!(guard_key_pattern_with_length("API_KEY").is_ok());
        assert!(guard_key_pattern_with_length("DB_HOST").is_ok());
    }

    #[test]
    fn test_guard_key_pattern_with_length_overlength() {
        let long = "A".repeat(MAX_KEY_LENGTH + 1);
        assert!(guard_key_pattern_with_length(&long).is_err());
    }

    #[test]
    fn test_guard_message_length_valid() {
        assert!(guard_message_length("reason", "fixing config").is_ok());
    }

    #[test]
    fn test_guard_message_length_overlength() {
        let long = "x".repeat(MAX_MESSAGE_LENGTH + 1);
        assert!(guard_message_length("reason", &long).is_err());
    }
}
