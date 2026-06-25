//! MCP supply-chain pin foundation.
//!
//! Owns:
//! - Lockfile schema + serde + atomic persistence (`lockfile`)
//! - Canonical JSON/JSONC hasher (DoS-resistant state machine)
//! - Binary file hasher with realpath canonicalization + symlink recording
//! - Value objects: `PinMethod`, `PackageManager`, `Transport`, `Platform`

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
