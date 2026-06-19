---
stepsCompleted: [1, 2, 3, 4, 5, 6, 7, 8]
status: 'complete'
completedAt: '2026-06-19'
lastStep: 8
inputDocuments:
  - 'docs/prd.md'
  - 'docs/research/ai-tooling-landscape-2025-2026.md'
  - 'docs/ide-behavior-contract.md'
  - 'docs/lsp-clients.md'
  - 'CLAUDE.md'
workflowType: 'architecture'
project_name: 'EnvForge'
user_name: 'Emre'
date: '2026-06-19'
---

# Architecture Decision Document — EnvForge v0.9 "Omnipresence"

_Builds collaboratively through step-by-step discovery. Sections appended per architectural decision._

**Author:** Emre · **Date:** 2026-06-19 · **Drives:** PRD `docs/prd.md` (FR1–FR26, NFRs)

## Project Context Analysis

### Requirements Overview

**Functional Requirements (29 FRs, 6 capability groups):**
- *AI-Tool Fence Coverage* (FR1–FR6, FR2a/FR2b) → a **data-driven fence registry** replacing the current 5-variant `FenceTarget` enum; per-tool mechanisms (ignore / rules / deny / `AGENTS.md` cross-tool); content-preserving disable.
- *Coverage Visibility & Detection* (FR7–FR11) → per-tool status, installed-but-unfenced state, JSON + exit codes, a `doctor --ai` detector.
- *Safe AI Egress* (FR12–FR17) → a new **`envforge mcp` server** with a read-safe, redacted, audited tool set; no raw-value tool.
- *Leak Linting* (FR18–FR19) → expand the existing `envforge-mcp` LSP diagnostic's `documentSelector` glob set.
- *Editor Reach* (FR20–FR23, FR23a) → first-party **Neovim** plugin; **Zed** via LSP + MCP (no custom UI); maintain LSP-only matrix; drop Fleet.
- *Governance & Docs* (FR24–FR26) → CI gating, per-entry source URLs, integration matrix.

**Non-Functional Requirements (21):** Security (no raw value on MCP wire — NFR-S1; redaction-by-default; audit), Performance (fence < 500 ms p95, status < 300 ms p95), Reliability (byte-identical round-trip, atomic writes, idempotent, partial-failure isolation, MCP process isolation), Compatibility (Linux+macOS, Rust 1.75, additive LSP contract, MCP spec `2025-11-25` over stdio for v0.9; remote/OAuth deferred), Maintainability (registry-as-data, build-time validation, parity tests).

### Scale & Complexity

- **Primary domain:** systems/CLI tool (Rust) — multi-faceted binary: CLI + TUI + LSP + (new) MCP server + IDE plugins.
- **Complexity:** High — security-critical, adversarial threat model, broad integration surface, strict parser/round-trip invariants.
- **Architectural components touched/added:** ~6 — fence registry (refactor `ops/fence.rs`), MCP server (`src/mcp/` + `mcp` subcommand), LSP diagnostic glob expansion, `doctor --ai` detector (`ops/`), Neovim plugin (new `editors/nvim/`), Zed extension (new `editors/zed/`).

### Technical Constraints & Dependencies

- **Brownfield invariants (non-negotiable):** parser parse→serialize byte-identical; atomic tempfile+rename writes; protected zones (conda, Amazon Q) untouched; `ops/` = pure logic, no I/O decisions; LSP is the single behavioral source of truth; thin CLI/TUI/plugin layers; `thiserror` errors with context.
- **Existing surface to extend, not fork:** `ops::fence` (`FenceTarget` enum + `FenceTargets` + `create_fence`/`remove_fence`/`check_fence_status` + `resolve_fence_targets`); `ops::mcp_scan` heuristics (reused by both the LSP linter and any new MCP-config checks); `lsp::redact::redact_for_label`; `lsp::server` custom `envforge/*` requests; `ops::canary`, `ops::lease`, `ops::audit`/monitor bus.
- **New external dependency:** an MCP server implementation (Rust MCP SDK or hand-rolled JSON-RPC over the existing framing) — decision deferred to the tech-stack step.

