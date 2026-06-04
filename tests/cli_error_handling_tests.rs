// ═══════════════════════════════════════════════════════════════
// CLI Error Handling Tests
// ═══════════════════════════════════════════════════════════════
// Tests for `src/cli/error.rs` — CLI error types, conversions,
// display formatting, and error message quality.

use envforge::cli::CliError;
use envforge::config::ConfigError;
use envforge::model::ParseError;
use envforge::ops::OpsError;

// ═══════════════════════════════════════════════════════════════
// CliError Constructors
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_cli_error_factory_methods() {
    let e = CliError::invalid_input("bad key name");
    assert!(matches!(e, CliError::InvalidInput(_)));
    assert_eq!(e.to_string(), "bad key name");

    let e = CliError::not_found("profile 'prod' not found");
    assert!(matches!(e, CliError::NotFound(_)));
    assert!(e.to_string().contains("profile 'prod'"));

    let e = CliError::git("not a git repository");
    assert!(matches!(e, CliError::Git(_)));
    assert!(e.to_string().contains("not a git repository"));

    let e = CliError::sync("conflict detected");
    assert!(matches!(e, CliError::Sync(_)));
    assert!(e.to_string().contains("conflict detected"));

    let e = CliError::secret("decryption failed");
    assert!(matches!(e, CliError::Secret(_)));
    assert!(e.to_string().contains("decryption failed"));

    let e = CliError::schema("missing required field");
    assert!(matches!(e, CliError::Schema(_)));
    assert!(e.to_string().contains("missing required field"));

    let e = CliError::protocol("LSP shutdown timeout");
    assert!(matches!(e, CliError::Protocol(_)));
    assert!(e.to_string().contains("LSP shutdown timeout"));

    let e = CliError::other("something went wrong");
    assert!(matches!(e, CliError::Other(_)));
    assert_eq!(e.to_string(), "something went wrong");
}

// ═══════════════════════════════════════════════════════════════
// CliError Display Formatting
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_cli_error_display_has_prefix_for_typed_errors() {
    // Typed errors should include a category prefix
    let err = CliError::git("rebase in progress");
    assert!(err.to_string().starts_with("git error: "));

    let err = CliError::sync("merge conflict");
    assert!(err.to_string().starts_with("sync error: "));

    let err = CliError::secret("key not found");
    assert!(err.to_string().starts_with("secret error: "));

    let err = CliError::schema("type mismatch");
    assert!(err.to_string().starts_with("schema error: "));

    let err = CliError::protocol("handshake failed");
    assert!(err.to_string().starts_with("protocol error: "));
}

#[test]
fn test_cli_error_invalid_input_no_prefix() {
    // InvalidInput and NotFound should NOT have a prefix — they ARE the message
    let err = CliError::invalid_input("must be alphanumeric");
    assert_eq!(err.to_string(), "must be alphanumeric");

    let err = CliError::not_found("file not found");
    assert_eq!(err.to_string(), "file not found");
}

// ═══════════════════════════════════════════════════════════════
// CliError From Conversions
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_cli_error_from_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "no such file");
    let cli_err: CliError = io_err.into();
    assert!(matches!(cli_err, CliError::Io(_)));
    assert!(cli_err.to_string().contains("no such file"));
}

#[test]
fn test_cli_error_from_config_error() {
    let config_err = ConfigError::HomeDirNotFound;
    let cli_err: CliError = config_err.into();
    assert!(matches!(cli_err, CliError::Config(_)));
    assert!(cli_err.to_string().contains("home directory"));
}

#[test]
fn test_cli_error_from_parse_error() {
    let parse_err = ParseError::ShellNotDetected;
    let cli_err: CliError = parse_err.into();
    assert!(matches!(cli_err, CliError::Parse(_)));
    assert!(cli_err.to_string().contains("shell"));
}

#[test]
fn test_cli_error_from_op_error() {
    let op_err = OpsError::Other("test operation failed".to_string());
    let cli_err: CliError = op_err.into();
    assert!(matches!(cli_err, CliError::Ops(_)));
    assert!(cli_err.to_string().contains("test operation failed"));
}

// ═══════════════════════════════════════════════════════════════
// Error Message Quality
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_cli_error_messages_are_not_empty() {
    let errors: Vec<CliError> = vec![
        CliError::invalid_input("bad input"),
        CliError::not_found("resource"),
        CliError::git("error"),
        CliError::sync("error"),
        CliError::secret("error"),
        CliError::schema("error"),
        CliError::protocol("error"),
        CliError::other("error"),
    ];
    for err in &errors {
        assert!(
            !err.to_string().is_empty(),
            "error variant returned empty message"
        );
    }
}

#[test]
fn test_cli_error_debug_format_contains_variant_name() {
    let err = CliError::invalid_input("test");
    let debug = format!("{:?}", err);
    assert!(debug.contains("InvalidInput"));
}

// ═══════════════════════════════════════════════════════════════
// Error Chaining via From Conversions
// ═══════════════════════════════════════════════════════════════

#[test]
fn test_cli_error_preserves_nested_error_message() {
    // Config error → Cli error should preserve the inner message
    let config_err = ConfigError::IoError {
        path: std::path::PathBuf::from("/tmp/test.toml"),
        source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
    };
    let cli_err: CliError = config_err.into();
    let msg = cli_err.to_string();
    assert!(
        msg.contains("permission denied")
            || msg.contains("PermissionDenied")
            || msg.contains("denied"),
        "expected IO error detail in message: {msg}"
    );
}

#[test]
fn test_cli_error_send_sync_bounds() {
    // Verify CliError implements Send + Sync (required for tokio, etc.)
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CliError>();
}
