#![allow(dead_code)]

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintIssue {
    pub id: String,
    pub rule: String,
    pub anchor: String,
    pub description: String,
    pub suggestion: String,
    pub auto_fixable: bool,
}

impl LintIssue {
    fn new(
        rule: &str, anchor: &str, description: &str, suggestion: &str, auto_fixable: bool,
    ) -> Self {
        let mut h = Sha256::new();
        h.update(rule); h.update(anchor); h.update(description);
        let id = format!("{:016x}", u64::from_be_bytes(h.finalize()[..8].try_into().unwrap()));
        Self {
            id, rule: rule.to_string(), anchor: anchor.to_string(),
            description: description.to_string(), suggestion: suggestion.to_string(),
            auto_fixable,
        }
    }
}

pub fn lint(content: &str) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    issues.extend(check_heading_hierarchy(content));
    issues.extend(check_duplicate_headings(content));
    issues.extend(check_missing_alt_text(content));
    issues.extend(check_long_paragraphs(content));
    issues.extend(check_unclosed_code_fences(content));
    issues.extend(check_malformed_links(content));
    issues.extend(check_thin_sections(content));
    issues
}

fn parse_headings(content: &str) -> Vec<(u32, String)> {
    let mut headings = Vec::new();
    let mut current_heading: Option<(u32, String)> = None;
    let parser = Parser::new_ext(content, Options::all());
    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current_heading = Some((level as u32, String::new()));
            }
            Event::Text(t) => {
                if let Some((_, ref mut text)) = current_heading {
                    text.push_str(&t);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(h) = current_heading.take() {
                    headings.push(h);
                }
            }
            _ => {}
        }
    }
    headings
}

fn anchor_before(content: &str, byte_offset: usize) -> String {
    let slice = &content[..byte_offset.min(content.len())];
    let mut last_heading = String::new();
    for line in slice.lines() {
        let trimmed = line.trim_start_matches('#').trim();
        if line.starts_with('#') && !trimmed.is_empty() {
            last_heading = trimmed.to_string();
        }
    }
    last_heading
}

fn check_heading_hierarchy(content: &str) -> Vec<LintIssue> {
    let headings = parse_headings(content);
    let mut issues = Vec::new();
    let mut prev_level = 0u32;
    for (level, text) in &headings {
        if prev_level > 0 && *level > prev_level + 1 {
            issues.push(LintIssue::new(
                "heading_hierarchy",
                text,
                &format!("Heading '{}' jumps from H{} to H{} — skips a level", text, prev_level, level),
                "Add an intermediate heading or promote this heading one level",
                false,
            ));
        }
        prev_level = *level;
    }
    issues
}

fn check_duplicate_headings(content: &str) -> Vec<LintIssue> {
    let headings = parse_headings(content);
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut issues = Vec::new();
    for (_, text) in &headings {
        let key = text.to_lowercase();
        *seen.entry(key.clone()).or_insert(0) += 1;
        if seen[&key] == 2 {
            issues.push(LintIssue::new(
                "duplicate_heading",
                text,
                &format!("Heading '{}' appears more than once", text),
                "Rename or remove the duplicate heading",
                false,
            ));
        }
    }
    issues
}

fn check_missing_alt_text(content: &str) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    let re_empty = "![](";
    let mut search = content;
    let mut offset = 0usize;
    while let Some(pos) = search.find(re_empty) {
        let abs = offset + pos;
        let anchor = anchor_before(content, abs);
        issues.push(LintIssue::new(
            "missing_alt_text",
            &anchor,
            "Image has no alt text — bad for accessibility and SEO",
            "Add a descriptive alt text: ![description of image](...)",
            false,
        ));
        offset = abs + re_empty.len();
        search = &content[offset..];
    }
    issues
}

fn check_long_paragraphs(content: &str) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    let mut offset = 0usize;
    let mut in_fence = false;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") { in_fence = !in_fence; }
        if !in_fence {
            let word_count = line.split_whitespace().count();
            if word_count > 150 {
                let anchor = anchor_before(content, offset);
                issues.push(LintIssue::new(
                    "long_paragraph",
                    &anchor,
                    &format!("Paragraph has {} words — very long paragraphs hurt readability", word_count),
                    "Break into 2–3 shorter paragraphs under 100 words each",
                    false,
                ));
            }
        }
        offset += line.len() + 1;
    }
    issues
}

