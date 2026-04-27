# Blog Engine v3 — Design Spec
_Date: 2026-04-18_

## Overview

Three sequential phases delivered as independent milestones:

- **Phase A** — Rich preview: side-by-side split editor, Markdown→HTML via `pulldown-cmark` (GFM extensions enabled), Mermaid.js lazy-loaded, inline SVG passthrough, DOMPurify with SVG allowlist, debounced auto-save
- **Phase B** — Platform expansion: WordPress.com, Beehiiv, Blogger.com (full API); Medium, Substack, devstack.io (HTML/MD export); diffstack.co + hellodk.io (GitHub Pages PR — hellodk.io uses `hellodk/staging.hellodk.io`, Jasper2 theme); LinkedIn, Twitter/X (copy + open composer)
- **Phase C** — LLM assistant: format fixer, platform adapter (produces copy, never mutates original), per-platform social snippets with correct character limits

Each phase is spec'd independently. Each gets its own implementation plan.

---

## Phase A — Rich Preview

### Architecture

**Rendering pipeline (hybrid):**
1. User types in left pane → debounced 400ms
2. Tauri command `blog_render_preview(markdown: String) -> String` (new) calls `pulldown-cmark` with `TABLES | FOOTNOTES | STRIKETHROUGH | TASKLISTS` extension flags → returns HTML string
3. Frontend receives HTML → passes through `DOMPurify.sanitize()` with SVG allowlist → injects into preview pane via `innerHTML`
4. After injection: if HTML contains `<pre class="language-mermaid">` blocks → dynamic `import('mermaid')` → `mermaid.default.run()` on those elements
5. Inline `<svg>` elements pass through DOMPurify allowlist unchanged

**Asset image protocol:**
- Stored assets at `{app_data}/blog/assets/{sha}.ext` are served via Tauri's existing asset protocol
- On render, the Rust command rewrites local image paths (`![alt](./img.png)` referencing assets) to `asset://{sha}.ext` URLs that Tauri's custom protocol handler serves
- Remote `http/https` image URLs are left unchanged

### New files

| File | Purpose |
|---|---|
| `src-tauri/src/blog_preview.rs` | `blog_render_preview` Tauri command, image path rewriting |
| `ui/src/pages/blog/EditorTab.tsx` | Side-by-side split editor — textarea left, preview right |
| `ui/src/pages/blog/PreviewPane.tsx` | Renders HTML, triggers Mermaid, handles SVG |

### Modified files

| File | Change |
|---|---|
| `src-tauri/src/lib.rs` | Declare `blog_preview` module, register command |
| `src-tauri/Cargo.toml` | Enable GFM flags on existing `pulldown-cmark` dep (already present) |
| `ui/src/pages/Blog.tsx` | Add Editor tab, move Import from tab → inline button next to "New Post" in the header toolbar |
| `ui/package.json` | Add `mermaid`, `highlight.js`, `dompurify`, `@types/dompurify` |

### Editor behaviour

- **Layout**: 50/50 split, resizable drag handle; toggle buttons for Editor-only / Split / Preview-only
- **Auto-save**: debounced 2s after last keystroke → saves to `draft_content` column (new column, migration 016)
- **Explicit save**: "Save" button commits `draft_content` → `content`, clears unsaved indicator
- **Unsaved indicator**: "● Unsaved changes" in tab when `draft_content` differs from `content`
- **Word count + reading time**: live update in status bar below editor (existing fields in `blog_posts`)

### DOMPurify config

```js
DOMPurify.sanitize(html, {
  USE_PROFILES: { html: true },
  ADD_TAGS: ['svg', 'path', 'circle', 'rect', 'line', 'polyline', 'polygon',
              'text', 'g', 'defs', 'clipPath', 'use', 'image', 'foreignObject'],
  ADD_ATTR: ['viewBox', 'xmlns', 'd', 'fill', 'stroke', 'stroke-width',
             'transform', 'cx', 'cy', 'r', 'x', 'y', 'width', 'height',
             'x1', 'y1', 'x2', 'y2', 'points', 'clip-path'],
  FORBID_TAGS: ['script', 'style', 'iframe', 'object', 'embed'],
})
```

### Mermaid lazy loading

```ts
async function renderMermaid(container: HTMLElement) {
  const blocks = container.querySelectorAll('pre.language-mermaid, code.language-mermaid');
  if (blocks.length === 0) return;
  const mermaid = await import('mermaid');
  mermaid.default.initialize({ startOnLoad: false, theme: 'neutral' });
  blocks.forEach((el, i) => {
    el.id = el.id || `mermaid-${i}`;
    mermaid.default.run({ nodes: [el as HTMLElement] });
  });
}
```

