use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

use crate::ops::listing::EnvEntry;

/// A fuzzy search result with score and matched indices.
#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    pub entry: EnvEntry,
    pub score: i64,
    pub matched_indices: Vec<usize>,
}

/// Perform fuzzy search on entries, searching both key and value.
///
/// Returns results sorted by score (best first).
/// Falls back to exact substring match if fuzzy returns no results.
pub fn fuzzy_search(entries: &[EnvEntry], query: &str) -> Vec<FuzzyMatch> {
    if query.is_empty() {
        return entries
            .iter()
            .map(|e| FuzzyMatch {
                entry: e.clone(),
                score: 0,
                matched_indices: vec![],
            })
            .collect();
    }

    let matcher = SkimMatcherV2::default();
    let mut results: Vec<FuzzyMatch> = Vec::new();

    for entry in entries {
        // Try matching on key first (higher priority)
        let key_match = matcher.fuzzy_indices(&entry.key, query);
        let value_match = matcher.fuzzy_indices(&entry.value, query);

        if let Some((score, indices)) = key_match {
            results.push(FuzzyMatch {
                entry: entry.clone(),
                score,
                matched_indices: indices,
            });
        } else if let Some((score, _indices)) = value_match {
            // Value matched — include but with no key highlights
            results.push(FuzzyMatch {
                entry: entry.clone(),
                score,
                matched_indices: vec![],
            });
        }
    }

    // If fuzzy found nothing, fallback to exact substring
    if results.is_empty() {
        let query_lower = query.to_lowercase();
        for entry in entries {
            if entry.key.to_lowercase().contains(&query_lower)
                || entry.value.to_lowercase().contains(&query_lower)
            {
                results.push(FuzzyMatch {
                    entry: entry.clone(),
                    score: 0,
                    matched_indices: vec![],
                });
            }
        }
    }

    results.sort_by_key(|a| std::cmp::Reverse(a.score));
    results
}
