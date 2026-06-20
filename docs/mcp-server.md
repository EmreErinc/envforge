# EnvForge MCP Server

`envforge mcp serve` runs a **read-safe** Model Context Protocol server over stdio, so MCP-capable AI agents can get env-var metadata through a guarded, audited channel instead of reading `.env` files raw.

> Built behind a Cargo feature. Install/build with it enabled:
> ```bash
> cargo install --path . --features mcp-server     # or
> cargo build --release --features mcp-server
> ```
> Without the feature, `envforge mcp serve` is not present (core fence commands are unaffected).

## What it exposes (and what it never does)

| Tool | Returns |
|---|---|
| `list_keys` | env-var key **names** (schema ∪ project `.env`), sorted, deduped — no values |
| `describe_schema` | per key: type, required, default, example, description, sensitive, and `current_value` = `"***"` (set) or `null` (unset) |

- **No raw secret value ever crosses the wire.** There is no `get_value`-class tool. `current_value` is only ever `"***"` or `null`. Enforced by construction and a 256-case property test (NFR-S1).
- **Every call is audited** to the EnvForge monitor bus (`MCP <tool>`, value excluded). Grep `MCP ` in `envforge monitor` to review agent access.
- **Workspace-scoped:** the server reads the schema (`.env.schema` / `.env.schema.toml`) and `.env` from its working directory.

## Client configuration

The server is a stdio command: `envforge mcp serve`. The standard server entry:

```json
{
  "command": "envforge",
  "args": ["mcp", "serve"]
}
```

### Claude Code

Project-scoped, committable — `.mcp.json` at repo root:

```json
{
  "mcpServers": {
    "envforge": { "command": "envforge", "args": ["mcp", "serve"] }
  }
}
```

Or via CLI: `claude mcp add envforge -- envforge mcp serve`

### Claude Desktop

`claude_desktop_config.json`:
- **macOS:** `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Windows:** `%APPDATA%\Claude\claude_desktop_config.json`
- ⚠️ **Linux:** path unverified (no official Linux build) — confirm against your install.

```json
{
  "mcpServers": {
    "envforge": { "command": "envforge", "args": ["mcp", "serve"] }
  }
}
```

### Cursor

Project `.cursor/mcp.json` (or global `~/.cursor/mcp.json`):

```json
{
  "mcpServers": {
    "envforge": { "command": "envforge", "args": ["mcp", "serve"] }
  }
}
```

### VS Code (GitHub Copilot agent mode)

Workspace `.vscode/mcp.json` (MCP GA since VS Code 1.102). Note VS Code uses the `servers` key + `type`:

```json
{
  "servers": {
    "envforge": { "type": "stdio", "command": "envforge", "args": ["mcp", "serve"] }
  }
}
```

### Windsurf (Cascade)

⚠️ Config path from a third-party guide — **verify before relying on it**:
`~/.codeium/windsurf/mcp_config.json` (macOS/Linux) · `%USERPROFILE%\.codeium\windsurf\mcp_config.json` (Windows).

```json
{
  "mcpServers": {
    "envforge": { "command": "envforge", "args": ["mcp", "serve"] }
  }
}
```

### Cline

⚠️ Config lives in VS Code globalStorage and varies by VS Code variant/OS — **verify against your install**: `cline_mcp_settings.json` under
`~/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/` (macOS, VS Code stable).

```json
{
  "mcpServers": {
    "envforge": { "command": "envforge", "args": ["mcp", "serve"] }
  }
}
```

## Notes

- Spec: conforms to MCP `2025-11-25` over **stdio** (local). Remote Streamable HTTP + OAuth is deferred (Vision).
- If `envforge` is not on `PATH`, use an absolute path in `command`.
- ⚠️ Paths flagged above (Windsurf/Cline exact strings, Claude Desktop Linux) are from secondary sources per the PRD research appendix — confirm before publishing in user-facing docs.
