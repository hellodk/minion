# Blog Module v2 — Design Plan

**Status:** Planned (not yet implemented). Will be picked up after Health Vault Week 5.

## Vision

Turn MINION's Blog module into a **complete authoring + multi-platform publishing pipeline**:
1. Bulk import existing markdown/HTML files with smart tagging
2. Manage embedded images centrally
3. Publish to platforms that have APIs; export to platforms that don't
4. Track where each post lives across platforms

---

## Open Decisions (Defaults)

These were proposed during planning. If not overridden, build with these defaults:

| # | Decision | Default |
|---|----------|---------|
| 1 | Folder-to-tag inference | Auto-suggest from parent folder, show editable tag list before save |
| 2 | Initial publishing platforms | LinkedIn + Medium + Substack (manual) PLUS WordPress + Hashnode + Dev.to (auto) |
| 3 | Manual export UX | Modal with platform-tailored preview, "Copy" button, and "Open editor" button |
| 4 | Image storage | Copy with SHA-256 dedup (same image used in N posts → stored once) |
| 5 | Concept primer depth | Living roadmap (concepts + concrete MINION integration milestones) |

---

## 1. Bulk import + tagging

### File sources
- Single file picker (`.md`, `.markdown`, `.txt`, `.html`)
- Folder picker (recursive scan)
- Drag-and-drop into UI

### Frontmatter parsing
If the file starts with YAML frontmatter:
```yaml
---
title: NFS Storage in Kubernetes
date: 2024-08-15
tags: [k8s, storage, nfs]
status: published
canonical_url: https://example.com/posts/nfs-k8s
---
```
Extract: `title`, `date` → `created_at`, `tags`, `status`, `canonical_url`.

Fallback if no frontmatter:
- Title = first `#` heading or filename without extension
- Date = file mtime
- Tags = inferred from folder hierarchy

### Tag inference
- Parent folder name → suggested tag (e.g., `blogs/kubernetes/nfs.md` → suggests `kubernetes`)
- Show tag editor pre-import so user can normalize: `kubernetes` → `k8s`
- Maintain a `blog_tags` table for normalized canonical tag list

### Schema additions (migration 010)
```sql
CREATE TABLE blog_tags (
    id TEXT PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    color TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE blog_post_tags (
    post_id TEXT NOT NULL REFERENCES blog_posts(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES blog_tags(id) ON DELETE CASCADE,
    PRIMARY KEY (post_id, tag_id)
);

-- Image asset registry (deduplicated)
CREATE TABLE blog_assets (
    id TEXT PRIMARY KEY,
    sha256 TEXT UNIQUE NOT NULL,
    stored_path TEXT NOT NULL,        -- ~/.minion/blog/assets/{sha}.{ext}
    original_filename TEXT,
    mime_type TEXT,
    width INTEGER,
    height INTEGER,
    size_bytes INTEGER,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE blog_post_assets (
    post_id TEXT NOT NULL REFERENCES blog_posts(id) ON DELETE CASCADE,
    asset_id TEXT NOT NULL REFERENCES blog_assets(id),
    referenced_as TEXT,               -- the original src= path in the markdown
    PRIMARY KEY (post_id, asset_id, referenced_as)
);

-- Per-platform publishing state (tracks where each post lives)
CREATE TABLE blog_platform_publications (
    id TEXT PRIMARY KEY,
    post_id TEXT NOT NULL REFERENCES blog_posts(id) ON DELETE CASCADE,
    platform TEXT NOT NULL,           -- wordpress, medium, hashnode, devto, ghost,
                                      -- substack, linkedin, twitter, custom
    status TEXT,                      -- draft, scheduled, published, exported, failed
    remote_id TEXT,                   -- platform's article ID
    remote_url TEXT,
    canonical_url TEXT,               -- which URL is the canonical
    published_at TEXT,
    last_synced_at TEXT,
    error TEXT,
    metadata TEXT                     -- platform-specific JSON
);

-- Platform credentials (encrypted)
CREATE TABLE blog_platform_accounts (
    id TEXT PRIMARY KEY,
    platform TEXT NOT NULL,
    account_label TEXT,               -- "Personal WordPress", "Work Hashnode"
    base_url TEXT,                    -- self-hosted WP / Ghost
    api_key_encrypted TEXT,
    publication_id TEXT,              -- Hashnode publication, Substack pub, etc.
    default_tags TEXT,                -- JSON array
    enabled INTEGER DEFAULT 1,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
```

