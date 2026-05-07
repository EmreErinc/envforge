use envforge::model::{LifecycleAction, LifecycleRule, LifecycleTrigger, RotationStrategy};
use envforge::ops::lifecycle::rule_manager;
use tempfile::TempDir;

fn setup_test_dir() -> (TempDir, std::path::PathBuf) {
    let tmp = TempDir::new().expect("create temp dir");
    let base = tmp.path().to_path_buf();
    (tmp, base)
}

fn make_test_rule(name: &str) -> LifecycleRule {
    LifecycleRule::new(
        name.into(),
        LifecycleTrigger::AgeExceeded { max_days: 90 },
        LifecycleAction::Rotate {
            strategy: RotationStrategy::Replace,
        },
    )
}

// ─── CRUD ────────────────────────────────────────────────

#[test]
fn test_create_and_get_rule() {
    let (_tmp, base) = setup_test_dir();
    let rule = make_test_rule("test-create");
    let rule_id = rule.id;

    let created = rule_manager::create_rule_in(rule, &base).expect("create");
    assert_eq!(created.name, "test-create");
    assert!(created.enabled);

    let fetched = rule_manager::get_rule_from(&rule_id, &base).expect("get");
    assert_eq!(fetched.name, "test-create");
    assert_eq!(fetched.id, rule_id);
}

#[test]
fn test_update_rule_changes_name() {
    let (_tmp, base) = setup_test_dir();
    let mut rule = make_test_rule("update-test");
    let rule_id = rule.id;

    rule_manager::create_rule_in(rule.clone(), &base).expect("create");

    rule.name = "updated-name".into();
    rule_manager::update_rule_in(&rule, &base).expect("update");

    let fetched = rule_manager::get_rule_from(&rule_id, &base).expect("get");
    assert_eq!(fetched.name, "updated-name");
}

#[test]
fn test_delete_rule_removes_file() {
    let (_tmp, base) = setup_test_dir();
    let rule = make_test_rule("delete-me");
    let rule_id = rule.id;

    rule_manager::create_rule_in(rule, &base).expect("create");
    rule_manager::delete_rule_from(&rule_id, &base).expect("delete");

    let result = rule_manager::get_rule_from(&rule_id, &base);
    assert!(result.is_err());
}

// ─── Queries ─────────────────────────────────────────────

#[test]
fn test_list_rules_returns_multiple() {
    let (_tmp, base) = setup_test_dir();

    rule_manager::create_rule_in(make_test_rule("rule-a"), &base).expect("create a");
    rule_manager::create_rule_in(make_test_rule("rule-b"), &base).expect("create b");
    rule_manager::create_rule_in(make_test_rule("rule-c"), &base).expect("create c");

    let rules = rule_manager::list_rules_in(&base).expect("list");
    assert_eq!(rules.len(), 3);
}

#[test]
fn test_list_enabled_rules_filters_disabled() {
    let (_tmp, base) = setup_test_dir();
    let rule = make_test_rule("enabled-rule");
    let rule_id = rule.id;

    rule_manager::create_rule_in(rule, &base).expect("create");
    rule_manager::disable_rule_in(&rule_id, &base).expect("disable");

    let all = rule_manager::list_rules_in(&base).expect("list all");
    assert_eq!(all.len(), 1);

    let enabled_only: Vec<_> = all.into_iter().filter(|r| r.enabled).collect();
    assert!(enabled_only.is_empty());
}

// ─── Enable / Disable ────────────────────────────────────

#[test]
fn test_enable_disable_rule() {
    let (_tmp, base) = setup_test_dir();
    let rule = make_test_rule("toggle-rule");
    let rule_id = rule.id;

    rule_manager::create_rule_in(rule, &base).expect("create");

    rule_manager::disable_rule_in(&rule_id, &base).expect("disable");
    let disabled = rule_manager::get_rule_from(&rule_id, &base).expect("get");
    assert!(!disabled.enabled);

    rule_manager::enable_rule_in(&rule_id, &base).expect("enable");
    let enabled = rule_manager::get_rule_from(&rule_id, &base).expect("get");
    assert!(enabled.enabled);
}

// ─── Edge Cases ──────────────────────────────────────────

#[test]
fn test_get_nonexistent_rule_errors() {
    let (_tmp, base) = setup_test_dir();
    let fake_id = uuid::Uuid::new_v4();
    let result = rule_manager::get_rule_from(&fake_id, &base);
    assert!(result.is_err());
}

#[test]
fn test_delete_nonexistent_rule_errors() {
    let (_tmp, base) = setup_test_dir();
    let fake_id = uuid::Uuid::new_v4();
    let result = rule_manager::delete_rule_from(&fake_id, &base);
    assert!(result.is_err());
}

#[test]
fn test_update_nonexistent_rule_errors() {
    let (_tmp, base) = setup_test_dir();
    let rule = make_test_rule("nonexistent-update");
    let result = rule_manager::update_rule_in(&rule, &base);
    assert!(result.is_err());
}
