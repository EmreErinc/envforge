---
stepsCompleted: ['step-01-validate-prerequisites', 'step-02-design-epics', 'step-03-create-stories', 'step-04-final-validation']
inputDocuments: ['docs/prd.md', 'docs/prd-validation-report.md', 'docs/ide-behavior-contract.md', 'docs/lsp-clients.md', 'docs/integration-matrix.md']
---

# EnvForge — Epic Breakdown

## Overview

Complete epic and story breakdown for EnvForge Phase-1 framework-config-file support (Java/JVM `application.properties`/`.yml` + profiles, Quarkus/MicroProfile, `.env` cascade), decomposing the PRD requirements into implementable stories. No separate Architecture or UX document — technical architecture is embedded in the PRD's "Developer-Tool Specific Requirements" section; product is CLI/TUI/LSP with no visual UX surface.

## Requirements Inventory

### Functional Requirements

**Config File Recognition**
- FR1: Recognize `application.properties`, `application.yml`, `application.yaml`.
- FR2: Recognize profile variants `application-{profile}.{properties,yml,yaml}`.
- FR3: Recognize Quarkus/MicroProfile `application.properties` and `microprofile-config.properties`.
- FR4: Recognize `.env` cascade (`.env`, `.env.local`, `.env.{environment}`) as a layered set.
- FR5: Classify each file's write-capability (read-write: properties/`.env`; read-only: YAML).

**Configuration Intelligence (Language Features)**
- FR6: Hover shows effective value, resolving layer, schema/sensitivity metadata.
- FR7: Completion of config keys from `.env.schema`, sibling profiles, same file.
- FR8: Completion of `${VAR}` references in values.
- FR9: Go-to-definition from key or `${VAR}` to its definition (schema/base file/`.env`), cross-file.
- FR10: Find references to a key across base + profile files.
- FR11: Document/semantic highlighting distinguishing keys, values, sensitive values.
- FR12: Diagnostics for duplicate key, unterminated `${`, unknown-key-vs-schema.
- FR13: Rename + reformat for write-capable formats; unavailable on read-only YAML.
- FR14: Never modify a read-only-format file via any language feature.

**Resolution & Interpolation**
- FR15: Resolve effective value across profile layers with documented precedence.
- FR16: Resolve `${VAR}` / `${VAR:default}` interpolation.
- FR17: Flatten nested YAML keys to dotted paths for hover/completion/refs.

**AI-Safety Coverage**
- FR18: Apply fence protection to recognized config files.
- FR19: Redact sensitive values in hover/labels/output.
- FR20: Count recognized config files in AI-exposure tracking.
- FR21: Apply canary-token detection to recognized config files.

**Editor Integration**
- FR22: Identical language features across VS Code, IntelliJ, Neovim.
- FR23: Register new document types without regressing existing `.env`/`.env.schema`/shell behavior.

**Extensibility Foundation**
- FR24: Dispatch handlers by config-format via a format-agnostic abstraction.
- FR25: Add a new format by implementing the abstraction without modifying existing handlers.

### NonFunctional Requirements

- NFR1: Hover/completion p95 ≤ 50 ms on ≤500-key file (harness-timed).
- NFR2: Parse files up to 10 MiB without blocking the LSP event loop.
- NFR3: Cached sibling parses; ≤1 file re-parsed per change event (parse-count test).
- NFR4: Redaction parity with `.env` — never expose secrets more readily.
- NFR5: Fence enforcement on recognized config files.
- NFR6: No new LSP command-execution surface (`executeCommandProvider` stays disabled).
- NFR7: Only allow-listed extensions (`.properties`, `.yml`, `.yaml`) read; no glob widening.
- NFR8: No network I/O in config parsing; local-only.
- NFR9: Round-trip byte-equality for write-capable formats; malformed file never silently rewritten.
- NFR10: YAML never modified by any write path in Phase 1; test asserts unreachable.
- NFR11: Malformed files → diagnostics, never crashes/panics; recoverable errors.
- NFR12: Zero regression to existing `.env`/`.env.schema`/shell LSP behavior; suite stays green.
- NFR13: Identical behavior across VS Code/IntelliJ/Neovim; recorded in integration-matrix/lsp-clients.
- NFR14: UTF-16-correct positions for nested YAML keys and multi-byte values.
- NFR15: No regression to Rust 1.75 min / crate baseline; new crates mature.
- NFR16: Parsing/resolution pure in `ops/`/`parser/`; `lsp/`/`cli/` stay thin.
- NFR17: Tests in `tests/{feature}_tests.rs`; insta + tempfile; CHANGELOG/README counts updated.

