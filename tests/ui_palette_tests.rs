use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

#[test]
fn test_palette_fuzzy_matching() {
    let matcher = SkimMatcherV2::default();
    let score1 = matcher.fuzzy_match("Switch profile to dev", "dev");
    let score2 = matcher.fuzzy_match("Toggle AI Fence", "dev");
    assert!(score1.is_some());
    assert!(score2.is_none());
}
