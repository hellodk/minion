# Blog v3 Phase C-1 — LLM Assistant MVP

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a slide-out LLM assistant panel to the blog editor with rule-based lint checks, AI title generation, grammar analysis, SEO meta description, tag suggestions, social snippets, and platform adapter.

**Architecture:** Two new Rust files — `blog_lint.rs` (deterministic checks, no LLM) and `blog_llm.rs` (LLM calls reusing the `get_endpoint`/`call_llm` pattern from `sysmon_analysis.rs`). All LLM outputs are ephemeral or stored in `blog_post_variants` (migration 018). A new `LlmAssistantPanel.tsx` slides in from the right edge of the blog editor.

**Tech Stack:** Rust `pulldown-cmark` AST for lint (already in Cargo.toml), `reqwest` for LLM calls, SolidJS for the panel UI. Migration 018 adds `blog_post_variants` table and `social_snippets_json` column.

---

## File Map

| Action | Path | Responsibility |
|---|---|---|
| Modify | `crates/minion-db/src/migrations.rs` | Migration 018 — blog_post_variants + social_snippets_json |
| Create | `src-tauri/src/blog_lint.rs` | 9 rule-based lint checks, no LLM |
| Create | `src-tauri/src/blog_llm.rs` | 12 LLM commands: titles, hook, conclusion, grammar, meta desc, tags, snippets, adapter, tone |
| Modify | `src-tauri/src/lib.rs` | Declare modules, register 14 new commands |
| Create | `ui/src/pages/blog/LlmAssistantPanel.tsx` | Slide-out panel: Lint + Writing + SEO + Distribute tabs |
| Modify | `ui/src/pages/Blog.tsx` | Add "✨ AI" toggle button, wire LlmAssistantPanel |

---

## Task 1: Migration 018

**Files:**
- Modify: `crates/minion-db/src/migrations.rs`

- [ ] **Step 1: Write the failing test**

In `crates/minion-db/src/migrations.rs`, inside `#[cfg(test)] mod tests`, add after the last test:

```rust
#[test]
fn test_migration_018_blog_llm_schema() {
    let conn = setup_test_db();
    run(&conn).expect("migrations failed");

    // social_snippets_json column must exist on blog_posts
    conn.execute(
        "UPDATE blog_posts SET social_snippets_json = '{\"twitter\":\"test\"}' WHERE 1=0",
        [],
    ).expect("social_snippets_json column missing");

    // blog_post_variants table must exist
    conn.execute(
        "INSERT INTO blog_post_variants (id, post_id, variant_type, label, content)
         VALUES ('v1','p1','test','Test','content')",
        [],
    ).expect("blog_post_variants insert failed");
}
```

- [ ] **Step 2: Run — confirm FAILS**

```bash
cd /home/dk/Documents/git/minion && cargo test -p minion-db test_migration_018_blog_llm_schema 2>&1 | tail -6
```
Expected: `FAILED` — column/table does not exist.

- [ ] **Step 3: Add to migrations array**

After `("017_fitness_gfit_columns", migrate_017_fitness_gfit_columns),` add:
```rust
("018_blog_llm", migrate_018_blog_llm),
```

- [ ] **Step 4: Implement the migration**

Add before `#[cfg(test)]`:

```rust
fn migrate_018_blog_llm(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        ALTER TABLE blog_posts ADD COLUMN social_snippets_json TEXT;

        CREATE TABLE IF NOT EXISTS blog_post_variants (
            id           TEXT PRIMARY KEY,
            post_id      TEXT NOT NULL REFERENCES blog_posts(id) ON DELETE CASCADE,
            variant_type TEXT NOT NULL,
            label        TEXT NOT NULL,
            content      TEXT NOT NULL,
            created_at   TEXT DEFAULT CURRENT_TIMESTAMP
        );
        CREATE INDEX IF NOT EXISTS idx_blog_variants_post
            ON blog_post_variants(post_id);
        ",
    )?;
    Ok(())
}
```

- [ ] **Step 5: Update migration count assertions**

```bash
grep -rn "assert_eq!(count, 17)" crates/minion-db/src/
```
Change all matches from `17` to `18`.

- [ ] **Step 6: Run — confirm PASSES**

```bash
cargo test -p minion-db 2>&1 | tail -6
```
Expected: all pass including `test_migration_018_blog_llm_schema`.

- [ ] **Step 7: Commit**

```bash
git add crates/minion-db/src/migrations.rs crates/minion-db/src/lib.rs
git commit -m "feat(blog): migration 018 — blog_post_variants + social_snippets_json"
```

---

## Task 2: blog_lint.rs — Rule-Based Checks

**Files:**
- Create: `src-tauri/src/blog_lint.rs`

- [ ] **Step 1: Create the file**

Create `/home/dk/Documents/git/minion/src-tauri/src/blog_lint.rs`:

```rust
//! Rule-based blog lint checks — no LLM required.
//!
//! Returns structured issues with anchor-based positioning (nearest preceding
//! heading text). Never uses line numbers — LLMs can't produce reliable ones
//! and they shift when content changes.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintIssue {
    /// Stable ID: hash of (rule, anchor, description)
    pub id: String,
    /// Machine-readable rule name
    pub rule: String,
    /// Text of the nearest preceding heading (empty = before first heading)
    pub anchor: String,
    /// Human-readable problem description
    pub description: String,
    /// Human-readable fix suggestion
    pub suggestion: String,
    /// True when the backend can apply the fix without user editing
    pub auto_fixable: bool,
}

impl LintIssue {
    fn new(
        rule: &str, anchor: &str, description: &str, suggestion: &str, auto_fixable: bool,
    ) -> Self {
        let mut h = Sha256::new();
        h.update(rule); h.update(anchor); h.update(description);
        let id = format!("{:.16x}", u64::from_be_bytes(h.finalize()[..8].try_into().unwrap()));
        Self {
            id, rule: rule.to_string(), anchor: anchor.to_string(),
            description: description.to_string(), suggestion: suggestion.to_string(),
            auto_fixable,
        }
    }
}

/// Run all rule-based lint checks against `content` (raw Markdown).
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

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Parse headings from content, returning (level, text) pairs in order.
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

/// Find the nearest preceding heading anchor for a given byte offset.
/// Walks the raw markdown looking for `## heading` lines.
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

// ── Checks ────────────────────────────────────────────────────────────────────

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
    // Match markdown images with empty alt: ![](...) 
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
    let mut offset = 0usize;
    // Look for '](' not preceded by '!'  (link without closing paren handled by parser)
    let parser = Parser::new_ext(content, Options::all());
    for event in parser {
        if let Event::Start(Tag::Link { dest_url, .. }) = event {
            if dest_url.is_empty() {
                issues.push(LintIssue::new(
                    "empty_link_url",
                    &anchor_before(content, offset),
                    "Link has an empty URL",
                    "Add a valid URL or remove the link markup",
                    false,
                ));
            }
        }
        offset += 1; // approximate
    }
    issues
}

