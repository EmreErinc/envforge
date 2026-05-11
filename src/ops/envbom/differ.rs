//! Diff between two `EnvBom` instances.

use serde::{Deserialize, Serialize};

use super::builder::{Classification, EnvBom, EnvBomKey, ValueState};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChangedField {
    ValueSha256,
    Classification,
    Owner,
    LastRotated,
    ProviderRef,
    SchemaRequired,
    ValueState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyChange {
    pub key: String,
    pub field: ChangedField,
    pub old: Option<String>,
    pub new: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BomDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<KeyChange>,
    pub unchanged_count: usize,
}

/// Structural diff of two BOMs. Old vs new.
pub fn diff(old: &EnvBom, new: &EnvBom) -> BomDiff {
    let mut report = BomDiff::default();

    for k in new.keys.keys() {
        if !old.keys.contains_key(k) {
            report.added.push(k.clone());
        }
    }
    for k in old.keys.keys() {
        if !new.keys.contains_key(k) {
            report.removed.push(k.clone());
        }
    }
    for (k, old_entry) in &old.keys {
        if let Some(new_entry) = new.keys.get(k) {
            let mut changed_in_this_key = false;
            collect_field_changes(
                k,
                old_entry,
                new_entry,
                &mut report.changed,
                &mut changed_in_this_key,
            );
            if !changed_in_this_key {
                report.unchanged_count += 1;
            }
        }
    }

    report.added.sort();
    report.removed.sort();
    report.changed.sort_by(|a, b| {
        (&a.key, format!("{:?}", a.field)).cmp(&(&b.key, format!("{:?}", b.field)))
    });
    report
}

fn collect_field_changes(
    key: &str,
    old: &EnvBomKey,
    new: &EnvBomKey,
    out: &mut Vec<KeyChange>,
    changed_flag: &mut bool,
) {
    if old.value_sha256 != new.value_sha256 {
        out.push(KeyChange {
            key: key.to_string(),
            field: ChangedField::ValueSha256,
            old: old.value_sha256.clone(),
            new: new.value_sha256.clone(),
        });
        *changed_flag = true;
    }
    if old.classification != new.classification {
        out.push(KeyChange {
            key: key.to_string(),
            field: ChangedField::Classification,
            old: Some(format!("{:?}", old.classification)),
            new: Some(format!("{:?}", new.classification)),
        });
        *changed_flag = true;
    }
    if old.owner != new.owner {
        out.push(KeyChange {
            key: key.to_string(),
            field: ChangedField::Owner,
            old: old.owner.clone(),
            new: new.owner.clone(),
        });
        *changed_flag = true;
    }
    if old.last_rotated != new.last_rotated {
        out.push(KeyChange {
            key: key.to_string(),
            field: ChangedField::LastRotated,
            old: old.last_rotated.clone(),
            new: new.last_rotated.clone(),
        });
        *changed_flag = true;
    }
    if old.provider_ref != new.provider_ref {
        out.push(KeyChange {
            key: key.to_string(),
            field: ChangedField::ProviderRef,
            old: old.provider_ref.clone(),
            new: new.provider_ref.clone(),
        });
        *changed_flag = true;
    }
    if old.schema_required != new.schema_required {
        out.push(KeyChange {
            key: key.to_string(),
            field: ChangedField::SchemaRequired,
            old: Some(old.schema_required.to_string()),
            new: Some(new.schema_required.to_string()),
        });
        *changed_flag = true;
    }
    if value_state_label(&old.value_state) != value_state_label(&new.value_state) {
        out.push(KeyChange {
            key: key.to_string(),
            field: ChangedField::ValueState,
            old: Some(value_state_label(&old.value_state).into()),
            new: Some(value_state_label(&new.value_state).into()),
        });
        *changed_flag = true;
    }
    let _ = (Classification::Public, ValueState::Plain); // silence warning about unused imports under cfg
}

fn value_state_label(s: &ValueState) -> &'static str {
    match s {
        ValueState::Plain => "Plain",
        ValueState::Encrypted => "Encrypted",
        ValueState::Missing => "Missing",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::envbom::builder::{build_bom_from_inputs, BuildInputs};
    use std::collections::{BTreeMap, BTreeSet};

    fn build(pairs: Vec<(&str, Option<&str>)>) -> EnvBom {
        let i = BuildInputs {
            project_id: "p",
            profile: None,
            key_pairs: pairs
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.map(String::from)))
                .collect(),
            provider_refs: BTreeMap::new(),
            classifications: BTreeMap::new(),
            schema_required: BTreeSet::new(),
            last_rotated: BTreeMap::new(),
            reachable_paths: BTreeMap::new(),
            owners: BTreeMap::new(),
            reproducible_now: Some("2026-05-08T00:00:00Z"),
        };
        build_bom_from_inputs(&i)
    }

    #[test]
    fn identical_boms_have_no_changes() {
        let a = build(vec![("X", Some("v"))]);
        let b = build(vec![("X", Some("v"))]);
        let d = diff(&a, &b);
        assert!(d.added.is_empty());
        assert!(d.removed.is_empty());
        assert!(d.changed.is_empty());
        assert_eq!(d.unchanged_count, 1);
    }

    #[test]
    fn added_key_detected() {
        let a = build(vec![]);
        let b = build(vec![("NEW", Some("v"))]);
        let d = diff(&a, &b);
        assert_eq!(d.added, vec!["NEW".to_string()]);
        assert!(d.removed.is_empty());
    }

    #[test]
    fn removed_key_detected() {
        let a = build(vec![("OLD", Some("v"))]);
        let b = build(vec![]);
        let d = diff(&a, &b);
        assert_eq!(d.removed, vec!["OLD".to_string()]);
        assert!(d.added.is_empty());
    }

    #[test]
    fn changed_value_hash_detected() {
        let a = build(vec![("X", Some("old"))]);
        let b = build(vec![("X", Some("new"))]);
        let d = diff(&a, &b);
        assert_eq!(d.changed.len(), 1);
        assert_eq!(d.changed[0].field, ChangedField::ValueSha256);
        assert_eq!(d.unchanged_count, 0);
    }

    #[test]
    fn changed_value_state_detected() {
        let a = build(vec![("X", Some("plaintext"))]);
        let b = build(vec![("X", Some("ENC[age:abc]"))]);
        let d = diff(&a, &b);
        // both ValueSha256 + ValueState change
        assert!(d
            .changed
            .iter()
            .any(|c| c.field == ChangedField::ValueSha256));
        assert!(d
            .changed
            .iter()
            .any(|c| c.field == ChangedField::ValueState));
    }

    #[test]
    fn diff_output_is_sorted() {
        let a = build(vec![("Z", Some("v"))]);
        let b = build(vec![("Z", Some("v")), ("A", Some("v")), ("M", Some("v"))]);
        let d = diff(&a, &b);
        let mut sorted = d.added.clone();
        sorted.sort();
        assert_eq!(d.added, sorted);
    }

    #[test]
    fn diff_serializes_for_json_output() {
        let a = build(vec![("X", Some("v"))]);
        let b = build(vec![("X", Some("w")), ("Y", Some("v"))]);
        let d = diff(&a, &b);
        let s = serde_json::to_string(&d).unwrap();
        assert!(s.contains("\"added\""));
        assert!(s.contains("\"changed\""));
    }
}