### Migration 016 additions

```sql
ALTER TABLE blog_posts ADD COLUMN draft_content TEXT;
-- draft_content: auto-saved working copy; NULL means no unsaved changes
```

---

## Phase B — Platform Expansion

### New platform support matrix

| Platform | Type | Auth | Notes |
|---|---|---|---|
| **WordPress.com** | Full API | OAuth2 (`public-api.wordpress.com/oauth2/token`) | Separate from self-hosted WP; endpoint: `public-api.wordpress.com/rest/v1.1/sites/{site}/posts/new` |
| **Beehiiv** | Full API | API key | `api.beehiiv.com/v2/publications/{pub_id}/posts`; requires Scale plan |
| **Blogger.com** | Full API | Google OAuth2 | `blogger.googleapis.com/v3/blogs/{blogId}/posts`; requires `blogger` scope |
| **Medium** | Export only | — | Export as `{slug}-medium.html`; link to `medium.com/p/import` |
| **Substack** | Export only | — | Export as `{slug}-substack.html`; link to `substack.com/publish/import` |
| **devstack.io** | Export only | — | Export `.md` + `.html`; show target directory hint |
| **diffstack.co** | GitHub Pages | GitHub token (optional) | Export `.md` → if token set, open GitHub PR; else download |
| **LinkedIn** | Copy + open | — | Generate 3000-char post body + blog URL; open `linkedin.com/sharing/share-offsite` |
| **Twitter/X** | Copy + open | — | Generate 280-char teaser OR numbered thread; open `x.com/intent/tweet` |
| **hellodk.io** | GitHub Pages | GitHub token (optional) | Repo: `hellodk/staging.hellodk.io`; Jasper2 theme uses `_posts/` Jekyll layout; export `.md` with Jasper2-compatible frontmatter → PR or download |

### Export formats

**Medium HTML export** — standard HTML with:
- `<title>` tag set to post title
- `<meta name="description">` set to excerpt
- All images as absolute URLs (asset images inlined as base64 if <200KB, else linked)
- Code blocks as `<pre><code class="language-X">` (Medium import preserves these)
- No `<style>` tags (Medium strips them)

**Substack HTML export** — same as Medium but:
- `<h1>` for title, `<h2>` for subtitle/excerpt
- Mermaid diagrams rendered to SVG server-side via a headless Mermaid call OR replaced with `[Diagram: <caption>]` placeholder with a note

**devstack.io export** — two files:
- `{slug}.md` — raw markdown with YAML frontmatter (title, date, tags, canonical_url)
- `{slug}.html` — full rendered HTML with inline styles for standalone viewing

**GitHub Pages export (diffstack.co + hellodk.io):**
- `{slug}.md` — Jekyll-compatible frontmatter (`layout`, `title`, `date`, `permalink`, `tags`)
- For **hellodk.io**: targets `hellodk/staging.hellodk.io`, branch `main`, path `_posts/`; Jasper2 frontmatter includes `image` field for cover photo
- For **diffstack.co**: targets `hellodk/diffstack.co`, branch `main`, path `_posts/`
- If GitHub token configured: opens GitHub PR via API (`POST /repos/{owner}/{repo}/pulls`)
- If no token: downloads `.md` file + shows exact repo path to drop it in

**LinkedIn copy:**
```
{emoji} Just published: "{title}"

{first 2-3 paragraphs, stripped of markdown, max 2500 chars}

Key takeaways:
• {bullet 1}
• {bullet 2}
• {bullet 3}

→ Read the full post: {canonical_url or blog_url}

#{tag1} #{tag2} #{tag3}
```
Character counter shown live. "Copy" button + "Open LinkedIn" button opens `https://www.linkedin.com/sharing/share-offsite/?url={encoded_url}`.

**Twitter/X copy:**
- Single tweet: 270-char excerpt + URL (leaves 10 chars for numbering)
- Thread mode: splits post at paragraph boundaries into tweets ≤270 chars each, appends `[N/total]`

### New files

| File | Purpose |
|---|---|
| `src-tauri/src/blog_export.rs` | All export logic: Medium, Substack, devstack, diffstack, LinkedIn/X copy generation |
| `src-tauri/src/blog_platforms_extra.rs` | WordPress.com, Beehiiv, Blogger API publishers |

