use std::path::Path;

use envforge::ops::grouping::*;
use envforge::ops::*;
use envforge::parser::*;

fn make_entries(content: &str) -> Vec<EnvEntry> {
    let sf = parse_shell_content(content, Path::new("/test/.zshrc")).unwrap();
    collect_entries(&sf)
}

#[test]
fn test_auto_group_by_prefix() {
    let entries = make_entries(
        "export DB_HOST=localhost\nexport DB_PORT=5432\nexport DB_NAME=mydb\nexport API_URL=http\nexport API_KEY=secret\nexport SOLO=alone\n",
    );
    let config = GroupConfig::default();
    let groups = group_entries(&entries, &config);

    // DB_* group, API_* group, "Other" for SOLO
    assert!(groups
        .iter()
        .any(|g| g.name == "DB_*" && g.entries.len() == 3));
    assert!(groups
        .iter()
        .any(|g| g.name == "API_*" && g.entries.len() == 2));
    assert!(groups
        .iter()
        .any(|g| g.name == "Other" && g.entries.len() == 1));
}

#[test]
fn test_singleton_not_grouped() {
    let entries = make_entries("export SINGLE_VAR=val\nexport OTHER=val2\n");
    let config = GroupConfig::default();
    let groups = group_entries(&entries, &config);

    // SINGLE_* has only 1 entry, shouldn't form a group
    // Both should be in "Other"
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].name, "Other");
    assert_eq!(groups[0].entries.len(), 2);
}

#[test]
fn test_user_defined_group_priority() {
    let entries =
        make_entries("export DB_HOST=localhost\nexport DB_PORT=5432\nexport DATABASE_URL=pg\n");
    let config = GroupConfig {
        groups: vec![(
            "Database".to_string(),
            vec!["DB_*".to_string(), "DATABASE_*".to_string()],
        )],
    };
    let groups = group_entries(&entries, &config);

    // All 3 should be in user-defined "Database" group
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].name, "Database");
    assert_eq!(groups[0].entries.len(), 3);
    assert!(groups[0].is_user_defined);
}

#[test]
fn test_user_group_overrides_auto() {
    let entries =
        make_entries("export DB_HOST=localhost\nexport DB_PORT=5432\nexport API_URL=http\n");
    let config = GroupConfig {
        groups: vec![("Infra".to_string(), vec!["DB_*".to_string()])],
    };
    let groups = group_entries(&entries, &config);

    // DB_* goes to user "Infra", API_URL to "Other" (only 1 API_ entry)
    assert!(groups
        .iter()
        .any(|g| g.name == "Infra" && g.entries.len() == 2));
    assert!(groups
        .iter()
        .any(|g| g.name == "Other" && g.entries.len() == 1));
}

#[test]
fn test_glob_match_prefix() {
    let entries = make_entries(
        "export NEXT_PUBLIC_API=val\nexport NEXT_PUBLIC_URL=val2\nexport PRIVATE=val3\n",
    );
    let config = GroupConfig {
        groups: vec![(
            "Next.js Public".to_string(),
            vec!["NEXT_PUBLIC_*".to_string()],
        )],
    };
    let groups = group_entries(&entries, &config);

    assert!(groups
        .iter()
        .any(|g| g.name == "Next.js Public" && g.entries.len() == 2));
}

#[test]
fn test_empty_entries() {
    let entries: Vec<EnvEntry> = vec![];
    let config = GroupConfig::default();
    let groups = group_entries(&entries, &config);
    assert!(groups.is_empty());
}

#[test]
fn test_all_in_user_groups() {
    let entries = make_entries("export A=1\nexport B=2\n");
    let config = GroupConfig {
        groups: vec![("All".to_string(), vec!["*".to_string()])],
    };
    let groups = group_entries(&entries, &config);

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].name, "All");
    assert_eq!(groups[0].entries.len(), 2);
}
