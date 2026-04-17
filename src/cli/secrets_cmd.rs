use clap::Subcommand;
use serde_json::json;

use crate::ops::secrets::credentials;
use crate::ops::secrets::modes;
use crate::ops::secrets::providers::create_default_registry;
use crate::ops::secrets::SecretsError;

#[derive(Subcommand)]
pub enum SecretsAction {
    /// Pull secrets from a provider
    Pull {
        /// Provider name (vault, aws-ssm, 1password, doppler, infisical, gcp, azure)
        #[arg(long)]
        from: String,

        /// Secret path in the provider
        #[arg(long, default_value = "")]
        path: String,

        /// Filter keys by glob pattern
        #[arg(long)]
        filter: Option<String>,
    },

    /// Push secrets to a provider
    Push {
        /// Provider name
        #[arg(long)]
        to: String,

        /// Secret path in the provider
        #[arg(long, default_value = "")]
        path: String,

        /// Specific keys to push (comma-separated)
        #[arg(long)]
        keys: Option<String>,

        /// Push all keys
        #[arg(long)]
        all: bool,

        /// Filter keys by glob pattern
        #[arg(long)]
        filter: Option<String>,
    },

    /// Create a reference to a remote secret
    Ref {
        /// ENV key name
        key: String,

        /// Provider name
        #[arg(long)]
        from: String,

        /// Full path in the provider (including key name)
        #[arg(long)]
        path: String,
    },

    /// Remove a reference (convert back to normal key)
    Unref {
        /// ENV key name
        key: String,
    },

    /// Resolve secret references
    Resolve {
        /// Specific key to resolve (omit for all)
        #[arg(long)]
        key: Option<String>,
    },

    /// Configure provider credentials
    Config {
        /// Provider name
        provider: String,

        /// Set a credential value (key=value format)
        #[arg(long)]
        set: Option<String>,

        /// Show stored credentials
        #[arg(long)]
        show: bool,

        /// Remove all credentials for this provider
        #[arg(long)]
        remove: bool,
    },

    /// List available providers and their status
    Providers,

    /// Show which keys come from which provider
    Status,
}

pub fn execute_secrets(
    action: &SecretsAction,
    json: bool,
    dry_run: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        SecretsAction::Pull { from, path, filter } => {
            cmd_secrets_pull(from, path, filter.as_deref(), dry_run, json)
        }
        SecretsAction::Push {
            to,
            path,
            keys,
            all,
            filter,
        } => cmd_secrets_push(
            to,
            path,
            keys.as_deref(),
            *all,
            filter.as_deref(),
            dry_run,
            json,
        ),
        SecretsAction::Ref { key, from, path } => cmd_secrets_ref(key, from, path, json),
        SecretsAction::Unref { key } => cmd_secrets_unref(key, json),
        SecretsAction::Resolve { key } => cmd_secrets_resolve(key.as_deref(), json),
        SecretsAction::Config {
            provider,
            set,
            show,
            remove,
        } => cmd_secrets_config(provider, set.as_deref(), *show, *remove, json),
        SecretsAction::Providers => cmd_secrets_providers(json),
        SecretsAction::Status => cmd_secrets_status(json),
    }
}

fn cmd_secrets_pull(
    provider_name: &str,
    path: &str,
    filter: Option<&str>,
    dry_run: bool,
    json_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let registry = create_default_registry();
    let existing = std::collections::HashMap::new(); // TODO: load from EnvForge config

    let (entries, result, _sources) =
        modes::pull_secrets(&registry, provider_name, path, filter, &existing)?;

    if json_output {
        println!(
            "{}",
            json!({
                "provider": result.provider,
                "total": result.total,
                "new": result.keys_new,
                "updated": result.keys_updated,
                "skipped": result.keys_skipped,
                "dry_run": dry_run,
            })
        );
    } else {
        if dry_run {
            println!("[dry-run] Would pull from {}:", provider_name);
        } else {
            println!("Pulled from {}:", provider_name);
        }
        println!(
            "  {} total, {} new, {} updated, {} skipped",
            result.total,
            result.keys_new.len(),
            result.keys_updated.len(),
            result.keys_skipped.len()
        );
        for (key, _) in &entries {
            println!("  + {}", key);
        }
    }

    Ok(())
}

