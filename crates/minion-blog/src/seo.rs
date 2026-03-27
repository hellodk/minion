//! SEO optimization

use serde::{Deserialize, Serialize};

/// Result of analysing a blog post for SEO quality.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeoAnalysis {
    /// Overall SEO score from 0 to 100.
    pub score: u32,
    /// Length of the title in characters.
    pub title_length: usize,
    /// Whether the content contains a meta-description-length excerpt.
    pub has_meta_description: bool,
    /// Fraction of total words that are target keywords (0.0 -- 1.0).
    pub keyword_density: f64,
    /// Whether the content uses markdown headings (`##`).
    pub heading_structure: bool,
    /// Total word count.
    pub word_count: usize,
    /// Human-readable improvement suggestions.
    pub suggestions: Vec<String>,
}

/// Stateless analyser that scores blog content for SEO.
pub struct SeoAnalyzer;

impl SeoAnalyzer {
    /// Analyse a post and return an `SeoAnalysis`.
    ///
    /// Scoring breakdown (100 points max):
    /// - Title length:        up to 25 points (50-60 chars ideal)
    /// - Keyword density:     up to 25 points (1-3 % ideal)
    /// - Heading structure:   up to 25 points (has `##` headings)
    /// - Content length:      up to 25 points (> 300 words)
    pub fn analyze(title: &str, content: &str, keywords: &[String]) -> SeoAnalysis {
        let mut suggestions = Vec::new();

        let (title_score, title_suggestion) = Self::check_title_length(title);
        if let Some(s) = title_suggestion {
            suggestions.push(s);
        }

        let (density, keyword_score, keyword_suggestion) =
            Self::check_keyword_density(content, keywords);
        if let Some(s) = keyword_suggestion {
            suggestions.push(s);
        }

        let (has_headings, heading_score, heading_suggestion) =
            Self::check_heading_structure(content);
        if let Some(s) = heading_suggestion {
            suggestions.push(s);
        }

        let (wc, content_score, content_suggestion) = Self::check_content_length(content);
        if let Some(s) = content_suggestion {
            suggestions.push(s);
        }

        let title_len = title.len();
        // A reasonable meta description is roughly a sentence between 50 and 160 chars.
        // We approximate "has meta description" as the first paragraph being in that range,
        // but since we only have title + content we simply check title length as a proxy.
        let has_meta_description = (50..=160).contains(&title_len);

        let score = (title_score + keyword_score + heading_score + content_score).min(100);

        SeoAnalysis {
            score,
            title_length: title_len,
            has_meta_description,
            keyword_density: density,
            heading_structure: has_headings,
            word_count: wc,
            suggestions,
        }
    }

    /// Check whether the title is in the ideal length range (50-60 chars).
    /// Returns (points, optional suggestion).
    fn check_title_length(title: &str) -> (u32, Option<String>) {
        let len = title.len();
        if (50..=60).contains(&len) {
            (25, None)
        } else if (40..50).contains(&len) || (60..=70).contains(&len) {
            (
                15,
                Some("Title length is acceptable but ideally should be 50-60 characters".into()),
            )
        } else if len < 40 {
            (
                5,
                Some(format!(
                    "Title is too short ({len} chars). Aim for 50-60 characters"
                )),
            )
        } else {
            (
                5,
                Some(format!(
                    "Title is too long ({len} chars). Aim for 50-60 characters"
                )),
            )
        }
    }

    /// Calculate keyword density and score it.
    /// Returns (density, points, optional suggestion).
    fn check_keyword_density(content: &str, keywords: &[String]) -> (f64, u32, Option<String>) {
        let words: Vec<&str> = content.split_whitespace().collect();
        let total = words.len();
        if total == 0 || keywords.is_empty() {
            return (0.0, 0, Some("Add target keywords to your content".into()));
        }

        let lower_content = content.to_lowercase();
        let keyword_count: usize = keywords
            .iter()
            .map(|kw| {
                let kw_lower = kw.to_lowercase();
                // Count non-overlapping occurrences of each keyword.
                lower_content.matches(&kw_lower).count()
            })
            .sum();

        let density = keyword_count as f64 / total as f64 * 100.0;

        if (1.0..=3.0).contains(&density) {
            (density, 25, None)
        } else if (0.5..1.0).contains(&density) || (3.0..5.0).contains(&density) {
            (
                density,
                15,
                Some(format!(
                    "Keyword density is {density:.1}%. Ideal range is 1-3%"
                )),
            )
        } else if density < 0.5 {
            (
                density,
                5,
                Some(format!(
                    "Keyword density is very low ({density:.1}%). Aim for 1-3%"
                )),
            )
        } else {
            (
                density,
                5,
                Some(format!(
                    "Keyword density is too high ({density:.1}%). Aim for 1-3%"
                )),
            )
        }
    }

