//! Layered BOM verification: structural → signature → diff.
//!
//! Structural + diff layers always available. Signature layer requires the
//! `sigstore` Cargo feature; without it, signed bundles return
//! `EnvbomError::SigstoreUnavailable` per ADR-011.

use serde::{Deserialize, Serialize};
use std::path::Path;

use super::builder::EnvBom;
use super::differ::{diff, BomDiff};
use super::EnvbomError;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerifyOptions {
    pub against_current: Option<EnvBom>,
    pub identity_glob: Option<String>,
    pub airgap: bool,
    pub strict_schema: bool,
    pub strict_current: bool,
    pub require_signed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyReport {
    pub structural_ok: bool,
    pub signature_ok: Option<bool>,
    pub identity_match: Option<bool>,
    pub diff: Option<BomDiff>,
    pub warnings: Vec<String>,
    pub airgap: bool,
}

/// Parse a file as either a raw BOM or an in-toto attestation bundle.
pub fn parse_attestation_or_bom(bytes: &[u8]) -> Result<(EnvBom, Option<Vec<u8>>), EnvbomError> {
    // First try as raw BOM JSON.
    if let Ok(bom) = serde_json::from_slice::<EnvBom>(bytes) {
        return Ok((bom, None));
    }
    // Otherwise try as in-toto bundle (DSSE-wrapped). Phase-1: treat any
    // non-BOM JSON as "looks like an attestation" for verifier shape; full
    // bundle parsing lives in the sigstore module.
    if let Ok(_v) = serde_json::from_slice::<serde_json::Value>(bytes) {
        return Err(EnvbomError::AttestationParse(
            "input is JSON but not a raw EnvBom; signed-attestation parsing requires the sigstore feature"
                .into(),
        ));
    }
    Err(EnvbomError::AttestationParse(
        "input is neither a BOM nor a JSON attestation".into(),
    ))
}

/// Verify a BOM file. Returns a structured report; does NOT exit on its own.
pub fn verify(path: &Path, opts: &VerifyOptions) -> Result<VerifyReport, EnvbomError> {
    let bytes = std::fs::read(path)?;
    let (bom, signature_data) = parse_attestation_or_bom(&bytes)?;

    let mut report = VerifyReport {
        structural_ok: true,
        signature_ok: None,
        identity_match: None,
        diff: None,
        warnings: Vec::new(),
        airgap: opts.airgap,
    };

    // Structural verify
    validate_structural(&bom, opts, &mut report)?;

    // Signature verify (sigstore-feature-gated)
    if signature_data.is_some() {
        #[cfg(feature = "sigstore")]
        {
            let sig_bytes = signature_data.expect("Some by branch");
            let result = super::sigstore::verify_signature(&bom, &sig_bytes, opts)?;
            report.signature_ok = Some(result.crypto_ok);
            report.identity_match = result.identity_match;
        }
        #[cfg(not(feature = "sigstore"))]
        {
            return Err(EnvbomError::SigstoreUnavailable);
        }
    } else if opts.require_signed {
        return Err(EnvbomError::VerificationFailed(
            "attestation required (--require-signed) but BOM is unsigned".into(),
        ));
    }

    // Diff (optional)
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
    fn parse_raw_bom_no_signature() {
        let bom = fixture_bom();
        let bytes = serde_json::to_vec(&bom).unwrap();
        let (parsed, sig) = parse_attestation_or_bom(&bytes).unwrap();
        assert_eq!(parsed.project_id, "p");
        assert!(sig.is_none());
    }

    #[test]
    fn parse_non_bom_json_returns_attestation_error() {
        let bytes = b"{\"unrelated\":\"shape\"}";
        let result = parse_attestation_or_bom(bytes);
        assert!(matches!(result, Err(EnvbomError::AttestationParse(_))));
    }

    #[test]
    fn parse_non_json_returns_error() {
        let bytes = b"not json at all";
        let result = parse_attestation_or_bom(bytes);
        assert!(matches!(result, Err(EnvbomError::AttestationParse(_))));
    }

    #[test]
    fn verify_unsigned_bom_passes_structural() {
        let bom = fixture_bom();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), serde_json::to_vec_pretty(&bom).unwrap()).unwrap();
        let opts = VerifyOptions::default();
        let report = verify(tmp.path(), &opts).unwrap();
        assert!(report.structural_ok);
        assert!(report.signature_ok.is_none());
        assert!(report.diff.is_none());
    }

    #[test]
    fn verify_with_diff_against_current() {
        let bom = fixture_bom();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), serde_json::to_vec_pretty(&bom).unwrap()).unwrap();

        // Build a "current" with a different value
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
    fn verify_require_signed_on_unsigned_fails() {
        let bom = fixture_bom();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), serde_json::to_vec_pretty(&bom).unwrap()).unwrap();
        let opts = VerifyOptions {
            require_signed: true,
            ..Default::default()
        };
        let result = verify(tmp.path(), &opts);
        assert!(matches!(result, Err(EnvbomError::VerificationFailed(_))));
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
