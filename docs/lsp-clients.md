# LSP Client Setup

`envforge lsp` is a standard LSP server (stdio transport, JSON-RPC framed
with `Content-Length` headers). Any LSP-speaking editor can connect.

EnvForge ships first-party plugins for **VS Code** and **IntelliJ** that
add native UI on top of the LSP (status bar, gutter heatmap, file
decorations). Other editors get the textDocument capabilities by
default, plus the named `envforge/*` custom requests
(`envforge/exposureMap`, `envforge/fenceStatus`, `envforge/revealValue`,
`envforge/canaryScan`, `envforge/canaryCheck`, `envforge/volatileStatus`,
`envforge/volatileExtend`, `envforge/runVolatile`) on demand. The generic
`workspace/executeCommand` provider is **disabled** (returns
`MethodNotFound`) — security operations use the named requests instead.

| Editor | First-party plugin? | Setup |
|---|---|---|
| VS Code | ✓ `envforge-env-manager` 0.2.2 | Install via Marketplace or the bundled `.vsix` |
| IntelliJ Platform (Idea, GoLand, PyCharm, RustRover, …) | ✓ `envforge-intellij` 0.2.2 | Settings → Plugins → Install Plugin from Disk |

| Neovim | ✓ first-party plugin (`editors/nvim`) | statusline + exposure heatmap + fence toggle — see [`editors/nvim/README.md`](../editors/nvim/README.md) |
| Zed | LSP + MCP (first-party extension `editors/zed`) | LSP + read-safe MCP context server; no custom UI (Zed API limit) — see [`editors/zed/README.md`](../editors/zed/README.md) |
| Helix | LSP only | [config below](#helix) |
| Emacs (`eglot` / `lsp-mode`) | LSP only | [config below](#emacs) |
| Sublime Text (LSP package) | LSP only | [config below](#sublime-text) |
| Kakoune (`kak-lsp`) | LSP only | [config below](#kakoune) |
| Lapce | LSP only | [config below](#lapce) |
| Any other LSP client | LSP only | [generic notes](#generic-lsp-client-notes) |

> **JetBrains Fleet** was dropped as a target — Fleet was discontinued (Dec 2025, succeeded by "Air"). The IntelliJ Platform plugin is unaffected.

## What works in every LSP client (no plugin needed)

All `textDocument/*` capabilities advertised by `envforge lsp`:

- `completion` — schema-aware keys + `${REF}` completions, plus
  cross-environment keys/values from the project manifest (sensitive
  cross-env values are never offered raw)
- `publishDiagnostics` — schema validation, unknown-key warnings, MCP
  config credential findings, save-time AI-guard scan, cross-environment
  missing-key warnings (`envforge-env`)
- `hover` — schema info + value provenance; for manifest projects, which
  environments set the key (presence only — no raw values)
- `definition` — env file key → schema + the key's definitions across
  every recognized env file. (Source files → schema still implemented
  server-side; not attached by first-party clients.)
- `references` — schema declaration + every open `.env*` entry
- `rename` — atomic WorkspaceEdit across schema + open `.env*` docs
- `codeAction` — 7 quick-fixes (Add to schema, Use secret ref, Mark
  as secret, Use default, Plant canary, Add all missing keys, Generate
  .env from schema)
- `codeLens` — Plant canary / Activate fence on sensitive lines
- `inlayHint` — `(default)`, `→ <redacted>`, `(<type>)`
- `formatting` — canonical `.env` whitespace
- `semanticTokens/full` — variable/string/comment + readonly modifier
  on sensitive keys
- `documentSymbol`, `workspace/symbol`, `foldingRange`

**Env-file recognition.** The server recognizes a project's env files from the
`.envforge.project.toml` manifest (its `[[environments]]` list), in addition to
the conventional `.env*` set. Profile variants (`.env.development`,
`.env.stage`, `.env.production`, …) already match the first-party clients'
`.env.*` selectors; non-`.env*` filenames declared via `extra_files` are
recognized server-side but client attach for them is a documented limitation.
See [`docs/envforge-project-toml.md`](envforge-project-toml.md).

The named `envforge/*` security requests are also reachable from any
client (the generic `workspace/executeCommand` is disabled) — see
`docs/api-reference.md` for the request list and `docs/ide-behavior-contract.md`
for argument schemas.

## What requires a plugin

Native UI surfaces that have no LSP-standard channel:

- Status bar (`<N> vars · AI BLOCKED · volatile: 7m left`)
- AI-exposure gutter heatmap (red/amber/green dots, canary shield)
- File-explorer badges on `.env*` files
- Command-palette polish (confirm modals, terminal launchers, clipboard
  auto-clear)

These ship today only for VS Code + IntelliJ. The underlying data lives
behind the `envforge/exposureMap` custom request + `envforge exposure
--file PATH` CLI subcommand — any editor extension author can wire them
into native UI in their plugin.

---

## Neovim

`nvim-lspconfig` 0.1.7+, register as a custom server:

```lua
-- ~/.config/nvim/lua/lsp/envforge.lua
local lspconfig = require('lspconfig')
local configs = require('lspconfig.configs')

if not configs.envforge then
  configs.envforge = {
    default_config = {
      cmd = { 'envforge', 'lsp' },
      filetypes = { 'dotenv', 'sh', 'typescript', 'javascript',
                    'python', 'rust', 'go', 'java', 'kotlin' },
      root_dir = lspconfig.util.root_pattern(
        '.env.schema.toml', '.env.schema', '.env', '.git'
      ),
      settings = {},
    },
  }
end
lspconfig.envforge.setup({})
```

Filetype detection for `.env*`:

```vim
autocmd BufRead,BufNewFile .env,.env.*,*.env set filetype=dotenv
```

## Helix

```toml
# ~/.config/helix/languages.toml
[[language]]
name = "dotenv"
scope = "source.dotenv"
file-types = [{ glob = ".env" }, { glob = ".env.*" }, { glob = "*.env" }]
roots = [".env.schema.toml", ".env.schema", ".git"]
language-servers = ["envforge"]

[language-server.envforge]
command = "envforge"
args = ["lsp"]
```

## Emacs

`eglot`:

```elisp
(require 'eglot)
(add-to-list 'eglot-server-programs
             '((dotenv-mode) . ("envforge" "lsp")))
(add-to-list 'auto-mode-alist
             '("\\.env\\(\\..*\\)?\\'" . dotenv-mode))
(add-hook 'dotenv-mode-hook 'eglot-ensure)
```

`lsp-mode`:

```elisp
(with-eval-after-load 'lsp-mode
  (lsp-register-client
   (make-lsp-client
     :new-connection (lsp-stdio-connection '("envforge" "lsp"))
     :activation-fn (lsp-activate-on "dotenv")
     :server-id 'envforge)))
```

## Sublime Text

LSP package settings (Preferences → Package Settings → LSP → Settings):

```json
{
  "clients": {
    "envforge": {
      "enabled": true,
      "command": ["envforge", "lsp"],
      "selector": "source.env, source.dotenv"
    }
  }
}
```

## Zed

Zed supports custom LSP servers via extensions. Minimal extension scaffold:

```toml
# extension.toml
id = "envforge"
name = "EnvForge"
version = "0.1.0"
schema_version = 1

[language_servers.envforge]
name = "EnvForge"
language = "Dotenv"
```

Wire in `extension.rs`:

```rust
zed::register_language_server(zed::LanguageServerConfig {
    command: "envforge".into(),
    args: vec!["lsp".into()],
    ..Default::default()
});
```

Heavier than other editors — Zed needs a compiled extension.

## Kakoune

`kak-lsp`:

```toml
# ~/.config/kak-lsp/kak-lsp.toml
[language.dotenv]
filetypes = ["dotenv"]
roots = [".env.schema.toml", ".env.schema", ".git"]
command = "envforge"
args = ["lsp"]
```

## Lapce

Settings → Plugins → Install Custom LSP:

```toml
[server]
binary = "envforge"
args = ["lsp"]
file-types = ["env", "dotenv"]
```

## Generic LSP client notes

- **Transport:** stdio. The server reads JSON-RPC messages framed with
  `Content-Length: <N>\r\n\r\n<body>` headers.
- **Initialize:** standard. Capabilities cover all 13 textDocument
  methods listed above plus the named `envforge/*` custom requests
  (`exposureMap` + the security operations). The generic
  `executeCommandProvider` is **not** advertised (disabled).
- **Trigger characters:** `=`, `$`, `{`. After typing any of these,
  the server returns context-appropriate completions (key, value, or
  `${REF}` insertion).
- **Workspace root:** the server uses `rootUri` (or the first
  `workspaceFolders` entry) to canonicalize file paths and locate
  `.env.schema.toml` / `.env.schema`.
- **Source-file goto-definition (opt-in):** the server reads source
  files directly from disk on demand, gated by an extension allow-list
  (`.ts .tsx .js .jsx .mjs .cjs .py .rs .go .java .kt .rb .php .cs
  .sh`) and a 1 MiB size cap. Path canonicalization keeps the read
  inside the workspace root. **The first-party VS Code, IntelliJ, and
  Neovim clients no longer attach to source languages** (they attach
  only to EnvForge's own files — `.env*`, `.env.schema*`, MCP config),
  so this feature does not fire there. A generic LSP client can still
  use it by adding the source-language document selectors itself.
- **Custom request `envforge/exposureMap`:** see
  `docs/api-reference.md` for the wire format.
- **Named `envforge/*` security requests:** see `docs/api-reference.md`
  for the list and `docs/ide-behavior-contract.md` for per-request
  argument schemas. (`workspace/executeCommand` is disabled.)

## Verifying a new client

Quick smoke test from any LSP client:

1. Open a `.env` file with a few `KEY=VALUE` lines.
2. Trigger completion at the start of a line — schema keys appear.
3. Hover a key — schema info + provenance shows.
4. Save the file — diagnostics for missing required vars / type
   violations / sensitive values render in the client's diagnostic UI.
5. Ctrl-click `${SOMEKEY}` inside another line — should jump to that
   key's declaration.

If any of those fail, the client is misconfigured. If all of those
work but a specific custom command does not, check the client's
`workspace/executeCommand` support and verify it forwards
`{ command, arguments }` payloads verbatim to the server.
