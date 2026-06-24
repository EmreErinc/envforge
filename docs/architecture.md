---
stepsCompleted: [1, 2, 3, 4, 5, 6, 7, 8]
lastStep: 8
status: 'complete'
completedAt: '2026-06-23'
inputDocuments:
  - docs/prd.md
  - docs/epics.md
  - docs/implementation-readiness-report-2026-06-23.md
  - docs/api-reference.md
  - docs/ide-behavior-contract.md
  - docs/lsp-clients.md
workflowType: 'architecture'
project_name: 'EnvForge'
user_name: 'Emre'
date: '2026-06-23'
---

# Architecture Decision Document

_Environment-aware `.env` IDE support driven by `.envforge.project.toml`. Brownfield — extends the existing `envforge lsp` server and first-party editor clients. Builds collaboratively through step-by-step discovery._

## Project Context Analysis

### Requirements Overview

**Functional Requirements:** 24 FRs in 6 capability areas — Project Manifest (FR1-6), Env-File Recognition & Attach Scoping (FR7-10), Key & Value Suggestion (FR11-14,16), Cross-Environment Intelligence (FR15,17,18), AI-Safety & Redaction Parity (FR19-21), Cross-Client Consistency (FR22-24). Architecturally these reduce to: (a) a new **manifest layer** that resolves a concrete recognized file set, (b) a **unified key-set model** (keys × per-environment values) that feeds completion/hover/diagnostics, (c) a **recognition predicate** wired into the existing attach surface, and (d) **redaction/scoping invariants** layered over all of the above.

**Non-Functional Requirements:** 14 NFRs. The shaping ones: latency budgets (NFR1 <100ms completion/hover, NFR2 <200ms manifest re-resolve), 100% redaction parity (NFR4/5/7), byte-round-trip safety (NFR9), zero source-language regressions (NFR10), deterministic cross-client output with no capability branching (NFR11), UTF-16 positions (NFR12), composition with existing `.env.schema`/`envforge/*` (NFR13).

**From Epics/Stories:** 6 epics / 23 stories, linear dependency chain (Manifest → Recognition → Completion → Cross-Env → Safety parity → Cross-client). 4 explicit design questions to resolve here: AR1 manifest schema, AR2 per-env value model, AR3 diagnostic semantics, AR4 manifest↔schema precedence.

**UX:** none — editor-native surfaces (completion/hover/diagnostics/overlays) governed by the existing `docs/ide-behavior-contract.md`; no custom UI.

### Scale & Complexity

- Primary domain: **LSP server (Rust, tower-lsp) + thin editor clients**.
- Complexity level: **Medium** — bounded feature, but multi-client determinism + secret-redaction parity + byte-round-trip demand care.
- Estimated new architectural components: ~4 server-side modules (manifest parser/resolver, env-set/recognition registry, unified key-set index, cross-env diagnostics) + per-client attach config; no new process, no DB.

### Technical Constraints & Dependencies

- Brownfield: must reuse the existing `envforge lsp` server (tower-lsp, stdio), the `RwLock<HashMap>` doc/schema/managed-vars state model, existing `redact::redact_for_label`, and the `envforge/*` custom requests (`exposureMap`, fence). The generic `executeCommand` stays disabled.
- Must honor the 0.8.4 decision: attach strictly to EnvForge-owned files, never source languages — recognition driven by concrete resolved paths/filenames, never blanket `language:`.
- Manifest + env-file parsing must round-trip byte-identically (project Parser principle).
- Min Rust 1.75; TOML parsing already available via existing deps (`toml`/`serde`) — no new framework-config crates (those were removed in 0.8.4; do not reintroduce).

### Cross-Cutting Concerns Identified

- **Redaction parity** — applies to every surface in Epics 3/4, hardened in Epic 5; must hook the single existing redaction path so no variant is a blind spot.
- **Attach scoping** — one recognition predicate feeds all clients; a single source of truth prevents per-client drift.
- **Determinism** — all logic server-side; clients only attach + render (enables NFR11 + cross-client conformance testing).
- **Live reload** — manifest change re-resolves the recognized set + rebuilds the key-set index without restart.

