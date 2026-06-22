# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] — Intent 040: Cross-Format Schema Unification

Makes one `.env.schema` the format-agnostic single source of truth for key
metadata (type, sensitivity, required, description) across `.env`,
`application.properties`, YAML, TOML, and JSON. Pure `src/ops/` generalization
of the intent-036 schema-validation + `schema_line_map` linkage — no new file
format, no new crate. The roadmap capstone: it only has value once ≥2 formats
are live, so it ships after 037 (TOML), 038 (YAML writes), and 039 (.NET).

### Added

- **Unified schema model + key normalizer (`src/ops/schema_unification.rs`):**
  `UnifiedSchema` exposes each `.env.schema` entry as a format-agnostic logical
  key; `KeyNormalizer` maps a concrete key (dotted / `:`-path / `UPPER_SNAKE` /
  kebab) to one `LogicalKey`. Two-tier normalization: a strict tier preserves
  distinct identities, and an explicitly-named, Spring-scoped relaxed tier
  collapses `spring.datasource.url` ≡ `SPRING_DATASOURCE_URL` ≡
  `spring-datasource-url` into one logical key (non-Spring keys unchanged).
- **Cross-format diagnostics:** unknown-key, type-mismatch, and missing-required
  computed against the unified schema for every `ConfigFormat`, not per-format
  special-cased. A key defined in any format/layer satisfies the schema (no
  false unknown-key); a `required` key absent from all layers across all formats
  is flagged once. Recoverable errors only — no panic on malformed schema/config.
- **Cross-format go-to-definition & find-references:** a key in any recognized
  format navigates to its `.env.schema` definition AND all concrete definitions
  across formats; find-references lists every occurrence. Reuses (generalizes)
  the existing `schema_line_map` linkage across `src/lsp/config_features.rs`,
  `definition.rs`, `references.rs`, `server.rs`, `document.rs`. Caches reused —
  no per-keystroke sibling re-scan (NFR3).

### Changed

- **Schema validation generalized** (`src/ops/schema.rs`, `schema_json.rs`,
  `config_format.rs`): per-format schema paths resolve through the unified model
  + normalizer rather than format-specific code.

### Tests

- **3,105 tests passing.** Adversarial review caught and fixed: dead-code wiring
  (the unified model is now actually wired into the LSP, proven by
  `tests/schema_unification_wired_tests.rs`), false-sensitivity over-collapse
  (resolved by the two-tier strict/relaxed normalization), false unknown-key
  diagnostics, and NFR3 caching. Coverage in `tests/schema_unification_tests.rs`
  and `tests/schema_unification_wired_tests.rs`.

---

## [Unreleased] — Intent 039: .NET `appsettings.json` (JSONC)

Adds full LSP intelligence over `appsettings.json` and the
`appsettings.{Environment}.json` cascade, round-trip-safe via the
comment-preserving `jsonc-parser` CST — a new `JsoncFormat` plugged into the
intent-036 `ConfigFormat` seam, scoped so it never collides with `mcp.json`,
`package.json`, or `tsconfig.json`.

### Added

- **`JsoncFormat` (`src/parser/jsonc_config_parser.rs`, `src/ops/config_format.rs`):**
  comment- and trailing-comma-preserving JSONC parse into the positioned entry
  model. Nested objects flatten to `:`-joined .NET paths (e.g.
  `Logging:LogLevel:Default`) with UTF-16-correct spans; `write_capability` =
  `ReadWrite`. Introduces a per-format path separator (`.` for properties/YAML,
  `:` for .NET) as a small generalization of the entry/resolution model.
- **Scoped recognition (`src/lsp/server.rs`, `config_file.rs`):** exact-name
  predicate for `appsettings.json` / `appsettings.{Environment}.json`;
  `is_mcp_config_file` is checked first so `mcp.json` and friends are never
  claimed as appsettings (no regression to existing JSON handlers).
- **`__`→`:` env-var binding:** `Logging__LogLevel__Default` (env) and
  `Logging:LogLevel:Default` (JSON) resolve as the same logical key, so
  go-to-definition and find-references link env-var overrides to the JSON keys
  they bind to. .NET environment cascade resolved via `ASPNETCORE_ENVIRONMENT`.
- **Read features + diagnostics + redaction:** hover, completion, go-to-def,
  find-refs, semantic tokens, duplicate-key and unknown-key-vs-schema
  diagnostics; sensitive values redacted at `.env` parity. Lossless rename/format
  via the `jsonc-parser` CST (or shared `SurgicalEdit`) — byte-for-byte except
  the intended change.
- **New dependency:** `jsonc-parser = "0.32"`.

### Tests

