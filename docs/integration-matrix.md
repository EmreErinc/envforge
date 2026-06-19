# EnvForge Integration Matrix (0.8.3 "Omnipresence")

What EnvForge covers across AI coding tools and editors. Generated from the
fence target registry (`src/ops/fence/registry.rs`) and the editor plugins.

## AI tools — fence coverage

`covered` = the tool has a native ignore mechanism EnvForge writes. `fallback`
= no ignore file exists, so EnvForge applies the tool's rules/deny mechanism +
`AGENTS.md` (still effective, but reported honestly — never a false "covered").

| Tool | Files written | Mechanism | Coverage | Source |
|---|---|---|---|---|
| Cursor | `.cursorignore`, `.cursorrules` | ignore + rules | covered | docs.cursor.com/context/ignore-files |
| GitHub Copilot | `.github/copilot-instructions.md` | rules | fallback | docs.github.com/.../copilot |
| Claude Code | `.claude/settings.json` | `permissions.deny Read()` | fallback | code.claude.com/docs/en/settings |
| Windsurf / Codeium | `.codeiumignore`, `.windsurf/rules/envforge.md` | ignore + rules | covered | docs.windsurf.com |
| Cline | `.clineignore`, `.clinerules` | ignore + rules | covered | docs.cline.bot |
| Aider | `.aiderignore` | ignore | covered | aider.chat/docs |
| Gemini CLI | `.geminiignore`, `GEMINI.md` | ignore + rules | covered | github.com/google-gemini/gemini-cli |
| Amazon Q | `.amazonq/rules/envforge.md` | rules (no ignore) | fallback | docs.aws.amazon.com/amazonq |
| **AGENTS.md** (cross-tool) | `AGENTS.md` | rules standard | fallback | agents.md |
| EnvForge | `.envforgeignore` | native ignore | covered | (EnvForge-native) |

`AGENTS.md` is honored by many tools (Codex, Zed, Cursor, Copilot, Gemini, Cline,
Roo, Junie, VS Code…), so writing it implicitly extends coverage to several
tools that lack a dedicated ignore file.

**Status & detection:** `envforge fence --status [--json]` reports each tool's
state (`covered`/`fallback`/`unfenced`/`not_installed`) and flags
installed-but-unfenced tools. Exit code gates CI (see `ci-gating.md`).

## Editors

| Editor | Integration | Capabilities |
|---|---|---|
| VS Code | first-party plugin | LSP, status bar, exposure gutter, file badges, MCP-config lint |
| IntelliJ Platform | first-party plugin | LSP, status widget, exposure gutter, project-view badges, MCP-config lint |
| Neovim | first-party plugin (`editors/nvim`) | LSP, statusline, exposure sign-column heatmap, fence toggle |
| Zed | first-party extension (`editors/zed`) | LSP + read-safe MCP context server (no custom UI — Zed API limit) |
| Helix · Emacs · Sublime · Lapce · Kakoune | LSP-only | full `textDocument/*` + `envforge/*` requests (see `lsp-clients.md`) |

JetBrains Fleet: **dropped** (discontinued Dec 2025).

## MCP (read-safe egress)

`envforge mcp serve` — stdio MCP server exposing `list_keys` + `describe_schema`
(redacted, audited, **no raw values**). Config snippets for Claude Desktop/Code,
Cursor, VS Code, Windsurf, Cline: see `mcp-server.md`.

## Capabilities by surface

| Capability | CLI | LSP | VS Code | IntelliJ | Neovim | Zed |
|---|---|---|---|---|---|---|
| Fence / status / detection | ✓ | ✓ (custom req) | ✓ | ✓ | ✓ | via CLI |
| Exposure classification | ✓ | ✓ | ✓ gutter | ✓ gutter | ✓ signs | — |
| MCP-config credential lint | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| MCP server (read-safe egress) | ✓ | — | ✓ client | ✓ client | ✓ client | ✓ extension |

## Deferred to Vision (not in 0.8.3)

- Long-tail fence tools as community registry data: Roo Code, Continue, Augment, Tabnine, OpenAI Codex (several already covered via `AGENTS.md`).
- MCP **remote** transport (Streamable HTTP + OAuth 2.1 / RFC 8707) and extra MCP tools (`exposure_map`, `canary_scan`, `canary_check`).
- First-party **Emacs** / **Sublime Text** plugins (native UI).
- Native **Zed** status-bar/gutter UI (blocked on Zed Visual Extension API, Draft RFC #53403).
- Standalone `envforge doctor --ai` (detection is folded into `fence --status` for 0.8.3).
