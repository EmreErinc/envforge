use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::Path;

use super::config::{active_env_path, load_project_config, save_project_config, DetectedConfig};
use super::ProjectError;

// ─── Report Types ───────────────────────────────────────────

/// Summary of wizard execution.
#[derive(Debug)]
pub struct WizardReport {
    pub steps_run: Vec<String>,
    pub schema_keys: usize,
    pub values_set: usize,
    pub values_skipped: usize,
}

/// Summary of values step.
#[derive(Debug)]
pub struct ValuesReport {
    pub set_count: usize,
    pub skipped_count: usize,
}

// ─── Wizard Runner ──────────────────────────────────────────

/// Run the 3-step project wizard.
/// Resumes from last completed step unless `force` is true.
pub fn run_wizard(
    root: &Path,
    detected: &DetectedConfig,
    force: bool,
    dry_run: bool,
) -> Result<WizardReport, ProjectError> {
    let mut config = load_project_config(detected)?;
    let completed = if force {
        Vec::new()
    } else {
        config.wizard.completed_steps.clone()
    };

    let mut report = WizardReport {
        steps_run: Vec::new(),
        schema_keys: 0,
        values_set: 0,
        values_skipped: 0,
    };

    // Step 1: Init — always already done if we got here
    if !completed.contains(&"init".to_string()) {
        // Config exists since we loaded it, mark init complete
        if !config.wizard.completed_steps.contains(&"init".to_string()) {
            config.wizard.completed_steps.push("init".to_string());
        }
        report.steps_run.push("init".to_string());
    }

    // Step 2: Schema
    if !completed.contains(&"schema".to_string()) {
        println!("Step 2/3: Schema generation");
        println!();

        if dry_run {
            println!("  [dry-run] Would generate schema");
        } else {
            let key_count = run_schema_step(root, &config)?;
            report.schema_keys = key_count;

            if !config
                .wizard
                .completed_steps
                .contains(&"schema".to_string())
            {
                config.wizard.completed_steps.push("schema".to_string());
            }
            save_project_config(&config, &detected.config_path, detected.format)?;
        }
        report.steps_run.push("schema".to_string());
    } else {
        println!("Step 2/3: Schema — already done (skip)");
    }

    // Step 3: Values
    if !completed.contains(&"values".to_string()) {
        println!();
        println!("Step 3/3: Key-value entry");
        println!("  Enter values for each key. Press Enter to skip, type 'done' to finish.");
        println!();

        if dry_run {
            println!("  [dry-run] Would prompt for values");
        } else {
            let schema_path = root.join(&config.project.schema_path);
            let env_path = active_env_path(&config, root)?;
            let values_report = run_values_step(&schema_path, &env_path)?;

            report.values_set = values_report.set_count;
            report.values_skipped = values_report.skipped_count;

            if !config
                .wizard
                .completed_steps
                .contains(&"values".to_string())
            {
                config.wizard.completed_steps.push("values".to_string());
            }
            save_project_config(&config, &detected.config_path, detected.format)?;
        }
        report.steps_run.push("values".to_string());
    } else {
        println!("Step 3/3: Values — already done (skip)");
    }

    Ok(report)
}

// ─── Schema Step ────────────────────────────────────────────

/// Generate .env.schema from current project .env.
/// Returns number of keys in schema.
pub fn run_schema_step(
    root: &Path,
    config: &super::config::ProjectConfig,
) -> Result<usize, ProjectError> {
    let schema_path = root.join(&config.project.schema_path);
    let env_path = active_env_path(config, root)?;

    // If schema exists, ask reuse
    if schema_path.exists() {
        println!("  Schema found at {}", config.project.schema_path.display());
        print!("  Reuse existing schema? [Y/n] ");
        io::stdout().flush().ok();

        let mut input = String::new();
        io::stdin().lock().read_line(&mut input).ok();
        let answer = input.trim().to_lowercase();

        if answer.is_empty() || answer == "y" || answer == "yes" {
            // Count keys in existing schema
            let content =
                std::fs::read_to_string(&schema_path).map_err(|e| ProjectError::IoError {
                    path: schema_path.clone(),
                    source: e,
                })?;
            let key_count = content.lines().filter(|l| l.starts_with('[')).count();
            println!("  Reusing schema ({} keys)", key_count);
            return Ok(key_count);
        }
    }

    // Parse .env to get key-value pairs
    let env = if env_path.exists() {
        parse_dotenv_simple(&env_path)?
    } else {
        HashMap::new()
    };

    if env.is_empty() {
        println!("  No env vars found. Creating empty schema.");
        std::fs::write(&schema_path, "# .env.schema — add variable definitions\n").map_err(
            |e| ProjectError::IoError {
                path: schema_path,
                source: e,
            },
        )?;
        return Ok(0);
    }

    // Generate schema using existing logic
    let schema_content = crate::ops::schema::generate_schema(&env);
    std::fs::write(&schema_path, &schema_content).map_err(|e| ProjectError::IoError {
        path: schema_path,
        source: e,
    })?;

    let key_count = env.len();
    println!("  Generated schema with {} keys", key_count);
    Ok(key_count)
}

