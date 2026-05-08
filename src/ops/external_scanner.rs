// ─── External Scanner Interface ───────────────────────────
//
// First-class configuration and execution pipeline for external
// security scanners. Replaces ENVFORGE_EXTERNAL_SCANNER env var.

use std::collections::BTreeMap;
use std::time::Duration;

// ─── Config ────────────────────────────────────────────────

/// Configuration for a single external scanner.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExternalScannerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_timeout_ms() -> u64 {
    5000
}

fn default_enabled() -> bool {
    true
}

impl Default for ExternalScannerConfig {
    fn default() -> Self {
        Self {
            command: String::new(),
            args: Vec::new(),
            timeout_ms: 5000,
            enabled: true,
        }
    }
}

// ─── Scanner Registry ──────────────────────────────────────

/// Registry of named external scanners.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ScannerRegistry {
    #[serde(flatten)]
    pub scanners: BTreeMap<String, ExternalScannerConfig>,
}

impl ScannerRegistry {
    /// Get an enabled scanner by name.
    pub fn get(&self, name: &str) -> Option<&ExternalScannerConfig> {
        self.scanners.get(name).filter(|s| s.enabled)
    }

    /// Iterate over all enabled scanners.
    pub fn enabled(&self) -> impl Iterator<Item = (&String, &ExternalScannerConfig)> {
        self.scanners.iter().filter(|(_, s)| s.enabled)
    }

    /// Total number of scanners.
    pub fn len(&self) -> usize {
        self.scanners.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scanners.is_empty()
    }
}

// ─── Execution ─────────────────────────────────────────────

/// Result of running a single scanner.
#[derive(Debug, Clone)]
pub struct ScannerFinding {
    pub scanner_name: String,
    pub findings: Vec<String>,
}

/// Run all enabled scanners concurrently on the given content.
///
/// Each scanner receives content via stdin. If a scanner exits non-zero,
/// its stdout+stderr lines are collected as findings.
pub async fn run_scanners(registry: &ScannerRegistry, content: &str) -> Vec<ScannerFinding> {
    let mut handles = Vec::new();

    for (name, config) in registry.enabled() {
        let name_clone = name.clone();
        let config = config.clone();
        let content = content.to_string();

        let handle =
            tokio::spawn(async move { run_single_scanner(&name_clone, &config, &content).await });
        handles.push((name.clone(), handle));
    }

    // Also check legacy ENVFORGE_EXTERNAL_SCANNER env var
    if let Ok(legacy_cmd) = std::env::var("ENVFORGE_EXTERNAL_SCANNER") {
        if !legacy_cmd.is_empty() && registry.is_empty() {
            static WARNED: std::sync::atomic::AtomicBool =
                std::sync::atomic::AtomicBool::new(false);
            if !WARNED.swap(true, std::sync::atomic::Ordering::SeqCst) {
                eprintln!(
                    "\u{26a0} EnvForge: ENVFORGE_EXTERNAL_SCANNER is deprecated. \
                     Use [scanners] in .envforge.project.toml."
                );
            }

            let content = content.to_string();
            let handle = tokio::spawn(async move {
                // Reject shell-style values to prevent argv smuggling
                // (e.g. `/bin/sh -c "evil"` re-interpreted by split_whitespace).
                // Only accept an absolute path optionally followed by simple
                // whitespace-separated args containing none of: quotes, $, `, ;, |, &, \, <, >, newline.
                let trimmed = legacy_cmd.trim();
                if trimmed.is_empty()
                    || !trimmed.starts_with('/')
                    || trimmed.contains([
                        '\'', '"', '$', '`', ';', '|', '&', '\\', '<', '>', '\n', '\r',
                    ])
                {
                    eprintln!(
                        "envforge: ENVFORGE_EXTERNAL_SCANNER must be an absolute path with simple, \
                         whitespace-separated args (no shell metacharacters); ignoring."
                    );
                    return None;
                }

                let mut parts = trimmed.split_whitespace();
                let program = match parts.next() {
                    Some(p) => p,
                    None => return None,
                };
                let args: Vec<&str> = parts.collect();
                let mut cmd = tokio::process::Command::new(program);
                cmd.args(&args);
                run_scanner_cmd("legacy", cmd, &content, 5000).await
            });
            handles.push(("legacy".to_string(), handle));
        }
    }

    let mut findings = Vec::new();
    for (_name, handle) in handles {
        match handle.await {
            Ok(Some(result)) if !result.findings.is_empty() => {
                findings.push(result);
            }
            _ => {}
        }
    }

    findings
}

