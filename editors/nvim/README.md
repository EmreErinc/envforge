# EnvForge — Neovim plugin

First-party Neovim integration for [EnvForge](../../). Layers native UI on top of
`envforge lsp` and the CLI: LSP diagnostics/hover/completion, an AI-exposure
heatmap in the sign column, a statusline indicator, and a fence-toggle command.

**Requires:** Neovim **0.10+** and the `envforge` binary on `PATH`.

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

## Features

- **LSP** — auto-starts `envforge lsp` for `.env*` and source files (TS/JS/Py/Rust/Go/…),
  giving schema completion, hover with provenance, diagnostics, MCP-config credential
  warnings, and the `envforge/*` custom requests.
- **Exposure heatmap** — red/amber/green signs per env-var line (shield glyph for
  canary tripwires), from `envforge exposure <file>`. Refreshes on read/save.
- **Statusline** — `12 vars · AI BLOCKED`. Add to your statusline:
  ```lua
  vim.o.statusline = "%{v:lua.require'envforge'.statusline()}"
  ```
  or in lualine: `sections.lualine_x = { require("envforge").statusline }`.
- **Commands** — `:EnvForgeFence`, `:EnvForgeFenceToggle`, `:EnvForgeStatus`.

## Design

This plugin is a **thin client**: every decision (fence state, exposure level,
diagnostics) comes from the EnvForge binary. It re-implements no logic — parity
with the VS Code / IntelliJ plugins is guaranteed because all three consume the
same LSP behavior and CLI JSON (see `docs/ide-behavior-contract.md`).

No custom UI beyond what Neovim natively supports (signs, statusline, commands).
