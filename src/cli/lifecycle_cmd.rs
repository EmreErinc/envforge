use clap::Subcommand;
use serde_json::json;

use crate::ops::lifecycle::{orchestrator, rollback, rule_manager, trigger_engine};
use crate::ops::OpError;

#[derive(Subcommand)]
pub enum LifecycleAction {
    /// Evaluate all enabled lifecycle rules against current state
    Check,

    /// Manage lifecycle rules
    #[command(subcommand)]
    Rule(RuleAction),

    /// Manage lifecycle state for a secret
    State {
        /// Secret key
        key: String,
    },

    /// Manage snapshots
    #[command(subcommand)]
    Snapshot(SnapshotAction),
}

#[derive(Subcommand)]
pub enum RuleAction {
    /// List all lifecycle rules
    List,

    /// Manage secret lifecycle via orchestrator
    #[allow(clippy::enum_variant_names)]
    RotateSecret {
        /// Secret key to rotate
        key: String,
        /// Rotation strategy
        #[arg(long, default_value = "replace")]
        strategy: String,
    },
}

#[derive(Subcommand)]
pub enum SnapshotAction {
    /// List snapshots for a key
    List {
        /// Filter by key
        #[arg(long)]
        key: Option<String>,
    },
    /// Delete a snapshot
    Delete {
        /// Snapshot ID
        id: String,
    },
}

pub fn handle_lifecycle(action: &LifecycleAction, json_output: bool) -> Result<(), OpError> {
    match action {
        LifecycleAction::Check => {
            let rules = rule_manager::list_enabled_rules()?;
            let context = crate::model::EvaluationContext {
                project_dir: std::env::current_dir().ok(),
                current_time: chrono::Utc::now(),
                last_check: None,
            };
            let events = trigger_engine::evaluate(&rules, &context)?;
            if json_output {
                println!("{}", serde_json::to_string_pretty(&events)?);
            } else {
                println!("Fired {} trigger(s):", events.len());
                for e in &events {
                    println!("  [{}] rule={} — {}", e.trigger_type, e.rule_id, e.payload);
                }
            }
        }
        LifecycleAction::Rule(rule_action) => match rule_action {
            RuleAction::List => {
                let rules = rule_manager::list_rules()?;
                if json_output {
                    println!("{}", serde_json::to_string_pretty(&rules)?);
                } else {
                    println!("Lifecycle rules ({}):", rules.len());
                    for r in &rules {
                        let status = if r.enabled { "✓" } else { "✗" };
                        println!("  {status} {} — {}", r.name, r.description);
                    }
                }
            }
            RuleAction::RotateSecret { key, strategy } => {
                let strat = match strategy.as_str() {
                    "replace" => crate::model::RotationStrategy::Replace,
                    "dual_write" => crate::model::RotationStrategy::DualWrite,
                    "blue_green" => crate::model::RotationStrategy::BlueGreen,
                    "provider_managed" => crate::model::RotationStrategy::ProviderManaged,
                    _ => return Err(OpError::Other(format!("unknown strategy: {strategy}"))),
                };
                let result = orchestrator::rotate_secret(key, &strat)?;
                if json_output {
                    println!("{}", serde_json::to_string_pretty(&result)?);
                } else {
                    println!("Rotated {}: success={}", result.key, result.success);
                }
            }
        },
        LifecycleAction::State { key } => {
            let state = orchestrator::get_state(key)?;
            if json_output {
                println!("{}", json!({ "key": key, "state": format!("{:?}", state) }));
            } else {
                println!("{key}: {state:?}");
            }
        }
        LifecycleAction::Snapshot(snap_action) => match snap_action {
            SnapshotAction::List { key } => {
                let metas = rollback::list_snapshots(key.as_deref())?;
                if json_output {
                    println!("{}", serde_json::to_string_pretty(&metas)?);
                } else {
                    println!("Snapshots ({}):", metas.len());
                    for m in &metas {
                        println!("  {} — {} ({:?})", m.id, m.key, m.operation_type);
                    }
                }
            }
            SnapshotAction::Delete { id } => {
                let uid = uuid::Uuid::parse_str(id)
                    .map_err(|e| OpError::Other(format!("invalid UUID: {e}")))?;
                rollback::delete_snapshot(&uid)?;
                println!("Deleted snapshot {id}");
            }
        },
    }
    Ok(())
}