### Additional Requirements (from PRD Developer-Tool Architecture)

- AR1: Add `is_jvm_config_file(uri)` + `.env` cascade recognition alongside existing `is_env_file`/`is_schema_file` predicates in `src/lsp/server.rs` — no replacement.
- AR2: Introduce format-agnostic entry model (generalize/replace `EnvDocEntry`) carrying key, value, position range, source-layer.
- AR3: `ConfigFormat` trait: `parse(content)->entries`, `resolve(entries,profiles)->effective`, `write_capability()->{ReadWrite|ReadOnly}`.
- AR4: Properties + `.env` cascade reuse existing byte-preserving line parser (`src/parser/`); YAML uses read-only parser (yaml-rust2/saphyr), not wired to any write path.
- AR5: Reuse `schema_line_map` (`HashMap<key,line>`) for cross-file go-to-def.
- AR6: Add `.properties`/`.yml`/`.yaml` to LSP read allowlist in `src/lsp/security.rs`.
- AR7: Register new file types in fence registry / redaction / exposure / canary (`src/ops/`).
- AR8: Extend `load_managed_vars` to surface JVM-config keys where appropriate.
- AR9: Update docs: integration-matrix, lsp-clients, ide-behavior-contract, CHANGELOG/README test counts.

### UX Design Requirements

N/A — no UX Design document; product has no visual UI surface (CLI + TUI + LSP). IDE-facing behavior is governed by `ide-behavior-contract.md`.

### FR Coverage Map

- FR1: Epic 1 (properties/`.env`) + Epic 2 (YAML) — file recognition.
- FR2: Epic 1 (properties profiles) + Epic 2 (YAML profiles).
- FR3: Epic 1 — Quarkus/MicroProfile properties.
- FR4: Epic 1 — `.env` cascade recognition.
- FR5: Epic 1 — write-capability classification (consumed by Epic 2 for YAML read-only).
- FR6–FR12: Epic 1 (properties/`.env`) + Epic 2 extends each to YAML read model.
- FR13: Epic 1 — rename/format on write-capable formats.
- FR14: Epic 2 — never modify read-only YAML via language features.
- FR15: Epic 1 — profile/cascade resolution engine.
- FR16: Epic 1 — `${VAR:default}` interpolation engine.
- FR17: Epic 2 — nested YAML key flattening.
- FR18: Epic 3 — fence protection on new surfaces.
- FR19: Epic 1 — redaction parity (must ship with hover; cannot leak).
- FR20: Epic 3 — exposure tracking.
- FR21: Epic 3 — canary detection.
- FR22: Epic 4 — identical behavior across IDE clients (validation).
- FR23: Epic 4 — no-regression registration (enforced every epic; finally validated here).
- FR24: Epic 1 — `ConfigFormat` dispatch abstraction.
- FR25: Epic 1 — add-format-without-touching-others (proven when Epic 2 adds YAML).

## Epic List

### Epic 1: Properties & `.env` Config Intelligence
A developer gets full IDE language features — hover, completion, go-to-definition, find-references, highlight, diagnostics, rename — on `application.properties`, `application-{profile}.properties`, Quarkus/MicroProfile `.properties`, and the `.env` cascade, with secret values redacted at parity with `.env`. Establishes the `ConfigFormat` seam, format-agnostic entry model, file routing, and the profile/cascade + interpolation resolution engines. Independently shippable: delivers real value to properties-based and dotenv-based stacks even if no later epic ships.
**FRs covered:** FR1, FR2, FR3, FR4, FR5, FR6, FR7, FR8, FR9, FR10, FR11, FR12, FR13, FR15, FR16, FR19, FR24, FR25

### Epic 2: YAML Config Intelligence (Read-Only)
A developer gets the read-only language-feature set — hover, completion, go-to-definition, find-references, highlight, diagnostics — on `application.yml`/`.yaml` and profile variants, with nested keys flattened to dotted paths. YAML is classified read-only; no language feature can write/reformat it. This is the parser-risk-boundary epic; it proves the `ConfigFormat` seam by adding a format without modifying Epic 1 handlers.
**FRs covered:** FR1, FR2, FR6, FR7, FR8, FR9, FR10, FR11, FR12, FR14, FR17

### Epic 3: AI-Safety Parity Across Config Surfaces
A security-conscious developer gets the same AI-agent protection on framework config files as on `.env`: fence enforcement, exposure tracking, and canary-token detection extended to all recognized config file types. (Redaction shipped in Epic 1.) Closes the secret blind-spot for `application-prod.yml` and friends.
**FRs covered:** FR18, FR20, FR21

