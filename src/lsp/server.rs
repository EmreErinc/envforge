use std::collections::HashMap;
use std::sync::RwLock;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use crate::ops::schema::{parse_schema_content, EnvSchema};

use super::completion::completions;
use super::definition::goto_definition;
use super::diagnostics::compute_diagnostics;
use super::document::{parse_env_document, schema_line_map, DocumentState};
use super::hover::hover_info;

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
            workspace_root: RwLock::new(None),
            managed_vars: RwLock::new(Vec::new()),
        }
    }

    fn is_env_file(uri: &Url) -> bool {
        let path = uri.path();
        let fname = path.rsplit('/').next().unwrap_or("");
        fname == ".env" || fname.starts_with(".env.") || fname.ends_with(".env") || fname == "env"
    }

    fn is_schema_file(uri: &Url) -> bool {
        uri.path().ends_with(".env.schema")
    }

    fn load_schema_from_workspace(&self) {
        let root = self.workspace_root.read().ok().and_then(|r| r.clone());
        if let Some(root_url) = root {
            if let Ok(root_path) = root_url.to_file_path() {
                let schema_path = root_path.join(".env.schema");
                if schema_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&schema_path) {
                        let lines = schema_line_map(&content);
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
                            if let Ok(uri) = Url::from_file_path(&schema_path) {
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
                    trigger_characters: Some(vec!["=".into(), "$".into()]),
                    ..Default::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
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

        if Self::is_schema_file(&uri) {
            let content = &params.text_document.text;
            let lines = schema_line_map(content);
            if let Ok(schema) = parse_schema_content(content) {
                if let Ok(mut w) = self.schema.write() {
                    *w = Some(schema);
                }
                if let Ok(mut w) = self.schema_lines.write() {
                    *w = lines;
                }
                if let Ok(mut w) = self.schema_uri.write() {
                    *w = Some(uri.clone());
                }
                self.republish_all();
            }
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

        if Self::is_schema_file(&uri) {
            if let Some(change) = params.content_changes.first() {
                let lines = schema_line_map(&change.text);
                if let Ok(schema) = parse_schema_content(&change.text) {
                    if let Ok(mut w) = self.schema.write() {
                        *w = Some(schema);
                    }
                    if let Ok(mut w) = self.schema_lines.write() {
                        *w = lines;
                    }
                    if let Ok(mut w) = self.schema_uri.write() {
                        *w = Some(uri.clone());
                    }
                    self.republish_all();
                }
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
        Ok(hover_info(pos, &doc.entries, schema.as_ref()))
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

        let doc = self
            .documents
            .read()
            .ok()
            .and_then(|docs| docs.get(uri).cloned());
        let doc = match doc {
            Some(d) => d,
            None => return Ok(None),
        };

        let schema_uri = self.schema_uri.read().ok().and_then(|r| r.clone());
        let schema_lines = self
            .schema_lines
            .read()
            .ok()
            .map(|lines| lines.clone())
            .unwrap_or_default();

        Ok(goto_definition(
            pos,
            &doc.entries,
            schema_uri.as_ref(),
            &schema_lines,
        ))
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

pub async fn serve() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
