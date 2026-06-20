use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use super::config::{
    active_env_path, detect_project_config, load_project_config, save_project_config, ConfigFormat,
    DetectedConfig, ProjectConfig, ProjectEnvironment,
};
use super::init::{
    add_to_gitignore, derive_project_name, env_file_template, init_project, InitOptions,
};
use super::ProjectError;

// ─── Public Types ───────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct WizardOptions {
    /// Re-run all steps regardless of prior `completed_steps`.
    pub force: bool,
    /// Use defaults only — no stdin reads.
    pub non_interactive: bool,
    /// Preseed values step from existing dotenv file (key=value lines).
    pub from_env: Option<PathBuf>,
    /// Clear `completed_steps` before running (stronger than `force`).
    pub reset: bool,
    /// Walk steps but perform no filesystem mutations.
    pub dry_run: bool,
}

#[derive(Debug, Default)]
pub struct WizardReport {
    pub steps_run: Vec<String>,
    pub project_name: String,
    pub format: Option<ConfigFormat>,
    pub environments: Vec<String>,
    pub schema_keys: usize,
    pub values_set: usize,
    pub values_skipped: usize,
    pub gitignore_updated: bool,
    pub ai_context_emitted: bool,
    pub fence_installed: bool,
    pub canary_installed: bool,
}

#[derive(Debug, Default)]
pub struct ValuesReport {
    pub set_count: usize,
    pub skipped_count: usize,
    pub aborted: bool,
}

// ─── Wizard Runner ──────────────────────────────────────────

/// Run the guided project wizard.
///
/// Detects project state and walks the user through every step required
/// to reach a valid project configuration. Standalone — does not require
/// prior `envforge project init`.
pub fn run_wizard(root: &Path, opts: &WizardOptions) -> Result<WizardReport, ProjectError> {
    let mut report = WizardReport::default();

    print_banner(opts.non_interactive);
    print_state_matrix(root);

    if opts.dry_run {
        return dry_run_walk(root, opts);
    }

    // Step 1 — Project Identity / Init
    let (detected, mut config, ran_init) = ensure_initialized(root, opts)?;
    report.project_name = config.project.name.clone();
    report.format = Some(detected.format);
    if ran_init {
        report.steps_run.push("identity".into());
    }

    if opts.reset {
        config.wizard.completed_steps.clear();
        save_project_config(&config, &detected.config_path, detected.format)?;
        println!("(reset: cleared completed_steps)");
    }

    if all_steps_done(&config) && !opts.force && !opts.reset {
        println!();
        println!("Project already fully configured.");
        println!("  Use --force to re-run pending logic on existing config.");
        println!("  Use --reset to wipe completed_steps and restart.");
        report.environments = config.environments.iter().map(|e| e.name.clone()).collect();
        report.schema_keys = count_schema_keys(root, &config);
        return Ok(report);
    }

    // Step 2 — Environments
    if step_pending(&config, "environments", opts.force) {
        step_environments(&mut config, opts.non_interactive)?;
        mark_done(&mut config, "environments");
        save_project_config(&config, &detected.config_path, detected.format)?;
        report.steps_run.push("environments".into());
    }
    report.environments = config.environments.iter().map(|e| e.name.clone()).collect();

    // Step 3 — Schema
    if step_pending(&config, "schema", opts.force) {
        let key_count = step_schema(root, &config, opts.non_interactive)?;
        report.schema_keys = key_count;
        mark_done(&mut config, "schema");
        save_project_config(&config, &detected.config_path, detected.format)?;
        report.steps_run.push("schema".into());
    } else {
        report.schema_keys = count_schema_keys(root, &config);
    }

    // Step 4 — Values (per environment)
    if step_pending(&config, "values", opts.force) {
        let preset = load_preset_values(opts.from_env.as_deref())?;
        let mut total_set = 0;
        let mut total_skipped = 0;
        for env in config.environments.clone() {
            println!();
            println!("── Values · environment '{}' ──", env.name);
            let env_path = root.join(&env.env_file);
            let schema_path = root.join(&config.project.schema_path);
            let r = run_values_step_interactive(
                &schema_path,
                &env_path,
                &preset,
                opts.non_interactive,
            )?;
            total_set += r.set_count;
            total_skipped += r.skipped_count;
            if r.aborted {
                break;
            }
        }
        report.values_set = total_set;
        report.values_skipped = total_skipped;
        mark_done(&mut config, "values");
        save_project_config(&config, &detected.config_path, detected.format)?;
        report.steps_run.push("values".into());
    }

    // Step 5 — Hardening
    if step_pending(&config, "hardening", opts.force) {
        let h = step_hardening(root, &config, opts.non_interactive)?;
        report.gitignore_updated = h.gitignore_updated;
        report.ai_context_emitted = h.ai_context_emitted;
        report.fence_installed = h.fence_installed;
        report.canary_installed = h.canary_installed;
        mark_done(&mut config, "hardening");
        save_project_config(&config, &detected.config_path, detected.format)?;
        report.steps_run.push("hardening".into());
    }

    print_summary(&report);
    Ok(report)
}

