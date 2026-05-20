# EnvForge IDE — Audit + Detailed Implementation Plan

**Source roadmap:** `_bmad-output/ide-roadmap.md`
**Date:** 2026-05-15
**Version baseline:** v0.7.6

---

## Part 1 — Audit (current state vs roadmap)

### LSP module inventory (`src/lsp/`)

| Module | Lines | Capability | Status |
|---|---|---|---|
| `completion.rs` | 388 | Schema-aware key/value/$ref completion, secret redaction in labels | **Solid** |
| `hover.rs` | 78 | Schema fields (type/required/sensitive/desc/default/example/pattern/values/min/max/env_overrides) | **Schema-only — no provenance** |
| `diagnostics.rs` | 89 | Missing required, type validation, sensitive-value warning | **Missing unknown-key + MCP + AI-guard** |
| `code_action.rs` | 162 | Add missing, Use secret reference, Use default | **3/7 actions** |
| `definition.rs` | 37 | `.env` key → schema line | **Code → schema missing** |
| `code_lens.rs` | 89 | "sensitive" / "type:" / "required" decorations with EMPTY command IDs | **Decorative only — not actionable** |
| `document_symbol.rs` | 48 | Full | **Done** |
| `workspace_symbol.rs` | 68 | Full | **Done** |
| `folding_range.rs` | 85 | Comment + blank region folds | **Done** |
| `document.rs` | 228 | Parse + state | **Done** |
| `server.rs` | 597 | Backend, capabilities | **Done — needs new caps wired** |

### Roadmap mapping

| Roadmap ID | Feature | LSP exists? | VS Code? | IntelliJ? |
|---|---|---|---|---|
| F0.1 | IDE Behavior Contract doc | n/a | n/a | n/a — **not written** |
| F0.2 | LSP parity test harness | partial — `tests/lsp_phase1_tests.rs` (437 LOC) | n/a | n/a |
| F0.3 | LSP capability manifest | **missing** | n/a | n/a |
| F0.4 | VS Code plugin = thin LSP client | partial — has `commands.ts`, `statusbar.ts`, `security.ts`, `treeview.ts` | check | n/a |
| F0.5 | IntelliJ plugin = thin LSP client | partial — has `EnvForgeLspFactory.kt` + 14 other Kotlin files | n/a | check |
| L1 | Schema completion | **yes** | renders via LSP | renders via LSP |
| L2 | Schema diagnostics | partial — no unknown-key warning | renders | renders |
| L3 | Hover with provenance | **no — schema only** | renders | renders |
| L4 | Go-to-definition code → schema | partial — only `.env` → schema | n/a | n/a |
| L5 | Find references | **no** | n/a | n/a |
| L6 | Code actions / quick-fixes | partial — 3/7 | n/a | n/a |
| L7 | Inlay hints | **no** | n/a | n/a |
| L8 | Rename symbol | **no** | n/a | n/a |
| L9 | Document + workspace symbols | **yes** | renders | renders |
| L10 | Formatting | **no** | n/a | n/a |
| L11 | Semantic tokens | **no** | n/a | n/a |
| L12 | Plaintext secret detection | **yes** | renders | renders |
| L13 | Save-time AI-guard diagnostic | **no** | n/a | n/a |
| L14 | MCP config linter | **no** | n/a | n/a |
| C1–C6 | Custom commands (sync, fetch, fence, canary, run-volatile, reveal) | **no `workspace/executeCommand` registration** | command wiring needed | command wiring needed |
| P1 | Status bar | n/a | `statusbar.ts` exists | `EnvForgeStatusBarFactory.kt` exists |
| P2 | Fence toggle | needs `envforge.fence.toggle` command | partial | partial |
| P3–P9 | All other plugin features | most missing both sides | most missing | most missing |

### Headline gaps

1. **`code_lens.rs` lenses have empty `command` strings.** Decorations only. Need command IDs + handlers.
2. **`hover.rs` doesn't query `managed_vars`.** Hover never shows source file or current value. Quick win.
3. **`definition.rs` only maps `.env` → schema.** No code-file → schema (the killer feature).
4. **No `inlayHint`, `rename`, `formatting`, `semanticTokens`, `references` providers.** Server capabilities not declared.
5. **No `workspace/executeCommand` handler.** Blocks all C1–C6 commands.
6. **Both plugins ship logic beyond LSP rendering.** Need audit pass to identify duplicated parsing.

---

## Part 2 — Detailed implementation plan (per feature)

Each feature follows the same template. Order = roadmap ship order from `ide-roadmap.md`.

### Template

