//! Coverage for `ops::envbom` build + diff: value-state classification, audit
//! summary aggregation, deterministic canonical output, and structural BOM diff.

use envforge::ops::envbom::builder::{
    build_bom_from_inputs, BuildInputs, Classification, EnvBom, ValueState,
};
use envforge::ops::envbom::differ::{diff, ChangedField};
use envforge::ops::envbom::serializer::canonical_json;
use std::collections::{BTreeMap, BTreeSet};

fn build(project: &str, kp: &[(&str, Option<&str>)], cls: &[(&str, Classification)]) -> EnvBom {
    let key_pairs = kp
        .iter()
        .map(|(k, v)| (k.to_string(), v.map(|s| s.to_string())))
        .collect();
    let classifications = cls
        .iter()
        .map(|(k, c)| (k.to_string(), c.clone()))
        .collect();
    let inputs = BuildInputs {
        project_id: project,
        profile: None,
        key_pairs,
        provider_refs: BTreeMap::new(),
        classifications,
        schema_required: BTreeSet::new(),
        last_rotated: BTreeMap::new(),
        reachable_paths: BTreeMap::new(),
        owners: BTreeMap::new(),
        reproducible_now: Some("2026-01-01T00:00:00Z"),
    };
    build_bom_from_inputs(&inputs)
}

// ---- builder ---------------------------------------------------------------

#[test]
fn test_build_value_states_and_hash() {
    let bom = build(
        "proj",
        &[("A", Some("alpha")), ("B", None), ("C", Some(""))],
        &[],
    );
    let a = &bom.keys["A"];
    assert_eq!(a.value_state, ValueState::Plain);
    assert!(a.value_sha256.is_some());

    assert_eq!(bom.keys["B"].value_state, ValueState::Missing);
    assert!(bom.keys["B"].value_sha256.is_none());
    // Empty string is treated as Missing, not an empty plaintext value.
    assert_eq!(bom.keys["C"].value_state, ValueState::Missing);
}

#[test]
fn test_build_spdx_envelope_and_reproducible_now() {
    let bom = build("my-project", &[("K", Some("v"))], &[]);
    assert_eq!(bom.spdx_id, "SPDXRef-DOCUMENT");
    assert_eq!(bom.spdx_version, "SPDX-2.3");
    assert_eq!(bom.name, "my-project");
    assert_eq!(bom.generated_at, "2026-01-01T00:00:00Z");
}

#[test]
fn test_build_audit_summary_classification_counts() {
    let bom = build(
        "proj",
        &[("A", Some("1")), ("B", Some("2")), ("C", Some("3"))],
        &[
            ("A", Classification::Restricted),
            ("B", Classification::Public),
            // C defaults to Internal
        ],
    );
    assert_eq!(bom.audit_summary.total_keys, 3);
    assert_eq!(bom.audit_summary.classified.restricted, 1);
    assert_eq!(bom.audit_summary.classified.public, 1);
    assert_eq!(bom.audit_summary.classified.internal, 1);
}

#[test]
fn test_canonical_json_is_deterministic() {
    let a = build("proj", &[("Z", Some("z")), ("A", Some("a"))], &[]);
    let b = build("proj", &[("A", Some("a")), ("Z", Some("z"))], &[]);
    // Same logical content (insertion order differs) → identical canonical bytes.
    assert_eq!(canonical_json(&a).unwrap(), canonical_json(&b).unwrap());
}

// ---- differ ----------------------------------------------------------------

#[test]
fn test_diff_added_removed_changed() {
    let old = build("p", &[("A", Some("alpha")), ("B", Some("beta"))], &[]);
    let new = build("p", &[("A", Some("ALPHA")), ("C", Some("gamma"))], &[]);
    let d = diff(&old, &new);

    assert_eq!(d.added, vec!["C".to_string()]);
    assert_eq!(d.removed, vec!["B".to_string()]);
    assert_eq!(d.unchanged_count, 0);
    assert!(d
        .changed
        .iter()
        .any(|c| c.key == "A" && c.field == ChangedField::ValueSha256));
}

#[test]
fn test_diff_identical_boms_no_changes() {
    let bom = build("p", &[("A", Some("a")), ("B", Some("b"))], &[]);
    let d = diff(&bom, &bom.clone());
    assert!(d.added.is_empty());
    assert!(d.removed.is_empty());
    assert!(d.changed.is_empty());
    assert_eq!(d.unchanged_count, 2);
}