## Starter Template Evaluation

**Not applicable — brownfield extension.** This feature adds modules to the existing `envforge` Rust crate and per-client config to the existing first-party plugins. No new project scaffold, no starter template, no first "initialize project" story.

**Existing stack the work builds on (fixed, no new framework deps):**
- Rust ≥ 1.75; `tower-lsp` 0.20 (stdio LSP), `tokio` async runtime, `dashmap`/`RwLock<HashMap>` for server state.
- Parsing: `serde` + `toml` (already in tree) for the manifest — **no** reintroduction of the framework-config crates removed in 0.8.4 (`yaml-rust2`, `toml_edit`, `jsonc-parser`, etc.).
- Existing modules to extend/reuse: `src/lsp/server.rs` (backend + dispatch), `src/lsp/definition.rs`, `src/parser/` (byte-round-trip discipline), `src/ops/` (schema, redaction `redact::redact_for_label`, fence, exposure).
- Editor clients: `editors/{vscode,intellij,nvim,zed}` attach via their existing mechanisms.

The first implementation story is therefore Epic 1 Story 1.1 (define the manifest schema), not a scaffold step.

## Core Architectural Decisions

### Decision Priority Analysis

**Critical (block implementation):** AR1 manifest schema, AR2 unified key-set model, AR3 diagnostic semantics, AR4 manifest↔schema precedence, recognition/attach mechanism.
**Important:** redaction hook point, reload mechanism, client-attach coverage of variants.
**Deferred (Growth/Vision):** auto-scaffold manifest, secret-provider value hints, multi-root manifests.

The standard data/auth/API/frontend/infra categories are **N/A** — no database, no auth, no network API, no web frontend, no deployment surface. This is an in-process LSP feature. The real decisions are the four readiness questions plus wiring.

### AR1 — `.envforge.project.toml` Manifest Schema

**REVISED AFTER CODE INSPECTION:** the manifest **already exists** — `src/ops/project/config.rs` defines `ProjectConfig` and `.envforge.project.toml` (+ `.yaml`/`.json` via `ConfigFormat`), with `detect_project_config` (walks up to workspace root) and `load_project_config`/`parse_project_config`. **Do not create a new manifest or a parallel schema.** The original `base`/`profiles`/`extra_files` proposal is superseded by the existing schema below.

**Existing schema (authoritative — reuse as-is):**

```toml
[project]
name = "my-service"
schema_path = ".env.schema"
active_environment = "development"

[[environments]]              # one entry per logical environment
name = "development"
env_file = ".env.development"
# description = "..."         # optional

[[environments]]
name = "production"
env_file = ".env.production"

[ai_guard]                    # existing: hardening / scanners (unrelated to recognition)
```

**Rules / new work:**
- The recognized env-file set = each `environments[].env_file` resolved against the project root (the `[[environments]]` list *is* the "base + profiles" the original design imagined — there is no separate `base`/`profiles` table).
- Paths resolve relative to the manifest's directory and must stay **inside** the project root; an `env_file` that is absolute or escapes via `..` is dropped (never recognized). Implemented in **`src/ops/project/resolve.rs`** (`resolve_env_set` + `ResolvedEnvSet::recognizes`), with `lexical_normalize` + `starts_with(root)` containment.
- Duplicate resolved paths collapse (first declaration wins).
- Malformed manifest → diagnostic + last-good fallback is handled at the LSP layer (Story 1.4), reusing existing `ProjectError::ParseError` from `parse_project_config`.
- There is no `schema_version` field in the existing schema; forward-compat versioning is deferred (not invented here).

### AR2 — Unified Key-Set + Per-Environment Value Model

**Decision:** A server-side index built from the resolved file set, keyed by env-var name, deterministically ordered.

