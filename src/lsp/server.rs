use std::collections::HashMap;
use std::sync::RwLock;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use crate::ops::schema::{parse_schema_content, EnvSchema};

use super::ai_guard_diagnostics::compute_ai_guard_diagnostics;
use super::code_action::code_actions;
use super::code_lens::code_lenses;
use super::commands::{dispatch_command, SUPPORTED_COMMANDS};
use super::completion::completions;
use super::definition::{
    extract_upper_snake_identifier, goto_definition, goto_definition_from_source,
};
use super::diagnostics::compute_diagnostics;
use super::document::{parse_env_document, schema_line_map, DocumentState};
use super::document_symbol::document_symbols;
use super::exposure::{compute_exposure_map, ExposureEntry};
use super::folding_range::compute_folding_ranges;
use super::format::format_text_edits;
use super::hover::hover_info;
use super::inlay::compute_inlay_hints;
use super::mcp_diagnostics::compute_mcp_diagnostics;
use super::references::find_references;
use super::rename::build_rename_edit;
use super::semantic_tokens::{compute_semantic_tokens, TOKEN_MODIFIERS, TOKEN_TYPES};
use super::workspace_symbol::workspace_symbols;

/// A known env var from envforge's managed files.
#[derive(Debug, Clone)]
pub struct ManagedVar {
    pub key: String,
    pub value: String,
    pub source_file: String,
}

pub struct Backend {
    client: Client,
    documents: RwLock<HashMap<Url, DocumentState>>,
    schema: RwLock<Option<EnvSchema>>,
    schema_uri: RwLock<Option<Url>>,
    schema_lines: RwLock<HashMap<String, u32>>,
    schema_line_count: RwLock<Option<u32>>,
    workspace_root: RwLock<Option<Url>>,
    managed_vars: RwLock<Vec<ManagedVar>>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            documents: RwLock::new(HashMap::new()),
            schema: RwLock::new(None),
            schema_uri: RwLock::new(None),
            schema_lines: RwLock::new(HashMap::new()),
            schema_line_count: RwLock::new(None),
            workspace_root: RwLock::new(None),
            managed_vars: RwLock::new(Vec::new()),
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

    /// Identify MCP configuration files we want to lint inline.
    /// Conservative: only known config filenames in known directories,
    /// not arbitrary `*.json`. Keeps the diagnostic from misfiring on
    /// unrelated JSON files in a workspace.
    fn is_mcp_config_file(uri: &Url) -> bool {
        let path = uri.path();
        let fname = path.rsplit('/').next().unwrap_or("");
        if fname == "mcp.json" || fname == ".mcp.json" {
            return true;
        }
        if fname == "claude_desktop_config.json" {
            return true;
        }
        // .cursor/mcp.json or .claude/settings.json under any parent
        path.contains("/.cursor/") && fname == "mcp.json"
            || path.contains("/.claude/") && fname == "settings.json"
    }

    fn publish_mcp_diagnostics(&self, uri: &Url, content: &str, version: i32) {
        let path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => std::path::PathBuf::from(uri.path()),
        };
        let diags = compute_mcp_diagnostics(content, &path);
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
        // Find envforge binary
        let binary = find_envforge_binary();
        let output = std::process::Command::new(&binary)
            .args(["list", "--json"])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                if let Ok(text) = String::from_utf8(out.stdout) {
                    if let Ok(vars) = serde_json::from_str::<Vec<serde_json::Value>>(&text) {
                        let managed: Vec<ManagedVar> = vars
                            .iter()
                            .filter_map(|v| {
                                Some(ManagedVar {
                                    key: v.get("key")?.as_str()?.to_string(),
                                    value: v.get("value")?.as_str()?.to_string(),
                                    source_file: v
                                        .get("source_file")
                                        .and_then(|s| s.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                })
                            })
                            .collect();
                        if let Ok(mut w) = self.managed_vars.write() {
                            *w = managed;
                        } else {
                            eprintln!(
                                "LSP: Failed to acquire write lock on managed_vars (lock poisoned)"
                            );
                        }
                    }
                }
            }
            _ => {}
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

