//! Two-layer BOM verification: structural → diff.
//!
//! Signing/signature verification was removed. Users who need a
//! signed BOM can wrap the emitted file with an external tool (e.g.
//! `cosign sign-blob`).

use serde::{Deserialize, Serialize};
use std::path::Path;

use super::builder::EnvBom;
use super::differ::{diff, BomDiff};
use super::EnvbomError;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerifyOptions {
    pub against_current: Option<EnvBom>,
    pub strict_schema: bool,
    pub strict_current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyReport {
    pub structural_ok: bool,
    pub diff: Option<BomDiff>,
    pub warnings: Vec<String>,
}

/// Parse a file as a BOM JSON document.
pub fn parse_bom(bytes: &[u8]) -> Result<EnvBom, EnvbomError> {
    serde_json::from_slice::<EnvBom>(bytes)
        .map_err(|e| EnvbomError::AttestationParse(format!("input is not a valid EnvBom: {e}")))
}

/// Verify a BOM file. Returns a structured report; does NOT exit on its own.
pub fn verify(path: &Path, opts: &VerifyOptions) -> Result<VerifyReport, EnvbomError> {
    let bytes = std::fs::read(path)?;
    let bom = parse_bom(&bytes)?;

    let mut report = VerifyReport {
        structural_ok: true,
        diff: None,
        warnings: Vec::new(),
    };

    validate_structural(&bom, opts, &mut report)?;

    if let Some(current) = &opts.against_current {
        let d = diff(&bom, current);
        if opts.strict_current
            && (!d.added.is_empty() || !d.removed.is_empty() || !d.changed.is_empty())
        {
            report.warnings.push(format!(
                "diff has {} changes; --strict-current was requested",
                d.added.len() + d.removed.len() + d.changed.len()
            ));
        }
        report.diff = Some(d);
    }

    Ok(report)
}

fn validate_structural(
    bom: &EnvBom,
    opts: &VerifyOptions,
    report: &mut VerifyReport,
) -> Result<(), EnvbomError> {
    if bom.spdx_id != "SPDXRef-DOCUMENT" {
        return Err(EnvbomError::InvalidBom(format!(
            "spdx_id must be SPDXRef-DOCUMENT (got: {})",
            bom.spdx_id
        )));
    }
    if !bom.spdx_version.starts_with("SPDX-2.") {
        return Err(EnvbomError::InvalidBom(format!(
            "unsupported spdx_version: {}",
            bom.spdx_version
        )));
    }
    if bom.project_id.is_empty() {
        return Err(EnvbomError::InvalidBom("empty project_id".into()));
    }
    if bom.creation_info.creators.is_empty() {
        return Err(EnvbomError::InvalidBom(
            "creation_info.creators must not be empty".into(),
        ));
    }
    let _ = opts.strict_schema;
    let _ = report;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::envbom::builder::{build_bom_from_inputs, BuildInputs};
    use std::collections::{BTreeMap, BTreeSet};

    fn fixture_bom() -> EnvBom {
        let i = BuildInputs {
            project_id: "p",
            profile: None,
            key_pairs: vec![("X".to_string(), Some("v".to_string()))],
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
    fn parse_raw_bom() {
        let bom = fixture_bom();
        let bytes = serde_json::to_vec(&bom).unwrap();
        let parsed = parse_bom(&bytes).unwrap();
        assert_eq!(parsed.project_id, "p");
    }

    #[test]
    fn parse_non_bom_json_returns_error() {
        let bytes = b"{\"unrelated\":\"shape\"}";
        let result = parse_bom(bytes);
        assert!(matches!(result, Err(EnvbomError::AttestationParse(_))));
    }

    #[test]
    fn parse_non_json_returns_error() {
        let bytes = b"not json at all";
        let result = parse_bom(bytes);
        assert!(matches!(result, Err(EnvbomError::AttestationParse(_))));
    }

    #[test]
    fn verify_bom_passes_structural() {
        let bom = fixture_bom();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), serde_json::to_vec_pretty(&bom).unwrap()).unwrap();
        let opts = VerifyOptions::default();
        let report = verify(tmp.path(), &opts).unwrap();
        assert!(report.structural_ok);
        assert!(report.diff.is_none());
    }

    #[test]
    fn verify_with_diff_against_current() {
        let bom = fixture_bom();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), serde_json::to_vec_pretty(&bom).unwrap()).unwrap();

        let i = BuildInputs {
            project_id: "p",
            profile: None,
            key_pairs: vec![("X".to_string(), Some("changed".to_string()))],
            provider_refs: BTreeMap::new(),
            classifications: BTreeMap::new(),
            schema_required: BTreeSet::new(),
            last_rotated: BTreeMap::new(),
            reachable_paths: BTreeMap::new(),
            owners: BTreeMap::new(),
            reproducible_now: Some("2026-05-08T00:00:00Z"),
        };
        let current = build_bom_from_inputs(&i);

        let opts = VerifyOptions {
            against_current: Some(current),
            ..Default::default()
        };
        let report = verify(tmp.path(), &opts).unwrap();
        assert!(report.diff.is_some());
        let d = report.diff.unwrap();
        assert_eq!(d.changed.len(), 1);
    }

    #[test]
    fn invalid_spdx_id_fails_structural() {
        let mut bom = fixture_bom();
        bom.spdx_id = "Wrong".into();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), serde_json::to_vec_pretty(&bom).unwrap()).unwrap();
        let result = verify(tmp.path(), &VerifyOptions::default());
        assert!(matches!(result, Err(EnvbomError::InvalidBom(_))));
    }

    #[test]
    fn empty_project_id_fails_structural() {
        let mut bom = fixture_bom();
        bom.project_id = String::new();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), serde_json::to_vec_pretty(&bom).unwrap()).unwrap();
        let result = verify(tmp.path(), &VerifyOptions::default());
        assert!(matches!(result, Err(EnvbomError::InvalidBom(_))));
    }
}
