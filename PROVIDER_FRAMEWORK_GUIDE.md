# EnvForge Secret Provider Framework Guide

## Overview

EnvForge supports 13 secret management providers through an extensible trait-based framework. This guide explains how the provider system works and how to add new providers or modify existing ones.

**Supported Providers:**
- HashiCorp Vault
- AWS Secrets Manager / Parameter Store
- Azure Key Vault
- Google Cloud Secret Manager
- Doppler
- Infisical
- 1Password
- Bitwarden Secrets Manager
- Akeyless Vault
- CyberArk Conjur
- Mozilla SOPS
- pass/gopass
- Keeper Secrets Manager

## Architecture

### Core Trait: `SecretProvider`

All providers implement the `SecretProvider` trait defined in `src/ops/secrets/provider.rs`:

```rust
pub trait SecretProvider: Send + Sync {
    fn name(&self) -> &str;                    // Machine-readable name (e.g., "vault")
    fn display_name(&self) -> &str;            // User-friendly name (e.g., "HashiCorp Vault")
    fn binary_name(&self) -> &str;             // CLI binary name (e.g., "vault")
    fn install_hint(&self) -> &str;            // Installation URL
    fn credential_fields(&self) -> Vec<&str>;  // Required credentials
    fn build_provider_env(&self, credentials: &HashMap<String, String>) -> Vec<(&'static str, String)>;
    
    fn pull(&self, credentials: &HashMap<String, String>, path: &str) -> Result<Vec<(String, String)>, SecretsError>;
    fn push(&self, credentials: &HashMap<String, String>, path: &str, secrets: &[(String, String)]) -> Result<usize, SecretsError>;
    fn get(&self, credentials: &HashMap<String, String>, path: &str, key: &str) -> Result<String, SecretsError>;
    fn list(&self, credentials: &HashMap<String, String>, path: &str) -> Result<Vec<String>, SecretsError>;
}
```

### Key Methods

#### `build_provider_env()`
Returns provider-specific environment variables to inject when running CLI commands.

**Example (Vault):**
```rust
fn build_provider_env(&self, credentials: &HashMap<String, String>) -> Vec<(&'static str, String)> {
    let mut env = Vec::new();
    if let Some(addr) = credentials.get("address") {
        env.push(("VAULT_ADDR", addr.clone()));
    }
    if let Some(token) = credentials.get("token") {
        env.push(("VAULT_TOKEN", token.clone()));
    }
    env
}
```

#### `pull()` and `push()`
Fetch all secrets from or push all secrets to the provider.

#### `get()` and `list()`
Get a single secret or list all secret names. Default implementations use `pull()`.

## Shared Utilities

### Helper Functions in `provider.rs`

#### 1. `env_refs_from_env()`
Converts owned environment variables to borrowed references for passing to `Command::env()`:

```rust
let env_vars = provider.build_provider_env(&credentials);
let env_refs = env_refs_from_env(&env_vars);  // Convert to &str references
run_cli("vault", &args, &env_refs, "vault")?;
```

**Why this matters:** Lifetime management allows safe passing of string slices without unnecessary clones.

#### 2. `sort_secret_pairs()`
Sorts a mutable slice of (String, String) tuples by key for deterministic output.

**Pattern:** All 13 providers use identical sorting logic:
```rust
let mut secrets: Vec<(String, String)> = /* ... */;
sort_secret_pairs(&mut secrets);  // In-place sort by key
```

#### 3. `extract_json_object()`
Extracts a JSON object at a specific path, filters out system keys, and returns sorted secrets.

**Parameters:**
- `json`: The parsed JSON value
- `path`: Vec of keys to traverse (e.g., `&["data", "data"]` for nested paths)
- `provider`: Provider name for error reporting

**Features:**
- Filters out keys starting with `_` (system keys)
- Handles deeply nested structures
- Returns sorted `Vec<(String, String)>`

**Example (Infisical):**
Infisical returns an array where each item has `{"key": "...", "value": "..."}`:
```rust
extract_json_object(&json, &["fields"], "infisical")  // Works with 1Password too
```

#### 4. `parse_json_secrets()`
Complete parsing pipeline: parse JSON, extract object at path, filter, sort, return secrets.

**Simplest abstraction:** Reduces JSON parsing boilerplate:
```rust
// Before refactoring (40+ LOC per provider):
let json: serde_json::Value = serde_json::from_str(output)?;
let obj = json.get("data").and_then(|d| d.get("data"))?;
// ... manual filtering and sorting ...

// After refactoring (1 LOC):
parse_json_secrets(output, &["data", "data"], "vault")
```

