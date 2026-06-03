//! EnvForge performance benchmarks.
//!
//! Run with: `cargo bench`
//! Run specific: `cargo bench -- parser_roundtrip`

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_parser_roundtrip_small(c: &mut Criterion) {
    let content = "export DATABASE_URL=\"postgres://localhost/mydb\"\nREDIS_HOST=localhost\n";
    c.bench_function("parser/roundtrip_small", |b| {
        b.iter(|| {
            let sf = envforge::parser::parse_shell_content(
                black_box(content),
                black_box(std::path::Path::new("/test/.zshrc")),
            )
            .unwrap();
            let _output = sf.serialize();
        });
    });
}

fn bench_parser_roundtrip_large(c: &mut Criterion) {
    // Build a large shell file with 500 exports
    let content: String = (0..500)
        .map(|i| format!("export VAR_{i:04}=\"value_{i:08}_with_some_longer_padding\"\n"))
        .collect::<Vec<_>>()
        .join("");
    c.bench_function("parser/roundtrip_large_500_exports", |b| {
        b.iter(|| {
            let sf = envforge::parser::parse_shell_content(
                black_box(&content),
                black_box(std::path::Path::new("/test/.zshrc")),
            )
            .unwrap();
            let _output = sf.serialize();
        });
    });
}

fn bench_sync_diff_small(c: &mut Criterion) {
    use envforge::ops::sync::model::SyncEntry;
    let local: Vec<(String, String)> = vec![
        ("DATABASE_URL".into(), "localhost".into()),
        ("REDIS_HOST".into(), "localhost".into()),
    ];
    let snapshot: Vec<SyncEntry> = vec![
        SyncEntry {
            key: "DATABASE_URL".into(),
            value: "production.example.com".into(),
            profile: None,
            group: None,
        },
        SyncEntry {
            key: "REDIS_HOST".into(),
            value: "localhost".into(),
            profile: None,
            group: None,
        },
    ];
    c.bench_function("sync/compute_diff_small", |b| {
        b.iter(|| {
            let _diff =
                envforge::ops::sync::diff::compute_diff(black_box(&local), black_box(&snapshot));
        });
    });
}

fn bench_secret_redaction(c: &mut Criterion) {
    let msg = "The secret is sk-abc123def456 and also gh_token_xyz789 in this message";
    let secrets: Vec<String> = vec!["sk-abc123def456".to_string(), "gh_token_xyz789".to_string()];
    c.bench_function("security/redact_secrets_in_message", |b| {
        b.iter(|| {
            let _redacted = envforge::lsp::redact::redact_secrets_in_message(
                black_box(msg),
                black_box(&secrets),
            );
        });
    });
}

criterion_group!(
    benches,
    bench_parser_roundtrip_small,
    bench_parser_roundtrip_large,
    bench_sync_diff_small,
    bench_secret_redaction
);
criterion_main!(benches);
