use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::load_or_create_default;
use crate::ops::dotenv::parse_dotenv;
use crate::ops::encrypt::{decrypt_value, is_encrypted};
use crate::ops::secrets::cache::{is_reference, resolve_reference, SecretRef};
use crate::ops::secrets::credentials::read_all_credentials;
use crate::ops::secrets::providers::create_default_registry;
use crate::parser::parse_shell_file;

// ─── Types ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RunConfig {
    pub profile: Option<String>,
    pub profiles: Vec<String>,
    pub resolve: bool,
    pub env_files: Vec<PathBuf>,
    pub overrides: Vec<(String, String)>,
}

#[derive(Debug)]
pub struct RunResult {
    pub exit_code: i32,
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("config error: {0}")]
    Config(String),

    #[error("profile '{0}' not found. Available: {1}")]
    ProfileNotFound(String, String),

    #[error("env file not found: {}", .0.display())]
    EnvFileNotFound(PathBuf),

    #[error("decrypt failed for key '{key}': {message}")]
    DecryptFailed { key: String, message: String },

    #[error("resolve failed for key '{key}': {message}")]
    ResolveFailed { key: String, message: String },

    #[error("command not found: {0}")]
    CommandNotFound(String),

    #[error("spawn failed: {0}")]
    SpawnFailed(String),
}

// ─── ENV Collection ──────────────────────────────────────────

/// Collect and merge ENV from all sources, decrypt and resolve as configured.
pub fn collect_env(run_config: &RunConfig) -> Result<HashMap<String, String>, RunError> {
    let config = load_or_create_default().map_err(|e| RunError::Config(e.to_string()))?;

    let mut env: HashMap<String, String> = HashMap::new();

    // Layer 1: Current shell environment
    for (k, v) in std::env::vars() {
        env.insert(k, v);
    }

    // Layer 2: Shared file
    let shared_path = shellexpand(&config.profiles.shared_file);
    if shared_path.exists() {
        merge_shell_file(&mut env, &shared_path);
    }

    // Layer 3: Profile file(s)
    if !run_config.profiles.is_empty() {
        // Multi-profile mode: iterate each profile, last wins
        for profile_name in &run_config.profiles {
            let profile = config.profiles.entries.get(profile_name).ok_or_else(|| {
                let available = config.profiles.profile_names().join(", ");
                RunError::ProfileNotFound(profile_name.to_string(), available)
            })?;
            let profile_path = shellexpand(&profile.file);
            if profile_path.exists() {
                merge_shell_file(&mut env, &profile_path);
            }
        }
    } else {
        // Single-profile mode (default)
        let profile_name = run_config
            .profile
            .as_deref()
            .unwrap_or(&config.profiles.active);

        if !profile_name.is_empty() {
            let profile = config.profiles.entries.get(profile_name).ok_or_else(|| {
                let available = config.profiles.profile_names().join(", ");
                RunError::ProfileNotFound(profile_name.to_string(), available)
            })?;
            let profile_path = shellexpand(&profile.file);
            if profile_path.exists() {
                merge_shell_file(&mut env, &profile_path);
            }
        }
    }

    // Layer 4: --env-file(s)
    for env_file in &run_config.env_files {
        if !env_file.exists() {
            return Err(RunError::EnvFileNotFound(env_file.clone()));
        }
        let entries = parse_dotenv(env_file)
            .map_err(|e| RunError::Config(format!("{}: {}", env_file.display(), e)))?;
        for entry in entries {
            env.insert(entry.key, entry.value);
        }
    }

    // Layer 5: --override KEY=VALUE
    for (k, v) in &run_config.overrides {
        env.insert(k.clone(), v.clone());
    }

    // Transform: Decrypt ENC[age:...] values
    let keys: Vec<String> = env.keys().cloned().collect();
    for key in &keys {
        if let Some(value) = env.get(key) {
            if is_encrypted(value) {
                match decrypt_value(value) {
                    Ok(decrypted) => {
                        env.insert(key.clone(), decrypted);
                    }
                    Err(e) => {
                        return Err(RunError::DecryptFailed {
                            key: key.clone(),
                            message: e.to_string(),
                        });
                    }
                }
            }
        }
    }

    // Transform: Resolve ref: values (only when --resolve is set)
    if run_config.resolve {
        let registry = create_default_registry();
        let keys: Vec<String> = env.keys().cloned().collect();
        for key in &keys {
            if let Some(value) = env.get(key) {
                if is_reference(value) {
                    if let Some(secret_ref) = SecretRef::parse(value) {
                        match resolve_with_fallback(&secret_ref, &registry) {
                            Ok(resolved) => {
                                env.insert(key.clone(), resolved);
                            }
                            Err(msg) => {
                                eprintln!(
                                    "warning: could not resolve '{}' from {}: {}",
                                    key, secret_ref.provider, msg
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(env)
}

// ─── Process Spawning ────────────────────────────────────────

/// Spawn a child process with the given environment.
pub fn spawn_process(
    command: &str,
    args: &[String],
    env: &HashMap<String, String>,
) -> Result<RunResult, RunError> {
    use std::process::Command;

    let mut cmd = Command::new(command);
    cmd.args(args);
    cmd.env_clear();
    cmd.envs(env);

    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            RunError::CommandNotFound(command.to_string())
        } else {
            RunError::SpawnFailed(e.to_string())
        }
    })?;

    // Child inherits the process group, so SIGINT/SIGTERM from the terminal
    // are automatically delivered to both parent and child. No manual
    // signal forwarding needed.
    let status = child
        .wait()
        .map_err(|e| RunError::SpawnFailed(e.to_string()))?;

    let exit_code = status.code().unwrap_or_else(|| {
        // Killed by signal on Unix
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            status.signal().map(|s| 128 + s).unwrap_or(1)
        }
        #[cfg(not(unix))]
        1
    });

    Ok(RunResult { exit_code })
}

// ─── Helpers ─────────────────────────────────────────────────

fn merge_shell_file(env: &mut HashMap<String, String>, path: &Path) {
    if let Ok(sf) = parse_shell_file(path) {
        let entries = crate::ops::collect_all_entries(std::slice::from_ref(&sf));
        for entry in entries {
            if entry.location != crate::ops::EntryLocation::Commented {
                env.insert(entry.key, entry.value);
            }
        }
    }
}

fn resolve_with_fallback(
    secret_ref: &SecretRef,
    registry: &crate::ops::secrets::provider::ProviderRegistry,
) -> Result<String, String> {
    let provider = registry
        .get(&secret_ref.provider)
        .map_err(|e| e.to_string())?;
    let credentials = read_all_credentials(&secret_ref.provider).map_err(|e| e.to_string())?;
    resolve_reference(secret_ref, provider, &credentials).map_err(|e| e.to_string())
}

fn shellexpand(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}
