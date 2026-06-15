# EnvForge v0.8.0 NFR Audit Remediation Plan

**Status:** 🚀 READY TO EXECUTE  
**Target:** Launch-Ready in 13-16 hours  
**Date Created:** 2026-06-07  
**Owner:** Remediation Team  

---

## Executive Summary

3 critical blockers prevent launch. All are fixable with focused 13-16 hour sprint. This document provides **step-by-step implementation** for each issue with code examples.

**Launch Readiness Sequence:**
1. ✅ Fix encryption test (1 hr) — FOUNDATION
2. ✅ Add LSP audit logging (2-3 hrs) — SECURITY
3. ✅ Code-sign plugins (2-3 hrs) — SUPPLY CHAIN
4. ✅ Add LSP load test (3-4 hrs) — PERFORMANCE
5. ✅ Add benchmark CI gate (1-2 hrs) — REGRESSION
6. ✅ Run full test suite (1 hr) — VALIDATION
7. 🚀 **LAUNCH** ✅

---

## BLOCKER #1: Fix Encryption Test Environment Contamination

**Severity:** 🚨 CRITICAL  
**Time:** 1 hour  
**File:** `tests/encryption_invariant_tests.rs`  

### Problem
Test `test_env_age_key_empty_is_rejected` fails under `cargo test` (parallel) but passes with `cargo test -- --test-threads=1` (serial).

**Root Cause:** Previous test `test_env_age_key_invalid_key_fails_encrypt` sets `ENVFORGE_AGE_KEY` to an invalid value but doesn't restore it properly. When tests run in parallel, this env var bleeds into other tests.

### Solution: Use `serial_test` Crate

#### Step 1: Add Dependency
```bash
cd /Users/emreerinc/projects/envforge
cargo add --dev serial_test
```

#### Step 2: Modify Test File
Edit `tests/encryption_invariant_tests.rs`:

```rust
// Add to imports
use serial_test::serial;

// Wrap BOTH tests that manipulate ENVFORGE_AGE_KEY
#[test]
#[serial]  // <-- Add this attribute
fn test_env_age_key_invalid_key_fails_encrypt() {
    std::env::set_var("ENVFORGE_AGE_KEY", "this-is-not-a-valid-age-key");
    let result = envforge::ops::encrypt::ensure_age_key();
    assert!(result.is_err(), "invalid age key must be rejected");
    std::env::remove_var("ENVFORGE_AGE_KEY");  // Ensure cleanup
}

#[test]
#[serial]  // <-- Add this attribute
fn test_env_age_key_empty_is_rejected() {
    std::env::set_var("ENVFORGE_AGE_KEY", "");
    let result = envforge::ops::encrypt::ensure_age_key();
    assert!(
        result.is_err(),
        "empty ENVFORGE_AGE_KEY must be rejected, got: {:?}",
        result
    );
    std::env::remove_var("ENVFORGE_AGE_KEY");  // Ensure cleanup
}
```

#### Step 3: Verify Fix
```bash
# Test passes in parallel now
cargo test test_env_age_key_empty_is_rejected

# Test passes in serial (as before)
cargo test test_env_age_key_empty_is_rejected -- --test-threads=1

# Full encryption invariant suite passes
cargo test --test encryption_invariant_tests
```

#### Step 4: Alternative Solution (if serial_test causes issues)
Use a per-test mutex instead:

```rust
// Add to top of file
use std::sync::Mutex;
lazy_static::lazy_static! {
    static ref ENV_TEST_LOCK: Mutex<()> = Mutex::new(());
}

#[test]
fn test_env_age_key_invalid_key_fails_encrypt() {
    let _lock = ENV_TEST_LOCK.lock().unwrap();
    std::env::set_var("ENVFORGE_AGE_KEY", "this-is-not-a-valid-age-key");
    let result = envforge::ops::encrypt::ensure_age_key();
    assert!(result.is_err(), "invalid age key must be rejected");
    std::env::remove_var("ENVFORGE_AGE_KEY");
}
```

**Recommended:** Use `serial_test` (simpler, cleaner).

---

## BLOCKER #2: Code-Sign LSP Plugins

**Severity:** 🚨 CRITICAL  
**Time:** 2-3 hours  
**Files:** `editors/vscode/`, `editors/intellij/`  

### Problem
VS Code and IntelliJ plugins lack code signatures. An attacker could publish a malicious plugin fork that steals all `.env` files.

### Solution A: VS Code Extension (1-1.5 hours)

