//! MCP tool poisoning scanner.
//!
//! Pure stateless pipeline. Detects prompt-injection payloads in MCP
//! tool descriptions, input schemas, and concatenated cross-tool blobs.
//!
//! See:
//! - `bolts/079-poisoning-scanner/ddd-01-domain-model.md`
//! - `bolts/079-poisoning-scanner/ddd-02-technical-design.md`
//! - `bolts/079-poisoning-scanner/adr-019-base64-test-fixtures.md`
//! - `bolts/079-poisoning-scanner/adr-020-no-raw-text-invariant.md`

pub mod blob_scan;
pub mod canonical;
pub mod description_scan;
pub mod emitter;
pub mod error;
pub mod finding;
pub mod patterns;
pub mod schema_scan;

pub use blob_scan::{BlobScanner, ToolDescriptor};
pub use canonical::{CanonicalText, Canonicalizer};
pub use description_scan::DescriptionScanner;
pub use emitter::FindingsEmitter;
pub use error::ScannerError;
pub use finding::{PoisonFinding, Severity};
pub use patterns::{Pattern, PatternKind, PATTERN_SET_VERSION};
pub use schema_scan::{SchemaScanner, ToolSchema};
