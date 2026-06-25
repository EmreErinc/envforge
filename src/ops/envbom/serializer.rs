//! Canonical serialization + digest.
//! Predicate URL: `https://envforge.dev/envbom/v1`.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::builder::EnvBom;
use super::EnvbomError;

/// Predicate URL for in-toto attestations. Versioned; major bump on breaking changes.
pub const PREDICATE_TYPE_V1: &str = "https://envforge.dev/envbom/v1";

/// in-toto subject identifying the artifact being attested.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomSubject {
    pub name: String,
    pub digest_alg: String,
    pub digest_hex: String,
}

/// Encode a BOM as canonical JSON. Object keys at every depth are sorted;
/// indentation is deterministic. Output is byte-stable across `serde_json` versions.
pub fn canonical_json(bom: &EnvBom) -> Result<Vec<u8>, EnvbomError> {
    let value: serde_json::Value = serde_json::to_value(bom)?;
    let canonical = canonicalize_value(value);
    let s = serde_json::to_string_pretty(&canonical)?;
    Ok(s.into_bytes())
}

fn canonicalize_value(v: serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(m) => {
            let sorted: std::collections::BTreeMap<String, serde_json::Value> = m
                .into_iter()
                .map(|(k, v)| (k, canonicalize_value(v)))
                .collect();
            serde_json::Value::Object(sorted.into_iter().collect())
        }
        serde_json::Value::Array(a) => {
            serde_json::Value::Array(a.into_iter().map(canonicalize_value).collect())
        }
        other => other,
    }
}

/// SHA-256 of arbitrary bytes; lowercase hex.
pub fn digest_sha256(bytes: &[u8]) -> [u8; 32] {
    let h = Sha256::digest(bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(&h);
    out
}

/// Compute a [`BomSubject`] for the given canonical-JSON bytes.
pub fn subject_for(canonical_bytes: &[u8]) -> BomSubject {
    let digest = digest_sha256(canonical_bytes);
    BomSubject {
        name: "envbom".into(),
        digest_alg: "sha256".into(),
        digest_hex: hex::encode(digest),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::envbom::builder::{build_bom_from_inputs, BuildInputs};
    use std::collections::{BTreeMap, BTreeSet};

    fn fixture(now: &str) -> EnvBom {
        let i = BuildInputs {
            project_id: "p",
            profile: None,
            key_pairs: vec![
                ("A".to_string(), Some("alpha".to_string())),
                ("B".to_string(), Some("beta".to_string())),
            ],
            provider_refs: BTreeMap::new(),
            classifications: BTreeMap::new(),
            schema_required: BTreeSet::new(),
            last_rotated: BTreeMap::new(),
            reachable_paths: BTreeMap::new(),
            owners: BTreeMap::new(),
            reproducible_now: Some(now),
        };
        build_bom_from_inputs(&i)
    }

    #[test]
    fn canonical_json_is_pretty_and_parses_back() {
        let bom = fixture("2026-05-08T00:00:00Z");
        let bytes = canonical_json(&bom).unwrap();
        let s = std::str::from_utf8(&bytes).unwrap();
        assert!(s.contains("\"spdx_version\""));
        // round-trip
        let parsed: EnvBom = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed.keys.len(), 2);
    }

    #[test]
    fn canonical_json_is_deterministic_across_calls() {
        let bom = fixture("2026-05-08T00:00:00Z");
        let a = canonical_json(&bom).unwrap();
        let b = canonical_json(&bom).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn digest_sha256_is_known_for_empty() {
        let h = digest_sha256(b"");
        // Known SHA-256 of empty string
        assert_eq!(
            hex::encode(h),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn subject_for_uses_sha256_alg() {
        let bom = fixture("2026-05-08T00:00:00Z");
        let bytes = canonical_json(&bom).unwrap();
        let sub = subject_for(&bytes);
        assert_eq!(sub.digest_alg, "sha256");
        assert_eq!(sub.name, "envbom");
        assert_eq!(sub.digest_hex.len(), 64);
        assert!(sub.digest_hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn predicate_url_is_versioned() {
        assert_eq!(PREDICATE_TYPE_V1, "https://envforge.dev/envbom/v1");
    }

    #[test]
    fn canonicalize_sorts_nested_object_keys() {
        let val: serde_json::Value = serde_json::json!({
            "z": { "b": 1, "a": 2 },
            "a": [1, 2, 3]
        });
        let canon = canonicalize_value(val);
        let s = serde_json::to_string(&canon).unwrap();
        // top-level key order
        let a_pos = s.find("\"a\"").unwrap();
        let z_pos = s.find("\"z\"").unwrap();
        assert!(a_pos < z_pos);
        // nested key order — after canonicalization, inside z: "a":2 comes before "b":1
        let nested_a = s.find("\"a\":2").unwrap();
        let nested_b = s.find("\"b\":1").unwrap();
        assert!(nested_a < nested_b);
    }
}
