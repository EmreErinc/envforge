# EnvForge

A Rust CLI + TUI for environment variables, with extra tools so AI coding agents are less likely to eat your secrets.

Fence secret files, harden MCP configs, and run commands with secrets in memory. Linux and macOS. Source-available (Elastic License 2.0).

[Website](https://envforge.tech) · [CLI reference](docs/cli-reference.md) · [Integration matrix](docs/integration-matrix.md) · [Security](SECURITY.md)

![License: ELv2](https://img.shields.io/badge/License-ELv2-blue.svg)
![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange.svg)
![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20macOS-blue.svg)

License: Elastic License 2.0 (source-available, not OSI open source).

![EnvForge Demo](assets/envforge_demo.gif)

GitGuardian's 2026 report found AI-assisted commits leak secrets at about 2x the baseline rate, and more than 24,000 credentials sitting in MCP config files on public GitHub.

EnvForge is not a sandbox. Ignore files do not block the terminal or MCP. See the [integration matrix](docs/integration-matrix.md) for covered vs fallback per tool.

## Install

```bash
brew install emreerinc/tap/envforge
```

Or:

```bash
cargo install env-forge-tui
```

Needs Rust 1.75+ if you build from source. Binary name is `envforge`. Crate name is `env-forge-tui`.

Linux and macOS tarballs: [Releases](https://github.com/EmreErinc/envforge/releases).

## Protect a project

```bash
envforge fence
envforge fence --status
envforge mcp status
envforge mcp harden
envforge ai-hook install cursor
```

To inject env into a process without writing secrets to disk, see `envforge run` in the CLI reference.

## Everyday use

```bash
envforge
envforge list
envforge set KEY=value
envforge doctor
```

Full command list: [CLI reference](docs/cli-reference.md).

## Editors

Language Server: `envforge lsp`.

- [VS Code](https://marketplace.visualstudio.com/items?itemName=emreerinc.envforge-env-manager)
- [IntelliJ](https://plugins.jetbrains.com/plugin/31385-envforge)
- Neovim: [`editors/`](editors/)
- Zed: LSP only (no gutter / status bar yet)

For full CLI features, install with the Homebrew tap above. Do not use `brew install envforge`.

## Limits

- Fence writes ignore files and rules. It is not a sandbox.
- `envforge mcp serve` is optional (Cargo feature mcp-server). Default brew and GitHub binaries do not include it.
- Windows is not supported.
- Secret providers call vendor CLIs (`vault`, `op`, `aws`). They are not native SDKs.

## Docs

| Doc | What |
|-----|------|
| [CLI reference](docs/cli-reference.md) | Commands |
| [Integration matrix](docs/integration-matrix.md) | Covered vs fallback per AI tool |
| [MCP server](docs/mcp-server.md) | Optional metadata-only MCP |
| [LSP clients](docs/lsp-clients.md) | Editor setup |
| [SECURITY.md](SECURITY.md) | Threat model |
| [CHANGELOG.md](CHANGELOG.md) | Releases |

## License

[Elastic License 2.0](LICENSE). Source-available, not OSI open source. See [CONTRIBUTING.md](CONTRIBUTING.md) to send a patch.