```rust
/// One logical environment in the resolved set.
enum EnvId { Base, Profile(String), Extra(PathBuf) }

struct ValueOccurrence {
    env: EnvId,
    file: PathBuf,
    line: usize,         // for goto + provenance
    raw_value: String,   // server-only; never surfaced raw when sensitive
    sensitive: bool,     // schema.sensitive OR is_sensitive_key(key)
}

struct KeyEntry {
    values: BTreeMap<EnvOrd, ValueOccurrence>,  // per-environment occurrences
}

struct EnvKeySet {
    keys: BTreeMap<String, KeyEntry>,           // BTreeMap ⇒ deterministic order (NFR11)
}
```

- Built by parsing each recognized file with the **existing dotenv/`.env` parser** (byte-round-trip safe; reuse `LineNode::EnvExport` extraction) — no new parser.
- Stored in the existing `RwLock<HashMap>` server state alongside docs/schema/managed-vars; rebuilt incrementally on file or manifest change.
- The union of `keys` powers key completion (FR11); per-key `values` powers value completion (FR12), hover (FR15), and missing-key diagnostics (FR17).

### AR3 — Cross-Environment Missing-Key Diagnostic Semantics

**Decision:**
- **Source tag:** `envforge-env` (distinct from existing `envforge` / `envforge-mcp` / `envforge-aiguard` so clients filter independently).
- **Severity:** `Warning` (default; may become configurable later).
- **Trigger:** published on `did_open` / `did_change` of a recognized env file (alongside existing schema diagnostics), recomputed against the current `EnvKeySet`.
- **Predicate:** fire for a key that exists in ≥1 *other* recognized environment and is absent from the current file. Do **not** fire for keys that exist only in `.env.schema` (those remain the schema's unknown/missing-key concern), nor for keys the schema marks optional/not-required.
- **Range:** anchor at the end of the file (last line, col 0) — a stable location that never clobbers real content; UTF-16 correct (NFR12).
- **Message:** `Key 'X' is set in {production, stage} but missing here.` (lists the source environments).

### AR4 — Manifest ↔ `.env.schema` Precedence

**Decision:** Orthogonal layers, no conflict by construction.
- **Manifest is authoritative for the FILE SET** — which files are recognized env files and how they group into environments. The schema cannot widen the recognized/attach surface (a schema referencing a file not in the manifest does not make that file recognized).
- **Schema is authoritative for the KEY CONTRACT** — types, required-ness, allowed values, and sensitivity within recognized files.
- **Sensitivity is the union:** a value is sensitive if `schema.sensitive == true` OR `is_sensitive_key(key)` (matches existing hover/redaction rule).
- With no manifest: conventional `.env*` recognition (FR6); schema still types keys as today. With no schema: manifest still recognizes files and unifies the key-set; keys are untyped.

### Recognition / Attach Mechanism (wiring)

**Decision:** Server-side recognition predicate is the single source of truth; clients attach broadly-but-safely and the server gates.
- New predicate `is_recognized_env_file(uri)` checked **after** the existing `is_env_file` / `is_schema_file` predicates in `server.rs`; returns true for files in the resolved set.
- **Client attach:** profile variants (`.env.development`, `.env.stage`, `.env.production`, …) already match the existing first-party `**/.env.*` / `**/*.env` document selectors — **no client change needed for the common case**. The server's predicate ignores any non-recognized file the broad selector lets through (functionally safe).
- **Non-conventional `extra_files`** (e.g. `config/app.env` that doesn't match `.env*`) are a documented attach limitation for MVP — recognized server-side, but a client may need a project-level selector entry; flagged for Growth.

### Redaction Hook (wiring)

**Decision:** Reuse the single existing redaction path `redact::redact_for_label` at the label layer for completion/hover/inlay. The `EnvKeySet` carries `sensitive` per occurrence; any surface consults it before emitting a label, so redaction parity (FR19/NFR4) holds across every variant by construction. Raw values flow only through `text_edit.new_text`.

**Display-surface posture (refined during implementation):** the existing LSP treats display surfaces as a read-only security boundary that **never emits raw values** — `redact_for_label` returns `***` even for non-sensitive values. Cross-environment surfaces honor this:
- **Hover (FR15)** shows **which environments set a key** (presence) + a `(sensitive)` marker — **not** the per-environment values. (The PRD's "see its value in each environment" is realized as presence, because emitting production values on passive hover would breach the boundary.)
- **Value completion (FR12)** offers a key's non-sensitive values from other environments via `text_edit.new_text` (user-initiated insertion, the sanctioned value channel — consistent with existing schema default/example completion); **sensitive keys get a safe marker only**, never a raw value.
- **Missing-key diagnostics (FR17)** name only the key and the source environment names, never values.

### Reload Mechanism (wiring)

**Decision:** Watch `.envforge.project.toml` and the recognized files via the existing LSP `did_change`/`did_save` + `workspace/didChangeWatchedFiles`. On manifest change → re-resolve set + rebuild `EnvKeySet`; on recognized-file change → update that file's occurrences in the index. Re-resolution is bounded (NFR2 <200ms) and does not block other requests (work behind the existing async dispatch).

### Decision Impact Analysis

**Implementation sequence:** AR1 schema → manifest parser/resolver (Epic 1) → recognition predicate (Epic 2) → `EnvKeySet` index (Epic 3, Story 3.1) → completion (Epic 3) → hover/diagnostics/goto (Epic 4) → redaction/fence parity hardening (Epic 5) → cross-client conformance (Epic 6).
**Cross-component dependencies:** `EnvKeySet` (AR2) is consumed by Epics 3, 4, 5 — it is the spine; build it once, early. Redaction hook depends on `sensitive` flags in `EnvKeySet`. Diagnostic semantics (AR3) depend on the per-env occurrence map.

## Implementation Patterns & Consistency Rules

These prevent divergent choices across agents implementing the epics. They follow the existing `CLAUDE.md` conventions.

### Module Placement (layering)

- **Pure logic in `src/ops/`** — manifest parse/resolve, `EnvKeySet` build, cross-env diagnostic computation, precedence rules. No LSP/UI knowledge, no I/O decisions.
- **Thin dispatch in `src/lsp/`** — `server.rs` calls ops functions and maps to LSP types; the recognition predicate lives next to existing `is_env_file`/`is_schema_file`.
- New modules (proposed): `src/ops/project_manifest.rs` (AR1 parse/resolve), `src/ops/env_keyset.rs` (AR2 index + cross-env queries). Reuse existing `src/parser` dotenv extraction; do **not** add a second env parser.
- **Anti-pattern:** putting resolution/diagnostic logic inside `server.rs` (breaks the ops/lsp split and the determinism guarantee).

### Naming

- Rust standard: `snake_case` functions/modules/fields, `CamelCase` types, `SCREAMING_SNAKE` consts. Module files `snake_case.rs`.
- Public functions read as capabilities: `resolve_manifest`, `build_env_keyset`, `missing_key_diagnostics`, `is_recognized_env_file`.
- Diagnostic **source tag** for cross-env findings: exactly `envforge-env` (new, distinct from `envforge` / `envforge-mcp` / `envforge-aiguard`).

### Error Handling

- Custom errors via `thiserror::Error` in `src/model/error.rs`; include context (file path, line). Manifest errors carry the offending path + span for the diagnostic.
- Propagate with `?`; wrap into `OpError` (existing `From` impls). **No `.unwrap()`** in library code.
- A manifest error must produce a diagnostic + last-good fallback — never a panic, never a silent drop (NFR8).

### Redaction (single path — mandatory)

- **All** label-producing surfaces (completion, hover, inlay) MUST route sensitive values through the existing `redact::redact_for_label`. Never format a raw value into a `label`.
- Raw values may appear only in `text_edit.new_text`. The `sensitive` flag on `ValueOccurrence` is the gate.
- **Anti-pattern:** any new code path that reads `raw_value` into a user-visible string without the redaction check.

### Determinism

- All decisions server-side; clients only attach + render. No branching on client capabilities.
- Use ordered collections (`BTreeMap`) wherever output ordering is observable (completion lists, hover env order, diagnostics) so the same input yields byte-identical output (NFR11).

### Byte-Round-Trip Discipline

- Manifest parsing must not rewrite the manifest. Any future env-file edit (Growth: rename/quick-fix) reuses the existing parser's serialize path — no reflow of untouched lines (NFR9).

### Positions

- All ranges computed in **UTF-16** code units (LSP requirement, NFR12); reuse the existing position-conversion helpers, do not hand-roll byte offsets.

### Scoping (hard rule)

- Recognition attaches only to resolved env files + existing EnvForge-owned files. **Never** add a `language:` selector for a source language or a blanket TOML/YAML/JSON selector (the 0.8.4 regression class). Client attach changes, if any, use filename/pattern selectors only.
- **No new crates** for parsing (no reintroduction of `yaml-rust2`/`toml_edit`/`jsonc-parser`).

### Tests (per `CLAUDE.md`)

- All tests in `tests/` (no in-module tests). File naming `{feature}_tests.rs` — e.g. `project_manifest_tests.rs`, `env_keyset_tests.rs`, `cross_env_diagnostics_tests.rs`.
- Test fn naming `test_{what}_{condition}`. Use `tempfile` for fs, `insta` for snapshots.
- **Mandatory coverage:** manifest resolution (valid/malformed/escape-root), key-set union + per-env values, missing-key diagnostic predicate + range, redaction parity across variants, negative attach (undeclared/source files → no attach), and a cross-client conformance test (one fixture ⇒ identical resolved set + feature output).

### Enforcement

All agents MUST: keep logic in `ops/` and dispatch thin in `lsp/`; route every sensitive value through `redact_for_label`; tag cross-env diagnostics `envforge-env`; never widen attach beyond recognized files; add `tests/{feature}_tests.rs`; run `cargo fmt && cargo clippy -- -D warnings && cargo test` before done; update CHANGELOG + README test counts.

## Project Structure & Boundaries

### Files to add / modify (brownfield)

```
env-forge/
├── src/
│   ├── ops/
│   │   ├── project/
│   │   │   ├── config.rs             # REUSE: ProjectConfig / detect_/load_project_config (AR1 — already exists)
│   │   │   ├── resolve.rs            # NEW ✅ DONE: AR1 resolver — environments → ResolvedEnvSet + recognizes()
│   │   │   └── mod.rs                # MODIFY ✅ DONE: pub mod resolve
│   │   ├── mod.rs                    # MODIFY: declare env_keyset (Epic 3)
│   │   ├── env_keyset.rs             # NEW: AR2 — build EnvKeySet; AR3 missing_key_diagnostics; cross-env queries
│   │   ├── redact.rs                 # REUSE: redact_for_label (no change expected)
│   │   └── fence.rs / exposure...    # REUSE: exposure classification extended to recognized variants (Epic 5)
│   ├── parser/
│   │   └── (dotenv extraction)       # REUSE: parse recognized env files; no new parser
│   ├── lsp/
│   │   ├── server.rs                 # MODIFY: is_recognized_env_file predicate; manifest load/reload;
│   │   │                             #         dispatch completion/hover/diagnostics over EnvKeySet
│   │   ├── definition.rs             # MODIFY: goto across recognized env files + schema (FR18)
│   │   └── mod.rs
│   └── model/
│       └── error.rs                  # MODIFY: ManifestError variants (thiserror)
├── tests/
│   ├── project_manifest_tests.rs     # NEW: resolve valid/malformed/escape-root, reload, no-manifest fallback
│   ├── env_keyset_tests.rs           # NEW: union key-set, per-env values, sensitivity union
│   ├── cross_env_diagnostics_tests.rs# NEW: missing-key predicate, severity, range, source tag
│   ├── env_recognition_tests.rs      # NEW: recognized vs undeclared/source files (negative attach)
│   └── cross_ide_conformance_tests.rs# NEW: one fixture ⇒ identical resolved set + feature output
├── editors/
│   ├── vscode/ intellij/ nvim/ zed/  # MOSTLY UNCHANGED: variants already match .env.* selectors;
│   │                                 # extra_files limitation documented (Epic 6 / Growth)
├── docs/
│   ├── envforge-project-toml.md      # NEW: published manifest schema (AR1 single source of truth)
│   └── ide-behavior-contract.md      # MODIFY: add recognized-variant + cross-env diagnostic behavior
├── Cargo.toml                        # NO new deps (serde + toml already present)
├── CHANGELOG.md / README.md          # MODIFY: feature entry + test counts
```

### Epic → Structure Mapping

- **Epic 1 (Manifest):** `src/ops/project_manifest.rs`, `src/model/error.rs`, `docs/envforge-project-toml.md`, `tests/project_manifest_tests.rs`; reload wiring in `src/lsp/server.rs`.
- **Epic 2 (Recognition/Scoping):** `is_recognized_env_file` in `src/lsp/server.rs`; `tests/env_recognition_tests.rs`.
- **Epic 3 (Key & Value Suggestion):** `src/ops/env_keyset.rs` (EnvKeySet build), completion dispatch in `src/lsp/server.rs`; `tests/env_keyset_tests.rs`.
- **Epic 4 (Cross-Env Intelligence):** `missing_key_diagnostics` in `src/ops/env_keyset.rs`, hover + goto in `src/lsp/server.rs`/`definition.rs`; `tests/cross_env_diagnostics_tests.rs`.
- **Epic 5 (AI-Safety Parity):** redaction routing at dispatch (reuse `redact.rs`), exposure/fence parity in existing `ops/fence`+`exposure` over recognized variants.
- **Epic 6 (Cross-Client):** `editors/*` (doc + any extra_files selector), `tests/cross_ide_conformance_tests.rs`.

### Architectural Boundaries

- **ops ↔ lsp:** ops functions take/return plain data (`ResolvedEnvSet`, `EnvKeySet`, diagnostic structs); `server.rs` converts to/from LSP types. ops never imports `tower_lsp`.
- **server ↔ clients:** clients attach + render only; the server is the sole authority for recognition + feature output (determinism boundary).
- **manifest ↔ schema:** manifest layer (file set) is consumed first; schema layer (key contract) applies within recognized files — per AR4. No circular dependency.

### Data Flow

`.envforge.project.toml` → `resolve_manifest` → `ResolvedEnvSet` (paths + EnvIds) → parse each file (existing dotenv parser) → `build_env_keyset` → `EnvKeySet` in `RwLock` state → consumed by completion / hover / `missing_key_diagnostics` / goto, all redaction-gated → LSP responses → clients render. File/manifest change → targeted index update → re-publish diagnostics.

## Architecture Validation Results

### Coherence Validation ✅

- **Decision compatibility:** all decisions reuse the existing stack (tower-lsp, RwLock state, serde/toml, redact_for_label) — no new framework, no version conflicts, no contradiction. AR1-AR4 are mutually consistent (manifest=files, schema=keys, orthogonal).
- **Pattern consistency:** ops/lsp split, redaction-single-path, deterministic ordering, and scoping rules all reinforce the AR decisions. No pattern contradicts a decision.
- **Structure alignment:** the file map places manifest logic + key-set in `ops/`, dispatch in `lsp/`, tests in `tests/` — matching both the patterns and `CLAUDE.md` layering.

### Requirements Coverage Validation ✅

- **Epic coverage:** all 6 epics have explicit module homes (Epic→Structure mapping). EnvKeySet (the spine) is built early (Epic 3) before its consumers (Epics 4, 5).
- **FR coverage:** FR1-6 (manifest, AR1), FR7-10 (recognition predicate), FR11-14/16 (EnvKeySet + completion, AR2), FR15/17/18 (hover/diagnostic/goto, AR3), FR19-21 (redaction single-path), FR22-24 (server-side determinism + client attach). All 24 supported.
- **NFR coverage:** latency (bounded re-resolve + index reuse), redaction parity (single path + sensitivity union), byte-round-trip (reuse parser serialize), zero source-lang regression (scoping rule), determinism (BTreeMap + server-only logic), UTF-16 (existing helpers), composition (AR4). All 14 addressed.
- **AR resolution:** AR1 (schema), AR2 (EnvKeySet model), AR3 (diagnostic semantics), AR4 (precedence) — all concretely answered.

### Implementation Readiness Validation ✅

- **Decision completeness:** concrete schema, concrete data types, concrete diagnostic semantics, concrete precedence — an agent can implement without re-deciding.
- **Structure completeness:** every new/modified file named with its responsibility; epic→file mapping explicit.
- **Pattern completeness:** module placement, naming, errors, redaction, determinism, scoping, tests — the conflict points an agent would otherwise guess are pinned.

### Gap Analysis Results

- **Critical gaps:** none.
- **Important gaps:** non-conventional `extra_files` (paths not matching `.env*`) need a client-attach story for full coverage — documented limitation, scheduled Growth (Epic 6). Diagnostic severity is fixed to Warning for MVP (configurability deferred).
- **Nice-to-have:** auto-scaffold manifest, secret-provider value hints, multi-root — Vision.

### Validation Issues Addressed

The four readiness-report design questions (AR1-AR4) are resolved in Core Architectural Decisions; the readiness report's "0 of 24 FRs covered" gap is closed by the epics + this architecture.

### Architecture Completeness Checklist

**Requirements Analysis**
- [x] Project context thoroughly analyzed
- [x] Scale and complexity assessed
- [x] Technical constraints identified
- [x] Cross-cutting concerns mapped

**Architectural Decisions**
- [x] Critical decisions documented with versions (stack versions fixed/known; AR1-AR4 specified)
- [x] Technology stack fully specified
- [x] Integration patterns defined
- [x] Performance considerations addressed

**Implementation Patterns**
- [x] Naming conventions established
- [x] Structure patterns defined
- [x] Communication patterns specified (ops↔lsp↔clients boundaries)
- [x] Process patterns documented (errors, reload, redaction)

**Project Structure**
- [x] Complete directory structure defined
- [x] Component boundaries established
- [x] Integration points mapped
- [x] Requirements to structure mapping complete

### Architecture Readiness Assessment

**Overall Status:** READY FOR IMPLEMENTATION (all 16 checklist items `[x]`, no critical gaps).
**Confidence Level:** High — brownfield reuse of a proven server; bounded scope; the only open items are documented Growth/Vision deferrals.

**Key Strengths:** reuses existing battle-tested server + redaction; one data structure (EnvKeySet) is the spine; strict scoping prevents the 0.8.4 regression class; fully deterministic ⇒ testable across clients.

**Areas for Future Enhancement:** client attach for non-`.env*` extra files; configurable diagnostic severity; manifest auto-scaffold; multi-root.

### Implementation Handoff

**AI Agent Guidelines:** follow AR1-AR4 exactly; keep logic in `ops/`, dispatch thin in `lsp/`; route every sensitive value through `redact_for_label`; never widen attach beyond recognized files; add `tests/{feature}_tests.rs`; gate with `cargo fmt && cargo clippy -- -D warnings && cargo test`; update CHANGELOG + README test counts.

**First Implementation Priority:** Epic 1, Story 1.1 — define + publish the `.envforge.project.toml` schema (`docs/envforge-project-toml.md`), then `src/ops/project_manifest.rs`.