fn check_thin_sections(content: &str) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    let headings = parse_headings(content);
    if headings.is_empty() { return issues; }

    // Split content into sections by heading
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
    // Check last section
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

// ── Tauri commands ────────────────────────────────────────────────────────────

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
```

- [ ] **Step 2: Verify it compiles and tests pass**

```bash
cd /home/dk/Documents/git/minion/src-tauri && cargo test blog_lint 2>&1 | tail -10
```
Expected: 5 tests pass. (Needs lib.rs mod declaration first — add it temporarily or proceed to Task 3 step 1 first.)

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/blog_lint.rs
git commit -m "feat(blog): blog_lint — 7 rule-based lint checks with anchor positioning"
```

---

## Task 3: blog_llm.rs — LLM Commands

**Files:**
- Create: `src-tauri/src/blog_llm.rs`

- [ ] **Step 1: Create the file**

Create `/home/dk/Documents/git/minion/src-tauri/src/blog_llm.rs`:

```rust
//! LLM-powered blog assistant commands.
//!
//! All functions degrade gracefully — Ok(None) when no endpoint is configured
//! or any LLM call fails. Never surfaces LLM errors to the user.

use crate::state::AppState;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;
use uuid::Uuid;

type AppStateHandle = Arc<RwLock<AppState>>;
type Conn = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;

// ── Shared types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlogVariant {
    pub id: String,
    pub post_id: String,
    pub variant_type: String,
    pub label: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TitleSuggestion {
    pub style: String,   // "seo" | "curiosity" | "direct" | "question" | "listicle"
    pub title: String,
    pub rationale: String,
}

// ── LLM helpers (same pattern as sysmon_analysis.rs) ─────────────────────────

fn get_endpoint(conn: &Conn) -> Option<(String, Option<String>, String)> {
    conn.query_row(
        "SELECT base_url, api_key_encrypted, COALESCE(default_model,'llama3')
         FROM llm_endpoints LIMIT 1",
        [],
        |r| Ok((r.get::<_,String>(0)?, r.get::<_,Option<String>>(1)?, r.get::<_,String>(2)?)),
    ).ok()
}

async fn call_llm(
    base_url: &str, api_key: Option<&str>, model: &str,
    system: &str, user: &str,
) -> Option<String> {
    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role":"system","content":system},
            {"role":"user","content":user}
        ],
        "stream": false
    });
    let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build().ok()?;
    let mut req = client.post(&url).json(&body);
    if let Some(k) = api_key { if !k.is_empty() { req = req.bearer_auth(k); } }
    let resp = req.send().await.map_err(|e| tracing::warn!("LLM call failed: {e}")).ok()?;
    if !resp.status().is_success() {
        tracing::warn!("LLM returned {}", resp.status());
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    json["choices"][0]["message"]["content"].as_str().map(|s| s.to_string())
}

/// Fetch post content from DB. Returns (title, content).
fn fetch_post(conn: &Conn, post_id: &str) -> Result<(String, String), String> {
    conn.query_row(
        "SELECT title, COALESCE(draft_content, content, '') FROM blog_posts WHERE id = ?1",
        params![post_id],
        |r| Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?)),
    ).map_err(|_| format!("Post {} not found", post_id))
}

/// Estimate tokens (rough: chars / 4).
fn estimate_tokens(text: &str) -> usize { text.len() / 4 }

/// Store a variant in blog_post_variants. Returns the new variant id.
fn store_variant(conn: &Conn, post_id: &str, variant_type: &str, label: &str, content: &str)
    -> Result<String, String>
{
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO blog_post_variants (id, post_id, variant_type, label, content, created_at)
         VALUES (?1,?2,?3,?4,?5,?6)",
        params![id, post_id, variant_type, label, content, now],
    ).map_err(|e| e.to_string())?;
    Ok(id)
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Generate 5 title alternatives in different styles.
/// Returns None gracefully if no LLM endpoint configured.
#[tauri::command]
pub async fn blog_llm_titles(
    state: State<'_, AppStateHandle>,
    post_id: String,
) -> Result<Option<Vec<TitleSuggestion>>, String> {
    let db = { state.read().await.db.clone() };
    let (title, content) = { let c = db.get().map_err(|e|e.to_string())?; fetch_post(&c, &post_id)? };
    let Some((base_url, api_key, model)) = { let c = db.get().map_err(|e|e.to_string())?; get_endpoint(&c) } else {
        return Ok(None);
    };

    let tokens = estimate_tokens(&content);
    let excerpt = if tokens > 2000 { &content[..content.len().min(8000)] } else { &content };

    let system = "You are a blog title expert. Respond with exactly 5 lines, each in the format:\n\
                  STYLE: TITLE | RATIONALE\n\
                  Styles: seo, curiosity, direct, question, listicle";
    let user = format!("Current title: {}\n\nContent excerpt:\n{}", title, excerpt);

    let Some(raw) = call_llm(&base_url, api_key.as_deref(), &model, system, &user).await else {
        return Ok(None);
    };

    let suggestions: Vec<TitleSuggestion> = raw.lines()
        .filter_map(|line| {
            let (style_title, rationale) = line.split_once(" | ")?;
            let (style, title) = style_title.split_once(": ")?;
            Some(TitleSuggestion {
                style: style.trim().to_lowercase(),
                title: title.trim().to_string(),
                rationale: rationale.trim().to_string(),
            })
        })
        .collect();

    Ok(if suggestions.is_empty() { None } else { Some(suggestions) })
}

/// Rewrite the opening paragraph — returns 3 variants (direct, story, question).
#[tauri::command]
pub async fn blog_llm_hook(
    state: State<'_, AppStateHandle>,
    post_id: String,
) -> Result<Option<Vec<String>>, String> {
    let db = { state.read().await.db.clone() };
    let (title, content) = { let c = db.get().map_err(|e|e.to_string())?; fetch_post(&c, &post_id)? };

    // Extract first non-empty paragraph
    let first_para: String = content.lines()
        .skip_while(|l| l.trim().starts_with('#') || l.trim().is_empty())
        .take_while(|l| !l.trim().is_empty())
        .collect::<Vec<_>>().join(" ");
    if first_para.is_empty() { return Ok(None); }

    let Some((base_url, api_key, model)) = { let c = db.get().map_err(|e|e.to_string())?; get_endpoint(&c) } else {
        return Ok(None);
    };

    let system = "You are a blog writing coach. Rewrite the opening paragraph 3 ways to maximise reader retention. \
                  Respond with exactly 3 paragraphs separated by --- (triple dash on its own line). \
                  Styles: 1) Direct/declarative, 2) Story/anecdote, 3) Question-led.";
    let user = format!("Post title: {}\n\nCurrent opening:\n{}", title, first_para);

    let Some(raw) = call_llm(&base_url, api_key.as_deref(), &model, system, &user).await else {
        return Ok(None);
    };

    let variants: Vec<String> = raw.split("---")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    Ok(if variants.is_empty() { None } else { Some(variants) })
}

/// Suggest a stronger conclusion and CTA.
#[tauri::command]
pub async fn blog_llm_conclusion(
    state: State<'_, AppStateHandle>,
    post_id: String,
) -> Result<Option<String>, String> {
    let db = { state.read().await.db.clone() };
    let (title, content) = { let c = db.get().map_err(|e|e.to_string())?; fetch_post(&c, &post_id)? };
    let Some((base_url, api_key, model)) = { let c = db.get().map_err(|e|e.to_string())?; get_endpoint(&c) } else {
        return Ok(None);
    };

    let system = "You are a blog writing coach. Write a strong 2-3 sentence conclusion paragraph \
                  plus a 1-sentence call to action. Be concise, specific, and direct. \
                  Return only the conclusion text — no preamble.";
    let user = format!("Post title: {}\n\nPost content (last 1000 chars):\n{}",
        title, &content[content.len().saturating_sub(1000)..]);

    Ok(call_llm(&base_url, api_key.as_deref(), &model, system, &user).await)
}

/// Grammar and language quality: passive voice, weak verbs, filler words.
/// Returns a list of issues as plain text (one per line: "ISSUE: original → suggestion").
#[tauri::command]
pub async fn blog_llm_grammar(
    state: State<'_, AppStateHandle>,
    post_id: String,
) -> Result<Option<Vec<String>>, String> {
    let db = { state.read().await.db.clone() };
    let (_, content) = { let c = db.get().map_err(|e|e.to_string())?; fetch_post(&c, &post_id)? };
    let Some((base_url, api_key, model)) = { let c = db.get().map_err(|e|e.to_string())?; get_endpoint(&c) } else {
        return Ok(None);
    };

    let excerpt = &content[..content.len().min(6000)];
    let system = "You are a grammar and style editor. Find issues: passive voice, weak verbs (is/was/get/got/have), \
                  filler words (very/just/really/quite/thing/stuff), and redundant phrases. \
                  Return each issue on its own line in the format: ISSUE_TYPE: \"original text\" → \"suggested rewrite\"\n\
                  Return at most 15 issues. Return only the issue lines — no preamble or summary.";
    let user = format!("Blog content:\n{}", excerpt);

    let Some(raw) = call_llm(&base_url, api_key.as_deref(), &model, system, &user).await else {
        return Ok(None);
    };

    let issues: Vec<String> = raw.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && l.contains('→'))
        .collect();

    Ok(if issues.is_empty() { None } else { Some(issues) })
}

/// Generate a 150-160 char SEO meta description. Optionally saves to excerpt.
#[tauri::command]
pub async fn blog_llm_meta_description(
    state: State<'_, AppStateHandle>,
    post_id: String,
    save_to_excerpt: bool,
) -> Result<Option<String>, String> {
    let db = { state.read().await.db.clone() };
    let (title, content) = { let c = db.get().map_err(|e|e.to_string())?; fetch_post(&c, &post_id)? };
    let Some((base_url, api_key, model)) = { let c = db.get().map_err(|e|e.to_string())?; get_endpoint(&c) } else {
        return Ok(None);
    };

    let excerpt = &content[..content.len().min(3000)];
    let system = "You are an SEO expert. Write exactly one meta description of 150-160 characters. \
                  Include the primary keyword naturally. Write in active voice. \
                  Return only the description text — no quotes, no preamble.";
    let user = format!("Post title: {}\n\nContent:\n{}", title, excerpt);

    let Some(desc) = call_llm(&base_url, api_key.as_deref(), &model, system, &user).await else {
        return Ok(None);
    };

    let desc = desc.trim().to_string();
    if save_to_excerpt && !desc.is_empty() {
        let c = db.get().map_err(|e| e.to_string())?;
        c.execute(
            "UPDATE blog_posts SET excerpt = ?1, updated_at = ?2 WHERE id = ?3",
            params![desc, chrono::Utc::now().to_rfc3339(), post_id],
        ).map_err(|e| e.to_string())?;
    }

    Ok(Some(desc))
}

/// Suggest tags from existing tag library + up to 3 new ones.
#[tauri::command]
pub async fn blog_llm_tags(
    state: State<'_, AppStateHandle>,
    post_id: String,
) -> Result<Option<Vec<String>>, String> {
    let db = { state.read().await.db.clone() };
    let (title, content) = { let c = db.get().map_err(|e|e.to_string())?; fetch_post(&c, &post_id)? };

    let existing_tags: Vec<String> = {
        let c = db.get().map_err(|e|e.to_string())?;
        let mut stmt = c.prepare("SELECT name FROM blog_tags ORDER BY name").map_err(|e|e.to_string())?;
        stmt.query_map([], |r| r.get::<_,String>(0))
            .map_err(|e|e.to_string())?
            .filter_map(|r| r.ok()).collect()
    };

    let Some((base_url, api_key, model)) = { let c = db.get().map_err(|e|e.to_string())?; get_endpoint(&c) } else {
        return Ok(None);
    };

    let excerpt = &content[..content.len().min(3000)];
    let system = "You are a content tagging expert. Return a comma-separated list of 5-8 tags. \
                  Prefer tags from the existing list when relevant. Add new ones only when the post \
                  clearly covers something not in the list. Return only the comma-separated tag names — no explanation.";
    let user = format!(
        "Post title: {}\nExisting tags: {}\n\nContent:\n{}",
        title, existing_tags.join(", "), excerpt
    );

    let Some(raw) = call_llm(&base_url, api_key.as_deref(), &model, system, &user).await else {
        return Ok(None);
    };

    let tags: Vec<String> = raw.split(',')
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty() && t.len() < 50)
        .collect();

    Ok(if tags.is_empty() { None } else { Some(tags) })
}

/// Generate social snippets for Twitter/X, LinkedIn, Substack, and generic.
/// Stores result in blog_posts.social_snippets_json.
#[tauri::command]
pub async fn blog_llm_snippets(
    state: State<'_, AppStateHandle>,
    post_id: String,
) -> Result<Option<serde_json::Value>, String> {
    let db = { state.read().await.db.clone() };
    let (title, content) = { let c = db.get().map_err(|e|e.to_string())?; fetch_post(&c, &post_id)? };
    let Some((base_url, api_key, model)) = { let c = db.get().map_err(|e|e.to_string())?; get_endpoint(&c) } else {
        return Ok(None);
    };

    let excerpt = &content[..content.len().min(3000)];
    let system = "You are a social media expert. Generate 4 promotional snippets for a blog post.\n\
                  Return EXACTLY in this format (each on its own line):\n\
                  TWITTER: <270 chars max, hook + emoji>\n\
                  LINKEDIN: <800 chars max, hook + 3 bullet takeaways + hashtags>\n\
                  SUBSTACK: <500 chars max, curiosity-gap teaser>\n\
                  GENERIC: <280 chars max, balanced>\n\
                  Do not include any other text.";
    let user = format!("Post title: {}\n\nContent:\n{}", title, excerpt);

    let Some(raw) = call_llm(&base_url, api_key.as_deref(), &model, system, &user).await else {
        return Ok(None);
    };

    let mut snippets = serde_json::json!({});
    for line in raw.lines() {
        for key in &["TWITTER", "LINKEDIN", "SUBSTACK", "GENERIC"] {
            let prefix = format!("{}: ", key);
            if line.starts_with(&prefix) {
                snippets[key.to_lowercase()] = serde_json::json!(line[prefix.len()..].trim());
            }
        }
    }

    if snippets.as_object().map(|m| m.is_empty()).unwrap_or(true) {
        return Ok(None);
    }

    // Persist to DB
    let json_str = snippets.to_string();
    let c = db.get().map_err(|e| e.to_string())?;
    c.execute(
        "UPDATE blog_posts SET social_snippets_json = ?1, updated_at = ?2 WHERE id = ?3",
        params![json_str, chrono::Utc::now().to_rfc3339(), post_id],
    ).map_err(|e| e.to_string())?;

    Ok(Some(snippets))
}

/// Adapt post for a target platform. Stores result as a blog_post_variant.
/// Returns the variant id + adapted content.
#[tauri::command]
pub async fn blog_llm_adapt(
    state: State<'_, AppStateHandle>,
    post_id: String,
    platform: String,  // "devto" | "hashnode" | "medium" | "substack" | "linkedin"
) -> Result<Option<BlogVariant>, String> {
    let db = { state.read().await.db.clone() };
    let (title, content) = { let c = db.get().map_err(|e|e.to_string())?; fetch_post(&c, &post_id)? };
    let Some((base_url, api_key, model)) = { let c = db.get().map_err(|e|e.to_string())?; get_endpoint(&c) } else {
        return Ok(None);
    };

    let platform_instructions = match platform.as_str() {
        "devto"     => "Dev.to style: technical, conversational, add a short TL;DR at top, use informal tone, \
                        add canonical URL note at the bottom as: 'Originally published at [URL]'",
        "hashnode"  => "Hashnode style: add a subtitle below the title, use numbered lists for steps, \
                        include a 'Key takeaways' section at the end",
        "medium"    => "Medium style: narrative-driven, add scene-setting opening, \
                        break up code-heavy sections with more prose explanation",
        "substack"  => "Substack newsletter style: personal opener (e.g. 'Hey friends,'), \
                        casual conversational tone, end with a personal sign-off and newsletter CTA",
        "linkedin"  => "LinkedIn article style: compress to key points, \
                        add bold headers for each major point, \
                        end with a question to drive comments",
        _           => return Err(format!("Unknown platform: {}", platform)),
    };

    let tokens = estimate_tokens(&content);
    let input = if tokens > 3000 { &content[..content.len().min(12000)] } else { &content };

    let system = format!(
        "You are a content adaptation expert. Rewrite the following blog post for {}. \
         Instructions: {}\n\
         Return only the adapted post content in Markdown — no preamble.",
        platform, platform_instructions
    );
    let user = format!("Title: {}\n\n{}", title, input);

    let Some(adapted) = call_llm(&base_url, api_key.as_deref(), &model, &system, &user).await else {
        return Ok(None);
    };

    let label = format!("{} adaptation", platform);
    let variant_type = format!("platform_{}", platform);
    let c = db.get().map_err(|e| e.to_string())?;
    let variant_id = store_variant(&c, &post_id, &variant_type, &label, &adapted)?;
    let now = chrono::Utc::now().to_rfc3339();

    Ok(Some(BlogVariant {
        id: variant_id, post_id: post_id.clone(),
        variant_type, label, content: adapted, created_at: now,
    }))
}

/// Rewrite post in a different tone. Stores result as a variant.
#[tauri::command]
pub async fn blog_llm_tone(
    state: State<'_, AppStateHandle>,
    post_id: String,
    target: String,  // "technical" | "balanced" | "conversational"
) -> Result<Option<BlogVariant>, String> {
    let db = { state.read().await.db.clone() };
    let (title, content) = { let c = db.get().map_err(|e|e.to_string())?; fetch_post(&c, &post_id)? };
    let Some((base_url, api_key, model)) = { let c = db.get().map_err(|e|e.to_string())?; get_endpoint(&c) } else {
        return Ok(None);
    };

    let instruction = match target.as_str() {
        "technical"      => "formal, precise, use technical terminology, no contractions",
        "balanced"       => "clear and professional but approachable, use some contractions",
        "conversational" => "casual, friendly, use contractions, first-person, relatable examples",
        _ => return Err(format!("Unknown tone: {}. Use: technical | balanced | conversational", target)),
    };

    let tokens = estimate_tokens(&content);
    let input = if tokens > 3000 { &content[..content.len().min(12000)] } else { &content };

    let system = format!(
        "Rewrite the following blog post with a {} tone ({}). \
         Preserve all headings, code blocks, and factual content. \
         Only change wording and sentence structure. Return Markdown only.",
        target, instruction
    );
    let user = format!("Title: {}\n\n{}", title, input);

    let Some(rewritten) = call_llm(&base_url, api_key.as_deref(), &model, &system, &user).await else {
        return Ok(None);
    };

    let label = format!("{} tone", target);
    let variant_type = format!("tone_{}", target);
    let c = db.get().map_err(|e| e.to_string())?;
    let variant_id = store_variant(&c, &post_id, &variant_type, &label, &rewritten)?;
    let now = chrono::Utc::now().to_rfc3339();

    Ok(Some(BlogVariant {
        id: variant_id, post_id,
        variant_type, label, content: rewritten, created_at: now,
    }))
}

// ── Variant management ────────────────────────────────────────────────────────

#[tauri::command]
pub async fn blog_get_variants(
    state: State<'_, AppStateHandle>,
    post_id: String,
) -> Result<Vec<BlogVariant>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, post_id, variant_type, label, content, created_at
         FROM blog_post_variants WHERE post_id = ?1 ORDER BY created_at DESC"
    ).map_err(|e| e.to_string())?;
    let rows: Vec<BlogVariant> = stmt
        .query_map(params![post_id], |r| Ok(BlogVariant {
            id: r.get(0)?, post_id: r.get(1)?, variant_type: r.get(2)?,
            label: r.get(3)?, content: r.get(4)?, created_at: r.get(5)?,
        }))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok()).collect();
    Ok(rows)
}

#[tauri::command]
pub async fn blog_delete_variant(
    state: State<'_, AppStateHandle>,
    variant_id: String,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM blog_post_variants WHERE id = ?1", params![variant_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Get stored social snippets for a post.
#[tauri::command]
pub async fn blog_get_snippets(
    state: State<'_, AppStateHandle>,
    post_id: String,
) -> Result<Option<serde_json::Value>, String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let json: Option<String> = conn.query_row(
        "SELECT social_snippets_json FROM blog_posts WHERE id = ?1",
        params![post_id],
        |r| r.get(0),
    ).ok().flatten();
    Ok(json.and_then(|j| serde_json::from_str(&j).ok()))
}
```

