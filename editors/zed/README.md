# EnvForge — Zed extension

First-party [EnvForge](../../) client for [Zed](https://zed.dev). EnvForge is a
Rust **CLI + TUI** for env **profiles**, `.env.schema` validation, and secret
scanning. This extension registers the **LSP** only — no sidebar, gutter, or
status bar (Zed WASM API limit; Draft RFC #53403).

**Requires:** `envforge` on PATH (`brew install emreerinc/tap/envforge` or
`cargo install env-forge-tui`).

The **MCP context server** (`envforge mcp serve`) is registered only if your
binary was built with `--features mcp-server`. Default brew and GitHub
binaries do **not** include it. When present it exposes `list_keys` +
`describe_schema` (redacted, audited, never raw values).

Fence (`envforge fence`) is a CLI command: it writes ignore/rules for configured
AI tools (including files Zed honors such as `AGENTS.md`). Not a sandbox.

## Install (dev)

1. Build the extension with the Zed toolchain (compiles to `wasm32-wasip2`):
   Zed → `zed: install dev extension` → select this `editors/zed/` directory.
2. Ensure `envforge` is on PATH.

## What you get

- LSP for `.env*` (diagnostics, hover, completion, MCP-config credential warnings,
  `envforge/*` custom requests).
- Optional `envforge-mcp` context server if the CLI has `mcp-server`.
- No native heatmap/status UI until Zed ships a richer extension API.

## Notes

- `src/lib.rs` targets `zed_extension_api = "0.2"`. Zed's extension API evolves —
  verify the `language_server_command` / `context_server_command` signatures
  against the release you build with.
- Thin client: no business logic here (see `docs/ide-behavior-contract.md`).
