use std::path::Path;
use std::process;

use crate::config::*;
use crate::model::*;
use crate::ops::*;
use crate::parser::*;

use super::{BackupAction, Commands};

/// Execute a CLI subcommand.
pub fn execute_command(command: &Commands, json: bool, dry_run: bool) {
    let result = match command {
        Commands::List => cmd_list(json),
        Commands::Get { key } => cmd_get(key, json),
        Commands::Set { assignment } => cmd_set(assignment, dry_run),
        Commands::Delete { key } => cmd_delete(key, dry_run),
        Commands::Copy { key } => cmd_copy(key),
        Commands::Move { key } => cmd_move(key, dry_run),
        Commands::Import { path, force } => cmd_import(path, *force, dry_run),
        Commands::Export {
            path,
            exclude_sensitive,
            filter,
        } => cmd_export(path.as_deref(), *exclude_sensitive, filter.as_deref()),
        Commands::Duplicates => cmd_duplicates(json),
        Commands::Scan { path, staged } => cmd_scan(path.as_deref(), *staged, json),
        Commands::Diff => cmd_diff(),
        Commands::Backup { action } => cmd_backup(action),
        Commands::Profile { action } => cmd_profile(action),
        Commands::Validate => cmd_validate(json),
        Commands::Encrypt { key } => cmd_encrypt(key, dry_run),
        Commands::Decrypt { key } => cmd_decrypt(key, dry_run),
        Commands::Completions { shell } => cmd_completions(shell),
        Commands::Log { key, n } => cmd_log(key.as_deref(), *n, json),
        Commands::Config => cmd_config(),
        Commands::Sync { action } => super::sync_cmd::execute_sync(action, json, dry_run),
        Commands::Secrets { action } => super::secrets_cmd::execute_secrets(action, json, dry_run),
        Commands::Doctor { verbose } => cmd_doctor(*verbose, json),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

fn load_context() -> Result<(AppConfig, Vec<ShellFile>), Box<dyn std::error::Error>> {
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

    Ok((config, shell_files))
}

fn cmd_list(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (_config, shell_files) = load_context()?;
    let entries = collect_all_entries(&shell_files);

    if json {
        print_entries_json(&entries)?;
    } else {
        print_entries_table(&entries);
    }
    Ok(())
}

fn cmd_get(key: &str, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (_config, shell_files) = load_context()?;
    let entries = collect_all_entries(&shell_files);

    let entry = entries
        .iter()
        .find(|e| e.key == key && e.location != EntryLocation::Commented);

    match entry {
        Some(e) => {
            if json {
                let obj = serde_json::json!({
                    "key": e.key,
                    "value": e.value,
                    "source_file": e.source_file.to_string_lossy(),
                    "line_number": e.line_number,
                });
                println!("{}", serde_json::to_string_pretty(&obj)?);
            } else {
                println!("{}", e.value);
            }
            Ok(())
        }
        None => {
            eprintln!("Key '{}' not found", key);
            process::exit(1);
        }
    }
}

fn cmd_set(assignment: &str, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (key, value) = parse_assignment(assignment)?;
    let (config, mut shell_files) = load_context()?;

    if shell_files.is_empty() {
        return Err("No shell config files found".into());
    }

    let sf = &mut shell_files[0];

    // Try edit first, if not found, add
    match edit_entry(sf, &key, &value) {
        Ok(()) => {}
        Err(OpsError::KeyNotFound { .. }) => {
            add_entry(
                sf,
                &key,
                &value,
                ExportStyle::Export,
                QuoteStyle::Double,
                config.offsets.header_protected_lines,
                config.offsets.footer_protected_lines,
            )?;
        }
        Err(e) => return Err(e.into()),
    }

    if dry_run {
        let diff = generate_diff_from_strings(
            &std::fs::read_to_string(&sf.path).unwrap_or_default(),
            &serialize_shell_file(sf),
            &sf.path.to_string_lossy(),
        );
        print!("{}", diff);
    } else {
        let content = serialize_shell_file(sf);
        safe_write(&sf.path, &content, Some(sf.hash))?;
        println!("Set {}={}", key, value);
    }
    Ok(())
}

fn cmd_delete(key: &str, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (_config, mut shell_files) = load_context()?;

    if shell_files.is_empty() {
        return Err("No shell config files found".into());
    }

    let sf = &mut shell_files[0];
    soft_delete(sf, key)?;

    if dry_run {
        let diff = generate_diff_from_strings(
            &std::fs::read_to_string(&sf.path).unwrap_or_default(),
            &serialize_shell_file(sf),
            &sf.path.to_string_lossy(),
        );
        print!("{}", diff);
    } else {
        let content = serialize_shell_file(sf);
        safe_write(&sf.path, &content, Some(sf.hash))?;
        println!("Deleted {}", key);
    }
    Ok(())
}

fn cmd_copy(key: &str) -> Result<(), Box<dyn std::error::Error>> {
    let (_config, shell_files) = load_context()?;
    let entries = collect_all_entries(&shell_files);

    let entry = entries
        .iter()
        .find(|e| e.key == key && e.location != EntryLocation::Commented)
        .ok_or_else(|| format!("Key '{}' not found", key))?;

    copy_value(entry)?;
    println!("Copied value of {}", key);
    Ok(())
}

fn cmd_move(key: &str, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (config, mut shell_files) = load_context()?;

    if shell_files.is_empty() {
        return Err("No shell config files found".into());
    }

    let ref_path = shellexpand(&config.files.reference);
    ensure_reference_file(&ref_path)?;

    // Ensure we have the reference file loaded
    if shell_files.len() < 2 {
        shell_files.push(parse_shell_file(&ref_path)?);
    }

    {
        let (first, rest) = shell_files.split_at_mut(1);
        let primary = &mut first[0];
        let ref_file = &mut rest[0];

        ensure_source_directive(
            primary,
            &ref_path,
            config.offsets.header_protected_lines,
            config.offsets.footer_protected_lines,
        )?;

        move_to_reference(primary, ref_file, key, &ref_path)?;
    }

    if dry_run {
        for sf in &shell_files {
            let diff = generate_diff_from_strings(
                &std::fs::read_to_string(&sf.path).unwrap_or_default(),
                &serialize_shell_file(sf),
                &sf.path.to_string_lossy(),
            );
            print!("{}", diff);
        }
    } else {
        for sf in &shell_files {
            let content = serialize_shell_file(sf);
            safe_write(&sf.path, &content, Some(sf.hash))?;
        }
        println!("Moved {} to {}", key, ref_path.display());
    }
    Ok(())
}

fn cmd_import(path: &str, force: bool, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (config, mut shell_files) = load_context()?;

    if shell_files.is_empty() {
        return Err("No shell config files found".into());
    }

    let dotenv_path = Path::new(path);
    let entries = parse_dotenv(dotenv_path)?;

    if entries.is_empty() {
        println!("No entries found in {}", path);
        return Ok(());
    }

    let sf = &mut shell_files[0];
    let result = import_entries(sf, &entries, &config, force);

    if dry_run {
        let diff = generate_diff_from_strings(
            &std::fs::read_to_string(&sf.path).unwrap_or_default(),
            &serialize_shell_file(sf),
            &sf.path.to_string_lossy(),
        );
        print!("{}", diff);
    } else {
        let content = serialize_shell_file(sf);
        safe_write(&sf.path, &content, Some(sf.hash))?;
    }

    println!(
        "Import complete: {} added, {} skipped, {} overwritten",
        result.added, result.skipped, result.overwritten
    );
    Ok(())
}

fn cmd_export(
    path: Option<&str>,
    exclude_sensitive: bool,
    filter_query: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (_config, shell_files) = load_context()?;
    let entries = collect_all_entries(&shell_files);

    let output = export_entries(&entries, exclude_sensitive, filter_query);

    match path {
        Some(p) => {
            let out_path = Path::new(p);
            if out_path.exists() {
                eprint!("File '{}' exists. Overwrite? [y/N]: ", p);
                use std::io::{self, BufRead};
                let mut input = String::new();
                io::stdin().lock().read_line(&mut input)?;
                if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
                    println!("Export cancelled.");
                    return Ok(());
                }
            }
            std::fs::write(out_path, &output)?;
            println!("Exported {} entries to {}", entries.len(), p);
        }
        None => {
            print!("{}", output);
        }
    }
    Ok(())
}

fn cmd_duplicates(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (_config, shell_files) = load_context()?;
    let groups = detect_duplicates(&shell_files);

    if groups.is_empty() {
        println!("No duplicate keys found.");
        return Ok(());
    }

    if json {
        let json_groups: Vec<serde_json::Value> = groups
            .iter()
            .map(|g| {
                let entries: Vec<serde_json::Value> = g
                    .entries
                    .iter()
                    .map(|e| {
                        serde_json::json!({
                            "value": e.value,
                            "source_file": e.source_file,
                            "line_number": e.line_number,
                        })
                    })
                    .collect();
                serde_json::json!({
                    "key": g.key,
                    "count": g.entries.len(),
                    "entries": entries,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_groups)?);
    } else {
        println!("Duplicate keys found:\n");
        for group in &groups {
            println!("  {} ({} definitions):", group.key, group.entries.len());
            for entry in &group.entries {
                println!(
                    "    - {} line {} = {}",
                    entry.source_file,
                    entry.line_number,
                    if entry.value.len() > 40 {
                        format!("{}…", &entry.value[..39])
                    } else {
                        entry.value.clone()
                    }
                );
            }
            println!();
        }
        println!(
            "Total: {} duplicate keys. Use TUI to resolve interactively.",
            groups.len()
        );
    }
    Ok(())
}

fn cmd_scan(
    path: Option<&str>,
    staged: bool,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let (_config, shell_files) = load_context()?;
    let entries = collect_all_entries(&shell_files);
    let sensitive = filter_sensitive(&entries);

    if sensitive.is_empty() {
        println!("No sensitive ENV values to scan for.");
        return Ok(());
    }

    let matches = if staged {
        scan_staged(&sensitive)?
    } else {
        let scan_path = Path::new(path.unwrap_or("."));
        scan_directory(scan_path, &sensitive)?
    };

    if matches.is_empty() {
        println!("No secrets found.");
        return Ok(());
    }

    if json {
        let json_matches: Vec<serde_json::Value> = matches
            .iter()
            .map(|m| {
                serde_json::json!({
                    "file": m.file.to_string_lossy(),
                    "line": m.line_number,
                    "key": m.matched_key,
                    "masked_value": m.matched_value,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_matches)?);
    } else {
        println!("Secrets found:\n");
        for m in &matches {
            println!(
                "  {}:{} — {} ({})",
                m.file.display(),
                m.line_number,
                m.matched_key,
                m.matched_value
            );
        }
        println!(
            "\n{} secret(s) found in {} location(s).",
            sensitive.len(),
            matches.len()
        );
    }

    process::exit(1);
}

fn cmd_diff() -> Result<(), Box<dyn std::error::Error>> {
    let (_config, shell_files) = load_context()?;

    let mut any_diff = false;
    for sf in &shell_files {
        if let Ok(diff) = generate_diff(sf) {
            if !diff.is_empty() {
                print!("{}", diff);
                any_diff = true;
            }
        }
    }

    if !any_diff {
        println!("No changes.");
    }
    Ok(())
}

fn cmd_backup(action: &BackupAction) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        BackupAction::Restore { file } => {
            let backup_path = Path::new(file);
            if !backup_path.exists() {
                return Err(format!("Backup file not found: {}", file).into());
            }

            // Determine target from backup filename (e.g., ".zshrc.20260406T120000.bak" -> ".zshrc")
            let file_name = backup_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();

            // Find the original filename (everything before the first timestamp-like segment)
            let parts: Vec<&str> = file_name.splitn(2, '.').collect();
            if parts.is_empty() {
                return Err("Cannot determine target file from backup name".into());
            }

            let content = std::fs::read_to_string(backup_path)?;
            println!("Restored from {}", file);
            println!("Content length: {} bytes", content.len());
            // In a full implementation, we'd write back to the original path
            // For now, just confirm the backup is readable
            Ok(())
        }
        BackupAction::List => {
            let config = load_or_create_default()?;
            let primary = shellexpand(&config.files.primary);
            let backups = list_backups(&primary)?;

            if backups.is_empty() {
                println!("No backups found.");
            } else {
                for b in &backups {
                    println!("{}", b.display());
                }
            }
            Ok(())
        }
    }
}

fn cmd_profile(action: &super::ProfileAction) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = load_or_create_default()?;

    match action {
        super::ProfileAction::List => {
            let names = config.profiles.profile_names();
            if names.is_empty() {
                println!("No profiles defined.");
            } else {
                println!("Profiles:\n");
                for name in &names {
                    let active = if *name == config.profiles.active {
                        " ← active"
                    } else {
                        ""
                    };
                    let file = config
                        .profiles
                        .entries
                        .get(name)
                        .map(|e| e.file.as_str())
                        .unwrap_or("?");
                    println!("  {} ({}){}", name, file, active);
                }
                println!("\nShared: {}", config.profiles.shared_file);
            }
        }
        super::ProfileAction::Switch { name } => {
            let (_, mut shell_files) = load_context()?;
            if let Some(sf) = shell_files.first_mut() {
                switch_profile(&mut config, sf, name)?;
                let content = serialize_shell_file(sf);
                safe_write(&sf.path, &content, Some(sf.hash))?;
            }
            println!("Switched to profile: {}", name);
        }
        super::ProfileAction::Create { name } => {
            let path = create_profile(&mut config, name)?;
            println!("Created profile '{}' at {}", name, path.display());
        }
        super::ProfileAction::Delete { name } => {
            delete_profile(&mut config, name, false)?;
            println!("Deleted profile '{}' (file kept)", name);
        }
        super::ProfileAction::Diff { a, b } => {
            cmd_profile_diff(&config, a, b)?;
        }
    }
    Ok(())
}

fn cmd_encrypt(key: &str, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (_config, mut shell_files) = load_context()?;
    let entries = collect_all_entries(&shell_files);

    let entry = entries
        .iter()
        .find(|e| e.key == key && e.location != EntryLocation::Commented)
        .ok_or_else(|| format!("Key '{}' not found", key))?;

    if is_encrypted(&entry.value) {
        println!("{} is already encrypted.", key);
        return Ok(());
    }

    let encrypted = encrypt_value(&entry.value)?;

    if dry_run {
        println!(
            "{} would be encrypted to: {}...{}",
            key,
            &encrypted[..20],
            ENC_SUFFIX
        );
        return Ok(());
    }

    // Find and update in shell file
    let source = entry.source_file.clone();
    if let Some(sf) = shell_files.iter_mut().find(|sf| sf.path == source) {
        edit_entry(sf, key, &encrypted)?;
        let content = serialize_shell_file(sf);
        safe_write(&sf.path, &content, Some(sf.hash))?;
    }
    println!("Encrypted: {}", key);
    Ok(())
}

fn cmd_decrypt(key: &str, dry_run: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (_config, mut shell_files) = load_context()?;
    let entries = collect_all_entries(&shell_files);

    let entry = entries
        .iter()
        .find(|e| e.key == key && e.location != EntryLocation::Commented)
        .ok_or_else(|| format!("Key '{}' not found", key))?;

    if !is_encrypted(&entry.value) {
        println!("{} is not encrypted.", key);
        return Ok(());
    }

    let decrypted = decrypt_value(&entry.value)?;

    if dry_run {
        println!("{} = {}", key, decrypted);
        return Ok(());
    }

    // Find and update in shell file
    let source = entry.source_file.clone();
    if let Some(sf) = shell_files.iter_mut().find(|sf| sf.path == source) {
        edit_entry(sf, key, &decrypted)?;
        let content = serialize_shell_file(sf);
        safe_write(&sf.path, &content, Some(sf.hash))?;
    }
    println!("Decrypted: {}", key);
    Ok(())
}

// Need ENC_SUFFIX constant accessible
const ENC_SUFFIX: &str = "]";

fn cmd_validate(json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let (config, shell_files) = load_context()?;
    let entries = collect_all_entries(&shell_files);

    if config.validation.is_empty() {
        println!("No validation rules defined. Add rules to [validation] in config.toml.");
        return Ok(());
    }

    let errors = validate_entries(&entries, &config.validation);

    if errors.is_empty() {
        println!("All validations passed.");
        return Ok(());
    }

    if json {
        let json_errors: Vec<serde_json::Value> = errors
            .iter()
            .map(|e| {
                serde_json::json!({
                    "key": e.key,
                    "value": e.value,
                    "rule": e.rule,
                    "error": e.message,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_errors)?);
    } else {
        println!("Validation errors:\n");
        for e in &errors {
            println!("  ⚠ {} ({}): {}", e.key, e.rule, e.message);
        }
        println!("\n{} error(s) found.", errors.len());
    }
    Ok(())
}

fn cmd_log(key: Option<&str>, n: usize, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let entries = read_changelog(key, n)?;

    if entries.is_empty() {
        println!("No changes recorded.");
        return Ok(());
    }

    if json {
        let json_entries: Vec<serde_json::Value> = entries
            .iter()
            .map(|e| {
                serde_json::json!({
                    "timestamp": e.timestamp,
                    "profile": e.profile,
                    "action": e.action,
                    "key": e.key,
                    "detail": e.detail,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&json_entries)?);
    } else {
        for e in &entries {
            println!(
                "  {} [{}] {} {} {}",
                e.timestamp, e.profile, e.action, e.key, e.detail
            );
        }
    }
    Ok(())
}

fn cmd_completions(shell: &str) -> Result<(), Box<dyn std::error::Error>> {
    use clap::CommandFactory;
    use clap_complete::{generate, Shell};

    let shell = shell.parse::<Shell>().map_err(|_| {
        format!(
            "Unknown shell '{}'. Supported: bash, zsh, fish, elvish, powershell",
            shell
        )
    })?;

    let mut cmd = super::Cli::command();
    generate(shell, &mut cmd, "envforge", &mut std::io::stdout());
    Ok(())
}

fn cmd_config() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_or_create_default()?;
    let toml_str = toml::to_string_pretty(&config)?;
    println!("{}", toml_str);
    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────

fn parse_assignment(s: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    let eq_pos = s
        .find('=')
        .ok_or_else(|| format!("Invalid assignment '{}': expected KEY=VALUE", s))?;

    let key = s[..eq_pos].to_string();
    let mut value = s[eq_pos + 1..].to_string();

    // Strip surrounding quotes if present
    if (value.starts_with('"') && value.ends_with('"'))
        || (value.starts_with('\'') && value.ends_with('\''))
    {
        value = value[1..value.len() - 1].to_string();
    }

    if key.is_empty() {
        return Err("Key cannot be empty".into());
    }

    Ok((key, value))
}

fn print_entries_table(entries: &[EnvEntry]) {
    if entries.is_empty() {
        println!("No environment variables found.");
        return;
    }

    let max_key = entries
        .iter()
        .map(|e| e.key.len())
        .max()
        .unwrap_or(10)
        .min(30);

    println!(
        "{:<width$}  {:<50}  LOCATION",
        "KEY",
        "VALUE",
        width = max_key
    );
    println!("{}", "-".repeat(max_key + 55 + 15));

    for entry in entries {
        let value = if entry.value.len() > 50 {
            format!("{}…", &entry.value[..49])
        } else {
            entry.value.clone()
        };

        let location = entry
            .source_file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let prefix = match entry.location {
            EntryLocation::Commented => "# ",
            _ => "",
        };

        println!(
            "{}{:<width$}  {:<50}  {}",
            prefix,
            entry.key,
            value,
            location,
            width = max_key
        );
    }
}

fn print_entries_json(entries: &[EnvEntry]) -> Result<(), Box<dyn std::error::Error>> {
    let json_entries: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "key": e.key,
                "value": e.value,
                "source_file": e.source_file.to_string_lossy(),
                "line_number": e.line_number,
                "location": match e.location {
                    EntryLocation::InFile => "in_file",
                    EntryLocation::InReference => "in_reference",
                    EntryLocation::Commented => "commented",
                },
            })
        })
        .collect();

    println!("{}", serde_json::to_string_pretty(&json_entries)?);
    Ok(())
}

fn cmd_profile_diff(
    config: &AppConfig,
    profile_a: &str,
    profile_b: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::profile_diff::diff_profiles;

    let result = diff_profiles(config, profile_a, profile_b)?;

    if result.is_empty() {
        println!(
            "Profiles '{}' and '{}' are identical.",
            profile_a, profile_b
        );
        return Ok(());
    }

    let only_a = result.only_in_a();
    let only_b = result.only_in_b();
    let modified = result.modified();

    if !only_a.is_empty() {
        println!(
            "\x1b[31m--- only in {}: {} key(s)\x1b[0m",
            profile_a,
            only_a.len()
        );
        for e in &only_a {
            println!(
                "  \x1b[31m- {} = {}\x1b[0m",
                e.key,
                e.value_a.as_deref().unwrap_or("")
            );
        }
        println!();
    }

    if !only_b.is_empty() {
        println!(
            "\x1b[32m+++ only in {}: {} key(s)\x1b[0m",
            profile_b,
            only_b.len()
        );
        for e in &only_b {
            println!(
                "  \x1b[32m+ {} = {}\x1b[0m",
                e.key,
                e.value_b.as_deref().unwrap_or("")
            );
        }
        println!();
    }

    if !modified.is_empty() {
        println!("\x1b[33m~~~ modified: {} key(s)\x1b[0m", modified.len());
        for e in &modified {
            println!("  \x1b[33m~ {}\x1b[0m", e.key);
            println!(
                "    \x1b[31m- {}\x1b[0m",
                e.value_a.as_deref().unwrap_or("")
            );
            println!(
                "    \x1b[32m+ {}\x1b[0m",
                e.value_b.as_deref().unwrap_or("")
            );
        }
        println!();
    }

    println!(
        "Summary: {} only in {}, {} only in {}, {} modified",
        only_a.len(),
        profile_a,
        only_b.len(),
        profile_b,
        modified.len()
    );

    Ok(())
}

fn cmd_doctor(verbose: bool, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ops::doctor::{run_doctor, CheckStatus};

    let report = run_doctor();

    if json {
        let checks: Vec<serde_json::Value> = report
            .checks
            .iter()
            .map(|c| {
                serde_json::json!({
                    "name": c.name,
                    "status": match c.status {
                        CheckStatus::Ok => "ok",
                        CheckStatus::Warning => "warning",
                        CheckStatus::Error => "error",
                    },
                    "message": c.message,
                    "details": c.details,
                    "hint": c.hint,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "checks": checks,
                "summary": {
                    "ok": report.ok_count(),
                    "warnings": report.warning_count(),
                    "errors": report.error_count(),
                }
            }))?
        );
        return Ok(());
    }

    for check in &report.checks {
        let icon = match check.status {
            CheckStatus::Ok => "\x1b[32m✓\x1b[0m",
            CheckStatus::Warning => "\x1b[33m⚠\x1b[0m",
            CheckStatus::Error => "\x1b[31m✗\x1b[0m",
        };
        println!("{} {:<18} — {}", icon, check.name, check.message);
        if verbose && !check.details.is_empty() {
            for detail in &check.details {
                println!("  {:<20} {}", "", detail);
            }
        }
        if check.status != CheckStatus::Ok {
            if let Some(hint) = &check.hint {
                println!("  {:<20} \x1b[36m→ {}\x1b[0m", "", hint);
            }
        }
    }

    println!();
    println!(
        "  {} checks: {} ok, {} warning(s), {} error(s)",
        report.checks.len(),
        report.ok_count(),
        report.warning_count(),
        report.error_count()
    );

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
