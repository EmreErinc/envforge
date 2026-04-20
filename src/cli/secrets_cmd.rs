use clap::Subcommand;
use serde_json::json;

use crate::ops::secrets::cache;
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

        /// Set TTL for the credential (e.g., "8h", "24h", "7d", "30d"). Used with --set.
        #[arg(long)]
        ttl: Option<String>,
    },

    /// List available providers and their status
    Providers,

    /// Show which keys come from which provider
    Status,

    /// Show age of tracked secrets, flag stale ones
    Age {
        /// Stale threshold in days (default: 90)
        #[arg(long, default_value = "90")]
        threshold: i64,

        /// Only show stale secrets
        #[arg(long)]
        stale_only: bool,
    },

    /// Compare local ENV vars vs provider state
    Diff {
        /// Provider name
        #[arg(long)]
        from: String,

        /// Secret path in the provider
        #[arg(long, default_value = "")]
        path: String,

        /// Filter keys by glob pattern
        #[arg(long)]
        filter: Option<String>,
    },

    /// Manage secret reference cache
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },
}

#[derive(Subcommand)]
pub enum CacheAction {
    /// List all cached secrets
    List,
    /// Clear cache
    Clear {
        /// Only clear cache for a specific provider
        #[arg(long)]
        provider: Option<String>,
    },
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
            ttl,
        } => cmd_secrets_config(provider, set.as_deref(), *show, *remove, ttl.as_deref(), json),
        SecretsAction::Providers => cmd_secrets_providers(json),
        SecretsAction::Status => cmd_secrets_status(json),
        SecretsAction::Age {
            threshold,
            stale_only,
        } => cmd_secrets_age(*threshold, *stale_only, json),
        SecretsAction::Diff { from, path, filter } => {
            cmd_secrets_diff(from, path, filter.as_deref(), json)
        }
        SecretsAction::Cache { action } => cmd_secrets_cache(action, json),
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

    // Track secret ages (skip on dry-run)
    if !dry_run {
        let all_keys: Vec<String> = entries.iter().map(|(k, _)| k.clone()).collect();
        let _ = crate::ops::secrets::age::record_pull(&all_keys, provider_name, path);
    }

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
    ttl: Option<&str>,
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
                    let mut items: Vec<serde_json::Value> = Vec::new();
                    for (key, value) in &creds {
                        let ttl_info = credentials::get_ttl_remaining(provider, key)
                            .ok()
                            .flatten();
                        let mut item = json!({
                            "key": key,
                            "value_preview": format!("{}***", &value[..value.len().min(4)]),
                        });
                        if let Some((expires_at, remaining)) = ttl_info {
                            item["expires_at"] = json!(expires_at);
                            item["ttl_remaining_secs"] = json!(remaining);
                            item["ttl_display"] =
                                json!(credentials::format_ttl_remaining(remaining));
                        }
                        items.push(item);
                    }
                    println!("{}", serde_json::to_string_pretty(&json!({
                        "provider": provider,
                        "credentials": items,
                    }))?);
                } else {
                    for (key, value) in &creds {
                        let ttl_info = credentials::get_ttl_remaining(provider, key)
                            .ok()
                            .flatten();
                        let ttl_display = match ttl_info {
                            Some((_, remaining)) => {
                                format!(" ({})", credentials::format_ttl_remaining(remaining))
                            }
                            None => String::new(),
                        };
                        println!(
                            "  {} = {}***{}",
                            key,
                            &value[..value.len().min(4)],
                            ttl_display
                        );
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

        // Validate TTL format early if provided
        if let Some(ttl_str) = ttl {
            credentials::parse_duration(ttl_str)
                .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        }

        credentials::store_credential_with_ttl(provider, parts[0], parts[1], ttl)?;

        if json_output {
            let mut result = json!({"stored": true, "provider": provider, "key": parts[0]});
            if let Some(ttl_str) = ttl {
                result["ttl"] = json!(ttl_str);
            }
            println!("{}", result);
        } else {
            let ttl_msg = match ttl {
                Some(t) => format!(", expires in {}", t),
                None => String::new(),
            };
            println!(
                "Credential '{}' stored for '{}' (encrypted{})",
                parts[0], provider, ttl_msg
            );
        }
        return Ok(());
    }

    // TTL without --set is invalid
    if ttl.is_some() {
        return Err("--ttl requires --set key=value".into());
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
        let mut provider_info: Vec<serde_json::Value> = Vec::new();
        for p in &configured {
            let expired = credentials::check_all_expiry(p).unwrap_or_default();
            let mut info = json!({"name": p, "configured": true});
            if !expired.is_empty() {
                info["expired_credentials"] = json!(expired.iter().map(|(k, t)| {
                    json!({"key": k, "expired_at": t})
                }).collect::<Vec<_>>());
            }
            provider_info.push(info);
        }
        println!("{}", serde_json::to_string_pretty(&json!({"providers": provider_info}))?);
    } else if configured.is_empty() {
        println!("No secret managers configured.");
        println!("Run `envforge secrets config <provider> --set token=<value>` to get started.");
    } else {
        println!("Configured providers:");
        for p in &configured {
            let expired = credentials::check_all_expiry(p).unwrap_or_default();
            if expired.is_empty() {
                println!("  \u{2713} {}", p);
            } else {
                let keys: Vec<String> = expired.iter().map(|(k, _)| k.clone()).collect();
                println!(
                    "  \u{26a0} {} (expired: {})",
                    p,
                    keys.join(", ")
                );
            }
        }
    }

    Ok(())
}