#### Step 1: Obtain Microsoft Certificate
```bash
# Option A: Personal Publisher ID (free)
vsce create-publisher emreerinc

# Option B: Organization Publisher ID (recommended for production)
# Register at https://marketplace.visualstudio.com
# Create publisher account
```

#### Step 2: Update package.json
Edit `editors/vscode/package.json`:

```json
{
  "publisher": "emreerinc",
  "name": "envforge",
  "version": "0.8.0",
  "displayName": "EnvForge - AI-Safe Environment Manager",
  "description": "Terminal environment variable manager with AI safety",
  "repository": {
    "type": "git",
    "url": "https://github.com/emreerinc/envforge.git"
  },
  "homepage": "https://github.com/emreerinc/envforge",
  "bugs": {
    "url": "https://github.com/emreerinc/envforge/issues"
  },
  "galleryBanner": {
    "color": "#1e1e1e",
    "theme": "dark"
  }
}
```

#### Step 3: Package and Sign
```bash
cd /Users/emreerinc/projects/envforge/editors/vscode

# Install VSCE if needed
npm install -g @vscode/vsce

# Package with signature
vsce package --pre-release

# Output: envforge-0.8.0.vsix
```

#### Step 4: Publish to Marketplace
```bash
# Get personal access token from https://marketplace.visualstudio.com
# Save as ~/.vsce (permissions: all)

vsce publish --packagePath ./envforge-0.8.0.vsix

# Verify published
vsce search --publisher emreerinc envforge
```

#### Step 5: Verify Signature
```bash
# Download from marketplace and verify
# The .vsix file will have: integrity hash + timestamp in marketplace

# Users can verify via VS Code:
# Extensions → EnvForge → Check "Verified Publisher"
```

### Solution B: IntelliJ Plugin (1-1.5 hours)

#### Step 1: Create JetBrains Account
Go to https://plugins.jetbrains.com/ and register organization account.

#### Step 2: Generate Private Key
```bash
cd /Users/emreerinc/projects/envforge/editors/intellij

# Generate key pair (store securely)
openssl genrsa -out privateKey.pem 2048
openssl pkcs8 -topk8 -inform PEM -in privateKey.pem -out privateKey.pkcs8 -nocrypt

# Store privateKey.pkcs8 securely (add to .gitignore)
echo "privateKey.pkcs8" >> .gitignore
```

#### Step 3: Update gradle.properties
Edit `gradle.properties`:

```properties
# JetBrains Marketplace
publishPlugin.token=<YOUR_JWT_TOKEN>

# Signing
signPlugin.certificateChain=<CERT_CHAIN>
signPlugin.privateKey=<PRIVATE_KEY_PATH>
signPlugin.password=<KEY_PASSWORD>
```

#### Step 4: Build and Sign
```bash
# Build plugin
./gradlew buildPlugin

# Sign plugin
./gradlew signPlugin

# Publish to marketplace
./gradlew publishPlugin

# Output: IntelliJ plugin published with signature
```

#### Step 5: Verify in Marketplace
```bash
# Verify at https://plugins.jetbrains.com/plugin/envforge
# Check: "Code Signature Verified" badge
```

### Testing Both Plugins

```bash
# Test VS Code extension
code --install-extension ./envforge-0.8.0.vsix

# Test IntelliJ plugin
# Download from JetBrains marketplace in IDE → Settings → Plugins

# Verify no security warnings in IDE
# Verify LSP server starts correctly: envforge lsp
```

---

## BLOCKER #3: Implement LSP Audit Logging

**Severity:** 🚨 CRITICAL  
**Time:** 2-3 hours  
**Files:** `src/lsp/security.rs` (new), `src/lsp/mod.rs` (modify)  

### Problem
LSP server has no audit trail for secret access. Can't detect if credentials were accessed via IDE.

### Solution: Add JSONL Audit Log

#### Step 1: Create LSP Security Module
Create `src/lsp/security.rs`:

```rust
use serde_json::json;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use chrono::Local;

pub struct LspAuditLogger {
    log_path: PathBuf,
}

impl LspAuditLogger {
    pub fn new() -> Result<Self, std::io::Error> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Config directory not found",
            ))?;
        
        let envforge_dir = config_dir.join("envforge");
        std::fs::create_dir_all(&envforge_dir)?;
        
        let log_path = envforge_dir.join("lsp-audit.log");
        Ok(LspAuditLogger { log_path })
    }

    pub fn log_operation(
        &self,
        operation: &str,
        file_path: &str,
        keys_accessed: &[String],
        status: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let entry = json!({
            "timestamp": Local::now().to_rfc3339(),
            "operation": operation,
            "file_path": file_path,
            "keys_accessed": keys_accessed,
            "status": status,
        });

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;

        writeln!(file, "{}", entry.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_log_creation() {
        let logger = LspAuditLogger::new().unwrap();
        logger.log_operation(
            "hover",
            ".env",
            &["DATABASE_URL".to_string()],
            "success",
        ).unwrap();
        
        // Verify log file exists
        assert!(logger.log_path.exists());
    }
}
```

