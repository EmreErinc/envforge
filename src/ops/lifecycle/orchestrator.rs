use crate::model::{
    CreateResult, DecommissionPlan, DecommissionResult, LifecycleState, RotateResult,
    RotationStrategy, SecretTemplate, StateEvent,
};
use crate::ops::lifecycle::state_machine;
use crate::ops::OpError;

/// Generate a random secret value.
fn generate_value(length: usize, chars: &str) -> String {
    use rand::RngExt;
    let mut rng = rand::rng();
    let charset: Vec<char> = chars.chars().collect();
    (0..length)
        .map(|_| charset[rng.random_range(0..charset.len())])
        .collect()
}

// ─── Create ──────────────────────────────────────────────

/// Create a new secret from a template.
pub fn create_secret(template: &SecretTemplate) -> Result<CreateResult, OpError> {
    use crate::model::SecretGenerator;

    let value = match &template.generator {
        SecretGenerator::Random { length, chars } => generate_value(*length, chars),
        SecretGenerator::Provider { provider, path } => {
            return Err(OpError::Other(format!(
                "provider-based creation not yet implemented ({provider}/{path})"
            )));
        }
        SecretGenerator::Certificate => {
            return Err(OpError::Other(
                "certificate generation deferred to future intent".into(),
            ));
        }
    };

    let key = template.name.to_uppercase().replace([' ', '-'], "_");

    // Write to primary shell file
    let config = crate::config::load_or_create_default()?;
    let shell_path = std::path::PathBuf::from(&config.files.primary);

    let mut shell_file = crate::parser::parse_shell_file(&shell_path)?;
    crate::ops::crud::add_entry(
        &mut shell_file,
        &key,
        &value,
        crate::model::ExportStyle::Export,
        crate::model::QuoteStyle::None,
        0,
        0,
    )?;

    let output = crate::parser::serialize_shell_file(&shell_file);
    crate::config::safe_write(&shell_path, &output, None)?;

    // Record in age tracker
    crate::ops::secrets::age::record_set(&key, "template", &shell_path.to_string_lossy())
        .map_err(|e| OpError::Other(e.to_string()))?;

    // Transition to Active
    let _ = apply_state_transition(&key, &StateEvent::CreateComplete);

    Ok(CreateResult {
        key,
        value_set: true,
        template_id: template.id,
    })
}

// ─── Rotate ──────────────────────────────────────────────

/// Orchestrate rotation of a secret key.
pub fn rotate_secret(key: &str, strategy: &RotationStrategy) -> Result<RotateResult, OpError> {
    apply_state_transition(key, &StateEvent::RotationRequested)?;

    let new_value = generate_value(
        32,
        "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",
    );

    let plan = crate::ops::rotate::plan_rotation(key)?;
    crate::ops::rotate::apply_rotation(key, &new_value, &plan).map_err(|e| {
        let _ = apply_state_transition(
            key,
            &StateEvent::Failure {
                reason: e.to_string(),
            },
        );
        OpError::Other(format!("rotation failed: {e}"))
    })?;

    crate::ops::secrets::age::record_set(key, "rotation", "orchestrator")
        .map_err(|e| OpError::Other(e.to_string()))?;

    apply_state_transition(key, &StateEvent::RotationComplete)?;

    Ok(RotateResult {
        key: key.to_string(),
        success: true,
        new_value_set: true,
        strategy: strategy.clone(),
    })
}

// ─── Decommission ────────────────────────────────────────

/// Decommission a secret key.
pub fn decommission_secret(
    key: &str,
    plan: &DecommissionPlan,
) -> Result<DecommissionResult, OpError> {
    let grace_applied = plan.recommended_grace_days > 0;

    if grace_applied {
        apply_state_transition(key, &StateEvent::DeprecationRequested)?;
    } else {
        apply_state_transition(key, &StateEvent::DeprecationRequested)?;
        apply_state_transition(key, &StateEvent::GracePeriodExpired)?;
        apply_state_transition(key, &StateEvent::DecommissionComplete)?;
    }

    Ok(DecommissionResult {
        key: key.to_string(),
        success: true,
        grace_period_applied: grace_applied,
    })
}

// ─── State Management ────────────────────────────────────

/// Get the current lifecycle state for a key.
pub fn get_state(key: &str) -> Result<LifecycleState, OpError> {
    use std::fs;
    use std::path::PathBuf;

    let dir = crate::config::config_dir()?.join("lifecycle/states");
    let path: PathBuf = dir.join(format!("{key}.jsonl"));

    if !path.exists() {
        return Ok(LifecycleState::Active);
    }

    let content = fs::read_to_string(&path)?;
    let last_line = content.lines().last().unwrap_or("");
    if last_line.is_empty() {
        return Ok(LifecycleState::Active);
    }

    let st: crate::model::StateTransition =
        serde_json::from_str(last_line).map_err(OpError::Json)?;
    Ok(st.to)
}

/// Apply a state transition and persist to the state log.
fn apply_state_transition(key: &str, event: &StateEvent) -> Result<LifecycleState, OpError> {
    use std::fs;
    use std::io::Write;

    let current = get_state(key)?;
    let next =
        state_machine::transition(&current, event).map_err(|e| OpError::Other(e.to_string()))?;

    let transition =
        state_machine::create_transition(current, next.clone(), &format!("{event:?}"), None);

    let dir = crate::config::config_dir()?.join("lifecycle/states");
    fs::create_dir_all(&dir)?;

    let path = dir.join(format!("{key}.jsonl"));
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;

    let line = serde_json::to_string(&transition)?;
    writeln!(file, "{line}")?;

    Ok(next)
}
