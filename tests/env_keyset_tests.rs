//! Tests for [`envforge::ops::env_keyset`] — Epic 3 (AR2): the unified key-set
//! with per-environment values. Exercises the pure builder
//! `build_env_keyset_from_sources` so no filesystem is needed.

use std::path::Path;

use envforge::ops::env_keyset::build_env_keyset_from_sources;

fn build<'a>(sources: &'a [(&'a str, &'a str, &'a str)]) -> envforge::ops::env_keyset::EnvKeySet {
    // (env_name, file_str, content) -> (&str, &Path, &str)
    let refs: Vec<(&str, &Path, &str)> = sources
        .iter()
        .map(|(name, file, content)| (*name, Path::new(*file), *content))
        .collect();
    build_env_keyset_from_sources(&refs)
}

#[test]
fn test_union_of_keys_across_environments() {
    let set = build(&[
        ("dev", "/p/.env.dev", "A=1\nB=2\n"),
        ("prod", "/p/.env.prod", "B=9\nC=3\n"),
    ]);

    let keys: Vec<&str> = set.key_names().collect();
    assert_eq!(keys, vec!["A", "B", "C"]); // BTreeMap → sorted, deterministic
}

#[test]
fn test_per_environment_values_recorded() {
    let set = build(&[
        ("dev", "/p/.env.dev", "DATABASE_URL=dev-db\n"),
        ("prod", "/p/.env.prod", "DATABASE_URL=prod-db\n"),
    ]);

    let entry = set.entry("DATABASE_URL").expect("key present");
    assert_eq!(entry.values["dev"].value, "dev-db");
    assert_eq!(entry.values["prod"].value, "prod-db");
    let envs: Vec<&str> = entry.environments().collect();
    assert_eq!(envs, vec!["dev", "prod"]);
}

#[test]
fn test_line_numbers_are_zero_based() {
    let set = build(&[("dev", "/p/.env.dev", "# comment\nA=1\n\nB=2\n")]);

    let a = &set.entry("A").unwrap().values["dev"];
    let b = &set.entry("B").unwrap().values["dev"];
    assert_eq!(a.line, 1); // line 0 is the comment
    assert_eq!(b.line, 3); // line 2 is blank
    assert_eq!(a.file, Path::new("/p/.env.dev"));
}

#[test]
fn test_last_assignment_wins_within_file() {
    let set = build(&[("dev", "/p/.env.dev", "A=1\nA=2\n")]);
    assert_eq!(set.entry("A").unwrap().values["dev"].value, "2");
}

#[test]
fn test_quotes_stripped() {
    let set = build(&[("dev", "/p/.env.dev", "D=\"quoted\"\nS='single'\nU=bare\n")]);
    let v = |k: &str| set.entry(k).unwrap().values["dev"].value.clone();
    assert_eq!(v("D"), "quoted");
    assert_eq!(v("S"), "single");
    assert_eq!(v("U"), "bare");
}

#[test]
fn test_comments_and_blanks_and_keyless_skipped() {
    let set = build(&[("dev", "/p/.env.dev", "# c\n\n=novalue\nGOOD=1\n")]);
    let keys: Vec<&str> = set.key_names().collect();
    assert_eq!(keys, vec!["GOOD"]);
}

#[test]
fn test_sensitivity_flagged_by_key_heuristic() {
    let set = build(&[("dev", "/p/.env.dev", "API_KEY=abc\nPORT=8080\n")]);
    assert!(set.entry("API_KEY").unwrap().is_sensitive());
    assert!(!set.entry("PORT").unwrap().is_sensitive());
}

#[test]
fn test_distinct_values_deduped_and_sorted() {
    let set = build(&[
        ("dev", "/p/.env.dev", "LEVEL=debug\n"),
        ("stage", "/p/.env.stage", "LEVEL=info\n"),
        ("prod", "/p/.env.prod", "LEVEL=info\n"),
    ]);
    // info appears twice → de-duped; sorted.
    assert_eq!(set.distinct_values("LEVEL"), vec!["debug", "info"]);
    assert!(set.distinct_values("MISSING").is_empty());
}

#[test]
fn test_missing_in_reports_cross_environment_gaps() {
    let set = build(&[
        ("dev", "/p/.env.dev", "A=1\nB=2\n"),
        ("prod", "/p/.env.prod", "A=1\nC=3\n"),
    ]);

    // In prod, B is missing (defined in dev).
    let missing_prod = set.missing_in("prod");
    assert_eq!(missing_prod, vec![("B", vec!["dev"])]);

    // In dev, C is missing (defined in prod).
    let missing_dev = set.missing_in("dev");
    assert_eq!(missing_dev, vec![("C", vec!["prod"])]);
}

#[test]
fn test_apply_schema_sensitivity_unions_schema_flag() {
    use std::collections::HashMap;

    use envforge::ops::schema::{EnvSchema, SchemaVariable, VarType};

    // GREETING is not flagged by the key-name heuristic.
    let mut set = build(&[("dev", "/p/.env.dev", "GREETING=hi\nPLAIN=1\n")]);
    assert!(
        !set.entry("GREETING").unwrap().is_sensitive(),
        "heuristic should not flag GREETING"
    );

    // Schema marks GREETING sensitive.
    let var = SchemaVariable {
        var_type: VarType::String,
        required: false,
        default: None,
        description: None,
        example: None,
        sensitive: true,
        pattern: None,
        values: None,
        min: None,
        max: None,
        env_overrides: HashMap::new(),
        ttl_days: None,
        rotation_strategy: None,
        auto_rotate: None,
        notify_days_before_expiry: None,
    };
    let mut variables = HashMap::new();
    variables.insert("GREETING".to_string(), var);
    let schema = EnvSchema { variables };

    set.apply_schema_sensitivity(&schema);

    assert!(
        set.entry("GREETING").unwrap().is_sensitive(),
        "schema sensitivity should be unioned in"
    );
    assert!(
        !set.entry("PLAIN").unwrap().is_sensitive(),
        "non-schema key untouched"
    );
}

#[test]
fn test_empty_when_no_sources() {
    let set = build(&[]);
    assert!(set.is_empty());
    assert_eq!(set.key_names().count(), 0);
}