## Provider Implementation Pattern

### New Provider Template

```rust
use std::collections::HashMap;
use super::super::provider::{
    env_refs_from_env, run_cli, sort_secret_pairs, SecretProvider, SecretsError,
};

pub struct MyProvider;

impl SecretProvider for MyProvider {
    fn name(&self) -> &str { "my-provider" }
    fn display_name(&self) -> &str { "My Provider" }
    fn binary_name(&self) -> &str { "my-cli" }
    fn install_hint(&self) -> &str { "https://example.com/install" }
    fn credential_fields(&self) -> Vec<&str> { vec!["api_key"] }

    fn build_provider_env(&self, credentials: &HashMap<String, String>) -> Vec<(&'static str, String)> {
        let mut env = Vec::new();
        if let Some(key) = credentials.get("api_key") {
            env.push(("MY_API_KEY", key.clone()));
        }
        env
    }

    fn pull(&self, credentials: &HashMap<String, String>, _path: &str) -> Result<Vec<(String, String)>, SecretsError> {
        let env_vars = self.build_provider_env(credentials);
        let env_refs = env_refs_from_env(&env_vars);

        let output = run_cli("my-cli", &["secrets", "export"], &env_refs, "my-provider")?;
        parse_my_output(&output)
    }

    // Implement push, get, list...
}

fn parse_my_output(output: &str) -> Result<Vec<(String, String)>, SecretsError> {
    let map: HashMap<String, String> = serde_json::from_str(output)
        .map_err(|e| SecretsError::ParseError {
            provider: "my-provider".to_string(),
            message: e.to_string(),
        })?;
    
    let mut secrets: Vec<(String, String)> = map.into_iter().collect();
    sort_secret_pairs(&mut secrets);
    Ok(secrets)
}
```

## Provider-Specific Variations

### No Environment Variables (Azure, GCP, Akeyless, Conjur, Keeper)

Some providers use only CLI flags or profile-based auth, not environment variables:

```rust
fn build_provider_env(&self, _credentials: &HashMap<String, String>) -> Vec<(&'static str, String)> {
    Vec::new()  // Azure uses `az`, GCP uses `gcloud`, Akeyless uses profile/flags
}
```

### Custom JSON Parsing

Each provider's JSON response is different:

| Provider | Response Format | Path |
|----------|-----------------|------|
| Vault | `{"data": {"data": {"key": "value"}}}` | `&["data", "data"]` |
| AWS SSM | `{"Parameters": [{...}]}` | Array iteration |
| Doppler | `{"KEY": "value"}` at root | Root map |
| Infisical | `[{"key": "...", "value": "..."}]` | Array iteration |
| 1Password | `{"fields": [{...}]}` | Array in fields |
| Bitwarden | `[{"key": "...", "value": "...", "id": "..."}]` | Array iteration |
| Akeyless | `{"/path": "value"}` (get), `[{"item_name": "..."}]` (list) | Two-step: list + get |
| Conjur | `["account:variable:path"]` (list), plaintext (get) | JSON list + plaintext get |
| SOPS | Decrypted flat JSON `{"KEY": "value"}` | Root map (filter `sops` key) |
| pass/gopass | Plaintext stdout (first line = value) | No JSON parsing |
| Keeper | `{"fields": [{"type": "password", "value": ["..."]}]}` | Nested typed field arrays |

### System Keys Filtering

Some providers inject system keys that shouldn't be treated as secrets:

```rust
// Doppler injects these that we filter out
const DOPPLER_SYSTEM_KEYS: &[&str] = &["DOPPLER_PROJECT", "DOPPLER_CONFIG", "DOPPLER_ENVIRONMENT"];

fn parse_doppler_output(output: &str) -> Result<Vec<(String, String)>, SecretsError> {
    let map: HashMap<String, String> = serde_json::from_str(output)?;
    let mut result: Vec<_> = map
        .into_iter()
        .filter(|(k, _)| !DOPPLER_SYSTEM_KEYS.contains(&k.as_str()))
        .collect();
    sort_secret_pairs(&mut result);
    Ok(result)
}
```

## Code Reuse Metrics