- [ ] **Step 2: Verify it compiles (after lib.rs is wired in Task 4)**

```bash
cd /home/dk/Documents/git/minion/src-tauri && cargo check 2>&1 | grep "^error" | head -10
```
Expected: no errors (run after Task 4).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/blog_llm.rs
git commit -m "feat(blog): blog_llm — 11 LLM commands: titles, hook, conclusion, grammar, meta desc, tags, snippets, adapter, tone, variant CRUD"
```

---

## Task 4: Wire lib.rs

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add module declarations**

In `src-tauri/src/lib.rs`, after `mod blog_preview;` add:
```rust
mod blog_lint;
mod blog_llm;
```

- [ ] **Step 2: Register all new commands in generate_handler!**

After the last `blog_preview::` entry, add:
```rust
// Blog v3 Phase C — Lint + LLM assistant
blog_lint::blog_lint,
blog_llm::blog_llm_titles,
blog_llm::blog_llm_hook,
blog_llm::blog_llm_conclusion,
blog_llm::blog_llm_grammar,
blog_llm::blog_llm_meta_description,
blog_llm::blog_llm_tags,
blog_llm::blog_llm_snippets,
blog_llm::blog_llm_adapt,
blog_llm::blog_llm_tone,
blog_llm::blog_get_variants,
blog_llm::blog_delete_variant,
blog_llm::blog_get_snippets,
```

- [ ] **Step 3: Build to confirm**

```bash
cd /home/dk/Documents/git/minion && cargo build 2>&1 | grep "^error" | head -10
```
Expected: no errors.

- [ ] **Step 4: Run all tests**

```bash
cargo test --workspace 2>&1 | grep "FAILED\|test result" | tail -10
```
Expected: all pass including the 5 blog_lint tests.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(blog): wire blog_lint and blog_llm modules + 13 commands into lib.rs"
```

