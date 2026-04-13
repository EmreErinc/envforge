# Versioning & Release Policy

EnvForge follows [Semantic Versioning 2.0.0](https://semver.org/).

## Version Format

```
MAJOR.MINOR.PATCH
```

- **MAJOR** — Breaking changes (config format, CLI interface, file format)
- **MINOR** — New features, backward compatible
- **PATCH** — Bug fixes, performance improvements

## Pre-1.0 Policy

While EnvForge is below `1.0.0`:

- **0.x.0** releases may include breaking changes
- **0.x.y** releases are backward compatible patches
- The config format may change between minor versions
- Shell file format (envforge tags) is considered stable from `0.1.0`

## Post-1.0 Policy

After `1.0.0`:

- Breaking changes only in major versions
- Config format changes include auto-migration
- Shell file tags are never changed (backward compatible forever)
- CLI subcommand removal only in major versions (deprecation in minor)

## Release Process

### Creating a Release

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Commit: `git commit -m "release: v0.x.y"`
4. Tag: `git tag v0.x.y`
5. Push: `git push && git push --tags`
6. GitHub Actions builds binaries and creates release

### Release Artifacts

Each release includes:

| Platform | Architecture | Artifact |
|----------|-------------|----------|
| Linux | x86_64 | `envforge-x86_64-unknown-linux-gnu.tar.gz` |
| Linux | aarch64 | `envforge-aarch64-unknown-linux-gnu.tar.gz` |
| macOS | x86_64 | `envforge-x86_64-apple-darwin.tar.gz` |
| macOS | aarch64 (Apple Silicon) | `envforge-aarch64-apple-darwin.tar.gz` |

### Distribution Channels

| Channel | Update Frequency |
|---------|-----------------|
| GitHub Releases | Every tag |
| `cargo install env-forge-tui` | Every tag (crates.io) |

## Stability Guarantees

### Stable (will not break without major version)
- Shell file round-trip fidelity
- EnvForge tag format (`#[envforge:deleted:KEY]`, etc.)
- CLI subcommand names and core flags
- Config file backward compatibility (new fields have defaults)

### Unstable (may change in minor versions)
- TUI layout and visual appearance
- Internal module API (library use not officially supported)
- JSON output field names
- Changelog log format

## Deprecation Policy

1. Feature marked as deprecated in release notes
2. Warning printed on use for at least one minor version
3. Removed in next major version
