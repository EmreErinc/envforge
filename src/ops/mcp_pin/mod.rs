//! MCP supply-chain pin foundation.
//!
//! Owns:
//! - Lockfile schema + serde + atomic persistence (`lockfile`)
//! - Canonical JSON/JSONC hasher (DoS-resistant; ADR-013 state machine)
//! - Binary file hasher with realpath canonicalization + symlink recording
//! - Value objects: `PinMethod`, `PackageManager`, `Transport`, `Platform`
//!
//! Does NOT own: resolution (Unit 002), reputation (Unit 003), CLI (Unit 004),
//! poisoning scan (Unit 005), launch wrappers (Unit 006), monitor/doctor
//! (Unit 007), or UI/docs (Unit 008). All downstream units depend on the
//! types and hashers exposed here.
//!
//! See: `bolts/075-lockfile-hasher/ddd-01-domain-model.md`,
//! `ddd-02-technical-design.md`, `adr-013-jsonc-state-machine.md`,
//! `adr-014-format-version-migration-pattern.md`.

pub mod hasher;
pub mod lockfile;
pub mod resolver;
pub mod types;

pub use hasher::{BinaryHasher, CanonicalJson, CanonicalJsonHasher, HashedBinary, HasherError};
pub use lockfile::pinned_by_machine_id;
pub use lockfile::{
    BinaryHash, FsLockfileRepository, Lockfile, LockfileError, LockfileRepository, LockfileSerde,
    ServerPin, CURRENT_FORMAT_VERSION,
};
pub use types::{PackageManager, PinMethod, Platform, Transport};

pub use resolver::{
    McpConfigFragment, ReputationLookup, ResolveOpts, ResolvedArtifact, Resolver, ResolverError,
    StubReputationLookup, Tier,
};
