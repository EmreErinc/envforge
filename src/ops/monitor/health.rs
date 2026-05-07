//! Health probes for secret infrastructure verification.
//!
//! Checks provider availability, canary integrity, fence status,
//! and encryption key accessibility. All probes are non-blocking and
//! return a [`HealthResult`].

use std::path::Path;
use std::time::Instant;

use super::{HealthResult, HealthStatus};

/// Run all health probes and return results.
pub fn run_all_checks() -> Vec<HealthResult> {
    vec![
        check_providers(),
        check_canary(),
        check_encryption(),
        check_fence(),
    ]
}

// ─── Provider Health ────────────────────────────────────────────

fn check_providers() -> HealthResult {
    let start = Instant::now();

    let registry = crate::ops::secrets::providers::create_default_registry();
    let count = registry.len();

    let latency = start.elapsed().as_millis() as u64;

    if count == 0 {
        HealthResult {
            name: "providers".into(),
            category: "provider".into(),
            status: HealthStatus::Degraded,
            message: "No secret providers registered".into(),
            latency_ms: Some(latency),
        }
    } else if count < 7 {
        // Extended providers not all loaded
        HealthResult {
            name: "providers".into(),
            category: "provider".into(),
            status: HealthStatus::Healthy,
            message: format!("{} provider(s) registered (7+ recommended)", count),
            latency_ms: Some(latency),
        }
    } else {
        HealthResult {
            name: "providers".into(),
            category: "provider".into(),
            status: HealthStatus::Healthy,
            message: format!("{} provider(s) registered", count),
            latency_ms: Some(latency),
        }
    }
}

// ─── Canary Health ──────────────────────────────────────────────

fn check_canary() -> HealthResult {
    let start = Instant::now();

    match crate::ops::canary::list_canaries() {
        Ok(canaries) => {
            let triggered: Vec<_> = canaries.iter().filter(|c| c.triggered).collect();
            if canaries.is_empty() {
                HealthResult {
                    name: "canary".into(),
                    category: "security".into(),
                    status: HealthStatus::Degraded,
                    message: "No canary tokens deployed".into(),
                    latency_ms: Some(start.elapsed().as_millis() as u64),
                }
            } else if !triggered.is_empty() {
                HealthResult {
                    name: "canary".into(),
                    category: "security".into(),
                    status: HealthStatus::Failed,
                    message: format!(
                        "{} canaries deployed, {} TRIGGERED: {}",
                        canaries.len(),
                        triggered.len(),
                        triggered
                            .iter()
                            .map(|c| c.key.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    latency_ms: Some(start.elapsed().as_millis() as u64),
                }
            } else {
                HealthResult {
                    name: "canary".into(),
                    category: "security".into(),
                    status: HealthStatus::Healthy,
                    message: format!("{} canaries deployed, none triggered", canaries.len()),
                    latency_ms: Some(start.elapsed().as_millis() as u64),
                }
            }
        }
        Err(e) => HealthResult {
            name: "canary".into(),
            category: "security".into(),
            status: HealthStatus::Failed,
            message: format!("Cannot check canaries: {}", e),
            latency_ms: Some(start.elapsed().as_millis() as u64),
        },
    }
}

// ─── Encryption Health ─────────────────────────────────────────

fn check_encryption() -> HealthResult {
    let start = Instant::now();

    match crate::ops::encrypt::age_key_path() {
        Ok(path) => {
            if Path::new(&path).exists() {
                HealthResult {
                    name: "encryption".into(),
                    category: "security".into(),
                    status: HealthStatus::Healthy,
                    message: format!("Encryption key found at {}", path.display()),
                    latency_ms: Some(start.elapsed().as_millis() as u64),
                }
            } else {
                HealthResult {
                    name: "encryption".into(),
                    category: "security".into(),
                    status: HealthStatus::Degraded,
                    message: "Encryption key not found — generate with envforge init".into(),
                    latency_ms: Some(start.elapsed().as_millis() as u64),
                }
            }
        }
        Err(e) => HealthResult {
            name: "encryption".into(),
            category: "security".into(),
            status: HealthStatus::Failed,
            message: format!("Cannot determine encryption key path: {}", e),
            latency_ms: Some(start.elapsed().as_millis() as u64),
        },
    }
}

// ─── Fence Health ───────────────────────────────────────────────

fn check_fence() -> HealthResult {
    let start = Instant::now();

    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            return HealthResult {
                name: "fence".into(),
                category: "security".into(),
                status: HealthStatus::Failed,
                message: format!("Cannot determine current directory: {}", e),
                latency_ms: Some(start.elapsed().as_millis() as u64),
            }
        }
    };

    match crate::ops::fence::check_fence_status(&cwd) {
        Ok(status) => {
            if status.all_fenced {
                HealthResult {
                    name: "fence".into(),
                    category: "security".into(),
                    status: HealthStatus::Healthy,
                    message: format!("Fence active — {} file(s) fenced", status.files.len()),
                    latency_ms: Some(start.elapsed().as_millis() as u64),
                }
            } else {
                let unfenced: Vec<_> = status
                    .files
                    .iter()
                    .filter(|f| !f.fenced)
                    .map(|f| f.path.clone())
                    .collect();
                HealthResult {
                    name: "fence".into(),
                    category: "security".into(),
                    status: HealthStatus::Degraded,
                    message: format!(
                        "Fence partially active — {} unfenced: {}",
                        unfenced.len(),
                        unfenced.join(", ")
                    ),
                    latency_ms: Some(start.elapsed().as_millis() as u64),
                }
            }
        }
        Err(e) => HealthResult {
            name: "fence".into(),
            category: "security".into(),
            status: HealthStatus::Degraded,
            message: format!("Fence not active: {}", e),
            latency_ms: Some(start.elapsed().as_millis() as u64),
        },
    }
}
