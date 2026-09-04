# EnvForge — Neovim plugin

First-party Neovim client for [EnvForge](../../): a Rust **CLI + TUI** for env
**profiles** (dev / staging / prod), `.env.schema`, and secret scanning.

This plugin is a thin layer on `envforge lsp` and the CLI: diagnostics/hover/completion,
an exposure heatmap in the sign column, a statusline indicator, and fence commands.
It does not reimplement EnvForge logic.

**Requires:** Neovim **0.10+** and the `envforge` binary on `PATH`
(`brew install emreerinc/tap/envforge` or `cargo install env-forge-tui`).

Fence writes ignore/rules for configured AI tools; it is **not a sandbox**.
`envforge mcp serve` is optional (`--features mcp-server`) and is not in the
default brew/GitHub binaries.

## Install

lazy.nvim:

```lua
{
  dir = "/path/to/envforge/editors/nvim", -- or a packaged repo
  config = function() require("envforge").setup() end,
}
```

packer / manual: ensure `editors/nvim/lua` is on `runtimepath`, then
`require("envforge").setup()`.

```lua
require("envforge").setup({
  bin = "envforge",        -- override binary path if not on PATH
  -- filetypes = { ... },  -- override the LSP-attached filetypes
})
```

## What this plugin adds

- **LSP** — auto-starts `envforge lsp` for `.env*` and source files (TS/JS/Py/Rust/Go/…):
  schema completion, hover (values redacted), diagnostics, MCP-config credential
  warnings, and `envforge/*` custom requests.
- **Exposure heatmap** — red/amber/green signs per env-var line (shield glyph for
  canary tripwires), from `envforge exposure <file>`. Refreshes on read/save.
- **Statusline** — e.g. `12 vars · AI BLOCKED` when fence is active (fence ≠ sandbox). Add to your statusline:
  ```lua
  vim.o.statusline = "%{v:lua.require'envforge'.statusline()}"
  ```
  or in lualine: `sections.lualine_x = { require("envforge").statusline }`.
- **Commands** — `:EnvForgeFence`, `:EnvForgeFenceToggle`, `:EnvForgeStatus`.

No custom UI beyond what Neovim natively supports (signs, statusline, commands).
Parity with VS Code / IntelliJ comes from the same LSP and CLI JSON
(see `docs/ide-behavior-contract.md`).
