//! ENV-BOM (Environment Bill of Materials) — manifest of env keys.
//!
//! Ship surface:
//! - `envforge envbom emit` — deterministic SPDX-shaped BOM with custom predicate
//! - `envforge envbom verify` — structural validation + optional diff vs current state
//!
//! Predicate URL: `https://envforge.dev/envbom/v1` (custom; SPDX shape with
//! EnvForge-specific extensions for `keys` field + `audit_summary` aggregation).
//!
//! Note: Signing was removed. Consumers who need
//! signed BOMs can pipe the output of `envforge envbom emit` through `cosign
//! sign-blob` externally.

pub mod builder;
pub mod differ;
pub mod serializer;
pub mod verifier;

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
    #[error("op error: {0}")]
    OpError(#[from] super::OpError),
}