---

## Task 5: LlmAssistantPanel.tsx

**Files:**
- Create: `ui/src/pages/blog/LlmAssistantPanel.tsx`

- [ ] **Step 1: Create the panel component**

Create `/home/dk/Documents/git/minion/ui/src/pages/blog/LlmAssistantPanel.tsx`:

```tsx
import { Component, createSignal, For, Show, onMount } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

interface LintIssue {
  id: string;
  rule: string;
  anchor: string;
  description: string;
  suggestion: string;
  auto_fixable: boolean;
}

interface TitleSuggestion {
  style: string;
  title: string;
  rationale: string;
}

interface BlogVariant {
  id: string;
  post_id: string;
  variant_type: string;
  label: string;
  content: string;
  created_at: string;
}

type PanelTab = 'lint' | 'writing' | 'seo' | 'distribute';

const btn = (label: string, loading: boolean, onClick: () => void, color = 'sky') => (
  <button
    onClick={onClick}
    disabled={loading}
    class={`px-3 py-1.5 text-xs font-medium rounded-lg border transition-colors cursor-pointer
            disabled:opacity-50 disabled:cursor-not-allowed
            ${color === 'sky'
              ? 'bg-sky-50 dark:bg-sky-900/20 border-sky-200 dark:border-sky-800 text-sky-700 dark:text-sky-300 hover:bg-sky-100'
              : 'bg-gray-50 dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700'
            }`}
  >
    {loading ? '…' : label}
  </button>
);

const ResultBox: Component<{ content: string; onCopy?: () => void }> = (props) => (
  <div class="relative mt-2 p-3 bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg text-xs text-gray-700 dark:text-gray-300 whitespace-pre-wrap leading-relaxed max-h-48 overflow-y-auto">
    {props.content}
    <Show when={props.onCopy}>
      <button
        onClick={props.onCopy}
        class="absolute top-2 right-2 px-2 py-0.5 text-[10px] bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-600 rounded text-gray-500 hover:text-gray-800 dark:hover:text-gray-200"
      >Copy</button>
    </Show>
  </div>
);

const LlmAssistantPanel: Component<{ postId: string | null; onClose: () => void }> = (props) => {
  const [tab, setTab] = createSignal<PanelTab>('lint');

  // ── Lint state ────────────────────────────────────────────────────────────
  const [lintIssues, setLintIssues] = createSignal<LintIssue[]>([]);
  const [lintLoading, setLintLoading] = createSignal(false);

  const runLint = async () => {
    if (!props.postId) return;
    setLintLoading(true);
    try {
      const issues = await invoke<LintIssue[]>('blog_lint', { postId: props.postId });
      setLintIssues(issues);
    } catch { setLintIssues([]); }
    finally { setLintLoading(false); }
  };

  // ── Writing state ─────────────────────────────────────────────────────────
  const [titles, setTitles] = createSignal<TitleSuggestion[]>([]);
  const [titlesLoading, setTitlesLoading] = createSignal(false);
  const [hook, setHook] = createSignal<string[]>([]);
  const [hookLoading, setHookLoading] = createSignal(false);
  const [conclusion, setConclusion] = createSignal('');
  const [conclusionLoading, setConclusionLoading] = createSignal(false);
  const [grammar, setGrammar] = createSignal<string[]>([]);
  const [grammarLoading, setGrammarLoading] = createSignal(false);

  const runTitles = async () => {
    if (!props.postId) return;
    setTitlesLoading(true);
    try { setTitles(await invoke<TitleSuggestion[]>('blog_llm_titles', { postId: props.postId }) ?? []); }
    catch { } finally { setTitlesLoading(false); }
  };

  const runHook = async () => {
    if (!props.postId) return;
    setHookLoading(true);
    try { setHook(await invoke<string[]>('blog_llm_hook', { postId: props.postId }) ?? []); }
    catch { } finally { setHookLoading(false); }
  };

  const runConclusion = async () => {
    if (!props.postId) return;
    setConclusionLoading(true);
    try { setConclusion(await invoke<string>('blog_llm_conclusion', { postId: props.postId }) ?? ''); }
    catch { } finally { setConclusionLoading(false); }
  };

  const runGrammar = async () => {
    if (!props.postId) return;
    setGrammarLoading(true);
    try { setGrammar(await invoke<string[]>('blog_llm_grammar', { postId: props.postId }) ?? []); }
    catch { } finally { setGrammarLoading(false); }
  };

  // ── SEO state ─────────────────────────────────────────────────────────────
  const [metaDesc, setMetaDesc] = createSignal('');
  const [metaLoading, setMetaLoading] = createSignal(false);
  const [tags, setTags] = createSignal<string[]>([]);
  const [tagsLoading, setTagsLoading] = createSignal(false);

  const runMetaDesc = async () => {
    if (!props.postId) return;
    setMetaLoading(true);
    try { setMetaDesc(await invoke<string>('blog_llm_meta_description', { postId: props.postId, saveToExcerpt: true }) ?? ''); }
    catch { } finally { setMetaLoading(false); }
  };

  const runTags = async () => {
    if (!props.postId) return;
    setTagsLoading(true);
    try { setTags(await invoke<string[]>('blog_llm_tags', { postId: props.postId }) ?? []); }
    catch { } finally { setTagsLoading(false); }
  };

  // ── Distribution state ────────────────────────────────────────────────────
  const [snippets, setSnippets] = createSignal<Record<string, string>>({});
  const [snippetsLoading, setSnippetsLoading] = createSignal(false);
  const [variants, setVariants] = createSignal<BlogVariant[]>([]);
  const [adaptLoading, setAdaptLoading] = createSignal(false);
  const [adaptPlatform, setAdaptPlatform] = createSignal('devto');
  const [toneLoading, setToneLoading] = createSignal(false);
  const [tonePlatform, setTonePlatform] = createSignal('balanced');

  const runSnippets = async () => {
    if (!props.postId) return;
    setSnippetsLoading(true);
    try {
      const s = await invoke<Record<string, string>>('blog_llm_snippets', { postId: props.postId });
      setSnippets(s ?? {});
    } catch { } finally { setSnippetsLoading(false); }
  };

  const loadVariants = async () => {
    if (!props.postId) return;
    try { setVariants(await invoke<BlogVariant[]>('blog_get_variants', { postId: props.postId })); }
    catch { }
  };

  const runAdapt = async () => {
    if (!props.postId) return;
    setAdaptLoading(true);
    try {
      await invoke('blog_llm_adapt', { postId: props.postId, platform: adaptPlatform() });
      await loadVariants();
    } catch { } finally { setAdaptLoading(false); }
  };

  const runTone = async () => {
    if (!props.postId) return;
    setToneLoading(true);
    try {
      await invoke('blog_llm_tone', { postId: props.postId, target: tonePlatform() });
      await loadVariants();
    } catch { } finally { setToneLoading(false); }
  };

  const deleteVariant = async (id: string) => {
    await invoke('blog_delete_variant', { variantId: id }).catch(() => {});
    await loadVariants();
  };

  onMount(async () => {
    await runLint();
    if (props.postId) {
      const s = await invoke<Record<string,string>>('blog_get_snippets', { postId: props.postId }).catch(() => null);
      if (s) setSnippets(s);
      await loadVariants();
    }
  });

  const tabs: { id: PanelTab; label: string; badge?: () => number }[] = [
    { id: 'lint',       label: 'Lint',      badge: () => lintIssues().length },
    { id: 'writing',    label: 'Writing' },
    { id: 'seo',        label: 'SEO' },
    { id: 'distribute', label: 'Distribute' },
  ];

  const noPost = () => !props.postId;

  return (
    <div class="flex flex-col h-full bg-white dark:bg-gray-800 border-l border-gray-200 dark:border-gray-700 w-80 shrink-0">
      {/* Header */}
      <div class="flex items-center justify-between px-3 py-2 border-b border-gray-200 dark:border-gray-700 shrink-0">
        <span class="text-sm font-semibold text-gray-800 dark:text-gray-200">✨ AI Assistant</span>
        <button onClick={props.onClose} class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 text-lg leading-none">×</button>
      </div>

      {/* Tabs */}
      <div class="flex border-b border-gray-200 dark:border-gray-700 shrink-0">
        <For each={tabs}>
          {(t) => (
            <button
              onClick={() => setTab(t.id)}
              class={`flex-1 py-2 text-xs font-medium transition-colors relative ${
                tab() === t.id
                  ? 'text-sky-600 dark:text-sky-400 border-b-2 border-sky-500'
                  : 'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300'
              }`}
            >
              {t.label}
              <Show when={t.badge && t.badge() > 0}>
                <span class="ml-1 px-1 py-0.5 text-[9px] bg-amber-100 dark:bg-amber-900/40 text-amber-700 dark:text-amber-300 rounded-full">{t.badge!()}</span>
              </Show>
            </button>
          )}
        </For>
      </div>

      {/* Content */}
      <div class="flex-1 overflow-y-auto p-3 space-y-4">
        <Show when={noPost()}>
          <p class="text-xs text-gray-400 text-center pt-8">Open a post to use the assistant.</p>
        </Show>

        {/* ── LINT TAB ────────────────────────────────────────────────────── */}
        <Show when={tab() === 'lint' && !noPost()}>
          <div class="flex items-center justify-between">
            <span class="text-xs font-semibold text-gray-600 dark:text-gray-300">Rule-Based Checks</span>
            {btn('↻ Re-run', lintLoading(), runLint, 'gray')}
          </div>
          <Show when={lintIssues().length === 0 && !lintLoading()}>
            <p class="text-xs text-green-600 dark:text-green-400">✓ No issues found.</p>
          </Show>
          <For each={lintIssues()}>
            {(issue) => (
              <div class="p-2 border border-amber-200 dark:border-amber-800 bg-amber-50 dark:bg-amber-900/20 rounded-lg">
                <div class="flex items-start gap-2">
                  <span class="text-amber-500 mt-0.5">⚠</span>
                  <div class="min-w-0">
                    <Show when={issue.anchor}>
                      <div class="text-[10px] text-gray-400 mb-0.5 truncate">In: {issue.anchor}</div>
                    </Show>
                    <p class="text-xs text-gray-700 dark:text-gray-300">{issue.description}</p>
                    <p class="text-[11px] text-sky-600 dark:text-sky-400 mt-1">{issue.suggestion}</p>
                  </div>
                </div>
              </div>
            )}
          </For>
        </Show>

        {/* ── WRITING TAB ─────────────────────────────────────────────────── */}
        <Show when={tab() === 'writing' && !noPost()}>
          {/* Titles */}
          <div>
            <div class="flex items-center justify-between mb-2">
              <span class="text-xs font-semibold text-gray-600 dark:text-gray-300">Title Generator</span>
              {btn('Generate', titlesLoading(), runTitles)}
            </div>
            <Show when={titles().length > 0}>
              <div class="space-y-2">
                <For each={titles()}>
                  {(t) => (
                    <div class="p-2 border border-gray-200 dark:border-gray-700 rounded-lg">
                      <div class="flex justify-between items-start gap-1 mb-1">
                        <span class="text-[10px] px-1 py-0.5 bg-sky-100 dark:bg-sky-900/40 text-sky-600 dark:text-sky-400 rounded">{t.style}</span>
                        <button onClick={() => navigator.clipboard.writeText(t.title)} class="text-[10px] text-gray-400 hover:text-gray-600">Copy</button>
                      </div>
                      <p class="text-xs font-medium text-gray-800 dark:text-gray-200">{t.title}</p>
                      <p class="text-[10px] text-gray-400 mt-0.5">{t.rationale}</p>
                    </div>
                  )}
                </For>
              </div>
            </Show>
          </div>

          {/* Hook */}
          <div class="border-t border-gray-100 dark:border-gray-700 pt-3">
            <div class="flex items-center justify-between mb-2">
              <span class="text-xs font-semibold text-gray-600 dark:text-gray-300">Hook Rewriter</span>
              {btn('Rewrite', hookLoading(), runHook)}
            </div>
            <Show when={hook().length > 0}>
              <div class="space-y-2">
                <For each={hook()}>
                  {(v, i) => (
                    <div class="p-2 border border-gray-200 dark:border-gray-700 rounded-lg">
                      <div class="flex justify-between mb-1">
                        <span class="text-[10px] text-gray-400">Variant {i() + 1}</span>
                        <button onClick={() => navigator.clipboard.writeText(v)} class="text-[10px] text-gray-400 hover:text-gray-600">Copy</button>
                      </div>
                      <p class="text-xs text-gray-700 dark:text-gray-300">{v}</p>
                    </div>
                  )}
                </For>
              </div>
            </Show>
          </div>

          {/* Conclusion */}
          <div class="border-t border-gray-100 dark:border-gray-700 pt-3">
            <div class="flex items-center justify-between mb-2">
              <span class="text-xs font-semibold text-gray-600 dark:text-gray-300">Conclusion + CTA</span>
              {btn('Generate', conclusionLoading(), runConclusion)}
            </div>
            <Show when={conclusion()}>
              <ResultBox content={conclusion()} onCopy={() => navigator.clipboard.writeText(conclusion())} />
            </Show>
          </div>

          {/* Grammar */}
          <div class="border-t border-gray-100 dark:border-gray-700 pt-3">
            <div class="flex items-center justify-between mb-2">
              <span class="text-xs font-semibold text-gray-600 dark:text-gray-300">Grammar & Style</span>
              {btn('Analyse', grammarLoading(), runGrammar)}
            </div>
            <Show when={grammar().length > 0}>
              <div class="space-y-1">
                <For each={grammar()}>
                  {(issue) => (
                    <div class="p-2 bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded text-xs text-gray-700 dark:text-gray-300">
                      {issue}
                    </div>
                  )}
                </For>
              </div>
            </Show>
          </div>
        </Show>

        {/* ── SEO TAB ──────────────────────────────────────────────────────── */}
        <Show when={tab() === 'seo' && !noPost()}>
          {/* Meta description */}
          <div>
            <div class="flex items-center justify-between mb-2">
              <span class="text-xs font-semibold text-gray-600 dark:text-gray-300">Meta Description</span>
              {btn('Generate', metaLoading(), runMetaDesc)}
            </div>
            <Show when={metaDesc()}>
              <ResultBox content={metaDesc()} onCopy={() => navigator.clipboard.writeText(metaDesc())} />
              <p class="text-[10px] text-green-600 dark:text-green-400 mt-1">✓ Saved to post excerpt</p>
            </Show>
          </div>

          {/* Tags */}
          <div class="border-t border-gray-100 dark:border-gray-700 pt-3">
            <div class="flex items-center justify-between mb-2">
              <span class="text-xs font-semibold text-gray-600 dark:text-gray-300">Tag Suggestions</span>
              {btn('Suggest', tagsLoading(), runTags)}
            </div>
            <Show when={tags().length > 0}>
              <div class="flex flex-wrap gap-1 mt-2">
                <For each={tags()}>
                  {(tag) => (
                    <span class="px-2 py-0.5 bg-sky-50 dark:bg-sky-900/30 border border-sky-200 dark:border-sky-800 text-sky-700 dark:text-sky-300 text-[11px] rounded-full">
                      {tag}
                    </span>
                  )}
                </For>
              </div>
            </Show>
          </div>
        </Show>

        {/* ── DISTRIBUTE TAB ───────────────────────────────────────────────── */}
        <Show when={tab() === 'distribute' && !noPost()}>
          {/* Social snippets */}
          <div>
            <div class="flex items-center justify-between mb-2">
              <span class="text-xs font-semibold text-gray-600 dark:text-gray-300">Social Snippets</span>
              {btn('Generate All', snippetsLoading(), runSnippets)}
            </div>
            <Show when={Object.keys(snippets()).length > 0}>
              <div class="space-y-2">
                <For each={Object.entries(snippets())}>
                  {([platform, text]) => (
                    <div class="p-2 border border-gray-200 dark:border-gray-700 rounded-lg">
                      <div class="flex justify-between items-center mb-1">
                        <span class="text-[10px] font-semibold uppercase tracking-wide text-gray-500">{platform}</span>
                        <button onClick={() => navigator.clipboard.writeText(text)} class="text-[10px] text-sky-600 hover:text-sky-800">Copy</button>
                      </div>
                      <p class="text-xs text-gray-700 dark:text-gray-300 line-clamp-3">{text}</p>
                    </div>
                  )}
                </For>
              </div>
            </Show>
          </div>

          {/* Platform adapter */}
          <div class="border-t border-gray-100 dark:border-gray-700 pt-3">
            <span class="text-xs font-semibold text-gray-600 dark:text-gray-300 block mb-2">Platform Adapter</span>
            <div class="flex gap-2">
              <select
                value={adaptPlatform()}
                onChange={(e) => setAdaptPlatform(e.currentTarget.value)}
                class="flex-1 text-xs border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 rounded-lg px-2 py-1.5 text-gray-700 dark:text-gray-300"
              >
                <option value="devto">Dev.to</option>
                <option value="hashnode">Hashnode</option>
                <option value="medium">Medium</option>
                <option value="substack">Substack</option>
                <option value="linkedin">LinkedIn</option>
              </select>
              {btn('Adapt', adaptLoading(), runAdapt)}
            </div>
          </div>

          {/* Tone rewrite */}
          <div class="border-t border-gray-100 dark:border-gray-700 pt-3">
            <span class="text-xs font-semibold text-gray-600 dark:text-gray-300 block mb-2">Tone Rewrite</span>
            <div class="flex gap-2">
              <select
                value={tonePlatform()}
                onChange={(e) => setTonePlatform(e.currentTarget.value)}
                class="flex-1 text-xs border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 rounded-lg px-2 py-1.5 text-gray-700 dark:text-gray-300"
              >
                <option value="technical">Technical</option>
                <option value="balanced">Balanced</option>
                <option value="conversational">Conversational</option>
              </select>
              {btn('Rewrite', toneLoading(), runTone)}
            </div>
          </div>

          {/* Variants list */}
          <Show when={variants().length > 0}>
            <div class="border-t border-gray-100 dark:border-gray-700 pt-3">
              <span class="text-xs font-semibold text-gray-600 dark:text-gray-300 block mb-2">Saved Variants ({variants().length})</span>
              <div class="space-y-1">
                <For each={variants()}>
                  {(v) => (
                    <div class="flex items-center justify-between p-2 bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg">
                      <div class="min-w-0">
                        <p class="text-xs font-medium text-gray-700 dark:text-gray-300 truncate">{v.label}</p>
                        <p class="text-[10px] text-gray-400">{new Date(v.created_at).toLocaleDateString()}</p>
                      </div>
                      <div class="flex gap-1 shrink-0">
                        <button onClick={() => navigator.clipboard.writeText(v.content)} class="text-[10px] text-sky-600 hover:text-sky-800">Copy</button>
                        <button onClick={() => deleteVariant(v.id)} class="text-[10px] text-red-400 hover:text-red-600">Del</button>
                      </div>
                    </div>
                  )}
                </For>
              </div>
            </div>
          </Show>
        </Show>
      </div>
    </div>
  );
};

export default LlmAssistantPanel;
```