    /// Compute the AI-exposure map for a given env-file URI. Reads the
    /// document state, current schema, and fence status (probed live
    /// from disk on each call — `.envforgeignore` etc. can be edited by
    /// other tools without going through us). Returns an empty vec for
    /// unknown URIs or non-env files rather than erroring so plugin
    /// clients can safely poll on every keystroke.
    pub fn exposure_for(&self, uri: &Url) -> Vec<ExposureEntry> {
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
        let fence_active = self
            .workspace_root
            .read()
            .ok()
            .and_then(|r| r.clone())
            .and_then(|url| url.to_file_path().ok())
            .and_then(|root| crate::ops::fence::check_fence_status(&root).ok())
            .map(|status| status.all_fenced)
            .unwrap_or(false);

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

    fn republish_all(&self) {
        if let Ok(docs) = self.documents.read() {
            // Collect URIs to avoid holding lock across publish calls
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
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: SUPPORTED_COMMANDS
                        .iter()
                        .map(|s| (*s).to_string())
                        .collect(),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                }),
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
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;

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

        if !Self::is_env_file(&uri) {
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
            w.insert(uri, doc);
        }
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;

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
                if let Ok(schema) = parse_schema_content(&change.text) {
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
            }
            return;
        }

        if Self::is_mcp_config_file(&uri) {
            if let Some(change) = params.content_changes.first() {
                self.publish_mcp_diagnostics(&uri, &change.text, params.text_document.version);
            }
            return;
        }

        if !Self::is_env_file(&uri) {
            return;
        }

        if let Some(change) = params.content_changes.first() {
            let entries = parse_env_document(&change.text);
            let doc = DocumentState {
                content: change.text.clone(),
                version: params.text_document.version,
                entries,
            };
            self.publish_diagnostics_for(&uri, &doc);
            if let Ok(mut w) = self.documents.write() {
                w.insert(uri, doc);
            }
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        if Self::is_schema_file(&uri) {
            self.load_schema_from_workspace();
            self.republish_all();
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
            w.remove(&params.text_document.uri);
        }
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

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
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let schema_uri = self.schema_uri.read().ok().and_then(|r| r.clone());
        let schema_lines = self
            .schema_lines
            .read()
            .ok()
            .map(|lines| lines.clone())
            .unwrap_or_default();

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

        Ok(document_symbols(&doc.entries))
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
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

        let schema = self.schema.read().ok().and_then(|r| r.clone());
        let schema_uri = self.schema_uri.read().ok().and_then(|r| r.clone());
        let schema_line_count = self.schema_line_count.read().ok().and_then(|r| *r);
        let schema_lines = self
            .schema_lines
            .read()
            .ok()
            .map(|m| m.clone())
            .unwrap_or_default();

        Ok(code_actions(
            uri,
            &doc.entries,
            &params.context.diagnostics,
            schema.as_ref(),
            schema_uri.as_ref(),
            schema_line_count,
            Some(&schema_lines),
        ))
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
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

    async fn execute_command(
        &self,
        params: ExecuteCommandParams,
    ) -> Result<Option<serde_json::Value>> {
        let workspace_path = self
            .workspace_root
            .read()
            .ok()
            .and_then(|r| r.clone())
            .and_then(|url| url.to_file_path().ok());

        let result = dispatch_command(
            &params.command,
            &params.arguments,
            workspace_path.as_deref(),
        );

        Ok(Some(result))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = &params.text_document.uri;
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
        let uri = &params.text_document.uri;
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
        let uri = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let include_declaration = params.context.include_declaration;

        // Resolve cursor → key. Mirrors rename's resolution: env files
        // use document entry hit-testing; source files use the same
        // UPPER_SNAKE_CASE extraction as L4 go-to-def.
        let key = if Self::is_env_file(uri) {
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
        let uri = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let new_name = params.new_name;

        // Resolve the key at the cursor.
        let old_key = if Self::is_env_file(uri) {
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

        let schema = self.schema.read().ok().and_then(|r| r.clone());
        let managed = self
            .managed_vars
            .read()
            .ok()
            .map(|m| m.clone())
            .unwrap_or_default();

        let hints = compute_inlay_hints(params.range, &doc.entries, schema.as_ref(), &managed);

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

fn find_envforge_binary() -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let candidates = [
        format!("{home}/.cargo/bin/envforge"),
        "/usr/local/bin/envforge".into(),
        "/opt/homebrew/bin/envforge".into(),
    ];
    for c in &candidates {
        if std::path::Path::new(c).exists() {
            return c.clone();
        }
    }
    "envforge".into()
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
        let entries = self.exposure_for(&params.uri);
        let fence_active = self
            .workspace_root
            .read()
            .ok()
            .and_then(|r| r.clone())
            .and_then(|url| url.to_file_path().ok())
            .and_then(|root| crate::ops::fence::check_fence_status(&root).ok())
            .map(|status| status.all_fenced)
            .unwrap_or(false);
        Ok(ExposureMapResponse {
            entries,
            fence_active,
        })
    }
}

pub async fn serve() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(Backend::new)
        .custom_method("envforge/exposureMap", Backend::exposure_map)
        .finish();
    Server::new(stdin, stdout, socket).serve(service).await;
}
