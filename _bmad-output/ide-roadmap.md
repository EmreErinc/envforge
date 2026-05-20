# EnvForge IDE Roadmap — VS Code + IntelliJ Parity

**Status:** Draft from party-mode roundtable (Winston, Sally, Amelia, Victor)
**Date:** 2026-05-15
**Project version:** v0.7.6
**Goal:** Both IDE plugins behave identically. Each feature below ships independently and does not block the others.

---

## Guiding rules (apply to every feature)

1. **LSP is the source of truth.** Anything that touches `.env`, `.env.schema`, or secrets lives in `envforge lsp`. Plugins render only.
2. **Behavior Contract first.** Each feature gets a row in `docs/ide-behavior-contract.md` with columns: trigger, wording, icon, keybind, LSP method. Both plugins implement against the row.
3. **Parity test harness.** Every feature gets a golden test in `tests/lsp_tests.rs` driving the LSP via JSON-RPC. Same input → byte-identical response.
4. **Feature flag per item.** Each ships behind a config toggle so partial rollout is safe.

---

## Phase 0 — Foundation (do once, unlocks all parallel work)

| # | Feature | Owner layer | Effort |
|---|---|---|---|
| F0.1 | `docs/ide-behavior-contract.md` skeleton | Docs | S |
| F0.2 | `tests/lsp_tests.rs` JSON-RPC harness | Tests | M |
| F0.3 | LSP capability manifest (versioned) | LSP | S |
| F0.4 | Strip env-parsing logic from VS Code plugin → call LSP | VS Code | M |
| F0.5 | Strip env-parsing logic from IntelliJ plugin → call LSP | IntelliJ | M |

Phase 0 is the only sequenced block. After it lands, every feature below is parallel.

---

## Phase 1 — LSP-only features (parity is free)

Each ships as a self-contained PR. No cross-feature blocking. Both plugins inherit automatically once LSP exposes the capability.

| # | Feature | LSP method | Effort |
|---|---|---|---|
| L1 | Schema-aware completion | `textDocument/completion` | S |
| L2 | Schema diagnostics (missing/type/unknown) | `textDocument/publishDiagnostics` | S |
| L3 | Hover with provenance | `textDocument/hover` | S |
| L4 | Go-to-definition (code → schema, var → export) | `textDocument/definition` | M |
| L5 | Find references | `textDocument/references` | M |
| L6 | Code actions / quick-fixes | `textDocument/codeAction` | M |
| L7 | Inlay hints (resolved value / `***redacted***`) | `textDocument/inlayHint` | M |
| L8 | Rename symbol (schema + .env + source) | `textDocument/rename` | M |
| L9 | Document + workspace symbols | `documentSymbol` + `workspace/symbol` | S |
| L10 | Formatting (canonical `.env`) | `textDocument/formatting` | S |
| L11 | Semantic tokens (secrets distinct) | `textDocument/semanticTokens` | M |
| L12 | Plaintext secret detection diagnostic | `publishDiagnostics` | S |
| L13 | Save-time AI-guard diagnostic | `publishDiagnostics` | S |
| L14 | MCP config linter (`.cursor/mcp.json`, `.vscode/mcp.json`, `.claude/settings.json`) | `publishDiagnostics` | M |

**Quick-fix payload for L6** (each independently shippable):
- Add missing key to `.env`
- Generate `.env` from schema
- Mark as secret
- Move to volatile
- Populate from provider (1Password / Vault / AWS SM / …)
- Harden MCP server entry
- Plant canary token here

---

## Phase 2 — LSP custom commands (parity-free via `workspace/executeCommand`)

Both plugins wire identical command IDs to identical menu entries.

| # | Command ID | Backing op | Effort |
|---|---|---|---|
| C1 | `envforge.sync.pull` / `sync.push` | `ops/sync` | S |
| C2 | `envforge.secrets.fetch` | `ops/secrets` | S |
| C3 | `envforge.fence.toggle` | `ops/fence` | S |
| C4 | `envforge.scan.canary` | `ops/canary` | S |
| C5 | `envforge.run.volatile` (wrap next run config) | `ops/volatile` | M |
| C6 | `envforge.reveal.value` (audit-logged) | `ops/audit` + reveal | M |

---

## Phase 3 — Per-IDE glue (twin-build, contract-enforced)