### Tauri commands
```
blog_import_files(paths: Vec<String>) → Vec<ImportPreview>
blog_import_folder(path: String) → Vec<ImportPreview>
blog_confirm_import(previews: Vec<ImportPreview>) → ImportResult
blog_list_tags() → Vec<Tag>
blog_create_tag(name, color?) → Tag
blog_set_post_tags(post_id, tag_ids: Vec<String>) → ()
```

`ImportPreview` includes parsed title, suggested tags, image references found, frontmatter detected, and the proposed final state. User can edit before confirming.

---

## 2. Image asset management

### Pipeline
On import:
1. Parse content for `![alt](path)` (markdown) or `<img src="...">` (HTML)
2. For each referenced image:
   - If `path` is a URL → download to temp
   - If `path` is local → resolve relative to source file
3. Compute SHA-256 of image bytes
4. If hash exists in `blog_assets` → reuse, otherwise copy to `~/.minion/blog/assets/{sha}.{ext}`
5. Insert `blog_post_assets` row mapping the post's `referenced_as` path to the asset
6. Rewrite content to use a stable internal URI: `minion-asset://{asset_id}` or relative `assets/{sha}.{ext}`

When publishing:
- **API-based platforms** (WordPress, Hashnode, Dev.to, Ghost): upload image first → get CDN URL → rewrite content
- **Manual export**: produce a ZIP `{post-slug}.zip` containing post content + `assets/` folder

### Image viewer/picker
A new "Assets" tab in Blog module:
- Grid of thumbnails of all stored images
- Click → see metadata + which posts use it + copy markdown reference
- Bulk delete unused assets (orphan cleanup)

### Tauri commands
```
blog_list_assets() → Vec<Asset>
blog_get_asset_usage(asset_id) → Vec<PostReference>
blog_upload_asset(file_path) → Asset                  // standalone upload, not tied to post
blog_delete_orphan_assets() → DeleteResult            // anything not referenced by any post
blog_get_asset_path(asset_id) → String                // for displaying in UI via convertFileSrc
```

---

## 3. Multi-platform publishing

### Platform matrix

| Platform | API? | Status | Auth | Notes |
|----------|------|--------|------|-------|
| **WordPress.com / self-hosted** | ✅ | Auto | App password | REST API at `/wp-json/wp/v2` |
| **Hashnode** | ✅ | Auto | Personal Access Token | GraphQL `/v1/api` |
| **Dev.to / Forem** | ✅ | Auto | API key | REST `/api/articles` |
| **Ghost** | ✅ | Auto | Admin API key | JWT-signed |
| **Custom Webhook** | ✅ | Auto | Bearer token | POST JSON to user-provided URL |
| **Notion** | ✅ | Auto | Integration token | Page-create API |
| **Medium** | ⚠️ Deprecated 2024 | Manual export | — | HTML preview + clipboard |
| **Substack** | ❌ No public API | Manual export | — | HTML preview + clipboard |
| **LinkedIn Articles** | ❌ Personal articles no API | Manual export | — | Open browser to article editor + clipboard |
| **X/Twitter** | ⚠️ Paid API | Manual export | — | Auto-split into thread + clipboard |

### Publishing modes

#### Auto-publish
For platforms with APIs:
- User clicks "Publish to WordPress" → backend calls API → returns `remote_url`
- Status badge updates to `published`
- `blog_platform_publications` row inserted with `remote_url` and `published_at`

#### Manual export
For platforms without APIs:
- User clicks "Export to LinkedIn" → modal opens with:
  - Platform-tailored preview (LinkedIn-friendly markdown, no code blocks → text + indentation)
  - "Copy to clipboard" button
  - "Open LinkedIn editor" button (uses tauri-plugin-shell to open browser)
- Status badge → `exported` (acknowledging user pasted it)
- `remote_url` left blank or user manually fills it

### Platform-tailored content transforms