### Cross-Cutting Concerns

- **Redaction** must wrap every new egress (MCP responses, status/detection output) by construction.
- **Audit** every access-class operation to the monitor bus, value excluded.
- **Parity** — every IDE-visible behavior implemented once in `ops/`/`lsp`, rendered identically by all plugins; enforced by parity tests (`ide-behavior-contract.md` discipline extended).
- **Registry correctness** — one source of truth for tool conventions, versioned, source-cited, build-time validated.
- **Round-trip + atomicity** — applies to every new fence-target writer.

## Starter Template Evaluation

### Primary Technology Domain

Systems/CLI tool in **Rust** — **brownfield**. EnvForge is an established Cargo workspace (single binary, Rust ≥ 1.75, ratatui/crossterm/clap/tokio/age, 1191+ tests). **No starter template applies** — a scaffold would conflict with the existing structure. v0.9 extends the current tree (`src/ops/`, `src/lsp/`, `src/cli/`, `editors/`), it does not re-bootstrap.

### One New Dependency: MCP Server Library

The only genuinely new foundation is the MCP server runtime (FR12, NFR-C4). Web-verified (June 2026):

- **`rmcp`** — the **official** Rust MCP SDK (`modelcontextprotocol/rust-sdk`), ~4.7M downloads, macro-driven tool/resource/prompt API, **pluggable transport** with built-in stdio + Streamable HTTP, Tokio-based.
- **Fit:** EnvForge already depends on tokio; stdio transport matches the v0.9 NFR-C4 target (remote Streamable HTTP available in `rmcp` if/when needed later); macro API keeps the server thin so it can delegate straight into `ops/`.
- **Decision:** adopt **`rmcp`** for `envforge mcp`, pinned to a spec-`2025-11-25`-compatible release, behind a Cargo feature flag (`mcp-server`) so the core fence breadth ships even if MCP is gated. Alternative considered — hand-rolled JSON-RPC over the existing LSP framing — rejected: re-implements transport/auth/OAuth that `rmcp` provides and maintains.

**Note:** No project-init story needed (brownfield). First MCP story = add `rmcp` dependency + `mcp` subcommand skeleton.

