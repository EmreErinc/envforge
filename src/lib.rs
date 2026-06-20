#![deny(rustdoc::broken_intra_doc_links)]

//! # EnvForge
//!
//! A Rust CLI + TUI tool for managing environment variables across multiple shell types with
//! encryption, synchronization, and Language Server Protocol (LSP) support.
//!
//! ## Architecture
//!
//! EnvForge is organized into functional modules that separate concerns and enable reuse:
//!
//! - [`model`] — Core data types (`ShellFile`, `LineNode`, `EnvEntry`, `EnvSchema`)
//!   and error types (`ParseError`, `ConfigError`, `CliError`)
//! - [`parser`] — Deterministic shell configuration file parsing and serialization for
//!   bash, zsh, fish, and sh. Maintains round-trip fidelity (parse → serialize produces
//!   byte-identical output)
//! - [`ops`] — Pure business logic for environment variable operations (CRUD, encryption,
//!   synchronization, profiles, exports). No I/O or UI concerns
//! - [`cli`] — Command-line interface with subcommands and argument parsing
//! - [`config`] — Application configuration, backup management, atomic file writes
//! - [`ui`] — Terminal UI built with ratatui and crossterm for interactive management
//! - [`lsp`] — Language Server Protocol implementation for IDE integration
//!
//! ## Getting Started
//!
//! ### Parsing Shell Configuration
//!
//! ```no_run
//! use envforge::parser::parse_shell_file;
//! use std::path::Path;
//!
//! let shell_file = parse_shell_file(Path::new("~/.bashrc"))?;
//! // Access entries, comments, source directives
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ### Managing Environment Variables
//!
//! ```no_run
//! use envforge::parser::parse_shell_file;
//! use envforge::model::LineNode;
//! use std::path::Path;
//!
//! let mut shell_file = parse_shell_file(Path::new("~/.bashrc"))?;
//! // Modify lines, add new entries, and serialize back
//! let output = shell_file.serialize();
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Design Principles
//!
//! 1. **Parser is foundational** — All other modules depend on parser correctness.
//!    Round-trip fidelity ensures shell configs aren't corrupted.
//! 2. **ops/ is pure logic** — Business operations contain no I/O or UI decisions.
//!    This enables easy testing and reuse.
//! 3. **Thin presentation layers** — CLI and UI modules only format and present data
//!    from ops layer.
//! 4. **Shell-agnostic where possible** — Parser handles bash, zsh, fish, sh variations.
//!    Detection logic gracefully handles ambiguous cases.
//!
//! ## Guarantees
//!
//! - **Round-trip fidelity**: Parse a shell config, serialize it, parse again → identical AST
//! - **No secrets in logs**: Sensitive values are redacted by default unless explicitly enabled
//! - **Lock-safety**: RwLock operations in LSP gracefully degrade rather than panic
//! - **Zero panics in library code** — `unwrap()` reserved for tests only

pub mod cli;
pub mod config;
pub mod lsp;
#[cfg(feature = "mcp-server")]
pub mod mcp;
pub mod model;
pub mod ops;
pub mod parser;
pub mod ui;
