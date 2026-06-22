# EnvForge Integration Matrix (0.9 "Framework Config")

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

## Config file types — LSP feature matrix (Intent 036, Phase 1)

Features are implemented once in `envforge lsp` and served identically to every client (VS Code, IntelliJ, Neovim, and any generic LSP client). Client-specific capability flags do not alter feature dispatch; parity is enforced by `tests/cross_ide_release_tests.rs`.

`full` = feature fully supported (read + write where applicable). `read-only` = feature works but the file is never written. `—` = not applicable for this file type.

| File type | Hover | Completion | Go-to-def | Find-refs | Highlight | Diagnostics | Rename | Format |
|---|---|---|---|---|---|---|---|---|
| `.env` / `.env.*` / `*.env` | full | full | full | full | full | full | full | full |
| `.env.local` / `.env.{env}` cascade | full | full | full | full | full | full | full | full |
| `.env.schema` / `.env.schema.toml` | full | full | full | full | full | full | full | — |
| `application.properties` | full | full | full | full | full | full | full | full |
| `application-{profile}.properties` | full | full | full | full | full | full | full | full |
| `microprofile-config.properties` | full | full | full | full | full | full | full | full |
| `*.properties` (generic Quarkus) | full | full | full | full | full | full | full | full |
| `application.yml` / `application.yaml` | full | full | full | full | full | full | ✓ | — |
| `application-{profile}.yml` / `.yaml` | full | full | full | full | full | full | ✓ | — |

**YAML write boundary (Intent 038):** `application.yml`/`.yaml` files support surgical key rename (upgraded from `ReadOnly` to `ReadWrite` in Intent 038). Rename uses `SurgicalEdit` — a byte-range splice that leaves every byte outside the edited key span identical by construction; no whole-document re-serialization, no comment or whitespace drift. Format intentionally returns empty edits (`[]`) — rename-only per Open decision 1. Anchor/alias documents return `None` (documented gap, never silently mis-edits). Tested in `tests/cross_ide_release_tests.rs::test_parity_yaml_readwrite_identical_on_all_clients` and `tests/yaml_writes_tests.rs`.

**AI-safety parity:** all file types above — including YAML read-only — receive identical fence enforcement (values classified red/amber/green in the exposure map), redaction in hover/completion labels, canary detection, and AI-guard diagnostics on save. See `docs/ide-behavior-contract.md` for per-feature details.

**Validated client combinations:** the feature functions accept no client-capability flags, making them deterministic across clients by construction. Validated combinations recorded: VS Code + IntelliJ + Neovim × all file types above × hover/completion/go-to-def/find-refs/highlight/diagnostics.

## Deferred to Vision (not in 0.9)

- Long-tail fence tools as community registry data: Roo Code, Continue, Augment, Tabnine, OpenAI Codex (several already covered via `AGENTS.md`).
- MCP **remote** transport (Streamable HTTP + OAuth 2.1 / RFC 8707) and extra MCP tools (`exposure_map`, `canary_scan`, `canary_check`).
- First-party **Emacs** / **Sublime Text** plugins (native UI).
- Native **Zed** status-bar/gutter UI (blocked on Zed Visual Extension API, Draft RFC #53403).
- Standalone `envforge doctor --ai` (detection is folded into `fence --status` for 0.9).
- YAML format (document-wide canonical formatting) — deferred; rename is surgical (Intent 038). Full formatting would require comment-preserving round-trip serialization.
- YAML anchor/alias rename support — refused conservatively when anchors are present; tracked as a known gap in Intent 038.
- TOML config files (`application.toml`, Spring Boot 3.2+), `.NET appsettings.json`, Rails `database.yml` (Phase 2+).
- Full cross-format schema unification (single `.env.schema` governing properties + YAML + TOML).