- **3,105 tests passing.** Adversarial review caught and fixed a BOM-offset bug
  (a leading UTF-8 BOM skewed every entry's position) and an `mcp.json` collision
  (mcp.json now excluded and checked first). Coverage in
  `tests/dotnet_appsettings_tests.rs`, plus 036/mcp.json non-regression tests.

---

## [Unreleased] — Intent 038: YAML Writes (Surgical Rename)

Upgrades YAML config files from `ReadOnly` to `ReadWrite` for rename operations,
using a new surgical byte-range splice primitive (`SurgicalEdit`) that guarantees
byte-identical output outside the edited key span — no whole-document re-serialization,
no comment loss, no whitespace drift. Inverts the 036 YAML write-path restriction
for rename; format remains a deliberate no-op (rename-only per Open decision 1).

### Added

- **`SurgicalEdit` utility (Unit 001, `src/ops/surgical_edit.rs`):** format-agnostic
  byte-range splice. `apply(source, range, replacement)` produces a string where every
  byte outside `range` is identical to `source` by construction (prefix + suffix
  guaranteed untouched). `to_text_edit(&self, content)` converts the same splice to an
  LSP `TextEdit` with UTF-16-correct positions. `assert_byte_identity` property harness
  for test verification. Zero-panic on invalid ranges — returns `None` rather than
  unwrapping.
- **YAML span resolver (Unit 002, `src/parser/yaml_span_resolver.rs`):** resolves a
  dotted-path key (e.g. `spring.datasource.url`) to the exact byte range of its leaf
  key token using `yamlpath` (tree-sitter-yaml). Returns `YamlKeySpan { byte_range,
  is_quoted }`. Anchor/alias guard: refuses all renames when the document contains
  anchors (`doc.has_anchors()`), returning `Err(ResolveError::AnchorAlias)` — never
  silently mis-edits an alias resolution site. Multi-document YAML gap documented.
- **`config_yaml_rename` (`src/lsp/config_features.rs`):** atomic `WorkspaceEdit`
  across base + profile YAML files using `SurgicalEdit` on the key span. Falls back to
  `e.key_range` when doc content is unavailable. URIs sorted for determinism. Returns
  `None` when `write_capability` is `ReadOnly` (capability gate preserved).
- **`config_yaml_format_text_edits` (`src/lsp/config_features.rs`):** always returns
  `Vec::new()` — format is intentionally a no-op (rename-only, Open decision 1).
- **New dependencies:** `yamlpath = "1.26.1"`, `yamlpatch = "1.26.1"`.

### Changed

- **`YamlFormat::write_capability()` → `ReadWrite`** (`src/lsp/config_file.rs`): YAML
  config files now participate in the rename write path. Format remains no-op.
- **036 YAML write-guard tests inverted** (`tests/yaml_intelligence_tests.rs`,
  `tests/cross_ide_release_tests.rs`): tests that asserted `ReadOnly` + empty rename
  now assert `ReadWrite` + surgical byte-identical rename output.

### Tests

- **3,105 tests passing** (full suite across intents 036–040). Intent-038 added 46
  tests in `tests/yaml_writes_tests.rs` covering: `SurgicalEdit` apply/identity/
  text-edit/constructor (Unit 001), YAML span resolver for keys and values, capability
  flip, rename (simple key, byte-identical, readonly gate, collision, same key, invalid
  key, anchor doc, comments preserved, CRLF, no trailing newline), format no-op, and
  round-trip / write-guard inversion. 036 non-regression tests confirm READ path and
  properties/toml/.env handlers are unchanged.

---

## [Unreleased] — Intent 037: TOML Support

Adds a second full-feature ReadWrite format to the intent-036 `ConfigFormat`
seam: the canonical TOML config files (`Cargo.toml`, `pyproject.toml`,
`config.toml`, `.cargo/config.toml`), round-trip-safe via the format-preserving
`toml_edit` CST — proving the seam carries a new format without touching any
existing properties/`.env`/YAML handler. Recognition is scoped to the canonical
names (no over-broad `*.toml`).

### Added

- **`TomlFormat` (`src/parser/toml_config_parser.rs`, `src/ops/config_format.rs`):**
  `toml_edit` parse into the positioned entry model with dotted table-path keys
  (`[dependencies].serde`), UTF-16-correct spans, and arrays-of-tables /
  inline-table flattening; `write_capability` = `ReadWrite`. `${}` completion is
  offered only where a value references an env var (TOML has no native
  interpolation).
- **Scoped recognition (`src/lsp/server.rs`, `config_file.rs`):**
  `is_toml_config_file` exact-match predicate added alongside the existing
  `is_*_file` predicates — no over-broad `*.toml`, no regression to `.env` /
  schema / shell / properties / YAML.
- **Read features + diagnostics + redaction (`src/lsp/config_features.rs`,
  `security.rs`):** hover, completion, go-to-def, find-refs, semantic tokens;
  duplicate-key, type-mismatch-vs-schema, and unknown-key-vs-schema diagnostics
  (arrays-of-tables `[[bin]]` and distinct dotted paths not flagged); sensitive
  values redacted at `.env` parity.
- **Lossless rename/format:** `toml_edit` lossless mutate (or shared
  `SurgicalEdit`) + tempfile + atomic rename; comments, ordering, and whitespace
  preserved. Rename collisions in the same table are rejected with no partial
  write.
- **New dependency:** `toml_edit = "0.25"`.

### Tests

- **3,105 tests passing.** Adversarial review caught and fixed 9 bugs, including
  3 Critical NFR9 round-trip defects (CRLF stripped on write; trailing newline
  dropped on format + atomic rename; data-corruption rename-target collision),
  text-search position bugs, and ahead-of-time false-positive diagnostics.
  Coverage in `tests/toml_support_tests.rs`.

---

## [Unreleased] — Intent 036: Framework Config Files (Phase 1)

Extends EnvForge's LSP intelligence to Java/JVM framework config files and
formalizes `.env` cascade semantics. Phase 1 delivers full Spring Boot /
Quarkus / MicroProfile `.properties` coverage and read-only YAML language
features (`application.yml`/`.yaml` + `application-{profile}.*`). The same
AI-safety guarantees that protect `.env` — fencing, redaction, exposure
tracking, canary detection, AI-guard diagnostics — now extend to all new
config surfaces automatically (zero new configuration required).

### Added

- **Framework config LSP (Unit 001 — properties + `.env` cascade):**
  `is_jvm_config_file` / `is_env_cascade_file` / `is_config_format_file`
  predicates route `application.properties`, `application-{profile}.properties`,
  `microprofile-config.properties`, and the `.env` cascade (`.env.local`,
  `.env.{env}`) through a new `ConfigFormat` dispatch layer without altering
  existing `is_env_file` / `is_schema_file` results. Full language features
  (hover, completion, go-to-def, find-refs, highlight, diagnostics, rename,
  format) over all `.properties` file types. Profile-layer resolution and
  `${VAR:default}` interpolation implemented as format-independent engines
  (`src/ops/config_resolution.rs`). Parser: `src/ops/properties_parser.rs`.
- **YAML config LSP read-only (Unit 002):** `application.yml`, `application.yaml`,
  and `application-{profile}.yml`/`.yaml` are recognized by `is_yaml_config_file`
  and served with `WriteCapability::ReadOnly`. Hover/completion/go-to-def/
  find-refs/highlight/diagnostics all work; rename and format return `None`/`[]`
  (no write path — comment-preserving YAML serialization deferred). Parser:
  `src/parser/yaml_config_parser.rs` via `yaml-rust2`.
- **AI-safety parity across config surfaces (Unit 003):** fence classification,
  exposure tracking (`compute_config_exposure_map`), value redaction, canary
  scan, and AI-guard diagnostics all apply to the new file types at the same
  fidelity as `.env`. `is_config_canary_target` extended to recognize
  `.properties` and `application.*yaml` files. Zero new user configuration
  required.
- **Cross-IDE validation + no-regression gate (Unit 004):** 22 new tests in
  `tests/cross_ide_release_tests.rs` proving (a) feature functions are
  deterministic and client-independent — same input yields same output on VS
  Code, IntelliJ, Neovim, and any generic LSP client (FR22, NFR13); (b) the
  new routing predicates do not alter results for any pre-existing
  `.env` / `.env.schema` / shell URI (FR23, NFR12).
- **Docs updated:** `docs/integration-matrix.md` (config file × feature matrix,
  AI-safety parity table, YAML read-only boundary), `docs/lsp-clients.md`
  (new document types + per-client setup snippets), `docs/ide-behavior-contract.md`
  (CF1 section: per-feature behavior for properties, YAML read-only, AI-safety
  parity across config surfaces).

### Tests

- **2,815 tests passing** (up from 2,569 pre-intent-036; +246). Intent-036 added ~245 tests
  across `tests/properties_env_intelligence_tests.rs` (118), `tests/yaml_intelligence_tests.rs` (57),
  `tests/ai_safety_parity_config_tests.rs` (45), and `tests/cross_ide_release_tests.rs` (25).
  Includes regression tests for all adversarial-review
  findings: routing fix, FR3 scope-narrowing, M-A AI-guard on cascade, M-B republish_all,
  M-C depth cap, M-D CLI scan-dir, BOM stripping, `KEY = value` off-by-one, col-0
  unterminated-ref, UTF-16 key range, NFR9 round-trip idempotency, goto-def determinism,
  FR9 goto-def across docs, and canary.plant workspace-containment security guard.

---

## [0.8.3] - 2026-06-20

Broad expansion of AI-tool and editor coverage so EnvForge's secret-fencing,
leak-linting, and exfil-detection are present wherever a developer's code and
AI agent run. See `docs/integration-matrix.md`.

### Security

- **Fence no longer clobbers an unparseable `.claude/settings.json`**
  (`src/ops/fence.rs`). A hand-edited settings file with a trailing comma or
  comment (common, but not strict JSON) was silently replaced with `{}` plus
  the deny rules, destroying every other setting. The write path now refuses
  to overwrite an existing-but-unparseable file and reports a per-file failure
  (matching `strip_deny_rule`), so the rest of the toolchain still fences.
- **Lease names are validated against path traversal** (`src/ops/lease.rs`).
  `create`/`revoke`/`renew` fed the name straight into `<name>.toml`, so
  `--name ../../tmp/evil` could write, read, or delete a lease file outside the
  leases directory. Names are now restricted to `[A-Za-z0-9._-]` (no `..`),
  rejected before any filesystem access. (JIT leases were already safe — UUID
  names.)
- **JIT lease redemption now requires a secret ticket** (`src/ops/lease.rs`).
  `jit_redeem` gated only on the lease name, which is emitted in audit metadata
  and returned in the handle — so anyone who learned the name could redeem the
  secret and the single-redeem guarantee was bypassable. `jit_grant` now mints
  a separate random UUID ticket, stored on the lease and required (constant-time
  compared) at redeem.
- **Secret backups are created `0600` at creation time** (`src/ops/mcp_scan.rs`,
  `src/config/backup.rs`). Both used `std::fs::copy` then `chmod`, leaving a
  window where the plaintext-secret backup was world-readable (and `mcp_scan`
  discarded the chmod error entirely, so a failure left it `0644` permanently).
  Backups are now opened `O_CREAT|O_EXCL` with mode `0600` and all IO errors
  propagated.

### Fixed

- **CRUD value quoting now escapes the closing quote** (`src/ops/crud.rs`).
  `add`/`edit`/`rename` built the on-disk line without escaping, so a value
  containing the quote character (e.g. `a"b`) broke out of its quotes —
  re-parsing to a truncated, different value and allowing shell-syntax
  injection into the rc file, violating the byte-for-byte round-trip
  invariant. Quoting is now centralized in a parser-correct `quote_value`
  helper (single-quoted values containing `'` fall back to double quotes,
  which EnvForge's parser can round-trip).
- **`secrets provider` lookup no longer panics on a non-ASCII name**
  (`src/ops/secrets/provider.rs`). The "did you mean" suggestion byte-sliced
  the user-supplied name (`name[..2]`), panicking when the first character was
  multi-byte (e.g. `ñx`). Now compared by character.
- **Parser round-trip is now byte-identical for trailing newlines**
  (`src/parser/parse.rs`, `src/model/shell_file.rs`). The serializer
  unconditionally appended `\n`, so a file without a trailing newline gained
  one on every managed write; `ShellFile` now records the original
  trailing-newline state and both serialize paths reproduce it exactly.

### Tests

- **+15 regression tests** pinning all the bugs above:
  `tests/hardening_regression_tests.rs` (quote-escaping round-trip + fixpoint,
  fence non-clobber + valid-merge, non-ASCII provider lookup, lease-name
  traversal, trailing-newline round-trip), `tests/jit_redeem_ticket_tests.rs`
  (forged ticket rejected / genuine accepted), `tests/backup_perms_tests.rs`
  (backup created `0600`). **2,569 tests passing** (up from 2,554).

### Added

- **Data-driven fence target registry** (`src/ops/fence/registry.rs`) — adding an
  AI tool is now a data entry, not new control flow. Config target-set is a
  registry-keyed map.
- **11 fence targets**: Cursor, GitHub Copilot, Claude Code, Windsurf/Codeium,
  Cline, Aider, Gemini CLI, Amazon Q, the `AGENTS.md` cross-tool standard, and
  `.envforgeignore`. Tools without a native ignore file are covered via
  rules/deny + `AGENTS.md` (reported honestly as `fallback`).
- **Honest per-tool fence status** (`fence --status`): `covered`/`fallback`/
  `unfenced`/`not_installed`, installed-but-unfenced detection, JSON + CI exit
  codes.
- **`project init` AI-tool fence selection**: init can fence AI tools after
  scaffolding, choosing targets interactively (a stdin prompt — Enter=detected /
  `all` / `none` / comma-separated ids) or via flags (`--fence`, `--no-fence`,
  `--fence-targets`, `--non-interactive`). Default set = detected tools.
- **EnvForge MCP server** (`envforge mcp serve`, behind the `mcp-server` Cargo
  feature) — read-safe stdio server exposing `list_keys` + `describe_schema`
  (redacted, audited, **no raw secret values**). Client config: `docs/mcp-server.md`.
- **Wider MCP-config credential linting** (Windsurf, Cline, Claude Code, VS Code
  paths) + an LSP quick-fix replacing hardcoded credentials with `${VAR}`.
- **First-party Neovim plugin** (`editors/nvim`: statusline, exposure heatmap,
  fence toggle) and **Zed extension** (`editors/zed`: LSP + read-safe MCP server).
- **CI gating** (`docs/ci-gating.md`): `fence --status` and `mcp status` exit
  non-zero when coverage is incomplete / a credential is hardcoded.

### Changed

- Shared redaction routine moved to `ops::redact` (LSP re-exports).
- `mcp status` now exits `2` when hardcoded credentials are found (CI gating).

### Fixed

- `ops::dotenv::strip_quotes` panicked on a lone quote char (`value[1..0]`) —
  found by the MCP no-secret property test, now guarded.

### Removed

- JetBrains Fleet support (Fleet was discontinued Dec 2025).

### Tests

- **2,554 tests passing** under the default build (up from 2,507; +47 across
  fence registry/writers/status/detection, MCP server, dotenv, CI-gating,
  per-project fence-target selection, and
  LSP quick-fix). The `mcp-server` feature adds 13 more (MCP handshake + tools +
  256-case no-secret property test).

### Security hardening

Security-hardening pass across all surfaces (TUI, CLI, providers/sync, plugins/LSP),
grounded in a 2026 best-practice audit + code re-validation (`docs/security-audit-findings.md`).

### Security

- **Secret cache now encrypted at rest (H1).** The provider secret cache
  (`~/.config/envforge/secrets-cache/*.cache`) is age-encrypted like credentials/sync
  instead of stored as cleartext. Legacy plaintext caches are treated as a miss and
  removed (graceful migration); caching is best-effort and never fails a resolve.
- **TUI diff preview no longer leaks secrets (H2).** Sensitive values are redacted and
  all diff lines are stripped of control/escape sequences before rendering.
- **TUI restores the terminal on panic (H3).** A panic hook + RAII guard guarantee raw/alt
  screen teardown, so a revealed secret can't be stranded on screen.
- **`get` masks sensitive values by default (H6).** Use `get --reveal` for cleartext;
  applies to text and `--json` output.
- **Workspace/project trust gating (H5).** VS Code declares `untrustedWorkspaces: limited`
  and starts the language server/binary only once trusted; IntelliJ gates the LSP launch on
  project trust.
- **VS Code security commands work again (H4).** Fence/reveal/canary/volatile commands were
  wired to a permanently-disabled `executeCommand` and silently failed; they now use
  constrained, named LSP requests (`envforge/fenceStatus`, `envforge/revealValue`, …). The
  generic `executeCommand` remains disabled.
- **IntelliJ binary resolution no longer falls back to PATH (M4).** Refuses to search PATH;
  requires an absolute path or `ENVFORGE_PATH`.
- **Concurrent age-key generation is now race-safe (M10).** First-run key generation is
  serialized so parallel callers converge on a single key.
- **`sync.enforce_ssh` now enforced on clone (M2).** `sync init --enforce-ssh` rejects
  non-SSH (http/https) remotes at clone time and persists the policy to the sync config;
  previously the setting was a dead control on the clone path.
- **Legacy plaintext-sync bypass closed (M3).** A legacy `require_encryption = false`
  no longer maps to a year-2099 (effectively permanent) plaintext window — it now fails
  safe to mandatory encryption. Use `migration-until <RFC3339 date>` for a bounded window.
- **Argv secret detection now entropy-aware (M1).** The guard that blocks secrets passed
  as command-line arguments (visible in `ps`/`/proc`/history) now also catches short,
  prefix-less, high-entropy values (e.g. a generated 12–16 char password/API key) that the
  length+prefix checks missed. Detection deduped into `ops::sanitize::value_looks_like_secret`.
- **Conjur appliance URL validated (L7).** Rejects non-http(s) schemes, missing host, and
  control characters before the URL reaches `CONJUR_APPLIANCE_URL` / `conjur init -u` (SSRF guard).
- **`secrets config --set` zeroizes the credential after storing (L3).** Parity with `set`.
- **LSP bounds per-document entry count (L10).** `parse_env_document` caps at 50k lines so a
  malformed/hostile document can't exhaust memory.
- **Stale-cache fallback warning no longer interpolates the provider error (L8).**
- **VS Code reveal minimizes value residency (M6).** The revealed value is shown once in an
  ephemeral modal and never logged; clipboard copy is opt-in, explicitly warns that clipboard
  managers/sync may retain it, and the auto-clear window is shortened to 15 s.

### Changed

- `set` no longer prints the secret's length (L1); `set --dry-run` redacts sensitive
  values on both diff sides (L2). Cleartext stays available via explicit `--reveal`.
- **`export --format` redacts sensitive values by default (M7).** Previously the
  multi-format export emitted every value in cleartext to stdout/file. Pass `--reveal`
  for cleartext (consistent with `get`).
- **`backup restore` is confined to the backups directory (L4).** A user-supplied path is
  now canonicalized, rejected if outside `~/.config/envforge/backups`, and read with
  `O_NOFOLLOW` (symlink/traversal hardening).
- Single canonical sensitivity decision shared across TUI mask, CLI redaction, and exports
  (L6/L12).

### Fixed

- **Crash on short rc file (M9).** `set`/`add` panicked (`insertion index > len`) when the
  primary shell file had fewer lines than the protected-header offset.
- **UTF-8 truncation panics (M5/M8).** TUI value truncation and CLI `mask_value` byte-sliced
  UTF-8 and could panic on multibyte values; all now use a shared char-boundary-safe helper.
- **TUI reveal tracked by key, not row index (L9).** Revealing a value then scrolling/
  sorting/regrouping could unmask the wrong secret when a row index was reused.
- **`get` emits a reveal audit event only on actual cleartext disclosure (L11).** A masked
  default read no longer fires a (now `Warn`) reveal event.

### Added — Configurable fence targets

- **Per-target fence configuration.** Each of the five AI-tool fence targets
  (`envforgeignore`, `cursor_ignore`, `cursor_rules`, `copilot`, `claude_code`) can now be
  individually enabled or disabled via `[fence.targets]` in the global config. Absent keys
  default to `true` (fail-safe); an unknown key is rejected at parse time (deny_unknown_fields).
- **`envforge fence config` CLI subcommand.** `--list` shows resolved state with source
  (`default` / `global`); `--enable TARGET` / `--disable TARGET` persist changes; `--json`
  for machine-readable output. All five canonical snake_case target IDs accepted.
- **Aggregate-fenced-state semantic change (behavior change for status consumers).**
  `check_fence_status` / `all_fenced` are now relative to the *enabled* target set only.
  A disabled target's stale file on disk does not make the fence `Partial`. Plugin status
  bars and any tooling that reads `all_fenced` from `envforge fence --status --json` should
  note this change: previously any missing file triggered `false`; now only missing *enabled*
  targets do.
- **LSP `envforge.fence.config` named command.** Returns `[{target, enabled, source}]` for
  the resolved config. `envforge.fence.status` response now includes `resolved_targets`
  alongside the existing `files`, `all_fenced`, and `completeness` fields. Both commands
  follow the stable `{ok, result|error}` contract; `envforge.fence.config` requires
  `workspace_root` and returns an error if absent.
- **TUI read-only target summary.** When the fence is on, the footer now shows a compact
  summary next to `[fence:on]`, e.g. `fence: cursor_ignore,copilot (2/5)`, derived from
  the resolved config. No new popup or key handler; the `F` key is unchanged.
- **2,507 tests passing** (up from 2,501; +6 new tests across fence_config_tests.rs and
  lsp_phase1_tests.rs).

## [0.8.2] - 2026-06-16

### Fixed

- **VS Code — Critical: Duplicate command registrations overwriting LSP-backed handlers.**  
  `registerSecurityCommands` ran after `registerCommands`, causing crude CLI-only versions of
  `runVolatile`, `revealValue`, `canaryScan`, and `canaryCheck` to silently overwrite the
  proper LSP-backed implementations. All four duplicates removed from `security.ts`.
- **VS Code — Conflicting "extend lease" commands.** `extendLease` (crude, no lease name
  resolution) conflicted with `volatileExtend` (LSP-backed, updates status bar). Removed the
  crude version; `package.json` command palette now correctly targets `envforge.volatileExtend`.
- **VS Code — Output channel leak.** `showOutput()` created a fresh `OutputChannel` on every
  call, flooding the Output dropdown with duplicate channels after repeated use. Now uses the
  shared extension output channel.
- **IntelliJ — `MonitorStreamAction` broken terminal API.** Replaced deprecated
  `JBTerminalWidget.getTerminalWidgets()` with the `TerminalView` API so `envforge monitor stream`
  runs as a live persistent stream rather than a captured subprocess.
- **IntelliJ — Status bar only painted once.** `EnvForgeStatusBarFactory` now polls every 30 s
  (vars + fence state) with an accelerated 10 s tick when a volatile lease is active, matching
  the VS Code status bar cadence.

### Added

- **IntelliJ — Profile context menu** with Switch, Open Profile File (opens `.env.<name>` in
  the IDE editor), Diff vs Active, and Delete actions — parity with VS Code
  `profileOpenFile` / `profileContextSwitch` / `profileContextDiff`.
- **IntelliJ — Restart Language Server action** (`Tools > EnvForge > Restart Language Server`).
  Parity with VS Code's `envforge.restartLsp` command. Uses lsp4ij `LanguageServiceManager` to
  stop and restart the EnvForge LSP server without restarting the IDE.

## [0.8.1] - 2026-06-15

### Added

- **IDE Governance Dashboard:** Added new **Lifecycle** and **Analytics** categories to the Security tree view in both VS Code and IntelliJ plugins.
- **Integrated Monitoring:** "Monitor Stream" action in IDEs now launches a real-time secret access event stream in a persistent terminal.
- **Full Profiles Parity:** Added a dedicated **Profiles** tab to the IntelliJ tool window, matching the VS Code experience for one-click environment switching.
- **CI Performance Gate:** Added benchmark execution and automated regression tracking to GitHub Actions.

### Fixed

- **IDE Plugin Alignment:**
  - Consolidated VS Code `Fence` toggle logic to use the LSP and refresh all UI components (Status Bar, Decorations, and Security View).
  - Refactored IntelliJ `Toggle Guard` to support tool-specific hook management (Claude Code, Cursor, etc.).
  - Unified command naming and behavior across both plugins to match the IDE Behavior Contract.
- **CI Pipeline Hardening:**
  - Fixed `Resource not accessible` error in security audit by adding `checks: write` permissions.
  - Added `audit.toml` to explicitly ignore unmaintained dependency warnings for `proc-macro-error2`.
  - Resolved `cargo-deny` license failure by adding `Elastic-2.0` to the allowed list.
- **Documentation Sync:** Synchronized `cli-reference.md`, `api-reference.md`, and `ide-behavior-contract.md` with the latest v0.8.1 subcommands and features.
- **Encryption test environment contamination:** Added `serial_test` to isolate tests that manipulate global environment variables.
- **LSP security:** Implemented JSONL audit logging for the Language Server to track secret access attempts (hover/completion).

## [0.8.0] - 2026-06-03

### Security — Pre-Launch Hardening

A systematic security audit and hardening pass across credential encryption, sync transport,
volatile mode, and fence completeness. All security-sensitive booleans migrated to exhaustive
sum types to make the safe path the only path that compiles.

#### Credential Encryption Policy (Launch-Blocker Fix)

- **New `CredentialEncryptionPolicy` enum:** `Mandatory | NotSupported { reason, reviewed_by, re_evaluate_after_secs }`. The old permissive `Reporting` variant is **deleted** — no compile-time `Default`, no silent plaintext pass-through. Every provider MUST explicitly declare its encryption posture via a required trait method; forgetting to implement `encryption_mode()` is a compile error.
- All 13 secret providers (`vault`, `aws-ssm`, `azure`, `gcp`, `onepassword`, `doppler`, `infisical`, `bitwarden`, `conjur`, `keeper`, `sops`, `pass`, `akeyless`) implement `encryption_mode() -> CredentialEncryptionPolicy::Mandatory`.
- `NotSupported` escape hatch requires: technical justification (≥16 chars), security reviewer name, and a `re_evaluate_after_secs` auto-expiry timer to prevent permanent bypass.
- `provider_audit()` now surfaces credential encryption posture per provider.

#### Key Management: CI / Headless Support

- **`ENVFORGE_AGE_KEY`** env var — raw age identity key content for CI pipelines and headless environments. Takes highest precedence over all other key sources.
- **`ENVFORGE_AGE_KEY_FILE`** env var — path to an alternative age key file. Takes precedence over the default `~/.config/envforge/age.key`.
- **Recovery key generation** — a second age keypair (`age-recovery.key`) is generated alongside the primary key on first run. Users are warned to store it offline. Losing the primary key without the recovery key means permanent credential data loss.

#### Boolean-to-Sum-Type Migrations (Security by Construction)

- **Sync `require_encryption: bool` → `SyncEncryptionPolicy { Mandatory, MigrationUntil(String) }`.** The `MigrationUntil` variant accepts an ISO-8601 datetime after which Mandatory enforcement auto-activates, preventing the "permanent bypass" bug where a migration flag was never re-enabled. Old `true`/`false` configs accepted via serde alias for backward compatibility.
- **Volatile `enabled: bool` → `VolatileMode { Off, On { ttl_seconds }, Strict { ttl_seconds, reauth } }`.** Default changed from `Off` to `On { ttl_seconds: 300 }` — secure by default. `Strict` variant requires re-authentication after volatile expiry.
- **Fence `all_fenced: bool` → `FenceCompleteness { Complete, Partial(Vec<FenceFileStatus>) }`.** `Partial` carries the list of unfenced files so callers can surface actionable diagnostics. The `all_fenced` field is retained for backward compatibility.

#### Security Invariant Tests

- **`tests/encryption_invariant_tests.rs`** — 19 compile-time and runtime invariant tests proving the encryption posture holds forever:
  - `VolatileMode::default()` is `On` (not `Off`) — regression anchor
  - All 13 providers return `CredentialEncryptionPolicy::Mandatory`
  - `SyncEncryptionPolicy` serde migration: old `true`/`false` deserialize correctly
  - `MigrationUntil` enforcement: past dates require encryption, future dates don't, invalid dates fail-safe
  - `ENVFORGE_AGE_KEY` resolution: empty rejected, invalid fails at encrypt-time
  - `CredentialEncryptionPolicy::NotSupported` requires ≥16-char justification

#### Migration Deadline UX

- **`SyncEncryptionPolicy::is_required_with_override()`** — accepts an explicit `--force-migration` flag that allows operators to bypass the deadline during migration windows. Emits a WARN-level log with a reminder to re-enable Mandatory.
- All `read_snapshot()` and `decrypt_snapshot()` callers pass `force_migration: false` by default. The `force_migration` flag is available at the CLI layer for operator override.

#### ENVFORGE_UNSAFE_ARGV Hardening

- The old `ENVFORGE_UNSAFE_ARGV=1` global bypass is **rejected**. Must now use `ENVFORGE_UNSAFE_ARGV=*` (all providers) or `ENVFORGE_UNSAFE_ARGV=vault,aws-ssm` (per-provider allowlist). Only available in debug builds (`#[cfg(debug_assertions)]`).
- **New `is_unsafe_argv_allowed(provider)`** function for per-provider argv bypass gating. Invalid old `=1` format is rejected with a migration hint.
- All `ENVFORGE_UNSAFE_ARGV` usage emits `Critical` severity audit events (was `Warn`). Provider name included in audit payload.

#### Operational Hardening

- **Volatile TTL UX:** New `volatile_remaining()` returns remaining duration before expiry. Expiry error message now includes the TTL value: `volatile session expired (TTL: 300s). Re-authenticate to continue.`
- **CI key audit logging:** `KeyProvisioning` audit event emitted when `ENVFORGE_AGE_KEY` is used — enables audit tooling to distinguish ephemeral CI keys from persistent file-based keys.
- **Recovery key first-run UX:** Visible `eprintln!` banner on first key generation — "STORE THIS FILE OFFLINE" with permanent data loss warning. No longer a silent `log::warn!`.
- **Key rotation placeholder:** `rotation_policy() -> Option<RotationPolicy>` added to `SecretProvider` trait. `RotationPolicy { interval_days, automatable, instructions }` provides the structural foundation for future automated credential rotation.

#### Risk Minimization — Acceptable Gaps Closed

Remaining pre-launch gaps from the threat model addressed:

- **LSP redaction utility:** `redact_secrets_in_message()` in `src/lsp/redact.rs` — centralized string-level secret redaction available to all LSP message handlers. Handles arbitrary secret patterns, sorts by length descending to prevent partial-match escapes, skips sub-8-char strings to avoid false positives.
- **Audit log integrity:** HMAC-style hash chain was already implemented in `src/ops/audit/tamper.rs` (616 lines, `verify_integrity()`, `ChainState`). Residual risk marked resolved — gap was documentation, not implementation.
- **Fence multi-tool propagation:** `KNOWN_TOOLS` registry (6 AI tools: Cursor, Claude, Copilot, Aider, Windsurf, Continue) + `apply_tool()` function. Uses symlinks on Unix (auto-updating) and file copies on Windows. New `envforge fence apply --tool <name>` subcommand.
- **GPG signature verification:** `gpg_fingerprint()` and `signature_url()` trait methods on `SecretProvider` + `verify_gpg_signature()` helper. GPG verification at provider registration time; SHA-256 hash pinning at load time. Zero new dependencies — uses system `gpg` binary.
- **Provider binary verification:** `verify_gpg_signature()` validates GOODSIG status and fingerprint match. Graceful fallback when `gpg` not installed.

## [0.7.8] - 2026-05-21

### Added — Carapace + Inshellisense Completions

- `envforge completions carapace` generates YAML completion specs for the carapace multi-shell completion engine
- `envforge completions inshellisense` generates Fig-format JS specs for Microsoft's IDE-style terminal autocomplete
- `--install` flag auto-installs to correct paths: `~/.config/carapace/specs/` and `~/.fig/autocomplete/build/`

## [0.7.7] - 2026-05-20

### Added — IDE-First Experience

A coordinated push to make the editor the primary EnvForge surface. Single language-server backend, two thin plugins (VS Code, IntelliJ), one shared behavior contract. Visual moat: AI-exposure heatmap in the gutter, fence shield in the status bar, volatile-lease countdown, canary tripwire glyphs.

#### Language Server (`envforge lsp`)

`textDocument/*` capabilities now advertised in `initialize`:

- `textDocument/completion` — schema-aware; sensitive values redacted in `label`, raw value flows only through `text_edit.new_text`.
- `textDocument/publishDiagnostics` — schema validation, unknown-key warnings (`envforge`), MCP Supply-Chain Integrity findings on `.cursor/mcp.json` / `.claude/settings.json` (`envforge-mcp`), save-time AI-guard prompt-injection scan (`envforge-aiguard`).
- `textDocument/hover` — schema info + provenance (source file, current value redacted if sensitive, defined-by).
- `textDocument/definition` — `.env` key → schema. Source files (TS / JS / Python / Rust / Go / Java / Kotlin / Ruby / PHP / C# / Shell) → schema via UPPER_SNAKE identifier extraction.
- `textDocument/references` — schema declaration + every open `.env*` entry.
- `textDocument/rename` — atomic `WorkspaceEdit` across schema + open `.env*` documents.
- `textDocument/codeAction` — `Add to schema`, `Use secret reference`, `Mark as secret`, `Use default`, `Plant canary tripwire`, `Add all missing keys`, `Generate .env from schema`.
- `textDocument/codeLens` — actionable `Plant canary` and `Activate fence` lenses on sensitive lines.
- `textDocument/inlayHint` — `(default)`, `→ <redacted>`, `(<type>)` for unset keys.
- `textDocument/formatting` — canonical `.env` whitespace normalization, blank-line collapse, trailing newline.
- `textDocument/semanticTokens/full` — `variable` / `string` / `comment` with `readonly` modifier on sensitive keys.

`workspace/executeCommand` provider (15 commands):

- `envforge.fence.enable`, `envforge.fence.disable`, `envforge.fence.toggle`, `envforge.fence.status`
- `envforge.canary.plant`, `envforge.canary.list`, `envforge.canary.scan`, `envforge.canary.check`
- `envforge.volatile.status`, `envforge.volatile.extend`
- `envforge.sync.push`, `envforge.sync.pull`, `envforge.sync.status`
- `envforge.run.volatile`, `envforge.reveal.value`

Custom request:

- `envforge/exposureMap` — per-line red / amber / green AI-exposure classification with `canary: bool` flag; backs the gutter heatmap and file-explorer badges.

#### New CLI Subcommands

- `envforge exposure --file <PATH>` — emit AI-exposure classification JSON (same wire format as the LSP custom request).
- `envforge fence --disable` — symmetric counterpart to `envforge fence`; surgically strips envforge-owned content while preserving user content.
- `envforge lease renew <NAME> --ttl <TTL>` — extend an existing lease without recreating it.

#### IDE Plugin Parity

VSCode `0.1.6`, IntelliJ `0.1.6`:

- Status bar trio: `<N> vars` · fence shield (`AI BLOCKED` / `AI ALLOWED`) · volatile-lease countdown with sub-minute precision and color escalation (amber ≤5 min, red ≤1 min). Click fence → toggle. Click countdown → extend.
- AI-exposure gutter heatmap: colored dot per env-var line; lines with a registered canary render a shield glyph instead. Hover tooltips quote the classification reason.
- File-explorer / project-view badges on `.env*` files: 🛡 (fenced) / ! (red) / ? (amber) / ✓ (all-green).
- Source-language goto-definition: Ctrl-click `process.env.X`, `os.environ["X"]`, `std::env::var("X")`, etc. to land on the schema entry.
- MCP Supply-Chain Integrity diagnostics on `mcp.json` / `.cursor/mcp.json` / `.claude/settings.json` — credential patterns flagged inline.
- New command-palette / tools-menu entries: Run Volatile Session, Reveal Value (audit-logged), Plant Canary, Canary Scan, Check Triggered Canaries, Extend Volatile Lease, Enable / Toggle Fence.

#### Security Posture

- Canary fake values are minted server-side; the plant action arguments carry only `{key, pattern, file}` — the payload never flows through plugin process memory.
- Reveal-value action emits a `RuntimeEvent` to the monitor bus (audit trail). The value crosses the LSP wire only on explicit user confirm; clipboard auto-clears 30 s after copy if the contents still match.
- LSP server canonicalizes every URI it touches and confines source-file reads to the workspace root with a 1 MiB size cap and extension allow-list.

#### Behavior Contract

- New `docs/ide-behavior-contract.md` documents every IDE feature row-by-row (trigger, LSP method, wording, keybind, test IDs) so VS Code and IntelliJ stay in lockstep. Drift is now a contract bug, not a maintenance footnote.
- 162 LSP-layer integration tests in `tests/lsp_phase1_tests.rs` fence every behavior listed in the contract.

### Changed

- Test count: **2073 → 2214** (+141 across workspace).
- README headline subtitle now mentions the LSP + IDE plugins.
- `docs/api-reference.md` LSP section rewritten to reflect the full capability surface and custom request schema.
- `docs/cli-reference.md` gains `envforge exposure`, `envforge fence --disable`, `envforge lease renew` entries; `envforge man` picks them up automatically through the embedded include.

## [0.7.6] - 2026-05-13

### Added — Project Wizard Redesign

`envforge project wizard` rewritten as the single canonical onboarding entry point. Self-bootstrapping (no `init` prerequisite), multi-environment, resumable, AI-safety aware.

#### New flags

- `envforge project wizard` — guided 5-step setup: identity → environments → schema → values → hardening.
- `envforge project wizard --force` — re-run all steps; preserves project config.
- `envforge project wizard --reset` — wipe `completed_steps` then run (deeper than `--force`).
- `envforge project wizard --non-interactive` — defaults-only path for CI / scripts.
- `envforge project wizard --from <env-file>` — preseed Step 4 values from existing dotenv file.
- `envforge project wizard --dry-run` — walk steps and print planned actions; no filesystem writes.
- `envforge project init --name X --active Y --schema PATH --env-file PATH` — extended CLI scaffold flags for non-interactive use.

#### New wizard behavior

- Schema step branches three ways: `[R]euse` existing, `[E]dit` per-key (replaces a single `[KEY]` block), `[G]enerate fresh` from active env file.
- Values step accepts per-key keystrokes: Enter / `<value>` / `s` (skip) / `c` (clear) / `d` (use schema default) / `q` (quit env) / `a` (abort cascade across envs).
- Sensitive schema keys (`sensitive = true`) prompt via `rpassword` — no terminal echo.
- Hardening step toggles: `.gitignore` patterns, `.env.ai.md` AI-safe context emit, `.aiignore`/`.cursorignore` fence install, canary token mint + append to active env file.
- Resume after partial run: `completed_steps` persisted in project config; subsequent runs skip done steps. Already-complete projects print guidance instead of re-prompting.
- JSON output (`--json`) emits structured `WizardReport` with all step outcomes including hardening flags.

#### Deprecation

- Top-level `envforge init` emits stderr warning: `'envforge init' is deprecated. Use 'envforge project wizard'. Removed in v0.8.0.` Functionality unchanged for one release.

#### IDE plugin parity

- VSCode `0.1.5` — command palette: "EnvForge: Run Project Wizard", "EnvForge: Initialize Project (non-interactive)".
- IntelliJ `0.1.5` — Tools → EnvForge → Run Project Wizard / Initialize Project.

#### Dependencies

- New: `rpassword = "7"` for masked sensitive input.

#### Tests

24 wizard tests pass (18 → 24). New coverage: non-interactive cold start, idempotency, force-resume, preset precedence, multi-env loop, sensitive-key masking path, branch reuse / infer / blank, edit-existing-key block replacement.

### Added — MCP Supply-Chain Integrity

First env-management tool to combine pin + reputation + tool-poisoning detection for Model Context Protocol (MCP) servers consumed by Claude Code and Cursor. Closes 10 documented attack classes against AI tooling. **200 new tests** (1873 → 2073 total).

#### New CLI commands

- `envforge mcp pin [--strict] [--inspect] [--lockfile PATH]` — pin all configured MCP servers to a lockfile at `.envforge/mcp.lock`. Captures package-manager integrity (npm `sha512-...`, pip `sha256:...`), binary SHA-256 (realpath-canonicalized, with symlink target recording), canonical-JSON hash of each server's config section, TLS SPKI hash for remote SSE/HTTP servers, and (with `--inspect`) the MCP `initialize` handshake response hash. `--strict` requires `KNOWN_GOOD` reputation tier.
- `envforge mcp verify [--json] [--strict] [--lockfile PATH]` — re-resolve and compare against lockfile. Exit 0 on clean match, 1 on mismatch, 2 on input error.
- `envforge mcp diff [--server NAME] [--lockfile PATH]` — human-readable per-server diff with reputation-tier annotations.
- `envforge mcp trust NAME --reason TEXT` / `envforge mcp untrust NAME` — manage `USER_TRUSTED` reputation overrides for community MCP servers not yet in the curated feed. Reason text is required (audit-bound).
- `envforge mcp explain --lock [--format text|markdown] [--lockfile PATH]` — render annotated lockfile suitable for PR review. Markdown format produces a GitHub-friendly table.
- `envforge mcp launch <ide> [args...]` — atomic verify-then-exec for Claude Code or Cursor. Uses `execvp` on Unix (process replacement, no TOCTOU gap) and spawn+wait on Windows. Refuses to exec if any pinned server's reputation has flipped to `KNOWN_BAD`.
- `envforge mcp pin --refresh --accept` / `--refresh --yes` — refresh existing lockfile after diff review (`--accept`) or CI-bypass mode (`--yes`, audit-logged).
- `envforge mcp pin --resolve-conflicts ours|theirs` — resolve git merge markers in `.envforge/mcp.lock` from one side.

#### Doctor extensions

- `envforge doctor --all` — include `UNKNOWN`-tier MCP servers in the report (default shows only `KNOWN_BAD`).
- `envforge doctor --fail-on mcp` — exit 2 if any pinned MCP server is `KNOWN_BAD`; CI-gating friendly.

#### Reputation feed

- Bundled gzip-compressed reputation feed shipped inside the binary. Five tier classifications: `KNOWN_GOOD`, `UNKNOWN`, `KNOWN_BAD`, `USER_TRUSTED`, `VOLATILE`. Lookup precedence is locked: `KNOWN_BAD` always wins (security floor — user trust cannot override a known-malicious package). `expires_at` field surfaces stale-feed warnings.
- User-trust overrides persist at `~/.config/envforge/mcp-trust.json` (or platform-equivalent via `dirs::config_dir`), atomic write, `0600` perms on Unix.

#### Tool-poisoning detection

- Pattern-based scanner for prompt-injection content embedded in MCP tool descriptions, input schemas (e.g. overly-permissive `env` / `shell` / `eval` parameters), and concatenated cross-tool description blobs. 7-step canonicalization pipeline catches common evasion techniques: NFKC normalization, leetspeak fold (`1gn0re` → `ignore`), zero-width character stripping, whitespace collapse, line-separator normalization (U+2028 / U+2029 / CRLF), bidi-control detection (U+202A-E, U+2066-9), unicode-tag-smuggling detection (U+E0000-E007F).
- 17 detection patterns covering critical exfil/role-injection/tool-call-smuggling vectors.
- All findings carry SHA-256 of the matched payload only — never the raw matched text — so the audit log cannot be re-read by an LLM to re-trigger the same injection.

#### TUI

- New `!M` marker in the env table header when any `KNOWN_BAD`-pinned MCP server is present in the lockfile. Inline only; no new popup.

#### JSON output

- `envforge mcp scan --json` output extended with a top-level `mcp_pin_status` field: `lockfile_exists`, `pinned_count`, `known_bad_count`, `unknown_count`, `feed_version`, `feed_stale`, `known_bad_servers[]`. Field is additive; existing consumers using only `findings`, `files_scanned`, `credentials_found` remain backward-compatible.

#### Hooks

- `envforge ai-hook install --tool claude-code` now installs a third hook stage (`SessionStart`) that invokes `envforge mcp verify --json` automatically when a Claude Code session opens. Existing `PreToolUse` + `PostToolUse` stages unchanged.
- `envforge ai-hook install --tool cursor` extends the `.cursor/rules` template with an MCP-pin advisory block. Cursor lacks a pre-load hook surface; hard enforcement is wrapper-only.

#### Periodic re-verify

- `ENVFORGE_MCP_REVERIFY_TTL` environment variable (in seconds) controls the cadence at which `ops/monitor` re-checks the lockfile against the resolution + reputation state. Default 7 days. Emits new audit events on tier flips.

#### New audit event types

`McpPinned`, `McpVerifyFailed`, `McpReverifyOk`, `McpReverifyFailed`, `McpPoisonDetected`, `McpFeedFlippedKnownBad`, `McpUserTrustGranted`, `McpUserTrustRevoked`, `McpLaunchBlocked`, `McpFeedStale`. All events carry SHA-256 hashes + identifiers only — no raw payload text.

### Threat-Model Coverage

Ten attack classes addressed:

1. Supply-chain swap — package-manager integrity or binary hash changed between releases
2. Tool description poisoning — prompt-injection inside MCP `tools/list` response
3. Tool schema poisoning — overly-permissive `env` / `shell` / `eval` input parameters
4. Typosquat / namespace-squat — reputation tier flags near-match package names
5. Config drift — canonical-JSON hash of the per-server MCP config section
6. TOCTOU between verify and IDE launch — atomic `execvp` wrapper closes the race
7. Self-updating server — `volatile` flag falls back to package-manager integrity as the anchor
8. Remote (SSE/HTTP) server compromise — SPKI (Subject Public Key Info) pinning survives Let's Encrypt cert rotation while detecting key swaps
9. Mid-session feed flip — periodic re-verify emits a dedicated event on transitions
10. Cross-tool injection — concatenated blob scan catches payloads split across adjacent tool descriptions

### Dependencies

- New: `rustls 0.23`, `webpki-roots 0.26`, `x509-cert 0.2` (TLS handshake + SPKI extraction for remote MCP servers)
- New: `flate2 1` (gzip decompression of the bundled reputation feed)
- New: `unicode-normalization 0.1` (NFKC for tool-description canonicalization)

All pure-Rust; ~3 MB combined binary-size increase. Verified clean against `cargo audit` (RustSec advisory database, 0 vulnerabilities across 447 transitive dependencies).

### Known limitations

- Detection patterns are English-only in this release; multilingual coverage is planned for a future release.
- Cursor lacks a pre-load hook surface; hard enforcement of pin verification on Cursor requires using the `envforge mcp launch cursor` wrapper. The `.cursor/rules` block is advisory only.
- The bundled reputation feed is shipped unsigned and refreshed via binary release; an externally-signed update channel is planned for a future release.
- Detection is deterministic (pattern-based), not ML-classifier-based; novel evasion techniques may bypass the v1 pattern set until patterns are updated.

### Added — AI Safety

All four address concrete CVE-class threats from agentic coding workflows (Claude Code, Cursor, Cline) and the broader CI supply-chain story. **133 new tests** (1740 → 1873)

#### JIT Lease — PID-binding extension to existing lease module (`envforge lease grant|revoke|status`)

- **New module surface in `src/ops/lease.rs`**: extended `Lease` struct with `pid`, `single_redeem`, `redeemed`, `tool_name` (all `#[serde(default)]` for backward compat). Net new ~350 LOC alongside existing 354 LOC; in-place extension per ADR-009.
- **JIT lifecycle**: `jit_grant(GrantRequest) → JitHandle`, `jit_redeem(handle) → Zeroizing<String>`, `jit_revoke(name, RevokeReason)`. Single-redeem semantics enforced; `Zeroizing` wrapper guarantees secret memory is overwritten on Drop.
- **PID watcher**: tokio task per active JIT lease, polls `libc::kill(pid, 0)` at 100ms cadence (configurable via `LEASE_WATCHER_POLL_MS`, clamped 50-500ms). Linux PID start-time fingerprint via `/proc/<pid>/stat` field 22 defeats reuse races; macOS falls back to PID-only with documented limitation. Per ADR-008 (tokio polling chosen over pidfd/kqueue for cross-platform simplicity).
- **WatcherRegistry**: process-wide `OnceLock<DashMap<String, JoinHandle<()>>>`. Watcher abort on explicit revoke is safe + idempotent.
- **Audit integration**: 3 new `EventType` variants (`LeaseGranted`, `LeaseRedeemed`, `LeaseRevoked`); audit emit failure is non-blocking (logs warning, lifecycle proceeds).
- **CLI**: `envforge lease grant --tool X --key K --pid P --ttl 30s [--multi-redeem] [--json]`, `envforge lease revoke <name>`, `envforge lease status <name>`. Existing `Create / List / Cleanup` unchanged.
- **`parse_lease_duration` extended** to accept `30s` (seconds suffix) alongside existing `m / h / d`.
- **New direct dep**: `libc = "0.2"` (was transitive only).
- **27 lease tests pass** (14 existing unchanged + 13 new). Critical bug caught + fixed during Stage 5 testing: deadlock in `jit_revoke` when pre-acquiring lock + calling `revoke_lease` which acquires same non-reentrant mutex; resolved by removing outer acquisition.

#### Canary v2 — Forensic decodable tokens (`envforge canary mint-v2|decode|scan|rotate-key|migrate`)

- **New submodule split** `src/ops/canary/{mod,v2,hmac_store,scanner,migration}.rs` per ADR-007 (existing `canary.rs` lifted to `mod.rs` verbatim; ~600 LOC of new code split into 4 cohesive concerns). Public API path unchanged.
- **Token format** `cnry_<39-char base32>_<13-char base32>` total 58 chars (within 64-char log-line budget). Payload: `machine_id[8] || pid[4 LE] || timestamp_secs[4 LE] || agent_name_hash[4] || key_name_hash[4]`. Timestamp epoch fixed at 2026-01-01T00:00:00Z (load-bearing constant).
- **HMAC-SHA256** with first 8 bytes truncated as integrity tag per ADR-006 (64-bit forgery cost ≈ 2^63 ops; well above local-oracle adversary capability for canary-class threats). Manual HMAC-SHA256 implementation avoids pinning hmac crate version against sha2 0.11. Constant-time comparison via manual `constant_time_eq` over XOR/OR accumulator.
- **HMAC key rotation**: `~/.envforge/canary-keys.age` (age-encrypted via existing recipient flow). Active key + max 2 retired keys verified on decode; oldest evicted on rotation.
- **CLI**: `mint-v2 <key> [--tool X] [--pid P] [--json]`, `decode <token> [--json]`, `scan <input> [--strict] [--json]`, `rotate-key [--dry-run]`, `migrate [--bulk] [--replace <key>] [--dry-run]`.
- **Backward compat**: existing v1 patterns (aws_key, github_token, stripe_key, slack_token, gitlab_token, database_url, jwt_token, openai_key, private_key_pem, smtp_credential, ftp_credential — 11 patterns) untouched. v1 records load via `serde(default)` on new fields. Migration is idempotent: `superseded_by` link added; v1 record never deleted.
- **CanarySecret extended**: `version: u8 (default 1)`, `forensic: bool`, `superseded_by: Option<String>`, `payload_summary: Option<PayloadSummary>`. `Default` impl added.
- **50 canary tests pass** (17 existing + 16 v2 + 5 hmac_store + 6 scanner + 6 migration). Includes RFC 4231 HMAC-SHA256 KAT (Test Case 1), 1000-payload deterministic roundtrip, tamper detection, retired-key fallback verify.

#### CI Comment-and-Control Guard (`envforge ci-trust classify|quarantine|summary` + GitHub Action `quarantine` input)

- **New module** `src/ops/ci_trust/{mod,classifier,quarantine,summary}.rs`. Per ADR-010, classification logic lives in the Rust binary (typed enums, unit-testable, future-reusable from local pre-push hooks) instead of inline bash + jq.
- **Classifier**: pure function `classify(TriggerContext) → TrustVerdict { level, reason, classifier_version }`. 12-row decision matrix covering `push`, `pull_request` (fork vs internal), `pull_request_target` (always Untrusted), `issue_comment` (author-association gated), `workflow_run` (conservative Untrusted), `workflow_dispatch`, `schedule`, unknown events. `Trusted` for owner/member/collaborator; `Untrusted` for everyone else. **Fail-closed** on missing/malformed input.
- **Verdict cache**: `$RUNNER_TEMP/envforge-trust.json` with `classifier_version` field for cache-format drift detection. Composite-action steps reuse without reclassifying.
- **Quarantine engine**: scrubs env by key-name regex (`(?i)(?:_KEY|_SECRET|_TOKEN|_PASSWORD|_PASS|_CREDENTIAL|_API_?KEY|_PRIVATE_KEY)$|_TOKEN_|_KEY_|_SECRET_`) OR value-shape (AWS `AKIA*`/`ASIA*`, GitHub `ghp_*`/`gho_*`/`ghu_*`, Stripe `sk_live_*`/`rk_live_*`, OpenAI `sk-*`, high-entropy ≥32-char strings). `GITHUB_TOKEN` always scrubbed unless explicitly allow-listed; `RUNNER_*` / `GITHUB_*` (non-token) / `CI` auto-allowed.
- **Step Summary + GitHub Outputs**: `envforge ci-trust summary` writes markdown to `$GITHUB_STEP_SUMMARY` plus 5 outputs to `$GITHUB_OUTPUT` (`quarantine_verdict`, `quarantine_reason`, `quarantine_applied`, `quarantine_scrubbed_count`, `quarantine_preserved_count`).
- **GitHub Action extended**: `action/action.yml` gains `quarantine` input (default `auto` — scrub on Untrusted; `force` always scrub; `off` opt out with explicit warning) and `allow-keys` input. `action/scripts/run.sh::apply_ci_trust` runs classify → decide → `eval "$(envforge ci-trust quarantine)"` → emit summary, all before mode dispatch.
- **Test workflow**: `.github/workflows/test-quarantine.yml` exercises 3 scenarios via `workflow_dispatch` input — `trusted` (canary preserved), `fork-pr` (canary scrubbed; verdict Untrusted/ForkPr), `external-comment` (verdict Untrusted/ExternalComment).
- **34 ci_trust tests pass** (18 classifier + 11 quarantine + 4 summary + 1 misc). Zero new dependencies.

#### ENV-BOM Attestation (`envforge envbom emit|verify|update-trust-root`)

- **Determinism**: `BTreeMap` for keys, sorted Vec for paths/profiles, recursive canonicalize-value pass before serialization → byte-identical output across `serde_json` versions and re-emits. CLI flag `--reproducible-now <RFC3339>` fixes `generated_at` for reproducible builds.
- **Audit-grade no-raw-values invariant**: enforced by `no_raw_value_in_serialized_bom` lint test that greps emitted JSON for the secret string.
- **Encrypted-value handling**: `ENC[age:...]` ciphertext recognized; hashed pre-decrypt; `value_state: "Encrypted"` annotates the entry. Missing values emit `value_sha256: null`, `value_state: "Missing"`.
- **Audit summary**: total keys, per-classification counts, unrotated-over-90d count, sorted+deduped provider list.
- **Diff (`verify --against-current`)**: BomDiff with added / removed / changed fields (ValueSha256, Classification, Owner, LastRotated, ProviderRef, SchemaRequired, ValueState).

### Tests

- **1873 total tests passing** under default build (up from 1740 in 0.7.5; +133 new tests across the 4 units)
- `cargo fmt --check` clean
- `cargo clippy --all-targets -- -D warnings` zero warnings under default

### Changed

- **`Lease` struct**: 4 new fields (`pid`, `single_redeem`, `redeemed`, `tool_name`) with `#[serde(default)]`; `Default` impl derived. Existing on-disk lease TOML files load unchanged.
- **`CanarySecret` struct**: 4 new fields (`version`, `forensic`, `superseded_by`, `payload_summary`) with `#[serde(default)]`; `Default` impl added. Existing on-disk canary records load unchanged.
- **`EventType` enum** (`src/ops/audit/types.rs`): 3 new variants (`LeaseGranted`, `LeaseRedeemed`, `LeaseRevoked`).
- **`parse_lease_duration`**: accepts `30s` suffix in addition to `m / h / d`.

## [0.7.5] - 2026-05-08

### Security

Comprehensive security hardening pass — 50 fixes across the entire codebase, organized below by severity. No external CVEs filed. Verification: `cargo build`, `cargo clippy --all-targets -- -D warnings`, full `cargo test` (1132 lib tests + integration suites) all green.

#### Critical

- **Git remote URL allowlist + RCE protection** (`src/ops/sync/git.rs`): `clone_repo` validates remote URLs via new `validate_remote_url` and rejects `ext::` (RCE via git-remote-ext), `file://` (local file disclosure), `rsync://`, `ftp(s)://`, `gopher://`, leading-dash, control-character URLs. Allowed: `https://`, `http://`, `ssh://`, `git://`, `git+{ssh,https}://`, scp-like `user@host:path`. Clone runs with `-c protocol.ext.allow=never` and an explicit `--` separator.
- **Hook prev-state file relocated out of project dir** (`src/ops/hook.rs`, `src/cli/{mod,commands}.rs`): `.envforge-prev` previously written into the project directory let any repo replace it with attacker-controlled shell that the bash/zsh/fish hook would `eval`. Now stored in user envforge config dir under `hook-state/<sha256(canonical_path)>.prev` (mode 0600); hooks call new `envforge env-unload --dir <dir>` subcommand instead of `eval "$(cat ...)"`. Output re-validated line-by-line before printing. Legacy in-project prev files auto-removed on first `envforge env`.
- **DNS-rebinding defense in proxy** (`src/ops/proxy.rs`): two layered defects — (a) `is_origin_allowed` accepted requests without an `Origin` header, (b) `format_response` always sent `Access-Control-Allow-Origin: *` — composed into a real exploit (DNS-rebound `evil.com` → 127.0.0.1 + simple GET → secret exfil). New `extract_host` + `is_host_loopback` helpers; the request handler now requires `Host` to match `127.0.0.1` / `localhost` / `[::1]` (with optional port) before serving any route. CORS header changed from `*` to `null` plus `Vary: Origin`. `extract_host` rejects requests with multiple Host headers (RFC 7230 §5.4).

#### High

- **Provider arg-flag injection** (`src/ops/secrets/providers/{onepassword,aws_ssm,gcp,azure,doppler,infisical,akeyless,sops,conjur,pass,vault,keeper,bitwarden}.rs`): every provider's `pull` / `push` / `get` / `list` calls `validate_provider_arg` for paths and `validate_secret_name` for keys. Where the underlying CLI supports it, positional args are placed after `--`. Provider-specific helpers (`checked_profile`, `checked_project`, `checked_vault`, `checked_project_config`, `checked_env_project`, `checked_project_id`) consolidate credential-field validation. `validate_secret_name` extended to reject leading `-`, `=`, control chars; new `validate_provider_arg` and `validate_provider_response_value` / `validate_provider_response_label` helpers (64 KiB / 512 B caps + NUL/control-char rejection) applied across all 14 providers' parsers.
- **`ENVFORGE_EXTERNAL_SCANNER` shell-meta rejection** (`src/ops/external_scanner.rs`): the deprecated env-var path used `split_whitespace`, which re-interpreted `/bin/sh -c "x"` as a shell-style argv. Now rejects values containing shell metacharacters and requires an absolute path. Plus `MAX_SCANNER_INPUT_BYTES = 4 MiB` cap on stdin payload sent to scanner subprocesses.
- **fsync before atomic rename** (`src/config/writer.rs`, `src/ops/sync/init.rs`): both atomic-write paths call `temp.as_file().sync_all()` before `persist`. Defends against torn writes on power loss leaving zero-length / corrupted files (encrypted secrets, sync snapshots).
- **Refuse to write secret-bearing files on non-unix** (`src/ops/encrypt.rs`, `src/ops/secrets/{cache,credentials}.rs`): the `#[cfg(not(unix))]` branches previously wrote age private keys / decrypted cache / credential store with default ACLs (world-readable on Windows). Now return runtime error.
- **Zeroize-on-drop for in-memory secrets** (`src/ops/secrets/{cache,credentials}.rs`): `CacheEntry` impls `Drop` that `zeroize::Zeroize`s its `value`. New `Credentials` wrapper struct (Drop+Deref) and `read_all_credentials_zeroizing()` for opt-in zero-on-scope-exit; existing `with_credentials` closure pattern preserved.
- **Audit log permissions + fsync on append** (`src/ops/audit/emitter.rs`): both append (default umask → 0644) and first-write (`atomic_write` no chmod) paths now set 0600 (defensive chmod on append-open if existing file has looser perms); append path calls `sync_all` after `flush` so power-loss between flush and writeback can no longer drop entries.
- **Profile name path traversal** (`src/ops/profile.rs`): `create_profile(name)` interpolated `name` into `~/.env_managed.{name}` and wrote there; same in `delete_profile` / `switch_profile`. New `validate_profile_name` enforces `[A-Za-z0-9_-]{1,64}` (no leading dash). New `ProfileError::InvalidName` variant.
- **Sync `decrypt_snapshot` size caps** (`src/ops/sync/encryption.rs`): `MAX_SNAPSHOT_CIPHERTEXT_BYTES = 8 MiB` checked before `Decryptor::new`; `Read::take(reader, MAX_SNAPSHOT_PLAINTEXT_BYTES)` on the decrypted reader. Defends against decompression-bomb age files in malicious sync remotes.

#### Medium

- **URI path traversal rejection** (`src/ops/uri_resolve.rs`): new `validate_uri_path` blocks `..` / `.` segments, leading `/`, `//`, control characters before dispatch to provider backends.
- **Provider response value validation** (`src/ops/secrets/{provider,providers/{onepassword,keeper,bitwarden,aws_ssm,gcp,azure,doppler,infisical,akeyless,sops,vault,conjur,pass}}.rs`): every JSON parser caps secret values at 64 KiB and rejects NUL / control chars; labels capped at 512 B.
- **Canary alerts redact own value** (`src/ops/canary.rs`): `trigger_canary` redacts the canary's `fake_value` from `source` / `details` before writing to `canary-alerts.jsonl`, monitor stream, and stderr — preventing the canary log from leaking the canary value it was meant to detect.
- **Sync `verify_signatures` config flag** (`src/ops/sync/{model,git,pull}.rs`): new `[sync] verify_signatures` (default `false`); when set, `envforge sync pull` runs `git verify-commit HEAD` after fetch and fails closed.
- **Sanitize length filter dropped** (`src/ops/sanitize.rs`): the `>= 4` cutoff allowed 3-character tokens (PINs / OTPs) to pass through redaction unchanged. Filter now only skips empty values.
- **Cache provider name sanitization** (`src/ops/secrets/cache.rs`): `cache_file_path` now sanitizes both `provider` and `key` (alphabet `[A-Za-z0-9_-]`, len + NUL checks); same in `invalidate_provider_cache`. Closes a path traversal where `provider="../etc"` could land cache writes outside the cache dir. Plus process-wide `Mutex` (`OnceLock`) over read-miss → fetch → write critical section in `resolve_reference`.
- **Lease lock + renew** (`src/ops/lease.rs`): process-wide mutex serializes every lease op (create / revoke / revoke-all / check) closing the in-process TOCTOU. New `with_lease_check_locked(key, f)` keeps the lock held from access check through secret release atomically. New `renew_lease(name, ttl_seconds)` for atomic extend.
- **Quote-style serialize escapes embedded quotes** (`src/model/shell_file.rs`): `Double` escapes `\` and `"`; `Single` uses POSIX close-escape-reopen `'\''` pattern. Restores byte-for-byte round-trip invariant on adversarial values.
- **Scanner per-file size cap** (`src/ops/scanner.rs`): 10 MiB cap before `read_to_string`. Plus centralized `MAX_SHELL_FILE_BYTES = 10 MiB` in `parse_shell_file` itself with new `ParseError::FileTooLarge` variant — fixes `check.rs`, `doctor.rs`, `hook.rs`, `profile.rs`, and every other caller in one place.
- **Audit query bounded read + filter input cap** (`src/ops/audit/{query_engine,query_types}.rs`): `read_all_events` streams via `BufReader::lines` (no whole-file load); `MAX_EVENTS_LOADED = 250_000` cap with stderr notice on truncation. `MetadataKey(String)` filter capped at 256 B.
- **Age decryption ciphertext + plaintext caps** (`src/ops/encrypt.rs`): `MAX_CIPHERTEXT_BYTES = 1 MiB` checked before `Decryptor::new`; `Read::take(reader, MAX_PLAINTEXT_BYTES = 1 MiB)` on output.
- **Share metadata `created_by` documented unauthenticated; share TTL hard-blocks expired** (`src/ops/share.rs`): rustdoc on `ShareMeta.created_by` makes the unauthentic-hostname contract explicit. `receive_share` now returns `ShareError::Expired` (was warn-only); new `receive_share_with(data, allow_expired)` for opt-in override. 8 MiB caps on encrypted blob and decrypted plaintext.
- **AI-guard secret detection: anchored prefixes + entropy fallback** (`src/ops/audit/ai_guard_integration.rs`): prefix matches must be anchored at a non-token boundary AND followed by ≥16 token-alphabet bytes; pattern table extended (12 → 18 prefixes). Entropy fallback flags any 32+ char run of `[A-Za-z0-9_\-./]` containing upper + lower + digits as credential-like.
- **Lifecycle snapshot 0600 + fsync** (`src/ops/lifecycle/rollback.rs`): new `write_atomic_snapshot` sets 0600 on the tempfile before write, `sync_all`s before persist, refuses non-unix. Plus `MAX_SNAPSHOT_FILE_BYTES = 4 MiB` cap on snapshot reads in `restore_snapshot`.
- **Cron min-interval guard** (`src/ops/lifecycle/trigger_engine.rs`): `MIN_CRON_INTERVAL_SECS = 60`; rules whose two consecutive next events are < 60 s apart are rejected.
- **Lifecycle state log 0600 + fsync** (`src/ops/lifecycle/orchestrator.rs::apply_state_transition`): state-transition `.jsonl` files open with `OpenOptions::mode(0o600)` on Unix; `sync_all` after `writeln`; defensive post-write chmod for files inherited from older versions.
- **Rule TOML size cap** (`src/ops/lifecycle/rule_manager.rs`): `MAX_RULE_FILE_BYTES = 256 KiB` checked before `read_to_string` + `toml::from_str` in every reader.
- **`mcp_scan` size + recursion-depth caps** (`src/ops/mcp_scan.rs`): `MAX_MCP_CONFIG_BYTES = 1 MiB` size cap before parse; `walk_json` carries `depth` parameter and bails at `MAX_JSON_DEPTH = 64`.
- **Audit chain-state deletion detection** (`src/ops/audit/tamper.rs`): `load_chain_state` returns `TamperError::InvalidState` when the state file is missing but log files exist (was silently re-initializing).
- **Analytics events file mode at create time** (`src/ops/analytics/storage.rs`): `OpenOptions::mode(0o600)` at create instead of post-chmod with `.ok()`. Errors no longer dropped.
- **Snapshot file 0600** (`src/ops/snapshot.rs`): new `write_snapshot_secure` helper uses `OpenOptions::mode(0o600)` on Unix.
- **Changelog file 0600** (`src/ops/changelog.rs::log_change`): `OpenOptions::mode(0o600)` at create + post-write defensive chmod for stale 0644 files.
- **LSP root URI canonicalization + size caps** (`src/lsp/server.rs`): `load_schema_from_workspace` canonicalizes the client-supplied `rootUri`, requires resolved schema path inside the canonicalized root, refuses schemas larger than 1 MiB. `did_open` / `did_change` enforce `MAX_DOCUMENT_BYTES = 1 MiB` on client-supplied content.
- **LSP completion no longer suggests live secret values as labels** (`src/lsp/completion.rs`): `redact_value_for_label` returns a redacted preview for the `label`; real value flows via `insert_text`.
- **Glob matcher iterative DP** (`src/ops/sync/marking.rs`, `src/ops/secrets/modes.rs`): replaces a recursive backtracker that was exponential on adversarial inputs (`a*a*a*a*a*b` × `aaaa…`). Now `O(P × T)`.
- **Profile diff masks plaintext values** (`src/ops/profile_diff.rs`): `DiffEntry.value_a` / `value_b` go through `mask_diff_value` at construction; new `values_differ: bool` field; rustdoc makes the masking contract explicit.
- **Markdown report escaping + RFC 4180 CSV** (`src/ops/audit/report_generator.rs`): new `markdown_escape` (escapes `<`, `>`, `&`, backticks, pipes, `[`, `]`, folds newlines) applied to violation rendering; new `csv_field` helper wraps every CSV field in `"…"`, doubles internal quotes, folds `\r` / `\n`. All six CSV columns now go through it (was missing `secret_key` and improperly quoted).
- **`ensure_age_key` chmod via file handle (TOCTOU)** (`src/ops/encrypt.rs`): opens the file once with `O_NOFOLLOW`, runs `File::set_permissions` on the handle, reads the same fd. Closes a path-level race where a same-uid attacker could swap the path between metadata and chmod.

#### Low

- **Cache TOCTOU narrowed** (`src/ops/secrets/cache.rs::resolve_reference`): process-wide `Mutex` (`OnceLock`) over read-miss → fetch → write. Eliminates in-process double-fetch races; cross-process safety still rests on tempfile + atomic rename + 0o600.
- **Clipboard auto-clear TTL** (`src/ops/clipboard.rs`): new `copy_to_clipboard_with_ttl(text, secs)` spawns a background thread that, after the TTL expires, clears the clipboard if-and-only-if it still holds the value we wrote. `copy_value` defaults to 30 s TTL when the key matches `is_sensitive_key`. Best-effort: macOS Pasteboard history and X11 PRIMARY/SECONDARY may still retain the value.
- **Profile / project_id validation** (`src/ops/secrets/providers/{keeper,bitwarden}.rs`): `checked_profile` / `checked_project_id` helpers run `validate_provider_arg` over Keeper's `profile` and Bitwarden's `project_id` before they are passed positionally to `ksm` / `bws`.
- **Env-var pair validation at run-CLI boundary** (`src/ops/secrets/provider.rs`, providers/`{vault,conjur,pass}.rs`): new `validate_env_pair(name, value)` enforces POSIX env name regex (`[A-Za-z_][A-Za-z0-9_]*`), rejects NUL bytes in name / value, rejects `\n` / `\r` in value. Called from `run_cli`, `run_cli_with_stdin` (covers `run_cli_with_tempfile` transitively), plus the three direct-spawn paths in pass / vault / conjur.
- **Monitor event message redaction** (`src/ops/monitor/mod.rs`): `emit_event` passes every `RuntimeEvent` through `redact_runtime_event`, which replaces high-entropy tokens (24+ chars of `[A-Za-z0-9_\-]`) in `message` with `[REDACTED]`. Last-line safety net for callers that put a secret value into a runtime event message.
- **TUI input length cap** (`src/ui/input.rs`): `MAX_INPUT_LEN = 128 KiB`; `insert` refuses past the cap, `new` truncates oversized initial text at a UTF-8 char boundary. Defends against bracketed-paste OOM.
- **Multi-Host header rejection** (`src/ops/proxy.rs::extract_host`): RFC 7230 §5.4 — returns `None` when more than one `host:` header present; handler treats `None` as fail → 403.
- **`ResolvedEntry` masked Debug** (`src/ops/uri_resolve.rs`): manual `Debug` impl renders `value` as `***(<n> chars)`. Closes leak via `format!("{:?}", entry)` / panic / `dbg!()`.
- **Backup file 0600** (`src/config/backup.rs`): post-`fs::copy` chmod 0600 on Unix; errors propagated.
- **`secret-sources.toml` 0600** (`src/ops/secrets/age.rs::save_sources`): `OpenOptions::mode(0o600)` at create + defensive chmod for stale files.
- **`secrets config --show` value preview no longer reveals credential prefix** (`src/cli/secrets_cmd.rs`): both JSON and human modes now show `***(<n> chars)` instead of leaking the first 4 chars (which exposed `AKIA…`/`sk-…`/`ghp_…`/`xoxb-…` credential-type fingerprints).

### False positives ruled out (after seven independent rescans)

- **`ai_hooks.rs` shell injection** — `"$TOOL_INPUT"` is double-quoted in the hook command string; shell `$`-expansion inside double quotes does not re-interpret metacharacters in the expanded value.
- **`ai_guard.rs::is_sensitive_path` Unicode bypass** — heuristic prepass; actual fence / permission decisions go through exact-byte path matches.
- **CORS / Origin / IDN bypasses on the proxy** — `to_ascii_lowercase` covers ASCII case folding; non-ASCII homoglyphs don't match `https://` and fall through to scp-like check requiring `@`. URL `?` / `#` smuggling — git does not interpret query strings as CLI flags.
- **`run.rs` argv leak** — secrets flow via `cmd.envs(env)` after `env_clear()`, NOT argv. `/proc/PID/cmdline` does not see them.
- **`vault.rs` role_id/secret_id payload "leak"** — written to `vault`'s **stdin** (deliberately avoiding `/proc/PID/cmdline`), not argv. The secure path.
- **`copy_key_value` clipboard exposure** / **`export_format::export_docker_secrets`** — both by design (user explicitly invokes); mitigated by clipboard TTL auto-clear.
- **Custody chain forgery** — addressed by `AuditEvent.entry_hash`/`prev_hash` chain plus chain-state deletion detection.
- **Parser `&s[1..]` panic in `parse_quoted_value`** — only entered when `trimmed.starts_with('"' | '\'')`; never panics.
- **C2 BOM-prefixed prev-state bypass** — envforge writes the file; we never emit BOM. BOM-prefixed lines from external tampering fail `is_safe_unload_line`.
- **`validate_env_pair` not applied to credentials** — every credential map flows `build_provider_env` → `env_refs_from_env` → `run_cli` / `run_cli_with_stdin`, all of which call `validate_env_pair`.
- **Volatile mode heap fragmentation** — `modes.rs` uses `zeroize` correctly; secrets cleared before scope exit.
- **`copypasta` unmaintained** — verified currently maintained (0.10.2); no open advisories.
- **`secrets/age.rs`** — usage-tracking module (records access events), not a decryption module. No bespoke crypto surface.
- **`rotate_secret` missing snapshot** — `rotate.rs::apply_rotation` performs its own atomic-write of the new value; not a security gap.
- **Machine override no auth** — `sync/machine.rs::set_override` operates on the local user's sync repo. Sync is consent-based.
- **`AuditEvent.metadata` non-deterministic hash** — `serde_json::Value` with default serde_json (no `preserve_order` feature) is alphabetically ordered → already deterministic.
- **`CustodyLink` missing hash field** — `event_id` references the `AuditEvent` whose `entry_hash` / `prev_hash` form the integrity chain.
- **CLI `--token` / `--password` / `--api-key` flags exist** — verified across multiple sweeps: no such flags. Sensitive values flow via stdin / prompt / file.
- **TUI edit dialog renders plaintext** — by design (the user asked to edit).
- **Proxy approval prompt no stdin timeout** — by design (interactive synchronous approval).
- **`model/error.rs::IoError` includes absolute path** — standard Rust error reporting; user files live under `$HOME` and the user is the local viewer.

### Tests

- 1132 lib tests pass, all integration suites green.
- New tests for `validate_remote_url`, `extract_host`, `is_host_loopback`, `csv_field`, glob-DP edge cases, sanitize length filter.

### Changed

- **Cargo.toml**: bumped to 0.7.5.
- **`SyncSettings`**: new `verify_signatures: bool` field (default `false`).
- **`ProfileError`**: new `InvalidName` variant.
- **`ParseError`**: new `FileTooLarge` variant.
- New public helpers in `src/ops/secrets/provider.rs`: `validate_provider_response_value`, `validate_provider_response_label`, `validate_provider_arg`, `validate_env_pair`, `MAX_PROVIDER_VALUE_LEN`, `MAX_PROVIDER_LABEL_LEN`.
- New public helpers in `src/ops/audit/report_generator.rs`: `markdown_escape`, `csv_field`.
- New constants: `parse::MAX_SHELL_FILE_BYTES`, `LspBackend::MAX_DOCUMENT_BYTES`, `MAX_RULE_FILE_BYTES`, `MAX_EVENTS_LOADED`, `MAX_CIPHERTEXT_BYTES`, `MAX_PLAINTEXT_BYTES`, `MAX_SNAPSHOT_CIPHERTEXT_BYTES`, `MAX_SNAPSHOT_PLAINTEXT_BYTES`, `MIN_CRON_INTERVAL_SECS`.
- New lease APIs: `renew_lease`, `with_lease_check_locked`.
- New share API: `receive_share_with(data, allow_expired)`. `ShareError::Expired` now returned (was warn-only).
- New proxy helpers: `extract_host`, `is_host_loopback`. CORS header `*` → `null` + `Vary: Origin`.
- `ResolvedEntry` uses manual masked `Debug` (was `derive(Debug)`).
- `walk_json` in `mcp_scan.rs` gained required `depth: usize` parameter (private function; no public-API impact).
- `read_all_credentials` doc-commented to prefer `with_credentials` or `read_all_credentials_zeroizing`.

## [0.7.4] - 2026-05-07

### Added — Secret Lifecycle Automation

#### Lifecycle Engine (`envforge lifecycle`)
- **New module**: `src/ops/lifecycle/` — automated secret lifecycle with 6 submodules:
  - **Orchestrator** (`orchestrator.rs`): Create/rotate/decommission workflows with result types (CreateResult, RotateResult, DecommissionResult)
  - **Rule Manager** (`rule_manager.rs`): CRUD for `LifecycleRule` entries, enable/disable toggles, serialization to TOML
  - **Trigger Engine** (`trigger_engine.rs`): Evaluate `LifecycleTrigger` variants — Cron (minute/hour precision), AgeExceeded, FileChange, PolicyViolation, and Composite (All/Any/Not operators) with `EvaluationContext`
  - **State Machine** (`state_machine.rs`): 7-state lifecycle (Creating → Active → Rotating → PendingDeprecation → Deprecated → Decommissioned → Failed) with `StateTransition` history
  - **Rollback** (`rollback.rs`): Snapshot-based rollback with `SnapshotMeta`/`Snapshot` tracking, UUID-keyed storage
  - **Schema Lifecycle** (`schema_lifecycle.rs`): Auto-generate lifecycle rules from `.env.schema` TTL/auto_rotate fields
- **Model types**: `LifecycleRule`, `LifecycleTrigger`, `LifecycleAction`, `RotationStrategy`, `LifecycleState`, `SecretLifecycle`, `TriggerEvent`, `EvaluationContext`, `SecretTemplate`, `RotationPolicy`, `LifecycleOperation`, `Snapshot`, `StateEvent`, `DecommissionPlan`, `RollbackResult`, `RecoveryResult`
- **CLI**: `envforge lifecycle check` — evaluate all rules; `envforge lifecycle rule list|rotate-secret` — manage rules; `envforge lifecycle state <KEY>` — show lifecycle state; `envforge lifecycle snapshot list|delete` — manage snapshots
- **Config**: `[lifecycle]` section in `.envforge.project.toml` — `default_stale_threshold_days`, `default_grace_period_days`, `default_rotation_strategy`, `snapshot_retention_days`

### Added — Secret Usage Analytics

#### Analytics Engine (`envforge analytics`)
- **New module**: `src/ops/analytics.rs` — track and analyze secret usage patterns:
  - **Unused Detection** (`unused`): Detect dormant secrets with no access in N days, confidence scoring
  - **Low Usage** (`unused`): Flag secrets with access count below threshold in time window
  - **Deprecation** (`unused`): Generate phased deprecation timelines (review → deprecate → remove) with dependent count
  - **Aggregation** (`aggregation`): Daily bucketing by key+date, accessor counts, type breakdowns
  - **Event Storage** (`storage`): JSONL log in `~/.local/share/envforge/analytics/`, enrichment pipeline
  - **Collection** (`collector`): Hook into proxy/CLI/sync access, auto-enrich with provider/environment/risk level
- **Model types**: `AnalyticsConfig`, `RawAccessEvent`, `EnrichedAccessEvent`, `AccessorInfo`, `AccessType`, `AccessSource`, `RiskLevel`, `AnalyticsError`
- **CLI**: `envforge analytics unused [--threshold N]` — dormant secrets; `envforge analytics low-usage [--max-accesses N] [--days N]` — low activity; `envforge analytics deprecation` — deprecation timelines; `envforge analytics summary [--days N]` — event/secret counts; `envforge analytics recompute` — recalculate aggregates; `envforge analytics retention show|set --days N` — retention policy; `envforge analytics prune [--before DATE]` — remove old events
- **Config**: `[analytics]` section — `enabled`, `retention_days`, `max_events`, `auto_aggregate`, `store_path`

### Added — Real-Time Monitoring

#### Health Monitor (`envforge monitor`)
- **New module**: `src/ops/monitor/` — real-time infrastructure health probes:
  - **Health checks** (`health.rs`): Provider availability (registry count, binary reachability), canary integrity, encryption key accessibility, fence status — all non-blocking with latency tracking
  - **Event fingerprinting** (`fingerprint.rs`): Unique event identity for deduplication and audit
- **CLI**: `envforge monitor status` — run all health probes (text/JSON output); `envforge monitor stream` — live event stream (JSON Lines to stdout)

### Added — Documentation

- **API Reference** (`docs/api-reference.md`): Full library reference covering `envforge::parser` (5 pub fns), `envforge::model` (LineNode, ShellFile, Shell, session/lifecycle/analytics types), `envforge::config` (AppConfig, backup, atomic_write), `envforge::ops` (OpError + re-exports), `envforge::lsp` (start_lsp_server). Includes 5 How-to code recipes.
- **CLI Reference** (`docs/cli-reference.md`): Updated to v0.7.4. Added Quick Recipes table (14 workflows), lifecycle (6 subcommands), analytics (8 subcommands), monitor (2 subcommands) sections.
- **README**: Architecture tree updated to 50+ ops modules, feature counts updated (25 AI safety tools, 90+ commands), new Lifecycle/Analytics/Monitoring feature rows.

### Changed

- **Cargo.toml**: bumped version to 0.7.4
- **CLI**: Added `--k8s-name`, `--k8s-namespace` flags to `export` for Kubernetes Secret name/namespace customization

### Fixed

- **Test quality**: Removed dead code (`make_context_with_last`), fixed redundant clones, resolved `field_reassign_with_default` clippy lints, fixed `needless_collect` patterns across 5 test files
- **Cron trigger test**: Fixed spurious `>= 0` assertion (always true) in composite trigger test — replaced with no-panic verification

## [0.7.2] - 2026-05-06

### Added — AI Safety Hardening Suite

#### Adversarial Input Hardening (`envforge hardening`)
- **New module**: `src/ops/hardening.rs` — 4-layer adversarial input detection pipeline
  - **Control character strip**: Removes null bytes, BIDI override marks, zero-width characters, normalizes Cyrillic homoglyphs (`а`→`a`, `е`→`e`, `о`→`o`)
  - **Base64 decode**: Finds and decodes Base64/URL-safe Base64 substrings >20 chars, re-scans decoded content for secrets
  - **Split string detection**: Detects `"sec" + "ret"` concatenation, `${VAR}` template expansion, `['a','b'].join('')` array joins
  - **Encoding chain decode**: Recursive decode up to depth 3 (Base64 → hex → Base64), re-scans at each level
- **Config**: `[ai_guard.hardening]` section in `.envforge.project.toml` with per-layer toggles
  - `control_chars`, `base64_decode` (min_length), `split_strings`, `encoding_chain` (max_depth)
- **CLI**: `envforge hardening show` — display current config; `envforge hardening enable/disable <layer>` — toggle layers
- **AI Guard integration**: `run_guard()` PreTool/PostTool stages now scan hardened (derived) strings alongside original input
  - Warnings include source layer: `"Secret value detected in decoded input (key: API_KEY)"`

#### External Scanner Interface (`envforge scanner`)
- **New module**: `src/ops/external_scanner.rs` — first-class multi-scanner pipeline
  - Replaces legacy `ENVFORGE_EXTERNAL_SCANNER` env var (deprecated but still supported with warning)
  - `[scanners.NAME]` section in `.envforge.project.toml` with `command`, `args`, `timeout_ms`, `enabled`
  - Concurrent execution via Tokio — all enabled scanners run in parallel
  - Per-scanner timeout (default 5s) — never blocks tool execution
  - Exit code 0 = clean, non-zero = stdout/stderr lines become findings
- **CLI**: `envforge scanner list` — show configured scanners; `envforge scanner test <name>` — test with sample content; `envforge scanner run <name> <content>` — ad-hoc scan; `envforge scanner enable/disable <name>` — toggle
- **AI Guard integration**: Scanner pipeline runs automatically in PreTool and PostTool stages when scanners configured

#### Canary Coverage Expansion (`envforge canary`)
- **6 new pattern types** (12 total):
  - `database_url` — `postgres://canary_user:...@canary-host:5432/canary_db`
  - `jwt_token` — Valid 3-part JWT structure with fake claims
  - `openai_key` — `sk-canary-...` format
  - `private_key_pem` — Plausible PEM block with BEGIN/END markers
  - `smtp_credential` — `smtp://canary_user:...@smtp.canary.local:587`
  - `ftp_credential` — `ftp://canary_user:...@ftp.canary.local:21`
- **Auto-rotation**: `rotate_after_days` field (default 14) per canary
  - `envforge canary rotate --all` — rotate all eligible canaries
  - `envforge canary rotate --key <KEY>` — rotate specific canary
  - `--dry-run` shows what would be rotated without changes
  - Rotation resets `triggered` and `trigger_count` state
- **Placement**: `envforge canary place <KEY> <FILE> [--position top|middle|bottom|random]`
  - Injects `# envforge canary: KEY=VALUE` line at specified position
  - Detects and skips duplicates

#### AI Tool Session Management (`envforge session`)
- **New module**: `src/ops/session.rs` — lightweight per-AI-tool session scoping
  - `SessionManager` — thread-safe in-memory store (`Mutex<HashMap>`) for active sessions
  - `create_session(tool, ttl)` — start a session with auto-generated UUID and expiry
  - `stop_session(id)` — expire a session (uses `ENVFORGE_SESSION_ID` env var as fallback)
  - `list_sessions()` — show all sessions with remaining TTL
  - `cleanup_expired()` — remove stale sessions
  - `detect_ai_tool()` — auto-detects tool from env vars (`CLAUDE_CODE`, `CURSOR`, `GITHUB_COPILOT`)
- **New model**: `src/model/session.rs` — session data types
  - `SessionId`, `SessionState` (Active/Expired), `AiTool` (ClaudeCode, Copilot, Cursor, Unknown)
  - `SessionConfig` with configurable default TTL (default: 1h)
- **CLI**: `envforge session start` — start session; `envforge session stop [id]` — stop session; `envforge session list` — list sessions; `envforge session show <id>` — session details; `envforge session cleanup` — remove expired
- **5 unit tests**: session lifecycle, TTL parsing, cleanup, tool detection, duration formatting

### Changed

- **AI Guard `run_guard()` signature**: Added `hardening` and `scanner_findings` parameters for integration with new safety layers
- **Project config schema**: Added `[ai_guard]` section with `hardening` and `scanners` sub-sections
- **Tokio features**: Added `process`, `time`, `io-util` for async scanner execution

### Fixed

- **Audit trail CLI routing**: Wired `AuditTrail` into `Commands` enum (`envforge audit-trail`). The 7 audit-trail subcommands (query, report, custody, integrity, stats, tail, retention) were fully implemented in `src/cli/audit_cmd.rs` but unreachable from the CLI. Added the missing enum variant and match arm.

### Dependencies

- Added `hex = "0.4"` for encoding chain decode layer

### Quality

- **805 total tests** (up from 610), all passing
  - 16 new hardening tests (control chars, Base64, split strings, encoding chains, composition)
  - 7 new external scanner tests (registry, concurrent execution, timeout, findings)
  - 6 new canary pattern tests (database_url, jwt, openai_key, pem, smtp, ftp)
  - 176 new audit trail tests (core-data, emitter, tamper, query-engine, custody, report-generator, ai-guard-integration)
  - 5 new session tests (lifecycle, TTL parsing, cleanup, tool detection, duration formatting)
- **cargo clippy**: 0 warnings
- **cargo fmt**: Clean
- No breaking changes — all existing AI Guard behavior preserved when new features disabled

#### AI Audit Trail (`envforge audit-trail`)
- **New module**: `src/ops/audit/` — comprehensive audit system with 8 submodules:
  - **Core Data** (`types.rs`, `query_types.rs`, `report_types.rs`): AuditEvent, Query, Filter, TimeRange, Pagination, ReportConfig, ComplianceScore, Aggregation types
  - **Event Emitter** (`emitter.rs`): JSONL log emitter with per-source separation, enrichment (hostname/PID/timestamp), atomic writes
  - **Tamper-Evident Writer** (`tamper.rs`): SHA-256 hash chain for all log entries, persistent chain state, integrity verification with ChainBreak detection
  - **Query Engine** (`query_engine.rs`): Execute queries with time/field filters, sort, pagination, aggregation (by EventType, Source, etc.)
  - **Chain of Custody** (`custody.rs`): Secret lineage tracking, session paths, ownership verification, custody gap detection
  - **Report Generator** (`report_generator.rs`): SOC2 compliance reports, violation detection (UnauthorizedAccess, CustodyGap, SecretExposure, AnomalousFrequency), compliance scoring, JSON/CSV/Markdown export
  - **AI Guard Integration** (`ai_guard_integration.rs`): Pre/post-tool audit events, secret binding/exposure logging, session lifecycle tracking, input secret detection
  - **CLI Commands** (`audit_cmd.rs`): `envforge audit-trail {query,report,custody,integrity,stats,tail,retention}` — 8 subcommands for full audit lifecycle
- **176 total audit tests** — all passing, covering all 8 submodules
- Full integration with existing `EventSource::AiGuard`, `EventType`, and emitter infrastructure
- No breaking changes — all existing audit (git sync) functionality preserved

## [0.7.0] - 2026-05-02

### Added — Editor CLI API Gaps (for Plugin Support v0.1.2)

- **New Subcommand: `search`**: Fuzzy search across all environment variables.
  - `envforge search <query>`: Interactive text output with scores.
  - `envforge search <query> --json`: Structured output with matched indices for IDE highlighting.
  - **Security**: Automatic value masking for sensitive keys (SECRET, TOKEN, PASSWORD, etc.).
- **Enhanced `move` (Rename)**: Support for in-place renaming of variables.
  - `envforge move OLD_KEY NEW_KEY`: Renames a key while preserving its value and position.
- **Enhanced `fence`**: Added status monitoring.
  - `envforge fence --status`: Check which AI ignore files exist and are correctly configured.
- **New Subcommand: `ai-hook status`**: Programmatic check for active security hooks.
  - `envforge ai-hook status [--json]`: Reports installation status for Claude Code and Cursor hooks.
- **New Subcommand: `mcp status`**: Security auditing for MCP configurations.
  - `envforge mcp status [--json]`: Scans Claude, Cursor, and Copilot configs for hardcoded plaintext secrets.
- **Comprehensive JSON Output**: Added `--json` support to 11 commands to support stable IDE integrations.
  - Now includes `ai-hook status`, `mcp status`, `canary list`, `profile list`, `snapshot list`, `sync status`, `audit`, `lease list`, `secrets providers`, `drift`, and `check`.
- **Standardized API**: All JSON outputs now include a `"version": 1` field for future-proof API stability.

### Changed

- **CLI Security**: `envforge list` and `envforge search` now automatically mask sensitive variable values (e.g., `ABC***XYZ`) to prevent plain-text exposure in terminal logs.
- **Internal**: derived `Serialize` on core models to support structured output across the codebase.
- **Testing**: Added 48 new integration tests covering JSON output schemas and new subcommand edge cases.

## [0.6.2] - 2026-04-27

### Added — Provider Security Hardening

- **Credential Exposure Prevention**: All 13 secret provider integrations now pass credentials via environment variables or stdin pipes instead of CLI flags, preventing credential leakage via `/proc/PID/cmdline` or `ps aux`.
  - `AkeylessProvider`: Migrated `access_id`/`access_key` from `--access-id`/`--access-key` CLI flags to `AKEYLESS_ACCESS_ID`/`AKEYLESS_ACCESS_KEY` environment variables.
  - `ConjurProvider`: Migrated `api_key` from `-p` CLI flag to stdin pipe, added `CONJUR_APPLIANCE_URL`, `CONJUR_ACCOUNT`, `CONJUR_AUTHN_LOGIN`, `CONJUR_AUTHN_API_KEY` environment variables.
  - `VaultProvider` (AppRole auth): Migrated `role_id`/`secret_id` from `vault write` positional args to stdin pipe.
- **Error Message Sanitization**: New `sanitize_error_output()` function redacts 11 credential patterns (tokens, API keys, passwords, etc.) from CLI error output before logging or display.
- **Input Validation**: New `validate_secret_name()` and `validate_secret_value()` functions reject null bytes, newlines, and oversized inputs in user-provided secret data.
- **CLI Version Verification**: New `minimum_version()` and `verify_version()` trait methods on all 13 providers, with minimum versions logged as warnings when CLI binaries are outdated.
- **CLI Binary Audit Workflow**: New `.github/workflows/cli-audit.yml` for weekly automated version checks of provider CLI binaries.

### Fixed — Encryption & File Permission Hardening

- **Age Plugin Feature Disabled**: `age` crate now compiled with `default-features = false` and only `armor` feature enabled, eliminating the RUSTSEC-2024-0433 arbitrary code execution attack vector.
- **Credential File Permissions**: `credentials.toml` now written with `0600` permissions using atomic writes (tempfile + rename). Permission check on load auto-fixes and warns if file is group/world-readable.
- **Age Key File Permissions**: `age.key` now written with `0600` permissions using atomic writes. Permission check on existing files auto-fixes and warns if overly permissive.
- **Secret Cache Permissions**: Cache files (containing decrypted secrets) now written with `0600` permissions instead of world-readable defaults.
- **SOPS Temp File**: Temp file for SOPS push operations now uses `NamedTempFile` with `0600` permissions instead of world-readable `/tmp` path.

### Changed — Security Documentation

- **SECURITY.md**: Complete rewrite with minimum CLI version table, credential passing method documentation, updated file permissions table, and CLI binary security model.
- **Provider Integrations Security Audit**: Comprehensive plan at `plans/provider-integrations-security-audit.md` with all 26 remediation items completed and verified.

## [0.6.1] - 2026-04-24

### Added — Automated Security Compliance
- **Security CI Workflows**: New GitHub Actions for automated security auditing and policy enforcement.
  - `cargo audit` scheduled daily checks for dependency vulnerabilities.
  - `cargo deny` enforcement of licenses, banned crates, and unmaintained packages.
  - `CodeQL` static application security testing (SAST) for automated code scanning.
- **Dependency Automation**: Configured Dependabot for weekly automated updates of Cargo dependencies and GitHub Actions.
- **Security Documentation**: Expanded `SECURITY.md` with details on automated scanning and security posture.

### Fixed — Dependency Vulnerabilities & Maintenance
- **Security Patches**:
  - Upgraded `rand` to `0.9.3` to fix unsoundness/Stacked Borrows violation (RUSTSEC-2026-0097).
  - Upgraded `age` to `0.11.3` to include the latest security and stability patches.
- **Maintenance & Soundness**:
  - Upgraded `ratatui` to `0.30.0`, moving to the latest modular architecture.
  - Fixed unsoundness in `lru` (RUSTSEC-2026-0002) and removed unmaintained `paste` (RUSTSEC-2024-0436) via the `ratatui` upgrade.
  - Migrated from deprecated and unsound `serde_yaml`/`serde_yml` to the maintained `serde_norway` fork.
  - Refreshed 23+ transitive dependencies to their latest compatible versions.

## [0.6.0] - 2026-04-23

### Added — Project-Scoped Configuration

18 new `envforge project` subcommands for managing project-level environment variables with multi-environment support.

#### Project Init & Wizard
- `envforge project init` — Create project config file (TOML/YAML/JSON format, user choice)
- `envforge project wizard` — 3-step guided setup: init → schema → key-value entry (resumable)
- Detects existing `.env` and `.env.schema` — prompts import or override
- Auto-adds `.env.*` patterns to `.gitignore`
- Config file: `.envforge.project.{toml,yaml,json}` (distinct from `.envforge.toml` shell hook)

#### Multi-Environment Support
- `envforge project env create <name>` — Create dev/staging/prod environments
- `envforge project env list` — List environments with active indicator
- `envforge project env switch <name>` — Switch active environment
- `envforge project env delete <name>` — Remove environment
- `envforge project env diff <a> <b>` — Compare two environments side-by-side
- Each environment maps to a separate `.env.<name>` file

#### Project Tools
- `envforge project validate` — Validate against project schema
- `envforge project scan` — Scan project for leaked secrets (`--staged`, `--mcp`)
- `envforge project schema generate` — Generate `.env.schema` from project `.env`
- `envforge project schema emit-ai` — AI-safe context (no values)
- `envforge project fence` — Create AI ignore rules for project
- `envforge project sanitize <file>` — Strip secrets using project env values
- `envforge project export` — Export project env (`--safe`, `--format json/yaml`)

#### Project Provider Integration
- `envforge project pull --from <provider>` — Pull secrets into project `.env`
- `envforge project push --to <provider>` — Push project `.env` to provider
- All 13 providers supported
- `envforge project status` — Project health overview
- `envforge project config` — View/edit project settings

#### Auto-Detection in `envforge run`
- `envforge run` auto-detects project config in cwd/parents
- Loads active environment's `.env` into env merge chain (after shell config, before `--env-file`)
- `--no-project` flag disables auto-detection
- Zero breaking changes to existing `envforge run` behavior

### Added — Provider Integration Documentation

Per-provider setup guides for all 13 secret manager providers, inline in CLI reference.

#### Provider Guides (in `docs/cli-reference.md`)
- Each provider: prerequisites, credential fields, path format, end-to-end workflow, auth details, troubleshooting
- Quick reference table with required fields and path behavior
- Providers: HashiCorp Vault, AWS SSM, 1Password, Doppler, Infisical, GCP Secret Manager, Azure Key Vault, Bitwarden, Akeyless, CyberArk Conjur, Mozilla SOPS, pass/gopass, Keeper

#### Documentation Site (`docs/docs.html`)
- Dynamic sidebar auto-generated from markdown headings (H2 sections, H3 sub-items)
- Collapsible sections with scroll-tracking active highlight
- H4 heading support for provider subsections
- Code block rendering fix (no more extra spacing from paragraph wrapping)
- Markdown link rendering (external links open in new tab)

### Changed

#### Test Suite
- 1325 total tests (up from 1191 in v0.5.7)
  - 59 new project-specific tests (46 core + 13 wizard)
  - 75 additional edge-case and coverage tests across modules
- All tests passing: 100% (0 failures)

#### Dependencies
- Added `serde_yaml = "0.9"` for YAML project config support

#### Documentation Updates
- `docs/cli-reference.md` — 4621 lines (up from ~2340), includes 18 project commands + 13 provider guides
- `README.md` — Added Projects to features table and Quick Start
- `PROVIDER_FRAMEWORK_GUIDE.md` — Updated test count (1325) and provider count (13)
- `action/action.yml` — Updated example version to 0.6.0
- Version bumped across all docs (v0.5.7 → v0.6.0)

#### Release Pipeline
- Multi-platform binary builds (x86_64 + aarch64, Linux + macOS)
- VSCode extension bundled with release
- IntelliJ plugin bundled with release
- Auto-extracted release notes from CHANGELOG

### Quality Assurance

- **Zero breaking changes** — 100% backward compatible
- **cargo clippy** — 0 warnings
- **cargo fmt** — Clean
- New dependency: `serde_yaml` only
- 26 files changed, +6800 lines across project, docs, and tests

## [0.5.7] - 2026-04-23

### Added — 6 New Secret Manager Providers

Total secret manager integrations expanded from 7 to **13 providers**.

#### Bitwarden Secrets Manager (`bitwarden`)
- CLI binary: `bws` (Bitwarden Secrets Manager CLI)
- Auth: Machine account access token (`BWS_ACCESS_TOKEN`)
- Features: Pull, push (create/update by key), list. Project-based organization.
- Install: `cargo install bws` or GitHub releases

#### Akeyless Vault (`akeyless`)
- CLI binary: `akeyless`
- Auth: Access ID + Access Key (profile or per-command)
- Features: Pull, push (create/update), get, list. Path-based hierarchy.
- Filters to STATIC_SECRET type only (ignores dynamic/rotated secrets)
- Install: Homebrew tap or binary download

#### CyberArk Conjur (`conjur`)
- CLI binary: `conjur` (Go-based CLI v8+)
- Auth: Account + URL + Login + API key (init + login flow)
- Features: Pull, push, get, list. Policy-based variable organization.
- Parses resource IDs (`account:variable:path/name`) automatically
- Install: `brew install cyberark/tools/conjur-cli`

#### Mozilla SOPS (`sops`)
- CLI binary: `sops`
- Auth: age key file (`SOPS_AGE_KEY_FILE`)
- Features: Pull (decrypt), push (decrypt-merge-encrypt cycle), get (extract), list
- **File-based paradigm**: `path` = encrypted file on disk, not remote URL
- Supports age, PGP, AWS KMS, GCP KMS, Azure KV encryption backends
- Install: `brew install sops`

#### pass/gopass (`pass`)
- CLI binary: `pass` or `gopass` (auto-detected, gopass preferred)
- Auth: GPG keyring (no explicit credentials needed)
- Features: Pull, push, get, list. Directory-based organization.
- Scans `~/.password-store/**/*.gpg` for secret enumeration
- Custom store path via `PASSWORD_STORE_DIR`
- Install: `brew install pass` / `brew install gopass`

#### Keeper Secrets Manager (`keeper`)
- CLI binary: `ksm`
- Auth: Device config (one-time token initialization via `ksm profile init`)
- Features: Pull, get, list. Push = update only (create not supported).
- Parses complex nested record JSON with typed field arrays
- Install: `pip3 install keeper-secrets-manager-cli`

### Changed

#### Test Suite Expansion
- 1191 total tests (up from 697 in v0.5.6)
  - 494 new tests including 110+ provider-specific tests for 6 new providers
  - Filesystem scanning tests for pass/gopass provider
  - Complex JSON parsing tests for Keeper nested records
  - Edge case coverage: missing fields, malformed entries, type fallback chains
- All tests passing: 100% (0 failures)

#### Provider Registry
- `create_default_registry()` now registers 13 providers
- Registry tests updated to verify all 13 providers

### Quality Assurance

- **Zero breaking changes** — 100% backward compatible
- **No API changes** — All existing 7 providers unchanged
- **No new dependencies** — All providers use existing CLI-wrapper pattern
- **cargo clippy** — 0 new warnings
- **cargo fmt** — Clean

## [0.5.6] - 2026-04-22

### Fixed — Critical Bug Fixes & Stability

#### UTF-8 Character Boundary Safety
- **Critical**: `scanner.rs::truncate_line()` now respects multi-byte UTF-8 character boundaries
  - Previously panicked when truncating error messages containing multi-byte characters (em-dash "—", accented letters, emoji, etc.)
  - Now iterates through `.chars()` to determine safe truncation points
  - Affects secret scanning output formatting across all UI contexts
  - Test `test_check_run_skips_missing_prerequisites` now passes (was panicking on em-dash)

#### Documentation Examples Correction
- Fixed crate-level documentation examples in `src/lib.rs`
  - Corrected API names: `parse_file` → `parse_shell_file`
  - Removed reference to non-existent `add_or_update()` function
  - Doc tests now compile and pass (2/2 examples verified)

### Changed

#### Test Suite Expansion
- 697 total tests (up from 664 in v0.5.3)
  - Library unit tests: 389
  - Integration tests: 306 (11 test suites)
  - Doc tests: 2 (new)
- All tests passing: 100% (0 failures)
- No new test dependencies

#### Code Quality
- 0 clippy warnings (maintained from v0.5.5)
- UTF-8 safety verified across all string handling paths
- Documentation accuracy: 100% verified against actual codebase

### Quality Assurance

- **Zero breaking changes** — 100% backward compatible
- **No API changes** — Drop-in replacement for v0.5.5
- **No new dependencies** — All changes internal to existing modules

## [0.5.4] - 2026-04-20

### Added — Advanced AI Safety

#### Canary Secrets (Honeypot Credentials)
- `envforge canary create KEY [--pattern aws_key|github_token|stripe_key|slack_token|gitlab_token|generic]` — Plant fake credentials
- `envforge canary list` — Show all canary secrets with trigger status
- `envforge canary check` — Check for triggered canaries (exfiltration detected)
- `envforge canary delete KEY` — Remove a canary
- Generates plausible-looking fake values per pattern type
- Integrated into AI guard: canary values in tool output trigger alerts
- JSONL alert log at `~/.config/envforge/canary-alerts.jsonl`

#### Zero-Access Approval Flow
- `envforge proxy --require-approval` — Human must approve each secret access request
- Agent request triggers terminal prompt: `🔒 Secret access request: KEY from 127.0.0.1`
- Human types `y` to approve, anything else denies
- Denied requests return 403 and are logged to audit trail
- Combinable with `--require-lease` for layered security

#### Secret Dependency Mapping
- `envforge deps KEY [--source]` — Find all references to an env var across project
- Scans: EnvForge managed files, .env files, config files (docker-compose, terraform, k8s, GitHub Actions)
- `--source` enables source code scanning (9 languages: JS, Python, Rust, Go, Java, Ruby, PHP, C, Shell)
- Skips `.git`, `node_modules`, `target`, `vendor`, `__pycache__`
- Grouped output by reference type with file:line context
- Answers: "If I rotate DB_PASSWORD, what breaks?"

#### External Scanner Hook
- Set `ENVFORGE_EXTERNAL_SCANNER="ggshield secret scan"` to delegate detection to external tools
- AI guard automatically calls external scanner on tool inputs/outputs
- Integrates with ggshield (500+ secret detectors) without competing
- Fallback: if external scanner not set, uses built-in pattern matching

#### Built-in Man Pages
- `envforge man` — Full command index grouped by category
- `envforge man COMMAND` — Detailed man page for any command (NAME, SYNOPSIS, DESCRIPTION, OPTIONS, EXAMPLES)
- 87 command entries parsed from embedded CLI reference
- Short name lookup: `man list` and `man "envforge list"` both work
- "Did you mean?" suggestions for typos

#### IDE Extensions & Language Server
- `envforge lsp` — Built-in LSP server (stdio transport) for IDE integration
- **VS Code extension** — [Marketplace](https://marketplace.visualstudio.com/items?itemName=emreerinc.envforge-env-manager)
  - Diagnostics, hover, completions, go-to-definition via LSP
  - Variables panel with prefix grouping, sensitive value masking
  - Profiles panel with one-click switching
  - 13 commands: validate, scan, export, sync, profile switch/diff, schema generate
  - Copy Key Name / Copy Value via click and context menu
  - Status bar with variable count
  - Syntax highlighting for `.env` and `.env.schema` files
  - Search in API documentation (`/` or `Cmd+K`)
- **IntelliJ IDEA plugin** — [JetBrains Marketplace](https://plugins.jetbrains.com/plugin/31385-envforge)
  - Same LSP features via LSP4IJ (diagnostics, hover, completions, go-to-definition)
  - Tool window with Variables + Profiles panels
  - Prefix grouping toggle, sensitive value masking
  - Right-click: Copy Key Name, Copy Value, Copy KEY=VALUE
  - 11 actions under Tools > EnvForge menu
  - Supports IntelliJ 2024.2 — 2025.2+
- Completions source from all envforge-managed vars (not hardcoded list)
- Value completions: bool → true/false, enum → allowed values, defaults, `${VAR}` references
- `K` key in TUI — copy key name to clipboard (new)
- `envforge copy KEY --key-only` — copy key name via CLI (new)

#### Documentation & Shell Setup
- `docs/cli-reference.md` — 2,266-line comprehensive CLI reference
- `docs/docs.html` — Styled documentation viewer with sidebar navigation
- Shell completions for zsh (Oh-My-Zsh compatible), bash, fish, Kiro CLI, Fig
- `envforge completions <shell> --install` — auto-install completions to correct system path
- Kiro CLI integration: auto-configures `devCompletionsFolder` + `developerMode`, strips TypeScript annotations for JS compatibility
- 3-page TUI help system (`?` key): Shortcuts, CLI Reference, About — navigate with Tab/1/2/3
- Landing page (`docs/index.html`) updated with install + completion setup instructions

### Quality
- 693 total tests (was 664), all passing
- 25 new tests (canary, deps, external scanner, man pages, TUI help)
- New modules: `canary.rs`, `deps.rs`, `man.rs`
- No new crate dependencies

## [0.5.3] - 2026-04-20

### Added — AI Safety Hardening

#### 3-Stage AI Guard Hooks
- `envforge ai-guard pre-tool TOOL INPUT` — Pre-tool scanning invoked by Claude Code/Cursor hooks
- **Sensitive file alerts**: Warns when AI agent accesses `.env`, `.pem`, `.ssh/`, `.aws/`, `credentials` files
- **Secret-in-command detection**: Catches known secret values in Bash command inputs
- **Post-tool output scanning**: Detects secrets leaked in tool output
- Safe file exclusions: `.env.schema`, `.env.example`, `.env.ai.md` don't trigger alerts
- Enhanced `envforge ai-hook install claude-code` now installs PreToolUse + PostToolUse hooks

#### Session Leases with Killswitch
- `envforge lease create --ttl 1h [--keys KEY1,KEY2] [--name SESSION]` — Time-bounded secret access
- `envforge lease list` — Show active leases with remaining time and key scope
- `envforge lease cleanup` — Remove expired/revoked leases
- `envforge revoke --all` — **KILLSWITCH**: Instantly revoke all active leases
- `envforge revoke --name SESSION` — Revoke specific lease
- Duration formats: `30m`, `1h`, `8h`, `24h`, `7d`
- Proxy integration: `envforge proxy --require-lease` enforces lease check per request

#### Proxy Domain Allowlist
- `envforge proxy --allow-origins api.stripe.com,api.openai.com` — Restrict which origins can call the proxy
- Default: localhost only (127.0.0.1, ::1)
- Denied origins logged to audit trail with 403 response

#### Secret Access Audit Log (JSONL)
- Every proxy request logged to `~/.config/envforge/access-audit.jsonl`
- Fields: timestamp, action, key accessed, client address, granted/denied
- **Values NEVER logged** — only key names and metadata
- `envforge audit --access` — View proxy access audit trail
- Append-only format for compliance

#### Sensitive File Access Alerts
- AI guard detects access to 14+ sensitive file patterns:
  `.env`, `.pem`, `.key`, `.p12`, `.ssh/`, `.aws/`, `.gnupg/`, `credentials`, `secret`, `token`, `id_rsa`, `id_ed25519`
- Integrated into PreToolUse hooks — alerts before AI agent reads sensitive files

### Quality
- 664 total tests (was 606), all passing
- 58 new tests across 5 features
- New modules: `ai_guard.rs`, `lease.rs`
- Enhanced: `proxy.rs` (audit + allowlist), `ai_hooks.rs` (3-stage)
- No new crate dependencies

## [0.5.2] - 2026-04-20

### Added — AI Safety Suite

#### MCP Config Hardening
- `envforge mcp harden` — Auto-rewrite MCP config files replacing plaintext secrets with `${VAR}` references
- Backs up originals as `.json.bak` before modifying
- `--dry-run` to preview changes without modifying files
- Covers Claude Desktop, Cursor, GitHub Copilot, project `.mcp.json`

#### Secret Fence
- `envforge fence` — Create AI tool ignore rules for all supported tools in one command
- Generates: `.envforgeignore`, `.cursorignore`, `.cursorrules`, `.github/copilot-instructions.md`, `.claude/settings.json`
- Idempotent: running twice doesn't duplicate rules
- `--dry-run` to preview files that would be created

#### AI Context Auto-Update
- `.env.ai.md` automatically regenerated when `envforge set`, `envforge import`, or `envforge secrets pull` modifies variables
- Only triggers when `.env.ai.md` already exists in project directory
- Keeps AI context file in sync with actual env configuration

#### Prompt Sanitizer
- `envforge sanitize FILE [--output FILE]` — Replace all known secret values in any file with `${KEY}` placeholders
- Longest-match-first replacement to avoid partial substitutions
- Skips values shorter than 4 characters to avoid false positives
- Works on any file type: code, configs, logs, docs

#### AI Leak Report
- `envforge audit --ai-leaks` — Scan git history for secrets leaked in AI-assisted commits
- Detects commits co-authored by Claude, Copilot, Cursor, and other AI tools
- Scans diffs for API key patterns, connection strings, and high-entropy tokens
- Reports: commit hash, date, AI tool, file path, leaked patterns

#### AI Coding Tool Hooks
- `envforge ai-hook install claude-code` — Install EnvForge security hooks in Claude Code
- `envforge ai-hook install cursor` — Add security rules to Cursor
- `envforge ai-hook remove claude-code|cursor` — Remove hooks
- Claude Code: PostToolUse hook scanning for secrets after Write/Edit operations
- Cursor: Rules file with secret safety instructions

#### Agent Credential Proxy
- `envforge proxy [--port 8100] [--keys KEY1,KEY2] [--profile NAME]` — Local HTTP proxy for AI agent credential access
- Endpoints: `GET /env` (all vars), `GET /env/KEY` (single), `GET /health`
- `--keys` restricts which secrets are served (scoped access)
- JSON responses with CORS headers for browser-based agents
- Secrets served via HTTP API — never written to disk files

### Quality
- 606 total tests (was 547), all passing
- 59 new tests across 7 AI safety features
- New modules: `fence.rs`, `sanitize.rs`, `ai_hooks.rs`, `proxy.rs`
- No new crate dependencies

## [0.5.1] - 2026-04-20

### Added

#### AI-Safe Schema Emission
- `envforge schema emit-ai [--output FILE] [--infer]` — Generate AI-agent-safe context file
- Contains variable names, types, descriptions, sensitivity flags — **NO actual values**
- AI coding tools get full context without seeing secrets
- `--infer` auto-detects types from current env values when no `.env.schema` exists
- Addresses GitGuardian 2026 finding: AI-assisted commits leak secrets at 2x baseline rate

#### MCP Configuration Scanning
- `envforge scan --mcp` — Scan AI tool config files for hardcoded credentials
- Scans: Claude Desktop, Cursor, GitHub Copilot, project `.mcp.json`
- Detects 23+ API key patterns (OpenAI, Stripe, AWS, GitHub, Slack, etc.)
- Detects connection strings with embedded passwords
- Shows masked values with fix suggestions: `→ Replace with: ${API_KEY}`
- `--json` output for CI integration

#### Docker Compose Secrets Export
- `envforge export --format docker-secrets` — Generate Docker `/run/secrets/` file structure
- Outputs shell script creating individual secret files + docker-compose.yml snippet
- Ready for Docker Compose `secrets:` mount configuration

#### Runtime Log Redaction
- `envforge run --redact` — Pipe subprocess stdout/stderr through secret redaction filter
- Automatically masks known sensitive values as `[REDACTED:KEY_NAME]`
- Runs in parallel threads for stdout and stderr (no performance penalty)
- Skips short values (<4 chars) to avoid false positives
- Combinable with `--volatile`, `--resolve`, `--profiles`

#### URI-Based Secret References
- `envforge resolve-uri FILE` — Resolve provider URIs in config files to actual values
- Supported URI schemes: `vault://`, `aws-ssm://`, `1password://`, `doppler://`, `infisical://`, `gcp://`, `azure://`
- Regular URLs (`https://`, `postgres://`) are NOT treated as secret URIs
- `--env` flag outputs `.env` format; default outputs shell `export` statements
- `--output FILE` writes resolved output to file
- Error per-URI without blocking other resolutions

### Quality
- 547 total tests (was 485), all passing
- 62 new tests across 5 features (21 MCP scan, 29 URI resolve, 3 AI schema, 4 docker-secrets, 5 redaction)
- New modules: `mcp_scan.rs`, `uri_resolve.rs`
- No new crate dependencies

## [0.5.0] - 2026-04-20

### Added

#### Pre-Commit Hook Integration
- `envforge scan --install-hook` — Auto-install git pre-commit hook running `envforge scan --staged`
- Appends to existing pre-commit hooks (doesn't overwrite)
- `envforge scan --remove-hook` — Clean removal of EnvForge hook lines
- Hook blocks commits when secrets detected (non-zero exit)

#### Shell Auto-Load Hook
- `eval "$(envforge hook zsh)"` — direnv-style auto-load on directory change
- Supports zsh (chpwd), bash (PROMPT_COMMAND), and fish (--on-variable PWD)
- Auto-detects `.envforge.toml` or `.env.schema` in directory
- Auto-unloads when leaving directory (restores previous env)
- `envforge env [--dir PATH]` — Output shell export statements for eval
- `.envforge.toml` project config with `profile` field

#### Volatile Mode (AI Agent Safety)
- `envforge run --volatile` — Secrets resolved in memory only, no disk I/O for secret values
- Forces `--resolve` mode automatically
- Ignores `--env-file` flags (no .env disk reads in volatile mode)
- `--dry-run` masks sensitive values as `****`
- Protects against AI agent file scanning (Claude Code, Cursor, Copilot)

#### Secure Secret Sharing
- `envforge share create --recipient <AGE_PUBKEY> [--keys|--all|--filter]` — Create encrypted share file
- `envforge share receive FILE [--import]` — Decrypt and import shared secrets
- Encrypted with recipient's age public key (not sender's)
- `--expire HOURS` — Soft expiry metadata (warns on receive after expiry)
- Self-contained `.age` file format with sender metadata

#### JSON Schema for `.env.schema`
- `envforge schema json-schema` — Output JSON Schema (Draft 2020-12) for `.env.schema` format
- Covers all fields: type, required, default, description, example, sensitive, pattern, values, min, max
- Environment override sub-objects supported
- Enables VSCode/JetBrains autocomplete and validation

#### Rotate with Propagation
- `envforge rotate KEY --propagate` — Auto-push to provider AND sync after local rotation
- No interactive prompts in propagate mode
- Graceful failure: provider/sync errors don't roll back local change
- Summary: `Rotated: local ✓, vault ✓, sync ✓`
- Works with `--stale --propagate` for bulk rotation

#### Git Author Audit Trail
- `envforge audit` — Per-variable change history with author attribution from sync git history
- `--key NAME` — Filter by variable name
- `--since DATE` — Filter changes after date
- `--machine ID` — Filter by machine
- `--json` — Machine-readable output
- Shows action type: added, modified, removed

#### Token TTL (Credential Expiry)
- `envforge secrets config PROVIDER --set key=value --ttl 8h` — Set credential expiry
- Duration formats: `8h`, `24h`, `7d`, `30d`
- Expired credentials rejected with clear message and renewal hint
- `envforge secrets status` shows TTL remaining per provider
- `envforge doctor` warns about credentials expiring within 24h
- TTL metadata stored in `_meta` sections of `credentials.toml`

#### Offline Fallback & Cache Management
- `envforge run --resolve` now falls back to stale cached values when provider unreachable
- Warning on stderr: "Using cached value for KEY (provider unreachable)"
- `envforge secrets cache list` — Show all cached secrets with provider, age, fresh/expired status
- `envforge secrets cache clear [--provider NAME]` — Clear cache (all or per-provider)

#### Multi-Profile Merge
- `envforge run --profiles dev,staging,custom` — Load and merge multiple profiles
- Left-to-right precedence (last profile wins on conflicts)
- Each profile overlaid on shared vars
- Compatible with `--env-file` and `--override` (highest precedence)
- Error if both `--profile` and `--profiles` used

### Quality
- 485 total tests (was 444), all passing
- 41 new tests across 10 features
- New modules: `hook.rs`, `share.rs`, `schema_json.rs`, `audit.rs`
- No new crate dependencies
- Clippy: 0 errors, 11 style warnings

## [0.4.3] - 2026-04-20

### Added

#### Unified Check (`envforge check`)
- `envforge check` — Single command running all health/safety checks: doctor, validate, scan, age, drift
- Grouped output by category with colored pass/fail/warning indicators
- Interactive fix hints per failure (`→ Run: envforge <fix command>`)
- `--only doctor,scan,age` — Run subset of categories
- `--json` — Machine-readable output for CI/CD
- Graceful skip when prerequisites missing (no schema → skip validate/drift)
- Non-zero exit on errors, zero on warnings-only

#### Encrypted Sync
- Sync snapshots now encrypted with age (X25519) before git commit
- Transparent auto-decrypt on `sync pull` — no extra flags needed
- Backward compatible: `read_snapshot` auto-detects encrypted vs plaintext
- `encrypted` config field in sync settings (default: `true` for new repos)
- Old unencrypted repos work without migration — next push encrypts automatically
- Clear error on key mismatch: "Cannot decrypt sync data. Key mismatch or corrupted."

#### Environment Snapshots (`envforge snapshot`)
- `envforge snapshot create [NAME]` — Capture active profile env state
- `envforge snapshot list` — Show all snapshots with name, date, variable count
- `envforge snapshot restore [NAME|--last]` — Restore with auto-backup before write
- `envforge snapshot diff [NAME|--last]` — Color-coded diff (added/removed/changed)
- `envforge snapshot delete NAME` — Remove snapshot with confirmation
- Auto-prune: keeps last 20 snapshots, oldest pruned on create
- Snapshots stored in `~/.config/envforge/snapshots/` as TOML

#### Key Explain (`envforge explain`)
- `envforge explain KEY` — Unified X-ray showing all known info about a key
- **Source**: file path, line number, export style, active/commented status
- **Profile**: which profile defines it (shared/profile-specific)
- **Schema**: type, required, description, sensitive flag (if `.env.schema` exists)
- **Encryption**: plaintext or encrypted status
- **Reference**: secret reference provider/path (if `ref:` value)
- **Sync**: synced/local/untracked status
- **Age**: days since last pull, stale flag
- `--json` — Full structured JSON output
- Similar key suggestions when key not found (substring + Levenshtein matching)

#### Secret Rotation (`envforge rotate`)
- `envforge rotate KEY` — Interactive 3-step flow: show masked current → prompt new → confirm
- Masked value display (`sk-ab****56` format)
- Atomic local update with backup before write
- Auto-resets secret age to 0 after rotation
- Logs rotation event to changelog
- Optional provider push: `Push to vault? [y/N]`
- Optional sync push: `Push to sync? [y/N]`
- `--dry-run` — Preview rotation plan without changes
- `--stale` — Bulk rotate all secrets older than 90 days (rotate/skip/quit per key)
- Handles encrypted values transparently (re-encrypts new value)

### Quality
- 444 total tests (was 398), all passing
- 46 new tests across 5 features
- No new crate dependencies
- Clippy clean

## [0.4.2] - 2026-04-20

### Added

#### Multi-Format Export
- `envforge export --format <fmt>` — Export env vars in 7 formats: `dotenv`, `json`, `yaml`, `toml`, `docker`, `k8s`, `tfvars`
- **JSON** — `{"KEY": "VALUE"}` object, valid JSON
- **YAML** — Properly quoted booleans (`true`/`false`), numbers, and YAML-special values (`null`, `~`, `yes`, `no`)
- **TOML** — All values quoted, backslash and quote escaping
- **Docker** — Bare `KEY=VALUE` for `--env-file`, no quotes or comments
- **Kubernetes Secret** — Full manifest with base64-encoded `data:` section
  - `--k8s-name NAME` — Set Secret name (default: `envforge-secrets`)
  - `--k8s-namespace NS` — Set namespace (default: `default`)
- **Terraform tfvars** — Lowercase keys, quoted values
- Works with existing `--filter` flag

#### Secret Age Tracking
- `envforge secrets age [--threshold DAYS] [--stale-only]` — Show age of all tracked secrets
- Automatically tracks when secrets are pulled from providers
- Flags stale secrets exceeding threshold (default: 90 days)
- Color-coded output: green ✓ for fresh, red ⚠ for stale
- `--json` output with age_days, stale flag, provider info
- Persistent tracking in `~/.config/envforge/secret-sources.toml`

#### Provider Diff
- `envforge secrets diff --from <provider> --path <path> [--filter PATTERN]` — Compare local ENV vs provider state
- Shows 4 categories: same, changed, only-local, only-remote
- Color-coded diff output with truncated values
- `--json` output for CI/CD integration
- Works with all 7 secret providers

#### GitHub Action (`action/`)
- **Composite GitHub Action** for CI/CD integration — manage ENV vars and secrets in pipelines
- 5 operation modes: `validate`, `secrets-pull`, `export`, `run`, `drift`
- **validate** — Check `.env` files against `.env.schema`, fail step on validation errors
- **secrets-pull** — Pull secrets from any of 7 providers (Vault, AWS SSM, 1Password, Doppler, Infisical, GCP, Azure) into `GITHUB_ENV`
- **export** — Export EnvForge-managed variables into workflow environment
- **run** — Execute commands with process-scoped secret injection (secrets never persist in `GITHUB_ENV`)
- **drift** — Detect configuration drift across `.env` files in PRs
- Automatic binary installation from GitHub Releases (Linux x86_64/aarch64, macOS x86_64/aarch64)
- Version pinning (`version` input) or auto-resolve latest release
- Value masking in GitHub Actions logs by default (`mask-values: true`)
- Multiline-safe `GITHUB_ENV` export using heredoc delimiters
- All 4 outputs wired: `variables`, `count`, `validation-result`, `drift-result`
- Example workflow (`.github/workflows/envforge-example.yml`)
- Comprehensive test suite (`action/tests/test_action.sh`) — 21 tests covering syntax, input validation, integration, structure, and permissions

#### Usage
```yaml
# Validate .env on PRs
- uses: emreerinc/envforge/action@v1
  with:
    mode: validate
    schema: .env.schema
    env-file: .env

# Pull secrets from AWS SSM
- uses: emreerinc/envforge/action@v1
  with:
    mode: secrets-pull
    provider: aws-ssm
    provider-path: /myapp/production

# Run tests with injected secrets
- uses: emreerinc/envforge/action@v1
  with:
    mode: run
    command: cargo test
    resolve-secrets: 'true'
```

### Quality
- 398 total tests (was 357), all passing
- 27 new Tier 1 feature tests (export formats, secret age, edge cases)
- 21 action tests, all passing
- Shell scripts pass `bash -n` syntax validation
- Action outputs correctly wired to composite step

## [0.4.0] - 2026-04-17

### Added

#### Subprocess Runner (`envforge run`)
- `envforge run [flags] -- <command> [args]` — Run any command with injected ENV vars
- `--profile NAME` — Use a specific profile without switching the active profile
- `--resolve` — Decrypt `ENC[age:...]` values and resolve `ref:` secret references at runtime
- `--env-file PATH` — Load additional .env file(s), repeatable, override order preserved
- `--override KEY=VALUE` — Override specific variables, repeatable
- `--dry-run` — Preview injected ENV vars without executing the command
- Exit code and signal passthrough from child process
- ENV merge order: shell env → shared file → profile file → env-file(s) → overrides

#### ENV Schema (`.env.schema`)
- `.env.schema` TOML format with per-variable type, required, default, description, example, sensitive, pattern, values, min, max
- Type system: `string`, `number`, `bool`, `url`, `email`, `enum`, `regex`, `port`
- Environment-specific overrides: `[VARIABLE.production]` sections
- `envforge validate --schema PATH [--env PATH] [--environment NAME]` — Schema-based validation with CI-friendly exit codes
- Schema and `config.toml` `[validation]` rules merged (schema takes priority)
- `envforge schema generate [--output PATH]` — Auto-generate schema from existing ENV with type heuristics
- `envforge docs --schema PATH [--output PATH]` — Generate Markdown documentation table
- `envforge drift --envs FILE1 FILE2 ...` — Multi-environment drift detection with color-coded matrix
- `envforge init --schema PATH [--output PATH]` — Interactive onboarding wizard for new developers

#### AI Agent Safety
- `envforge export --safe` — Export with sensitive values redacted as `[REDACTED]`
- `envforge export --env-example` — Generate `.env.example` from schema with placeholder values
- `.envforgeignore` convention — List files AI tools should not read (`.gitignore` syntax)
- `envforge doctor` AI safety check — Warns when `.env` exists without `.envforgeignore`

#### Git Merge Driver
- `envforge git install-merge-driver` — Register semantic `.env` merge driver in git config
- Three-way merge with key=value understanding: auto-merges non-conflicting changes
- Per-key conflict markers for true conflicts (same key changed on both sides)
- `envforge git remove-merge-driver` — Clean uninstall

#### Health Check (`envforge doctor`)
- 10 health checks: config, encryption key, shell files, duplicates, validation, references, AI safety, sync, providers, credentials
- Every warning includes an actionable fix suggestion (cyan `→` hint)
- `--verbose` for detailed output, `--json` for machine-readable output

#### Other
- `envforge secrets resolve [--key KEY]` — Output `export KEY='value'` for shell init (`eval "$(envforge secrets resolve)"`)
- `envforge profile diff A B` — Side-by-side profile comparison with color-coded output

### Fixed

#### Secret Manager Providers (all 7 validated against official CLI docs)
- **AWS SSM**: Added pagination loop for `get-parameters-by-path` (>10 results), added `--recursive` flag
- **Doppler**: Filter system keys (`DOPPLER_PROJECT`, `DOPPLER_CONFIG`, `DOPPLER_ENVIRONMENT`) from pull results, batch push into single CLI call
- **Infisical**: Changed pull from invalid `secrets get --plain --format=json` to correct `export --format=json`, fixed set syntax from `set KEY VALUE` to `set KEY=VALUE`
- **Azure Key Vault**: Added underscore-to-hyphen name mapping (`DB_HOST` → `DB-HOST` on push, reverse on pull)
- **Vault**: Fixed `kv list` parsing to read `data.keys` instead of bare array
- **1Password**: Fixed `--fields` flag to `--fields=label=KEY` per official docs
- **GCP**: Handle empty `secrets list` output (empty string instead of `[]`)

### Quality
- 357 total tests (was 256), all passing
- 59 new provider validation tests covering all 7 providers
- 42 new feature tests (schema, run, safe export, doctor, drift, merge)
- Clippy clean, rustfmt clean
- No new crate dependencies

## [0.3.0] - 2026-04-15

### Added

#### Secret Manager Integration
- **Provider pattern** with `SecretProvider` trait — extensible for any secret manager
- 7 built-in providers: HashiCorp Vault, AWS SSM, 1Password, Doppler, Infisical, GCP Secret Manager, Azure Key Vault
- `envforge secrets pull --from <provider>` — Pull secrets from any provider
- `envforge secrets push --to <provider> [--keys|--all|--filter]` — Push secrets to provider
- `envforge secrets ref <KEY> --from <provider> --path <path>` — Create lazy references
- `envforge secrets unref <KEY>` — Remove a reference
- `envforge secrets resolve` — Resolve all references (with TTL cache + offline fallback)
- `envforge secrets config <provider> --set key=value` — Store credentials (age encrypted)
- `envforge secrets providers` — List all providers with binary and config status
- `envforge secrets status` — Show configured providers
- **Credential store** — Provider credentials encrypted with age in `credentials.toml`
- **Reference cache** — TTL-based caching for resolved secrets, offline fallback
- **authenticate()** and **validate_config()** on SecretProvider trait
- **Source tracking** — Pulled keys track which provider they came from
- **Progress indicator** — Shown for large pull/push operations (>50 keys)
- Vault AppRole authentication support
- AWS profile and IAM role authentication support
- "Did you mean?" suggestion for misspelled provider names

### Quality
- 256 total tests, all passing
- Clippy clean, rustfmt clean

## [0.2.0] - 2026-04-15

### Added

#### Remote Sync
- **Git-based sync** to share ENV variables across machines via any Git remote
- `envforge sync init [--remote URL] [--machine-id ID] [--force]` — Initialize sync repository
- `envforge sync push [-m MSG] [--dry-run]` — Export marked keys, commit, and push
- `envforge sync pull [--dry-run]` — Pull remote snapshot and apply changes
- `envforge sync status` — Show diff between local state and sync snapshot
- `envforge sync mark <KEY|PATTERN> --sync|--local [--all]` — Select which keys to sync
- `envforge sync list-keys` — View all keys with sync/local status
- `envforge sync override <KEY> <VALUE> [--remove] [--list]` — Machine-specific overrides
- `envforge sync history [-n N]` — View snapshot commit history
- `envforge sync rollback <COMMIT|--last>` — Restore previous snapshot with auto-backup
- `envforge sync log [-n N]` — View sync operation log
- `envforge sync machine` — Show machine identity and override count
- Selective sync with glob pattern support (`AWS_*`, `DB_?`)
- Auto-generated machine identity (`{hostname}-{hex}`) per machine
- Machine overrides — per-machine values that take precedence over shared snapshot
- Three-way conflict detection when both local and remote modify the same key
- Conflict resolution strategies: `keep-local`, `keep-remote`, `manual-edit`, with configurable default
- Offline-first design — all operations work locally, remote sync is optional
- Sync history and rollback via Git commit history
- Sync operation log (local, append-only, auto-rotated at 100 entries)
- JSON output (`--json`) and dry-run (`--dry-run`) for all sync commands

### Changed

- `envforge sync mark` now requires explicit `--sync` or `--local` flag (no silent defaults)
- `--all` flag no longer requires a key argument

### Dependencies

- Added `hostname` (0.4) for machine ID generation
- Added `rand` (0.9) for random hex suffix

### Quality

- 115 sync-specific tests (82 unit + 33 integration)
- 240 total project tests, all passing
- Clippy clean (`-D warnings`), rustfmt clean

## [0.1.0] - 2026-04-12

### Added

#### Core
- Shell file parser with line-level AST and byte-for-byte round-trip fidelity
- Support for `.zshrc`, `.bashrc`, `.bash_profile`, `.profile`, `.zprofile`
- Auto-detection of shell type from `$SHELL`
- Soft-delete with envforge tags (never physically removes content)
- Atomic file writes (tempfile + rename)
- SHA-256 hash verification before writes
- Automatic backup before every write (last 10 retained)
- Protected zone management (header/footer offsets, conda/Amazon Q block detection)
- Reference file strategy (`~/.env_managed` with source injection)
- Conflict detection (hash-based external change detection)

#### TUI Interface
- Table view with KEY / VALUE / LOCATION columns
- Keyboard navigation (vim-style j/k, search with /)
- Mouse support (click to select, scroll to navigate)
- Value masking for sensitive keys (SECRET, TOKEN, PASSWORD, CREDENTIAL, KEY)
- Edit popup, add dialog, confirm dialogs, diff preview
- Fuzzy search with match character highlighting
- ENV grouping — auto-detected prefixes + user-defined groups in config
- Collapsible group headers (arrow keys to expand/collapse)
- Active/passive toggle with Space key (visual status box)
- Full in-session undo history (`u` key)
- Profile switching with `P` key selector popup
- Profile badge in header
- Import (`I`) and export (`E`) from TUI
- Status bar with unsaved indicator, undo count, notifications
- Help screen (`?`)
- Save with `S` or `Ctrl+S`

#### CLI Interface
- `envforge list` — list all variables (with `--json`)
- `envforge get KEY` — get a value
- `envforge set KEY=VALUE` — create or update
- `envforge delete KEY` — soft-delete
- `envforge copy KEY` — copy to clipboard
- `envforge move KEY` — move to reference file
- `envforge import <file>` — import from .env file
- `envforge export [path]` — export to .env format
- `envforge duplicates` — detect duplicate keys
- `envforge scan [path]` — scan for leaked secrets
- `envforge validate` — validate ENV values against config rules
- `envforge encrypt KEY` — encrypt value with age
- `envforge decrypt KEY` — decrypt value
- `envforge profile list|switch|create|delete` — manage profiles
- `envforge log [KEY]` — view change history
- `envforge completions zsh|bash|fish` — generate shell completions
- `envforge diff` — show pending changes
- `envforge config` — show configuration
- `envforge --dry-run` — preview without writing
- First-run setup wizard

#### Profiles
- Multiple environment profiles (dev/staging/prod)
- Shared ENV file (`~/.env_managed.shared`) always sourced
- Profile-specific files (`~/.env_managed.{name}`)
- Precedence: profile > shared > shell config
- Last used profile remembered across sessions
- Migration from single reference file to profile-based

#### Security
- ENV encryption at rest using age (X25519)
- Auto-generated age keypair with 0600 permissions
- Secret scanning — detect leaked values in source code
- Git staged file scanning (`--staged` flag for pre-commit hooks)
- Sensitive value masking in TUI and changelog

#### Quality
- ENV validation rules in config (url, number, bool, email, nonempty, regex)
- Automatic changelog on save
- Log viewer with key filtering
- 125 automated tests
- CI pipeline (GitHub Actions: Linux + macOS matrix)
- Release pipeline (multi-platform binaries)
