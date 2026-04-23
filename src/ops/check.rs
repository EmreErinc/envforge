use std::collections::HashMap;
use std::path::Path;

use crate::config::load_or_create_default;

use super::OpError;
use crate::ops::doctor::{self, CheckStatus as DoctorStatus};
use crate::ops::listing::{EntryLocation, EnvEntry};
use crate::ops::scanner::filter_sensitive;
use crate::ops::schema::{
    detect_drift, find_schema, parse_schema, validate_against_schema, DriftStatus,
};
use crate::ops::secrets::age::{get_age_report, load_sources};
use crate::parser::parse_shell_file;

// ─── Types ──────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum CheckCategory {
    Doctor,
    Validate,
    Scan,
    Age,
    Drift,
}

impl CheckCategory {
    pub fn name(&self) -> &str {
        match self {
            Self::Doctor => "doctor",
            Self::Validate => "validate",
            Self::Scan => "scan",
            Self::Age => "age",
            Self::Drift => "drift",
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Self::Doctor => "Doctor",
            Self::Validate => "Validate",
            Self::Scan => "Scan",
            Self::Age => "Age",
            Self::Drift => "Drift",
        }
    }

    pub fn all() -> Vec<CheckCategory> {
        vec![
            Self::Doctor,
            Self::Validate,
            Self::Scan,
            Self::Age,
            Self::Drift,
        ]
    }

    pub fn parse(s: &str) -> Option<CheckCategory> {
        match s.to_lowercase().as_str() {
            "doctor" => Some(Self::Doctor),
            "validate" => Some(Self::Validate),
            "scan" => Some(Self::Scan),
            "age" => Some(Self::Age),
            "drift" => Some(Self::Drift),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CheckStatus {
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub category: CheckCategory,
    pub status: CheckStatus,
    pub message: String,
    pub hint: Option<String>,
}

#[derive(Debug)]
pub struct CheckReport {
    pub results: Vec<CheckResult>,
    pub skipped: Vec<(CheckCategory, String)>,
}

impl CheckReport {
    pub fn ok_count(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.status == CheckStatus::Ok)
            .count()
    }

    pub fn warning_count(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.status == CheckStatus::Warning)
            .count()
    }

    pub fn error_count(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.status == CheckStatus::Error)
            .count()
    }

    pub fn has_errors(&self) -> bool {
        self.error_count() > 0
    }
}

// ─── Parse --only flag ──────────────────────────────────────

pub fn parse_category_filter(input: &str) -> Result<Vec<CheckCategory>, String> {
    let mut cats = Vec::new();
    for part in input.split(',') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        match CheckCategory::parse(trimmed) {
            Some(c) => cats.push(c),
            None => {
                return Err(format!(
                    "Unknown category '{}'. Available: doctor, validate, scan, age, drift",
                    trimmed
                ))
            }
        }
    }
    if cats.is_empty() {
        return Err("No valid categories specified".into());
    }
    Ok(cats)
}

// ─── Core Runner ────────────────────────────────────────────

pub fn run_checks(only: Option<&[CheckCategory]>) -> CheckReport {
    let categories = match only {
        Some(cats) => cats.to_vec(),
        None => CheckCategory::all(),
    };

    let mut results = Vec::new();
    let mut skipped = Vec::new();

    for cat in &categories {
        match check_prerequisites(cat) {
            Ok(()) => {
                let mut cat_results = run_category(cat);
                results.append(&mut cat_results);
            }
            Err(reason) => {
                skipped.push((cat.clone(), reason));
            }
        }
    }

    CheckReport { results, skipped }
}

// ─── Prerequisites ──────────────────────────────────────────

fn check_prerequisites(cat: &CheckCategory) -> Result<(), String> {
    match cat {
        CheckCategory::Doctor => Ok(()),
        CheckCategory::Validate => {
            if find_schema().is_some() {
                Ok(())
            } else {
                Err("No .env.schema found".into())
            }
        }
        CheckCategory::Scan => {
            // Always attempt scan — graceful if no sensitive entries
            Ok(())
        }
        CheckCategory::Age => match load_sources() {
            Ok(sources) if !sources.secrets.is_empty() => Ok(()),
            _ => Err("No tracked secrets. Pull secrets to start tracking.".into()),
        },
        CheckCategory::Drift => {
            if find_schema().is_none() {
                return Err("No .env.schema found for drift detection".into());
            }
            // Check for .env.* files in cwd
            let cwd = std::env::current_dir().unwrap_or_default();
            let has_env_files = std::fs::read_dir(&cwd)
                .map(|entries| {
                    entries.filter_map(|e| e.ok()).any(|e| {
                        let name = e.file_name().to_string_lossy().to_string();
                        name.starts_with(".env.")
                            && name != ".env.schema"
                            && name != ".envforgeignore"
                    })
                })
                .unwrap_or(false);
            if has_env_files {
                Ok(())
            } else {
                Err("No .env.* files found for drift comparison".into())
            }
        }
    }
}

