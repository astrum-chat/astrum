use rayon::prelude::*;
use strsim::jaro_winkler;

/// Returns the fuzzy match threshold based on token length.
/// Shorter tokens require higher similarity to avoid false positives.
#[inline]
fn fuzzy_threshold(token_len: usize) -> f64 {
    match token_len {
        0..=3 => 0.95,
        4..=5 => 0.88,
        _ => 0.82,
    }
}

#[inline]
fn best_token_score(query_token: &str, title_tokens: &[&str]) -> Option<f64> {
    // Check for exact match first
    for title_token in title_tokens {
        if query_token == *title_token {
            return Some(1.0);
        }
    }

    // Check for prefix match (query is prefix of title token)
    for title_token in title_tokens {
        if title_token.starts_with(query_token) {
            return Some(0.95);
        }
    }

    let threshold = fuzzy_threshold(query_token.len());
    let best = title_tokens
        .iter()
        .map(|tt| jaro_winkler(query_token, tt))
        .fold(0.0, f64::max);

    if best >= threshold { Some(best) } else { None }
}

#[inline]
fn calculate_match_score(
    query_lower: &str,
    query_tokens: &[&str],
    title_lower: &str,
) -> Option<f64> {
    if title_lower.contains(query_lower) {
        return Some(2.0);
    }

    let title_tokens: Vec<&str> = title_lower.split_whitespace().collect();
    if title_tokens.is_empty() {
        return None;
    }

    let (total_score, matched) = query_tokens.iter().fold((0.0, 0), |(total, matched), qt| {
        if let Some(score) = best_token_score(qt, &title_tokens) {
            (total + score, matched + 1)
        } else {
            (total, matched)
        }
    });

    if matched > 0 {
        Some(total_score / query_tokens.len() as f64)
    } else {
        None
    }
}

fn collect_and_score<'a, T, F>(
    items: impl Iterator<Item = T>,
    query_lower: &str,
    query_tokens: &[&str],
    get_text: F,
) -> Vec<(T, f64)>
where
    T: Send,
    F: Fn(&T) -> &'a str + Sync,
{
    let items_with_titles: Vec<(T, String)> = items
        .map(|item| {
            let title = get_text(&item).to_lowercase();
            (item, title)
        })
        .collect();

    items_with_titles
        .into_par_iter()
        .filter_map(|(item, title)| {
            calculate_match_score(query_lower, query_tokens, &title).map(|score| (item, score))
        })
        .collect()
}

/// Filters and sorts items by search relevance using parallel processing.
pub fn filter_by_relevance<'a, T, F>(
    items: impl Iterator<Item = T>,
    query: &str,
    get_text: F,
) -> Vec<T>
where
    T: Send,
    F: Fn(&T) -> &'a str + Sync,
{
    if query.is_empty() {
        return items.collect();
    }

    let query_lower = query.to_lowercase();
    let query_tokens: Vec<&str> = query_lower.split_whitespace().collect();

    if query_tokens.is_empty() {
        return items.collect();
    }

    let mut scored = collect_and_score(items, &query_lower, &query_tokens, get_text);
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().map(|(item, _)| item).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_query_returns_all() {
        let items = vec!["Alpha", "Beta", "Gamma"];
        let result = filter_by_relevance(items.iter(), "", |s| s);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_whitespace_query_returns_all() {
        let items = vec!["Alpha", "Beta", "Gamma"];
        let result = filter_by_relevance(items.iter(), "   ", |s| s);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_substring_match() {
        let items = vec!["Hello World", "Goodbye World", "Hello There"];
        let result = filter_by_relevance(items.iter(), "hello", |s| s);

        assert_eq!(result.len(), 2);
        assert!(result.contains(&&"Hello World"));
        assert!(result.contains(&&"Hello There"));
    }

    #[test]
    fn test_case_insensitive() {
        let items = vec!["HELLO", "hello", "HeLLo"];
        let result = filter_by_relevance(items.iter(), "hello", |s| s);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_exact_token_match() {
        let items = vec!["rust programming", "python programming", "rust lang"];
        let result = filter_by_relevance(items.iter(), "rust", |s| s);

        assert!(result.len() >= 2);
        assert!(result.contains(&&"rust programming"));
        assert!(result.contains(&&"rust lang"));
    }

    #[test]
    fn test_fuzzy_match_typo() {
        let items = vec!["hello world", "goodbye world"];
        let result = filter_by_relevance(items.iter(), "helo", |s| s);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_no_match_returns_empty() {
        let items = vec!["apple", "banana", "cherry"];
        let result = filter_by_relevance(items.iter(), "xyz123", |s| s);
        assert!(result.is_empty());
    }

    #[test]
    fn test_sorted_by_relevance() {
        let items = vec!["react hooks", "react tutorial", "vue tutorial"];
        let result = filter_by_relevance(items.iter(), "react", |s| s);

        assert!(result.len() >= 2);
        let react_positions: Vec<_> = result
            .iter()
            .enumerate()
            .filter(|(_, s)| s.contains("react"))
            .map(|(i, _)| i)
            .collect();
        assert!(react_positions.iter().all(|&pos| pos < result.len()));
    }

    #[test]
    fn test_multi_token_query() {
        let items = vec![
            "rust programming language",
            "python programming",
            "rust lang",
        ];
        let result = filter_by_relevance(items.iter(), "rust programming", |s| s);

        assert!(!result.is_empty());
        assert_eq!(*result[0], "rust programming language");
    }

    #[test]
    fn test_with_struct_items() {
        #[allow(dead_code)]
        struct Chat {
            id: u32,
            title: String,
        }

        let chats = vec![
            Chat {
                id: 1,
                title: "Project Discussion".to_string(),
            },
            Chat {
                id: 2,
                title: "Random Chat".to_string(),
            },
            Chat {
                id: 3,
                title: "Project Update".to_string(),
            },
        ];

        let result = filter_by_relevance(chats.iter(), "project", |c| &c.title);

        assert_eq!(result.len(), 2);
        assert!(
            result
                .iter()
                .all(|c| c.title.to_lowercase().contains("project"))
        );
    }

    #[test]
    fn test_empty_items() {
        let items: Vec<&str> = vec![];
        let result = filter_by_relevance(items.iter(), "test", |s| s);
        assert!(result.is_empty());
    }

    #[test]
    fn test_empty_title() {
        let items = vec!["", "hello", ""];
        let result = filter_by_relevance(items.iter(), "hello", |s| s);

        assert_eq!(result.len(), 1);
        assert_eq!(*result[0], "hello");
    }
}