// ─── Values Step ────────────────────────────────────────────

/// Prompt user for key-value pairs based on schema.
/// Returns count of set and skipped values.
pub fn run_values_step(schema_path: &Path, env_path: &Path) -> Result<ValuesReport, ProjectError> {
    // Parse schema to get key list
    let keys = if schema_path.exists() {
        parse_schema_keys(schema_path)?
    } else {
        Vec::new()
    };

    if keys.is_empty() {
        println!("  No keys defined in schema.");
        return Ok(ValuesReport {
            set_count: 0,
            skipped_count: 0,
        });
    }

    // Load existing values
    let existing = if env_path.exists() {
        parse_dotenv_simple(env_path)?
    } else {
        HashMap::new()
    };

    let mut new_values: Vec<(String, String)> = Vec::new();
    let mut set_count = 0;
    let mut skipped_count = 0;
    let stdin = io::stdin();

    for (key, type_hint, is_sensitive) in &keys {
        let current = existing.get(key.as_str());
        let current_display = match current {
            Some(_) if *is_sensitive => Some("****".to_string()),
            Some(v) => Some(v.clone()),
            None => None,
        };

        let prompt = match &current_display {
            Some(cur) => format!("  {} ({}) [{}]: ", key, type_hint, cur),
            None => format!("  {} ({}): ", key, type_hint),
        };

        print!("{}", prompt);
        io::stdout().flush().ok();

        let mut input = String::new();
        match stdin.lock().read_line(&mut input) {
            Ok(0) => break, // EOF / Ctrl+D
            Ok(_) => {}
            Err(_) => break,
        }

        let value = input.trim();

        if value == "done" {
            break;
        }

        if value.is_empty() {
            // Keep existing value or skip
            if let Some(cur) = current {
                new_values.push((key.clone(), cur.clone()));
            }
            skipped_count += 1;
        } else {
            new_values.push((key.clone(), value.to_string()));
            set_count += 1;
        }
    }

    // Write env file: merge new values with existing
    let mut final_env = existing;
    for (k, v) in &new_values {
        final_env.insert(k.clone(), v.clone());
    }

    let mut output = String::new();
    output.push_str("# EnvForge project environment\n");
    let mut sorted_keys: Vec<&String> = final_env.keys().collect();
    sorted_keys.sort();
    for key in sorted_keys {
        let val = &final_env[key];
        if val.contains(' ') || val.contains('"') || val.contains('\'') || val.is_empty() {
            output.push_str(&format!("{}=\"{}\"\n", key, val.replace('"', "\\\"")));
        } else {
            output.push_str(&format!("{}={}\n", key, val));
        }
    }

    std::fs::write(env_path, &output).map_err(|e| ProjectError::IoError {
        path: env_path.to_path_buf(),
        source: e,
    })?;

    println!();
    println!("  {} keys set, {} skipped", set_count, skipped_count);

    Ok(ValuesReport {
        set_count,
        skipped_count,
    })
}

// ─── Helpers ────────────────────────────────────────────────

/// Simple .env parser — returns HashMap<key, value>.
pub fn parse_dotenv_simple(path: &Path) -> Result<HashMap<String, String>, ProjectError> {
    let content = std::fs::read_to_string(path).map_err(|e| ProjectError::IoError {
        path: path.to_path_buf(),
        source: e,
    })?;

    let mut map = HashMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let stripped = trimmed.strip_prefix("export ").unwrap_or(trimmed);
        if let Some((key, val)) = stripped.split_once('=') {
            let key = key.trim().to_string();
            let val = val.trim().trim_matches('"').trim_matches('\'').to_string();
            if !key.is_empty() {
                map.insert(key, val);
            }
        }
    }

    Ok(map)
}

/// Parse schema file to extract key names with type hints.
/// Returns Vec<(key, type_hint, is_sensitive)>.
pub fn parse_schema_keys(path: &Path) -> Result<Vec<(String, String, bool)>, ProjectError> {
    let content = std::fs::read_to_string(path).map_err(|e| ProjectError::IoError {
        path: path.to_path_buf(),
        source: e,
    })?;

    let mut keys = Vec::new();
    let mut current_key: Option<String> = None;
    let mut current_type = "string".to_string();
    let mut current_sensitive = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // New section header: [KEY_NAME]
        if trimmed.starts_with('[') && trimmed.ends_with(']') && !trimmed.contains('.') {
            // Save previous key
            if let Some(ref key) = current_key {
                keys.push((key.clone(), current_type.clone(), current_sensitive));
            }
            current_key = Some(trimmed[1..trimmed.len() - 1].to_string());
            current_type = "string".to_string();
            current_sensitive = false;
        } else if let Some(ref _key) = current_key {
            if let Some(val) = trimmed.strip_prefix("type = ") {
                current_type = val.trim_matches('"').to_string();
            } else if trimmed == "sensitive = true" {
                current_sensitive = true;
            }
        }
    }

    // Don't forget last key
    if let Some(key) = current_key {
        keys.push((key, current_type, current_sensitive));
    }

    Ok(keys)
}