- [ ] **Step 2: TypeScript check**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | grep "LlmAssistant\|error" | head -10
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add ui/src/pages/blog/LlmAssistantPanel.tsx
git commit -m "feat(blog): LlmAssistantPanel — Lint/Writing/SEO/Distribute tabs with all Phase C-1 features"
```

---

## Task 6: Wire LlmAssistantPanel into Blog.tsx

**Files:**
- Modify: `ui/src/pages/Blog.tsx`

- [ ] **Step 1: Import the panel**

At the top of `ui/src/pages/Blog.tsx`, after the existing blog imports, add:
```tsx
import LlmAssistantPanel from './blog/LlmAssistantPanel';
```

- [ ] **Step 2: Add panel open signal**

In the component signals section (after `const [tab, setTab]`), add:
```tsx
const [showLlmPanel, setShowLlmPanel] = createSignal(false);
```

- [ ] **Step 3: Add "✨ AI" button to the editor toolbar**

In the editor Match block, find the view-mode toggle bar (the div with Editor/Split/Preview buttons). After the last toggle button and before the `renderingPreview()` span, add:
```tsx
<div class="ml-auto">
  <button
    onClick={() => setShowLlmPanel(v => !v)}
    class={`px-3 py-1 rounded text-xs font-medium transition-colors ${
      showLlmPanel()
        ? 'bg-sky-500 text-white'
        : 'bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300 hover:bg-gray-200'
    }`}
    title="AI Writing Assistant"
  >
    ✨ AI
  </button>
