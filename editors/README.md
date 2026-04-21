# EnvForge IDE Extensions

EnvForge provides IDE integration through a built-in Language Server Protocol (LSP) server and editor-specific extensions.

## Architecture

```
envforge lsp (Rust binary, stdio transport)
    ├── VS Code Extension (TypeScript)
    └── IntelliJ Plugin (Kotlin, via LSP4IJ)
```

Both editors share the same LSP server. Same diagnostics, hover, completions, and go-to-definition everywhere.

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

### Feature Parity

| Feature | VS Code | IntelliJ |
|---------|---------|----------|
| Diagnostics (LSP) | Yes | Yes |
| Hover info (LSP) | Yes | Yes |
| Completions (LSP) | Yes | Yes |
| Go-to-definition (LSP) | Yes | Yes |
| Variables tree view | Yes (sidebar) | Yes (tool window) |
| Prefix grouping + toggle | Yes | Yes |
| Profiles tree view | Yes (sidebar) | Yes (tool window) |
| Profile switching | Click / command | Double-click / menu |
| Copy Key Name | Click + right-click | Double-click + right-click |
| Copy Value | Right-click | Right-click |
| Copy KEY=VALUE | — | Right-click |
| Status bar | Variable count | Variable count |
| Commands | 13 (command palette) | 11 (Tools menu) |
| Secret masking | Sensitive values masked | Sensitive values masked |

See each directory's README for installation instructions.