    /// Check whether the content contains markdown `##` headings.
    /// Returns (has_headings, points, optional suggestion).
    fn check_heading_structure(content: &str) -> (bool, u32, Option<String>) {
        let has_headings = content
            .lines()
            .any(|line| line.trim_start().starts_with("## "));

        if has_headings {
            (true, 25, None)
        } else {
            (
                false,
                0,
                Some("Add markdown headings (## Heading) to improve content structure".into()),
            )
        }
    }

    /// Check content length (word count).
    /// Returns (word_count, points, optional suggestion).
    fn check_content_length(content: &str) -> (usize, u32, Option<String>) {
        let wc = content.split_whitespace().count();
        if wc >= 300 {
            (wc, 25, None)
        } else if wc >= 150 {
            (
                wc,
                15,
                Some(format!(
                    "Content has {wc} words. Aim for at least 300 words"
                )),
            )
        } else {
            (
                wc,
                5,
                Some(format!(
                    "Content is too short ({wc} words). Aim for at least 300 words"
                )),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generate a string of `n` words.
    fn words(n: usize) -> String {
        (0..n).map(|_| "word").collect::<Vec<_>>().join(" ")
    }

    // ---- title length ----

    #[test]
    fn test_title_ideal_length() {
        // 55 chars
        let title = "A".repeat(55);
        let (score, suggestion) = SeoAnalyzer::check_title_length(&title);
        assert_eq!(score, 25);
        assert!(suggestion.is_none());
    }

    #[test]
    fn test_title_too_short() {
        let title = "Short";
        let (score, suggestion) = SeoAnalyzer::check_title_length(title);
        assert_eq!(score, 5);
        assert!(suggestion.unwrap().contains("too short"));
    }

    #[test]
    fn test_title_acceptable() {
        // 45 chars
        let title = "A".repeat(45);
        let (score, suggestion) = SeoAnalyzer::check_title_length(&title);
        assert_eq!(score, 15);
        assert!(suggestion.is_some());
    }

    #[test]
    fn test_title_too_long() {
        let title = "A".repeat(80);
        let (score, suggestion) = SeoAnalyzer::check_title_length(&title);
        assert_eq!(score, 5);
        assert!(suggestion.unwrap().contains("too long"));
    }

    // ---- keyword density ----

    #[test]
    fn test_keyword_density_ideal() {
        // 100 words, 2 occurrences of "rust" -> 2%
        let mut content_words: Vec<&str> = vec!["other"; 98];
        content_words.insert(10, "rust");
        content_words.insert(50, "rust");
        let content = content_words.join(" ");

        let keywords = vec!["rust".to_string()];
        let (density, score, suggestion) = SeoAnalyzer::check_keyword_density(&content, &keywords);

        assert!((1.0..=3.0).contains(&density));
        assert_eq!(score, 25);
        assert!(suggestion.is_none());
    }

    #[test]
    fn test_keyword_density_no_keywords() {
        let content = words(100);
        let keywords: Vec<String> = Vec::new();
        let (density, score, suggestion) = SeoAnalyzer::check_keyword_density(&content, &keywords);
        assert_eq!(density, 0.0);
        assert_eq!(score, 0);
        assert!(suggestion.is_some());
    }

    #[test]
    fn test_keyword_density_empty_content() {
        let keywords = vec!["rust".to_string()];
        let (density, score, _) = SeoAnalyzer::check_keyword_density("", &keywords);
        assert_eq!(density, 0.0);
        assert_eq!(score, 0);
    }

    #[test]
    fn test_keyword_density_too_high() {
        // 10 words, all "rust" -> 100%
        let content = (0..10).map(|_| "rust").collect::<Vec<_>>().join(" ");
        let keywords = vec!["rust".to_string()];
        let (density, score, suggestion) = SeoAnalyzer::check_keyword_density(&content, &keywords);
        assert!(density > 5.0);
        assert_eq!(score, 5);
        assert!(suggestion.unwrap().contains("too high"));
    }

    // ---- heading structure ----

    #[test]
    fn test_heading_present() {
        let content = "Intro text\n\n## Section One\n\nBody text.";
        let (has, score, suggestion) = SeoAnalyzer::check_heading_structure(content);
        assert!(has);
        assert_eq!(score, 25);
        assert!(suggestion.is_none());
    }

    #[test]
    fn test_heading_missing() {
        let content = "Just a flat wall of text with no headings.";
        let (has, score, suggestion) = SeoAnalyzer::check_heading_structure(content);
        assert!(!has);
        assert_eq!(score, 0);
        assert!(suggestion.is_some());
    }

    #[test]
    fn test_heading_h1_not_counted() {
        // Only `##` (h2) and deeper should count.
        let content = "# Top level heading only";
        let (has, _, _) = SeoAnalyzer::check_heading_structure(content);
        assert!(!has);
    }

    // ---- content length ----

    #[test]
    fn test_content_long_enough() {
        let content = words(400);
        let (wc, score, suggestion) = SeoAnalyzer::check_content_length(&content);
        assert_eq!(wc, 400);
        assert_eq!(score, 25);
        assert!(suggestion.is_none());
    }

    #[test]
    fn test_content_medium() {
        let content = words(200);
        let (wc, score, suggestion) = SeoAnalyzer::check_content_length(&content);
        assert_eq!(wc, 200);
        assert_eq!(score, 15);
        assert!(suggestion.unwrap().contains("300 words"));
    }

    #[test]
    fn test_content_too_short() {
        let content = words(50);
        let (_, score, suggestion) = SeoAnalyzer::check_content_length(&content);
        assert_eq!(score, 5);
        assert!(suggestion.unwrap().contains("too short"));
    }

    // ---- full analyze ----

    #[test]
    fn test_analyze_perfect_score() {
        // Title: exactly 55 chars
        let title = "A".repeat(55);
        // Content: 400 words with headings and 2% keyword density
        let mut lines = vec!["## Introduction".to_string()];
        // Build content with keywords sprinkled in.
        let mut body_words: Vec<String> = (0..396).map(|_| "lorem".to_string()).collect();
        // Insert 8 keywords among 400 words -> ~2%
        for i in [10, 50, 100, 150, 200, 250, 300, 350] {
            body_words[i] = "rust".to_string();
        }
        lines.push(body_words.join(" "));
        let content = lines.join("\n\n");

        let keywords = vec!["rust".to_string()];
        let analysis = SeoAnalyzer::analyze(&title, &content, &keywords);

        assert_eq!(analysis.score, 100);
        assert!(analysis.suggestions.is_empty());
        assert!(analysis.heading_structure);
        assert!(analysis.has_meta_description);
    }

    #[test]
    fn test_analyze_low_score() {
        let title = "Hi";
        let content = "short";
        let keywords: Vec<String> = Vec::new();

        let analysis = SeoAnalyzer::analyze(title, content, &keywords);
        assert!(analysis.score < 50);
        assert!(!analysis.suggestions.is_empty());
    }

    #[test]
    fn test_analyze_serde_roundtrip() {
        let analysis = SeoAnalyzer::analyze("Some Title", "Some content", &[]);
        let json = serde_json::to_string(&analysis).unwrap();
        let deserialized: SeoAnalysis = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.score, analysis.score);
        assert_eq!(deserialized.title_length, analysis.title_length);
        assert_eq!(deserialized.word_count, analysis.word_count);
    }

    #[test]
    fn test_analyze_score_capped_at_100() {
        // Even in the best case, the score should never exceed 100.
        let title = "A".repeat(55);
        let mut body = words(500);
        body.push_str("\n\n## Heading\n\n");
        body.push_str(&"keyword ".repeat(10));
        let keywords = vec!["keyword".to_string()];

        let analysis = SeoAnalyzer::analyze(&title, &body, &keywords);
        assert!(analysis.score <= 100);
    }
}
