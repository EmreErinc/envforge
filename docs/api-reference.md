# EnvForge API Reference v0.7.4

> Rust library reference for integrating envforge types and functions into your own tools.  
> IDE extension authors, CI/CD tool builders, custom shell managers — this is your dictionary.

## Table of Contents

- [Reference](#reference)
  - [Parser (`envforge::parser`)](#parser-envforgeparser)
  - [Model Types (`envforge::model`)](#model-types-envforgemodel)
  - [Config (`envforge::config`)](#config-envforgeconfig)
  - [Ops Layer (`envforge::ops`)](#ops-layer-envforgeops)
  - [LSP (`envforge::lsp`)](#lsp-envforgelsp)
- [How-to Recipes](#how-to-recipes)
  - [Parse a shell file and list all env vars](#parse-a-shell-file-and-list-all-env-vars)
  - [Add a variable and write back atomically](#add-a-variable-and-write-back-atomically)
  - [Detect duplicates across shell files](#detect-duplicates-across-shell-files)
  - [Use OpError for typed error handling](#use-operror-for-typed-error-handling)
  - [Build a validation pipeline with schema](#build-a-validation-pipeline-with-schema)

---

## Reference

### Parser (`envforge::parser`)

The parser reads shell configuration files (`.zshrc`, `.bashrc`, `.profile`) into a line-level AST. Every byte — comments, blanks, formatting — is preserved. A parse → serialize round-trip produces byte-identical output.

```rust
// Public API
pub fn parse_shell_file(path: &Path) -> Result<ShellFile, ParseError>
pub fn parse_shell_content(content: &str, path: &Path) -> Result<ShellFile, ParseError>
pub fn detect_shell() -> Result<Shell, ParseError>
pub fn scan_config_files(shell: &Shell) -> Result<Vec<PathBuf>, ParseError>
pub fn default_primary_file(shell: &Shell) -> Result<PathBuf, ParseError>
```

#### `parse_shell_file(path)`

Reads a shell file from disk, computes its SHA-256 hash, and parses every line into a `LineNode` variant.

- **Returns:** `ShellFile` with populated `path`, `lines`, and `hash` fields.
- **Errors:** `ParseError::IoError` if the file cannot be read.

#### `parse_shell_content(content, path)`

Parse shell text already in memory. The `path` argument is used for error context and metadata only — no disk I/O is performed.

```rust
use envforge::parser::parse_shell_content;
use std::path::Path;

let sf = parse_shell_content("export FOO=\"bar\"\n", Path::new("/test/.zshrc"))?;
```

#### `detect_shell()`

Reads `$SHELL` from the environment and classifies it.

- `Shell::Zsh` — path ends with `/zsh`
- `Shell::Bash` — path ends with `/bash`
- `Shell::Unknown(s)` — anything else (e.g., `/usr/bin/fish`)

**Errors:** `ParseError::ShellNotDetected` if `$SHELL` is unset.

#### `scan_config_files(shell)`

Scans the user's home directory for config files relevant to the shell type. Returns only files that actually exist on disk.

| Shell | Scanned files |
|-------|--------------|
| Zsh | `.zshrc`, `.zprofile` |
| Bash | `.bashrc`, `.bash_profile`, `.profile` |
| Unknown | All five |

#### `default_primary_file(shell)`

Returns the conventional primary config path for the shell type (does not check existence).

---

### Model Types (`envforge::model`)

#### `LineNode`

The fundamental unit of a parsed shell file. Every line becomes one variant.

```rust
pub enum LineNode {
    Blank { line_number: usize, original_text: String },
    Comment { line_number: usize, original_text: String, text: String },
    EnvExport {
        line_number: usize,
        original_text: String,
        key: String,
        value: String,
        export_style: ExportStyle,
        quote_style: QuoteStyle,
        inline_comment: Option<String>,
    },
    ManagedComment { line_number: usize, original_text: String, tag: String, original_export: String },
    EnvforgeStart { line_number: usize, original_text: String },
    EnvforgeEnd { line_number: usize, original_text: String },
    SourceDirective { line_number: usize, original_text: String, path: String },
    Other { line_number: usize, original_text: String },
}
```

| Variant | Description |
|---------|------------|
| `Blank` | Empty or whitespace-only line |
| `Comment` | A `#` comment line (unless it's an envforge tag) |
| `EnvExport` | `export KEY=VALUE` or `KEY=VALUE` assignment |
| `ManagedComment` | A line envforge commented out: `#[envforge:tagname] export KEY=VALUE` |
| `EnvforgeStart` | Managed zone start: `# >>> envforge >>>` |
| `EnvforgeEnd` | Managed zone end: `# <<< envforge <<<` |
| `SourceDirective` | `source` or `.` directive |
| `Other` | Anything else (aliases, functions, path mods, etc.) |

**Methods:**

```rust
impl LineNode {
    pub fn line_number(&self) -> usize
    pub fn original_text(&self) -> &str
    pub fn serialize(&self, modified: bool) -> String
}
```

`serialize(false)` returns the original text unchanged (round-trip safety).
`serialize(true)` regenerates `EnvExport` lines from `key`, `value`, `export_style`, `quote_style`, and `inline_comment` fields. Non-export variants always return `original_text`.

#### `ExportStyle`

```rust
pub enum ExportStyle {
    Export,  // "export KEY=VALUE"
    Bare,    // "KEY=VALUE" (no export prefix)
}
```

#### `QuoteStyle`

```rust
pub enum QuoteStyle {
    Double,  // "value"
    Single,  // 'value'
    None,    // value (no quotes)
}
```

#### `Shell`

```rust
pub enum Shell {
    Zsh,
    Bash,
    Unknown(String),
}
```

#### `ShellFile`

```rust
pub struct ShellFile {
    pub path: PathBuf,
    pub lines: Vec<LineNode>,
    pub hash: [u8; 32],  // SHA-256 of original file content
}

impl ShellFile {
    pub fn serialize(&self) -> String
}
```

`serialize()` joins all lines with `\n`. Each line uses `serialize(false)` — unchanged nodes return original text, modified `EnvExport` nodes regenerate from fields. The result is the full file content ready for `atomic_write`.

#### `ParseError`

```rust
pub enum ParseError {
    IoError { path: PathBuf, source: std::io::Error },
    HomeDirNotFound,
    ShellNotDetected,
}
```

All parser functions return `Result<T, ParseError>`.

#### Session Types

```rust
pub struct SessionId(pub String);

pub enum AiTool {
    ClaudeCode,
    GitHubCopilot,
    Cursor,
    Unknown,
}

pub enum SessionState { Active, Expired }

pub struct SessionConfig {
    pub default_ttl: chrono::Duration,  // default: 1 hour
    pub auto_detect: bool,
    pub auto_cleanup: bool,
}
```

`AiTool` implements `FromStr` (accepts `"claude-code"`, `"cursor"`, `"copilot"`), `Display`, and `Serialize/Deserialize`. `SessionId` implements `Default` (generates UUID v4), `Display`, `Hash`, `Eq`.

#### Lifecycle Types

```rust
pub struct LifecycleRule { pub id: Uuid, pub name: String, pub description: String,
    pub trigger: LifecycleTrigger, pub condition: Option<LifecycleCondition>,
    pub action: LifecycleAction, pub enabled: bool, pub tags: Vec<String>, ... }

pub enum LifecycleTrigger { Cron { expression: String }, AgeExceeded { max_days: u32 },
    FileChange { paths: Vec<PathBuf> }, PolicyViolation { policy: String },
    Composite { triggers: Vec<LifecycleTrigger>, operator: LogicalOp } }

pub enum LogicalOp { All, Any, Not }

pub enum LifecycleAction { Create { template_id: Uuid }, Rotate { strategy: RotationStrategy },
    Decommission { grace_days: Option<u32> }, Notify { message: String },
    Composite { actions: Vec<LifecycleAction> } }

pub enum RotationStrategy { Replace, DualWrite, BlueGreen, ProviderManaged }

pub enum LifecycleState { Creating, Active, Rotating, PendingDeprecation, Deprecated,
    Decommissioned, Failed }

pub struct SecretLifecycle { pub key: String, pub state: LifecycleState,
    pub history: Vec<StateTransition>, pub last_rotation: Option<DateTime<Utc>>,
    pub rotation_count: u32, pub expiry: Option<DateTime<Utc>> }

pub struct TriggerEvent { pub trigger_type: TriggerType, pub rule_id: Uuid,
    pub secret_key: Option<String>, pub payload: String, ... }

pub struct EvaluationContext { pub project_dir: Option<PathBuf>,
    pub current_time: DateTime<Utc>, pub last_check: Option<DateTime<Utc>> }

pub enum LifecycleError { RuleNotFound { id: Uuid }, InvalidTransition { ... },
    RuleConflict { reason: String }, TriggerEvalFailed { rule_id: Uuid, reason: String },
    StorageError { message: String, path: Option<PathBuf> },
    SnapshotNotFound { id: Uuid } }
```

#### Analytics Types

```rust
pub struct AnalyticsConfig { pub enabled: bool, pub retention_days: u32,
    pub max_events: usize, pub auto_aggregate: bool, pub store_path: Option<PathBuf> }

pub struct RawAccessEvent { pub secret_name: String, pub access_type: AccessType,
    pub accessor: AccessorInfo, pub timestamp: DateTime<Utc>, pub source: AccessSource, ... }

pub struct EnrichedAccessEvent { pub id: Uuid, pub raw: RawAccessEvent,
    pub secret_id: String, pub provider: String, pub risk_level: RiskLevel, ... }

pub enum AccessType { Read, Write, Reference, Proxy }

pub enum RiskLevel { Low, Medium, High, Critical }

pub enum AnalyticsError { StorageError { path: PathBuf, source: io::Error },
    EventParseError { source: serde_json::Error }, InvalidTimeWindow { description: String },
    NoDataAvailable { message: String }, ProviderNotFound { provider: String },
    ExportError { format: String, reason: String } }
```

---

### Config (`envforge::config`)

#### `AppConfig`

The full application configuration, serializable to `~/.config/envforge/config.toml`.

```rust
pub struct AppConfig {
    pub general: GeneralConfig,
    pub files: FilesConfig,
    pub offsets: OffsetsConfig,
    pub protected_blocks: ProtectedBlocksConfig,
    pub groups: HashMap<String, Vec<String>>,
    pub profiles: ProfilesConfig,
    pub validation: HashMap<String, String>,
    pub clipboard: ClipboardConfig,
    pub lifecycle: LifecycleConfig,
    pub analytics: AnalyticsConfig,
}
```

Key sub-types:

| Type | Key fields | Default |
|------|-----------|---------|
| `GeneralConfig` | `default_shell: String` | `"zsh"` |
| `FilesConfig` | `primary`, `reference: String`, `use_reference_file: bool` | `~/.zshrc`, `~/.env_managed`, `true` |
| `OffsetsConfig` | `header_protected_lines: usize`, `footer_protected_lines: usize` | `0`, `0` |
| `ProtectedBlocksConfig` | `markers: Vec<String>` | `[]` |
| `ClipboardConfig` | `enabled: bool`, `warn_on_secret: bool` | `true`, `true` |
| `LifecycleConfig` | `default_stale_threshold_days`, `default_grace_period_days`, `default_rotation_strategy`, `snapshot_retention_days` | `90`, `7`, `"replace"`, `30` |
| `ProfilesConfig` | `active: String`, `shared_file: String`, `entries: HashMap<String, ProfileEntry>` | `"default"`, `~/.env_managed.shared` |

`ProfilesConfig` convenience methods:

```rust
impl ProfilesConfig {
    pub fn active_entry(&self) -> Option<&ProfileEntry>
    pub fn active_file(&self) -> Option<String>
    pub fn profile_names(&self) -> Vec<String>
}
```

#### Config lifecycle functions

```rust
pub fn config_file_path() -> Result<PathBuf, ConfigError>
pub fn load_or_create_default() -> Result<AppConfig, ConfigError>
pub fn save_config(config: &AppConfig, path: &Path) -> Result<(), ConfigError>
```

#### `ConfigError`

```rust
pub enum ConfigError {
    IoError { path: PathBuf, source: std::io::Error },
    TomlSerializeError { path: PathBuf, source: toml::ser::Error },
    TomlDeserializeError { path: PathBuf, source: toml::de::Error },
    HomeDirNotFound,
    BackupError { path: PathBuf, message: String },
}
```

#### Backup functions

```rust
pub fn create_backup(file_path: &Path) -> Result<PathBuf, ConfigError>
pub fn list_backups(file_path: &Path) -> Result<Vec<PathBuf>, ConfigError>
pub fn restore_backup(backup_path: &Path, target_path: &Path) -> Result<(), ConfigError>
pub fn prune_old_backups(file_path: &Path, keep: usize) -> Result<(), ConfigError>
```

`create_backup` copies the file to `~/.config/envforge/backups/{filename}.{timestamp}.bak`.  
`prune_old_backups` retains only the most recent `keep` backups.

#### `WriteError`

```rust
pub enum WriteError {
    IoError { path: PathBuf, source: std::io::Error },
    HashMismatch { path: PathBuf },
    TempFileError { dir: PathBuf, source: std::io::Error },
    PersistError { path: PathBuf, source: tempfile::PersistError },
}
```

#### Atomic write

```rust
pub fn atomic_write(
    path: &Path,
    content: &str,
    expected_hash: Option<&[u8; 32]>,
    create_backup: bool,
) -> Result<(), WriteError>
```

Writes content to a temp file in the same directory, then renames it to the target path (atomic on the same filesystem). If `expected_hash` is provided, reads the current file first and verifies the hash — aborts with `HashMismatch` if the file was modified externally. If `create_backup` is true, creates a backup before overwriting.

---

### Ops Layer (`envforge::ops`)

The ops layer contains pure business logic. 50+ modules covering CRUD, encryption, sync, secrets, AI safety, lifecycle, analytics, and more.

#### `OpError` — Unified Error Type

```rust
pub enum OpError {
    Io(std::io::Error),
    TomlDeserialize(toml::de::Error),
    TomlSerialize(toml::ser::Error),
    Json(serde_json::Error),
    Config(crate::config::ConfigError),
    Parse(crate::model::ParseError),
    Encrypt(encrypt::EncryptError),
    Sync(sync::SyncError),
    Crud(crud::OpsError),
    Write(crate::config::WriteError),
    Other(String),
}
```

`OpError` implements `From` for all wrapped types, `From<String>`, and `From<&str>`. This enables ergonomic `?` propagation throughout ops functions.

#### Re-exports (from `envforge::ops`)

The `ops` module re-exports commonly-used types from submodules:

```
envforge::ops::changelog  →  change tracking API
envforge::ops::clipboard  →  clipboard integration
envforge::ops::crud       →  add, edit, delete, move, toggle
envforge::ops::dotenv     →  .env parsing, safe export
envforge::ops::duplicates →  duplicate key detection
envforge::ops::encrypt    →  age encryption/decryption + EncryptError
envforge::ops::fuzzy      →  fuzzy search with highlighting
envforge::ops::grouping   →  variable grouping
envforge::ops::listing    →  entry collection and filtering
envforge::ops::offset     →  protected zone management
envforge::ops::profile    →  profile management
envforge::ops::scanner    →  secret scanning
envforge::ops::undo       →  undo/redo stack
envforge::ops::validation →  rule-based value validation
```

---

### LSP (`envforge::lsp`)

#### `start_lsp_server()`

```rust
pub async fn start_lsp_server() -> Result<(), Box<dyn std::error::Error + Send + Sync>>
```

Starts a TCP-based Language Server Protocol server on stdin/stdout. Provides:

| Feature | LSP capability |
|---------|---------------|
| Diagnostics | `textDocument/diagnostic` — missing required vars, type errors, secret leaks |
| Hover | `textDocument/hover` — type, description, default from `.env.schema` |
| Completions | `textDocument/completion` — envforge-managed vars + schema keys |
| Go-to-definition | `textDocument/definition` — click key in `.env` → jump to `.env.schema` section |
| Code actions | `textDocument/codeAction` — quick fixes for schema violations |
| Code lenses | `textDocument/codeLens` — inline hints for encrypted/referenced values |
| Document symbols | `textDocument/documentSymbol` — hierarchical symbol outline |
| Workspace symbols | `workspace/symbol` — project-wide env var search |
| Folding ranges | `textDocument/foldingRange` — fold managed zones and profile sections |

Uses `rowLock<HashMap>` for concurrent document access. Managed vars are loaded from envforge's shell files at startup.

---

## How-to Recipes

### Parse a shell file and list all env vars

```rust
use envforge::parser::parse_shell_file;
use envforge::model::LineNode;
use std::path::Path;

fn list_env_vars(shell_config: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let sf = parse_shell_file(shell_config)?;

    for node in &sf.lines {
        if let LineNode::EnvExport { key, value, .. } = node {
            println!("{} = {}", key, value);
        }
    }

    Ok(())
}
```

### Add a variable and write back atomically

```rust
use envforge::parser::parse_shell_file;
use envforge::config::{create_backup, atomic_write};
use envforge::model::LineNode;

fn upsert_var(path: &Path, key: &str, new_value: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut sf = parse_shell_file(path)?;
    let original_hash = sf.hash;

    // Find existing export or create a new one
    let mut found = false;
    for node in &mut sf.lines {
        if let LineNode::EnvExport { key: k, value, export_style, quote_style, .. } = node {
            if k == key {
                *value = new_value.to_string();
                found = true;
            }
        }
    }

    if !found {
        // Append a new export line
        sf.lines.push(LineNode::EnvExport {
            line_number: sf.lines.len(),
            original_text: String::new(),
            key: key.to_string(),
            value: new_value.to_string(),
            export_style: envforge::model::ExportStyle::Export,
            quote_style: envforge::model::QuoteStyle::Double,
            inline_comment: None,
        });
    }

    let content = sf.serialize();
    create_backup(path)?;
    atomic_write(path, &content, Some(&original_hash), false)?;

    Ok(())
}
```

### Detect duplicates across shell files

```rust
use envforge::parser::parse_shell_file;
use envforge::model::LineNode;
use std::collections::{HashMap, HashSet};

fn find_duplicates(files: &[&Path]) -> HashMap<String, Vec<String>> {
    let mut key_sources: HashMap<String, Vec<String>> = HashMap::new();

    for file in files {
        if let Ok(sf) = parse_shell_file(file) {
            let path_str = sf.path.display().to_string();
            for node in &sf.lines {
                if let LineNode::EnvExport { key, .. } = node {
                    key_sources.entry(key.clone()).or_default().push(path_str.clone());
                }
            }
        }
    }

    key_sources.retain(|_, sources| sources.len() > 1);
    key_sources
}
```

### Use OpError for typed error handling

```rust
use envforge::ops::OpError;
use envforge::config::ConfigError;

fn my_ops_function() -> Result<(), OpError> {
    // All these error types auto-convert to OpError via `?`
    let config = envforge::config::load_or_create_default()?; // ConfigError
    let sf = envforge::parser::parse_shell_file(
        &std::path::PathBuf::from(&config.files.primary)
    )?; // ParseError

    // Custom errors
    if sf.lines.is_empty() {
        return Err(OpError::Other("No env vars found".into()));
    }

    Ok(())
}
```

### Build a validation pipeline with schema

```rust
use envforge::ops::schema;
use envforge::ops::validation;
use envforge::parser::parse_shell_file;
use std::path::Path;

fn validate_with_schema(
    schema_path: &Path,
    env_file: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let schema_content = std::fs::read_to_string(schema_path)?;
    let schema = schema::parse_schema_content(&schema_content, schema_path)?;

    let envs: Vec<_> = std::fs::read_to_string(env_file)?
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let errors = schema::validate_entries(&schema, &envs, None);
    if errors.is_empty() {
        println!("All {} variables valid", envs.len());
    } else {
        for e in &errors {
            eprintln!("{}: {}", e.variable, e.message);
        }
    }

    Ok(())
}
```
