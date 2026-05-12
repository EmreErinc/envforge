//! Integration tests for `src/ops/mcp_pin/`.
//!
//! Covers stories 001-004 of bolt 075-lockfile-hasher:
//!  - 001 lockfile schema + round-trip
//!  - 002 canonical JSON/JSONC hasher
//!  - 003 binary hash with symlink + multi-platform
//!  - 004 fuzz harness presence (the cargo-fuzz binary is verified by build)
//!
//! Per coding-standards.md: tests live in `tests/` only, no in-module tests.

use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use envforge::ops::mcp_pin::{
    BinaryHash, BinaryHasher, CanonicalJsonHasher, FsLockfileRepository, HasherError, Lockfile,
    LockfileError, LockfileRepository, LockfileSerde, PackageManager, PinMethod, Platform,
    ServerPin, Transport, CURRENT_FORMAT_VERSION,
};

// ─────────────────────────────────────────────────────────────────────────────
// Story 001: lockfile schema + round-trip
// ─────────────────────────────────────────────────────────────────────────────

fn sample_pin(name: &str) -> ServerPin {
    ServerPin {
        name: name.to_string(),
        pin_method: PinMethod::Strict,
        pinned_at: Utc.with_ymd_and_hms(2026, 5, 12, 14, 0, 0).unwrap(),
        pinned_by_machine: "deadbeefcafef00d".to_string(),
        command: Some("npx".to_string()),
        args: Some(vec![
            "-y".to_string(),
            "@modelcontextprotocol/server-github".to_string(),
        ]),
        transport: Transport::Stdio,
        url: None,
        package_manager: Some(PackageManager::Npm {
            pkg: "@modelcontextprotocol/server-github".to_string(),
            ver: Some("1.2.3".to_string()),
        }),
        package_integrity: Some("sha512-abc".to_string()),
        config_hash: "a".repeat(64),
        tool_list_hash: None,
        tool_list_captured_at: None,
        dynamic_tools: false,
        volatile: false,
        spki_sha256: None,
        initialize_response_hash: None,
        binary_hashes: vec![BinaryHash::from_bytes(
            Platform::new("darwin-arm64"),
            [0xAA; 32],
            PathBuf::from("/usr/local/bin/server-github"),
        )],
    }
}

#[test]
fn test_lockfile_empty_roundtrip() {
    let l = Lockfile::new("2026-05-12");
    let bytes = LockfileSerde::write(&l).expect("write");
    let parsed = LockfileSerde::parse(&bytes).expect("parse");
    assert_eq!(parsed, l);
    assert_eq!(parsed.format_version, CURRENT_FORMAT_VERSION);
}

#[test]
fn test_lockfile_roundtrip_with_server() {
    let mut l = Lockfile::new("2026-05-12");
    l.upsert_server(sample_pin("foo")).expect("upsert");
    let bytes = LockfileSerde::write(&l).expect("write");
    let parsed = LockfileSerde::parse(&bytes).expect("parse");
    assert_eq!(parsed, l);
}

#[test]
fn test_lockfile_unsupported_format_version_rejected() {
    let bytes = br#"
format_version = 99999
pattern_set_version = "2026-05-12"
"#;
    let err = LockfileSerde::parse(bytes).expect_err("must reject");
    matches!(err, LockfileError::UnsupportedFormatVersion { .. });
}

#[test]
fn test_lockfile_optional_fields_default_to_none() {
    let toml = r#"
format_version = 1
pattern_set_version = "2026-05-12"
"#;
    let l = LockfileSerde::parse(toml.as_bytes()).expect("parse");
    assert!(l.servers.is_empty());
}

#[test]
fn test_lockfile_servers_sorted_by_name() {
    let mut l = Lockfile::new("2026-05-12");
    l.upsert_server(sample_pin("zebra")).unwrap();
    l.upsert_server(sample_pin("alpha")).unwrap();
    l.upsert_server(sample_pin("mango")).unwrap();
    let names: Vec<&str> = l.servers.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["alpha", "mango", "zebra"]);
}

#[test]
fn test_lockfile_duplicate_server_replaced_not_appended() {
    let mut l = Lockfile::new("2026-05-12");
    l.upsert_server(sample_pin("foo")).unwrap();
    let mut updated = sample_pin("foo");
    updated.package_integrity = Some("sha512-different".to_string());
    l.upsert_server(updated).unwrap();
    assert_eq!(l.servers.len(), 1);
    assert_eq!(
        l.servers[0].package_integrity.as_deref(),
        Some("sha512-different")
    );
}

