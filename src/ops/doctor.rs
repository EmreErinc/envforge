use crate::config::{load_or_create_default, AppConfig};
use crate::model::ShellFile;
use crate::ops::duplicates::detect_duplicates;
use crate::ops::encrypt::{age_key_path, is_encrypted};
use crate::ops::listing::EnvEntry;
use crate::ops::secrets::cache::is_reference;
use crate::ops::secrets::credentials::list_configured_providers;
use crate::ops::secrets::providers::create_default_registry;
use crate::ops::sync::{is_initialized as sync_is_initialized, sync_dir};
use crate::ops::validation::validate_entries;
use crate::parser::parse_shell_file;

// ─── Types ───────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum CheckStatus {
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct HealthCheck {
    pub name: String,
    pub status: CheckStatus,
    pub message: String,
    pub details: Vec<String>,
    /// Actionable hint for the user when status is Warning or Error.
    pub hint: Option<String>,
}

#[derive(Debug)]
pub struct HealthReport {
    pub checks: Vec<HealthCheck>,
}

impl HealthReport {
    pub fn ok_count(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.status == CheckStatus::Ok)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.status == CheckStatus::Warning)
            .count()
    }

    pub fn error_count(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.status == CheckStatus::Error)
            .count()
    }
}

// ─── Main Doctor Function ────────────────────────────────────

pub fn run_doctor() -> HealthReport {
    let mut checks = Vec::new();

    // 1. Config check
    let config_result = check_config();
    let config_ok = config_result.status == CheckStatus::Ok;
    checks.push(config_result);

    // Only proceed with deeper checks if config loaded
    if config_ok {
        let config = load_or_create_default().unwrap();
        let shell_files = load_shell_files(&config);

        // 2. Encryption key
        checks.push(check_encryption_key());

        // 3. Shell files
        checks.push(check_shell_files(&config, &shell_files));

        // 4. Duplicates
        checks.push(check_duplicates(&shell_files));

        // 5. Validation
        let entries = collect_entries(&shell_files);
        checks.push(check_validation(&config, &entries));

        // 6. References
        checks.push(check_references(&entries));

        // 7. Sync status
        checks.push(check_sync());

        // 8. Provider binaries
        checks.push(check_provider_binaries());

        // 9. Provider credentials
        checks.push(check_provider_credentials());
    }

    HealthReport { checks }
}

// ─── Individual Checks ───────────────────────────────────────

fn check_config() -> HealthCheck {
    match load_or_create_default() {
        Ok(_) => HealthCheck {
            name: "Config".into(),
            status: CheckStatus::Ok,
            message: "loaded OK".into(),
            details: vec![],
            hint: None,
        },
        Err(e) => HealthCheck {
            name: "Config".into(),
            status: CheckStatus::Error,
            message: format!("failed to load: {}", e),
            details: vec![],
            hint: Some(
                "Delete ~/.config/envforge/config.toml and re-run envforge to regenerate".into(),
            ),
        },
    }
}

fn check_encryption_key() -> HealthCheck {
    match age_key_path() {
        Ok(path) => {
            if path.exists() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(meta) = std::fs::metadata(&path) {
                        let mode = meta.permissions().mode() & 0o777;
                        if mode != 0o600 {
                            return HealthCheck {
                                name: "Encryption key".into(),
                                status: CheckStatus::Warning,
                                message: format!(
                                    "age key permissions are {:o}, expected 600",
                                    mode
                                ),
                                details: vec![format!("path: {}", path.display())],
                                hint: Some(format!("Run: chmod 600 {}", path.display())),
                            };
                        }
                    }
                }
                HealthCheck {
                    name: "Encryption key".into(),
                    status: CheckStatus::Ok,
                    message: "age key found".into(),
                    details: vec![format!("path: {}", path.display())],
                    hint: None,
                }
            } else {
                HealthCheck {
                    name: "Encryption key".into(),
                    status: CheckStatus::Warning,
                    message: "no age key yet".into(),
                    details: vec![],
                    hint: Some(
                        "Run: envforge encrypt <KEY> to generate a key and encrypt a value".into(),
                    ),
                }
            }
        }
        Err(e) => HealthCheck {
            name: "Encryption key".into(),
            status: CheckStatus::Error,
            message: format!("cannot resolve key path: {}", e),
            details: vec![],
            hint: Some("Ensure $HOME is set and ~/.config/envforge/ is writable".into()),
        },
    }
}

