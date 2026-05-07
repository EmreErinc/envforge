use envforge::model::OperationType;
use envforge::ops::lifecycle::rollback;
use uuid::Uuid;

fn random_key() -> String {
    format!("ROLLBACK_{}", Uuid::new_v4().to_string().replace('-', "_"))
}

// ─── Create Snapshot ────────────────────────────────────

#[test]
fn test_create_snapshot_succeeds() {
    let key = random_key();
    let op = OperationType::Rotate;

    let meta =
        rollback::create_snapshot(&key, &op, Some("test-value-12345")).expect("create snapshot");

    assert_eq!(meta.key, key);
    assert_eq!(meta.operation_type, op);
    assert!(meta.masked_value.contains("****"));
    assert!(!meta.id.is_nil());
}

#[test]
fn test_create_snapshot_without_value() {
    let key = random_key();
    let meta = rollback::create_snapshot(&key, &OperationType::Decommission, None)
        .expect("create snapshot");

    assert_eq!(meta.key, key);
    assert!(meta.masked_value.is_empty());
}

// ─── List Snapshots ─────────────────────────────────────

#[test]
fn test_list_snapshots_returns_created() {
    let key = random_key();
    rollback::create_snapshot(&key, &OperationType::Rotate, Some("val")).expect("create");

    let metas = rollback::list_snapshots(Some(&key)).expect("list");
    assert!(!metas.is_empty());
    assert_eq!(metas[0].key, key);
}

#[test]
fn test_list_snapshots_filter_by_key() {
    let key_a = random_key();
    let key_b = random_key();

    rollback::create_snapshot(&key_a, &OperationType::Rotate, Some("val")).expect("create a");
    rollback::create_snapshot(&key_b, &OperationType::Rotate, Some("val")).expect("create b");

    let a_metas = rollback::list_snapshots(Some(&key_a)).expect("list a");
    assert!(a_metas.iter().all(|m| m.key == key_a));
}

// ─── Delete Snapshot ────────────────────────────────────

#[test]
fn test_delete_snapshot() {
    let key = random_key();
    let meta =
        rollback::create_snapshot(&key, &OperationType::Rotate, Some("val")).expect("create");
    let id = meta.id;

    rollback::delete_snapshot(&id).expect("delete");

    let metas = rollback::list_snapshots(Some(&key)).expect("list");
    assert!(metas.iter().all(|m| m.id != id));
}

#[test]
fn test_delete_nonexistent_snapshot_no_error() {
    let result = rollback::delete_snapshot(&Uuid::new_v4());
    assert!(result.is_ok());
}

// ─── Rollback ───────────────────────────────────────────

#[test]
fn test_rollback_nonexistent_errors() {
    let result = rollback::rollback(&Uuid::new_v4());
    assert!(result.is_err());
}