### Epic 4: Cross-IDE Validation, Docs & Release
A developer gets identical config intelligence across VS Code, IntelliJ, and Neovim, with the supported file-type/client matrix documented and a clean release. Final no-regression validation against the existing `.env`/shell suite, integration-matrix/lsp-clients/ide-behavior-contract updates, and CHANGELOG/README test-count sync.
**FRs covered:** FR22, FR23

## Epic 1: Properties & `.env` Config Intelligence

Full IDE language features on `application.properties` (+ profiles), Quarkus/MicroProfile `.properties`, and the `.env` cascade — with redaction parity and the reusable `ConfigFormat` seam. Stories ordered so each builds only on prior ones.

### Story 1.1: Recognize properties & `.env` config files behind a format abstraction

As a developer,
I want EnvForge to detect `application.properties`, profile variants, Quarkus/MicroProfile files, and the `.env` cascade,
So that the LSP knows which files it can act on without touching existing `.env`/shell behavior.

**Acceptance Criteria:**

**Given** a workspace containing `application.properties`, `application-prod.properties`, `microprofile-config.properties`, `.env`, `.env.local`, `.env.staging`
**When** the LSP opens each file
**Then** each is recognized as a supported config file and routed via a new `is_jvm_config_file` / `.env`-cascade predicate added alongside (not replacing) `is_env_file`/`is_schema_file`
**And** each recognized file is classified read-write (properties, `.env`)
**And** a `ConfigFormat` abstraction (`parse`, `resolve`, `write_capability`) exists and dispatch goes through it rather than per-file-name branching (FR1, FR3, FR4, FR5, FR24, FR25; AR1, AR2, AR3, AR6)
**And** existing `.env`/`.env.schema`/shell recognition is unchanged (NFR12).

### Story 1.2: Parse properties & `.env` into a positioned entry model

As a developer,
I want properties and `.env` files parsed into key/value entries with source positions,
So that language features have a structured, position-accurate model to query.

**Acceptance Criteria:**

**Given** a `.properties`/`.env` file with comments, blank lines, quoted values, and duplicate keys
**When** EnvForge parses it
**Then** it produces a format-agnostic entry list (key, value, UTF-16 position range, source-layer) reusing the existing byte-preserving line parser
**And** re-serializing the parsed model reproduces the file byte-for-byte (NFR9)
**And** files up to 10 MiB parse without blocking the LSP event loop (NFR2)
**And** parsing performs no network I/O (NFR8; AR4).

### Story 1.3: Resolve effective values across profiles and `${VAR}` interpolation

As a developer,
I want a key's effective value computed across profile layers and interpolation,
So that hover/completion can show what actually wins at runtime.

**Acceptance Criteria:**

**Given** `application.properties` and `application-prod.properties` defining the same key, and a value `${DB_URL:localhost}`
**When** EnvForge resolves the key
**Then** profile precedence follows a documented order (base < profile; `.env` < `.env.local` < `.env.{environment}`)
**And** `${VAR}` and `${VAR:default}` are resolved, falling back to the default when unset
**And** resolution is a pure, format-independent engine with unit tests (FR15, FR16; NFR16).

### Story 1.4: Hover with effective value, layer, schema metadata, and redaction

As a developer,
I want hovering a key to show its resolved value, origin layer, and schema/sensitivity info with secrets masked,
So that I understand config without leaking secrets.

**Acceptance Criteria:**

**Given** a key whose value resolves from a profile file and is marked sensitive in `.env.schema`
**When** I hover the key
**Then** the popover shows the effective value, the resolving layer, and schema metadata
**And** a sensitive value is redacted in the hover at parity with `.env` (FR6, FR19; NFR4)
**And** hover returns at p95 ≤ 50 ms on a ≤500-key file (NFR1).

### Story 1.5: Completion for keys and `${VAR}` references

As a developer,
I want completion suggestions for config keys and interpolation references,
So that I stop guessing key names and variables.

**Acceptance Criteria:**

**Given** I am typing a key in `application.properties` with a `.env.schema` and sibling profile files present
**When** completion triggers
**Then** known keys from schema, sibling profiles, and the same file are offered
**And** typing `${` inside a value offers `${VAR}` references from the schema (FR7, FR8).

### Story 1.6: Go-to-definition and find-references across files

