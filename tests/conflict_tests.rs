//! Coverage for `ops::conflict`: external-modification detection via stored
//! hash, and unified-diff generation from strings.

use envforge::ops::{check_conflict, generate_diff_from_strings};
use envforge::parser::parse_shell_file;

#[test]
fn test_generate_diff_from_strings_identical_is_empty() {
    let s = "export A=1\nexport B=2\n";
    assert_eq!(generate_diff_from_strings(s, s, "/test/.zshrc"), "");
}

#[test]
fn test_generate_diff_from_strings_shows_changes() {
    let original = "export A=1\nexport B=2\n";
    let modified = "export A=1\nexport B=3\n";
    let diff = generate_diff_from_strings(original, modified, "/test/.zshrc");
    assert!(!diff.is_empty());
    assert!(diff.contains("B=2") || diff.contains("-export B=2"));
    assert!(diff.contains("B=3") || diff.contains("+export B=3"));
}

#[test]
fn test_check_conflict_none_when_unchanged() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join(".zshrc");
    std::fs::write(&path, "export A=1\n").unwrap();

    let sf = parse_shell_file(&path).unwrap();
    assert!(
        check_conflict(&sf).is_none(),
        "no external edit → no conflict"
    );
}

#[test]
fn test_check_conflict_some_when_modified_externally() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join(".zshrc");
    std::fs::write(&path, "export A=1\n").unwrap();

    let sf = parse_shell_file(&path).unwrap();
    // Simulate an external editor changing the file after we parsed it.
    std::fs::write(&path, "export A=2\n").unwrap();

    let conflict = check_conflict(&sf).expect("external edit must be detected");
    assert_eq!(conflict.path, path);
    assert_ne!(conflict.stored_hash, conflict.current_hash);
}