// ─── Category Runners ───────────────────────────────────────

fn run_category(cat: &CheckCategory) -> Vec<CheckResult> {
    match cat {
        CheckCategory::Doctor => run_doctor_checks(),
        CheckCategory::Validate => run_validate_checks(),
        CheckCategory::Scan => run_scan_checks(),
        CheckCategory::Age => run_age_checks(),
        CheckCategory::Drift => run_drift_checks(),
    }
}

fn run_doctor_checks() -> Vec<CheckResult> {
    let report = doctor::run_doctor();
    report
        .checks
        .iter()
        .map(|hc| CheckResult {
            category: CheckCategory::Doctor,
            status: match hc.status {
                DoctorStatus::Ok => CheckStatus::Ok,
                DoctorStatus::Warning => CheckStatus::Warning,
                DoctorStatus::Error => CheckStatus::Error,
            },
            message: format!("{} — {}", hc.name, hc.message),
            hint: hc.hint.clone(),
        })
        .collect()
}

fn run_validate_checks() -> Vec<CheckResult> {
    let schema_path = match find_schema() {
        Some(p) => p,
        None => return vec![],
    };

    let schema = match parse_schema(&schema_path) {
        Ok(s) => s,
        Err(e) => {
            return vec![CheckResult {
                category: CheckCategory::Validate,
                status: CheckStatus::Error,
                message: format!("Schema parse error: {}", e),
                hint: Some("Check .env.schema syntax".into()),
            }];
        }
    };

    // Load env from shell files
    let env: HashMap<String, String> = match load_env_map() {
        Ok(e) => e,
        Err(e) => {
            return vec![CheckResult {
                category: CheckCategory::Validate,
                status: CheckStatus::Error,
                message: format!("Cannot load env: {}", e),
                hint: None,
            }];
        }
    };

    let config = load_or_create_default().unwrap_or_default();
    let errors = validate_against_schema(&env, &schema, None, &config.validation);

    if errors.is_empty() {
        vec![CheckResult {
            category: CheckCategory::Validate,
            status: CheckStatus::Ok,
            message: format!("Schema validation passed ({} variables)", env.len()),
            hint: None,
        }]
    } else {
        let mut results = Vec::new();
        for e in &errors {
            results.push(CheckResult {
                category: CheckCategory::Validate,
                status: CheckStatus::Error,
                message: format!("{} — {}", e.key, e.message),
                hint: Some(format!("Run: envforge set {}=<valid_value>", e.key)),
            });
        }
        results
    }
}

