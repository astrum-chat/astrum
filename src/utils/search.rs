use strsim::normalized_levenshtein;

const FUZZY_THRESHOLD: f64 = 0.4;

/// Calculates the search relevance of a title against a query.
/// Returns Some(score) if the title matches, None otherwise.
///
/// Score interpretation:
/// - 2.0+ = substring match (higher = earlier position)
/// - 0.0-1.0 = fuzzy match (higher = more similar)
pub fn calculate_match_score(query: &str, title: &str) -> Option<f64> {
    if query.is_empty() {
        return Some(1.0);
    }

    let query_lower = query.to_lowercase();
    let title_lower = title.to_lowercase();

    // Check for substring match first (highest priority)
    if let Some(position) = title_lower.find(&query_lower) {
        // Score: 2.0 + bonus for earlier matches
        let position_bonus = 1.0 / (1.0 + position as f64 * 0.1);
        return Some(2.0 + position_bonus);
    }

    // Fall back to fuzzy matching
    let similarity = normalized_levenshtein(&query_lower, &title_lower);

    if similarity >= FUZZY_THRESHOLD {
        Some(similarity)
    } else {
        None
    }
}

/// Filters and sorts items by search relevance.
/// Returns items sorted by score (highest first).
pub fn filter_by_relevance<'a, T, F>(
    items: impl Iterator<Item = T>,
    query: &str,
    get_text: F,
) -> Vec<T>
where
    F: Fn(&T) -> &'a str,
{
    let mut matches: Vec<(T, f64)> = items
        .filter_map(|item| {
            let text = get_text(&item);
            calculate_match_score(query, text).map(|score| (item, score))
        })
        .collect();

    // Sort by score descending (highest relevance first)
    matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    matches.into_iter().map(|(item, _)| item).collect()
}
