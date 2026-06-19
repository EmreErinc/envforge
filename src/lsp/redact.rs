//! Re-export of the shared redaction routine (Story 2.2).
//!
//! The implementation moved to [`crate::ops::redact`] so CLI, LSP, and the
//! MCP server share one choke point (Architecture D5). This re-export keeps
//! every existing `lsp::redact::*` call site and the public path
//! `envforge::lsp::redact::redact_for_label` working unchanged.

pub use crate::ops::redact::{redact_for_label, redact_secrets_in_message};