fn check_shell_files(config: &AppConfig, shell_files: &[ShellFile]) -> HealthCheck {
    let primary = shellexpand(&config.files.primary);
    let mut details = Vec::new();

    if !primary.exists() {
        return HealthCheck {
            name: "Shell files".into(),
            status: CheckStatus::Warning,
            message: format!("primary file not found: {}", primary.display()),
            details: vec![],
            hint: Some("Check 'files.primary' in ~/.config/envforge/config.toml".into()),
        };
    }

    details.push(format!("primary: {}", primary.display()));

    if config.files.use_reference_file {
        let ref_path = shellexpand(&config.files.reference);
        if ref_path.exists() {
            details.push(format!("reference: {}", ref_path.display()));
        } else {
            details.push("reference: not yet created".into());
        }
    }

    let total_entries: usize = shell_files
        .iter()
        .map(|f| crate::ops::collect_all_entries(std::slice::from_ref(f)).len())
        .sum();

    HealthCheck {
        name: "Shell files".into(),
        status: CheckStatus::Ok,
        message: format!(
            "{} file(s) parsed, {} entries",
            shell_files.len(),
            total_entries
        ),
        details,
        hint: None,
    }
}

fn check_duplicates(shell_files: &[ShellFile]) -> HealthCheck {
    let groups = detect_duplicates(shell_files);
    if groups.is_empty() {
        HealthCheck {
            name: "Duplicates".into(),
            status: CheckStatus::Ok,
            message: "no duplicate keys".into(),
            details: vec![],
            hint: None,
        }
    } else {
        let keys: Vec<String> = groups.iter().map(|g| g.key.clone()).collect();
        HealthCheck {
            name: "Duplicates".into(),
            status: CheckStatus::Warning,
            message: format!("{} duplicate key(s) found", groups.len()),
            details: keys,
            hint: Some("Run: envforge duplicates to see details and resolve them".into()),
        }
    }
}

fn check_validation(config: &AppConfig, entries: &[EnvEntry]) -> HealthCheck {
    if config.validation.is_empty() {
        return HealthCheck {
            name: "Validation".into(),
            status: CheckStatus::Ok,
            message: "no rules configured".into(),
            details: vec![],
            hint: None,
        };
    }

    let errors = validate_entries(entries, &config.validation);
    if errors.is_empty() {
        HealthCheck {
            name: "Validation".into(),
            status: CheckStatus::Ok,
            message: format!("all values valid ({} rules)", config.validation.len()),
            details: vec![],
            hint: None,
        }
    } else {
        let details: Vec<String> = errors
            .iter()
            .map(|e| format!("{}: {}", e.key, e.message))
            .collect();
        HealthCheck {
            name: "Validation".into(),
            status: CheckStatus::Warning,
            message: format!("{} validation error(s)", errors.len()),
            details,
            hint: Some(
                "Run: envforge validate to see full details, then fix with: envforge set KEY=VALUE"
                    .into(),
            ),
        }
    }
}

