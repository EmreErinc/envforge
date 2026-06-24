# `.envforge.project.toml` — Project Manifest

The project manifest declares which env files belong to a project and how they
group into environments. It is the **source of truth for env-file recognition**
in the EnvForge LSP (Epic 1). The file lives at the project root and may also be
written as `.envforge.project.yaml` / `.yml` / `.json` (same schema, different
encoding — see `ConfigFormat`).

> The manifest type, parser, and detection already exist in
> `src/ops/project/config.rs` (`ProjectConfig`, `detect_project_config`,
> `load_project_config`). This document is the published schema reference; the
> LSP env-recognition layer reuses it via `src/ops/project/resolve.rs`.

## Schema

```toml
[project]
name = "my-service"                 # project name
schema_path = ".env.schema"         # path to the .env.schema (key contract)
active_environment = "development"  # which environment is "active"

[[environments]]                    # one entry per logical environment
name = "development"
env_file = ".env.development"
# description = "Local dev"         # optional

[[environments]]
name = "stage"
env_file = ".env.stage"

[[environments]]
name = "production"
env_file = ".env.production"

[ai_guard]                          # optional: hardening / scanner config (unrelated to recognition)
```

- `[[environments]]` is the env-file set. Each `env_file` is resolved relative
  to the project root.
- There is no `base`/`profiles` split and no `schema_version` field; the flat
  `environments` list covers both the base and per-environment files.

## Recognition rules (LSP)

Implemented by `resolve_env_set` + `ResolvedEnvSet::recognizes`
(`src/ops/project/resolve.rs`):

1. Each `env_file` is joined onto the project root and lexically normalized.
2. An `env_file` that is **absolute** or **escapes the root** via `..` is
   **dropped** — the LSP never recognizes a file outside the workspace.
3. Duplicate resolved paths collapse (first declaration wins).
4. The LSP recognizes a file if it is a conventional `.env*` file **or** is in
   the resolved set. Conventional `.env*` files (including profile variants like
   `.env.development`, `.env.stage`, `.env.production`) are always recognized,
   so the manifest is only required to recognize **non-`.env*`** names (e.g.
   `config/app.env` is covered by the `*.env` rule; a custom name like
   `settings.env.dev` needs a manifest entry).
5. **No manifest present →** recognition falls back to the conventional `.env*`
   set (backward compatible).

## Lifecycle

- Loaded on LSP `initialized`; re-resolved live on save of the manifest
  (no restart).
- A malformed manifest publishes an `envforge-project` diagnostic on the
  manifest file and the **last successfully resolved set is retained** — a typo
  never silently disables recognition.

## Precedence: manifest vs `.env.schema`

The manifest and the schema are **orthogonal layers**:

| Concern | Authority |
|---|---|
| Which files are recognized env files (the file set) | **Manifest** (`[[environments]]`) — the schema cannot widen the recognized set |
| Key contract — types, required-ness, allowed values, sensitivity | **`.env.schema`** (referenced via `project.schema_path`) |
| Value sensitivity for redaction | **Union** — sensitive if the schema marks it sensitive OR `is_sensitive_key(key)` |

- With a manifest but no schema: files are recognized and the key-set is
  unified, but keys are untyped.
- With a schema but no manifest: conventional `.env*` recognition applies and
  the schema types keys as today.
- A schema that references a file **not** in the manifest does not make that
  file a recognized env file — the manifest is authoritative for the file set.
