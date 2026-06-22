# Deferred Config-Format Support — Implementation Plan

**Author:** Emre · **Date:** 2026-06-21 · **Status:** plan (not yet scheduled)
**Predecessor:** intent 036 (framework config files — Phase 1: properties/.env full, YAML read-only, AI-safety parity). See `docs/prd.md`, `docs/epics.md`.

## Purpose

Intent 036 shipped the `ConfigFormat` seam (`parse` / `resolve` / `write_capability`) and proved it extensible by adding YAML without touching properties handlers. This plan covers the four capabilities deferred from the 036 PRD (Growth/Vision), each of which now plugs into that seam incrementally rather than as a rewrite:

1. **YAML writes** — upgrade YAML from `ReadOnly` → `ReadWrite` (rename/format) round-trip-safe.
2. **TOML family** — `Cargo.toml`, `pyproject.toml`, `config.toml` (new full-feature format).
3. **.NET `appsettings.json`** — JSONC + `appsettings.{Environment}.json` cascade.
4. **Cross-format schema unification** — one `.env.schema` validating keys regardless of file format.

## The unlock: round-trip is now a solved problem (crate research, 2026-06)

The original 036 deferral reason — "no comment-preserving Rust YAML crate" — is no longer blocking. Two facts changed the plan:

- **Format-preserving editors exist and are mature** for TOML (`toml_edit` 0.25, gold standard, format-preserving `Document`) and JSON/JSONC (`jsonc-parser` 0.32, mutable CST preserving comments + trailing commas — exact fit for `appsettings.json`).
- **Surgical span-edit makes round-trip safety structural, not crate-dependent.** Instead of parse→mutate→re-serialize (which risks reformatting), locate the target value's **byte range** and splice only that range — every other byte is untouched, so the byte-for-byte invariant holds *by construction*. `yamlpath`+`yamlpatch` 1.26 (tree-sitter-yaml, from the `zizmor` project) productionizes exactly this for YAML; `tree-sitter` grammars cover all three formats as a uniform fallback. Unit 002's YAML parser already captures the spans (`Marker`), so the data needed for surgical edits is largely in hand.

**Decision:** introduce a shared **`SurgicalEdit`** utility (locate byte-range → splice) as the uniform write mechanism. Dedicated editors (`toml_edit`, `jsonc-parser`) are used where their lossless mutate-and-`to_string()` APIs are solid; YAML uses surgical patching. This keeps "only changed bytes change" true for every format and satisfies CLAUDE.md principle #1.

### Crate decisions (verified current)

| Format | Approach | Crate | Write-capability |
|---|---|---|---|
| TOML | format-preserving CST | `toml_edit` 0.25 (mature, Cargo uses it) | ReadWrite |
| JSON/JSONC | mutable CST (comments + trailing commas) | `jsonc-parser` 0.32 (dprint) | ReadWrite |
| YAML | surgical span-patch | `yamlpatch`/`yamlpath` 1.26 (tree-sitter-yaml) | ReadWrite (upgrade) |
| (shared) | byte-range splice fallback | `tree-sitter` + grammars, or hand-rolled over existing spans | — |

Avoid: `serde_yaml` (dead/RUSTSEC), `serde_yml` (deprecation shim, unsound FFI), `noyalib`/`saphyr` comment-preservation (not shipped / too immature for a hard invariant).

---

## Roadmap (recommended sequence)

Ordered by value × (inverse) risk. Each is an independent specsmd intent; each ships standalone.

| # | Intent | Capability | Risk | Effort | Why this order |
|---|--------|-----------|------|--------|----------------|
| 1 | **037 — TOML support** | `Cargo.toml`/`pyproject.toml`/`config.toml`, full features | Low | M | Lowest-risk format (toml_edit is gold standard); proves a 2nd full ReadWrite format through the seam; high value (Rust + Python) |
| 2 | **038 — YAML writes** | upgrade YAML to ReadWrite (rename/format) | Medium | M | Completes an already-shipped format; surgical-patch tech now de-risked; reuses 038's `SurgicalEdit` util |
| 3 | **039 — .NET appsettings.json** | JSONC + `appsettings.{Env}.json` cascade + `__`→`:` | Medium | M–L | Adds JSON family + a new cascade convention; JSONC CST mature |
| 4 | **040 — Cross-format schema unification** | one `.env.schema` across all formats | Low–Med | M | Capstone — needs ≥2 formats present; pure-ops, high payoff for diagnostics/go-to-def |

Foundational note: the **`SurgicalEdit`** utility (intent 038, but extract early) and any shared cascade-resolution generalization are cross-intent; build once, reuse.

---

## Intent 037 — TOML Support

**Goal:** Full IDE language features (hover/completion/go-to-def/refs/highlight/diagnostics/rename/format) on `Cargo.toml`, `pyproject.toml`, and `config.toml`, round-trip-safe via `toml_edit`.