fn check_references(entries: &[EnvEntry]) -> HealthCheck {
    let refs: Vec<&EnvEntry> = entries.iter().filter(|e| is_reference(&e.value)).collect();
    let encrypted: Vec<&EnvEntry> = entries.iter().filter(|e| is_encrypted(&e.value)).collect();

    let mut details = Vec::new();
    if !refs.is_empty() {
        details.push(format!(
            "references: {}",
            refs.iter()
                .map(|e| e.key.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !encrypted.is_empty() {
        details.push(format!("{} encrypted value(s)", encrypted.len()));
    }

    let message = format!("{} reference(s), {} encrypted", refs.len(), encrypted.len());

    HealthCheck {
        name: "References".into(),
        status: CheckStatus::Ok,
        message,
        details,
        hint: None,
    }
}

fn check_sync() -> HealthCheck {
    match sync_dir() {
        Ok(sync_path) => {
            if !sync_is_initialized(&sync_path) {
                return HealthCheck {
                    name: "Sync".into(),
                    status: CheckStatus::Ok,
                    message: "not initialized".into(),
                    details: vec![],
                    hint: None,
                };
            }

            let git_dir = sync_path.join(".git");
            if !git_dir.exists() {
                return HealthCheck {
                    name: "Sync".into(),
                    status: CheckStatus::Warning,
                    message: "sync dir exists but no .git found".into(),
                    details: vec![format!("path: {}", sync_path.display())],
                    hint: Some("Run: envforge sync init --force to reinitialize".into()),
                };
            }

            let output = std::process::Command::new("git")
                .args(["status", "--porcelain"])
                .current_dir(&sync_path)
                .output();

            match output {
                Ok(out) if out.status.success() => {
                    let status_text = String::from_utf8_lossy(&out.stdout);
                    let changed_count = status_text.lines().filter(|l| !l.is_empty()).count();
                    if changed_count == 0 {
                        HealthCheck {
                            name: "Sync".into(),
                            status: CheckStatus::Ok,
                            message: "in sync, no local changes".into(),
                            details: vec![format!("path: {}", sync_path.display())],
                            hint: None,
                        }
                    } else {
                        HealthCheck {
                            name: "Sync".into(),
                            status: CheckStatus::Warning,
                            message: format!("{} local change(s) not pushed", changed_count),
                            details: vec![format!("path: {}", sync_path.display())],
                            hint: Some("Run: envforge sync push to push local changes".into()),
                        }
                    }
                }
                _ => HealthCheck {
                    name: "Sync".into(),
                    status: CheckStatus::Warning,
                    message: "cannot read sync status".into(),
                    details: vec![],
                    hint: Some("Ensure git is installed and sync directory is accessible".into()),
                },
            }
        }
        Err(_) => HealthCheck {
            name: "Sync".into(),
            status: CheckStatus::Ok,
            message: "not initialized".into(),
            details: vec![],
            hint: None,
        },
    }
}

fn check_provider_binaries() -> HealthCheck {
    let registry = create_default_registry();
    let statuses = registry.list_with_status();
    let found: Vec<&str> = statuses
        .iter()
        .filter(|s| s.binary_found)
        .map(|s| s.name.as_str())
        .collect();
    let total = statuses.len();

    HealthCheck {
        name: "Providers".into(),
        status: CheckStatus::Ok,
        message: if found.is_empty() {
            format!("0/{} binaries found", total)
        } else {
            format!(
                "{} ({}/{} binaries found)",
                found.join(", "),
                found.len(),
                total
            )
        },
        details: vec![],
        hint: if found.is_empty() {
            Some("Run: envforge secrets providers to see install instructions".into())
        } else {
            None
        },
    }
}

fn check_provider_credentials() -> HealthCheck {
    match list_configured_providers() {
        Ok(providers) => {
            if providers.is_empty() {
                return HealthCheck {
                    name: "Credentials".into(),
                    status: CheckStatus::Ok,
                    message: "no providers configured".into(),
                    details: vec![],
                    hint: None,
                };
            }

            let registry = create_default_registry();
            let mut ok_providers = Vec::new();
            let mut problem_providers = Vec::new();
            let mut problem_names = Vec::new();

            for name in &providers {
                if let Ok(provider) = registry.get(name) {
                    match crate::ops::secrets::credentials::read_all_credentials(name) {
                        Ok(creds) => match provider.validate_config(&creds) {
                            Ok(_) => ok_providers.push(name.clone()),
                            Err(e) => {
                                problem_providers.push(format!("{}: {}", name, e));
                                problem_names.push(name.clone());
                            }
                        },
                        Err(e) => {
                            problem_providers.push(format!("{}: {}", name, e));
                            problem_names.push(name.clone());
                        }
                    }
                }
            }

            if problem_providers.is_empty() {
                HealthCheck {
                    name: "Credentials".into(),
                    status: CheckStatus::Ok,
                    message: format!("{} provider(s) configured OK", ok_providers.len()),
                    details: ok_providers,
                    hint: None,
                }
            } else {
                let hint_cmds: Vec<String> = problem_names
                    .iter()
                    .map(|n| format!("envforge secrets config {}", n))
                    .collect();
                HealthCheck {
                    name: "Credentials".into(),
                    status: CheckStatus::Warning,
                    message: format!(
                        "{} OK, {} with issues",
                        ok_providers.len(),
                        problem_providers.len()
                    ),
                    details: problem_providers,
                    hint: Some(format!(
                        "Run: {} to see required fields",
                        hint_cmds.join(", ")
                    )),
                }
            }
        }
        Err(_) => HealthCheck {
            name: "Credentials".into(),
            status: CheckStatus::Ok,
            message: "no credentials file".into(),
            details: vec![],
            hint: None,
        },
    }
}

// ─── Helpers ─────────────────────────────────────────────────

fn load_shell_files(config: &AppConfig) -> Vec<ShellFile> {
    let mut files = Vec::new();
    let primary = shellexpand(&config.files.primary);
    if primary.exists() {
        if let Ok(sf) = parse_shell_file(&primary) {
            files.push(sf);
        }
    }
    let ref_path = shellexpand(&config.files.reference);
    if config.files.use_reference_file && ref_path.exists() {
        if let Ok(sf) = parse_shell_file(&ref_path) {
            files.push(sf);
        }
    }

    // Load active profile file if set
    if !config.profiles.active.is_empty() {
        if let Some(profile) = config.profiles.entries.get(&config.profiles.active) {
            let profile_path = shellexpand(&profile.file);
            if profile_path.exists() {
                if let Ok(sf) = parse_shell_file(&profile_path) {
                    files.push(sf);
                }
            }
        }
    }

    // Load shared file
    let shared = shellexpand(&config.profiles.shared_file);
    if shared.exists() {
        if let Ok(sf) = parse_shell_file(&shared) {
            files.push(sf);
        }
    }

    files
}

fn collect_entries(shell_files: &[ShellFile]) -> Vec<EnvEntry> {
    crate::ops::collect_all_entries(shell_files)
}

fn shellexpand(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    std::path::PathBuf::from(path)
}
