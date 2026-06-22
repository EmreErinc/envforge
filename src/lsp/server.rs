use std::collections::HashMap;
use std::sync::RwLock;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use zeroize::Zeroize;

use crate::ops::config_format::{ConfigEntry, SourceLayer, WriteCapability};
use crate::ops::schema::{parse_schema_content, EnvSchema};
use crate::ops::schema_unification::{
    cross_format_diagnostics_to_lsp, cross_format_entry_diagnostics, cross_format_find_references,
    cross_format_goto_definition, missing_required_diagnostics, UnifiedSchema,
};

use super::ai_guard_diagnostics::compute_ai_guard_diagnostics;
use super::code_action::code_actions;
use super::code_lens::code_lenses;
use super::completion::completions;
use super::config_features::{
    config_diagnostics, config_format_text_edits, config_hover, config_jsonc_diagnostics,
    config_jsonc_rename, config_semantic_tokens, config_toml_diagnostics,
    config_toml_format_text_edits, config_toml_rename, config_yaml_diagnostics,
    config_yaml_format_text_edits, config_yaml_rename,
};
use super::config_file::{
    format_for_uri, is_appsettings_file, is_config_format_file, is_toml_config_file,
    is_yaml_config_file,
};
use super::definition::{
    extract_upper_snake_identifier, goto_definition, goto_definition_from_source,
};
use super::diagnostics::compute_diagnostics;
use super::document::{parse_env_document, schema_line_map, DocumentState};
use super::document_symbol::document_symbols;
use super::exposure::{compute_config_exposure_map, compute_exposure_map, ExposureEntry};
use super::folding_range::compute_folding_ranges;
use super::format::format_text_edits;
use super::hover::hover_info;
use super::inlay::compute_inlay_hints;
use super::mcp_diagnostics::{compute_mcp_diagnostics, mcp_config_code_actions};
use super::rate_limit::RateLimiters;
use super::references::find_references;
use super::rename::build_rename_edit;
use super::security::{LspAuditLogger, LspSecurityPolicy};
use super::semantic_tokens::{compute_semantic_tokens, TOKEN_MODIFIERS, TOKEN_TYPES};
use super::workspace_symbol::workspace_symbols;

/// A known env var from envforge's managed files.
/// Values are intentionally NOT stored here — consumers must look up
/// values on-demand from the document state or the `reveal.value` command.
#[derive(Debug, Clone)]
pub struct ManagedVar {
    pub key: String,
    pub source_file: String,
}

/// State for an open config-format document (`.properties`, `.env`-cascade).
#[derive(Debug, Clone)]
pub struct ConfigDocumentState {
    pub content: String,
    pub version: i32,
    pub entries: Vec<ConfigEntry>,
    pub source_layer: SourceLayer,
    pub write_capability: WriteCapability,
}

pub struct Backend {
    client: Client,
    documents: RwLock<HashMap<Url, DocumentState>>,
    /// Tracked open config-format documents (`.properties` / `.env`-cascade).
    /// Kept separate from `documents` so existing env-file handlers are
    /// completely unaffected.
    config_documents: RwLock<HashMap<Url, ConfigDocumentState>>,
    schema: RwLock<Option<EnvSchema>>,
    schema_uri: RwLock<Option<Url>>,
    schema_lines: RwLock<HashMap<String, u32>>,
    schema_line_count: RwLock<Option<u32>>,
    workspace_root: RwLock<Option<Url>>,
    managed_vars: RwLock<Vec<ManagedVar>>,
    rate_limiters: RateLimiters,
    security_policy: LspSecurityPolicy,
    audit_logger: LspAuditLogger,
    /// Last LSP method name for audit attribution of security-relevant
    /// side effects (reveal, fence toggle, sync push, etc.). Tracked so
    /// the audit log records which LSP endpoint triggered the mutation.
    request_method: RwLock<String>,
}

impl Backend {
    fn new(client: Client) -> Self {
        let audit_logger = LspAuditLogger::new().expect("failed to initialize LSP audit logger");

        Self {
            client,
            documents: RwLock::new(HashMap::new()),
            config_documents: RwLock::new(HashMap::new()),
            schema: RwLock::new(None),
            schema_uri: RwLock::new(None),
            schema_lines: RwLock::new(HashMap::new()),
            schema_line_count: RwLock::new(None),
            workspace_root: RwLock::new(None),
            managed_vars: RwLock::new(Vec::new()),
            rate_limiters: RateLimiters::default(),
            security_policy: LspSecurityPolicy::default(),
            audit_logger,
            request_method: RwLock::new(String::new()),
        }
    }

    /// Maximum size of a single LSP document we will parse / store.
    /// Editor clients normally ship single-digit-KB files; megabytes
    /// here is a sign of a malicious or buggy client. Without this
    /// cap, a 1 GiB `did_open` payload OOMs the parser. Mirrors
    /// `MAX_SCHEMA_BYTES` in `load_schema_from_workspace`.
    const MAX_DOCUMENT_BYTES: usize = 1024 * 1024;

    fn is_env_file(uri: &Url) -> bool {
        let path = uri.path();
        let fname = path.rsplit('/').next().unwrap_or("");
        fname == ".env" || fname.starts_with(".env.") || fname.ends_with(".env") || fname == "env"
    }

    fn is_schema_file(uri: &Url) -> bool {
        let p = uri.path();
        p.ends_with(".env.schema") || p.ends_with(".env.schema.toml")
    }

    fn is_fenced_env_file(&self, uri: &Url) -> bool {
        let root = match self.workspace_root.read() {
            Ok(r) => r.clone(),
            Err(_) => return false,
        };
        let Some(root_url) = root else { return false };
        let Ok(root_path) = root_url.to_file_path() else {
            return false;
        };
        let Ok(file_path) = uri.to_file_path() else {
            return false;
        };
        let Ok(canonical_file) = std::fs::canonicalize(&file_path) else {
            return false;
        };
        let Ok(canonical_root) = std::fs::canonicalize(&root_path) else {
            return false;
        };
        if !canonical_file.starts_with(&canonical_root) {
            return false;
        }
        crate::ops::fence::check_fence_status(&canonical_root)
            .map(|status| status.all_fenced)
            .unwrap_or(false)
    }

    /// Identify MCP configuration files we want to lint inline.
    /// Conservative: only known config filenames in known directories,
    /// not arbitrary `*.json`. Keeps the diagnostic from misfiring on
    /// unrelated JSON files in a workspace.
    fn is_mcp_config_file(uri: &Url) -> bool {
        let path = uri.path();
        let fname = path.rsplit('/').next().unwrap_or("");
        // Cross-tool MCP/agent config filenames (matches `mcp.json` in any
        // dir, so `.vscode/mcp.json` and `.cursor/mcp.json` are covered).
        // Story 3.1 (FR18) widens coverage to Windsurf, Cline, Claude Code.
        if matches!(
            fname,
            "mcp.json"
                | ".mcp.json"
                | "claude_desktop_config.json"
                | ".claude.json"          // Claude Code user config
                | "mcp_config.json"       // Windsurf (Cascade)
                | "cline_mcp_settings.json" // Cline
        ) {
            return true;
        }
        // .claude/settings.json under any parent (Claude Code deny rules etc.)
        path.contains("/.claude/") && fname == "settings.json"
    }

    fn publish_mcp_diagnostics(&self, uri: &Url, content: &str, version: i32) {
        let file_path = if let Ok(p) = uri.to_file_path() {
            p
        } else {
            eprintln!(
                "LSP: mcp diagnostics skipped — URI cannot convert to file path: {}",
                uri
            );
            return;
        };
        let root = self
            .workspace_root
            .read()
            .ok()
            .and_then(|r| r.clone())
            .and_then(|url| url.to_file_path().ok());
        if let Some(root_path) = root {
            let Ok(canonical_file) = std::fs::canonicalize(&file_path) else {
                eprintln!(
                    "LSP: mcp diagnostics skipped — cannot canonicalize path: {}",
                    file_path.display()
                );
                return;
            };
            let Ok(canonical_root) = std::fs::canonicalize(&root_path) else {
                eprintln!(
                    "LSP: mcp diagnostics skipped — cannot canonicalize workspace root: {}",
                    root_path.display()
                );
                return;
            };
            if !canonical_file.starts_with(&canonical_root) {
                eprintln!(
                    "LSP: mcp diagnostics blocked — file outside workspace: {}",
                    canonical_file.display()
                );
                return;
            }
        }
        let diags = compute_mcp_diagnostics(content, &file_path);
        let client = self.client.clone();
        let uri = uri.clone();
        tokio::spawn(async move {
            client.publish_diagnostics(uri, diags, Some(version)).await;
        });
    }