### Before Refactoring
- **~200 LOC** duplicated across 13 providers (env var building)
- **~7 instances** of identical sorting logic
- **~40 LOC** per provider for JSON parsing + filtering
- **16+ instances** of `.map(|(k, v)| (*k, v.as_str())).collect()` pattern

### After Refactoring
- **0 LOC** duplicated (moved to trait methods + helper utils)
- **1 shared function** `sort_secret_pairs()`
- **1-2 LOC** per provider for JSON parsing (using `parse_json_secrets()`)
- **1 shared function** `env_refs_from_env()`

**Result:** ~80% LOC reduction in provider-specific code while maintaining identical behavior.

## Testing

### Integration Tests Location
`tests/secrets_provider_tests.rs` contains integration tests for all 13 providers.

### Provider Test Pattern
```rust
#[test]
fn test_vault_build_env() {
    let mut creds = HashMap::new();
    creds.insert("address".to_string(), "http://localhost:8200".to_string());
    creds.insert("token".to_string(), "mytoken".to_string());

    let env = vault::build_env(&creds);
    assert_eq!(env.len(), 2);
    assert_eq!(env[0], ("VAULT_ADDR", "http://localhost:8200".to_string()));
    assert_eq!(env[1], ("VAULT_TOKEN", "mytoken".to_string()));
}
```

**Current Test Coverage:** 1325 tests, 100% passing (includes 150+ provider-specific tests)

## Adding a New Provider

### Step 1: Create Provider File
Create `src/ops/secrets/providers/new_provider.rs`:

```rust
use std::collections::HashMap;
use super::super::provider::{
    env_refs_from_env, run_cli, sort_secret_pairs, SecretProvider, SecretsError,
};

pub struct NewProvider;

impl SecretProvider for NewProvider {
    // Implement trait methods
}
```

### Step 2: Register in Module
Update `src/ops/secrets/providers/mod.rs`:

```rust
pub mod new_provider;

pub use new_provider::NewProvider;
```

### Step 3: Register in Traits
Update `src/ops/secrets/provider.rs` in the provider registration code.

### Step 4: Add Tests
Add integration tests to `tests/secrets_provider_tests.rs`.

### Step 5: Document
Update provider list in README and FEATURES.md.

## Performance Considerations

### Trait Objects vs Generics
The `SecretProvider` trait uses dynamic dispatch (trait objects), appropriate for:
- Runtime provider selection (user chooses provider at runtime)
- Heterogeneous collections (HashMaps of providers)
- Moderate number of providers (13 total)

### Lifetime Patterns
`env_refs_from_env()` uses lifetime tying to avoid unnecessary clones:
```rust
fn env_refs_from_env<'a>(env_vars: &'a [(& 'static str, String)]) -> Vec<(&'static str, &'a str)> {
    env_vars.iter().map(|(k, v)| (*k, v.as_str())).collect()
}
```

**Why:** The `'static` applies to env var name constants (never deallocated), while `&'a str` borrows the String value for the duration of the CLI call.

## Future Enhancements

### 1. Async Provider Support
Current: Synchronous CLI calls via `Command`
Future: Native async HTTP clients for direct API calls

### 2. Batching and Streaming
Current: All secrets loaded into memory
Future: Streaming interface for large secret stores

### 3. Hot Reload
Current: Static configuration per session
Future: Watch configuration for changes and reload

### 4. Provider Chaining
Current: Single provider per operation
Future: Multi-provider fallback (try Vault, fall back to AWS SSM if offline)

## Debugging

### Enable CLI Output
Providers call `run_cli()` which executes external binaries. To debug:

```rust
// In provider code, before run_cli():
eprintln!("Running: {} {:?}", binary, args);
eprintln!("Environment: {:?}", env_refs);
```

### Test with Real Providers
For integration testing, set up test instances:

```bash
# Vault
docker run -p 8200:8200 vault server -dev

# LocalStack (AWS)
docker run -p 4566:4566 localstack/localstack

# Azure Emulator
az cosmosdb emulator start
```

## Common Issues and Solutions

### Issue: "Expected object at path"
**Cause:** JSON structure doesn't match expected path
**Solution:** Verify provider's API returns expected structure, adjust path in `extract_json_object()`

### Issue: System keys appearing in secrets
**Cause:** Provider injects non-user secrets
**Solution:** Add filter in `parse_*_output()` function (see Doppler example)

### Issue: Sorting inconsistency
**Cause:** Manual sort logic in provider
**Solution:** Always use `sort_secret_pairs()` or let it handle sorting