pub async fn run_single_scanner(
    name: &str,
    config: &ExternalScannerConfig,
    content: &str,
) -> Option<ScannerFinding> {
    let mut cmd = tokio::process::Command::new(&config.command);
    cmd.args(&config.args);

    run_scanner_cmd(name, cmd, content, config.timeout_ms).await
}

async fn run_scanner_cmd(
    name: &str,
    mut cmd: tokio::process::Command,
    content: &str,
    timeout_ms: u64,
) -> Option<ScannerFinding> {
    use tokio::io::AsyncWriteExt;

    let timeout = Duration::from_millis(timeout_ms);

    let result = tokio::time::timeout(timeout, async {
        let mut child = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .ok()?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(content.as_bytes()).await;
            let _ = stdin.shutdown().await;
        }

        let output = child.wait_with_output().await.ok()?;
        Some(output)
    })
    .await;

    match result {
        Ok(Some(output)) if !output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let lines: Vec<String> = format!("{}{}", stdout, stderr)
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.to_string())
                .collect();

            if !lines.is_empty() {
                Some(ScannerFinding {
                    scanner_name: name.to_string(),
                    findings: lines,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

// ─── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scanner_registry() {
        let mut registry = ScannerRegistry::default();
        registry.scanners.insert(
            "test".to_string(),
            ExternalScannerConfig {
                command: "echo".to_string(),
                args: vec!["hello".to_string()],
                timeout_ms: 1000,
                enabled: true,
            },
        );

        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());

        let enabled: Vec<_> = registry.enabled().collect();
        assert_eq!(enabled.len(), 1);
        assert_eq!(enabled[0].0, "test");
    }

    #[test]
    fn test_scanner_registry_disabled_not_returned() {
        let mut registry = ScannerRegistry::default();
        registry.scanners.insert(
            "disabled".to_string(),
            ExternalScannerConfig {
                command: "echo".to_string(),
                args: vec![],
                timeout_ms: 1000,
                enabled: false,
            },
        );

        assert!(registry.enabled().next().is_none());
    }

    #[test]
    fn test_external_scanner_config_defaults() {
        let config = ExternalScannerConfig::default();
        assert_eq!(config.timeout_ms, 5000);
        assert!(config.enabled);
        assert!(config.args.is_empty());
    }

    #[tokio::test]
    async fn test_run_single_scanner_success_returns_none() {
        // A command that exits 0 means "no findings" -> None
        let config = ExternalScannerConfig {
            command: "true".to_string(),
            args: vec![],
            timeout_ms: 1000,
            enabled: true,
        };
        let result = run_single_scanner("test", &config, "content").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_run_single_scanner_failure_returns_findings() {
        // A command that exits 1 with output means findings
        let config = ExternalScannerConfig {
            command: "sh".to_string(),
            args: vec![
                "-c".to_string(),
                "echo 'found secret' && exit 1".to_string(),
            ],
            timeout_ms: 1000,
            enabled: true,
        };
        let result = run_single_scanner("test", &config, "content").await;
        assert!(result.is_some());
        let finding = result.unwrap();
        assert_eq!(finding.scanner_name, "test");
        assert!(!finding.findings.is_empty());
        assert!(finding.findings[0].contains("found secret"));
    }

    #[tokio::test]
    async fn test_run_scanners_empty_registry() {
        let registry = ScannerRegistry::default();
        let findings = run_scanners(&registry, "content").await;
        assert!(findings.is_empty());
    }

    #[tokio::test]
    async fn test_run_scanners_timeout() {
        let mut registry = ScannerRegistry::default();
        registry.scanners.insert(
            "slow".to_string(),
            ExternalScannerConfig {
                command: "sleep".to_string(),
                args: vec!["10".to_string()],
                timeout_ms: 50, // very short timeout
                enabled: true,
            },
        );

        let findings = run_scanners(&registry, "content").await;
        assert!(findings.is_empty()); // timeout -> no findings
    }
}