</div>
```

- [ ] **Step 4: Wrap the editor content and panel in a flex row**

Find the inner `<div class="flex flex-1 overflow-hidden min-h-0">` in the editor section. This div contains the editor textarea area and the side panel (status/actions). Replace:
```tsx
<div class="flex flex-1 overflow-hidden min-h-0">
```
with:
```tsx
<div class="flex flex-1 overflow-hidden min-h-0" style={{ position: 'relative' }}>
```

Then before the closing `</div></div></Match>` of the editor, add:
```tsx
<Show when={showLlmPanel()}>
  <LlmAssistantPanel
    postId={editingId()}
    onClose={() => setShowLlmPanel(false)}
  />
</Show>
```

- [ ] **Step 5: TypeScript check**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | grep "error" | head -10
```
Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add ui/src/pages/Blog.tsx
git commit -m "feat(blog): wire LlmAssistantPanel into editor with ✨ AI toggle button"
```

---

## Task 7: E2E Smoke Test

- [ ] **Step 1: Run all Rust tests**

```bash
cd /home/dk/Documents/git/minion && cargo test --workspace 2>&1 | grep "FAILED\|test result" | grep -v "ok. 0" | tail -15
```
Expected: all pass including `test_migration_018_blog_llm_schema` and 5 blog_lint tests.

- [ ] **Step 2: Run clippy**

```bash
cargo clippy --workspace -- -D warnings 2>&1 | grep "^error" | head -10
```
Expected: no errors.

- [ ] **Step 3: Frontend typecheck + lint**

```bash
cd ui && pnpm typecheck 2>&1 | grep "error" | head -5
pnpm lint 2>&1 | grep "error" | head -5
```
Expected: no errors.

- [ ] **Step 4: Final commit**

```bash
cd /home/dk/Documents/git/minion && git add -A && git commit -m "feat(blog): Phase C-1 MVP — LLM assistant: lint, titles, hook, conclusion, grammar, meta desc, tags, snippets, adapter, tone"
```

---

## Self-Review Checklist

| Spec requirement | Task |
|---|---|
| Migration 018 (blog_post_variants + social_snippets_json) | Task 1 |
| 9 rule-based lint checks, anchor-based positioning | Task 2 |
| Graceful degradation when no LLM endpoint | Task 3 (all commands return Ok(None)) |
| Token estimate awareness | Task 3 (excerpt truncation at ~3000 tokens) |
| Title generator (5 styles) | Task 3 `blog_llm_titles` |
| Hook rewriter (3 variants) | Task 3 `blog_llm_hook` |
| Conclusion + CTA | Task 3 `blog_llm_conclusion` |
| Grammar + language quality | Task 3 `blog_llm_grammar` |
| Meta description generator | Task 3 `blog_llm_meta_description` |
| Tag suggester from existing library | Task 3 `blog_llm_tags` |
| Social snippets (Twitter/LinkedIn/Substack/Generic) | Task 3 `blog_llm_snippets` |
| Platform adapter (5 platforms) | Task 3 `blog_llm_adapt` |
| Tone rewrite (3 tones) | Task 3 `blog_llm_tone` |
| Variant CRUD | Task 3 `blog_get_variants` / `blog_delete_variant` |
| LlmAssistantPanel with 4 tabs | Task 5 |
| ✨ AI toggle in editor | Task 6 |
| Never mutates source content | Task 3 (all variants stored separately) |
| No diff view for platform adapter | Task 5 (side-by-side read-only in variants list) |

**Phase C-2 (future plan):** TOC generator, content gap detector, section expander, post compressor, code block explainer, series planner, newsletter version, Twitter thread, internal link suggester, keyword density, FAQ extractor, search intent classifier.