    fn load_schema_from_workspace(&self) {
        let root = self.workspace_root.read().ok().and_then(|r| r.clone());
        if let Some(root_url) = root {
            if let Ok(root_path) = root_url.to_file_path() {
                // Canonicalize the client-supplied workspace root before
                // reading from it. Without this, a malicious editor /
                // LSP client could pass a `rootUri` containing `..` or
                // a symlink that escapes the intended workspace and
                // trick us into opening a file the user did not consent
                // to load.
                let root_path = match std::fs::canonicalize(&root_path) {
                    Ok(p) => p,
                    Err(_) => return,
                };
                // Prefer .env.schema.toml; fall back to legacy .env.schema.
                let schema_path = {
                    let toml_path = root_path.join(".env.schema.toml");
                    if toml_path.exists() {
                        toml_path
                    } else {
                        root_path.join(".env.schema")
                    }
                };
                // Resolve `schema_path` and require it stays under
                // `root_path` (defense-in-depth — the join with a literal
                // filename doesn't traverse, but we guard anyway).
                let canonical_schema = match std::fs::canonicalize(&schema_path) {
                    Ok(p) => p,
                    Err(_) => return,
                };
                if !canonical_schema.starts_with(&root_path) {
                    eprintln!("LSP: refusing to read schema outside workspace root");
                    return;
                }
                // Cap file size to defend against a 100 MB schema crashing the server.
                const MAX_SCHEMA_BYTES: u64 = 1024 * 1024;
                if let Ok(meta) = std::fs::metadata(&canonical_schema) {
                    if meta.len() > MAX_SCHEMA_BYTES {
                        eprintln!(
                            "LSP: schema {} exceeds {}-byte limit; refusing to load",
                            canonical_schema.display(),
                            MAX_SCHEMA_BYTES
                        );
                        return;
                    }
                }
                if canonical_schema.exists() {
                    if let Ok(content) = std::fs::read_to_string(&canonical_schema) {
                        let lines = schema_line_map(&content);
                        let line_count = content.lines().count() as u32;
                        if let Ok(schema) = parse_schema_content(&content) {
                            if let Ok(mut w) = self.schema.write() {
                                *w = Some(schema);
                            } else {
                                eprintln!(
                                    "LSP: Failed to acquire write lock on schema (lock poisoned)"
                                );
                            }
                            if let Ok(mut w) = self.schema_lines.write() {
                                *w = lines;
                            } else {
                                eprintln!("LSP: Failed to acquire write lock on schema_lines (lock poisoned)");
                            }
                            if let Ok(mut w) = self.schema_line_count.write() {
                                *w = Some(line_count);
                            }
                            if let Ok(uri) = Url::from_file_path(&canonical_schema) {
                                if let Ok(mut w) = self.schema_uri.write() {
                                    *w = Some(uri);
                                } else {
                                    eprintln!("LSP: Failed to acquire write lock on schema_uri (lock poisoned)");
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn load_managed_vars(&self) {
        let config = match crate::config::load_or_create_default() {
            Ok(c) => c,
            Err(_) => return,
        };
        let primary_path = shellexpand(&config.files.primary);
        let ref_path = shellexpand(&config.files.reference);

        let mut shell_files = Vec::new();
        if primary_path.exists() {
            if let Ok(sf) = crate::parser::parse_shell_file(&primary_path) {
                shell_files.push(sf);
            }
        }
        if config.files.use_reference_file && ref_path.exists() {
            if let Ok(sf) = crate::parser::parse_shell_file(&ref_path) {
                shell_files.push(sf);
            }
        }

        if shell_files.is_empty() {
            return;
        }

        let entries = crate::ops::collect_all_entries(&shell_files);
        let managed: Vec<ManagedVar> = entries
            .into_iter()
            .map(|e| ManagedVar {
                key: e.key,
                source_file: e.source_file.to_string_lossy().into_owned(),
            })
            .collect();

        if let Ok(mut w) = self.managed_vars.write() {
            *w = managed;
        } else {
            eprintln!("LSP: Failed to acquire write lock on managed_vars (lock poisoned)");
        }
    }

    /// Read a non-env source file from disk for goto-definition lookups
    /// originating outside `.env*` / schema files. Enforces:
    /// - File must canonicalize successfully and stay inside the
    ///   canonicalized workspace root (defends against `..`, symlink
    ///   escape, or a malicious `textDocument/definition` URI).
    /// - File extension must be on the allow-list of common source
    ///   languages; we never read binaries or arbitrary user data.
    /// - Size capped to `MAX_DOCUMENT_BYTES`.
    fn read_source_text_for_uri(&self, uri: &Url) -> Option<String> {
        let path = uri.to_file_path().ok()?;
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())?;
        if !matches!(
            ext.as_str(),
            "ts" | "tsx"
                | "js"
                | "jsx"
                | "mjs"
                | "cjs"
                | "py"
                | "rs"
                | "go"
                | "java"
                | "kt"
                | "rb"
                | "php"
                | "cs"
                | "sh"
        ) {
            return None;
        }

        let canonical = std::fs::canonicalize(&path).ok()?;

        let root = self.workspace_root.read().ok().and_then(|r| r.clone())?;
        let root_path = root.to_file_path().ok()?;
        let canonical_root = std::fs::canonicalize(&root_path).ok()?;
        if !canonical.starts_with(&canonical_root) {
            return None;
        }

        let meta = std::fs::metadata(&canonical).ok()?;
        if meta.len() > Self::MAX_DOCUMENT_BYTES as u64 {
            return None;
        }

        std::fs::read_to_string(&canonical).ok()
    }

    /// Zeroize and clear a `ConfigDocumentState` in place.
    /// Used by both `did_close` and the fence-activation purge path (C-2).
    fn zeroize_config_state(state: &mut ConfigDocumentState) {
        state.content.zeroize();
        for entry in &mut state.entries {
            entry.key.zeroize();
            entry.value.zeroize();
        }
    }

    /// Record the current LSP method for audit attribution. Called at
    /// the top of every handler so that security-relevant side effects
    /// (reveal, fence toggle, sync push) carry the originating endpoint
    /// in their audit log entries.
    fn set_request_method(&self, method: &str) {
        if let Ok(mut m) = self.request_method.write() {
            *m = method.to_string();
        }
    }

    /// Compute the AI-exposure map for a given env-file URI. Reads the
    /// document state, current schema, and fence status (probed live
    /// from disk on each call — `.envforgeignore` etc. can be edited by
    /// other tools without going through us). Returns an empty vec for
    /// unknown URIs or non-env files rather than erroring so plugin
    /// clients can safely poll on every keystroke.
    pub fn exposure_for(&self, uri: &Url) -> Vec<ExposureEntry> {
        // Resolve fence status once; shared by both the .env and config paths.
        let fence_active = self
            .workspace_root
            .read()
            .ok()
            .and_then(|r| r.clone())
            .and_then(|url| url.to_file_path().ok())
            .and_then(|root| crate::ops::fence::check_fence_status(&root).ok())
            .map(|status| status.all_fenced)
            .unwrap_or(false);

        // H-1: when the fence is active, return empty before reading any
        // document store — exposure data must never leak key names or canary
        // annotations to callers while the fence is up.
        if fence_active {
            return Vec::new();
        }

        // FR20: recognized config files (properties/.env-cascade/YAML) are
        // counted in the exposure map alongside .env (AR7 — reuse engine).
        if is_config_format_file(uri) {
            let cfg_doc = self
                .config_documents
                .read()
                .ok()
                .and_then(|docs| docs.get(uri).cloned());
            if let Some(cfg_doc) = cfg_doc {
                let schema = self.schema.read().ok().and_then(|r| r.clone());
                return compute_config_exposure_map(
                    &cfg_doc.entries,
                    schema.as_ref(),
                    fence_active,
                );
            }
            return Vec::new();
        }

        if !Self::is_env_file(uri) {
            return Vec::new();
        }
        let doc = self
            .documents
            .read()
            .ok()
            .and_then(|docs| docs.get(uri).cloned());
        let Some(doc) = doc else { return Vec::new() };

        let schema = self.schema.read().ok().and_then(|r| r.clone());
        compute_exposure_map(&doc.entries, schema.as_ref(), fence_active)
    }

    fn publish_diagnostics_for(&self, uri: &Url, doc: &DocumentState) {
        let schema = self.schema.read().ok().and_then(|r| r.clone());
        let diags = compute_diagnostics(&doc.entries, schema.as_ref());
        let client = self.client.clone();
        let uri = uri.clone();
        let version = doc.version;
        tokio::spawn(async move {
            client.publish_diagnostics(uri, diags, Some(version)).await;
        });
    }

    /// Publish the union of schema diagnostics plus the AI-guard prompt
    /// injection scan. Called on `did_save` rather than `did_change` so
    /// the heavier scanner does not run on every keystroke and so users
    /// only see the warnings on intentional save points (pasted content
    /// from outside sources is the high-risk surface for injection).
    fn publish_diagnostics_with_ai_guard(&self, uri: &Url, doc: &DocumentState) {
        let schema = self.schema.read().ok().and_then(|r| r.clone());
        let mut diags = compute_diagnostics(&doc.entries, schema.as_ref());
        diags.extend(compute_ai_guard_diagnostics(&doc.content));
        let client = self.client.clone();
        let uri = uri.clone();
        let version = doc.version;
        tokio::spawn(async move {
            client.publish_diagnostics(uri, diags, Some(version)).await;
        });
    }

    /// Returns `true` when the document's source layer is a `.env`-cascade
    /// variant (`.env.local` / `.env.{environment}`). Used by M-A to decide
    /// whether to union in the AI-guard prompt-injection scan.
    fn is_dotenv_cascade_doc(doc: &ConfigDocumentState) -> bool {
        matches!(
            doc.source_layer,
            SourceLayer::DotEnvLocal | SourceLayer::DotEnvEnvironment(_)
        )
    }

    /// Publish diagnostics for an open config-format document.
    ///
    /// For read-only YAML documents, uses the YAML-specific diagnostic pipeline
    /// (which surfaces parse errors, duplicate keys, and unterminated `${}`).
    /// For read-write formats (`.properties`, `.env`-cascade), uses the existing
    /// flat-entry diagnostic pipeline.
    ///
    /// FR2 (Intent 040): also composes cross-format diagnostics (unknown-key,
    /// type-mismatch, missing-required) from `UnifiedSchema`. Cross-format
    /// diagnostics are deduplicated against per-format ones by (range, message)
    /// so the same warning never appears twice.
    ///
    /// M-A: dotenv-cascade files also run the AI-guard prompt-injection scan
    /// (they carry secrets too — same attack surface as plain `.env`).
    fn publish_config_diagnostics_for(&self, uri: &Url, doc: &ConfigDocumentState) {
        let schema = self.schema.read().ok().and_then(|r| r.clone());

        // Dispatch diagnostics by format (H-1 fix: format-specific functions
        // use canonical-key matching so JSONC/TOML keys aren't falsely flagged).
        let mut diags = if is_yaml_config_file(uri) {
            // YAML: use the YAML-specific diagnostic pipeline (parse errors,
            // duplicate keys, unterminated ${}).
            config_yaml_diagnostics(&doc.content, doc.source_layer.clone())
        } else if is_toml_config_file(uri) {
            // TOML: use the TOML-specific diagnostic pipeline (parse errors,
            // AoT-aware duplicate keys, schema type-check with canonical matching).
            config_toml_diagnostics(&doc.content, doc.source_layer.clone(), schema.as_ref())
        } else if is_appsettings_file(uri) {
            // JSONC: use the JSONC-specific diagnostic pipeline (parse errors,
            // duplicate keys, schema with canonical matching).
            config_jsonc_diagnostics(&doc.content, doc.source_layer.clone(), schema.as_ref())
        } else {
            // .properties / .env-cascade: flat-entry diagnostic pipeline.
            config_diagnostics(&doc.entries, schema.as_ref())
        };

        // FR2 (Intent 040): cross-format diagnostics via UnifiedSchema.
        // Build the schema once; skip if no schema is loaded.
        if let Some(raw_schema) = schema {
            let unified = UnifiedSchema::new(raw_schema);

            // Per-entry cross-format diagnostics (unknown-key, type-mismatch).
            let cross_entry_diags = cross_format_entry_diagnostics(&doc.entries, &unified);

            // Gather all open config entries for missing-required check.
            let all_open_entries: Vec<Vec<ConfigEntry>> = self
                .config_documents
                .read()
                .ok()
                .map(|m| m.values().map(|d| d.entries.clone()).collect())
                .unwrap_or_default();
            let all_refs: Vec<&[ConfigEntry]> =
                all_open_entries.iter().map(Vec::as_slice).collect();
            let missing_diags = missing_required_diagnostics(&all_refs, &unified);

            // Convert and deduplicate: skip cross-format diag if per-format
            // already emitted a diagnostic at the same (range, message prefix).
            let existing_keys: std::collections::HashSet<(u32, u32, String)> = diags
                .iter()
                .map(|d| {
                    (
                        d.range.start.line,
                        d.range.start.character,
                        d.message.clone(),
                    )
                })
                .collect();

            let mut combined_cross: Vec<_> = cross_entry_diags;
            combined_cross.extend(missing_diags);
            let new_lsp = cross_format_diagnostics_to_lsp(&combined_cross);
            for d in new_lsp {
                let key = (
                    d.range.start.line,
                    d.range.start.character,
                    d.message.clone(),
                );
                if !existing_keys.contains(&key) {
                    diags.push(d);
                }
            }
        }

        // M-A: union AI-guard diagnostics for dotenv-cascade files.
        if Self::is_dotenv_cascade_doc(doc) {
            diags.extend(compute_ai_guard_diagnostics(&doc.content));
        }
        let client = self.client.clone();
        let uri = uri.clone();
        let version = doc.version;
        tokio::spawn(async move {
            client.publish_diagnostics(uri, diags, Some(version)).await;
        });
    }

    /// Re-publish diagnostics for all open documents (plain `.env` files and
    /// config-format documents). Called when the schema changes so editors see
    /// updated unknown-key / type diagnostics without reopening files.
    ///
    /// M-B: also iterates `config_documents` so properties/yaml/.env.local files
    /// get refreshed diagnostics when `.env.schema` is edited.
    fn republish_all(&self) {
        // Plain .env documents.
        if let Ok(docs) = self.documents.read() {
            let uris: Vec<Url> = docs.keys().cloned().collect();
            drop(docs);
            for uri in uris {
                if let Ok(doc_map) = self.documents.read() {
                    if let Some(doc) = doc_map.get(&uri) {
                        self.publish_diagnostics_for(&uri, doc);
                    }
                }
            }
        }
        // Config-format documents (.properties / .env-cascade / YAML) — M-B.
        if let Ok(cfg_docs) = self.config_documents.read() {
            let uris: Vec<Url> = cfg_docs.keys().cloned().collect();
            drop(cfg_docs);
            for uri in uris {
                if let Ok(cfg_map) = self.config_documents.read() {
                    if let Some(doc) = cfg_map.get(&uri) {
                        self.publish_config_diagnostics_for(&uri, doc);
                    }
                }
            }
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        if let Some(root_uri) = params.root_uri {
            if let Ok(mut w) = self.workspace_root.write() {
                *w = Some(root_uri);
            }
        } else if let Some(folders) = params.workspace_folders {
            if let Some(folder) = folders.first() {
                if let Ok(mut w) = self.workspace_root.write() {
                    *w = Some(folder.uri.clone());
                }
            }
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec!["=".into(), "$".into(), "{".into()]),
                    ..Default::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                code_lens_provider: Some(CodeLensOptions {
                    resolve_provider: Some(false),
                }),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                inlay_hint_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                // execute_command_provider intentionally disabled.
                // The LSP is a read-only security boundary. All mutations
                // (fence, canary, sync, reveal) must go through the CLI.
                // Advertising workspace/executeCommand opens a remote-execution
                // surface that contradicts envforge's zero-trust posture.
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            work_done_progress_options: WorkDoneProgressOptions::default(),
                            legend: SemanticTokensLegend {
                                token_types: TOKEN_TYPES.to_vec(),
                                token_modifiers: TOKEN_MODIFIERS.to_vec(),
                            },
                            range: Some(false),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                        },
                    ),
                ),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "envforge-lsp".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.load_schema_from_workspace();
        self.load_managed_vars();
        self.client
            .log_message(MessageType::INFO, "envforge LSP initialized")
            .await;

        let managed_count = self.managed_vars.read().map(|m| m.len()).unwrap_or(0);
        let root_path_opt = self
            .workspace_root
            .read()
            .ok()
            .and_then(|r| r.clone())
            .and_then(|u| u.to_file_path().ok());
        if managed_count > 0 {
            if let Some(root_path) = root_path_opt {
                match crate::ops::fence::check_fence_status(&root_path) {
                    Ok(status) if !status.all_fenced => {
                        match crate::ops::fence::create_fence(&root_path, false) {
                            Ok(_) => {
                                self.client
                                    .log_message(
                                        MessageType::INFO,
                                        "envforge: auto-enabled AI secret fence",
                                    )
                                    .await;
                            }
                            Err(e) => {
                                eprintln!("LSP: failed to auto-create fence: {}", e);
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("LSP: fence status check failed: {}", e);
                    }
                }
            }
        }
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;

        if !self.rate_limiters.did_open.try_consume(1) {
            eprintln!("LSP: rate limit exceeded for did_open");
            return;
        }
        self.set_request_method("textDocument/did_open");

        // Cap document size before parsing. Without this, a malicious or
        // buggy client can ship a multi-GB `text` and OOM us.
        if params.text_document.text.len() > Self::MAX_DOCUMENT_BYTES {
            eprintln!(
                "LSP: refusing did_open for {} ({} bytes > {})",
                uri,
                params.text_document.text.len(),
                Self::MAX_DOCUMENT_BYTES
            );
            return;
        }

        if Self::is_schema_file(&uri) {
            let content = &params.text_document.text;
            let lines = schema_line_map(content);
            let line_count = content.lines().count() as u32;
            if let Ok(schema) = parse_schema_content(content) {
                if let Ok(mut w) = self.schema.write() {
                    *w = Some(schema);
                }
                if let Ok(mut w) = self.schema_lines.write() {
                    *w = lines;
                }
                if let Ok(mut w) = self.schema_line_count.write() {
                    *w = Some(line_count);
                }
                if let Ok(mut w) = self.schema_uri.write() {
                    *w = Some(uri);
                }
                self.republish_all();
            }
            return;
        }

        if Self::is_mcp_config_file(&uri) {
            self.publish_mcp_diagnostics(
                &uri,
                &params.text_document.text,
                params.text_document.version,
            );
            return;
        }

        // Config-format files (.properties, .env-cascade) — new handler.
        // Checked BEFORE is_env_file so cascade .env files get config
        // features (FR4) while plain .env still falls through to the
        // existing env handler when not matched.
        if is_config_format_file(&uri) {
            // FR18/NFR5: Fence enforcement at parity with .env files.
            // Config files that carry secrets must be refused when the
            // workspace fence is active — identical to the is_fenced_env_file
            // guard that protects plain .env files below.
            if self.is_fenced_env_file(&uri) {
                eprintln!(
                    "LSP: refusing did_open for {} — workspace is fenced (config file)",
                    uri
                );
                return;
            }
            if let Some((fmt, layer)) = format_for_uri(&uri) {
                let entries = fmt.parse(&params.text_document.text, layer.clone());
                let doc = ConfigDocumentState {
                    content: params.text_document.text.clone(),
                    version: params.text_document.version,
                    entries,
                    source_layer: layer,
                    write_capability: fmt.write_capability(),
                };
                self.publish_config_diagnostics_for(&uri, &doc);
                if let Ok(mut w) = self.config_documents.write() {
                    let max_docs = self.security_policy.max_tracked_documents;
                    if w.len() >= max_docs {
                        if let Some(oldest) = w.keys().next().cloned() {
                            w.remove(&oldest);
                        }
                    }
                    w.insert(uri, doc);
                }
                return;
            }
        }

        if !Self::is_env_file(&uri) {
            return;
        }

        if self.is_fenced_env_file(&uri) {
            eprintln!("LSP: refusing did_open for {} — workspace is fenced", uri);
            return;
        }

        let entries = parse_env_document(&params.text_document.text);
        let doc = DocumentState {
            content: params.text_document.text.clone(),
            version: params.text_document.version,
            entries,
        };
        self.publish_diagnostics_for(&uri, &doc);
        if let Ok(mut w) = self.documents.write() {
            // Cap tracked document count to prevent memory exhaustion.
            let max_docs = self.security_policy.max_tracked_documents;
            if w.len() >= max_docs {
                if let Some(oldest) = w.keys().next().cloned() {
                    w.remove(&oldest);
                }
            }
            w.insert(uri, doc);
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;

        if !self.rate_limiters.did_change.try_consume(1) {
            eprintln!("LSP: rate limit exceeded for did_change ({})", uri);
            return;
        }
        self.set_request_method("textDocument/did_change");

        // Same per-document cap as `did_open`. We process only the first
        // content change (full-document sync mode), so it's the relevant
        // one to size-check.
        if let Some(change) = params.content_changes.first() {
            if change.text.len() > Self::MAX_DOCUMENT_BYTES {
                eprintln!(
                    "LSP: refusing did_change for {} ({} bytes > {})",
                    uri,
                    change.text.len(),
                    Self::MAX_DOCUMENT_BYTES
                );
                return;
            }
        }

        if Self::is_schema_file(&uri) {
            if let Some(change) = params.content_changes.first() {
                let lines = schema_line_map(&change.text);
                let line_count = change.text.lines().count() as u32;
                if let Ok(new_schema) = parse_schema_content(&change.text) {
                    let old_schema = self.schema.read().ok().and_then(|r| r.clone());
                    if detect_sensitivity_downgrade(old_schema.as_ref(), &new_schema) {
                        eprintln!(
                            "LSP: refusing schema change for {} — sensitivity downgrade detected",
                            uri
                        );
                        return;
                    }
                    if let Ok(mut w) = self.schema.write() {
                        *w = Some(new_schema);
                    }
                    if let Ok(mut w) = self.schema_lines.write() {
                        *w = lines;
                    }
                    if let Ok(mut w) = self.schema_line_count.write() {
                        *w = Some(line_count);
                    }
                    if let Ok(mut w) = self.schema_uri.write() {
                        *w = Some(uri);
                    }
                    self.republish_all();
                }
            }
            return;
        }

        if Self::is_mcp_config_file(&uri) {
            if let Some(change) = params.content_changes.first() {
                self.publish_mcp_diagnostics(&uri, &change.text, params.text_document.version);
            }
            return;
        }

        // Config-format files (.properties, .env-cascade) — new handler (FR4).
        if is_config_format_file(&uri) {
            // FR18/NFR5: Fence enforcement at parity with .env files.
            if self.is_fenced_env_file(&uri) {
                eprintln!(
                    "LSP: refusing did_change for {} — workspace is fenced (config file)",
                    uri
                );
                return;
            }
            if let Some((fmt, layer)) = format_for_uri(&uri) {
                if let Some(change) = params.content_changes.first() {
                    let entries = fmt.parse(&change.text, layer.clone());
                    let doc = ConfigDocumentState {
                        content: change.text.clone(),
                        version: params.text_document.version,
                        entries,
                        source_layer: layer,
                        write_capability: fmt.write_capability(),
                    };
                    self.publish_config_diagnostics_for(&uri, &doc);
                    if let Ok(mut w) = self.config_documents.write() {
                        w.insert(uri, doc);
                    }
                }
                return;
            }
        }

        if !Self::is_env_file(&uri) {
            return;
        }

        if self.is_fenced_env_file(&uri) {
            eprintln!("LSP: refusing did_change for {} — workspace is fenced", uri);
            return;
        }

        if let Some(change) = params.content_changes.first() {
            let entries = parse_env_document(&change.text);
            let doc = DocumentState {
                content: change.text.clone(),
                version: params.text_document.version,
                entries,
            };
            self.publish_diagnostics_with_ai_guard(&uri, &doc);
            if let Ok(mut w) = self.documents.write() {
                w.insert(uri, doc);
            }
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;

        if !self.rate_limiters.did_save.try_consume(1) {
            return;
        }
        self.set_request_method("textDocument/did_save");

        if Self::is_schema_file(&uri) {
            self.load_schema_from_workspace();
            self.republish_all();
            return;
        }

        // Config-format files (.properties, .env-cascade, YAML) — re-publish
        // diagnostics on save so editors see the current state even if the
        // file was modified outside the editor (NFR12).
        // M-1: skip publishing (which would leak key names) when the file is fenced.
        if is_config_format_file(&uri) {
            if self.is_fenced_env_file(&uri) {
                return;
            }
            let cfg_doc = self
                .config_documents
                .read()
                .ok()
                .and_then(|docs| docs.get(&uri).cloned());
            if let Some(doc) = cfg_doc {
                self.publish_config_diagnostics_for(&uri, &doc);
            }
            return;
        }

        if Self::is_env_file(&uri) {
            let doc = self
                .documents
                .read()
                .ok()
                .and_then(|docs| docs.get(&uri).cloned());
            if let Some(doc) = doc {
                self.publish_diagnostics_with_ai_guard(&uri, &doc);
            }
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        if let Ok(mut w) = self.documents.write() {
            if let Some(mut state) = w.remove(&params.text_document.uri) {
                state.content.zeroize();
                for entry in &mut state.entries {
                    entry.key.zeroize();
                    entry.value.zeroize();
                }
            }
        }
        // Also remove config-format document if present — zeroize secrets first (C-2).
        if let Ok(mut w) = self.config_documents.write() {
            if let Some(mut state) = w.remove(&params.text_document.uri) {
                Self::zeroize_config_state(&mut state);
            }
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        if !self.rate_limiters.hover.try_consume(1) {
            return Ok(None);
        }
        tokio::time::sleep(std::time::Duration::from_micros(
            super::rate_limit::timing_jitter_micros(),
        ))
        .await;

        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let keys_accessed = self.extract_keys_from_hover_position(&params);
        let _ = self
            .audit_logger
            .log_operation("hover", uri.as_str(), &keys_accessed, "success");

        // Config-format hover (properties / .env-cascade) — FR15.
        // C-1(b): per-request fence guard — refuse if fenced, regardless of
        // whether the document is still in config_documents (stale-data defense).
        if is_config_format_file(uri) && self.is_fenced_env_file(uri) {
            return Ok(None);
        }
        if let Some(cfg_doc) = self
            .config_documents
            .read()
            .ok()
            .and_then(|docs| docs.get(uri).cloned())
        {
            let schema = self.schema.read().ok().and_then(|r| r.clone());
            // Assemble all open config documents in the same workspace,
            // sorted by source_layer precedence (base < profile;
            // .env < .env.local < .env.{env}) so the resolution engine
            // picks the correct winning layer (FR15).
            let all_layers: Vec<Vec<crate::ops::config_format::ConfigEntry>> = {
                let docs = self.config_documents.read();
                let mut layer_docs: Vec<ConfigDocumentState> = docs
                    .as_ref()
                    .map(|m| m.values().cloned().collect())
                    .unwrap_or_default();
                layer_docs.sort_by_key(|d| d.source_layer.precedence());
                layer_docs.into_iter().map(|d| d.entries).collect()
            };
            // FR4 (Intent 040): build UnifiedSchema for cross-format sensitivity.
            let unified = schema.as_ref().map(|s| UnifiedSchema::new(s.clone()));
            return Ok(config_hover(
                pos,
                &cfg_doc.entries,
                &all_layers,
                schema.as_ref(),
                unified.as_ref(),
            ));
        }

        let doc = self
            .documents
            .read()
            .ok()
            .and_then(|docs| docs.get(uri).cloned());
        let doc = match doc {
            Some(d) => d,
            None => return Ok(None),
        };

        let schema = self.schema.read().ok().and_then(|r| r.clone());
        let managed = self
            .managed_vars
            .read()
            .ok()
            .map(|m| m.clone())
            .unwrap_or_default();
        Ok(hover_info(pos, &doc.entries, schema.as_ref(), &managed))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;

        let keys_suggested = self.extract_suggested_keys(&params);
        let _ =
            self.audit_logger
                .log_operation("completion", uri.as_str(), &keys_suggested, "success");

        if !self.rate_limiters.completion.try_consume(1) {
            return Ok(None);
        }

        tokio::time::sleep(std::time::Duration::from_micros(
            super::rate_limit::timing_jitter_micros(),
        ))
        .await;

        // Config-format completion (properties / .env-cascade).
        // C-1(b): per-request fence guard.
        if is_config_format_file(uri) && self.is_fenced_env_file(uri) {
            return Ok(None);
        }
        if let Some(cfg_doc) = self
            .config_documents
            .read()
            .ok()
            .and_then(|docs| docs.get(uri).cloned())
        {
            let schema = self.schema.read().ok().and_then(|r| r.clone());
            let items = super::config_features::config_completions(
                pos,
                &cfg_doc.content,
                &cfg_doc.entries,
                schema.as_ref(),
            );
            if items.is_empty() {
                return Ok(None);
            }
            return Ok(Some(CompletionResponse::Array(items)));
        }

        let doc = self
            .documents
            .read()
            .ok()
            .and_then(|docs| docs.get(uri).cloned());
        let doc = match doc {
            Some(d) => d,
            None => return Ok(None),
        };

        let schema = self.schema.read().ok().and_then(|r| r.clone());
        let managed = self
            .managed_vars
            .read()
            .ok()
            .map(|m| m.clone())
            .unwrap_or_default();
        let items = completions(pos, &doc.content, &doc.entries, schema.as_ref(), &managed);

        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(CompletionResponse::Array(items)))
        }
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        if !self.rate_limiters.goto_definition.try_consume(1) {
            return Ok(None);
        }

        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let schema_uri = self.schema_uri.read().ok().and_then(|r| r.clone());
        let schema_lines = self
            .schema_lines
            .read()
            .ok()
            .map(|lines| lines.clone())
            .unwrap_or_default();

        // Config-format files (.properties / .env-cascade / YAML) — use the
        // config_documents store so Unit-001/002 files retain goto-def (F6).
        // C-1(b): per-request fence guard.
        if is_config_format_file(uri) && self.is_fenced_env_file(uri) {
            return Ok(None);
        }
        if is_config_format_file(uri) {
            if let Some(cfg_doc) = self
                .config_documents
                .read()
                .ok()
                .and_then(|docs| docs.get(uri).cloned())
            {
                let open_config_docs: std::collections::HashMap<Url, Vec<ConfigEntry>> = self
                    .config_documents
                    .read()
                    .ok()
                    .map(|m| {
                        m.iter()
                            .map(|(u, d)| (u.clone(), d.entries.clone()))
                            .collect()
                    })
                    .unwrap_or_default();

                // Per-format result (single-format, existing behaviour).
                let per_format = super::config_features::config_goto_definition(
                    pos,
                    &cfg_doc.entries,
                    schema_uri.as_ref(),
                    &schema_lines,
                    &open_config_docs,
                );

                // FR3 (Intent 040): union with cross-format goto-definition.
                // Determine the key under cursor to pass to cross_format_goto_definition.
                let key_under_cursor = cfg_doc
                    .entries
                    .iter()
                    .find(|e| {
                        !e.key.is_empty()
                            && e.line == pos.line
                            && pos.character >= e.key_range.start.character
                            && pos.character <= e.key_range.end.character
                    })
                    .map(|e| e.key.clone());

                if let Some(key) = key_under_cursor {
                    let cross_locs = cross_format_goto_definition(
                        &key,
                        schema_uri.as_ref(),
                        &schema_lines,
                        &open_config_docs,
                    );
                    if cross_locs.is_empty() {
                        return Ok(per_format);
                    }
                    // Merge: start with per-format scalar (if any), add cross locs.
                    let mut merged: Vec<Location> = match per_format {
                        Some(GotoDefinitionResponse::Scalar(loc)) => vec![loc],
                        Some(GotoDefinitionResponse::Array(locs)) => locs,
                        Some(GotoDefinitionResponse::Link(links)) => links
                            .into_iter()
                            .map(|l| Location {
                                uri: l.target_uri,
                                range: l.target_selection_range,
                            })
                            .collect(),
                        None => Vec::new(),
                    };
                    for loc in cross_locs {
                        merged.push(loc);
                    }
                    // Sort and dedup by (uri, line).
                    merged.sort_by(|a, b| {
                        a.uri
                            .as_str()
                            .cmp(b.uri.as_str())
                            .then_with(|| a.range.start.line.cmp(&b.range.start.line))
                    });
                    merged.dedup_by(|a, b| {
                        a.uri == b.uri && a.range.start.line == b.range.start.line
                    });
                    return Ok(Some(GotoDefinitionResponse::Array(merged)));
                }

                return Ok(per_format);
            }
        }

        // .env file → existing key → schema dispatch.
        if Self::is_env_file(uri) {
            let doc = self
                .documents
                .read()
                .ok()
                .and_then(|docs| docs.get(uri).cloned());
            let Some(doc) = doc else { return Ok(None) };
            return Ok(goto_definition(
                pos,
                &doc.entries,
                schema_uri.as_ref(),
                &schema_lines,
            ));
        }

        // Source-file dispatch — extract the UPPER_SNAKE_CASE identifier
        // at the cursor and resolve via the schema line map. We do a
        // disk-backed read with workspace-root containment + size cap so
        // a hostile client cannot direct us into reading files the user
        // never intended to expose through the LSP.
        if let Some(source_text) = self.read_source_text_for_uri(uri) {
            return Ok(goto_definition_from_source(
                pos,
                &source_text,
                schema_uri.as_ref(),
                &schema_lines,
            ));
        }

        Ok(None)
    }

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        if !self.rate_limiters.document_symbol.try_consume(1) {
            return Ok(None);
        }
        tokio::time::sleep(std::time::Duration::from_micros(
            super::rate_limit::timing_jitter_micros(),
        ))
        .await;

        let uri = &params.text_document.uri;

        // Config-format files (.properties / .env-cascade / YAML) — F6 fix.
        // C-1(b): per-request fence guard.
        if is_config_format_file(uri) && self.is_fenced_env_file(uri) {
            return Ok(None);
        }
        if let Some(cfg_doc) = self
            .config_documents
            .read()
            .ok()
            .and_then(|docs| docs.get(uri).cloned())
        {
            let schema = self.schema.read().ok().and_then(|r| r.clone());
            return Ok(super::document_symbol::config_document_symbols(
                &cfg_doc.entries,
                schema.as_ref(),
            ));
        }

        let doc = self
            .documents
            .read()
            .ok()
            .and_then(|docs| docs.get(uri).cloned());
        let doc = match doc {
            Some(d) => d,
            None => return Ok(None),
        };

        let schema = self.schema.read().ok().and_then(|r| r.clone());
        Ok(document_symbols(&doc.entries, schema.as_ref()))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        if !self.rate_limiters.document_symbol.try_consume(1) {
            return Ok(None);
        }

        let query = params.query;
        let managed = self
            .managed_vars
            .read()
            .ok()
            .map(|m| m.clone())
            .unwrap_or_default();
        let schema = self.schema.read().ok().and_then(|r| r.clone());

        let symbols = workspace_symbols(&query, &managed, schema.as_ref());

        if symbols.is_empty() {
            Ok(None)
        } else {
            Ok(Some(symbols))
        }
    }

    async fn code_lens(&self, params: CodeLensParams) -> Result<Option<Vec<CodeLens>>> {
        if !self.rate_limiters.code_lens.try_consume(1) {
            return Ok(None);
        }

        let uri = &params.text_document.uri;

        // Config-format files (.properties / .env-cascade / YAML) — F6 fix.
        // No code-lenses defined for config-format files yet; return None rather
        // than crash on a missing doc lookup.
        if is_config_format_file(uri) {
            return Ok(None);
        }

        let doc = self
            .documents
            .read()
            .ok()
            .and_then(|docs| docs.get(uri).cloned());
        let doc = match doc {
            Some(d) => d,
            None => return Ok(None),
        };

        let schema = self.schema.read().ok().and_then(|r| r.clone());
        let canary_keys: std::collections::HashSet<String> = crate::ops::canary::list_canaries()
            .map(|cs| cs.into_iter().map(|c| c.key).collect())
            .unwrap_or_default();
        let lenses = code_lenses(&doc.entries, schema.as_ref(), Some(&canary_keys), Some(uri));

        if lenses.is_empty() {
            Ok(None)
        } else {
            Ok(Some(lenses))
        }
    }

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        if !self.rate_limiters.code_action.try_consume(1) {
            return Ok(None);
        }
        self.set_request_method("textDocument/code_action");

        let uri = &params.text_document.uri;

        // Config-format files (.properties / .env-cascade / YAML) — F6 fix.
        // These are stored in config_documents, not documents.
        if is_config_format_file(uri) {
            // No code-actions defined for config-format files yet; return None.
            // This preserves the feature parity baseline and avoids crashing on
            // a missing doc lookup.
            return Ok(None);
        }

        let doc = self
            .documents
            .read()
            .ok()
            .and_then(|docs| docs.get(uri).cloned());
        let doc = match doc {
            Some(d) => d,
            None => return Ok(None),
        };

        // MCP config files: offer quick-fixes that replace hardcoded credentials
        // with `${ENV_VAR}` references (FR19 / Story 3.2). Skip the env/schema
        // code-action path for these files — it does not apply to JSON configs.
        if Self::is_mcp_config_file(uri) {
            let actions = mcp_config_code_actions(uri, &doc.content, &params.context.diagnostics);
            if !actions.is_empty() {
                return Ok(Some(
                    actions
                        .into_iter()
                        .map(CodeActionOrCommand::CodeAction)
                        .collect(),
                ));
            }
            return Ok(None);
        }

        let schema = self.schema.read().ok().and_then(|r| r.clone());
        let schema_uri = self.schema_uri.read().ok().and_then(|r| r.clone());
        let schema_line_count = self.schema_line_count.read().ok().and_then(|r| *r);
        let schema_lines = self
            .schema_lines
            .read()
            .ok()
            .map(|m| m.clone())
            .unwrap_or_default();

        let workspace_root = self
            .workspace_root
            .read()
            .ok()
            .and_then(|r| r.clone())
            .and_then(|url| url.to_file_path().ok());

        Ok(code_actions(
            uri,
            &doc.entries,
            &params.context.diagnostics,
            schema.as_ref(),
            schema_uri.as_ref(),
            schema_line_count,
            Some(&schema_lines),
            workspace_root.as_deref(),
        ))
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        if !self.rate_limiters.folding_range.try_consume(1) {
            return Ok(None);
        }

        let uri = &params.text_document.uri;

        let doc = self
            .documents
            .read()
            .ok()
            .and_then(|docs| docs.get(uri).cloned());
        let doc = match doc {
            Some(d) => d,
            None => return Ok(None),
        };

        let ranges = compute_folding_ranges(&doc.entries);

        if ranges.is_empty() {
            Ok(None)
        } else {
            Ok(Some(ranges))
        }
    }

    /// executeCommand is permanently disabled regardless of capability
    /// advertisement. Returning a protocol-level `MethodNotFound` error
    /// ensures that even if `execute_command_provider` is accidentally
    /// re-added to `ServerCapabilities`, no command ever dispatches.
    /// The LSP is a read-only security boundary — all mutations must
    /// go through the CLI.
    async fn execute_command(
        &self,
        _params: ExecuteCommandParams,
    ) -> Result<Option<serde_json::Value>> {
        Err(tower_lsp::jsonrpc::Error {
            code: tower_lsp::jsonrpc::ErrorCode::MethodNotFound,
            message:
                "executeCommand is permanently disabled — use `envforge` CLI for all mutations"
                    .into(),
            data: None,
        })
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        if !self.rate_limiters.semantic_tokens.try_consume(1) {
            return Ok(None);
        }
        tokio::time::sleep(std::time::Duration::from_micros(
            super::rate_limit::timing_jitter_micros(),
        ))
        .await;

        let uri = &params.text_document.uri;

        // Config-format semantic tokens.
        // C-1(b): per-request fence guard.
        if is_config_format_file(uri) && self.is_fenced_env_file(uri) {
            return Ok(None);
        }
        if let Some(cfg_doc) = self
            .config_documents
            .read()
            .ok()
            .and_then(|docs| docs.get(uri).cloned())
        {
            let schema = self.schema.read().ok().and_then(|r| r.clone());
            let tokens = config_semantic_tokens(&cfg_doc.entries, schema.as_ref());
            if tokens.data.is_empty() {
                return Ok(None);
            }
            return Ok(Some(SemanticTokensResult::Tokens(tokens)));
        }

        if !Self::is_env_file(uri) {
            return Ok(None);
        }

        let doc = self
            .documents
            .read()
            .ok()
            .and_then(|docs| docs.get(uri).cloned());
        let Some(doc) = doc else { return Ok(None) };

        let schema = self.schema.read().ok().and_then(|r| r.clone());
        let tokens = compute_semantic_tokens(&doc.entries, schema.as_ref());

        if tokens.data.is_empty() {
            Ok(None)
        } else {
            Ok(Some(SemanticTokensResult::Tokens(tokens)))
        }
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        if !self.rate_limiters.formatting.try_consume(1) {
            return Ok(None);
        }

        let uri = &params.text_document.uri;

        // Config-format formatting.
        // C-1(b): per-request fence guard.
        if is_config_format_file(uri) && self.is_fenced_env_file(uri) {
            return Ok(None);
        }
        if let Some(cfg_doc) = self
            .config_documents
            .read()
            .ok()
            .and_then(|docs| docs.get(uri).cloned())
        {
            // C-1: dispatch by format — never run the .properties KV-normalizing
            // regex on YAML / TOML / JSONC content.
            let edits = if is_yaml_config_file(uri) {
                // YAML format is deliberately a no-op (Intent 038 Open decision 1).
                config_yaml_format_text_edits(&cfg_doc.content)
            } else if is_toml_config_file(uri) {
                // TOML uses toml_edit lossless round-trip.
                config_toml_format_text_edits(&cfg_doc.content)
            } else if is_appsettings_file(uri) {
                // JSONC: no formatter defined — return no edits (preserve content).
                Vec::new()
            } else {
                // .properties / .env-cascade: existing KV-normalizing formatter.
                config_format_text_edits(&cfg_doc.content, cfg_doc.write_capability)
            };
            if edits.is_empty() {
                return Ok(None);
            }
            return Ok(Some(edits));
        }

        if !Self::is_env_file(uri) {
            return Ok(None);
        }

        let content = self
            .documents
            .read()
            .ok()
            .and_then(|docs| docs.get(uri).map(|d| d.content.clone()));
        let Some(content) = content else {
            return Ok(None);
        };

        let edits = format_text_edits(&content);
        if edits.is_empty() {
            Ok(None)
        } else {
            Ok(Some(edits))
        }
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        if !self.rate_limiters.references.try_consume(1) {
            return Ok(None);
        }

        let uri = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let include_declaration = params.context.include_declaration;

        // C-1(b): per-request fence guard for config-format files.
        if is_config_format_file(uri) && self.is_fenced_env_file(uri) {
            return Ok(None);
        }

        // Resolve cursor → key. Config-format files checked first (F6),
        // then env files, then source files.
        let key = if is_config_format_file(uri) {
            let cfg_docs = match self.config_documents.read() {
                Ok(g) => g,
                Err(_) => return Ok(None),
            };
            let Some(doc) = cfg_docs.get(uri) else {
                return Ok(None);
            };
            doc.entries
                .iter()
                .find(|e| {
                    !e.key.is_empty()
                        && e.line == pos.line
                        && pos.character >= e.key_range.start.character
                        && pos.character <= e.key_range.end.character
                })
                .map(|e| e.key.clone())
        } else if Self::is_env_file(uri) {
            let docs = match self.documents.read() {
                Ok(g) => g,
                Err(_) => return Ok(None),
            };
            let Some(doc) = docs.get(uri) else {
                return Ok(None);
            };
            doc.entries
                .iter()
                .find(|e| {
                    e.line == pos.line
                        && pos.character >= e.key_range.start.character
                        && pos.character <= e.key_range.end.character
                })
                .map(|e| e.key.clone())
        } else if let Some(source_text) = self.read_source_text_for_uri(uri) {
            let line = source_text
                .lines()
                .nth(pos.line as usize)
                .map(str::to_string);
            line.and_then(|l| extract_upper_snake_identifier(&l, pos.character as usize))
        } else {
            None
        };

        let Some(key) = key else { return Ok(None) };

        let schema_uri = self.schema_uri.read().ok().and_then(|r| r.clone());
        let schema_lines = self
            .schema_lines
            .read()
            .ok()
            .map(|m| m.clone())
            .unwrap_or_default();

        // For config-format files, search across config_documents (F6).
        if is_config_format_file(uri) {
            let open_config_docs: std::collections::HashMap<Url, Vec<ConfigEntry>> = self
                .config_documents
                .read()
                .ok()
                .map(|m| {
                    m.iter()
                        .map(|(u, d)| (u.clone(), d.entries.clone()))
                        .collect()
                })
                .unwrap_or_default();

            // Per-format references (existing behaviour, exact-key match).
            let mut locs = super::config_features::config_find_references(
                &key,
                schema_uri.as_ref(),
                &schema_lines,
                &open_config_docs,
                include_declaration,
            );

            // FR3 (Intent 040): union with cross-format find-references.
            let cross_locs = cross_format_find_references(
                &key,
                schema_uri.as_ref(),
                &schema_lines,
                &open_config_docs,
                include_declaration,
            );
            for loc in cross_locs {
                locs.push(loc);
            }
            // Sort and dedup by (uri, line).
            locs.sort_by(|a, b| {
                a.uri
                    .as_str()
                    .cmp(b.uri.as_str())
                    .then_with(|| a.range.start.line.cmp(&b.range.start.line))
            });
            locs.dedup_by(|a, b| a.uri == b.uri && a.range.start.line == b.range.start.line);

            return if locs.is_empty() {
                Ok(None)
            } else {
                Ok(Some(locs))
            };
        }

        let open_docs = self
            .documents
            .read()
            .ok()
            .map(|m| m.clone())
            .unwrap_or_default();

        let locs = find_references(
            &key,
            schema_uri.as_ref(),
            &schema_lines,
            &open_docs,
            include_declaration,
        );

        if locs.is_empty() {
            Ok(None)
        } else {
            Ok(Some(locs))
        }
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        if !self.rate_limiters.rename.try_consume(1) {
            return Ok(None);
        }

        let uri = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let new_name = params.new_name;

        // C-1(b): per-request fence guard for config-format files.
        if is_config_format_file(uri) && self.is_fenced_env_file(uri) {
            return Ok(None);
        }

        // Resolve the key at the cursor. Config-format files checked first (F6).
        let old_key = if is_config_format_file(uri) {
            let cfg_docs = match self.config_documents.read() {
                Ok(g) => g,
                Err(_) => return Ok(None),
            };
            let Some(doc) = cfg_docs.get(uri) else {
                return Ok(None);
            };
            doc.entries
                .iter()
                .find(|e| {
                    !e.key.is_empty()
                        && e.line == pos.line
                        && pos.character >= e.key_range.start.character
                        && pos.character <= e.key_range.end.character
                })
                .map(|e| e.key.clone())
        } else if Self::is_env_file(uri) {
            let docs = match self.documents.read() {
                Ok(g) => g,
                Err(_) => return Ok(None),
            };
            let Some(doc) = docs.get(uri) else {
                return Ok(None);
            };
            doc.entries
                .iter()
                .find(|e| {
                    e.line == pos.line
                        && pos.character >= e.key_range.start.character
                        && pos.character <= e.key_range.end.character
                })
                .map(|e| e.key.clone())
        } else if let Some(source_text) = self.read_source_text_for_uri(uri) {
            let line = source_text
                .lines()
                .nth(pos.line as usize)
                .map(str::to_string);
            line.and_then(|l| extract_upper_snake_identifier(&l, pos.character as usize))
        } else {
            None
        };

        let Some(old_key) = old_key else {
            return Ok(None);
        };

        let schema_uri = self.schema_uri.read().ok().and_then(|r| r.clone());
        let schema_lines = self
            .schema_lines
            .read()
            .ok()
            .map(|m| m.clone())
            .unwrap_or_default();

        // For config-format files, dispatch by format (C-2 fix).
        // The generic config_rename writes the full new_name into a leaf key_range,
        // causing data corruption for YAML/TOML/JSONC where the key_range spans
        // only the leaf token. Use format-specific rename functions instead.
        if is_config_format_file(uri) {
            let write_cap = self
                .config_documents
                .read()
                .ok()
                .and_then(|docs| docs.get(uri).map(|d| d.write_capability))
                .unwrap_or(WriteCapability::ReadOnly);

            // Collect entries and content for all open config docs.
            let (open_config_docs, doc_contents): (
                std::collections::HashMap<Url, Vec<ConfigEntry>>,
                std::collections::HashMap<Url, String>,
            ) = self
                .config_documents
                .read()
                .ok()
                .map(|m| {
                    let entries = m
                        .iter()
                        .map(|(u, d)| (u.clone(), d.entries.clone()))
                        .collect();
                    let contents = m
                        .iter()
                        .map(|(u, d)| (u.clone(), d.content.clone()))
                        .collect();
                    (entries, contents)
                })
                .unwrap_or_default();

            // M-1 fix: scope collision check to same-format docs only.
            // For YAML/TOML/JSONC renames we pass only same-format docs to
            // the format-specific rename functions which do their own collision
            // check scoped to their key namespace.

            if is_yaml_config_file(uri) {
                // C-2: YAML uses surgical byte-range splice via yamlpath.
                // Scope to YAML docs only (M-1).
                let yaml_docs: std::collections::HashMap<Url, Vec<ConfigEntry>> = open_config_docs
                    .iter()
                    .filter(|(u, _)| is_yaml_config_file(u))
                    .map(|(u, v)| (u.clone(), v.clone()))
                    .collect();
                let yaml_contents: std::collections::HashMap<Url, String> = doc_contents
                    .iter()
                    .filter(|(u, _)| is_yaml_config_file(u))
                    .map(|(u, v)| (u.clone(), v.clone()))
                    .collect();
                return Ok(config_yaml_rename(
                    &old_key,
                    &new_name,
                    write_cap,
                    &yaml_docs,
                    &yaml_contents,
                ));
            }

            if is_toml_config_file(uri) {
                // C-2: TOML uses toml_edit lossless mutation.
                // Scope to TOML docs only (M-1).
                let toml_docs: std::collections::HashMap<Url, Vec<ConfigEntry>> = open_config_docs
                    .iter()
                    .filter(|(u, _)| is_toml_config_file(u))
                    .map(|(u, v)| (u.clone(), v.clone()))
                    .collect();
                let toml_contents: std::collections::HashMap<Url, String> = doc_contents
                    .iter()
                    .filter(|(u, _)| is_toml_config_file(u))
                    .map(|(u, v)| (u.clone(), v.clone()))
                    .collect();
                return Ok(config_toml_rename(
                    &old_key,
                    &new_name,
                    write_cap,
                    &toml_docs,
                    &toml_contents,
                ));
            }

            if is_appsettings_file(uri) {
                // C-2: JSONC uses surgical byte-range splice.
                // Scope to JSONC docs only (M-1).
                let jsonc_docs: std::collections::HashMap<Url, Vec<ConfigEntry>> = open_config_docs
                    .iter()
                    .filter(|(u, _)| is_appsettings_file(u))
                    .map(|(u, v)| (u.clone(), v.clone()))
                    .collect();
                let jsonc_contents: std::collections::HashMap<Url, String> = doc_contents
                    .iter()
                    .filter(|(u, _)| is_appsettings_file(u))
                    .map(|(u, v)| (u.clone(), v.clone()))
                    .collect();
                return Ok(config_jsonc_rename(
                    &old_key,
                    &new_name,
                    write_cap,
                    &jsonc_docs,
                    &jsonc_contents,
                ));
            }

            // .properties / .env-cascade: use existing generic config_rename.
            return Ok(super::config_features::config_rename(
                &old_key,
                &new_name,
                write_cap,
                schema_uri.as_ref(),
                &schema_lines,
                &open_config_docs,
            ));
        }

        let open_docs = self
            .documents
            .read()
            .ok()
            .map(|m| m.clone())
            .unwrap_or_default();

        Ok(build_rename_edit(
            &old_key,
            &new_name,
            schema_uri.as_ref(),
            &schema_lines,
            &open_docs,
        ))
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        if !self.rate_limiters.inlay_hint.try_consume(1) {
            return Ok(None);
        }
        tokio::time::sleep(std::time::Duration::from_micros(
            super::rate_limit::timing_jitter_micros(),
        ))
        .await;

        let uri = &params.text_document.uri;

        // Config-format files (.properties / .env-cascade / YAML) — F6 fix.
        // No inlay-hints defined for config-format files yet; return None rather
        // than crash on a missing doc lookup.
        if is_config_format_file(uri) {
            return Ok(None);
        }

        let doc = self
            .documents
            .read()
            .ok()
            .and_then(|docs| docs.get(uri).cloned());
        let doc = match doc {
            Some(d) => d,
            None => return Ok(None),
        };

        let schema = self.schema.read().ok().and_then(|r| r.clone());

        let hints = compute_inlay_hints(params.range, &doc.entries, schema.as_ref());

        if hints.is_empty() {
            Ok(None)
        } else {
            Ok(Some(hints))
        }
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

/// Detect whether a schema update removes sensitive flags from any variable.
/// Returns true if the new schema has at least one variable whose `sensitive`
/// field was `true` in the old schema but is now `false` (or not present).
pub fn detect_sensitivity_downgrade(old: Option<&EnvSchema>, new: &EnvSchema) -> bool {
    let Some(old) = old else { return false };
    for (key, new_var) in &new.variables {
        if let Some(old_var) = old.variables.get(key) {
            if old_var.sensitive && !new_var.sensitive {
                eprintln!(
                    "LSP: sensitivity downgrade detected for '{}': sensitive was true, now false",
                    key
                );
                return true;
            }
        }
    }
    for (key, old_var) in &old.variables {
        if old_var.sensitive && !new.variables.contains_key(key) {
            eprintln!(
                "LSP: sensitivity downgrade detected for '{}': sensitive key removed from schema",
                key
            );
            return true;
        }
    }
    false
}

fn shellexpand(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    std::path::PathBuf::from(path)
}

/// Parameters for the custom `envforge/exposureMap` request. Plugin
/// clients pass the URI of an `.env*` file they want classified.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ExposureMapParams {
    pub uri: Url,
}

/// Response for `envforge/exposureMap`. Carries the per-line
/// classification plus a global `fence_active` snapshot so the client
/// can render a consistent legend without an additional round trip.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ExposureMapResponse {
    pub entries: Vec<ExposureEntry>,
    pub fence_active: bool,
}

impl Backend {
    /// Custom-request handler for `envforge/exposureMap`. Mirrors
    /// `exposure_for` but is shaped for the LSP RPC frame.
    ///
    /// The async signature is required by tower-lsp's custom-method
    /// registration even though no awaits occur — we read state from
    /// `RwLock`s synchronously and probe fence status off a fast disk
    /// stat. The trait bound forces this shape.
    #[allow(clippy::unused_async)]
    pub async fn exposure_map(&self, params: ExposureMapParams) -> Result<ExposureMapResponse> {
        self.set_request_method("envforge/exposure_map");
        if !self.rate_limiters.exposure_map.try_consume(1) {
            return Err(tower_lsp::jsonrpc::Error {
                code: tower_lsp::jsonrpc::ErrorCode::RequestCancelled,
                message: "exposure map rate limit exceeded".into(),
                data: None,
            });
        }
        let fence_active = self
            .workspace_root
            .read()
            .ok()
            .and_then(|r| r.clone())
            .and_then(|url| url.to_file_path().ok())
            .and_then(|root| crate::ops::fence::check_fence_status(&root).ok())
            .map(|status| status.all_fenced)
            .unwrap_or(false);
        let entries = if fence_active {
            eprintln!(
                "LSP: exposure_map request blocked — workspace fence is active for {}",
                params.uri
            );
            Vec::new()
        } else {
            self.exposure_for(&params.uri)
        };
        Ok(ExposureMapResponse {
            entries,
            fence_active,
        })
    }

    // ── EnvForge custom LSP requests (H4) ──────────────────────────────
    // Constrained, *named* security operations. The generic
    // `workspace/executeCommand` is permanently disabled (arbitrary-command
    // surface); each method below instead maps to exactly ONE fixed command
    // id, so a client can only invoke this vetted allowlist. All share the
    // stable `{ ok, result|error }` JSON shape from `dispatch_command`.
    fn ef_workspace_root(&self) -> Option<std::path::PathBuf> {
        self.workspace_root
            .read()
            .ok()
            .and_then(|r| r.clone())
            .and_then(|u| u.to_file_path().ok())
    }

    fn ef_dispatch(&self, id: &str, arg: serde_json::Value) -> Result<serde_json::Value> {
        let root = self.ef_workspace_root();
        Ok(super::commands::dispatch_command(
            id,
            &[arg],
            root.as_deref(),
        ))
    }

    #[allow(clippy::unused_async)]
    pub async fn fence_status(&self, _params: serde_json::Value) -> Result<serde_json::Value> {
        self.ef_dispatch("envforge.fence.status", serde_json::Value::Null)
    }

    #[allow(clippy::unused_async)]
    pub async fn fence_toggle(&self, _params: serde_json::Value) -> Result<serde_json::Value> {
        let result = self.ef_dispatch("envforge.fence.toggle", serde_json::Value::Null)?;
        // C-1(a): if the fence was just activated, purge in-memory secret stores
        // so stale data cannot be served by hover/completion/etc. after toggle.
        let action = result
            .get("result")
            .and_then(|r| r.get("action"))
            .and_then(|a| a.as_str())
            .unwrap_or("");
        if action == "enabled" {
            // Zeroize and clear config documents.
            if let Ok(mut w) = self.config_documents.write() {
                for state in w.values_mut() {
                    Self::zeroize_config_state(state);
                }
                w.clear();
            }
            // Zeroize and clear .env documents.
            if let Ok(mut w) = self.documents.write() {
                for state in w.values_mut() {
                    state.content.zeroize();
                    for entry in &mut state.entries {
                        entry.key.zeroize();
                        entry.value.zeroize();
                    }
                }
                w.clear();
            }
            eprintln!("LSP: fence enabled — in-memory document stores purged (C-1)");
        }
        Ok(result)
    }

    #[allow(clippy::unused_async)]
    pub async fn canary_scan(&self, params: serde_json::Value) -> Result<serde_json::Value> {
        self.ef_dispatch("envforge.canary.scan", params)
    }

    #[allow(clippy::unused_async)]
    pub async fn canary_check(&self, _params: serde_json::Value) -> Result<serde_json::Value> {
        self.ef_dispatch("envforge.canary.check", serde_json::Value::Null)
    }

    #[allow(clippy::unused_async)]
    pub async fn reveal_value(&self, params: serde_json::Value) -> Result<serde_json::Value> {
        self.ef_dispatch("envforge.reveal.value", params)
    }

    #[allow(clippy::unused_async)]
    pub async fn run_volatile(&self, params: serde_json::Value) -> Result<serde_json::Value> {
        self.ef_dispatch("envforge.run.volatile", params)
    }

    #[allow(clippy::unused_async)]
    pub async fn volatile_status(&self, _params: serde_json::Value) -> Result<serde_json::Value> {
        self.ef_dispatch("envforge.volatile.status", serde_json::Value::Null)
    }

    #[allow(clippy::unused_async)]
    pub async fn volatile_extend(&self, params: serde_json::Value) -> Result<serde_json::Value> {
        self.ef_dispatch("envforge.volatile.extend", params)
    }

    fn extract_keys_from_hover_position(&self, params: &HoverParams) -> Vec<String> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        // M-3: consult config_documents first so config-file hover audits log
        // the accessed key rather than an empty list.
        if let Ok(docs) = self.config_documents.read() {
            if let Some(doc) = docs.get(uri) {
                let keys: Vec<String> = doc
                    .entries
                    .iter()
                    .filter(|e| {
                        !e.key.is_empty()
                            && e.line == pos.line
                            && pos.character >= e.key_range.start.character
                            && pos.character <= e.value_range.end.character
                    })
                    .map(|e| e.key.clone())
                    .collect();
                if !keys.is_empty() {
                    return keys;
                }
            }
        }

        if let Ok(docs) = self.documents.read() {
            if let Some(doc) = docs.get(uri) {
                return doc
                    .entries
                    .iter()
                    .filter(|e| {
                        e.line == pos.line
                            && pos.character >= e.key_range.start.character
                            && pos.character <= e.value_range.end.character
                    })
                    .map(|e| e.key.clone())
                    .collect();
            }
        }
        vec![]
    }

    fn extract_suggested_keys(&self, _params: &CompletionParams) -> Vec<String> {
        let mut keys = vec![];
        if let Ok(vars) = self.managed_vars.read() {
            keys.extend(vars.iter().map(|v| v.key.clone()));
        }
        if let Ok(schema) = self.schema.read() {
            if let Some(s) = schema.as_ref() {
                keys.extend(s.variables.keys().cloned());
            }
        }
        keys.sort();
        keys.dedup();
        keys
    }
}

pub async fn serve() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(Backend::new)
        .custom_method("envforge/exposureMap", Backend::exposure_map)
        // H4: constrained, named security requests (NOT generic executeCommand).
        .custom_method("envforge/fenceStatus", Backend::fence_status)
        .custom_method("envforge/fenceToggle", Backend::fence_toggle)
        .custom_method("envforge/canaryScan", Backend::canary_scan)
        .custom_method("envforge/canaryCheck", Backend::canary_check)
        .custom_method("envforge/revealValue", Backend::reveal_value)
        .custom_method("envforge/runVolatile", Backend::run_volatile)
        .custom_method("envforge/volatileStatus", Backend::volatile_status)
        .custom_method("envforge/volatileExtend", Backend::volatile_extend)
        .finish();
    Server::new(stdin, stdout, socket).serve(service).await;
}

/// Test-only dispatch helpers. These functions mirror the exact dispatch
/// logic used by the Backend handlers (formatting, rename, diagnostics).
/// They accept the Backend's internal state directly so tests can exercise
/// the routing code without instantiating a full LSP transport.
///
/// Tests in `tests/deferred_config_dispatch_tests.rs` use these to verify
/// that C-1 / C-2 / H-1 / H-2 bugs are fixed at the dispatch level.
///
/// This module is compiled unconditionally so integration tests in `tests/`
/// can import it. The `#[doc(hidden)]` attribute prevents it from appearing
/// in rustdoc output — it is not part of the public API.
#[doc(hidden)]
pub mod test_dispatch {
    use std::collections::HashMap;

    use tower_lsp::lsp_types::{Diagnostic, TextEdit, Url, WorkspaceEdit};

    use crate::lsp::config_features::{
        config_diagnostics, config_format_text_edits, config_jsonc_diagnostics,
        config_jsonc_rename, config_toml_diagnostics, config_toml_format_text_edits,
        config_toml_rename, config_yaml_diagnostics, config_yaml_format_text_edits,
        config_yaml_rename,
    };
    use crate::lsp::config_file::{
        format_for_uri, is_appsettings_file, is_toml_config_file, is_yaml_config_file,
    };
    use crate::ops::config_format::{ConfigEntry, SourceLayer, WriteCapability};
    use crate::ops::schema::EnvSchema;

    /// A lightweight in-memory "Backend state" for dispatch tests.
    /// Holds the same fields as the Backend's config_documents / schema stores
    /// but without the RwLock overhead (tests are single-threaded).
    pub struct TestBackend {
        pub config_docs: HashMap<Url, (String, Vec<ConfigEntry>, SourceLayer, WriteCapability)>,
        pub schema: Option<EnvSchema>,
    }

    impl Default for TestBackend {
        fn default() -> Self {
            Self::new()
        }
    }

    impl TestBackend {
        /// Create an empty TestBackend.
        pub fn new() -> Self {
            Self {
                config_docs: HashMap::new(),
                schema: None,
            }
        }

        /// Load a config document — mirrors `did_open` config path.
        pub fn open_doc(&mut self, uri: Url, content: String) {
            let (fmt, layer) = format_for_uri(&uri).expect("URI must be a recognized config file");
            let entries = fmt.parse(&content, layer.clone());
            let write_cap = fmt.write_capability();
            self.config_docs
                .insert(uri, (content, entries, layer, write_cap));
        }

        /// Set the schema.
        pub fn set_schema(&mut self, schema: EnvSchema) {
            self.schema = Some(schema);
        }

        /// Run the C-1 formatting dispatch — exact routing logic from Backend::formatting.
        pub fn formatting_dispatch(&self, uri: &Url) -> Vec<TextEdit> {
            let Some((content, _entries, _layer, write_cap)) = self.config_docs.get(uri) else {
                return Vec::new();
            };

            if is_yaml_config_file(uri) {
                config_yaml_format_text_edits(content)
            } else if is_toml_config_file(uri) {
                config_toml_format_text_edits(content)
            } else if is_appsettings_file(uri) {
                Vec::new()
            } else {
                config_format_text_edits(content, *write_cap)
            }
        }

        /// Run the C-2 rename dispatch — exact routing logic from Backend::rename.
        pub fn rename_dispatch(
            &self,
            uri: &Url,
            old_key: &str,
            new_name: &str,
        ) -> Option<WorkspaceEdit> {
            let write_cap = self
                .config_docs
                .get(uri)
                .map(|(_, _, _, wc)| *wc)
                .unwrap_or(WriteCapability::ReadOnly);

            let open_docs: HashMap<Url, Vec<ConfigEntry>> = self
                .config_docs
                .iter()
                .map(|(u, (_, entries, _, _))| (u.clone(), entries.clone()))
                .collect();
            let doc_contents: HashMap<Url, String> = self
                .config_docs
                .iter()
                .map(|(u, (content, _, _, _))| (u.clone(), content.clone()))
                .collect();

            if is_yaml_config_file(uri) {
                let yaml_docs: HashMap<Url, Vec<ConfigEntry>> = open_docs
                    .iter()
                    .filter(|(u, _)| is_yaml_config_file(u))
                    .map(|(u, v)| (u.clone(), v.clone()))
                    .collect();
                let yaml_contents: HashMap<Url, String> = doc_contents
                    .iter()
                    .filter(|(u, _)| is_yaml_config_file(u))
                    .map(|(u, v)| (u.clone(), v.clone()))
                    .collect();
                config_yaml_rename(old_key, new_name, write_cap, &yaml_docs, &yaml_contents)
            } else if is_toml_config_file(uri) {
                let toml_docs: HashMap<Url, Vec<ConfigEntry>> = open_docs
                    .iter()
                    .filter(|(u, _)| is_toml_config_file(u))
                    .map(|(u, v)| (u.clone(), v.clone()))
                    .collect();
                let toml_contents: HashMap<Url, String> = doc_contents
                    .iter()
                    .filter(|(u, _)| is_toml_config_file(u))
                    .map(|(u, v)| (u.clone(), v.clone()))
                    .collect();
                config_toml_rename(old_key, new_name, write_cap, &toml_docs, &toml_contents)
            } else if is_appsettings_file(uri) {
                let jsonc_docs: HashMap<Url, Vec<ConfigEntry>> = open_docs
                    .iter()
                    .filter(|(u, _)| is_appsettings_file(u))
                    .map(|(u, v)| (u.clone(), v.clone()))
                    .collect();
                let jsonc_contents: HashMap<Url, String> = doc_contents
                    .iter()
                    .filter(|(u, _)| is_appsettings_file(u))
                    .map(|(u, v)| (u.clone(), v.clone()))
                    .collect();
                config_jsonc_rename(old_key, new_name, write_cap, &jsonc_docs, &jsonc_contents)
            } else {
                crate::lsp::config_features::config_rename(
                    old_key,
                    new_name,
                    write_cap,
                    None,
                    &HashMap::new(),
                    &open_docs,
                )
            }
        }

        /// Run the H-1 diagnostics dispatch — exact routing from
        /// Backend::publish_config_diagnostics_for.
        pub fn diagnostics_dispatch(&self, uri: &Url) -> Vec<Diagnostic> {
            let Some((content, entries, layer, _write_cap)) = self.config_docs.get(uri) else {
                return Vec::new();
            };

            if is_yaml_config_file(uri) {
                config_yaml_diagnostics(content, layer.clone())
            } else if is_toml_config_file(uri) {
                config_toml_diagnostics(content, layer.clone(), self.schema.as_ref())
            } else if is_appsettings_file(uri) {
                config_jsonc_diagnostics(content, layer.clone(), self.schema.as_ref())
            } else {
                config_diagnostics(entries, self.schema.as_ref())
            }
        }
    }
}

#[cfg(test)]
mod mcp_config_match_tests {
    use super::*;

    fn is_cfg(p: &str) -> bool {
        Backend::is_mcp_config_file(&Url::parse(&format!("file://{p}")).unwrap())
    }

    /// Story 3.1 (FR18): the widened MCP/agent config filename set is recognized.
    #[test]
    fn test_mcp_config_recognized_paths() {
        // Pre-existing coverage.
        assert!(is_cfg("/proj/mcp.json"));
        assert!(is_cfg("/proj/.mcp.json"));
        assert!(is_cfg("/proj/.cursor/mcp.json"));
        assert!(is_cfg("/proj/.claude/settings.json"));
        assert!(is_cfg("/proj/claude_desktop_config.json"));
        // New in 3.1.
        assert!(is_cfg("/proj/.vscode/mcp.json"), "VS Code mcp.json");
        assert!(is_cfg("/home/u/.claude.json"), "Claude Code user config");
        assert!(
            is_cfg("/home/u/.codeium/windsurf/mcp_config.json"),
            "Windsurf"
        );
        assert!(is_cfg("/proj/cline_mcp_settings.json"), "Cline");
    }

    #[test]
    fn test_non_mcp_files_not_matched() {
        assert!(!is_cfg("/proj/package.json"));
        assert!(!is_cfg("/proj/src/main.rs"));
        assert!(!is_cfg("/proj/.env"));
    }
}