As a developer,
I want to jump from a key or `${VAR}` to its definition and find all its uses,
So that I can navigate config spread across files.

**Acceptance Criteria:**

**Given** a key/`${VAR}` defined in `.env.schema` or a base config file
**When** I invoke go-to-definition
**Then** it navigates to the defining location (schema entry, base file, or `.env`) using the existing `schema_line_map`
**And** find-references lists every occurrence across base + profile config files (FR9, FR10; AR5).

### Story 1.7: Highlighting / semantic tokens

As a developer,
I want keys, values, and sensitive values visually distinguished,
So that config files are readable and secrets stand out.

**Acceptance Criteria:**

**Given** an open properties/`.env` file with a sensitive key
**When** the editor requests semantic tokens
**Then** keys, values, and comments are tokenized distinctly
**And** sensitive values carry a modifier marking them readonly/sensitive (FR11).

### Story 1.8: Diagnostics for config-file problems

As a developer,
I want diagnostics for common config mistakes,
So that I catch errors in-editor instead of at runtime.

**Acceptance Criteria:**

**Given** a file with a duplicate key, an unterminated `${`, and a key absent from `.env.schema`
**When** the LSP computes diagnostics
**Then** duplicate-key, unterminated-interpolation, and unknown-key-vs-schema diagnostics are published
**And** a malformed file produces diagnostics, never a crash or panic (FR12; NFR11).

### Story 1.9: Rename and format for write-capable formats (round-trip safe)

As a developer,
I want to rename a key and format properties/`.env` files safely,
So that edits stay consistent without damaging the file.

**Acceptance Criteria:**

**Given** a write-capable (`.properties`/`.env`) file
**When** I rename a key or format the document
**Then** the edit is applied via tempfile + atomic rename and round-trips byte-for-byte except the intended change
**And** rename/format are offered only on write-capable formats (FR13; NFR9).

## Epic 2: YAML Config Intelligence (Read-Only)

Read-only language features on `application.yml`/`.yaml` + profiles. Proves the `ConfigFormat` seam by adding a format without modifying Epic 1 handlers.

### Story 2.1: Recognize YAML config files and classify read-only

As a developer,
I want `application.yml`/`.yaml` and profile variants recognized as read-only config,
So that YAML gets language features without any write risk.

**Acceptance Criteria:**

**Given** `application.yml`, `application.yaml`, `application-prod.yml`
**When** the LSP opens them
**Then** each is recognized and classified read-only via `write_capability()`
**And** `.yml`/`.yaml` are added to the LSP read allowlist in `src/lsp/security.rs` with no glob widening (FR1, FR2, FR5; NFR7; AR6).

### Story 2.2: YAML read parser with nested-key flattening

As a developer,
I want nested YAML keys flattened to dotted paths with accurate positions,
So that `spring.datasource.url` behaves like a flat key for language features.

**Acceptance Criteria:**

**Given** `application.yml` with nested maps and multi-byte values
**When** EnvForge parses it with the read-only YAML parser
**Then** nested keys are flattened to dotted paths (e.g. `spring.datasource.url`)
**And** each entry carries a UTF-16-correct position range (FR17; NFR14; AR4)
**And** the YAML parser is not wired to any write path.

### Story 2.3: Extend read language features to YAML

As a developer,
I want hover, completion, go-to-definition, find-references, and highlighting on YAML config,
So that YAML config has the same read intelligence as properties.

**Acceptance Criteria:**

**Given** an open `application.yml` resolving values across base + profile + `.env`
**When** I hover, complete, go-to-definition, find-references, or request semantic tokens
**Then** each works against the YAML read model with redaction parity
**And** these features were added by implementing the `ConfigFormat` abstraction without modifying Epic 1's properties/`.env` handlers (FR6, FR7, FR8, FR9, FR10, FR11, FR25).

### Story 2.4: YAML diagnostics (read-only)

As a developer,
I want diagnostics on YAML config problems,
So that I catch YAML config errors in-editor.

**Acceptance Criteria:**

**Given** an `application.yml` with a duplicate key and a dangling `${`
**When** diagnostics compute
**Then** duplicate-key and unterminated-interpolation diagnostics are published
**And** malformed YAML degrades to diagnostics, never a crash (FR12; NFR11).

### Story 2.5: Enforce the YAML write-guard

As a developer,
I want guaranteed that no language feature ever writes YAML in Phase 1,
So that I never lose comments or formatting.

**Acceptance Criteria:**

**Given** a recognized read-only YAML file
**When** any rename/format/write-producing feature is attempted
**Then** no write edit is produced for the YAML file
**And** an automated test asserts the YAML write path is unreachable (FR14; NFR10).