**ConfigFormat integration:** new `TomlFormat: ConfigFormat`, `WriteCapability::ReadWrite`. Recognition predicate `is_toml_config_file` scoped to known names (`Cargo.toml`, `pyproject.toml`, `config.toml`, `.cargo/config.toml`) — NOT every `*.toml` (avoid the FR3 over-broad lesson).

**Units / stories (outline):**
- **U1 recognition + TomlFormat** — predicate (scoped), `ConfigFormat` impl, routing alongside existing predicates (no regression).
- **U2 parse → positioned entry model** — `toml_edit` document → `ConfigEntry` with dotted-path keys (`[table].key`), UTF-16 positions from `toml_edit` spans. Round-trip byte-identity.
- **U3 read features** — hover (effective value + table path + schema), completion (keys + `${}` if interpolation applies — TOML has no native interpolation, so `${}` only where a value references env), go-to-def, refs, semantic tokens.
- **U4 diagnostics** — duplicate key, type-mismatch-vs-schema, unknown key.
- **U5 write features** — rename/format via `toml_edit` lossless mutate; AND/OR shared `SurgicalEdit`. Byte-for-byte except the change.

**Risks:** nested tables/arrays-of-tables → dotted-path flattening must be unambiguous (`[a.b]` vs `a.b =`); `toml_edit` handles formatting but confirm our `ConfigEntry` model maps cleanly. Pyproject has no env-style keys — value is in navigation/validation, not interpolation.

**Effort:** M. New crate `toml_edit`. Mostly mirrors the properties unit shape.

---

## Intent 038 — YAML Writes (ReadOnly → ReadWrite)

**Goal:** Enable rename + format on `application.yml`/`.yaml` (and any future YAML config) without losing comments/formatting — lifting the 036 read-only restriction.

**Approach:** **surgical span-patch.** Build a shared `SurgicalEdit` utility: given a `ConfigEntry`'s byte range, produce a `TextEdit`/file write that replaces only that range. Use `yamlpath`/`yamlpatch` (or `tree-sitter-yaml` directly) to resolve a key's value span precisely (including block scalars, flow style, quoted values). No whole-document re-serialization.