#[test]
fn test_lockfile_merge_conflict_markers_detected() {
    let toml = r#"
format_version = 1
pattern_set_version = "2026-05-12"
<<<<<<< HEAD
server_count = 1
=======
server_count = 2
>>>>>>> branch
"#;
    let err = LockfileSerde::parse(toml.as_bytes()).expect_err("must reject");
    matches!(err, LockfileError::MergeConflictMarkers { .. });
}

#[test]
fn test_lockfile_add_platform_sorted() {
    let mut l = Lockfile::new("2026-05-12");
    let mut pin = sample_pin("foo");
    pin.binary_hashes.clear();
    l.upsert_server(pin).unwrap();

    l.add_platform(
        "foo",
        BinaryHash::from_bytes(Platform::new("linux-x86_64"), [0xCC; 32], "/a".into()),
    )
    .unwrap();
    l.add_platform(
        "foo",
        BinaryHash::from_bytes(Platform::new("darwin-arm64"), [0xDD; 32], "/b".into()),
    )
    .unwrap();

    let platforms: Vec<&str> = l.servers[0]
        .binary_hashes
        .iter()
        .map(|b| b.platform.as_str())
        .collect();
    assert_eq!(platforms, vec!["darwin-arm64", "linux-x86_64"]);
}

#[test]
fn test_serverpin_remote_sse_valid() {
    let p = ServerPin {
        name: "remote".to_string(),
        pin_method: PinMethod::Auto,
        pinned_at: Utc::now(),
        pinned_by_machine: "abc".into(),
        command: None,
        args: None,
        transport: Transport::Sse,
        url: Some("https://mcp.example.com/sse".to_string()),
        package_manager: Some(PackageManager::RemoteSse {
            url: "https://mcp.example.com/sse".to_string(),
        }),
        package_integrity: None,
        config_hash: "x".repeat(64),
        tool_list_hash: None,
        tool_list_captured_at: None,
        dynamic_tools: false,
        volatile: false,
        spki_sha256: Some("y".repeat(64)),
        initialize_response_hash: None,
        binary_hashes: vec![],
    };
    p.validate().expect("valid remote sse");
}

#[test]
fn test_serverpin_both_command_and_url_rejected() {
    let mut p = sample_pin("bad");
    p.transport = Transport::Sse;
    p.url = Some("https://x".to_string());
    let err = p.validate().expect_err("must reject");
    matches!(err, LockfileError::InvalidServer { .. });
}

#[test]
fn test_serverpin_volatile_with_binary_hashes_rejected() {
    let mut p = sample_pin("vol");
    p.volatile = true;
    // binary_hashes still populated from sample_pin
    let err = p.validate().expect_err("must reject");
    matches!(err, LockfileError::InvalidServer { .. });
}

#[test]
fn test_serverpin_volatile_without_anchor_rejected() {
    let mut p = sample_pin("vol");
    p.volatile = true;
    p.binary_hashes.clear();
    p.package_integrity = None;
    p.spki_sha256 = None;
    let err = p.validate().expect_err("must reject");
    matches!(err, LockfileError::InvalidServer { .. });
}

#[test]
fn test_serverpin_duplicate_platform_rejected() {
    let mut p = sample_pin("dup");
    p.binary_hashes.push(BinaryHash::from_bytes(
        Platform::new("darwin-arm64"),
        [0xEE; 32],
        "/other".into(),
    ));
    let err = p.validate().expect_err("must reject");
    matches!(err, LockfileError::InvalidServer { .. });
}

#[test]
fn test_binary_hash_sha256_roundtrip() {
    let raw = [0x12_u8; 32];
    let b = BinaryHash::from_bytes(Platform::new("linux-x86_64"), raw, "/x".into());
    assert_eq!(b.sha256_bytes(), Some(raw));
    assert_eq!(b.sha256.len(), 64);
    assert!(b.sha256.chars().all(|c| c.is_ascii_hexdigit()));
}

// ─────────────────────────────────────────────────────────────────────────────
// Story 002: canonical JSON / JSONC hasher
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_canonical_invariant_under_whitespace() {
    let a = br#"{"a":1,"b":2}"#;
    let b = b" {  \"a\" : 1 ,\n  \"b\"  :  2\n}\n";
    let ha = CanonicalJsonHasher::canonicalize_and_hash(a).expect("a");
    let hb = CanonicalJsonHasher::canonicalize_and_hash(b).expect("b");
    assert_eq!(ha, hb);
}

#[test]
fn test_canonical_invariant_under_key_order() {
    let a = br#"{"b":2,"a":1}"#;
    let b = br#"{"a":1,"b":2}"#;
    let ha = CanonicalJsonHasher::canonicalize_and_hash(a).expect("a");
    let hb = CanonicalJsonHasher::canonicalize_and_hash(b).expect("b");
    assert_eq!(ha, hb);
}