#### Step 2: Update LSP Module
Edit `src/lsp/mod.rs` to include security module:

```rust
pub mod security;

// ... existing code ...
```

#### Step 3: Instrument LSP Server
Edit `src/lsp/server.rs` to add logging to key endpoints:

```rust
// Add to imports
use crate::lsp::security::LspAuditLogger;

// In LanguageServer struct
pub struct LanguageServer {
    // ... existing fields ...
    audit_logger: LspAuditLogger,
}

// In initialization
impl LanguageServer {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // ... existing initialization ...
        
        let audit_logger = LspAuditLogger::new()?;
        
        Ok(Self {
            // ... existing fields ...
            audit_logger,
        })
    }
}

// Instrument hover endpoint (around line 751)
pub fn hover(&mut self, params: HoverParams) -> Result<Option<Hover>> {
    let file_path = params.text_document_position_params
        .text_document
        .uri
        .to_file_path()
        .ok()?;
    
    let keys_accessed = extract_keys_from_hover_position(&params);
    
    // LOG: User hovered over these keys
    let _ = self.audit_logger.log_operation(
        "hover",
        &file_path.display().to_string(),
        &keys_accessed,
        "success",
    );
    
    // ... existing hover logic ...
}

// Instrument completion endpoint (around line 783)
pub fn completion(&mut self, params: CompletionParams) -> Result<Option<CompletionList>> {
    let file_path = params.text_document_position
        .text_document
        .uri
        .to_file_path()
        .ok()?;
    
    let keys_suggested = extract_suggested_keys(&params);
    
    // LOG: Completion suggestions were shown
    let _ = self.audit_logger.log_operation(
        "completion",
        &file_path.display().to_string(),
        &keys_suggested,
        "success",
    );
    
    // ... existing completion logic ...
}

// Similar for: document_symbol, goto_definition, semantic_tokens
```

#### Step 4: Test Audit Logging

Create `tests/lsp_audit_tests.rs`:

```rust
#[test]
fn test_lsp_audit_log_creation() {
    // Start LSP server
    let lsp = envforge::lsp::server::LanguageServer::new().unwrap();
    
    // Perform operations
    // Verify audit.log contains entries
    let log_path = dirs::config_dir().unwrap().join("envforge/lsp-audit.log");
    assert!(log_path.exists());
    
    let contents = std::fs::read_to_string(&log_path).unwrap();
    assert!(contents.contains("hover"));
    assert!(contents.contains("completion"));
}
```

#### Step 5: Verify Audit Trail
```bash
# Run LSP server and interact via VS Code
envforge lsp

# Check audit log
tail -50 ~/.config/envforge/lsp-audit.log

# Expected output (JSONL format):
# {"timestamp":"2026-06-07T01:15:30+03:00","operation":"hover","file_path":".env","keys_accessed":["DATABASE_URL"],"status":"success"}
# {"timestamp":"2026-06-07T01:15:35+03:00","operation":"completion","file_path":".env","keys_accessed":["API_KEY","DB_HOST"],"status":"success"}
```

---

## HIGH-PRIORITY ADDITIONS

### P1.1: Add LSP Load Test

**Time:** 3-4 hours  
**File:** `tests/lsp_load_tests.rs` (new)  

