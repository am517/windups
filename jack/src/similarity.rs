//! Title similarity: normalized Levenshtein on normalized titles.
use strsim::normalized_levenshtein;

/// "Breaking.Bad" / "breaking_bad" / "Breaking  Bad" -> "breaking bad". Other punctuation stays.
pub fn norm(s: &str) -> String {
    s.to_lowercase()
        .replace(['.', '_', '-'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// 1.0 is the same title. Empty titles score 0.
pub fn score(query: &str, candidate: &str) -> f64 {
    let query = norm(query);
    let candidate = norm(candidate);
    if query.is_empty() || candidate.is_empty() {
        return 0.0;
    }
    normalized_levenshtein(&query, &candidate)
}

/// A missing or out-of-range config value means the default, 0.90.
pub fn threshold(configured: f64) -> f64 {
    if configured > 0.0 && configured <= 1.0 {
        configured
    } else {
        0.90
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_titles_score_one_and_a_near_title_stays_under_the_default() {
        assert!((score("Breaking.Bad", "Breaking Bad") - 1.0).abs() < 1e-9);
        assert!((score("breaking_bad", "Breaking-Bad") - 1.0).abs() < 1e-9);
        let near = score("Joe Rogan Experience", "The Joe Rogan Experience");
        assert!(near < 0.90, "a leading word is not close enough: {near}");
        let one = score("The Why Files", "The Why Files?");
        assert!(one >= 0.90, "one extra mark is still one result: {one}");
        assert_eq!(score("", "The Matrix"), 0.0);
        assert_eq!(threshold(0.0), 0.90);
        assert_eq!(threshold(0.85), 0.85);
    }
}