### Modified files

| File | Change |
|---|---|
| `src-tauri/src/blog_publish.rs` | Register new platform types, route to new publishers |
| `src-tauri/src/lib.rs` | Declare new modules, register new commands |
| `ui/src/pages/blog/PlatformsTab.tsx` | Add new platform cards with correct auth flows |
| `ui/src/pages/blog/PublishTab.tsx` | Add export/copy actions for manual platforms |
| `crates/minion-db/src/migrations.rs` | Migration 016: no schema changes for platforms (existing `blog_platform_accounts` handles all new ones) |

### New Tauri commands

| Command | Description |
|---|---|
| `blog_export_medium(post_id)` | Returns base64 HTML file content |
| `blog_export_substack(post_id)` | Returns base64 HTML file content |
| `blog_export_devstack(post_id)` | Returns `{ md: string, html: string }` |
| `blog_export_github_pages(post_id, repo?)` | Returns `.md` content + optional GitHub PR URL |
| `blog_copy_linkedin(post_id)` | Returns formatted LinkedIn post string |
| `blog_copy_twitter(post_id, mode: 'single'|'thread')` | Returns tweet string or `string[]` thread |
| `blog_publish_wordpress_com(post_id, account_id)` | Publish to WordPress.com |
| `blog_publish_beehiiv(post_id, account_id)` | Publish to Beehiiv |
| `blog_publish_blogger(post_id, account_id)` | Publish to Blogger |

### WordPress.com OAuth2 flow

WordPress.com uses a redirect-based OAuth2 flow (same pattern as Google/Outlook calendar):
1. `blog_wpcom_open_auth(account_id)` → opens browser to `wordpress.com/oauth2/authorize`
2. Deep-link callback → `blog_wpcom_save_token(code)` → exchanges code for token
3. Token stored encrypted in `blog_platform_accounts.api_key_encrypted`

### Blogger OAuth2 flow

Reuses the Google OAuth2 infrastructure already present in `calendar_integration.rs`:
1. `blog_blogger_open_auth(account_id)` → opens Google OAuth consent with `blogger` scope
2. Callback saves token same as Google Calendar pattern

---

## Phase C — LLM Assistant
_Revised 2026-04-27 — corrected migration reference, fixed line-number issue, separated rule-based from LLM checks, added full feature set._

### Principles

1. **Never mutate source content** — all LLM outputs are ephemeral previews or stored separately in `blog_post_variants`
2. **Graceful degradation** — if no LLM endpoint configured, show "Configure LLM in Settings"; no errors surfaced to user
3. **Rule-based checks are NOT LLM calls** — structural checks (broken markdown, heading gaps, word count) run as deterministic Rust code; LLM is called only for genuine language-quality tasks
4. **Anchor-based positioning, not line numbers** — fix locations use heading text anchors or paragraph hashes, never raw line numbers (LLMs cannot reliably produce these)
5. **Token estimate before call** — any call that sends >500 tokens shows an estimate and requires confirmation; prevents surprise cost/latency
6. **Reuse existing infrastructure** — calls `llm_endpoints` table, uses `reqwest` same as `sysmon_analysis.rs`

---

### Feature Catalogue

#### Group 1 — Rule-Based Checks (Rust only, no LLM)

Run instantly as the user types (debounced 1s). Return structured issues with paragraph-level anchors.

| Check | What it flags |
|---|---|
| Broken markdown | Unclosed code fences, malformed links `[text](` with no closing `)` |
| Missing alt text | `![]()` images with empty alt text |
| Heading hierarchy | H1 → H3 with no H2; multiple H1s |
| Duplicate headings | Two headings with identical text |
| Thin sections | Heading with <50 words below it before the next heading |
| Missing excerpt | `excerpt` field empty at publish time |
| Orphan images | Images in asset vault not referenced in post |
| Long paragraphs | Any paragraph >250 words (readability signal) |
| Reading level | Flesch-Kincaid grade estimate (computed from sentence length + syllable count) |

#### Group 2 — LLM: Writing Quality

Called on-demand, one feature at a time. Each shows token estimate before firing.