## Epic 3: AI-Safety Parity Across Config Surfaces

Fence, exposure tracking, and canary detection extended to framework config files. (Redaction shipped in Epic 1.)

### Story 3.1: Fence enforcement on framework config files

As a security-conscious developer,
I want recognized config files covered by the fence,
So that fenced `application-prod.yml` is guarded from AI-agent reads like `.env`.

**Acceptance Criteria:**

**Given** a fenced workspace containing `application-prod.yml`
**When** a fenced consumer requests the file via the LSP
**Then** the file is not read/served, identically to a fenced `.env`
**And** the new file types are registered in the fence registry in `src/ops/` (FR18; NFR5; AR7).

### Story 3.2: Exposure tracking counts config files

As a security-conscious developer,
I want recognized config files included in AI-exposure tracking,
So that the exposure map reflects all secret-bearing surfaces.

**Acceptance Criteria:**

**Given** a workspace with secrets in `application-prod.yml` and `.env`
**When** EnvForge computes the exposure map
**Then** the framework config files are counted alongside `.env` (FR20).

### Story 3.3: Canary detection on config files

As a security-conscious developer,
I want canary-token detection applied to recognized config files,
So that exfiltration of config secrets can be detected.

**Acceptance Criteria:**

**Given** a canary token placed in `application.yml`
**When** canary detection runs
**Then** the token in the framework config file is detected like one in `.env` (FR21).

## Epic 4: Cross-IDE Validation, Docs & Release

Identical behavior across clients, a no-regression gate, and documentation/release.

### Story 4.1: Cross-client behavior parity

As a developer using multiple IDEs,
I want identical config intelligence across VS Code, IntelliJ, and Neovim,
So that behavior does not depend on my editor.

**Acceptance Criteria:**

**Given** the EnvForge LSP wired to VS Code, IntelliJ, and Neovim
**When** I exercise hover/completion/go-to-def/find-references/highlight on the same config file
**Then** results are identical across all three clients (FR22; NFR13)
**And** validated client/file-type combinations are captured for the integration matrix.

### Story 4.2: No-regression gate against existing suite

As a maintainer,
I want the full existing `.env`/`.env.schema`/shell test suite to stay green,
So that adding config formats never breaks the current install base.

**Acceptance Criteria:**

**Given** the new config-format support merged
**When** the full test suite runs
**Then** all pre-existing `.env`/schema/shell LSP tests pass unchanged (FR23; NFR12)
**And** new file-type routing did not alter existing predicates' results.

### Story 4.3: Documentation and release sync

As a maintainer,
I want docs and counts updated,
So that the feature is discoverable and the docs-sync convention is upheld.

**Acceptance Criteria:**

**Given** the feature is complete
**When** preparing release
**Then** `integration-matrix.md`, `lsp-clients.md`, and `ide-behavior-contract.md` document the new file types/clients
**And** CHANGELOG and README test counts are updated per the docs-sync convention (AR9; NFR17).

## Final Validation

- **FR coverage:** 25/25 FRs covered by ≥1 story (verified). No orphans.
- **NFR coverage:** material NFRs land in ACs — NFR1 (1.4), NFR2/NFR8/NFR9 (1.2), NFR4 (1.4), NFR5 (3.1), NFR7 (2.1), NFR9 (1.9), NFR10 (2.5), NFR11 (1.8/2.4), NFR12 (4.2), NFR13 (4.1), NFR14 (2.2), NFR16 (1.2/1.3), NFR17 (4.3). NFR3/6/15 are cross-cutting gates referenced in PRD.
- **Starter template:** N/A — brownfield extension; no project scaffold story.
- **DB/entities:** N/A — LSP feature, no persistence tables.
- **Within-epic dependencies:** correct ordering — resolution engine (1.3) precedes hover (1.4); YAML parser (2.2) precedes YAML features (2.3); recognition precedes features in every epic. No forward dependencies.
- **Epic independence:** E1 standalone (properties/`.env` value); E2 builds on E1 seam but delivers YAML value independently; E3 uses E1/E2 recognition; E4 is validation/release. E2/E3/E4 do not require each other to function.
- **File-churn check:** Epics 1–3 touch `src/lsp/` + `src/ops/`. Split is justified, not churn — genuine risk boundary (properties-certain vs YAML parser-risk), distinct security domain, and a release gate. Consolidation considered and rejected with rationale (see Epic List).
- **Status:** PASS — ready for development.
