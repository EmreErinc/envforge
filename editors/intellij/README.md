# EnvForge for IntelliJ IDEA

Environment variable management with schema validation, secret scanning, and multi-profile support.

## Features

### LSP (Language Server)
- **Diagnostics** — Missing required vars, type errors, secret leak warnings
- **Hover** — Schema info: type, description, default, example, sensitive flag
- **Completions** — All envforge-managed vars + schema keys + value suggestions (bool, enum, defaults)
- **Go-to-definition** — Navigate from `.env` key to `.env.schema` section

### Tool Window (Sidebar)
- **Variables panel** — Grouped by prefix (toggle grouping), masked sensitive values
- **Profiles panel** — View all profiles, double-click to switch active profile
- **Copy operations** — Right-click context menu: Copy Key Name, Copy Value, Copy KEY=VALUE
- **Refresh button** — Manual refresh of variables and profiles
- **Grouping toggle** — Switch between grouped and flat variable view

### Actions (Tools > EnvForge)
- List Variables, Switch Profile, Diff Profiles
- Validate, Scan, Health Check, Run All Checks
- Export (7 formats), Sync Push/Pull, Generate Schema

### Status Bar
- Variable count widget

## Requirements

### 1. EnvForge CLI

```bash
cargo install env-forge-tui
```

Verify:

```bash
envforge --version
```

### 2. LSP4IJ Plugin

Install [LSP4IJ](https://plugins.jetbrains.com/plugin/23257-lsp4ij) from JetBrains Marketplace:

1. Open **Settings** > **Plugins** > **Marketplace**
2. Search "LSP4IJ"
3. Install and restart IDE

LSP4IJ provides the Language Server Protocol client framework that this plugin uses.

### 3. IntelliJ IDEA 2024.2+

This plugin requires IntelliJ IDEA 2024.2 or later. Works with:
- IntelliJ IDEA Community / Ultimate
- WebStorm, PyCharm, GoLand, RustRover (any JetBrains IDE with LSP4IJ support)

## Installation

### From JetBrains Marketplace (recommended)

1. Open **Settings** > **Plugins** > **Marketplace**
2. Search "EnvForge"
3. Install and restart IDE

Or visit: [JetBrains Marketplace](https://plugins.jetbrains.com/plugin/com.envforge.intellij)

### From source (development)

```bash
cd editors/intellij

# Build the plugin
./gradlew buildPlugin

# The plugin ZIP will be at:
# build/distributions/envforge-intellij-0.1.3.zip
```

Install manually:
1. Open **Settings** > **Plugins** > gear icon > **Install Plugin from Disk...**
2. Select `build/distributions/envforge-intellij-0.1.3.zip`
3. Restart IDE

### Run in development mode

```bash
cd editors/intellij
./gradlew runIde
```

This launches a sandboxed IntelliJ instance with the plugin loaded.

## Configuration

The plugin auto-detects `envforge` from these locations (in order):

1. `ENVFORGE_PATH` environment variable
2. `~/.cargo/bin/envforge`
3. `/usr/local/bin/envforge`
4. `/opt/homebrew/bin/envforge`
5. System `PATH`

## Actions

All actions available under **Tools > EnvForge** menu:

| Action | Description |
|--------|-------------|
| List Variables | Show all environment variables |
| Switch Profile... | Pick a profile to switch to |
| Diff Profiles... | Compare two profiles |
| Validate Against Schema | Run schema validation on current project |
| Scan for Secret Leaks | Scan for leaked secrets |
| Health Check | Run envforge doctor |
| Run All Checks | Run doctor + validate + scan + age + drift |
| Export Variables... | Pick format and export (dotenv/json/yaml/toml/docker/k8s/tfvars) |
| Sync Push | Push env changes to remote |
| Sync Pull | Pull env changes from remote |
| Generate Schema | Auto-generate `.env.schema` from current variables |

## Tool Window

The **EnvForge** tool window appears on the right sidebar:

```
EnvForge (right sidebar)
├── [Refresh] [Toggle Grouping]
├── Profiles
│   ├── ✓ default (active)
│   └── ○ dev          ← double-click to switch
└── Variables
    ├── AWS_* (3)
    │   ├── AWS_ACCESS_KEY = AKI***
    │   ├── AWS_SECRET_KEY = wJa***
    │   └── AWS_REGION = us-east-1
    ├── DATABASE_* (2)
    │   ├── DATABASE_URL = postgres://...
    │   └── DATABASE_PORT = 5432
    └── Other (5)
        └── PORT = 3000
```

**Context menu** (right-click any variable):
- Copy Key Name
- Copy Value
- Copy KEY=VALUE

## How It Works

The plugin uses [LSP4IJ](https://github.com/redhat-developer/lsp4ij) to connect to the EnvForge Language Server (`envforge lsp`) over stdio.

```
IntelliJ IDEA
  └── LSP4IJ (LSP client framework)
        └── envforge lsp (stdio)
              ├── Diagnostics (schema validation, secret scanning)
              ├── Hover (schema metadata)
              ├── Completions (key suggestions)
              └── Go-to-definition (.env → .env.schema)
```

Actions use the CLI directly by spawning `envforge <command>` processes.

## Schema Example

Create `.env.schema` in your project root:

```toml
[DATABASE_URL]
type = "url"
required = true
description = "PostgreSQL connection string"
example = "postgres://user:pass@localhost:5432/mydb"
sensitive = true

[PORT]
type = "port"
required = true
default = "3000"
description = "Server port"

[LOG_LEVEL]
type = "enum"
values = ["debug", "info", "warn", "error"]
default = "info"
description = "Application log level"
```

The LSP server validates `.env` files against this schema in real-time, providing inline errors and warnings.

## Troubleshooting

**LSP not starting:**
- Check `envforge --version` works in terminal
- Check LSP4IJ plugin is installed and enabled
- Check **Help > Show Log** for errors

**No diagnostics:**
- Ensure `.env.schema` exists in project root
- Open a `.env` file — the LSP activates on file open

**Binary not found:**
- Set `ENVFORGE_PATH` environment variable to the full path
- Or ensure `envforge` is in your shell PATH that IDE inherits