```rust
#[tokio::test]
async fn test_lsp_load_10_concurrent_editors() {
    use std::time::Instant;
    
    let mut server = LspServer::new().await.unwrap();
    let mut tasks = vec![];
    
    // Simulate 10 concurrent editors
    for i in 0..10 {
        let server = server.clone();
        let task = tokio::spawn(async move {
            let start = Instant::now();
            
            // Hover request
            let hover_result = server.hover(HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier {
                        uri: format!("file://.env{}", i).parse().unwrap(),
                    },
                    position: Position { line: 0, character: 0 },
                },
            }).await;
            
            let elapsed = start.elapsed();
            assert!(elapsed.as_millis() < 100, "hover response >100ms (SLA violated)");
            assert!(hover_result.is_ok());
            
            elapsed
        });
        tasks.push(task);
    }
    
    // Wait for all tasks
    let results = futures::future::join_all(tasks).await;
    
    // Verify all under SLA
    for result in results {
        let elapsed = result.unwrap();
        assert!(elapsed.as_millis() < 100, "LSP response exceeded 100ms SLA");
    }
}

#[tokio::test]
async fn test_lsp_completion_latency_p95() {
    // Run 100 completion requests, measure p95 latency
    let mut latencies = vec![];
    
    for _ in 0..100 {
        let start = Instant::now();
        let _ = server.completion(/* ... */).await;
        latencies.push(start.elapsed().as_millis());
    }
    
    latencies.sort();
    let p95 = latencies[(latencies.len() * 95) / 100];
    
    assert!(p95 < 200, "Completion p95 latency {} > 200ms", p95);
}
```

### P1.2: Add Benchmark CI Gate

**Time:** 1-2 hours  
**Files:** `.github/workflows/ci.yml` (modify), `benches/benchmarks.rs` (verify)  

Edit `.github/workflows/ci.yml`:

```yaml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
      
      - name: Run tests
        run: cargo test --all
      
      - name: Run benchmarks
        run: cargo bench --bench benchmarks -- --output-format bencher | tee output.txt
      
      - name: Store benchmark result
        uses: benchmark-action/github-action-benchmark@v1
        with:
          tool: 'cargo'
          output-file-path: output.txt
          github-token: ${{ secrets.GITHUB_TOKEN }}
          auto-push: true
          alert-threshold: '110%'  # Fail if regression > 10%
```

---

## Verification Checklist

After implementing all fixes:

```bash
# 1. Fix encryption test
cargo test test_env_age_key_empty_is_rejected -- --nocapture
# Expected: PASS ✅

# 2. Verify full encryption suite
cargo test --test encryption_invariant_tests
# Expected: 19/19 PASS ✅

# 3. Verify VS Code plugin
vsce search --publisher emreerinc envforge
# Expected: Published, signed ✅

# 4. Verify IntelliJ plugin
# Check: https://plugins.jetbrains.com/plugin/envforge
# Expected: Published, code signed ✅

# 5. Verify LSP audit logging
tail ~/.config/envforge/lsp-audit.log
# Expected: JSONL entries with operations ✅

# 6. Run LSP load test
cargo test test_lsp_load_10_concurrent_editors -- --nocapture
# Expected: All responses <100ms ✅

# 7. Full test suite
cargo fmt --check
cargo clippy -- -D warnings
cargo test --lib
cargo test --test '*'
# Expected: All PASS ✅

# 8. Run benchmarks
cargo bench --bench benchmarks -- --verbose
# Expected: Parser <1000ms for 1K exports ✅
```

---

## Timeline & Execution Order

| Phase | Tasks | Time | Dependencies |
|-------|-------|------|--------------|
| **Phase 1** | Fix encryption test | 1 hr | None |
| **Phase 2** | Add LSP audit logging | 2-3 hrs | Phase 1 ✓ |
| **Phase 3** | Code-sign plugins | 2-3 hrs | Phase 1 ✓ |
| **Phase 4** | Add LSP load test | 3-4 hrs | Phase 1-3 ✓ |
| **Phase 5** | Add benchmark CI gate | 1-2 hrs | Phase 1-4 ✓ |
| **Phase 6** | Full validation | 1 hr | Phase 1-5 ✓ |
| **LAUNCH** | 🚀 | **~13-16 hrs** | All phases ✓ |

---

## Risk Mitigation

| Risk | Mitigation | Confidence |
|------|-----------|------------|
| Test fix breaks other tests | Run full suite after each fix | HIGH |
| Plugin signing fails | Test with sandbox account first | HIGH |
| Audit log I/O overhead | Use async logging, buffer writes | MEDIUM |
| LSP performance regression | Benchmark before/after | HIGH |
| CI gate is too strict | Set 10% threshold (not 5%) | HIGH |

---

## Success Criteria

✅ All 3 blockers resolved  
✅ 956 lib tests + 72KB integration tests PASS  
✅ Benchmark CI gate enabled (no regressions)  
✅ LSP plugins signed and published  
✅ LSP audit logging captures all operations  
✅ LSP load test passes (<100ms p95)  

**Status:** READY TO LAUNCH 🚀

---

**Document Version:** 1.0  
**Last Updated:** 2026-06-07  
**Owner:** Remediation Team  
**Status:** EXECUTION-READY
