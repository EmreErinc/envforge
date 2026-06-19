//! EnvForge Zed extension (Story 4.3 / FR21).
//!
//! Registers two commands with Zed, both backed by the `envforge` binary:
//!   - the LSP language server (`envforge lsp`)
//!   - the read-safe MCP context server (`envforge mcp serve`)
//!
//! No custom UI — Zed's extension API does not yet support status-bar/gutter
//! decorations (Draft RFC #53403). This is a thin client; all logic lives in
//! the `envforge` binary (parity, FR22).
//!
//! Build/publish with the Zed extension toolchain (compiles to
//! `wasm32-wasip2`). `envforge` must be on the user's PATH.

use zed_extension_api as zed;

struct EnvForgeExtension;

impl zed::Extension for EnvForgeExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        _worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        Ok(zed::Command {
            command: "envforge".to_string(),
            args: vec!["lsp".to_string()],
            env: Default::default(),
        })
    }

    fn context_server_command(
        &mut self,
        _context_server_id: &zed::ContextServerId,
        _project: &zed::Project,
    ) -> zed::Result<zed::Command> {
        Ok(zed::Command {
            command: "envforge".to_string(),
            args: vec!["mcp".to_string(), "serve".to_string()],
            env: Default::default(),
        })
    }
}

zed::register_extension!(EnvForgeExtension);