Each platform has different rendering quirks. We need transforms:

| Platform | Transform |
|----------|-----------|
| **LinkedIn** | Strip code blocks → plain text with indentation; Convert `>` quotes to `"`; Limit to 110K chars |
| **Medium** | Keep markdown but use 4-space indented code (Medium prefers this); Add "Originally published at..." footer |
| **Substack** | Keep markdown; convert images to absolute URLs (Substack inlines them) |
| **Twitter/X** | Split into thread of 280-char chunks at paragraph boundaries; number them `1/n` |
| **Hashnode** | Use canonicalUrl frontmatter to avoid SEO duplicate penalty |
| **Dev.to** | Add `published: false` initially (always start as draft for review) |

### Tauri commands
```
blog_publish_to_platform(post_id, platform, account_id) → PublicationResult
blog_export_for_platform(post_id, platform) → ExportPayload  // {content, format, copy_text, open_url}
blog_list_publications(post_id) → Vec<Publication>
blog_unpublish(publication_id) → ()
blog_test_platform_connection(account_id) → bool
```

### Publish status grid (UI)
Per post, show a grid:
```
┌─────────────────────────────────────────────────────┐
│ "Kubernetes Storage Patterns"                       │
├─────────────────────────────────────────────────────┤
│ Platform     │ Status      │ URL          │ Action  │
├─────────────────────────────────────────────────────┤
│ WordPress    │ ✓ Published │ blog.foo.com │ [Edit]  │
│ Hashnode     │ ✓ Published │ hash.../...  │ [Edit]  │
│ Dev.to       │ ⚠ Draft     │ dev.to/...   │ [Pub]   │
│ Medium       │ — Exported  │ —            │ [Re-cp] │
│ LinkedIn     │ ◯ Pending   │ —            │ [Export]│
│ Substack     │ ◯ Pending   │ —            │ [Export]│
└─────────────────────────────────────────────────────┘
```

---

## 4. Cross-posting helpers

- **Canonical URL handling** — pick one platform as canonical (usually self-hosted WordPress or personal blog), set `rel="canonical"` headers when publishing to other platforms
- **Reading time** — compute once, reuse across all platforms
- **SEO meta** — generate Open Graph tags + Twitter cards
- **Auto social snippets** — extract a 280-char teaser from the post body
- **Newsletter version** — generate plain-text + minimal-HTML email-friendly version

---

## 5. UI tabs

```
Blog
├── Posts            (list with filter by tag/status/platform)
├── Editor           (existing markdown editor)
├── Import           (NEW - file/folder picker, frontmatter preview, tag editor)
├── Publish          (NEW - platform matrix per post, auto + manual)
├── Assets           (NEW - image gallery, usage tracking)
├── Platforms        (NEW - configure WordPress/Hashnode/Dev.to/Ghost accounts)
├── SEO Tools        (existing - keep)
└── Settings         (defaults, canonical platform pref)
```

---

## 6. Implementation phases

### Phase A — Import + Assets (1.5 days)
- Migration 010 schema
- File/folder import with frontmatter parsing
- Image extraction + dedup + storage
- New "Import" and "Assets" tabs

### Phase B — Auto-publish platforms (2 days)
- WordPress REST client
- Hashnode GraphQL client
- Dev.to API client
- Ghost Admin API (JWT)
- Custom webhook
- Account configuration UI

### Phase C — Manual export workflows (1 day)
- Platform-specific content transforms
- Export modal with preview + copy + open-browser
- Platform-tailored markdown/HTML conversion

### Phase D — Publish matrix UI (0.5 day)
- Per-post grid showing status across all platforms
- Bulk-publish actions ("publish to all configured")
- Re-publish (update existing posts)

### Phase E — Polish (0.5 day)
- Canonical URL handling
- Auto social snippets
- Newsletter format export

**Total estimated time: ~5.5 days**

---

## 7. Open questions for future iterations

- Scheduled publishing (cron-driven publish at a specific time)
- Cross-platform comment aggregation (read-only)
- Analytics ingestion from each platform (where APIs exist)
- AI-assisted writing (rewrite for tone, suggest improvements)
- Translations (multilingual publishing)
- A/B testing different titles per platform