These genuinely need native APIs in each IDE. Same data, same wording, same icon — different rendering. Each row is independently shippable in either IDE first.

| # | Feature | VS Code API | IntelliJ API | Effort each side |
|---|---|---|---|---|
| P1 | Status bar item (fence · volatile · var count, color states) | `StatusBarItem` | `StatusBarWidget` | S |
| P2 | Fence toggle action (one-click red↔gray shield) | command + status bar click | `AnAction` + widget click | S |
| P3 | Volatile countdown indicator (`volatile: 7m left`) | timer + status bar | `Alarm` + widget | S |
| P4 | Secret reveal modal (confirm → reveal → clipboard auto-clear) | `showWarningMessage` + `env.clipboard` | `Messages` + `CopyPasteManager` | M |
| P5 | AI-exposure gutter heatmap (red/amber/green per line) | `DecorationType` | `LineMarkerProvider` | M |
| P6 | CodeLens above secret-shaped values | `CodeLensProvider` | `CodeVisionProvider` | M |
| P7 | Canary gutter glyph + hover explainer | `DecorationType` | `LineMarkerProvider` | S |
| P8 | Canary "plant here" quick-fix wiring | code action client | code action client | S |
| P9 | Project view decorations on `.env*` files (status badge) | `FileDecorationProvider` | `ProjectViewNodeDecorator` | M (VS Code) / L (IntelliJ) |

---

## Phase 4 — Heavy lifts (ship only after telemetry justifies)

Gate behind plugin install/retention data. Big surfaces, big maintenance, easy to build something nobody opens.

| # | Feature | Notes |
|---|---|---|
| H1 | EnvForge Explorer tree view / tool window | Same data source `envforge/listEnvs`. Different rendering. |
| H2 | Sync status tool window (conflicts, pending pushes) | Consider whether TUI already covers this. |
| H3 | Settings UI mirroring `.envforge.toml` | Keys identical both sides. |
| H4 | Webview / JCEF secrets manager panel | Last resort. |
| H5 | Inline secret provider browser (13 providers × 2 IDEs) | Defer until one provider dominates usage. |

---

## Suggested ship order (independent, ordered by leverage × effort)

Each line below is a standalone PR. Skip, reorder, or parallelize freely.

1. **F0.1–F0.5** — Foundation (must land first to unlock parallelism)
2. **L2** — Schema diagnostics (quick win, visible)
3. **L3** — Hover with provenance (kills the bug, ships brand)
4. **L1** — Schema completion via LSP (deletes duplicated bug surface)
5. **P1 + P2** — Status bar + fence toggle (demo-ready, screenshot gold)
6. **L7** — Inlay hints with redaction
7. **L14** — MCP config linter (sells to security teams)
8. **L4 + L8** — Go-to-definition + rename (unique vs marketplace)
9. **P5** — AI-exposure gutter heatmap (unique moat)
10. **L6** — Code actions / quick-fixes (compounds with everything above)
11. **L11** — Semantic tokens
12. **P3** — Volatile countdown
13. **P4 + C6** — Secret reveal with audit
14. **P7 + P8** — Canary visualization + quick-fix
15. **L12 + L13** — Secret detection + AI-guard save-time diagnostics
16. **L5 + L9 + L10** — References, symbols, formatting (table stakes polish)
17. **P6** — CodeLens on secrets
18. **C1–C5** — Custom commands wiring
19. **P9** — File decorations
20. **Phase 4** — Only after data justifies

---

## Open decisions (Emre to call)

- **Adoption-first vs retention-first.** Victor pushes unique features early (P2, P5, L14, P7/P8). Amelia/Winston push parity polish first (L1–L9). Pick a lane; the order above hedges by alternating.
- **Sync tool window in IDE or stays in TUI?** Amelia recommends TUI; if you agree, drop H2.
- **Provider browser scope.** Build for top-2 providers only, or skip entirely until signal?
- **Canary UI surface.** Sally wants gutter visibility; Amelia calls low-signal. Defaults to off via feature flag — flip on after first user request.

---

## Definition of done (per feature)

- LSP exposes capability + manifest entry
- Behavior Contract row written
- `tests/lsp_tests.rs` golden test green
- Both plugins call shared command ID / consume LSP method
- CHANGELOG + README test count updated
- Manual smoke: identical hover/menu/wording in both IDEs side-by-side