fn run_scan_checks() -> Vec<CheckResult> {
    let entries = match load_entries() {
        Ok(e) => e,
        Err(_) => return vec![],
    };

    let sensitive = filter_sensitive(&entries);
    if sensitive.is_empty() {
        return vec![CheckResult {
            category: CheckCategory::Scan,
            status: CheckStatus::Ok,
            message: "No sensitive values to scan for".into(),
            hint: None,
        }];
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    match crate::ops::scanner::scan_directory(&cwd, &sensitive) {
        Ok(matches) => {
            if matches.is_empty() {
                vec![CheckResult {
                    category: CheckCategory::Scan,
                    status: CheckStatus::Ok,
                    message: format!(
                        "No secrets found in source ({} sensitive keys checked)",
                        sensitive.len()
                    ),
                    hint: None,
                }]
            } else {
                vec![CheckResult {
                    category: CheckCategory::Scan,
                    status: CheckStatus::Error,
                    message: format!(
                        "{} secret(s) found in {} location(s)",
                        sensitive.len(),
                        matches.len()
                    ),
                    hint: Some("Run: envforge scan to see details".into()),
                }]
            }
        }
        Err(e) => vec![CheckResult {
            category: CheckCategory::Scan,
            status: CheckStatus::Warning,
            message: format!("Scan error: {}", e),
            hint: None,
        }],
    }
}

fn run_age_checks() -> Vec<CheckResult> {
    match get_age_report(90) {
        Ok(entries) => {
            let stale_count = entries.iter().filter(|e| e.stale).count();
            if stale_count == 0 {
                vec![CheckResult {
                    category: CheckCategory::Age,
                    status: CheckStatus::Ok,
                    message: format!("All {} secrets within 90-day threshold", entries.len()),
                    hint: None,
                }]
            } else {
                let stale_keys: Vec<String> = entries
                    .iter()
                    .filter(|e| e.stale)
                    .take(5)
                    .map(|e| e.key.clone())
                    .collect();
                let suffix = if stale_count > 5 {
                    format!(" (+{} more)", stale_count - 5)
                } else {
                    String::new()
                };
                vec![CheckResult {
                    category: CheckCategory::Age,
                    status: CheckStatus::Warning,
                    message: format!(
                        "{} secret(s) older than 90 days: {}{}",
                        stale_count,
                        stale_keys.join(", "),
                        suffix
                    ),
                    hint: Some("Run: envforge secrets age --stale-only to review".into()),
                }]
            }
        }
        Err(e) => vec![CheckResult {
            category: CheckCategory::Age,
            status: CheckStatus::Warning,
            message: format!("Age check error: {}", e),
            hint: None,
        }],
    }
}

fn run_drift_checks() -> Vec<CheckResult> {
    let schema_path = match find_schema() {
        Some(p) => p,
        None => return vec![],
    };

    let schema = match parse_schema(&schema_path) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    // Find .env.* files in cwd
    let cwd = std::env::current_dir().unwrap_or_default();
    let env_files: Vec<String> = std::fs::read_dir(&cwd)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.starts_with(".env.")
                        && name != ".env.schema"
                        && name != ".envforgeignore"
                        && name != ".env.example"
                    {
                        Some(name)
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    if env_files.is_empty() {
        return vec![];
    }

    // Load env files
    let mut envs: Vec<(String, HashMap<String, String>)> = Vec::new();
    for path in &env_files {
        if let Ok(entries) = crate::ops::dotenv::parse_dotenv(Path::new(path)) {
            let map = entries.into_iter().map(|e| (e.key, e.value)).collect();
            envs.push((path.clone(), map));
        }
    }

    let drift = detect_drift(&envs, Some(&schema));
    let differ_count = drift
        .iter()
        .filter(|d| d.status == DriftStatus::Differs)
        .count();
    let missing_count = drift
        .iter()
        .filter(|d| d.status == DriftStatus::Missing)
        .count();

    if differ_count == 0 && missing_count == 0 {
        vec![CheckResult {
            category: CheckCategory::Drift,
            status: CheckStatus::Ok,
            message: format!("No drift across {} env files", env_files.len()),
            hint: None,
        }]
    } else {
        vec![CheckResult {
            category: CheckCategory::Drift,
            status: CheckStatus::Warning,
            message: format!(
                "{} differ, {} missing across {} env files",
                differ_count,
                missing_count,
                env_files.len()
            ),
            hint: Some(format!(
                "Run: envforge drift --envs {}",
                env_files.join(" --envs ")
            )),
        }]
    }
}

// ─── Output Formatting ─────────────────────────────────────

pub fn print_report(report: &CheckReport) {
    let categories = CheckCategory::all();

    for cat in &categories {
        let cat_results: Vec<&CheckResult> = report
            .results
            .iter()
            .filter(|r| r.category == *cat)
            .collect();

        let is_skipped = report.skipped.iter().any(|(c, _)| c == cat);

        if cat_results.is_empty() && !is_skipped {
            continue;
        }

        if is_skipped {
            let reason = report
                .skipped
                .iter()
                .find(|(c, _)| c == cat)
                .map(|(_, r)| r.as_str())
                .unwrap_or("unknown");
            println!(
                "\n\x1b[90m── {} (skipped) ──────────────────────────────────\x1b[0m",
                cat.display_name()
            );
            println!("  \x1b[90m{}\x1b[0m", reason);
            continue;
        }

        println!(
            "\n── {} ──────────────────────────────────────────",
            cat.display_name()
        );

        for r in &cat_results {
            let icon = match r.status {
                CheckStatus::Ok => "\x1b[32m✓\x1b[0m",
                CheckStatus::Warning => "\x1b[33m⚠\x1b[0m",
                CheckStatus::Error => "\x1b[31m✗\x1b[0m",
            };
            println!("{} {}", icon, r.message);
            if let Some(hint) = &r.hint {
                println!("  \x1b[36m→ {}\x1b[0m", hint);
            }
        }
    }

    // Summary
    let total = report.results.len();
    let cats_run = CheckCategory::all()
        .iter()
        .filter(|c| report.results.iter().any(|r| r.category == **c))
        .count();

    println!();
    println!("════════════════════════════════════════════════════");
    println!(
        "  {} categories: {} run, {} skipped",
        cats_run + report.skipped.len(),
        cats_run,
        report.skipped.len()
    );
    println!(
        "  {} checks: {} ok, {} warning(s), {} error(s)",
        total,
        report.ok_count(),
        report.warning_count(),
        report.error_count()
    );
}

pub fn report_to_json(report: &CheckReport) -> serde_json::Value {
    let results: Vec<serde_json::Value> = report
        .results
        .iter()
        .map(|r| {
            serde_json::json!({
                "category": r.category.name(),
                "status": match r.status {
                    CheckStatus::Ok => "ok",
                    CheckStatus::Warning => "warning",
                    CheckStatus::Error => "error",
                },
                "message": r.message,
                "hint": r.hint,
            })
        })
        .collect();

    let skipped: Vec<serde_json::Value> = report
        .skipped
        .iter()
        .map(|(c, reason)| {
            serde_json::json!({
                "category": c.name(),
                "reason": reason,
            })
        })
        .collect();

    serde_json::json!({
        "categories_run": results.iter().map(|r| r["category"].as_str().unwrap_or("")).collect::<std::collections::HashSet<_>>().len(),
        "categories_skipped": skipped,
        "results": results,
        "summary": {
            "total": report.results.len(),
            "ok": report.ok_count(),
            "warnings": report.warning_count(),
            "errors": report.error_count(),
        }
    })
}

// ─── Helpers ────────────────────────────────────────────────

fn load_env_map() -> Result<HashMap<String, String>, OpError> {
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
    let entries = crate::ops::collect_all_entries(&shell_files);
    Ok(entries
        .into_iter()
        .filter(|e| e.location != EntryLocation::Commented)
        .map(|e| (e.key, e.value))
        .collect())
}

fn load_entries() -> Result<Vec<EnvEntry>, OpError> {
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
    Ok(crate::ops::collect_all_entries(&shell_files))
}

fn shellexpand(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    std::path::PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_parse() {
        assert_eq!(CheckCategory::parse("doctor"), Some(CheckCategory::Doctor));
        assert_eq!(
            CheckCategory::parse("VALIDATE"),
            Some(CheckCategory::Validate)
        );
        assert_eq!(CheckCategory::parse("scan"), Some(CheckCategory::Scan));
        assert_eq!(CheckCategory::parse("age"), Some(CheckCategory::Age));
        assert_eq!(CheckCategory::parse("drift"), Some(CheckCategory::Drift));
        assert_eq!(CheckCategory::parse("unknown"), None);
    }

    #[test]
    fn test_category_all() {
        assert_eq!(CheckCategory::all().len(), 5);
    }

    #[test]
    fn test_parse_category_filter() {
        let cats = parse_category_filter("doctor,scan").unwrap();
        assert_eq!(cats.len(), 2);
        assert_eq!(cats[0], CheckCategory::Doctor);
        assert_eq!(cats[1], CheckCategory::Scan);
    }

    #[test]
    fn test_parse_category_filter_spaces() {
        let cats = parse_category_filter("doctor, scan, age").unwrap();
        assert_eq!(cats.len(), 3);
    }

    #[test]
    fn test_parse_category_filter_invalid() {
        assert!(parse_category_filter("doctor,invalid").is_err());
    }

    #[test]
    fn test_parse_category_filter_empty() {
        assert!(parse_category_filter("").is_err());
    }

    #[test]
    fn test_report_counts() {
        let report = CheckReport {
            results: vec![
                CheckResult {
                    category: CheckCategory::Doctor,
                    status: CheckStatus::Ok,
                    message: "good".into(),
                    hint: None,
                },
                CheckResult {
                    category: CheckCategory::Doctor,
                    status: CheckStatus::Warning,
                    message: "warn".into(),
                    hint: Some("fix".into()),
                },
                CheckResult {
                    category: CheckCategory::Scan,
                    status: CheckStatus::Error,
                    message: "bad".into(),
                    hint: Some("fix".into()),
                },
            ],
            skipped: vec![(CheckCategory::Age, "no data".into())],
        };
        assert_eq!(report.ok_count(), 1);
        assert_eq!(report.warning_count(), 1);
        assert_eq!(report.error_count(), 1);
        assert!(report.has_errors());
    }

    #[test]
    fn test_report_to_json() {
        let report = CheckReport {
            results: vec![CheckResult {
                category: CheckCategory::Doctor,
                status: CheckStatus::Ok,
                message: "Config loaded".into(),
                hint: None,
            }],
            skipped: vec![(CheckCategory::Drift, "no schema".into())],
        };
        let json = report_to_json(&report);
        assert_eq!(json["summary"]["total"], 1);
        assert_eq!(json["summary"]["ok"], 1);
        assert_eq!(json["categories_skipped"][0]["category"], "drift");
    }

    #[test]
    fn test_category_name() {
        assert_eq!(CheckCategory::Doctor.name(), "doctor");
        assert_eq!(CheckCategory::Validate.name(), "validate");
        assert_eq!(CheckCategory::Scan.name(), "scan");
        assert_eq!(CheckCategory::Age.name(), "age");
        assert_eq!(CheckCategory::Drift.name(), "drift");
    }

    #[test]
    fn test_category_display_name() {
        assert_eq!(CheckCategory::Doctor.display_name(), "Doctor");
        assert_eq!(CheckCategory::Validate.display_name(), "Validate");
        assert_eq!(CheckCategory::Scan.display_name(), "Scan");
        assert_eq!(CheckCategory::Age.display_name(), "Age");
        assert_eq!(CheckCategory::Drift.display_name(), "Drift");
    }

    #[test]
    fn test_report_has_errors_false() {
        let report = CheckReport {
            results: vec![
                CheckResult {
                    category: CheckCategory::Doctor,
                    status: CheckStatus::Ok,
                    message: "good".into(),
                    hint: None,
                },
                CheckResult {
                    category: CheckCategory::Scan,
                    status: CheckStatus::Warning,
                    message: "meh".into(),
                    hint: None,
                },
            ],
            skipped: vec![],
        };
        assert!(!report.has_errors());
        assert_eq!(report.ok_count(), 1);
        assert_eq!(report.warning_count(), 1);
        assert_eq!(report.error_count(), 0);
    }

    #[test]
    fn test_parse_category_filter_single() {
        let cats = parse_category_filter("doctor").unwrap();
        assert_eq!(cats.len(), 1);
        assert_eq!(cats[0], CheckCategory::Doctor);
    }

    #[test]
    fn test_report_to_json_mixed() {
        let report = CheckReport {
            results: vec![
                CheckResult {
                    category: CheckCategory::Doctor,
                    status: CheckStatus::Ok,
                    message: "ok".into(),
                    hint: None,
                },
                CheckResult {
                    category: CheckCategory::Doctor,
                    status: CheckStatus::Error,
                    message: "bad".into(),
                    hint: Some("fix it".into()),
                },
                CheckResult {
                    category: CheckCategory::Scan,
                    status: CheckStatus::Warning,
                    message: "warn".into(),
                    hint: None,
                },
            ],
            skipped: vec![],
        };
        let json = report_to_json(&report);
        assert_eq!(json["summary"]["total"], 3);
        assert_eq!(json["summary"]["ok"], 1);
        assert_eq!(json["summary"]["warnings"], 1);
        assert_eq!(json["summary"]["errors"], 1);
        assert!(json["results"].as_array().unwrap().len() == 3);
    }

    #[test]
    fn test_run_checks_doctor_only() {
        let report = run_checks(Some(&[CheckCategory::Doctor]));
        // Doctor always runs (no prerequisites)
        assert!(!report.results.is_empty());
        assert!(report
            .results
            .iter()
            .all(|r| r.category == CheckCategory::Doctor));
    }

    #[test]
    fn test_check_result_fields() {
        let result = CheckResult {
            category: CheckCategory::Scan,
            status: CheckStatus::Warning,
            message: "test".into(),
            hint: Some("hint".into()),
        };
        assert_eq!(result.message, "test");
        assert_eq!(result.hint.as_deref(), Some("hint"));
        assert_eq!(result.category, CheckCategory::Scan);
        assert_eq!(result.status, CheckStatus::Warning);
    }
}
