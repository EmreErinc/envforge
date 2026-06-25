//! Coverage for `ops::check` pure helpers: `CheckCategory` name/parse/display
//! round-trips, `parse_category_filter`, and `CheckReport` status counts.

use envforge::ops::check::{
    parse_category_filter, CheckCategory, CheckReport, CheckResult, CheckStatus,
};

#[test]
fn test_category_name_parse_roundtrip() {
    let all = CheckCategory::all();
    assert_eq!(all.len(), 5);
    for c in &all {
        assert_eq!(CheckCategory::parse(c.name()).as_ref(), Some(c));
        assert!(!c.display_name().is_empty());
    }
}

#[test]
fn test_category_parse_case_insensitive_and_unknown() {
    assert_eq!(CheckCategory::parse("DOCTOR"), Some(CheckCategory::Doctor));
    assert_eq!(CheckCategory::parse("Scan"), Some(CheckCategory::Scan));
    assert_eq!(CheckCategory::parse("nope"), None);
}

#[test]
fn test_parse_category_filter_list_blanks_and_unknown() {
    assert_eq!(
        parse_category_filter("doctor,scan").unwrap(),
        vec![CheckCategory::Doctor, CheckCategory::Scan]
    );
    // Blank segments are skipped.
    assert_eq!(
        parse_category_filter("doctor, ,age").unwrap(),
        vec![CheckCategory::Doctor, CheckCategory::Age]
    );
    let err = parse_category_filter("doctor,bogus").unwrap_err();
    assert!(err.contains("Unknown category"));
}

#[test]
fn test_check_report_status_counts() {
    let mk = |status: CheckStatus| CheckResult {
        category: CheckCategory::Doctor,
        status,
        message: "m".to_string(),
        hint: None,
    };
    let report = CheckReport {
        results: vec![
            mk(CheckStatus::Ok),
            mk(CheckStatus::Ok),
            mk(CheckStatus::Warning),
            mk(CheckStatus::Error),
        ],
        skipped: vec![],
    };
    assert_eq!(report.ok_count(), 2);
    assert_eq!(report.warning_count(), 1);
    assert_eq!(report.error_count(), 1);
    assert!(report.has_errors());
}
