# EnvForge Release Preparation Guide

This guide is the **Operations phase** playbook for shipping EnvForge. It combines the AI-DLC release workflow with the automated [`release-checker`](../../.agents/skills/release-checker/SKILL.md) skill.

For SemVer policy and stability guarantees, see [VERSIONING.md](../VERSIONING.md).

---

## Quick Start

| Goal | Command / Trigger |
|------|-------------------|
| Automated release (recommended) | Tell the agent: **"prepare release"**, **"cut release vX.Y.Z"**, or **`/release`** |
| Manual release | Follow [Phase-by-Phase Checklist](#phase-by-phase-checklist) below |
| Operations agent (AI-DLC) | `/specsmd-operations-agent` after construction is complete |

The release-checker skill runs all readiness checks, bumps version, updates the changelog, commits (with your approval), tags, and optionally pushes.

---

## AI-DLC Context

EnvForge release maps to the **Operations** phase of AI-DLC:

```text
Inception ──► Construction ──► Operations (you are here)
   Plan           Build              Ship & verify
```

| AI-DLC Step | EnvForge Equivalent |
|-------------|---------------------|
| Build artifacts | `cargo build --release`, VSCode `.vsix`, IntelliJ plugin zip |
| Verify | fmt, clippy, test, audit, changelog, git state |
| Deploy | Tag push → GitHub Actions → GitHub Releases |
| Monitor | Post-release smoke tests, issue triage, crates.io publish |

**Master Agent routing:** `/specsmd-master-agent` → analyze state → route to Operations when code is ready to ship.

---

## Current Snapshot (update before each release)

Run these commands to refresh the snapshot section mentally or in your release notes:

```bash
grep '^version' Cargo.toml
git describe --tags --abbrev=0
git log $(git describe --tags --abbrev=0)..HEAD --oneline | wc -l
git branch --show-current
git status --porcelain
```

| Field | How to interpret |
|-------|------------------|
| `Cargo.toml` version | Target release version (must match tag without `v`) |
| Last tag | Previous shipped release |
| Commits since tag | Scope of this release |
| Branch | Must be `main` for release-checker (merge release branch first) |
| Dirty tree | Must be clean before tagging |

---

## Phase-by-Phase Checklist

### Phase 0 — Pre-Release (Human)

Complete before invoking release-checker or cutting a tag.

- [ ] All feature work merged to `main` (or release branch merged to `main`)
- [ ] [CHANGELOG.md](../CHANGELOG.md) has an `## [Unreleased]` section **or** a dated `## [X.Y.Z] - YYYY-MM-DD` entry
- [ ] Unreleased changes categorized: `Added`, `Changed`, `Fixed`, `Security`, `Removed`, `Deprecated`
- [ ] Breaking changes documented (required for major / post-1.0 minor with deprecations)
- [ ] Security-sensitive changes reviewed (fence, MCP hardening, secret providers)
- [ ] Open release-blocking issues closed or explicitly deferred

**EnvForge-specific:**

- [ ] Run `envforge scan --staged` before any commit that touches env-related files
- [ ] No secrets hardcoded; `.env` files not committed
- [ ] Editor extensions tested locally if UI/LSP behavior changed (`editors/vscode`, `editors/intellij`)

---

### Phase 1 — Determine Version

Release-checker automates this; manual equivalent:

1. Read `version` in [Cargo.toml](../Cargo.toml)
2. Last tag: `git describe --tags --abbrev=0`
3. Commits since tag: `git log <last-tag>..HEAD --oneline`
4. Choose SemVer (`MAJOR.MINOR.PATCH`) per [VERSIONING.md](../VERSIONING.md)
5. Validate: `^\d+\.\d+\.\d+$`

**Heuristic:**

| CHANGELOG content | Bump |
|-------------------|------|
| Breaking API / config / CLI removal | MAJOR |
| New features, backward compatible | MINOR |
| Bug fixes, docs, internal refactors | PATCH |

---

### Phase 2 — Readiness Checks (Automated)

Release-checker runs all checks and **aborts on any failure**.

| # | Check | Command | Fix |
|---|-------|---------|-----|
| 1 | On `main` | `git branch --show-current` | `git checkout main && git pull` |
| 2 | Clean tree | `git status --porcelain` | Commit or stash |
| 3 | Nothing unpushed | `git log origin/main..HEAD` | `git push` |
| 4 | Formatting | `cargo fmt --check` | `cargo fmt` |
| 5 | Lint | `cargo clippy --all-targets --all-features --locked -- -D warnings` | Fix warnings |
| 6 | Tests | `cargo test` | Fix failures |
| 7 | Audit | `cargo audit` | Fix or update deps; install: `cargo install cargo-audit` |
| 8 | Changelog exists | grep `## [Unreleased]` or `## [VERSION]` | Add section (see Phase 3) |
| 9 | Changelog extractable | awk script (see below) | Fix header format |
| 10 | Commits covered | Match git log to CHANGELOG | Add missing entries |

**Changelog extraction test** (must match [`.github/workflows/release.yml`](../.github/workflows/release.yml)):

```bash
VERSION="1.0.0"  # replace with target version
awk "/^## \\[${VERSION}\\]/{found=1; next} /^## \\[/{if(found) exit} found{print}" CHANGELOG.md
```

Output must be non-empty. Header format is strict:

```markdown
## [1.0.0] - 2026-07-25
```

Date format: `YYYY-MM-DD` only.

**Local pre-commit equivalent:**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
```

---

### Phase 3 — Execute Release

Release-checker performs these steps with two human gates (commit message + push).

#### 3.1 Bump version

Edit [Cargo.toml](../Cargo.toml):

```toml
version = "X.Y.Z"
```

#### 3.2 Update CHANGELOG

**If `## [Unreleased]` exists:** rename to dated release:

```markdown
## [X.Y.Z] - YYYY-MM-DD
```

**If missing:** create section after the SemVer link (line 7), convention:

```markdown
## [Unreleased]

### Added — Short Title

- Change description
```

Categories use em-dash titles: `### Added — Feature Name`, `### Fixed — Bug Name`.

**If entry exists but date wrong:** set today's date.

#### 3.3 Build

```bash
cargo build --locked
```

Updates [Cargo.lock](../Cargo.lock) if needed; confirms compile.

#### 3.4 Review

```bash
git diff
git log $(git describe --tags --abbrev=0)..HEAD --oneline
```

#### 3.5 Commit (gate 1)

Default message: `chore: release vX.Y.Z`

```bash
git add -A
git commit -m "chore: release vX.Y.Z"
```

#### 3.6 Tag

```bash
git tag vX.Y.Z
```

Tag format: **`v` prefix + SemVer** (e.g. `v1.0.0`).

#### 3.7 Push (gate 2)

```bash
git push && git push --tags
```

**Never force-push** tags or `main`.

---

### Phase 4 — CI & GitHub Release (Automatic)

Pushing `v*` triggers [`.github/workflows/release.yml`](../.github/workflows/release.yml):

| Job | Output |
|-----|--------|
| `build` (matrix) | Linux/macOS binaries (x86_64 + aarch64) |
| `build-vscode` | `.vsix` extension |
| `build-intellij` | IntelliJ plugin `.zip` |
| `release` | GitHub Release with CHANGELOG body + all artifacts |

Release notes are extracted from CHANGELOG via the same awk pattern as Phase 2 check #9.

**Verify after CI completes:**

- [ ] GitHub Release page shows correct version and notes
- [ ] All 6 artifacts attached (4 binaries + VSCode + IntelliJ)
- [ ] Download and smoke-test one binary: `envforge --version`

---

### Phase 5 — Post-Release (Manual)

Release-checker **does not** do these — complete manually:

| Task | Action |
|------|--------|
| **crates.io** | `cargo publish` (crate: `env-forge-tui`) |
| **VSCode Marketplace** | Publish `.vsix` if version bumped in `editors/vscode/package.json` |
| **JetBrains Marketplace** | Upload IntelliJ plugin if version bumped |
| **Docs site** | Update hosted docs if API/CLI changed |
| **Announce** | Release notes, social, changelog link |

**Note:** Release-checker intentionally does not bump editor extension versions. Bump `editors/vscode/package.json` and IntelliJ plugin version before tag if marketplace release is planned.

---

## Release-Checker Rules (Non-Negotiable)

- Never skip Phase 2 checks
- Never force push
- Tag format: `vX.Y.Z`
- Commit format: `chore: release vX.Y.Z` (unless overridden at gate 1)
- No destructive git (rebase, reset, amend release commit, delete tags)
- Abort on ambiguous changelog format
- Preserve CHANGELOG header format for CI extraction

---

## Troubleshooting

### "Switch to main"

Release-checker requires `main`. Merge your release branch:

```bash
git checkout main
git pull
git merge release/X.Y.Z
```

### "Commit or stash changes first"

```bash
git status
git add -A && git commit -m "fix: ..."
# or
git stash -u
```

### Changelog extraction empty

- Header must be exactly `## [X.Y.Z] - YYYY-MM-DD`
- Content must follow immediately (blank line OK)
- Next `## [` section ends extraction

### `cargo audit` not installed

```bash
cargo install cargo-audit
```

### Tag pushed but CI failed

- Fix on `main`, cut patch release, or delete tag locally only if **not pushed** (never delete remote tags without team agreement)
- Re-run workflow from Actions tab after fix

### Version already in CHANGELOG but no tag

If changelog and Cargo.toml were updated ahead of tag (common on release branches):

1. Ensure checks pass on the branch
2. Merge to `main`
3. Tag the merge commit — do **not** re-edit version/changelog unless content changed

---

## Example Session

**User:** `cut release v1.0.0`

**Agent:**

1. Cargo.toml → `1.0.0`, last tag `v0.8.2`, 48 commits
2. Runs 10 readiness checks
3. Confirms CHANGELOG `## [1.0.0] - YYYY-MM-DD` extractable
4. `cargo build --locked`
5. Shows diff + commit log
6. *"Commit message: `chore: release v1.0.0`. Accept? (y/N/edit)"*
7. Commits + tags `v1.0.0`
8. *"Push v1.0.0 to remote? (y/N)"*
9. CI creates GitHub Release at `https://github.com/emreerinc/envforge/releases`

---

## Related Docs

| Document | Purpose |
|----------|---------|
| [VERSIONING.md](../VERSIONING.md) | SemVer policy, stability guarantees |
| [CLAUDE.md](../CLAUDE.md) | Build, test, lint commands |
| [ci-gating.md](./ci-gating.md) | CI integration for consumers |
| [.github/workflows/release.yml](../.github/workflows/release.yml) | Release CI pipeline |
| [release-checker SKILL](../../.agents/skills/release-checker/SKILL.md) | Agent automation spec |

---

## Agent Commands Reference

```text
/specsmd-master-agent     # Orchestrator — routes to Operations when ready
/specsmd-operations-agent # Build → deploy → verify → monitor
prepare release           # Start release-checker (interactive)
cut release vX.Y.Z        # Start release-checker with version
/release                  # Alias for release-checker
```