| Feature | What it does |
|---|---|
| **Title generator** ⭐ | Generates 5 alternative titles per post: one SEO-optimised, one curiosity-gap, one direct/declarative, one question-form, one listicle. Shows character count and click-prediction rationale for each. |
| **Hook rewriter** | Rewrites the opening paragraph for maximum retention. Returns 3 variants (direct, story-led, question-led). User picks one. |
| **Conclusion + CTA generator** | Suggests a stronger ending with a platform-appropriate call to action. |
| **Sentence simplifier** | Flags sentences above Grade 12 Flesch-Kincaid and suggests plain-English rewrites. |
| **Tone adjuster** | Shifts register on a 3-point scale: Technical → Balanced → Conversational. Rewrites the full post in the target tone. |
| **Grammar + language quality** | Passive voice detection, weak verbs ("is", "was", "get"), filler words ("very", "just", "really", "thing"), redundant phrases. Returns a list; user applies each fix individually. |

#### Group 3 — LLM: SEO & Discoverability

| Feature | What it does |
|---|---|
| **Meta description generator** | Generates a 150–160 char SEO-optimised description (different from social snippet). Stores in `blog_posts.excerpt`. |
| **Keyword density analyser** | Given a target keyword entered by user, shows current density and natural insertion points. |
| **Tag/category suggester** | Reads post content, suggests tags from existing tag library + up to 3 new ones. |
| **Search intent classifier** | Labels the post: Informational / Transactional / Navigational. Flags mismatches (e.g., post answers "how to" questions but title implies a product page). |
| **FAQ extractor** | Identifies the questions the post implicitly answers; formats them as a `## FAQ` section suitable for Google featured snippets. |

#### Group 4 — LLM: Content Structure

| Feature | What it does |
|---|---|
| **Content gap detector** | Compares post to a short topic description; identifies standard subtopics typically covered that are missing. |
| **Section expander** | User selects a thin section; LLM adds 3 supporting points or examples. Output shown as a suggestion, not applied automatically. |
| **Post compressor** | Condenses the post to a target word count (user specifies). Produces a variant, never overwrites. |
| **Table of contents generator** | Extracts all headings and generates anchor-linked TOC markdown. Inserted at cursor position. No LLM needed — deterministic. |
| **Series planner** | Suggests how to split a long post (>2000 words) into a 2–4 part series with logical break points. |
| **Code block explainer** | For each fenced code block, generates a plain-English explanation paragraph. Applied as suggestions above each block. |

#### Group 5 — LLM: Distribution & Social

| Feature | What it does |
|---|---|
| **Social snippets (per-platform)** | Twitter/X (270 chars), LinkedIn (3000 chars), Substack teaser (500 chars), Generic (280 chars). One-click copy per platform. |
| **Platform adapter** | Rewrites post for a target platform's conventions (tone, structure, formatting). Stored as a named variant. |
| **Newsletter version** | Email-ready version: greeting, scannable summary bullets, unsubscribe-aware CTA. Different from Substack adaptation. |
| **Twitter/X thread generator** | Splits post into numbered tweets ≤270 chars at paragraph boundaries. |
| **Internal link suggester** | Compares current post against all other posts in the DB; suggests where to add cross-links. No LLM needed for basic version — uses TF-IDF similarity from existing minion-rag. |

---

### Architecture

#### Rule-based checks (Group 1)

Implemented in `src-tauri/src/blog_lint.rs`. No LLM. Returns:

```rust
pub struct LintIssue {
    pub id: String,              // deterministic hash of (rule, anchor)
    pub rule: String,            // "missing_alt_text" | "heading_gap" | etc.
    pub anchor: String,          // nearest heading text above the issue
    pub description: String,     // human-readable problem
    pub suggestion: String,      // human-readable fix
    pub auto_fixable: bool,      // can be applied without LLM
}
```

Apply uses the anchor to locate the correct paragraph, never a line number.

#### LLM features (Groups 2–5)

All in `src-tauri/src/blog_llm.rs`. Each call:
1. Estimates token count from post length
2. If >2000 tokens, shows estimate in UI before proceeding
3. Returns result as a `BlogLlmResult` (text + metadata)
4. Stores outputs in `blog_post_variants` — never overwrites `content`

#### Platform adapter — no diff view

The original spec proposed a diff view. Dropped: markdown diffs of LLM-rewritten content are noisy and unhelpful (everything changes). Instead: show original and adapted versions side by side as two read-only panes. User copies the adapted version.

---

### New files

| File | Purpose |
|---|---|
| `src-tauri/src/blog_lint.rs` | Rule-based lint checks (no LLM) |
| `src-tauri/src/blog_llm.rs` | All LLM features: writing quality, SEO, structure, distribution |
| `ui/src/pages/blog/LlmAssistantPanel.tsx` | Slide-out panel: Lint tab + AI tab with grouped features |