```
Feature: Lx — <name>
Capability: <LSP method or command ID>

LSP changes:
  - file(s): src/lsp/<file>.rs
  - server.rs: add capability + dispatcher entry
  - mod.rs: pub mod if new

VS Code changes:
  - file(s): editors/vscode/src/<file>.ts
  - package.json: contributes.{commands,menus,configuration}

IntelliJ changes:
  - file(s): editors/intellij/src/main/kotlin/com/envforge/intellij/<File>.kt
  - plugin.xml: extensions + actions

Tests:
  - tests/lsp_phase1_tests.rs : <test name>
  - Behavior contract row in docs/ide-behavior-contract.md

Done when:
  - LSP test green
  - VS Code render matches contract
  - IntelliJ render matches contract
```

---

### Phase 0 detail

**F0.1 — Behavior Contract doc.** Create `docs/ide-behavior-contract.md`. Columns: feature, trigger, LSP method, wording, icon, keybind (VS Code), keybind (IntelliJ), test ID. One row per feature below. No code changes.

**F0.2 — Extend parity test harness.** `tests/lsp_phase1_tests.rs` already drives the server. Add helper for hover/inlay/rename/format/references requests. Each future feature adds golden tests here.

**F0.3 — Capability manifest.** Add `docs/lsp-capability-manifest.md`. Versioned list: capability, since-version, custom-request name (if any), notification name (if any). Both plugins reference this file.

**F0.4 / F0.5 — Plugin slimming.** Grep VS Code `src/*.ts` and IntelliJ Kotlin files for any `.env` parsing not coming from LSP. Replace with LSP requests. Track findings in audit notes.

---

### Phase 1 detail — LSP features

#### L3 — Hover with provenance (first to implement)

Capability: `textDocument/hover` (already declared)

LSP changes:
- `src/lsp/hover.rs` — extend `hover_info` signature to accept `&[ManagedVar]` and optional fence state. Append sections after the existing schema lines:
  - `**Defined by:** <schema | local | managed | both>`
  - `**Current value:** <redacted preview if sensitive, raw otherwise, or "not set">`
  - `**Source file:** <basename(source_file)>`
  - `**AI fence:** <on | off>` (if fence state available; otherwise omit)
- `src/lsp/server.rs` — pass `managed_vars` (and later fence state) into `hover_info` call.
- Reuse `redact_value_for_label` helper from `completion.rs` — move it to a shared `src/lsp/redact.rs` (small refactor) so both modules share the redaction definition.

VS Code changes:
- None required — LSP markdown hover already renders.
- Smoke: hover over a key in a `.env` file with `.env.schema` present and managed vars loaded. Confirm provenance lines render.

IntelliJ changes:
- None required — lsp4ij renders markdown hover identically.
- Smoke: same as VS Code.

Tests:
- `tests/lsp_phase1_tests.rs::test_hover_provenance_managed_var` — open `.env` with a schema and a managed var; assert hover markdown contains "Defined by", "Current value:", "Source file:".
- `tests/lsp_phase1_tests.rs::test_hover_provenance_redacts_sensitive` — sensitive key shows redacted preview, not raw.

Behavior contract row:
- Trigger: hover over key in `.env` file
- Wording: exact section headers fixed in test
- Icon: n/a (markdown only)
- Keybind: VS Code default hover, IntelliJ default hover

Done when: cargo test green, manual smoke in both IDEs matches.

---

#### L2 — Schema diagnostics enhancement

Add unknown-key warning (key present in `.env` but absent from schema, when schema declares `strict_unknown_keys = true` or default mode).

LSP changes:
- `src/lsp/diagnostics.rs` — add unknown-key loop; severity `Warning`; gated by schema field `strict_mode` (default behaviour TBD per `EnvSchema`).
- Code action in `code_action.rs`: "Add `<KEY>` to schema" (writes to schema file URI).

Tests: `tests/lsp_phase1_tests.rs::test_diagnostic_unknown_key`.

---

#### L1 — Schema completion (already done, drift-proof)

Add regression test asserting both `.env.schema.toml` and legacy `.env.schema` produce identical completion responses for the same key.

---

#### L7 — Inlay hints

Capability: `textDocument/inlayHint` (new — declare in `server.rs`).

LSP changes:
- New `src/lsp/inlay.rs` — for each `EnvVar` entry: emit a hint after the value showing `(default)` if matches schema default, or `(secret: ***)` if sensitive and value populated.
- `mod.rs` + `server.rs` plumbing.

Tests: `test_inlay_hint_default` + `test_inlay_hint_sensitive_redaction`.

---

#### L4 — Go-to-definition from source code (the killer)

Capability: extend existing `goto_definition`.

LSP changes:
- `src/lsp/definition.rs` — detect when current URI is **not** an `.env` / schema file but a source file (`.ts`, `.js`, `.rs`, `.py`, `.go`, …). Identify env-var read patterns at cursor (`process.env.X`, `std::env::var("X")`, `os.environ["X"]`, `os.getenv("X")`).
- Resolve to schema line via existing `schema_line_map`.
- `server.rs` — register additional file extensions for `definition_provider` and ensure docs aren't gated by `is_env_file`.

