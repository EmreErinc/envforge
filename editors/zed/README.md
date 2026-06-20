# EnvForge — Zed extension

First-party [EnvForge](../../) integration for [Zed](https://zed.dev): registers
the EnvForge **LSP** (diagnostics, hover provenance, completion, MCP-config
credential warnings) and the read-safe **MCP context server** (`envforge mcp
serve`) so Zed's AI assistant gets env metadata without reading secrets.

**No custom status-bar/gutter UI** — Zed's extension API (WASM) does not yet
support it (Draft RFC #53403). EnvForge therefore reaches Zed via LSP + MCP. If
Zed ships its Visual Extension API, a native exposure heatmap can follow.

**Requires:** `envforge` on PATH (LSP + the `mcp-server` feature build for the
MCP context server).

## Install (dev)

1. Build the extension with the Zed toolchain (compiles to `wasm32-wasip2`):
   Zed → `zed: install dev extension` → select this `editors/zed/` directory.
2. Ensure `envforge` is on PATH.

## What you get

- LSP for `.env*` (and the `envforge/*` custom requests).
- `envforge-mcp` context server available to Zed's assistant — `list_keys` +
  `describe_schema`, redacted, audited, never raw values.
- The fence (`envforge fence`) writes the rules-chain files Zed honors
  (`AGENTS.md` canonical), so fencing Zed works from the CLI today.

## Notes

- `src/lib.rs` targets `zed_extension_api = "0.2"`. Zed's extension API evolves —
  verify the `language_server_command` / `context_server_command` signatures
  against the release you build with.
- Thin client: no business logic here (parity, FR22 — see
  `docs/ide-behavior-contract.md`).
