//! Sigstore signing path. Phase-1 stub returning typed errors; Phase-2 wires
//! up `sigstore-rs` for keyless OIDC signing per ADR-011.
//!
//! This file is only compiled when the `sigstore` Cargo feature is enabled.

use super::builder::EnvBom;
use super::verifier::VerifyOptions;
use super::EnvbomError;

#[derive(Debug, Clone)]
pub struct SignOptions {
    pub identity_token: Option<String>,
    pub fulcio_url: Option<String>,
    pub rekor_url: Option<String>,
}

impl Default for SignOptions {
    fn default() -> Self {
        Self {
            identity_token: None,
            fulcio_url: None,
            rekor_url: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VerifyResult {
    pub crypto_ok: bool,
    pub identity_match: Option<bool>,
}

/// Sign a BOM via Sigstore Cosign keyless flow.
/// Phase-1: returns `SigstoreUnavailable` placeholder; full implementation in Phase-2 intent.
pub fn sign_bom(_bom_bytes: &[u8], _opts: SignOptions) -> Result<Vec<u8>, EnvbomError> {
    Err(EnvbomError::SigstoreUnavailable)
}

/// Verify the cryptographic signature on an attestation bundle.
/// Phase-1: returns a structured error so the verifier surface is testable.
pub fn verify_signature(
    _bom: &EnvBom,
    _signature_bytes: &[u8],
    _opts: &VerifyOptions,
) -> Result<VerifyResult, EnvbomError> {
    Err(EnvbomError::VerificationFailed(
        "sigstore phase-1 stub: real signature verification ships in a future intent".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_returns_unavailable_placeholder() {
        let r = sign_bom(b"{}", SignOptions::default());
        assert!(matches!(r, Err(EnvbomError::SigstoreUnavailable)));
    }

    #[test]
    fn verify_signature_returns_structured_error() {
        use crate::ops::envbom::builder::{build_bom_from_inputs, BuildInputs};
        use std::collections::{BTreeMap, BTreeSet};
        let bom = build_bom_from_inputs(&BuildInputs {
            project_id: "p",
            profile: None,
            key_pairs: vec![],
            provider_refs: BTreeMap::new(),
            classifications: BTreeMap::new(),
            schema_required: BTreeSet::new(),
            last_rotated: BTreeMap::new(),
            reachable_paths: BTreeMap::new(),
            owners: BTreeMap::new(),
            reproducible_now: Some("2026-05-08T00:00:00Z"),
        });
        let r = verify_signature(&bom, b"sig", &VerifyOptions::default());
        assert!(matches!(r, Err(EnvbomError::VerificationFailed(_))));
    }
}
