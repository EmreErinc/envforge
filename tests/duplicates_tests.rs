use std::path::Path;

use envforge::model::*;
use envforge::ops::*;
use envforge::parser::*;

fn make_sf(content: &str, path: &str) -> ShellFile {
    parse_shell_content(content, Path::new(path)).unwrap()
}

#[test]
fn test_no_duplicates() {
    let sf = make_sf("export A=1\nexport B=2\n", "/test/.zshrc");
    let groups = detect_duplicates(&[sf]);
    assert!(groups.is_empty());
}

#[test]
fn test_same_file_duplicate() {
    let sf = make_sf("export FOO=1\nexport FOO=2\n", "/test/.zshrc");
    let groups = detect_duplicates(&[sf]);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].key, "FOO");
    assert_eq!(groups[0].entries.len(), 2);
}

#[test]
fn test_cross_file_duplicate() {
    let sf1 = make_sf("export FOO=1\n", "/test/.zshrc");
    let sf2 = make_sf("export FOO=2\n", "/test/.env_managed");
    let groups = detect_duplicates(&[sf1, sf2]);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].key, "FOO");
    assert_eq!(groups[0].entries.len(), 2);
    assert_ne!(
        groups[0].entries[0].source_file,
        groups[0].entries[1].source_file
    );
}

#[test]
fn test_commented_not_counted() {
    let sf = make_sf(
        "export FOO=1\n#[envforge:deleted:FOO] export FOO=2\n",
        "/test/.zshrc",
    );
    let groups = detect_duplicates(&[sf]);
    assert!(groups.is_empty()); // Only 1 active, deleted doesn't count
}

#[test]
fn test_resolve_detection_count() {
    // Resolve is complex with same-key entries — verify detection counts are correct
    let sf = make_sf(
        "export FOO=first\nexport BAR=only\nexport FOO=second\n",
        "/test/.zshrc",
    );
    let groups = detect_duplicates(&[sf]);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].key, "FOO");
    assert_eq!(groups[0].entries.len(), 2);
    assert_eq!(groups[0].entries[0].value, "first");
    assert_eq!(groups[0].entries[1].value, "second");
}

#[test]
fn test_duplicate_key_set() {
    let sf = make_sf("export A=1\nexport A=2\nexport B=3\n", "/test/.zshrc");
    let set = duplicate_key_set(&[sf]);
    assert!(set.contains("A"));
    assert!(!set.contains("B"));
}

#[test]
fn test_multiple_duplicate_groups() {
    let sf = make_sf(
        "export A=1\nexport A=2\nexport B=1\nexport B=2\nexport C=1\n",
        "/test/.zshrc",
    );
    let groups = detect_duplicates(&[sf]);
    assert_eq!(groups.len(), 2); // A and B are duplicated, C is not
}
