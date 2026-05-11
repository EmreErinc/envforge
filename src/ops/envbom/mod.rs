//! ENV-BOM (Environment Bill of Materials) — signed manifest of env keys.
//!
//! See `memory-bank/bolts/074-envbom-attestation/` for the design docs and ADRs:
//! - ADR-011: Sigstore behind `--features sigstore` Cargo feature (default off)
//! - ADR-012: Custom predicate URL `https://envforge.dev/envbom/v1`
//!
//! Phase-1 ship surface (default build):
//! - `envforge envbom emit` (unsigned)
//! - `envforge envbom verify` (structural + diff)
//! - `envforge envbom verify --airgap` (offline cert + Rekor proof against bundled root)
//!
//! Phase-2 ship surface (with `--features sigstore`): adds keyless Cosign signing.

pub mod airgap;
pub mod builder;
pub mod differ;
pub mod serializer;
pub mod verifier;

#[cfg(feature = "sigstore")]
pub mod sigstore;

pub use airgap::{AirgapTrustRoot, TrustRootSource};
pub use builder::{
    build_bom, AuditSummary, Classification, ClassifiedCounts, CreationInfo, EnvBom, EnvBomKey,
    PathKind, PathRef, ValueState,
};
pub use differ::{diff, BomDiff, ChangedField, KeyChange};
pub use serializer::{canonical_json, digest_sha256, BomSubject, PREDICATE_TYPE_V1};
pub use verifier::{verify, VerifyOptions, VerifyReport};

/// Errors specific to the envbom flow.
#[derive(Debug, thiserror::Error)]
pub enum EnvbomError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("invalid bom: {0}")]
    InvalidBom(String),
    #[error("attestation parse: {0}")]
    AttestationParse(String),
    #[error("verification failed: {0}")]
    VerificationFailed(String),
    #[error("sigstore feature not enabled (rebuild with --features sigstore)")]
    SigstoreUnavailable,
    #[error("oidc resolution failed: {0}")]
    OidcResolution(String),
    #[error("trust root: {0}")]
    TrustRoot(String),
    #[error("op error: {0}")]
    OpError(#[from] super::OpError),
}