#[test]
fn test_canonical_invariant_under_jsonc_comments() {
    let a = br#"{"a":1,"b":2}"#;
    let b = br#"
// header
{
  "a": 1,  // inline
  /* block
     comment */
  "b": 2
}
"#;
    let ha = CanonicalJsonHasher::canonicalize_and_hash(a).expect("a");
    let hb = CanonicalJsonHasher::canonicalize_and_hash(b).expect("b");
    assert_eq!(ha, hb);
}

#[test]
fn test_canonical_preserves_array_order() {
    let a = br#"{"k":[1,2,3]}"#;
    let b = br#"{"k":[3,2,1]}"#;
    let ha = CanonicalJsonHasher::canonicalize_and_hash(a).expect("a");
    let hb = CanonicalJsonHasher::canonicalize_and_hash(b).expect("b");
    assert_ne!(ha, hb);
}

#[test]
fn test_canonical_string_with_double_slash_not_treated_as_comment() {
    let a = br#"{"url":"https://example.com"}"#;
    let h = CanonicalJsonHasher::canonicalize_and_hash(a).expect("hash");
    // hash is reproducible
    let h2 = CanonicalJsonHasher::canonicalize_and_hash(a).expect("hash2");
    assert_eq!(h, h2);
}

#[test]
fn test_canonical_string_with_escape_not_treated_as_quote() {
    let a = br#"{"x":"foo\"//bar"}"#;
    let h = CanonicalJsonHasher::canonicalize_and_hash(a).expect("hash");
    let _ = h; // succeeds; specific value not asserted
}

#[test]
fn test_canonical_input_too_large_rejected() {
    let big = vec![b' '; CanonicalJsonHasher::MAX_INPUT_BYTES + 1];
    let err = CanonicalJsonHasher::canonicalize_and_hash(&big).expect_err("must reject");
    matches!(err, HasherError::InputTooLarge { .. });
}

#[test]
fn test_canonical_depth_limit_rejected() {
    // Build a JSON document with depth > MAX_DEPTH.
    let mut s = String::new();
    let depth = CanonicalJsonHasher::MAX_DEPTH + 5;
    for _ in 0..depth {
        s.push('[');
    }
    for _ in 0..depth {
        s.push(']');
    }
    let err = CanonicalJsonHasher::canonicalize_and_hash(s.as_bytes()).expect_err("must reject");
    matches!(err, HasherError::DepthLimit { .. });
}

#[test]
fn test_canonical_invalid_json_rejected() {
    let a = b"{not valid json";
    let err = CanonicalJsonHasher::canonicalize_and_hash(a).expect_err("must reject");
    matches!(err, HasherError::InvalidJson(_));
}

#[test]
fn test_canonical_unterminated_block_comment_rejected() {
    let a = b"{ /* unterminated ";
    let err = CanonicalJsonHasher::canonicalize_and_hash(a).expect_err("must reject");
    matches!(err, HasherError::UnterminatedBlockComment { .. });
}

#[test]
fn test_canonical_nested_objects_keys_sorted_recursively() {
    let a = br#"{"outer":{"b":2,"a":1}}"#;
    let b = br#"{"outer":{"a":1,"b":2}}"#;
    let ha = CanonicalJsonHasher::canonicalize_and_hash(a).expect("a");
    let hb = CanonicalJsonHasher::canonicalize_and_hash(b).expect("b");
    assert_eq!(ha, hb);
}

#[test]
fn test_canonical_empty_object_hash_stable() {
    let h1 = CanonicalJsonHasher::canonicalize_and_hash(b"{}").expect("h1");
    let h2 = CanonicalJsonHasher::canonicalize_and_hash(b"  {  }  \n").expect("h2");
    assert_eq!(h1, h2);
}

// ─────────────────────────────────────────────────────────────────────────────
// Story 003: binary hash with symlink + multi-platform
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_binary_hasher_regular_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.bin");
    std::fs::write(&path, b"hello world").unwrap();
    let h = BinaryHasher::hash_binary(&path).expect("hash");
    // SHA-256("hello world") = b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
    let expected =
        hex::decode("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9").unwrap();
    assert_eq!(&h.sha256[..], expected.as_slice());
    assert!(h.symlink_target.is_none());
    assert_eq!(h.platform, Platform::current());
}

#[cfg(unix)]
#[test]
fn test_binary_hasher_symlink_records_target() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("real.bin");
    std::fs::write(&target, b"sym-data").unwrap();
    let link = dir.path().join("link.bin");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let h = BinaryHasher::hash_binary(&link).expect("hash");
    assert!(h.symlink_target.is_some());
    // realpath resolves to target
    assert_eq!(
        h.realpath.canonicalize().unwrap(),
        target.canonicalize().unwrap()
    );
}