fn cmd_secrets_push(
    provider_name: &str,
    path: &str,
    keys: Option<&str>,
    all: bool,
    filter: Option<&str>,
    dry_run: bool,
    json_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let registry = create_default_registry();

    // TODO: load from EnvForge config — for now collect from available entries
    let all_secrets: Vec<(String, String)> = Vec::new();

    let secrets: Vec<(String, String)> = if let Some(key_list) = keys {
        let wanted: Vec<&str> = key_list.split(',').collect();
        all_secrets
            .into_iter()
            .filter(|(k, _)| wanted.contains(&k.as_str()))
            .collect()
    } else if all {
        all_secrets
    } else if filter.is_some() {
        // Filter applied in modes::push_secrets
        all_secrets
    } else {
        return Err("Specify --keys, --all, or --filter to select keys to push".into());
    };

    if dry_run {
        let display_secrets = if let Some(pattern) = filter {
            secrets
                .iter()
                .filter(|(k, _)| modes::glob_match(pattern, k))
                .cloned()
                .collect::<Vec<_>>()
        } else {
            secrets.clone()
        };
        if json_output {
            println!(
                "{}",
                json!({"dry_run": true, "keys": display_secrets.iter().map(|(k,_)| k).collect::<Vec<_>>()})
            );
        } else {
            println!(
                "[dry-run] Would push {} keys to {}",
                display_secrets.len(),
                provider_name
            );
        }
        return Ok(());
    }

    let result = modes::push_secrets(&registry, provider_name, path, &secrets, filter)?;

    if json_output {
        println!(
            "{}",
            json!({"provider": result.provider, "keys_pushed": result.keys_pushed})
        );
    } else {
        println!("Pushed {} keys to {}", result.keys_pushed, provider_name);
    }

    Ok(())
}

fn cmd_secrets_ref(
    key: &str,
    provider: &str,
    path: &str,
    json_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let ref_string = format!("ref:{}:{}", provider, path);

    if json_output {
        println!("{}", json!({"key": key, "reference": ref_string}));
    } else {
        println!("Reference created: {} → {}", key, ref_string);
        println!("Store this value for the key '{}' in your config.", key);
    }

    Ok(())
}

fn cmd_secrets_unref(key: &str, json_output: bool) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: read key from EnvForge config, check if it's a reference, convert to plain value
    // For now, instruct user
    if json_output {
        println!(
            "{}",
            json!({"key": key, "action": "unref", "note": "resolve the reference first, then replace the ref: value with the resolved value"})
        );
    } else {
        println!("To unreference '{}':", key);
        println!("  1. Run: envforge secrets resolve --key {}", key);
        println!(
            "  2. Set the resolved value: envforge set {}=<resolved_value>",
            key
        );
        println!("  (This removes the ref: prefix and stores the actual value)");
    }

    Ok(())
}

fn cmd_secrets_resolve(
    key: Option<&str>,
    json_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::config::load_or_create_default;
    use crate::ops::secrets::cache::is_reference;
    use crate::parser::parse_shell_file;

    let registry = create_default_registry();

    // Load entries from shell files
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

    // Load profile files
    if !config.profiles.active.is_empty() {
        if let Some(profile) = config.profiles.entries.get(&config.profiles.active) {
            let profile_path = shellexpand(&profile.file);
            if profile_path.exists() {
                shell_files.push(parse_shell_file(&profile_path)?);
            }
        }
    }

    let shared = shellexpand(&config.profiles.shared_file);
    if shared.exists() {
        shell_files.push(parse_shell_file(&shared)?);
    }

    let all_entries = crate::ops::collect_all_entries(&shell_files);

    // Convert to (key, value) tuples, filtering inactive
    let entries: Vec<(String, String)> = all_entries
        .iter()
        .filter(|e| e.location != crate::ops::EntryLocation::Commented)
        .map(|e| (e.key.clone(), e.value.clone()))
        .collect();

    // Filter by key if specified, or only references if no key
    let filtered: Vec<(String, String)> = if let Some(k) = key {
        entries.into_iter().filter(|(ek, _)| ek == k).collect()
    } else {
        // By default, resolve only references for shell eval
        entries
            .into_iter()
            .filter(|(_, v)| is_reference(v))
            .collect()
    };

    if filtered.is_empty() {
        if let Some(k) = key {
            eprintln!("# key '{}' not found", k);
        }
        // Output nothing — safe for eval
        return Ok(());
    }

    let resolved = modes::resolve_all_references(&filtered, &registry)?;

    if json_output {
        let items: Vec<serde_json::Value> = resolved
            .iter()
            .map(|(k, v, is_ref)| json!({"key": k, "value": v, "was_reference": is_ref}))
            .collect();
        println!("{}", serde_json::to_string_pretty(&json!(items))?);
    } else {
        // Output shell-compatible export statements for eval
        for (k, v, _) in &resolved {
            // Escape single quotes for safe shell embedding
            let escaped = v.replace('\'', "'\\''");
            println!("export {}='{}'", k, escaped);
        }
    }

    Ok(())
}