Sources: [rmcp README (official rust-sdk)](https://github.com/modelcontextprotocol/rust-sdk/blob/main/crates/rmcp/README.md) · [rmcp on lib.rs](https://lib.rs/crates/rmcp) · [docs.rs/rmcp](https://docs.rs/rmcp)

## Core Architectural Decisions

### Decision Priority Analysis

**Critical (block implementation):** D1 fence-registry data model · D2 fence-writer abstraction · D5 MCP egress safety model.
**Important (shape architecture):** D3 status/coverage model · D4 detection · D6 MCP server placement · D7 LSP linter expansion.
**Deferred / nice-to-have:** D8 plugin distribution (Band-2), D9 Zed MCP-extension packaging.

### D1 — Fence target registry data model (Data Architecture analog)

**Decision:** Replace the 5-variant `FenceTarget` enum + bespoke writers with a **compile-time static registry** — a `&'static [FenceTargetSpec]` table in `ops/fence/registry.rs`.

```text
FenceTargetSpec {
  id: &str,                 // snake_case, stable (status/config keys)
  display: &str,            // "Cursor", "Windsurf / Codeium"
  tool: &str,               // marketing name for status output
  files: &[TargetFile],     // one or more
  detection: &[&str],       // path/marker hints for doctor --ai
  source_url: &str,         // FR25 verifiability
  has_real_ignore: bool,    // false ⇒ fallback-only (honest status, FR2b)
}
TargetFile {
  path: &str,               // ".cursorignore", ".claude/settings.json", "AGENTS.md"
  kind: FileKind,           // Ignore | Rules | DenyRule | CrossTool
  ownership: Ownership,     // FullyOwned (delete on disable) | Shared (surgical strip)
}
```

**Rationale:** const data → zero runtime parse, type-safe, and **build-time validated by tests** (NFR-M2) — an invalid/incomplete entry fails CI. Chosen over embedded TOML (would add a runtime parse + a parse-failure mode for data we control). `FenceTarget` collapses to a thin handle/index into the registry; `FenceTargets`/`resolve_fence_targets`/`fence config` iterate the table instead of hardcoded match arms. **Affects:** `ops/fence.rs` (refactor), `lsp/commands.rs` (`fence.config`/`fence.status`), FR1–FR6/FR25, NFR-M1/M2.

### D2 — Fence writer abstraction (by FileKind)

**Decision:** One writer/stripper dispatched on `FileKind`, replacing per-target `write_*` functions:
- **Ignore** → marked line-block (existing `FENCE_MARKER` convention) listing `.env*` globs.
- **Rules / CrossTool** → marked markdown block (the Secret-Safety-Rules text) appended to `*instructions*.md` / `AGENTS.md`.
- **DenyRule** → structure-aware JSON merge: insert `Read(...)` deny entries into `permissions.deny` in `.claude/settings.json` without disturbing sibling keys.
Each kind implements `write_block(dir, spec, file) -> WriteOutcome` and `strip_block(...)` honoring `Ownership`. **Rationale:** new tools reuse a kind, never new write logic (NFR-M1); each kind has its own round-trip + content-preservation test (NFR-R1/S4). **Affects:** `ops/fence.rs`, FR3/FR5/FR6.

### D3 — Coverage & status model (honest reporting)

**Decision:** `fence --status` resolves the registry and emits, per target: `covered` | `fallback` (rules/deny only, `has_real_ignore=false`) | `unfenced` | `not_installed`. Aggregate "AI BLOCKED" is true **only** when every *detected* target is `covered` or `fallback`; any detected-but-`unfenced` → amber. JSON shape extends the existing `fence.status` `resolved_targets` contract. **Rationale:** FR7/FR8/FR11 "no silent holes"; reuses v0.8.3 `resolved_targets` so plugins/CLI don't diverge. **Affects:** `ops/fence.rs`, `lsp` `fence.status`, status-bar tooltip (no plugin logic change).

### D4 — AI-tool detection (folded into `fence --status`)

**Decision:** A detection fn in `ops/fence` (or small `ops/detect.rs`) matches each registry entry's `detection` hints against workspace + known config dirs, returning `{tool, installed, fenced, mechanism}`, surfaced **inside `fence --status`** (no separate command for v0.9). **Rationale:** serves FR8/FR10 recovery journey without a second CLI surface; scope-trimmed per over-engineering analysis. A richer standalone `doctor --ai` is deferred to Vision. **Affects:** `ops/fence.rs` (+ maybe `ops/detect.rs`), `cli` status path.

### D5 — MCP egress safety model (Security)

**Decision:** The `envforge mcp` server exposes a **fixed allowlist** of read-safe tools — **for v0.9 just `list_keys` and `describe_schema`** — each delegating to the same `ops/` fn the LSP/CLI use. **No `get_value`-class tool is registered** (the absence is the control). Every tool's output passes through `lsp::redact::redact_for_label` (relocated/shared as `ops::redact`) before serialization. A `RuntimeEvent` is emitted per call (value excluded). A **property/fuzz test** asserts no high-entropy token appears in any response over randomized state (NFR-S1). **Rationale:** structural, not filtered, secret-safety; maps to OWASP MCP01/MCP03. Tool surface kept minimal per the over-engineering analysis — `exposure_map`/`canary_scan`/`canary_check` (already in CLI/LSP) deferred to Vision. **Affects:** new `src/mcp/`, `ops::redact`, `ops::audit`.

### D6 — MCP server placement & transport (API & Communication)

**Decision:** New `src/mcp/` module + `envforge mcp` subcommand built on **`rmcp`** behind Cargo feature `mcp-server`. Transport: **stdio only for v0.9** (NFR-C4, spec `2025-11-25`) — the transport every mainstream local MCP client uses. Runs as its own process/transport → a fault can't take down the LSP (NFR-R5). Tools are thin `rmcp` macro handlers → `ops/`. **Remote Streamable HTTP + OAuth 2.1 / RFC 8707 deferred to Vision** (per over-engineering analysis: adds a network/auth attack surface to a security tool with no current remote consumer). **Rationale:** reuse official SDK; isolation; feature-flag de-risks spec movement; stdio-only keeps surface minimal. **Affects:** `Cargo.toml`, `src/mcp/`, `cli`.

### D7 — LSP MCP-config linter expansion

**Decision:** Additive only — extend the `envforge-mcp` diagnostic's `documentSelector` glob set (`.vscode/mcp.json`, `.cursor/mcp.json`, Claude `.mcp.json`/`~/.claude.json`, Windsurf/Cline configs); reuse `ops::mcp_scan` heuristics unchanged. Plugin `documentSelector`s + IntelliJ `languageMapping` patterns extended to match. **Rationale:** FR18/FR19 with zero new scan logic; preserves parity. **Affects:** `lsp/mcp_diagnostics.rs`, plugin manifests, `ide-behavior-contract.md` (L14 row).

### D8 — Plugin distribution (Editor Reach, Band-2)

**Decision:** New `editors/nvim/` (Lua plugin: statusline + `sign_column`/extmark exposure heatmap + fence-toggle cmd, consuming `envforge lsp` + `envforge exposure --file`); `editors/zed/` (extension manifest registering the LSP language server **and** the `envforge mcp` server — no custom UI per Zed API limits). Both are thin clients; **no business logic** (FR22). **Rationale:** FR20/FR21; Neovim native UI viable, Zed UI is not (Draft RFC). **Affects:** new `editors/` subdirs, docs matrix.

### Decision Impact Analysis

**Implementation sequence:** D1 (registry) → D2 (writers) → D3 (status) / D4 (detection) → D7 (linter) → D5+D6 (MCP server) → D8 (plugins). D1 is the keystone; D2/D3/D4 depend on it. MCP (D5/D6) is independent of fence work and can parallelize. D8 depends on a stable LSP contract (unchanged).

**Cross-component dependencies:** `ops::redact` shared by fence-status, MCP, exposure. Registry `id`s are the stable keys across CLI/LSP/status/config — renaming is a breaking change. `resolved_targets` JSON contract is consumed by both plugins → extend, never reshape.

## Implementation Patterns & Consistency Rules

These pin choices where parallel AI agents building v0.9 stories could diverge. Defaults follow existing EnvForge conventions (CLAUDE.md); deviations are bugs.

### Naming

- **Registry `id`:** `snake_case`, stable, == status/config key (`cursor`, `copilot`, `claude_code`, `windsurf`, `cline`, `aider`, `continue_dev`, `gemini_cli`, `roo_code`, `jetbrains_ai`, `augment`, `agents_md`). Never rename without a migration note.
- **MCP tool names:** `snake_case` verb-noun, namespaced in docs as `envforge.<tool>` but registered as `list_keys`, `describe_schema`, `exposure_map`, `canary_scan`, `canary_check`. No tool name implies raw retrieval.
- **CLI subcommands/flags:** kebab in help, match existing style; every new command supports `--json`. New: `envforge mcp`, `envforge doctor --ai`.
- **Rust:** modules `snake_case`, types `UpperCamel` (`FenceTargetSpec`, `TargetFile`, `FileKind`, `Ownership`), fns `snake_case`. Files: `ops/fence/registry.rs`, `ops/doctor_ai.rs`, `src/mcp/{mod,server,tools}.rs`.

### Structure

- **Logic in `ops/`** — pure, no I/O policy, no UI. CLI/LSP/MCP/plugins are thin layers calling `ops/`. The MCP tool handlers and fence writers contain **no business logic** beyond delegation + serialization.
- **Tests** live in `tests/` (no in-module tests), named `{feature}_tests.rs`: `fence_registry_tests.rs`, `mcp_server_tests.rs`, `doctor_ai_tests.rs`; LSP parity continues in `tests/lsp_*`. Fn names `test_{what}_{condition}`.
- **One behavior, one source:** any IDE-visible behavior implemented once in `ops/`/`lsp`, never re-implemented in a plugin.

### Format

- **JSON field naming:** `snake_case` (matches existing `--json`/LSP payloads, e.g. `resolved_targets`, `all_fenced`). New status field per target: `{ id, tool, state: "covered"|"fallback"|"unfenced"|"not_installed", files: [...] }`.
- **MCP responses:** plain JSON via `rmcp`; redacted by construction; no envelope reshaping of existing `ops/` JSON — pass through so CLI/LSP/MCP agree byte-for-byte.
- **Fence block markers:** reuse the existing `FENCE_MARKER` begin/end convention for all `Ignore`/`Rules`/`CrossTool` blocks; `DenyRule` uses JSON-key presence (no text marker). Markers are how `strip_block` stays surgical.

### Communication & Process

- **Errors:** `thiserror` types in `src/model/error.rs` (`OpError`) with context (paths); propagate with `?`; no `.unwrap()` in library code. MCP/CLI map `OpError` → their own surface; MCP failures return structured tool errors, never panic.
- **Audit events:** every access-class op emits a `RuntimeEvent` to the monitor bus; **message excludes the secret value** (mirror existing C6 "LSP reveal:" pattern → "MCP <tool>: <key>").
- **Redaction:** any value that could be sensitive flows through `ops::redact::redact_for_label` before crossing CLI/LSP/MCP boundaries — no ad-hoc masking.
- **Atomicity & round-trip:** all file writes via the existing atomic tempfile+rename path; parse→serialize byte-identical; protected zones untouched. Every new writer has a round-trip + content-preservation test.
- **Idempotency:** `fence` re-runs produce no diff; re-plant of an existing canary is a no-op.

### Enforcement

**All agents MUST:** keep logic in `ops/`; add a registry entry (not a code path) for a new tool; never register an MCP tool that returns a raw value; route sensitive output through `redact`; add round-trip + parity tests; keep existing `tests/lsp_*` IDs green; update README/CHANGELOG test counts after test changes (per [[feedback-docs-sync]] discipline).

**Good vs anti-pattern:**
- ✅ New tool → `FenceTargetSpec { id: "augment", files: &[TargetFile{ path: ".augmentignore", kind: Ignore, ownership: Shared }], … }` + tests.
- ❌ New `fn write_augmentignore()` with a new match arm in `create_fence`.
- ✅ MCP `describe_schema` returns key names + types + redacted previews.
- ❌ MCP `get_env` returning `value`.

## Project Structure & Boundaries

### v0.9 changes against the existing tree (additions/edits only — brownfield)

```
env-forge/
├── Cargo.toml                         # + [features] mcp-server; + rmcp dep (optional, gated)
├── src/
│   ├── cli/
│   │   ├── mod.rs                      # EDIT: register `mcp` + `doctor --ai` subcommands
│   │   ├── commands.rs                 # EDIT: fence --status per-tool output; doctor --ai handler
│   │   └── mcp_serve_cmd.rs            # NEW: `envforge mcp` entry (gated on mcp-server)
│   ├── ops/
│   │   ├── fence.rs                    # REFACTOR: consume registry; drop hardcoded match arms
│   │   ├── fence/
│   │   │   ├── registry.rs             # NEW: &'static [FenceTargetSpec] table (D1) — the data
│   │   │   └── writers.rs              # NEW: FileKind-dispatched write_block/strip_block (D2)
│   │   ├── detect.rs                   # NEW (optional): registry-hint detection, surfaced via fence --status (D4)
│   │   ├── redact.rs                   # NEW (or move from lsp::redact): shared redaction (D5)
│   │   └── mcp_scan.rs                 # UNCHANGED: heuristics reused by LSP linter + (opt) MCP
│   ├── mcp/                            # NEW MODULE (gated): the EnvForge MCP server (D5/D6)
│   │   ├── mod.rs
│   │   ├── server.rs                   # rmcp wiring: stdio only (v0.9); remote/OAuth deferred
│   │   └── tools.rs                    # 2 read-safe tools (list_keys, describe_schema) → ops/ (no get_value)
│   ├── lsp/
│   │   ├── mcp_diagnostics.rs          # EDIT: widen documentSelector globs (D7)
│   │   ├── commands.rs                 # EDIT: fence.status/config read registry; resolved_targets
│   │   └── redact.rs                   # re-export ops::redact (keep call sites stable)
│   └── model/error.rs                  # EDIT: any new OpError variants (paths/context)
├── editors/
│   ├── vscode/                         # EDIT: documentSelector + MCP config snippet docs
│   ├── intellij/                       # EDIT: languageMapping patterns for new MCP config files
│   ├── nvim/                           # NEW: Lua plugin (statusline, sign_column heatmap, toggle)
│   │   ├── lua/envforge/{init,status,exposure,fence}.lua
│   │   └── README.md
│   └── zed/                            # NEW: extension manifest (LSP + MCP server reg; no UI)
│       └── extension.toml
├── fuzz/                               # EDIT: add fuzz target asserting no-secret-in-MCP-response (NFR-S1)
├── tests/
│   ├── fence_registry_tests.rs         # NEW: per-target round-trip, content-preserve, idempotent, detection
│   ├── mcp_server_tests.rs             # NEW: tool allowlist (2 tools), redaction, audit emission
│   └── lsp_*                           # UNCHANGED IDs must stay green; + new parity rows
└── docs/
    ├── lsp-clients.md                  # EDIT: add nvim/zed first-party rows; drop Fleet
    ├── ide-behavior-contract.md        # EDIT: L14 glob rows; new client columns
    ├── integration-matrix.md           # NEW: tools × editors × capabilities (FR26)
    └── README.md / CHANGELOG.md        # EDIT: test counts (docs-sync discipline)
```

### Requirements → Structure mapping

| FR group | Lives in |
|---|---|
| FR1–FR6, FR2a/2b (fence registry + writers) | `ops/fence/registry.rs`, `ops/fence/writers.rs`, `ops/fence.rs` |
| FR7–FR9, FR11 (status/coverage) | `ops/fence.rs`, `cli/commands.rs`, `lsp/commands.rs` |
| FR10 (detection, folded into status) | `ops/fence.rs` (+ `ops/detect.rs`), `cli` status path |
| FR12–FR13 (MCP server, stdio, 2 tools) | `src/mcp/`, `cli/mcp_serve_cmd.rs`, `ops/redact.rs`, `ops/audit` |
| FR18–FR19 (linter) | `lsp/mcp_diagnostics.rs`, plugin manifests |
| FR20–FR23, FR23a (editors) | `editors/nvim/`, `editors/zed/`, `editors/{vscode,intellij}` edits |
| FR24–FR26 (governance/docs) | CI config, `registry.rs` `source_url`, `docs/integration-matrix.md` |

### Boundaries

- **`ops/` ↔ surfaces:** all four surfaces (CLI, LSP, MCP, plugins) call `ops/`; none duplicate logic. MCP `tools.rs` and fence `writers.rs` are delegation-only.
- **Process boundary:** `envforge mcp` runs as its own process/transport (NFR-R5) — never in-process with the LSP.
- **Data boundary (the registry):** `registry.rs` is the single source of tool conventions; CLI/LSP/`doctor`/status all read it. No tool knowledge lives anywhere else.
- **Wire boundary:** raw values may cross the LSP wire only on audited human reveal (existing C6); **never** the MCP wire.
- **Feature boundary:** `mcp-server` Cargo feature gates `src/mcp/` + `rmcp` so core fence breadth builds without it.

### Integration points

- **Internal:** registry → fence ops → status JSON (`resolved_targets`) → CLI/LSP/plugins. `ops::redact` is the shared choke point for sensitive output.
- **External:** MCP clients (Claude/Cursor/VS Code/Windsurf) ↔ `envforge mcp` via local stdio; editors ↔ `envforge lsp` via JSON-RPC; both plugins also subprocess `envforge exposure --file`.
- **Data flow:** `.env*`/schema on disk → `ops/` classification → redacted metadata out (status, exposure map, MCP responses); secrets never leave except audited human reveal.

## Architecture Validation Results

### Coherence Validation ✅

- **Decision compatibility:** D1 (registry) feeds D2/D3/D4/D7; D5/D6 (MCP) are independent and gated. No contradictions. All Rust, single binary, one new optional dep (`rmcp`).
- **Pattern consistency:** naming/structure/format rules align with the const-registry + thin-surface decisions and existing CLAUDE.md conventions.
- **Structure alignment:** the file tree realizes every decision (`registry.rs`/`writers.rs` for D1/D2, `src/mcp/` for D5/D6, `editors/{nvim,zed}` for D8) and respects the `ops/`-is-truth boundary.

### Requirements Coverage Validation ✅

- **Functional (29/29 covered):** every FR maps to a component in the Requirements→Structure table. No orphan FR; no FR without a home.
- **Non-Functional:**
  - *Security* — D5 (no raw-value tool, redact-by-construction, audit) + NFR-S1 fuzz target in `fuzz/`. ✅
  - *Performance* — const registry = zero parse; status reuses `resolved_targets`; p95 targets testable in integration tests. ✅
  - *Reliability* — D2 writers carry round-trip/atomic/idempotent tests; MCP process-isolated (D6). ✅
  - *Compatibility* — additive LSP contract (existing `lsp_*` IDs untouched); `rmcp` stdio (NFR-C4; remote deferred); Linux+macOS. ✅
  - *Maintainability* — registry-as-data + build-time validation (D1); parity tests. ✅

### Implementation Readiness Validation ✅

Decisions documented with the one new version-bearing dep (`rmcp`, pinned, gated). Patterns enumerate the real conflict points (registry shape, markers, JSON casing, redaction, audit). Structure is concrete (named files, not placeholders).

### Gap Analysis

- **Critical gaps:** none.
- **Important (resolve during stories):** exact MCP config-path strings for Windsurf/Cline + Claude Desktop Linux are flagged unverified in the PRD appendix — confirm before writing those registry/linter entries + stdio config snippets. (Remote/OAuth deferred to Vision — no longer a v0.9 concern.)
- **Nice-to-have:** Zed native UI (blocked on Zed RFC); Emacs/Sublime plugins (Band-2 stretch).

### Architecture Completeness Checklist

**Requirements Analysis** — [x] context analyzed · [x] scale/complexity assessed · [x] constraints identified · [x] cross-cutting concerns mapped
**Architectural Decisions** — [x] critical decisions documented (+`rmcp` version) · [x] stack specified · [x] integration patterns defined · [x] performance addressed
**Implementation Patterns** — [x] naming · [x] structure · [x] communication · [x] process
**Project Structure** — [x] directory structure defined · [x] boundaries established · [x] integration points mapped · [x] requirements→structure mapping complete

### Architecture Readiness Assessment

**Overall Status: READY FOR IMPLEMENTATION** (16/16 checklist items met; no critical gaps).
**Confidence: High** — brownfield extension of a mature, test-disciplined codebase; the registry refactor + gated MCP server are low-blast-radius; one external dep, official and widely used.

**Key strengths:** registry collapses N tool integrations to data; MCP secret-safety is structural not filtered; additive LSP contract preserves parity; feature flag de-risks MCP spec movement.
**Future enhancement:** Zed native UI, MCP gateway/proxy, Emacs/Sublime plugins, CI enforcement product.

### Implementation Handoff

**AI agents MUST:** keep logic in `ops/`; add tools via registry data + tests, never new control flow; never register a raw-value MCP tool; route sensitive output through `ops::redact`; keep existing `tests/lsp_*` green; sync README/CHANGELOG test counts.
**First implementation priority:** D1 — `ops/fence/registry.rs` (the keystone); everything else depends on it.