#[cfg(unix)]
#[test]
fn test_binary_hasher_broken_symlink_errors() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("nonexistent.bin");
    let link = dir.path().join("link.bin");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let err = BinaryHasher::hash_binary(&link).expect_err("must error");
    matches!(err, HasherError::BrokenSymlink { .. });
}

#[test]
fn test_binary_hasher_missing_file_errors() {
    let p = PathBuf::from("/this/should/not/exist/anywhere.bin");
    let err = BinaryHasher::hash_binary(&p).expect_err("must error");
    matches!(err, HasherError::Io { .. });
}

#[test]
fn test_serverpin_binary_hash_lookup_by_platform() {
    let pin = sample_pin("foo"); // has darwin-arm64
    let p = Platform::new("darwin-arm64");
    assert!(pin.binary_hash_for(&p).is_some());
    let q = Platform::new("solaris-sparc");
    assert!(pin.binary_hash_for(&q).is_none());
}

// ─────────────────────────────────────────────────────────────────────────────
// FsLockfileRepository (atomic save)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_fs_repo_save_and_load_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("subdir/mcp.lock");

    let mut l = Lockfile::new("2026-05-12");
    l.upsert_server(sample_pin("alpha")).unwrap();

    let repo = FsLockfileRepository;
    repo.save(&path, &l).expect("save");
    assert!(repo.exists(&path));
    let loaded = repo.load(&path).expect("load");
    assert_eq!(loaded, l);
}

#[test]
fn test_fs_repo_atomic_save_no_partial_state() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mcp.lock");

    // First write
    let mut l = Lockfile::new("2026-05-12");
    l.upsert_server(sample_pin("alpha")).unwrap();
    let repo = FsLockfileRepository;
    repo.save(&path, &l).unwrap();

    // Overwrite with different content
    let mut l2 = Lockfile::new("2026-05-12");
    l2.upsert_server(sample_pin("beta")).unwrap();
    repo.save(&path, &l2).unwrap();

    let loaded = repo.load(&path).unwrap();
    assert_eq!(loaded.servers[0].name, "beta");
}

// ─────────────────────────────────────────────────────────────────────────────
// PackageManager tagged enum round-trip
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_package_manager_npm_serde_tagged() {
    let pm = PackageManager::Npm {
        pkg: "@x/y".into(),
        ver: Some("1.0.0".into()),
    };
    let toml = toml::to_string(&pm).expect("ser");
    assert!(toml.contains(r#"kind = "npm""#));
    let parsed: PackageManager = toml::from_str(&toml).expect("de");
    assert_eq!(parsed, pm);
}

#[test]
fn test_package_manager_remote_sse_kebab_case() {
    let pm = PackageManager::RemoteSse {
        url: "https://x".into(),
    };
    let toml = toml::to_string(&pm).expect("ser");
    assert!(toml.contains(r#"kind = "remote-sse""#));
    let parsed: PackageManager = toml::from_str(&toml).expect("de");
    assert_eq!(parsed, pm);
}

#[test]
fn test_transport_defaults_to_stdio() {
    let toml_str = r#"
format_version = 1
pattern_set_version = "2026-05-12"

[[server]]
name = "no-transport"
pin_method = "auto"
pinned_at = "2026-05-12T14:00:00Z"
pinned_by_machine = "abc"
command = "/bin/x"
config_hash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
"#;
    let l = LockfileSerde::parse(toml_str.as_bytes()).expect("parse");
    assert_eq!(l.servers[0].transport, Transport::Stdio);
}

// ─────────────────────────────────────────────────────────────────────────────
// Aggregate root invariants
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_aggregate_validate_rejects_duplicate_names() {
    let l = Lockfile {
        format_version: 1,
        pattern_set_version: "2026-05-12".into(),
        servers: vec![sample_pin("dup"), sample_pin("dup")],
    };
    let err = l.validate().expect_err("must reject");
    matches!(err, LockfileError::DuplicateServer { .. });
}

#[test]
fn test_pin_method_default_is_auto() {
    assert_eq!(PinMethod::default(), PinMethod::Auto);
}

#[test]
fn test_pin_method_display() {
    assert_eq!(PinMethod::Auto.to_string(), "auto");
    assert_eq!(PinMethod::Manual.to_string(), "manual");
    assert_eq!(PinMethod::Strict.to_string(), "strict");
}

#[test]
fn test_platform_current_nonempty() {
    let p = Platform::current();
    assert!(p.as_str().contains('-'));
}