**Units / stories (outline):**
- **U1 SurgicalEdit foundation** — shared util (locate span → splice), with a byte-identity property test harness; crate-independent. (Extract here; reused by TOML/JSON if desired.)
- **U2 YAML value-span resolution** — map a `ConfigEntry` (dotted path) → exact value byte-span via yamlpath/tree-sitter, covering nested maps, flow `{a: b}`, block scalars `|`/`>`, quoted/anchored values.
- **U3 flip YamlFormat to ReadWrite + rename** — `write_capability()` → ReadWrite; `config_rename` produces surgical edits across base + profile YAML; preserves comments/indent.
- **U4 format** — formatting is conservative/no-op unless safe (YAML formatting is opinionated; default to "no reformat", only normalize what's provably safe, or omit format and keep rename only). Decide: rename-yes, format-maybe.
- **U5 round-trip + write-guard inversion tests** — replace 036's "YAML write path unreachable" assertions with "YAML write path is surgical + byte-identical except the edit"; fuzz over a real Spring YAML corpus.

**Risks:** YAML's many value styles (block scalars, anchors/aliases, flow) make span resolution the hard part — `yamlpatch` handles most; anchors/aliases may be out-of-scope (document). Renaming a key used via an alias is ambiguous — define behavior. Multi-document YAML (`---`) edge cases.

**Effort:** M. New crate `yamlpatch`/`yamlpath` (+ `tree-sitter-yaml`). Inverts a 036 constraint, so update PRD/CHANGELOG/integration-matrix (YAML rename: — → ✓).

---

## Intent 039 — .NET `appsettings.json` (JSONC)

**Goal:** Language features on `appsettings.json` + `appsettings.{Environment}.json`, round-trip-safe via `jsonc-parser` CST (preserves comments + trailing commas).

**ConfigFormat integration:** `JsoncFormat: ConfigFormat`, `WriteCapability::ReadWrite`. Recognition: `appsettings.json`, `appsettings.{env}.json` (scoped, not every `.json` — avoid stealing `package.json`/`tsconfig.json`/`mcp.json`; note `mcp.json` already has its own handler — must not collide).

**Units / stories (outline):**
- **U1 recognition + JsoncFormat + cascade** — predicate scoped to `appsettings*`; `.NET` environment cascade convention (`appsettings.json` < `appsettings.{Environment}.json`, env from `ASPNETCORE_ENVIRONMENT`); ensure no collision with `is_mcp_config_file`.
- **U2 JSONC parse → entry model** — `jsonc-parser` CST → `ConfigEntry`; nested objects → `:`-joined paths (the .NET convention, e.g. `Logging:LogLevel:Default`); UTF-16 spans.
- **U3 env-var mapping** — model the `__`→`:` environment-variable binding (`Logging__LogLevel__Default`) so go-to-def/refs link env vars to JSON keys.
- **U4 read features + diagnostics** — hover/completion/goto/refs/highlight; duplicate key, unknown-vs-schema.
- **U5 write features** — rename/format via `jsonc-parser` mutable CST (or SurgicalEdit), lossless.

**Risks:** JSON key path separator is `:` in .NET but our existing dotted-path model uses `.` — need a per-format path separator in `ConfigEntry`/resolution (small generalization). `appsettings.json` arrays (`"Urls": [...]`) — index addressing. Collision-avoidance with mcp.json/other JSON.

**Effort:** M–L (the `__`→`:` mapping + path-separator generalization add work). New crate `jsonc-parser`.

---

## Intent 040 — Cross-Format Schema Unification

**Goal:** One `.env.schema` is the single source of truth for key metadata (type, sensitivity, required, description) regardless of which file format a key lives in — so diagnostics, hover metadata, and go-to-def-to-schema work uniformly across `.env`, properties, YAML, TOML, JSON.

**Approach (pure ops — no new format):** generalize the schema-validation + `schema_line_map` linkage (already reused in 036) so any `ConfigFormat`'s entries validate against and link to the shared schema. Add cross-format reference resolution: a key `DATABASE_URL` referenced in `application.yml` resolves to its `.env.schema` definition and to its concrete value wherever defined (`.env`, properties, etc.).

**Units / stories (outline):**
- **U1 unified schema model** — schema keys are format-agnostic; map dotted/`:`-path keys ↔ schema entries (normalization rules: relaxed binding for Spring `spring.datasource.url` ↔ `SPRING_DATASOURCE_URL`).
- **U2 cross-format diagnostics** — unknown-key, type-mismatch, missing-required computed against the unified schema for every format.
- **U3 cross-format go-to-def / refs** — from a key in any format → schema def AND all concrete definitions across formats.
- **U4 relaxed-binding equivalence** — Spring relaxed binding (`my.key` ≡ `MY_KEY` ≡ `my-key`) recognized so the same logical key unifies across `.env` (UPPER_SNAKE) and `application.yml` (dotted).

**Risks:** relaxed-binding normalization is fiddly and Spring-specific — scope it explicitly. Performance: cross-format resolution must reuse caches (NFR3-style) not re-scan on every keystroke.

**Effort:** M. Pure `ops/`; no new crate. Highest "feels magical" payoff (the PRD's "developers think in configuration, not formats" thesis fully realized).

---

## Cross-cutting requirements (apply to every intent above)

- **Round-trip:** byte-for-byte on every write (surgical or lossless-CST); each unit ships a byte-identity test over a rich fixture + a real-world corpus.
- **UTF-16 positions:** every format converts spans to UTF-16 code units (the 036 bug class — reuse `char_col_to_utf16_col`/`byte_offset_to_utf16_col`).
- **AI-safety parity:** new file types registered in fence/redaction/exposure/canary (`is_config_canary_target`, `compute_config_exposure_map`, fence predicates) — same as 036 unit 003.
- **No regression / scoped recognition:** predicates exact-match known config names (no over-broad `*.toml`/`*.json`/`*.yml`); routing added alongside existing predicates; full suite stays green.
- **Tests in `tests/`**, insta + tempfile; no in-module `mod tests`; update CHANGELOG/README counts (docs-sync).
- **Adversarial review per unit + a whole-feature integration review** before marking each intent done (lesson from 036: per-unit review misses cross-cutting regressions; the integration review caught the `.env` routing regression).

## Sequencing & dependencies

```
037 TOML ──┐
038 YAML-writes (extract SurgicalEdit) ──┐
039 .NET JSONC (path-separator generalization) ──┤
                                                  ├──► 040 Schema Unification (needs ≥2 formats)
```

- 040 depends on having multiple formats live (do last).
- `SurgicalEdit` util: build in 038, reuse in 039 (and retrofit 037 if desired).
- Per-format path separator (`.` vs `:`) generalization: introduced in 039, but design the `ConfigEntry` key model for it when starting 037 to avoid rework.

## Open decisions (resolve at each intent's inception)

1. **YAML format (not rename):** ship conservative format, or rename-only? (YAML formatting is opinionated; recommend rename-only first.)
2. **TOML scope:** just the 3 canonical names, or user-configurable list?
3. **`.json` collision policy:** explicit allowlist of JSON config names; confirm precedence vs `is_mcp_config_file`.
4. **Relaxed-binding scope (040):** Spring-only, or general normalization?

## Suggested next step

Scaffold **intent 037 (TOML)** as a specsmd intent (requirements / system-context / units / inception-log + unit briefs + stories), mirroring the 036 structure, then run inception review → bolt planning → construction with the per-unit + whole-feature review discipline.
