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

### Principles

1. **Never mutate source content** — all LLM outputs are ephemeral previews or stored separately
2. **Graceful degradation** — if no LLM endpoint configured, show "Configure LLM in Settings" in assistant panel, no errors
3. **Reuse existing infrastructure** — calls `llm_endpoints` table, uses `reqwest` same as `sysmon_analysis.rs`

### Three modes

#### 1. Format Fixer
Scans post for structural issues and returns annotated suggestions:
- Broken Markdown syntax (unclosed code fences, malformed links)
- Missing alt text on images
- Heading hierarchy violations (H1 → H3 with no H2)
- Paragraphs >300 words (readability)
- Missing meta description / excerpt
- Duplicate headings

Returns a structured list of `{ line: number, issue: string, suggestion: string }` items. User can apply each individually with "Apply" button (patches the editor content) or "Apply All".

Complements (does not replace) existing `blog_analyze_seo` — SEO score focuses on keywords; format fixer focuses on structure.

#### 2. Platform Adapter
Given a target platform and the post content, produces a **platform-optimised variant**:

| Platform | Adaptations |
|---|---|
| Dev.to | Add `:::note` callout blocks, shorter intro, canonical URL note |
| Hashnode | Add cover image suggestion, subtitle, series tag |
| Medium | Remove code-heavy sections, add narrative bridges |
| Substack | Add personal opener, newsletter CTA at end, casual tone |
| LinkedIn | Compress to 3000 chars, add bullet takeaways, 3 hashtags |

Output shown in a diff view (original left, adapted right). User can copy adapted version or save as a named variant in new `blog_post_variants` table (migration 016).

#### 3. Social Snippets (per-platform)

Extends existing `social_snippet()` function. Generates per-platform with correct limits:

| Platform | Limit | Format |
|---|---|---|
| Twitter/X | 270 chars | Hook + link |
| LinkedIn | 3000 chars | Hook + takeaways + link + hashtags |
| Substack teaser | 500 chars | Curiosity-gap opener |
| Dev.to | No limit | First paragraph + tags |
| Generic | 280 chars | Existing `social_snippet()` output |

All shown in the assistant panel with one-click copy per platform. Stored in `blog_posts.social_snippets_json` (new column, migration 016) as `{ "twitter": "...", "linkedin": "...", ... }`.

### New files

| File | Purpose |
|---|---|
| `src-tauri/src/blog_llm.rs` | `blog_fix_format`, `blog_adapt_for_platform`, `blog_generate_snippets` commands |
| `ui/src/pages/blog/LlmAssistantPanel.tsx` | Floating panel: Format Fixer tab + Platform Adapter tab + Snippets tab |

### New Tauri commands

| Command | Description |
|---|---|
| `blog_fix_format(post_id)` | Returns `Vec<FormatIssue>` |
| `blog_apply_fix(post_id, fix)` | Applies single fix to `draft_content` |
| `blog_adapt_for_platform(post_id, platform)` | Returns adapted markdown string |
| `blog_generate_snippets(post_id)` | Returns `HashMap<platform, snippet>`, stores in DB |
| `blog_get_snippets(post_id)` | Returns stored snippets |

### Migration 016 additions (Phase C)

```sql
ALTER TABLE blog_posts ADD COLUMN social_snippets_json TEXT;
-- Stored as JSON: {"twitter":"...","linkedin":"...","substack":"..."}

CREATE TABLE IF NOT EXISTS blog_post_variants (
    id          TEXT PRIMARY KEY,
    post_id     TEXT NOT NULL REFERENCES blog_posts(id) ON DELETE CASCADE,
    platform    TEXT NOT NULL,   -- 'devto' | 'medium' | 'linkedin' | etc.
    content     TEXT NOT NULL,   -- adapted markdown
    created_at  TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(post_id, platform)
);
```

---

## Migration 016 — Full Summary

```sql
-- Phase A
ALTER TABLE blog_posts ADD COLUMN draft_content TEXT;

-- Phase C
ALTER TABLE blog_posts ADD COLUMN social_snippets_json TEXT;

CREATE TABLE IF NOT EXISTS blog_post_variants (
    id          TEXT PRIMARY KEY,
    post_id     TEXT NOT NULL REFERENCES blog_posts(id) ON DELETE CASCADE,
    platform    TEXT NOT NULL,
    content     TEXT NOT NULL,
    created_at  TEXT DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(post_id, platform)
);
```

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