fn cmd_secrets_age(
    threshold: i64,
    stale_only: bool,
    json_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::secrets::age::get_age_report;

    let entries = get_age_report(threshold)?;

    if entries.is_empty() {
        if json_output {
            println!("{}", json!({"secrets": [], "stale_count": 0}));
        } else {
            println!("No tracked secrets. Pull secrets to start tracking age.");
        }
        return Ok(());
    }

    let filtered: Vec<_> = if stale_only {
        entries.into_iter().filter(|e| e.stale).collect()
    } else {
        entries
    };

    let stale_count = filtered.iter().filter(|e| e.stale).count();

    if json_output {
        let items: Vec<serde_json::Value> = filtered
            .iter()
            .map(|e| {
                json!({
                    "key": e.key,
                    "provider": e.provider,
                    "path": e.path,
                    "updated_at": e.updated_at,
                    "age_days": e.age_days,
                    "stale": e.stale,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "secrets": items,
                "stale_count": stale_count,
                "threshold_days": threshold,
            }))?
        );
    } else {
        println!(
            "{:<30} {:<12} {:<8} {:<12}",
            "KEY", "PROVIDER", "AGE", "STATUS"
        );
        println!("{}", "-".repeat(65));

        for e in &filtered {
            let age_str = if e.age_days < 0 {
                "unknown".to_string()
            } else if e.age_days == 0 {
                "today".to_string()
            } else if e.age_days == 1 {
                "1 day".to_string()
            } else {
                format!("{} days", e.age_days)
            };

            let status = if e.stale {
                "\x1b[31m⚠ STALE\x1b[0m"
            } else {
                "\x1b[32m✓ ok\x1b[0m"
            };

            println!("{:<30} {:<12} {:<8} {}", e.key, e.provider, age_str, status);
        }

        println!();
        if stale_count > 0 {
            println!(
                "{} secret(s) older than {} days. Consider rotating them.",
                stale_count, threshold
            );
        } else {
            println!("All {} secrets within {} day threshold.", filtered.len(), threshold);
        }
    }

    Ok(())
}