fn shellexpand(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    std::path::PathBuf::from(path)
}

fn cmd_secrets_config(
    provider: &str,
    set: Option<&str>,
    show: bool,
    remove: bool,
    json_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if remove {
        let removed = credentials::remove_credentials(provider)?;
        if json_output {
            println!("{}", json!({"removed": removed, "provider": provider}));
        } else if removed {
            println!("Credentials removed for '{}'", provider);
        } else {
            println!("No credentials found for '{}'", provider);
        }
        return Ok(());
    }

    if show {
        match credentials::read_all_credentials(provider) {
            Ok(creds) => {
                if json_output {
                    println!("{}", serde_json::to_string_pretty(&creds)?);
                } else {
                    for (key, value) in &creds {
                        println!("  {} = {}***", key, &value[..value.len().min(4)]);
                    }
                }
            }
            Err(SecretsError::CredentialNotFound { .. }) => {
                if json_output {
                    println!("{}", json!({"configured": false, "provider": provider}));
                } else {
                    println!("No credentials configured for '{}'", provider);
                }
            }
            Err(e) => return Err(Box::new(e)),
        }
        return Ok(());
    }

    if let Some(assignment) = set {
        let parts: Vec<&str> = assignment.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err("Format: --set key=value".into());
        }
        credentials::store_credential(provider, parts[0], parts[1])?;
        if json_output {
            println!(
                "{}",
                json!({"stored": true, "provider": provider, "key": parts[0]})
            );
        } else {
            println!(
                "Credential '{}' stored for '{}' (encrypted)",
                parts[0], provider
            );
        }
        return Ok(());
    }

    // Show help for this provider
    let registry = create_default_registry();
    if let Ok(p) = registry.get(provider) {
        let fields = p.credential_fields();
        if json_output {
            println!(
                "{}",
                json!({"provider": provider, "required_fields": fields})
            );
        } else {
            println!("Provider '{}' requires:", provider);
            for f in &fields {
                println!("  envforge secrets config {} --set {}=<value>", provider, f);
            }
        }
    } else {
        eprintln!("Unknown provider '{}'", provider);
    }

    Ok(())
}

fn cmd_secrets_providers(json_output: bool) -> Result<(), Box<dyn std::error::Error>> {
    let registry = create_default_registry();
    let statuses = registry.list_with_status();

    let configured = credentials::list_configured_providers().unwrap_or_default();

    if json_output {
        let items: Vec<serde_json::Value> = statuses
            .iter()
            .map(|s| {
                json!({
                    "name": s.name,
                    "display_name": s.display_name,
                    "binary": s.binary_name,
                    "binary_found": s.binary_found,
                    "configured": configured.contains(&s.name),
                })
            })
            .collect();
        println!("{}", json!(items));
    } else {
        for s in &statuses {
            let binary_icon = if s.binary_found { "✓" } else { "✗" };
            let config_icon = if configured.contains(&s.name) {
                "✓"
            } else {
                "·"
            };
            println!(
                "  {} [bin:{}] [cfg:{}] {} ({})",
                s.name, binary_icon, config_icon, s.display_name, s.binary_name
            );
        }
    }

    Ok(())
}

fn cmd_secrets_status(json_output: bool) -> Result<(), Box<dyn std::error::Error>> {
    let configured = credentials::list_configured_providers().unwrap_or_default();

    if json_output {
        println!("{}", json!({"configured_providers": configured}));
    } else if configured.is_empty() {
        println!("No secret managers configured.");
        println!("Run `envforge secrets config <provider> --set token=<value>` to get started.");
    } else {
        println!("Configured providers:");
        for p in &configured {
            println!("  ✓ {}", p);
        }
    }

    Ok(())
}