Tests: language-by-language fixture in `tests/lsp_phase1_tests.rs`.

---

#### L8 — Rename symbol

Capability: `textDocument/rename` (new).

LSP changes:
- New `src/lsp/rename.rs` — produce `WorkspaceEdit` touching schema entry + every `.env*` file + (best-effort) source references.
- `server.rs` — declare `rename_provider`.

Tests: `test_rename_propagates_to_schema_and_env_files`.

---

#### L6 — Code actions expansion

Add to `code_action.rs`:
- "Generate `.env` from schema" — when active doc is empty or partial.
- "Mark `<KEY>` as secret" — toggles `sensitive = true` in schema.
- "Move `<KEY>` to volatile" — wraps next run config invocation (custom command `envforge.run.volatile`).
- "Populate from provider" — opens provider picker (custom command `envforge.secrets.fetch`).
- "Plant canary here" — emits a canary token via `envforge.scan.canary` plant variant.
- "Harden MCP server entry" — only on `mcp.json` URIs.

---

#### L14 — MCP config linter

LSP changes:
- `src/lsp/diagnostics.rs` — detect URI matches `**/mcp.json` or `**/.cursor/mcp.json` or `**/.claude/settings.json`. Parse JSON, run `ops::mcp_scan` rules, emit diagnostics inline.
- `server.rs` — extend `is_env_file` predicate or add `is_mcp_config_file` and trigger diagnostics path.

Tests: `test_mcp_diagnostic_unscoped_env`, `test_mcp_diagnostic_prompt_injection_pattern`.

---

#### L13 — Save-time AI-guard

LSP changes:
- New diagnostic batch run only on `did_save` for `.env*` files: pipe `ops::ai_guard` output to `Diagnostic` with severity `Warning`.

---

#### L5 — Find references, L9 — symbols, L10 — formatting, L11 — semantic tokens

Same template. Each gets its own `src/lsp/<feature>.rs` and capability declaration. Details deferred until L3 lands and contract proves out.

---

### Phase 2 detail — custom commands

Capability: `executeCommandProvider` (new — declare list in `server.rs`).

LSP changes:
- New `src/lsp/commands.rs` — dispatch table keyed on command ID:
  - `envforge.sync.pull` → `ops::sync::pull`
  - `envforge.sync.push` → `ops::sync::push`
  - `envforge.secrets.fetch` → calls existing provider ops
  - `envforge.fence.toggle` → `ops::fence`
  - `envforge.scan.canary` → `ops::canary`
  - `envforge.run.volatile` → emits launch arg to client
  - `envforge.reveal.value` → audit-logged reveal

VS Code changes per command:
- `editors/vscode/src/commands.ts` — register `vscode.commands.registerCommand("envforge.X", () => client.sendRequest("workspace/executeCommand", { command, arguments }))`.
- `package.json` — `contributes.commands` entries.

IntelliJ changes per command:
- New `actions/EnvForgeLspCommandAction.kt` base class — sends `workspace/executeCommand` via lsp4ij.
- `plugin.xml` — register each as `<action>`.

---

### Phase 3 detail — per-IDE glue

Each feature spec lives in the Behavior Contract. Examples:

**P2 — Fence toggle**
- LSP: backs via `envforge.fence.toggle` command.
- VS Code: status bar item `$(shield) EnvForge: AI BLOCKED`. Click → command.
- IntelliJ: `EnvForgeStatusBarFactory` already exists — add fence indicator widget. Click → action that sends LSP command.
- Wording: "EnvForge: AI BLOCKED" (red) / "EnvForge: AI ALLOWED" (gray). Tooltip: "Click to toggle AI fence." Same string both IDEs.

**P5 — AI exposure gutter heatmap**
- LSP custom notification: `envforge/exposureMap` — array of `{line, level: "red"|"amber"|"green"}` per file.
- VS Code: `TextEditorDecorationType` with gutter icon per level.
- IntelliJ: `LineMarkerProvider` rendering colored gutter glyphs.
- Wording: tooltip shows agent list + reason from `mcp scan` + `ai-guard`.

---

## Part 3 — Build / test workflow per feature

Run before claiming feature done:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
# If plugin code changed:
cd editors/vscode && npm run compile && npm test
cd editors/intellij && ./gradlew test verifyPlugin
```

CHANGELOG + README test-count update on every feature merge.

---

## Part 4 — First-feature pick

**L3 — Hover with provenance.** Reasons:
- Smallest LSP delta (extend existing `hover_info` signature).
- Exercises full pipeline: LSP returns markdown, both plugins render natively, parity test asserts markdown body.
- Visible to user immediately on first hover.
- Forces the redaction helper into a shared module — pays down duplication debt.
- Unblocks pattern other features re-use (managed_vars threading).

Implementation begins next.
