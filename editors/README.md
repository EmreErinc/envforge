# EnvForge IDE Extensions

EnvForge is a Rust CLI + TUI for environment variables (profiles, `.env.schema`, secret scanning). Editor plugins are thin clients of `envforge lsp` and the CLI.

## Architecture

```
envforge lsp (Rust binary, stdio transport)
    ├── VS Code Extension (TypeScript)
    └── IntelliJ Plugin (Kotlin, via LSP4IJ)
```

Both editors share the same LSP server. Same diagnostics, hover, completions, and go-to-definition everywhere. Plugin packages are versioned separately from the CLI (plugins **0.3.0**, CLI **1.0.3**).

Store publish is **not** tied to CLI tags. Bump the plugin version on `main` and [publish-editors.yml](../.github/workflows/publish-editors.yml) sends VS Code Marketplace (and Open VSX if `OVSX_PAT` is set) plus JetBrains Marketplace. Neovim and Zed have no equivalent token publish (Zed goes through the `zed-industries/extensions` repo).

## Language Server

The LSP server is built into the `envforge` binary. No separate installation needed.

### Start the server

```bash
envforge lsp
```

This starts an LSP server on stdio (stdin/stdout). It is not meant to be run manually — IDE extensions launch it automatically.

### Capabilities

| Feature | Description |
|---------|-------------|
| **Diagnostics** | Missing required variables, type validation errors, secret leak warnings |
| **Hover** | Show type, description, default, example, sensitive flag from `.env.schema` |
| **Completions** | Suggest key names from `.env.schema` that are not yet in the file |
| **Go-to-definition** | Jump from a key in `.env` to its `[KEY]` section in `.env.schema` |

### Supported files

- `.env`, `.env.*`, `*.env` — validated against schema
- `.env.schema` — editing triggers re-validation of all open `.env` files

### How diagnostics work

1. On file open/change, the LSP parses the `.env` file
2. It loads `.env.schema` from the workspace root
3. It checks:
   - **Required variables** — keys marked `required = true` without defaults must be present
   - **Type validation** — values checked against declared type (number, bool, url, email, port, enum, regex)
   - **Secret scanning** — keys matching sensitive patterns (SECRET, TOKEN, PASSWORD, KEY) with real-looking values get warnings

### Configuration for other editors

Any editor with LSP support can use `envforge lsp`. Configure it as:

- **Command:** `envforge lsp`
- **Transport:** stdio
- **File patterns:** `.env`, `.env.*`, `*.env`, `.env.schema`

#### Neovim (nvim-lspconfig)

```lua
local lspconfig = require('lspconfig')
local configs = require('lspconfig.configs')

configs.envforge = {
  default_config = {
    cmd = { 'envforge', 'lsp' },
    filetypes = { 'sh', 'conf' },
    root_dir = lspconfig.util.root_pattern('.env.schema', '.env'),
    settings = {},
  },
}

lspconfig.envforge.setup({})
```

#### Helix

Add to `~/.config/helix/languages.toml`:

```toml
[[language]]
name = "env"
scope = "source.env"
file-types = [".env", { glob = ".env.*" }]
language-servers = ["envforge"]

[language-server.envforge]
command = "envforge"
args = ["lsp"]
```

#### Sublime Text (LSP package)

Add to LSP settings:

```json
{
  "clients": {
    "envforge": {
      "enabled": true,
      "command": ["envforge", "lsp"],
      "selector": "source.env"
    }
  }
}
```

## Extensions

| Editor | Directory | Status |
|--------|-----------|--------|
| [VS Code](vscode/) | `editors/vscode/` | Ready — `.vsix` packaged |
| [IntelliJ](intellij/) | `editors/intellij/` | Ready — Gradle project |
| [Neovim](nvim/) | `editors/nvim/` | Ready — Lua plugin |
| [Zed](zed/) | `editors/zed/` | Ready — Extension manifest |


### Feature Parity

Both plugins are built on the same LSP server and CLI binary. Native UI differs (see table). Zed is a separate extension: LSP only, no gutter or status bar.

| Feature | VS Code | IntelliJ |
|---------|---------|----------|
| **LSP: Diagnostics** | ✅ | ✅ |
| **LSP: Hover info** | ✅ | ✅ |
| **LSP: Completions** | ✅ | ✅ |
| **LSP: Go-to-definition** | ✅ `.env` → schema | ✅ `.env` → schema |
| **LSP: MCP config inline diagnostics** | ✅ | ✅ |
| **Variables tree view** | ✅ Sidebar panel | ✅ Tool window |
| **Profiles tree view** | ✅ Sidebar panel | ✅ Tool window |
| **Security dashboard** | ✅ Sidebar panel | ✅ Tool window |
| **Prefix grouping + toggle** | ✅ | ✅ |
| **Profile switching** | ✅ Click / command | ✅ Double-click / menu |
| **Profile diff** | ✅ | ✅ |
| **Copy Key Name** | ✅ Click + right-click | ✅ Double-click + right-click |
| **Copy Value** | ✅ Right-click | ✅ Right-click |
| **Copy KEY=VALUE** | ✅ Right-click | ✅ Right-click |
| **Add / Edit / Delete / Rename variable** | ✅ | ✅ |
| **Search variables** | ✅ (Ctrl+Shift+F) | ✅ |
| **Status bar widget** | ✅ Variable count | ✅ Variable count |
| **Gutter exposure heatmap** | ✅ | — |
| **File explorer badge** | ✅ | — |
| **Fence Inactive inspection** | — | ✅ |
| **Project view decorator** | — | ✅ |
| **Toggle AI Fence** | ✅ | ✅ |
| **Toggle AI Guard** | ✅ | ✅ |
| **MCP Scan** | ✅ | ✅ |
| **Add / Remove Canary Token** | ✅ | ✅ |
| **Canary Check** (list triggered) | ✅ | ✅ |
| **Canary Scan** (scan text/file) | ✅ | ✅ |
| **Run Volatile Session** | ✅ | ✅ |
| **Reveal Value (audit-logged)** | ✅ | ✅ |
| **Extend Volatile Lease** | — | ✅ |
| **Lifecycle Check** | ✅ | ✅ |
| **Manage Lifecycle Rules** | ✅ | ✅ |
| **View Audit Trail** | ✅ | ✅ |
| **Show Unused Secrets** | ✅ | ✅ |
| **Show Usage Summary** | ✅ | ✅ |
| **Monitor Stream** | ✅ | ✅ |
| **Validate Against Schema** | ✅ | ✅ |
| **Scan for Secret Leaks** | ✅ | ✅ |
| **Export Variables** | ✅ | ✅ |
| **Sync Push / Pull / Status** | ✅ | ✅ |
| **Generate Schema** | ✅ | ✅ |
| **Health Check (doctor)** | ✅ | ✅ |
| **Run All Checks** | ✅ | ✅ |
| **Run Project Wizard** | ✅ | ✅ |
| **Initialize Project** | ✅ | ✅ |
| **Restart LSP** | ✅ | — |
| **Getting Started walkthrough** | ✅ | — |

See each directory's README for installation instructions.
