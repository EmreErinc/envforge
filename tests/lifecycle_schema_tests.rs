use envforge::ops::lifecycle::schema_lifecycle;
use envforge::ops::schema::{EnvSchema, SchemaVariable, VarType};
use std::collections::HashMap;

fn make_schema() -> EnvSchema {
    let mut vars = HashMap::new();

    let with_ttl = SchemaVariable {
        ttl_days: Some(30),
        auto_rotate: Some(true),
        rotation_strategy: Some("dual_write".into()),
        var_type: VarType::String,
        ..SchemaVariable::default()
    };
    vars.insert("API_KEY".into(), with_ttl);

    let without_ttl = SchemaVariable {
        var_type: VarType::String,
        ..SchemaVariable::default()
    };
    vars.insert("PLAIN_KEY".into(), without_ttl);

    let with_notify = SchemaVariable {
        ttl_days: Some(90),
        notify_days_before_expiry: Some(7),
        var_type: VarType::String,
        ..SchemaVariable::default()
    };
    vars.insert("DB_PASSWORD".into(), with_notify);

    EnvSchema { variables: vars }
}

#[test]
fn test_generate_rules_only_for_ttl_variables() {
    let schema = make_schema();
    let rules = schema_lifecycle::generate_rules_from_schema(&schema);

    // PLAIN_KEY has no ttl_days → excluded
    // API_KEY has ttl_days=30 + auto_rotate → included (Rotate)
    // DB_PASSWORD has ttl_days=90 + notify → included (Notify)
    assert_eq!(rules.len(), 2);
    assert!(rules.iter().any(|r| r.name == "auto-API_KEY"));
    assert!(rules.iter().any(|r| r.name == "auto-DB_PASSWORD"));
}

#[test]
fn test_generated_rules_have_schema_tags() {
    let schema = make_schema();
    let rules = schema_lifecycle::generate_rules_from_schema(&schema);

    let api_rule = rules.iter().find(|r| r.name == "auto-API_KEY").unwrap();
    assert!(api_rule.tags.contains(&"schema".to_string()));
    assert!(api_rule.tags.contains(&"schema_key:API_KEY".to_string()));
}

#[test]
fn test_empty_schema_produces_no_rules() {
    let schema = EnvSchema {
        variables: HashMap::new(),
    };
    let rules = schema_lifecycle::generate_rules_from_schema(&schema);
    assert!(rules.is_empty());
}

#[test]
fn test_ttl_without_action_skipped() {
    let mut vars = HashMap::new();
    let var = SchemaVariable {
        ttl_days: Some(60),
        var_type: VarType::String,
        ..SchemaVariable::default()
    };
    // No auto_rotate, no notify → no action, skipped
    vars.insert("NO_ACTION_KEY".into(), var);
    let schema = EnvSchema { variables: vars };

    let rules = schema_lifecycle::generate_rules_from_schema(&schema);
    assert!(rules.is_empty());
}
