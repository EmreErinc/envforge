use crate::ops::EnvEntry;

pub fn shannon_entropy(s: &str) -> f64 {
    if s.is_empty() {
        return 0.0;
    }
    let mut map = std::collections::HashMap::new();
    for c in s.chars() {
        *map.entry(c).or_insert(0usize) += 1;
    }
    let len = s.chars().count() as f64;
    let mut entropy = 0.0;
    for &count in map.values() {
        let p = count as f64 / len;
        entropy -= p * p.log2();
    }
    entropy
}

pub fn entropy_rating(entropy: f64) -> &'static str {
    if entropy > 4.5 {
        "High (Secure)"
    } else if entropy > 3.0 {
        "Medium"
    } else {
        "Low (Weak)"
    }
}

pub struct InspectorDetails {
    pub key: String,
    pub raw_value: String,
    pub location_str: String,
    pub is_active: bool,
    pub entropy: f64,
    pub rating: &'static str,
}

impl InspectorDetails {
    pub fn from_entry(entry: &EnvEntry, config: &crate::config::AppConfig) -> Self {
        let entropy = shannon_entropy(&entry.value);
        let rating = entropy_rating(entropy);
        let is_active = entry.location != crate::ops::EntryLocation::Commented;
        let location_str = crate::ops::source_display_name(config, &entry.source_file);

        Self {
            key: entry.key.clone(),
            raw_value: entry.value.clone(),
            location_str,
            is_active,
            entropy,
            rating,
        }
    }
}
