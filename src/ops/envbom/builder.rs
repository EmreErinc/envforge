//! Build an `EnvBom` from project state — schema + values + audit data.
//! Pure-data assembly; no signing, no I/O beyond reading the schema/env files
//! that the existing schema module already accesses.

use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::EnvbomError;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Classification {
    Public,
    #[default]
    Internal,
    Confidential,
    Restricted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueState {
    Plain,
    Encrypted,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PathKind {
    Read,
    Expand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathRef {
    pub file: String,
    pub line: u32,
    pub kind: PathKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvBomKey {
    pub name: String,
    pub provider_ref: Option<String>,
    pub owner: Option<String>,
    pub classification: Classification,
    pub last_rotated: Option<String>,
    pub value_sha256: Option<String>,
    pub value_state: ValueState,
    pub reachable_paths: Vec<PathRef>,
    pub schema_required: bool,
    pub profiles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreationInfo {
    pub created: String,
    pub creators: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClassifiedCounts {
    pub public: usize,
    pub internal: usize,
    pub confidential: usize,
    pub restricted: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuditSummary {
    pub total_keys: usize,
    pub classified: ClassifiedCounts,
    pub unrotated_over_90d: usize,
    pub providers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvBom {
    pub spdx_id: String,
    pub spdx_version: String,
    pub name: String,
    pub document_namespace: String,
    pub creation_info: CreationInfo,
    pub project_id: String,
    pub generator: String,
    pub generated_at: String,
    pub keys: BTreeMap<String, EnvBomKey>,
    pub audit_summary: AuditSummary,
}

/// Inputs to a BOM build.
pub struct BuildInputs<'a> {
    pub project_id: &'a str,
    pub profile: Option<&'a str>,
    pub key_pairs: Vec<(String, Option<String>)>,
    pub provider_refs: BTreeMap<String, String>,
    pub classifications: BTreeMap<String, Classification>,
    pub schema_required: BTreeSet<String>,
    pub last_rotated: BTreeMap<String, String>,
    pub reachable_paths: BTreeMap<String, Vec<PathRef>>,
    pub owners: BTreeMap<String, String>,
    pub reproducible_now: Option<&'a str>,
}

/// Build a BOM from explicit inputs. Callers usually go through the higher-level
/// `build_bom()` which gathers inputs from the project; this fn is the testable
/// pure core.
pub fn build_bom_from_inputs(inputs: &BuildInputs) -> EnvBom {
    let now = inputs
        .reproducible_now
        .map(String::from)
        .unwrap_or_else(|| Utc::now().to_rfc3339());

    let mut keys: BTreeMap<String, EnvBomKey> = BTreeMap::new();
    for (name, value_opt) in &inputs.key_pairs {
        let entry = build_key_entry(
            name,
            value_opt.as_deref(),
            inputs.provider_refs.get(name).cloned(),
            inputs.owners.get(name).cloned(),
            inputs
                .classifications
                .get(name)
                .cloned()
                .unwrap_or_default(),
            inputs.last_rotated.get(name).cloned(),
            inputs
                .reachable_paths
                .get(name)
                .cloned()
                .unwrap_or_default(),
            inputs.schema_required.contains(name),
            inputs
                .profile
                .map(|p| vec![p.to_string()])
                .unwrap_or_default(),
        );
        keys.insert(name.clone(), entry);
    }

    let audit_summary = aggregate_audit_summary(&keys);

    EnvBom {
        spdx_id: "SPDXRef-DOCUMENT".into(),
        spdx_version: "SPDX-2.3".into(),
        name: inputs.project_id.to_string(),
        document_namespace: format!("urn:envforge:envbom:{}:{}", inputs.project_id, now),
        creation_info: CreationInfo {
            created: now.clone(),
            creators: vec![format!("Tool: envforge-{}", env!("CARGO_PKG_VERSION"))],
        },
        project_id: inputs.project_id.to_string(),
        generator: format!("envforge-{}", env!("CARGO_PKG_VERSION")),
        generated_at: now,
        keys,
        audit_summary,
    }
}

/// Convenience for callers that have already collected key pairs from the env;
/// stub for now. Real `build_bom` (gathering schema + providers) lives at the
/// edge of the module and is wired up in CLI / Stage-4 follow-ups.
pub fn build_bom(
    project_id: &str,
    profile: Option<&str>,
    key_pairs: Vec<(String, Option<String>)>,
    reproducible_now: Option<&str>,
) -> Result<EnvBom, EnvbomError> {
    let inputs = BuildInputs {
        project_id,
        profile,
        key_pairs,
        provider_refs: BTreeMap::new(),
        classifications: BTreeMap::new(),
        schema_required: BTreeSet::new(),
        last_rotated: BTreeMap::new(),
        reachable_paths: BTreeMap::new(),
        owners: BTreeMap::new(),
        reproducible_now,
    };
    Ok(build_bom_from_inputs(&inputs))
}

#[allow(clippy::too_many_arguments)]
fn build_key_entry(
    name: &str,
    value: Option<&str>,
    provider_ref: Option<String>,
    owner: Option<String>,
    classification: Classification,
    last_rotated: Option<String>,
    mut reachable_paths: Vec<PathRef>,
    schema_required: bool,
    profiles: Vec<String>,
) -> EnvBomKey {
    let (value_sha256, value_state) = match value {
        Some("") => (None, ValueState::Missing),
        Some(v) => {
            let state = if is_encrypted(v) {
                ValueState::Encrypted
            } else {
                ValueState::Plain
            };
            (Some(hash_hex(v)), state)
        }
        None => (None, ValueState::Missing),
    };

    reachable_paths.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    let mut profiles = profiles;
    profiles.sort();

    EnvBomKey {
        name: name.to_string(),
        provider_ref,
        owner,
        classification,
        last_rotated,
        value_sha256,
        value_state,
        reachable_paths,
        schema_required,
        profiles,
    }
}

fn is_encrypted(v: &str) -> bool {
    v.starts_with("ENC[age:") && v.ends_with(']')
}

fn hash_hex(input: &str) -> String {
    let h = Sha256::digest(input.as_bytes());
    hex::encode(h)
}

fn aggregate_audit_summary(keys: &BTreeMap<String, EnvBomKey>) -> AuditSummary {
    let mut classified = ClassifiedCounts::default();
    let mut providers: BTreeSet<String> = BTreeSet::new();
    let mut unrotated_over_90d = 0usize;
    let cutoff = Utc::now() - chrono::Duration::days(90);

    for k in keys.values() {
        match k.classification {
            Classification::Public => classified.public += 1,
            Classification::Internal => classified.internal += 1,
            Classification::Confidential => classified.confidential += 1,
            Classification::Restricted => classified.restricted += 1,
        }
        if let Some(p) = &k.provider_ref {
            providers.insert(p.clone());
        }
        if let Some(rot) = &k.last_rotated {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(rot) {
                if dt.with_timezone(&Utc) <= cutoff {
                    unrotated_over_90d += 1;
                }
            }
        }
    }

    AuditSummary {
        total_keys: keys.len(),
        classified,
        unrotated_over_90d,
        providers: providers.into_iter().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs<'a>(
        project_id: &'a str,
        pairs: Vec<(&'a str, Option<&'a str>)>,
        reproducible_now: Option<&'a str>,
    ) -> BuildInputs<'a> {
        BuildInputs {
            project_id,
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
            reproducible_now,
        }
    }

    #[test]
    fn empty_project_emits_minimal_bom() {
        let bom = build_bom_from_inputs(&inputs("test", vec![], Some("2026-05-08T00:00:00Z")));
        assert_eq!(bom.spdx_version, "SPDX-2.3");
        assert_eq!(bom.spdx_id, "SPDXRef-DOCUMENT");
        assert_eq!(bom.keys.len(), 0);
        assert_eq!(bom.audit_summary.total_keys, 0);
    }

    #[test]
    fn key_with_value_gets_hash_plain_state() {
        let bom = build_bom_from_inputs(&inputs(
            "p",
            vec![("FOO", Some("bar"))],
            Some("2026-05-08T00:00:00Z"),
        ));
        let k = bom.keys.get("FOO").unwrap();
        assert!(matches!(k.value_state, ValueState::Plain));
        assert!(k.value_sha256.is_some());
        assert_eq!(
            k.value_sha256.as_deref().unwrap(),
            "fcde2b2edba56bf408601fb721fe9b5c338d10ee429ea04fae5511b68fbf8fb9"
        );
    }

    #[test]
    fn key_with_no_value_gets_missing_state() {
        let bom = build_bom_from_inputs(&inputs(
            "p",
            vec![("BAR", None)],
            Some("2026-05-08T00:00:00Z"),
        ));
        let k = bom.keys.get("BAR").unwrap();
        assert!(matches!(k.value_state, ValueState::Missing));
        assert!(k.value_sha256.is_none());
    }

    #[test]
    fn key_with_empty_value_is_missing() {
        let bom = build_bom_from_inputs(&inputs(
            "p",
            vec![("E", Some(""))],
            Some("2026-05-08T00:00:00Z"),
        ));
        let k = bom.keys.get("E").unwrap();
        assert!(matches!(k.value_state, ValueState::Missing));
    }

    #[test]
    fn encrypted_value_recognized() {
        let bom = build_bom_from_inputs(&inputs(
            "p",
            vec![("ENC_KEY", Some("ENC[age:abc]"))],
            Some("2026-05-08T00:00:00Z"),
        ));
        let k = bom.keys.get("ENC_KEY").unwrap();
        assert!(matches!(k.value_state, ValueState::Encrypted));
        assert!(k.value_sha256.is_some());
    }

    #[test]
    fn determinism_emit_twice_with_reproducible_now() {
        let i = inputs(
            "p",
            vec![("A", Some("v1")), ("B", Some("v2"))],
            Some("2026-05-08T00:00:00Z"),
        );
        let b1 = build_bom_from_inputs(&i);
        let i2 = inputs(
            "p",
            vec![("B", Some("v2")), ("A", Some("v1"))], // reordered
            Some("2026-05-08T00:00:00Z"),
        );
        let b2 = build_bom_from_inputs(&i2);
        let s1 = serde_json::to_string(&b1).unwrap();
        let s2 = serde_json::to_string(&b2).unwrap();
        assert_eq!(s1, s2);
    }

    #[test]
    fn classification_default_is_internal() {
        assert_eq!(Classification::default(), Classification::Internal);
    }

    #[test]
    fn audit_summary_counts_keys() {
        let mut classifications = BTreeMap::new();
        classifications.insert("PUB".to_string(), Classification::Public);
        classifications.insert("CONF".to_string(), Classification::Confidential);
        let mut i = inputs(
            "p",
            vec![("PUB", Some("x")), ("CONF", Some("y")), ("INT", Some("z"))],
            Some("2026-05-08T00:00:00Z"),
        );
        i.classifications = classifications;
        let bom = build_bom_from_inputs(&i);
        assert_eq!(bom.audit_summary.total_keys, 3);
        assert_eq!(bom.audit_summary.classified.public, 1);
        assert_eq!(bom.audit_summary.classified.confidential, 1);
        assert_eq!(bom.audit_summary.classified.internal, 1);
        assert_eq!(bom.audit_summary.classified.restricted, 0);
    }

    #[test]
    fn audit_summary_lists_providers_sorted() {
        let mut providers = BTreeMap::new();
        providers.insert("KEY_A".to_string(), "vault://path/a".to_string());
        providers.insert("KEY_B".to_string(), "doppler://config/b".to_string());
        providers.insert("KEY_C".to_string(), "vault://path/a".to_string()); // dup
        let mut i = inputs(
            "p",
            vec![
                ("KEY_A", Some("x")),
                ("KEY_B", Some("y")),
                ("KEY_C", Some("z")),
            ],
            Some("2026-05-08T00:00:00Z"),
        );
        i.provider_refs = providers;
        let bom = build_bom_from_inputs(&i);
        assert_eq!(bom.audit_summary.providers.len(), 2);
        // sorted lexicographically
        assert_eq!(bom.audit_summary.providers[0], "doppler://config/b");
        assert_eq!(bom.audit_summary.providers[1], "vault://path/a");
    }

    #[test]
    fn audit_summary_unrotated_count() {
        let mut last_rotated = BTreeMap::new();
        // 100 days ago — over 90d threshold
        let old = (Utc::now() - chrono::Duration::days(100)).to_rfc3339();
        last_rotated.insert("OLD".into(), old);
        // 30 days ago — fresh
        let fresh = (Utc::now() - chrono::Duration::days(30)).to_rfc3339();
        last_rotated.insert("FRESH".into(), fresh);
        let mut i = inputs(
            "p",
            vec![("OLD", Some("x")), ("FRESH", Some("y"))],
            Some("2026-05-08T00:00:00Z"),
        );
        i.last_rotated = last_rotated;
        let bom = build_bom_from_inputs(&i);
        assert_eq!(bom.audit_summary.unrotated_over_90d, 1);
    }

    #[test]
    fn no_raw_value_in_serialized_bom() {
        // Audit-grade lint: serialized BOM must NEVER contain the raw value.
        let secret = "super_secret_value_xyzzy_98765";
        let bom = build_bom_from_inputs(&inputs(
            "p",
            vec![("S", Some(secret))],
            Some("2026-05-08T00:00:00Z"),
        ));
        let s = serde_json::to_string(&bom).unwrap();
        assert!(
            !s.contains(secret),
            "BOM contains raw value (lint failure): {s}"
        );
    }

    #[test]
    fn reachable_paths_sorted_in_output() {
        let mut paths = BTreeMap::new();
        paths.insert(
            "X".to_string(),
            vec![
                PathRef {
                    file: "z.rs".into(),
                    line: 10,
                    kind: PathKind::Read,
                },
                PathRef {
                    file: "a.rs".into(),
                    line: 20,
                    kind: PathKind::Read,
                },
                PathRef {
                    file: "a.rs".into(),
                    line: 5,
                    kind: PathKind::Read,
                },
            ],
        );
        let mut i = inputs("p", vec![("X", Some("v"))], Some("2026-05-08T00:00:00Z"));
        i.reachable_paths = paths;
        let bom = build_bom_from_inputs(&i);
        let k = bom.keys.get("X").unwrap();
        assert_eq!(k.reachable_paths[0].file, "a.rs");
        assert_eq!(k.reachable_paths[0].line, 5);
        assert_eq!(k.reachable_paths[1].line, 20);
        assert_eq!(k.reachable_paths[2].file, "z.rs");
    }

    #[test]
    fn schema_required_flag_propagates() {
        let mut required = BTreeSet::new();
        required.insert("MUST_HAVE".to_string());
        let mut i = inputs(
            "p",
            vec![("MUST_HAVE", Some("v")), ("OPTIONAL", Some("v"))],
            Some("2026-05-08T00:00:00Z"),
        );
        i.schema_required = required;
        let bom = build_bom_from_inputs(&i);
        assert!(bom.keys.get("MUST_HAVE").unwrap().schema_required);
        assert!(!bom.keys.get("OPTIONAL").unwrap().schema_required);
    }
}
