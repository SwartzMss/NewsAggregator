use std::collections::BTreeSet;

/// Normalize a title for duplicate comparison: lowercase, replace punctuation with spaces,
/// and collapse multiple spaces.
pub fn normalize_title_for_comparison(title: &str) -> String {
    let mut normalized = String::with_capacity(title.len());
    let mut space_pending = false;

    for ch in title.chars() {
        if ch.is_alphanumeric() {
            for lower in ch.to_lowercase() {
                normalized.push(lower);
            }
            space_pending = false;
        } else if ch.is_whitespace() {
            if !space_pending {
                normalized.push(' ');
                space_pending = true;
            }
        } else {
            if !space_pending {
                normalized.push(' ');
                space_pending = true;
            }
        }
    }

    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Prepare a normalized title and token set for Jaccard comparison.
pub fn prepare_title_signature(title: &str) -> (String, BTreeSet<String>) {
    let normalized = normalize_title_for_comparison(title);
    let tokens = normalized
        .split_whitespace()
        .filter(|token| token.len() >= 2)
        .map(|token| token.to_string())
        .collect::<BTreeSet<_>>();

    (normalized, tokens)
}

pub fn jaccard_similarity(a: &BTreeSet<String>, b: &BTreeSet<String>) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let intersection = a.intersection(b).count() as f32;
    let union = a.union(b).count() as f32;

    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}