fn check_unclosed_code_fences(content: &str) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    let mut fence_count = 0usize;
    let mut offset = 0usize;
    let mut last_fence_offset = 0usize;
    for line in content.lines() {
        if line.trim_start().starts_with("```") {
            fence_count += 1;
            last_fence_offset = offset;
        }
        offset += line.len() + 1;
    }
    if fence_count % 2 != 0 {
        let anchor = anchor_before(content, last_fence_offset);
        issues.push(LintIssue::new(
            "unclosed_code_fence",
            &anchor,
            "Odd number of ``` fences — at least one code block is not closed",
            "Add a closing ``` after the last open code block",
            false,
        ));
    }
    issues
}

fn check_malformed_links(content: &str) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    let parser = Parser::new_ext(content, Options::all());
    for event in parser {
        if let Event::Start(Tag::Link { dest_url, .. }) = event {
            if dest_url.is_empty() {
                issues.push(LintIssue::new(
                    "empty_link_url",
                    &anchor_before(content, 0),
                    "Link has an empty URL",
                    "Add a valid URL or remove the link markup",
                    false,
                ));
            }
        }
    }
    issues
}

fn check_thin_sections(content: &str) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    let headings = parse_headings(content);
    if headings.is_empty() { return issues; }

    let lines: Vec<&str> = content.lines().collect();
    let mut current_heading = String::new();
    let mut section_words = 0usize;
    let mut in_fence = false;

    for line in &lines {
        let trimmed = line.trim_start_matches('#').trim();
        if line.starts_with('#') {
            if !current_heading.is_empty() && section_words < 30 && section_words > 0 {
                issues.push(LintIssue::new(
                    "thin_section",
                    &current_heading,
                    &format!("Section '{}' has only {} words — very thin", current_heading, section_words),
                    "Expand this section or merge it with an adjacent section",
                    false,
                ));
            }
            current_heading = trimmed.to_string();
            section_words = 0;
        } else {
            if line.trim_start().starts_with("```") { in_fence = !in_fence; }
            if !in_fence {
                section_words += line.split_whitespace().count();
            }
        }
    }
    if !current_heading.is_empty() && section_words < 30 && section_words > 0 {
        issues.push(LintIssue::new(
            "thin_section",
            &current_heading,
            &format!("Section '{}' has only {} words", current_heading, section_words),
            "Expand this section or merge it with an adjacent section",
            false,
        ));
    }
    issues
}

use crate::state::AppState;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

type AppStateHandle = Arc<RwLock<AppState>>;

#[tauri::command]
pub async fn blog_lint(
    state: State<'_, AppStateHandle>,
    post_id: String,
) -> Result<Vec<LintIssue>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;

    let content: String = conn.query_row(
        "SELECT COALESCE(draft_content, content, '') FROM blog_posts WHERE id = ?1",
        rusqlite::params![post_id],
        |r| r.get(0),
    ).map_err(|_| format!("Post {} not found", post_id))?;

    Ok(lint(&content))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_gap_detected() {
        let md = "# Title\n\n### Skipped H2\n\nContent";
        let issues = lint(md);
        assert!(issues.iter().any(|i| i.rule == "heading_hierarchy"),
            "expected heading_hierarchy issue");
    }

    #[test]
    fn duplicate_heading_detected() {
        let md = "# Title\n\n## Intro\n\nContent\n\n## Intro\n\nMore";
        let issues = lint(md);
        assert!(issues.iter().any(|i| i.rule == "duplicate_heading"));
    }

    #[test]
    fn missing_alt_text_detected() {
        let md = "Some text\n\n![](https://example.com/img.png)\n\nMore";
        let issues = lint(md);
        assert!(issues.iter().any(|i| i.rule == "missing_alt_text"));
    }

    #[test]
    fn unclosed_fence_detected() {
        let md = "```rust\nfn main() {}\n\nNo closing fence";
        let issues = lint(md);
        assert!(issues.iter().any(|i| i.rule == "unclosed_code_fence"));
    }

    #[test]
    fn clean_post_has_no_issues() {
        let md = "# Title\n\n## Section\n\nA short paragraph.\n\n## Another\n\nAnother paragraph.";
        let issues = lint(md);
        let critical: Vec<_> = issues.iter()
            .filter(|i| i.rule != "thin_section")
            .collect();
        assert!(critical.is_empty(), "expected no issues, got: {:?}", critical);
    }
}
