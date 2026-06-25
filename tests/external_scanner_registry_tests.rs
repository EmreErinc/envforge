//! Coverage for `ops::external_scanner::ScannerRegistry` — enabled-filtered
//! lookup and iteration over configured external secret scanners.

use envforge::ops::external_scanner::{ExternalScannerConfig, ScannerRegistry};

fn config(command: &str, enabled: bool) -> ExternalScannerConfig {
    ExternalScannerConfig {
        command: command.to_string(),
        enabled,
        ..Default::default()
    }
}

#[test]
fn test_registry_default_is_empty() {
    let reg = ScannerRegistry::default();
    assert!(reg.is_empty());
    assert_eq!(reg.len(), 0);
}

#[test]
fn test_registry_get_filters_disabled() {
    let mut reg = ScannerRegistry::default();
    reg.scanners
        .insert("trufflehog".to_string(), config("trufflehog", true));
    reg.scanners
        .insert("gitleaks".to_string(), config("gitleaks", false));

    assert!(reg.get("trufflehog").is_some());
    assert!(
        reg.get("gitleaks").is_none(),
        "disabled scanner is not returned"
    );
    assert!(reg.get("missing").is_none());
}

#[test]
fn test_registry_enabled_iterates_only_enabled() {
    let mut reg = ScannerRegistry::default();
    reg.scanners.insert("a".to_string(), config("a", true));
    reg.scanners.insert("b".to_string(), config("b", false));
    reg.scanners.insert("c".to_string(), config("c", true));

    let enabled: Vec<&String> = reg.enabled().map(|(name, _)| name).collect();
    assert_eq!(enabled.len(), 2);
    assert!(enabled.contains(&&"a".to_string()));
    assert!(enabled.contains(&&"c".to_string()));
    assert!(!enabled.contains(&&"b".to_string()));

    // len counts all registered scanners, enabled or not.
    assert_eq!(reg.len(), 3);
    assert!(!reg.is_empty());
}

#[test]
fn test_external_scanner_config_defaults() {
    let c = ExternalScannerConfig::default();
    assert!(c.command.is_empty());
    assert!(c.args.is_empty());
    assert!(c.enabled);
    assert_eq!(c.timeout_ms, 5000);
}
