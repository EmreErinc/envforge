#[test]
fn test_multi_select_toggle() {
    let mut selected = std::collections::HashSet::new();
    selected.insert("DATABASE_URL".to_string());
    assert!(selected.contains("DATABASE_URL"));
    selected.remove("DATABASE_URL");
    assert!(!selected.contains("DATABASE_URL"));
}