// ─── Step 0 — Banner / State ────────────────────────────────

fn print_banner(non_interactive: bool) {
    println!("╔══════════════════════════════════════════╗");
    println!("║   EnvForge — Project Setup Wizard        ║");
    println!("╚══════════════════════════════════════════╝");
    if non_interactive {
        println!("(non-interactive mode — defaults only)");
    }
    println!();
}

fn print_state_matrix(root: &Path) {
    let has_cfg = detect_project_config(root).is_some();
    let has_env = root.join(".env").exists() || any_dotenv_present(root);
    let has_schema = root.join(".env.schema.toml").exists() || root.join(".env.schema").exists();
    let has_gitignore = root.join(".gitignore").exists();

    println!("Detected state in {}:", root.display());
    println!("  project config        : {}", yn(has_cfg));
    println!("  .env / .env.*         : {}", yn(has_env));
    println!("  .env.schema[.toml]    : {}", yn(has_schema));
    println!("  .gitignore            : {}", yn(has_gitignore));
    println!();
}

fn any_dotenv_present(root: &Path) -> bool {
    std::fs::read_dir(root)
        .map(|rd| {
            rd.flatten()
                .any(|e| e.file_name().to_string_lossy().starts_with(".env"))
        })
        .unwrap_or(false)
}

fn yn(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

// ─── Step 1 — Identity / Init ───────────────────────────────

fn ensure_initialized(
    root: &Path,
    opts: &WizardOptions,
) -> Result<(DetectedConfig, ProjectConfig, bool), ProjectError> {
    if let Some(detected) = detect_project_config(root) {
        let config = load_project_config(&detected)?;
        println!(
            "Project '{}' already initialized at {} — continuing setup.",
            config.project.name,
            detected.config_path.display()
        );
        println!();
        return Ok((detected, config, false));
    }

    println!("── Step 1/5: Project identity ──");

    let default_name = derive_project_name(root);
    let project_name = if opts.non_interactive {
        default_name
    } else {
        prompt_with_default("Project name", &default_name)?
    };

    let format = if opts.non_interactive {
        ConfigFormat::Toml
    } else {
        let raw = prompt_with_default("Config format (toml/yaml/json)", "toml")?;
        ConfigFormat::parse(&raw)?
    };

    let default_env = if opts.non_interactive {
        "development".to_string()
    } else {
        prompt_with_default("Initial environment name", "development")?
    };

    let env_file: PathBuf = format!(".env.{}", default_env).into();

    let init_opts = InitOptions {
        root: root.to_path_buf(),
        format,
        project_name,
        default_env_name: default_env,
        env_file_path: env_file.clone(),
        schema_path: ".env.schema.toml".into(),
        force: false,
    };
    init_project(&init_opts)?;

    println!("  Created {}", format.default_filename());
    println!("  Created {}", env_file.display());
    println!();

    let detected = detect_project_config(root).ok_or(ProjectError::ConfigNotFound)?;
    let config = load_project_config(&detected)?;
    Ok((detected, config, true))
}

// ─── Step 2 — Environments ──────────────────────────────────

fn step_environments(
    config: &mut ProjectConfig,
    non_interactive: bool,
) -> Result<(), ProjectError> {
    println!("── Step 2/5: Environments ──");
    println!("Current environments:");
    for env in &config.environments {
        let marker = if env.name == config.project.active_environment {
            "*"
        } else {
            " "
        };
        println!("  {} {}  ({})", marker, env.name, env.env_file.display());
    }
    println!();

    if non_interactive {
        return Ok(());
    }

    loop {
        let answer = prompt_with_default("Add another environment? [y/N]", "n")?;
        if !is_yes(&answer) {
            break;
        }

        let name = prompt("Environment name (e.g. staging, production)")?;
        if name.is_empty() {
            break;
        }
        if config.environments.iter().any(|e| e.name == name) {
            println!("  Environment '{}' already exists — skipped.", name);
            continue;
        }
        if !is_valid_env_name(&name) {
            println!("  Invalid name (lowercase + hyphens only).");
            continue;
        }

        let desc = prompt_optional("Description (optional)")?;
        let env_file: PathBuf = format!(".env.{}", name).into();

        config.environments.push(ProjectEnvironment {
            name: name.clone(),
            env_file: env_file.clone(),
            description: desc,
        });

        let env_path = config_root(config)?.join(&env_file);
        if !env_path.exists() {
            std::fs::write(&env_path, env_file_template(&name)).map_err(|e| {
                ProjectError::IoError {
                    path: env_path.clone(),
                    source: e,
                }
            })?;
        }
        println!("  Added '{}' → {}", name, env_file.display());
    }

    if !non_interactive && config.environments.len() > 1 {
        let names: Vec<String> = config.environments.iter().map(|e| e.name.clone()).collect();
        let prompt_msg = format!(
            "Active environment [{}] (default: {})",
            names.join("/"),
            config.project.active_environment
        );
        let chosen = prompt_with_default(&prompt_msg, &config.project.active_environment)?;
        if names.contains(&chosen) {
            config.project.active_environment = chosen;
        }
    }

    Ok(())
}

fn is_valid_env_name(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn config_root(_config: &ProjectConfig) -> Result<PathBuf, ProjectError> {
    std::env::current_dir().map_err(|e| ProjectError::IoError {
        path: PathBuf::from("."),
        source: e,
    })
}

// ─── Step 3 — Schema ────────────────────────────────────────

fn step_schema(
    root: &Path,
    config: &ProjectConfig,
    non_interactive: bool,
) -> Result<usize, ProjectError> {
    println!();
    println!("── Step 3/5: Schema ──");
    let schema_path = root.join(&config.project.schema_path);
    let env_path = active_env_path(config, root)?;
    let env_present = env_path.exists() && {
        let m = parse_dotenv_simple(&env_path)?;
        !m.is_empty()
    };

    if schema_path.exists() && non_interactive {
        return Ok(count_keys_in_schema_file(&schema_path));
    }

    if schema_path.exists() {
        println!(
            "  Existing schema found at {} ({} keys).",
            config.project.schema_path.display(),
            count_keys_in_schema_file(&schema_path)
        );
        let choice = prompt_with_default("[R]euse / [E]dit / [G]enerate fresh", "R")?;
        match choice.to_ascii_lowercase().as_str() {
            "r" | "reuse" | "" => Ok(count_keys_in_schema_file(&schema_path)),
            "g" | "gen" | "generate" => generate_from_env(&schema_path, &env_path),
            "e" | "edit" => interactive_edit_schema(&schema_path),
            _ => Ok(count_keys_in_schema_file(&schema_path)),
        }
    } else if env_present {
        println!("  No schema yet — found existing env file with values.");
        if non_interactive {
            return generate_from_env(&schema_path, &env_path);
        }
        let choice = prompt_with_default("[I]nfer from env / [B]lank-then-add-keys", "I")?;
        match choice.to_ascii_lowercase().as_str() {
            "b" | "blank" => interactive_add_keys(&schema_path, false),
            _ => generate_from_env(&schema_path, &env_path),
        }
    } else {
        println!("  No schema, no env values — starting blank.");
        if non_interactive {
            write_empty_schema(&schema_path)?;
            return Ok(0);
        }
        interactive_add_keys(&schema_path, false)
    }
}

fn generate_from_env(schema_path: &Path, env_path: &Path) -> Result<usize, ProjectError> {
    let env = if env_path.exists() {
        parse_dotenv_simple(env_path)?
    } else {
        HashMap::new()
    };
    if env.is_empty() {
        write_empty_schema(schema_path)?;
        println!("  Wrote empty schema.");
        return Ok(0);
    }
    let content = crate::ops::schema::generate_schema(&env);
    std::fs::write(schema_path, &content).map_err(|e| ProjectError::IoError {
        path: schema_path.to_path_buf(),
        source: e,
    })?;
    println!(
        "  Generated schema with {} keys (review types/required).",
        env.len()
    );
    Ok(env.len())
}

fn write_empty_schema(schema_path: &Path) -> Result<(), ProjectError> {
    std::fs::write(schema_path, SCHEMA_TEMPLATE).map_err(|e| ProjectError::IoError {
        path: schema_path.to_path_buf(),
        source: e,
    })
}

const SCHEMA_TEMPLATE: &str =
    "# .env.schema.toml — single source of truth for project ENV requirements.
#
# Each top-level `[KEY]` block defines one variable.
# Fields:
#   type        — string | number | bool | url | email | enum | regex | port
#   required    — true | false
#   description — human-readable note (shown in CLI/IDE)
#   default     — value to use when env var is missing
#   example     — placeholder shown in prompts (NEVER a real secret)
#   sensitive   — true marks the var as a secret (masked input, redacted logs)
#   values      — for type = \"enum\", array of allowed values
#   pattern     — for type = \"regex\", validation regex
#   min / max   — for type = \"number\", inclusive bounds
#
# Per-env overrides use `[KEY.environment]` sub-tables:
#   [DATABASE_URL.production]
#   pattern = \"^postgres://prod-\"
#
# ─── Examples (uncomment + adapt) ──────────────────────────
#
# [FOO]
# type = \"string\"
# required = true
# description = \"A short string ENV — example pattern\"
# example = \"BAR\"
#
# [DATABASE_URL]
# type = \"url\"
# required = true
# description = \"Primary Postgres connection string\"
# example = \"postgres://user:pass@host:5432/db\"
# sensitive = true
#
# [PORT]
# type = \"port\"
# required = false
# default = \"8080\"
#
# [LOG_LEVEL]
# type = \"enum\"
# values = [\"debug\", \"info\", \"warn\", \"error\"]
# default = \"info\"
#
# [FEATURE_FLAG_X]
# type = \"bool\"
# default = \"false\"

";

fn count_keys_in_schema_file(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .map(|c| {
            c.lines()
                .filter(|l| {
                    let t = l.trim();
                    t.starts_with('[') && t.ends_with(']') && !t.contains('.')
                })
                .count()
        })
        .unwrap_or(0)
}

fn count_schema_keys(root: &Path, config: &ProjectConfig) -> usize {
    let p = root.join(&config.project.schema_path);
    count_keys_in_schema_file(&p)
}

/// Interactive add-keys loop. If `append`, merges into existing schema; else overwrites.
fn interactive_add_keys(schema_path: &Path, append: bool) -> Result<usize, ProjectError> {
    println!();
    println!("  Add schema keys. Empty key name finishes.");
    println!("  Types: string, number, bool, url, email, enum, port");
    println!();

    let mut existing_content = if append && schema_path.exists() {
        std::fs::read_to_string(schema_path).unwrap_or_default()
    } else {
        String::new()
    };

    if existing_content.is_empty() {
        existing_content.push_str("# .env.schema\n\n");
    }

    loop {
        let key = prompt_optional("  KEY name")?;
        let Some(key) = key else { break };
        if key.is_empty() {
            break;
        }
        if !is_valid_key_name(&key) {
            println!("  Invalid key (UPPERCASE + digits + underscore).");
            continue;
        }

        let typ = prompt_with_default("    type", "string")?;
        let required = is_yes(&prompt_with_default("    required? [y/N]", "n")?);
        let desc = prompt_optional("    description")?;
        let default = prompt_optional("    default")?;
        let example = prompt_optional("    example")?;
        let sensitive = is_yes(&prompt_with_default("    sensitive? [y/N]", "n")?);
        let values = if typ.eq_ignore_ascii_case("enum") {
            prompt_optional("    values (comma-separated)")?
        } else {
            None
        };

        existing_content.push_str(&format_schema_block(&SchemaBlock {
            key: &key,
            typ: &typ,
            required,
            desc: desc.as_deref(),
            default: default.as_deref(),
            example: example.as_deref(),
            sensitive,
            values: values.as_deref(),
        }));
        println!("  + added {}", key);
        println!();
    }

    std::fs::write(schema_path, existing_content).map_err(|e| ProjectError::IoError {
        path: schema_path.to_path_buf(),
        source: e,
    })?;

    Ok(count_keys_in_schema_file(schema_path))
}

fn is_valid_key_name(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        && s.chars()
            .next()
            .map(|c| !c.is_ascii_digit())
            .unwrap_or(false)
}

struct SchemaBlock<'a> {
    key: &'a str,
    typ: &'a str,
    required: bool,
    desc: Option<&'a str>,
    default: Option<&'a str>,
    example: Option<&'a str>,
    sensitive: bool,
    values: Option<&'a str>,
}

fn format_schema_block(b: &SchemaBlock<'_>) -> String {
    let mut s = String::new();
    s.push_str(&format!("[{}]\n", b.key));
    s.push_str(&format!("type = \"{}\"\n", b.typ));
    s.push_str(&format!("required = {}\n", b.required));
    if let Some(d) = b.desc.filter(|s| !s.is_empty()) {
        s.push_str(&format!("description = \"{}\"\n", escape_toml(d)));
    }
    if let Some(d) = b.default.filter(|s| !s.is_empty()) {
        s.push_str(&format!("default = \"{}\"\n", escape_toml(d)));
    }
    if let Some(e) = b.example.filter(|s| !s.is_empty()) {
        s.push_str(&format!("example = \"{}\"\n", escape_toml(e)));
    }
    if b.sensitive {
        s.push_str("sensitive = true\n");
    }
    if b.typ.eq_ignore_ascii_case("enum") {
        if let Some(v) = b.values.filter(|s| !s.is_empty()) {
            let arr: Vec<String> = v
                .split(',')
                .map(|x| format!("\"{}\"", escape_toml(x.trim())))
                .collect();
            s.push_str(&format!("values = [{}]\n", arr.join(", ")));
        }
    }
    s.push('\n');
    s
}

fn escape_toml(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Edit existing schema: list keys, allow per-key replacement or appending new keys.
/// Replaces the matching `[KEY]` block in-place; appends new blocks at end.
fn interactive_edit_schema(schema_path: &Path) -> Result<usize, ProjectError> {
    loop {
        let keys = parse_schema_keys(schema_path)?;
        println!();
        println!("  Current schema keys:");
        for (i, (k, t, sens)) in keys.iter().enumerate() {
            let s = if *sens { " (sensitive)" } else { "" };
            println!("    {} - {} : {}{}", i + 1, k, t, s);
        }
        println!();
        println!(
            "  Choose: 1..{} = edit | 'a' = add new | 'q' = finish",
            keys.len()
        );

        let choice = prompt("  > ")?;
        let trimmed = choice.trim().to_ascii_lowercase();
        if trimmed == "q" || trimmed.is_empty() {
            break;
        }
        if trimmed == "a" {
            interactive_add_keys(schema_path, true)?;
            continue;
        }
        let Ok(idx) = trimmed.parse::<usize>() else {
            println!("  invalid choice");
            continue;
        };
        if idx == 0 || idx > keys.len() {
            println!("  out of range");
            continue;
        }
        let key = keys[idx - 1].0.clone();
        edit_one_key(schema_path, &key)?;
    }
    Ok(count_keys_in_schema_file(schema_path))
}

fn edit_one_key(schema_path: &Path, key: &str) -> Result<(), ProjectError> {
    let typ = prompt_with_default("    type", "string")?;
    let required = is_yes(&prompt_with_default("    required? [y/N]", "n")?);
    let desc = prompt_optional("    description")?;
    let default = prompt_optional("    default")?;
    let example = prompt_optional("    example")?;
    let sensitive = is_yes(&prompt_with_default("    sensitive? [y/N]", "n")?);
    let values = if typ.eq_ignore_ascii_case("enum") {
        prompt_optional("    values (comma-separated)")?
    } else {
        None
    };
    let new_block = format_schema_block(&SchemaBlock {
        key,
        typ: &typ,
        required,
        desc: desc.as_deref(),
        default: default.as_deref(),
        example: example.as_deref(),
        sensitive,
        values: values.as_deref(),
    });
    replace_schema_block(schema_path, key, &new_block)?;
    println!("  + replaced [{}]", key);
    Ok(())
}

/// Replace a `[KEY]` block (key + body up to next `[` or EOF) with `new_block`.
fn replace_schema_block(
    schema_path: &Path,
    key: &str,
    new_block: &str,
) -> Result<(), ProjectError> {
    let content = std::fs::read_to_string(schema_path).map_err(|e| ProjectError::IoError {
        path: schema_path.to_path_buf(),
        source: e,
    })?;
    let header = format!("[{}]", key);
    let mut out = String::new();
    let mut iter = content.lines().peekable();
    let mut replaced = false;
    while let Some(line) = iter.next() {
        if !replaced && line.trim() == header {
            // Skip lines until next top-level `[...]` or EOF
            while let Some(next) = iter.peek() {
                let t = next.trim();
                if t.starts_with('[') && t.ends_with(']') && !t.contains('.') {
                    break;
                }
                iter.next();
            }
            out.push_str(new_block);
            replaced = true;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !replaced {
        // Key not found — append new block
        out.push_str(new_block);
    }
    std::fs::write(schema_path, out).map_err(|e| ProjectError::IoError {
        path: schema_path.to_path_buf(),
        source: e,
    })
}

// ─── Step 4 — Values ────────────────────────────────────────

fn load_preset_values(path: Option<&Path>) -> Result<HashMap<String, String>, ProjectError> {
    let Some(p) = path else {
        return Ok(HashMap::new());
    };
    if !p.exists() {
        return Ok(HashMap::new());
    }
    parse_dotenv_simple(p)
}

/// Public — kept for backward compat with existing tests.
pub fn run_schema_step(root: &Path, config: &ProjectConfig) -> Result<usize, ProjectError> {
    let schema_path = root.join(&config.project.schema_path);
    let env_path = active_env_path(config, root)?;

    if schema_path.exists() {
        return Ok(count_keys_in_schema_file(&schema_path));
    }

    generate_from_env(&schema_path, &env_path)
}

/// Public — kept for backward compat with existing tests.
pub fn run_values_step(schema_path: &Path, env_path: &Path) -> Result<ValuesReport, ProjectError> {
    run_values_step_interactive(schema_path, env_path, &HashMap::new(), true)
}

fn run_values_step_interactive(
    schema_path: &Path,
    env_path: &Path,
    preset: &HashMap<String, String>,
    non_interactive: bool,
) -> Result<ValuesReport, ProjectError> {
    let keys = if schema_path.exists() {
        parse_schema_keys(schema_path)?
    } else {
        Vec::new()
    };

    // Full schema needed for default values on `d` keystroke (parse_schema_keys drops them).
    let full_schema = if schema_path.exists() {
        crate::ops::schema::parse_schema(schema_path).ok()
    } else {
        None
    };
    let defaults: HashMap<String, String> = full_schema
        .map(|s| {
            s.variables
                .into_iter()
                .filter_map(|(k, v)| v.default.map(|d| (k, d)))
                .collect()
        })
        .unwrap_or_default();

    if keys.is_empty() {
        println!("  No schema keys to prompt.");
        return Ok(ValuesReport::default());
    }

    let existing = if env_path.exists() {
        parse_dotenv_simple(env_path)?
    } else {
        HashMap::new()
    };

    let mut final_env = existing.clone();
    let mut set_count = 0;
    let mut skipped_count = 0;
    let mut aborted = false;

    if non_interactive {
        // Precedence: preset > existing > schema default.
        for (key, _typ, _sensitive) in &keys {
            if let Some(v) = preset.get(key) {
                final_env.insert(key.clone(), v.clone());
                set_count += 1;
            } else if existing.contains_key(key) {
                // Keep what's there.
                skipped_count += 1;
            } else if let Some(d) = defaults.get(key) {
                final_env.insert(key.clone(), d.clone());
                set_count += 1;
            } else {
                skipped_count += 1;
            }
        }
        write_env(env_path, &final_env)?;
        return Ok(ValuesReport {
            set_count,
            skipped_count,
            aborted: false,
        });
    }

    println!(
        "  Keys: Enter=keep | <value>=set | 'd'=default | 'c'=clear | 's'=skip | 'q'=quit env | 'a'=abort all"
    );
    println!();

    for (key, typ, sensitive) in &keys {
        if aborted {
            break;
        }
        let current = existing.get(key).cloned();
        let preset_v = preset.get(key).cloned();
        let display_current = match (&current, *sensitive) {
            (Some(_), true) => Some("****".to_string()),
            (Some(v), false) => Some(v.clone()),
            (None, _) => None,
        };

        let hint = preset_v
            .as_ref()
            .map(|v| format!(" preset=\"{}\"", if *sensitive { "****" } else { v }))
            .unwrap_or_default();
        let cur_hint = display_current
            .as_ref()
            .map(|v| format!(" [current: {}]", v))
            .unwrap_or_default();

        println!("  {} ({}){}{}", key, typ, cur_hint, hint);

        let input = if *sensitive {
            read_sensitive("    > ")?
        } else {
            prompt("    >")?
        };

        match input.trim() {
            "" => {
                if let Some(p) = preset_v.clone() {
                    final_env.insert(key.clone(), p);
                    set_count += 1;
                } else {
                    skipped_count += 1;
                }
            }
            "s" | "skip" => {
                skipped_count += 1;
            }
            "c" | "clear" => {
                final_env.remove(key);
                skipped_count += 1;
            }
            "d" | "default" => {
                if let Some(d) = defaults.get(key) {
                    final_env.insert(key.clone(), d.clone());
                    set_count += 1;
                    println!("    (using default: {})", d);
                } else {
                    println!("    (no schema default for {} — skipped)", key);
                    skipped_count += 1;
                }
            }
            "q" | "quit" => {
                println!("  (quit this environment)");
                break;
            }
            "a" | "abort" => {
                println!("  (abort all environments)");
                aborted = true;
                break;
            }
            other => {
                final_env.insert(key.clone(), other.to_string());
                set_count += 1;
            }
        }
    }

    write_env(env_path, &final_env)?;
    println!();
    println!("  {} set, {} skipped", set_count, skipped_count);

    Ok(ValuesReport {
        set_count,
        skipped_count,
        aborted,
    })
}

fn write_env(env_path: &Path, env: &HashMap<String, String>) -> Result<(), ProjectError> {
    let mut out = String::new();
    out.push_str("# EnvForge project environment\n");
    let mut keys: Vec<&String> = env.keys().collect();
    keys.sort();
    for k in keys {
        let v = &env[k];
        if v.contains(' ') || v.contains('"') || v.contains('\'') || v.is_empty() {
            out.push_str(&format!("{}=\"{}\"\n", k, v.replace('"', "\\\"")));
        } else {
            out.push_str(&format!("{}={}\n", k, v));
        }
    }
    std::fs::write(env_path, &out).map_err(|e| ProjectError::IoError {
        path: env_path.to_path_buf(),
        source: e,
    })
}

// ─── Step 5 — Hardening ─────────────────────────────────────

#[derive(Default)]
struct HardenReport {
    gitignore_updated: bool,
    ai_context_emitted: bool,
    fence_installed: bool,
    canary_installed: bool,
}

fn step_hardening(
    root: &Path,
    config: &ProjectConfig,
    non_interactive: bool,
) -> Result<HardenReport, ProjectError> {
    println!();
    println!("── Step 5/5: Hardening ──");

    let mut r = HardenReport::default();

    let want_gitignore = if non_interactive {
        true
    } else {
        is_yes(&prompt_with_default(
            "Append .env* patterns to .gitignore? [Y/n]",
            "y",
        )?)
    };
    if want_gitignore {
        r.gitignore_updated = add_to_gitignore(root)?;
        println!(
            "  .gitignore {}",
            if r.gitignore_updated {
                "updated"
            } else {
                "already current"
            }
        );
    }

    let want_ai = if non_interactive {
        false
    } else {
        is_yes(&prompt_with_default(
            "Emit AI-safe context (.env.ai.md)? [y/N]",
            "n",
        )?)
    };
    if want_ai {
        let schema_path = root.join(&config.project.schema_path);
        if schema_path.exists() {
            let schema = crate::ops::schema::parse_schema(&schema_path).map_err(|e| {
                ProjectError::ParseError {
                    path: schema_path.clone(),
                    details: e.to_string(),
                }
            })?;
            let docs = crate::ops::schema::generate_docs(&schema);
            let out = root.join(".env.ai.md");
            std::fs::write(&out, docs).map_err(|e| ProjectError::IoError {
                path: out.clone(),
                source: e,
            })?;
            r.ai_context_emitted = true;
            println!("  Wrote {}", out.display());
        } else {
            println!("  No schema to derive AI context from — skipped.");
        }
    }

    let want_fence = if non_interactive {
        false
    } else {
        is_yes(&prompt_with_default(
            "Install AI fence (choose tools)? [y/N]",
            "n",
        )?)
    };
    if want_fence {
        let chosen = crate::ops::fence::select_targets_interactive(root).unwrap_or_default();
        if chosen.is_empty() {
            println!("  No tools selected — fence skipped.");
        } else {
            match crate::ops::fence::create_fence_for(root, &chosen, false) {
                Ok(_) => {
                    r.fence_installed = true;
                    let names: Vec<&str> = chosen.iter().map(|t| t.as_str()).collect();
                    println!("  Fence installed: {}", names.join(", "));
                }
                Err(e) => println!("  Fence install skipped: {}", e),
            }
        }
    }

    let want_canary = if non_interactive {
        false
    } else {
        is_yes(&prompt_with_default(
            "Mint a canary token + append to active env file? [y/N]",
            "n",
        )?)
    };
    if want_canary {
        match crate::ops::canary::create_canary("ENVFORGE_CANARY", "api_token") {
            Ok(secret) => {
                let env_path = active_env_path(config, root)?;
                let line = format!(
                    "\n# canary — alerts if read by AI\nENVFORGE_CANARY={}\n",
                    secret.fake_value
                );
                if let Err(e) = std::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(&env_path)
                    .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()))
                {
                    println!("  Canary minted but env-file append failed: {}", e);
                } else {
                    r.canary_installed = true;
                    println!("  Canary installed in {}", env_path.display());
                }
            }
            Err(e) => println!("  Canary install skipped: {}", e),
        }
    }

    Ok(r)
}

// ─── Step 6 — Summary ───────────────────────────────────────

fn print_summary(report: &WizardReport) {
    println!();
    println!("╔══════════════════════════════════════════╗");
    println!("║   Wizard complete                        ║");
    println!("╚══════════════════════════════════════════╝");
    println!("  Project       : {}", report.project_name);
    if let Some(f) = report.format {
        println!("  Config format : {:?}", f);
    }
    println!("  Environments  : {}", report.environments.join(", "));
    println!("  Schema keys   : {}", report.schema_keys);
    println!(
        "  Values        : {} set, {} skipped",
        report.values_set, report.values_skipped
    );
    if report.gitignore_updated {
        println!("  .gitignore    : updated");
    }
    if report.ai_context_emitted {
        println!("  AI context    : emitted");
    }
    if report.fence_installed {
        println!("  Fence         : installed");
    }
    if report.canary_installed {
        println!("  Canary        : installed");
    }
    println!("  Steps run     : {}", report.steps_run.join(", "));
    println!();
    println!("Next steps:");
    println!("  envforge project status");
    println!("  envforge project validate");
    println!("  envforge project scan");
    println!("  envforge project env list");
    println!();
}

// ─── Resume / Step Tracking ─────────────────────────────────

fn step_pending(config: &ProjectConfig, step: &str, force: bool) -> bool {
    if force {
        return true;
    }
    !is_step_complete(config, step)
}

fn is_step_complete(config: &ProjectConfig, step: &str) -> bool {
    let matches = |c: &String| -> bool {
        if c == step {
            return true;
        }
        // "init" is legacy alias for "identity".
        step == "identity" && c == "init"
    };
    config.wizard.completed_steps.iter().any(matches)
}

fn mark_done(config: &mut ProjectConfig, step: &str) {
    if !is_step_complete(config, step) {
        config.wizard.completed_steps.push(step.into());
    }
}

fn all_steps_done(config: &ProjectConfig) -> bool {
    const STEPS: &[&str] = &["identity", "environments", "schema", "values", "hardening"];
    STEPS.iter().all(|s| is_step_complete(config, s))
}

fn dry_run_walk(root: &Path, opts: &WizardOptions) -> Result<WizardReport, ProjectError> {
    println!("── DRY-RUN — planned actions ──");
    match detect_project_config(root) {
        None => {
            println!(
                "  step 1 identity      : would init project at {}",
                root.display()
            );
            println!("  step 2 environments  : would create 'development' env");
            println!("  step 3 schema        : would write .env.schema (empty or inferred)");
            println!("  step 4 values        : would prompt per schema key");
            println!("  step 5 hardening     : would prompt y/N for gitignore/ai/fence/canary");
        }
        Some(detected) => {
            let config = load_project_config(&detected)?;
            let pending = |s: &str| {
                if step_pending(&config, s, opts.force || opts.reset) {
                    "PENDING"
                } else {
                    "done"
                }
            };
            println!("  step 1 identity      : {}", pending("identity"));
            println!("  step 2 environments  : {}", pending("environments"));
            println!("  step 3 schema        : {}", pending("schema"));
            println!("  step 4 values        : {}", pending("values"));
            println!("  step 5 hardening     : {}", pending("hardening"));
        }
    }
    println!();
    println!("(no writes performed)");
    Ok(WizardReport::default())
}

// ─── I/O Helpers ────────────────────────────────────────────

fn prompt(label: &str) -> Result<String, ProjectError> {
    print!("{}: ", label);
    io::stdout().flush().ok();
    let mut input = String::new();
    io::stdin()
        .lock()
        .read_line(&mut input)
        .map_err(|e| ProjectError::IoError {
            path: PathBuf::from("<stdin>"),
            source: e,
        })?;
    Ok(input.trim().to_string())
}

fn prompt_with_default(label: &str, default: &str) -> Result<String, ProjectError> {
    print!("{} [{}]: ", label, default);
    io::stdout().flush().ok();
    let mut input = String::new();
    io::stdin()
        .lock()
        .read_line(&mut input)
        .map_err(|e| ProjectError::IoError {
            path: PathBuf::from("<stdin>"),
            source: e,
        })?;
    let trimmed = input.trim();
    Ok(if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    })
}

fn prompt_optional(label: &str) -> Result<Option<String>, ProjectError> {
    print!("{} (Enter to skip): ", label);
    io::stdout().flush().ok();
    let mut input = String::new();
    io::stdin()
        .lock()
        .read_line(&mut input)
        .map_err(|e| ProjectError::IoError {
            path: PathBuf::from("<stdin>"),
            source: e,
        })?;
    let trimmed = input.trim();
    Ok(if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    })
}

fn read_sensitive(label: &str) -> Result<String, ProjectError> {
    rpassword::prompt_password(label).map_err(|e| ProjectError::IoError {
        path: PathBuf::from("<stdin>"),
        source: e,
    })
}

fn is_yes(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "y" | "yes" | "1" | "true"
    )
}

// ─── Public Helpers (kept for compat) ───────────────────────

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

        if trimmed.starts_with('[') && trimmed.ends_with(']') && !trimmed.contains('.') {
            if let Some(ref key) = current_key {
                keys.push((key.clone(), current_type.clone(), current_sensitive));
            }
            current_key = Some(trimmed[1..trimmed.len() - 1].to_string());
            current_type = "string".to_string();
            current_sensitive = false;
        } else if current_key.is_some() {
            if let Some(val) = trimmed.strip_prefix("type = ") {
                current_type = val.trim_matches('"').to_string();
            } else if trimmed == "sensitive = true" {
                current_sensitive = true;
            }
        }
    }

    if let Some(key) = current_key {
        keys.push((key, current_type, current_sensitive));
    }

    Ok(keys)
}