fn cmd_secrets_diff(
    provider_name: &str,
    path: &str,
    filter: Option<&str>,
    json_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let registry = create_default_registry();

    // Pull remote secrets
    let existing = std::collections::HashMap::new();
    let (remote_entries, _, _) =
        modes::pull_secrets(&registry, provider_name, path, filter, &existing)?;
    let remote: std::collections::BTreeMap<String, String> =
        remote_entries.into_iter().collect();

    // Load local entries
    let config = crate::config::load_or_create_default()?;
    let mut shell_files = Vec::new();
    let primary = shellexpand(&config.files.primary);
    if primary.exists() {
        shell_files.push(crate::parser::parse_shell_file(&primary)?);
    }
    let ref_path = shellexpand(&config.files.reference);
    if config.files.use_reference_file && ref_path.exists() {
        shell_files.push(crate::parser::parse_shell_file(&ref_path)?);
    }

    // Load profile files
    if !config.profiles.active.is_empty() {
        if let Some(profile) = config.profiles.entries.get(&config.profiles.active) {
            let profile_path = shellexpand(&profile.file);
            if profile_path.exists() {
                shell_files.push(crate::parser::parse_shell_file(&profile_path)?);
            }
        }
    }
    let shared = shellexpand(&config.profiles.shared_file);
    if shared.exists() {
        shell_files.push(crate::parser::parse_shell_file(&shared)?);
    }

    let all_entries = crate::ops::collect_all_entries(&shell_files);
    let local: std::collections::BTreeMap<String, String> = all_entries
        .iter()
        .filter(|e| e.location != crate::ops::EntryLocation::Commented)
        .map(|e| (e.key.clone(), e.value.clone()))
        .collect();

    // Apply filter to local keys too
    let local: std::collections::BTreeMap<String, String> = if let Some(pattern) = filter {
        local
            .into_iter()
            .filter(|(k, _)| modes::glob_match(pattern, k))
            .collect()
    } else {
        local
    };

    // Compute diff
    let mut all_keys: Vec<String> = Vec::new();
    for k in remote.keys().chain(local.keys()) {
        if !all_keys.contains(k) {
            all_keys.push(k.clone());
        }
    }
    all_keys.sort();

    let mut only_remote = Vec::new();
    let mut only_local = Vec::new();
    let mut changed = Vec::new();
    let mut same = Vec::new();

    for key in &all_keys {
        match (local.get(key), remote.get(key)) {
            (Some(lv), Some(rv)) => {
                if lv == rv {
                    same.push(key.clone());
                } else {
                    changed.push((key.clone(), lv.clone(), rv.clone()));
                }
            }
            (None, Some(_rv)) => {
                only_remote.push(key.clone());
            }
            (Some(_lv), None) => {
                only_local.push(key.clone());
            }
            (None, None) => {}
        }
    }

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "provider": provider_name,
                "path": path,
                "same": same.len(),
                "changed": changed.iter().map(|(k, l, r)| json!({"key": k, "local": l, "remote": r})).collect::<Vec<_>>(),
                "only_local": only_local,
                "only_remote": only_remote,
            }))?
        );
    } else {
        println!(
            "Diff: local vs {} ({})\n",
            provider_name,
            if path.is_empty() { "/" } else { path }
        );

        if !changed.is_empty() {
            println!(
                "\x1b[33m~~~ Changed: {} key(s)\x1b[0m",
                changed.len()
            );
            for (key, local_val, remote_val) in &changed {
                println!("  \x1b[33m~ {}\x1b[0m", key);
                let lv = if local_val.len() > 40 {
                    format!("{}...", &local_val[..37])
                } else {
                    local_val.clone()
                };
                let rv = if remote_val.len() > 40 {
                    format!("{}...", &remote_val[..37])
                } else {
                    remote_val.clone()
                };
                println!("    \x1b[31m- local:  {}\x1b[0m", lv);
                println!("    \x1b[32m+ remote: {}\x1b[0m", rv);
            }
            println!();
        }

        if !only_local.is_empty() {
            println!(
                "\x1b[31m--- Only local: {} key(s)\x1b[0m",
                only_local.len()
            );
            for key in &only_local {
                println!("  - {}", key);
            }
            println!();
        }

        if !only_remote.is_empty() {
            println!(
                "\x1b[32m+++ Only remote: {} key(s)\x1b[0m",
                only_remote.len()
            );
            for key in &only_remote {
                println!("  + {}", key);
            }
            println!();
        }

        println!(
            "Summary: {} same, {} changed, {} only local, {} only remote",
            same.len(),
            changed.len(),
            only_local.len(),
            only_remote.len()
        );
    }

    Ok(())
}

fn cmd_secrets_cache(
    action: &CacheAction,
    json_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        CacheAction::List => cmd_secrets_cache_list(json_output),
        CacheAction::Clear { provider } => {
            cmd_secrets_cache_clear(provider.as_deref(), json_output)
        }
    }
}

fn cmd_secrets_cache_list(json_output: bool) -> Result<(), Box<dyn std::error::Error>> {
    let entries = cache::list_all_cached().map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    if entries.is_empty() {
        if json_output {
            println!("{}", json!({"entries": [], "total": 0}));
        } else {
            println!("No cached secrets.");
        }
        return Ok(());
    }

    if json_output {
        let items: Vec<serde_json::Value> = entries
            .iter()
            .map(|e| {
                json!({
                    "provider": e.provider,
                    "key": e.key,
                    "fetched_at": e.fetched_at,
                    "ttl_secs": e.ttl_secs,
                    "expired": e.expired,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "entries": items,
                "total": entries.len(),
            }))?
        );
    } else {
        println!(
            "{:<20} {:<20} {:<25} {:<10}",
            "PROVIDER", "KEY", "FETCHED", "STATUS"
        );
        println!("{}", "-".repeat(75));

        for e in &entries {
            let status = if e.expired {
                "\x1b[33mexpired\x1b[0m"
            } else {
                "\x1b[32mfresh\x1b[0m"
            };

            // Format fetched_at for display
            let fetched = if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&e.fetched_at) {
                dt.format("%Y-%m-%d %H:%M:%S").to_string()
            } else {
                e.fetched_at.clone()
            };

            println!("{:<20} {:<20} {:<25} {}", e.provider, e.key, fetched, status);
        }

        let expired_count = entries.iter().filter(|e| e.expired).count();
        let fresh_count = entries.len() - expired_count;
        println!(
            "\n{} cached entries ({} fresh, {} expired)",
            entries.len(),
            fresh_count,
            expired_count
        );
    }

    Ok(())
}

fn cmd_secrets_cache_clear(
    provider: Option<&str>,
    json_output: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(p) = provider {
        cache::invalidate_provider_cache(p)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        if json_output {
            println!("{}", json!({"cleared": true, "provider": p}));
        } else {
            println!("Cache cleared for provider '{}'", p);
        }
    } else {
        cache::clear_all_cache().map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        if json_output {
            println!("{}", json!({"cleared": true, "provider": "all"}));
        } else {
            println!("All secret cache cleared.");
        }
    }

    Ok(())
}