### Modified files

| File | Change |
|---|---|
| `crates/minion-db/src/migrations.rs` | Migration 018 — blog_post_variants + social_snippets_json column |
| `src-tauri/src/lib.rs` | Register new commands |
| `ui/src/pages/Blog.tsx` | Wire LlmAssistantPanel into editor |

### New Tauri commands

| Command | Description |
|---|---|
| `blog_lint(post_id)` | Rule-based checks, returns `Vec<LintIssue>` instantly |
| `blog_apply_lint_fix(post_id, issue_id)` | Apply an auto-fixable lint issue to `draft_content` |
| `blog_llm_titles(post_id)` | Generate 5 title alternatives |
| `blog_llm_hook(post_id)` | Rewrite opening paragraph (3 variants) |
| `blog_llm_conclusion(post_id)` | Suggest conclusion + CTA |
| `blog_llm_simplify(post_id)` | Flag complex sentences |
| `blog_llm_tone(post_id, target)` | Rewrite for tone: technical/balanced/conversational |
| `blog_llm_grammar(post_id)` | Passive voice, weak verbs, filler words |
| `blog_llm_meta_description(post_id)` | Generate SEO excerpt |
| `blog_llm_keywords(post_id, keyword)` | Keyword density + insertion points |
| `blog_llm_tags(post_id)` | Suggest tags |
| `blog_llm_faq(post_id)` | Extract FAQ section |
| `blog_llm_gaps(post_id, topic)` | Content gap analysis |
| `blog_llm_expand_section(post_id, anchor)` | Expand a thin section |
| `blog_llm_compress(post_id, target_words)` | Compress to target word count |
| `blog_llm_code_explain(post_id)` | Add explanations above code blocks |
| `blog_llm_snippets(post_id)` | Generate all social snippets |
| `blog_llm_adapt(post_id, platform)` | Platform-adapted variant |
| `blog_llm_newsletter(post_id)` | Email-ready version |
| `blog_llm_thread(post_id)` | Twitter/X thread split |
| `blog_get_variants(post_id)` | List all stored variants |
| `blog_delete_variant(variant_id)` | Delete a stored variant |
| `blog_toc(post_id)` | Generate TOC from headings (deterministic, no LLM) |
| `blog_internal_links(post_id)` | Suggest internal links via minion-rag similarity |

### Migration 018 (Phase C)

```sql
-- Social snippets per post (cached LLM output)
ALTER TABLE blog_posts ADD COLUMN social_snippets_json TEXT;
-- Format: {"twitter":"...","linkedin":"...","substack":"...","newsletter":"..."}

-- LLM-generated variants (platform adaptations, compressions, tone rewrites)
-- Stores multiple versions per post+type without overwriting originals
CREATE TABLE IF NOT EXISTS blog_post_variants (
    id           TEXT PRIMARY KEY,
    post_id      TEXT NOT NULL REFERENCES blog_posts(id) ON DELETE CASCADE,
    variant_type TEXT NOT NULL,   -- 'platform_devto' | 'tone_casual' | 'compressed_1500' | etc.
    label        TEXT NOT NULL,   -- human-readable: "Dev.to adaptation" | "Casual tone" | etc.
    content      TEXT NOT NULL,   -- adapted markdown
    created_at   TEXT DEFAULT CURRENT_TIMESTAMP
    -- No UNIQUE constraint: multiple variants of the same type are kept as history
);
CREATE INDEX IF NOT EXISTS idx_blog_variants_post ON blog_post_variants(post_id);
```

---

## Migration Summary

| Migration | Phase | Contents | Status |
|---|---|---|---|
| 016 | A | `draft_content TEXT` on `blog_posts` | ✅ Applied |
| 017 | — | Fitness gfit columns (unrelated) | ✅ Applied |
| 018 | C | `social_snippets_json` on `blog_posts` + `blog_post_variants` table | ⏳ Pending |

---

## Out of Scope

- Direct Twitter/X API posting (paid API)
- LinkedIn Articles (requires business partnership)
- Medium API (closed to new integrations)
- Substack API (none exists)
- Analytics / view tracking per platform
- Scheduled posting for new platforms (existing scheduler handles it for API platforms)
- devstack.io GitHub PR (no repo URL — file export only)
- Paid LinkedIn Articles API (URL share + snippet covers all personal use cases)
