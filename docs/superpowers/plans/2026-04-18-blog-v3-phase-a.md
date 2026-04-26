# Blog v3 Phase A — Rich Preview Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a side-by-side split editor with live Markdown preview (pulldown-cmark GFM + Mermaid.js + SVG), debounced auto-save, and move the Import tab to a button next to New Post.

**Architecture:** Rust renders Markdown→HTML via pulldown-cmark (already in Cargo.toml) with GFM extensions enabled. Frontend injects sanitised HTML via DOMPurify (SVG allowlisted), then lazy-loads mermaid.js only when diagram blocks are present. Auto-save writes to a new `draft_content` column. The existing editor section in Blog.tsx gains a split-view toggle; a new `PreviewPane.tsx` handles all rendering concerns.

**Tech Stack:** Rust `pulldown-cmark 0.12` (existing dep), SolidJS, `mermaid` (npm), `highlight.js` (npm), `dompurify` + `@types/dompurify` (npm), SQLite migration 016.

---

## File Map

| Action | Path | Responsibility |
|---|---|---|
| Modify | `crates/minion-db/src/migrations.rs` | Migration 016 — `draft_content` column on `blog_posts` |
| Create | `src-tauri/src/blog_preview.rs` | `blog_render_preview` command — MD→HTML via pulldown-cmark GFM |
| Modify | `src-tauri/src/commands.rs` | Add `blog_update_draft` command (saves draft_content) |
| Modify | `src-tauri/src/lib.rs` | Declare `blog_preview` module, register 2 new commands |
| Modify | `ui/package.json` | Add mermaid, highlight.js, dompurify, @types/dompurify |
| Create | `ui/src/pages/blog/PreviewPane.tsx` | Renders HTML, DOMPurify sanitise, Mermaid lazy-load, highlight.js |
| Modify | `ui/src/pages/Blog.tsx` | Split-view toggle in editor, wire PreviewPane, auto-save, Import→button |

---

## Task 1: Migration 016 — draft_content column

**Files:**
- Modify: `crates/minion-db/src/migrations.rs`

- [ ] **Step 1: Write the failing test**

In `crates/minion-db/src/migrations.rs`, inside `#[cfg(test)] mod tests`, add after the last test:

```rust
#[test]
fn test_migration_016_blog_draft_content() {
    let conn = setup_test_db();
    run(&conn).expect("migrations failed");

    // draft_content column must exist and be nullable
    conn.execute(
        "INSERT INTO blog_posts (id, title, slug, content, status, created_at, updated_at, draft_content)
         VALUES ('p1', 'Test', 'test', 'body', 'draft', '2026-01-01', '2026-01-01', 'draft body')",
        [],
    ).expect("insert with draft_content failed");

    let draft: Option<String> = conn.query_row(
        "SELECT draft_content FROM blog_posts WHERE id = 'p1'",
        [],
        |r| r.get(0),
    ).unwrap();
    assert_eq!(draft.as_deref(), Some("draft body"));

    // NULL is also valid
    conn.execute(
        "INSERT INTO blog_posts (id, title, slug, content, status, created_at, updated_at)
         VALUES ('p2', 'Test2', 'test2', 'body2', 'draft', '2026-01-01', '2026-01-01')",
        [],
    ).expect("insert without draft_content failed");
}
```

- [ ] **Step 2: Run test — confirm it fails**

```bash
cd /home/dk/Documents/git/minion && cargo test -p minion-db test_migration_016_blog_draft_content 2>&1 | tail -8
```

Expected: `FAILED` — column `draft_content` does not exist.

- [ ] **Step 3: Add migration to the array**

In the `migrations` slice (line ~35), after `("015_sysmon", migrate_015_sysmon)`, add:

```rust
("016_blog_draft_content", migrate_016_blog_draft_content),
```

- [ ] **Step 4: Implement the migration function**

Add before `#[cfg(test)]`:

```rust
fn migrate_016_blog_draft_content(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "ALTER TABLE blog_posts ADD COLUMN draft_content TEXT;",
    )?;
    Ok(())
}
```

- [ ] **Step 5: Run test — confirm it passes**

```bash
cd /home/dk/Documents/git/minion && cargo test -p minion-db test_migration_016_blog_draft_content 2>&1 | tail -5
```

Expected: `test test_migration_016_blog_draft_content ... ok`

- [ ] **Step 6: Run all db tests for regressions**

```bash
cd /home/dk/Documents/git/minion && cargo test -p minion-db 2>&1 | tail -6
```

Expected: all pass (now 18+ tests).

- [ ] **Step 7: Commit**

```bash
cd /home/dk/Documents/git/minion
git add crates/minion-db/src/migrations.rs
git commit -m "feat(blog): migration 016 — draft_content column on blog_posts"
```

---

## Task 2: blog_render_preview Tauri command

**Files:**
- Create: `src-tauri/src/blog_preview.rs`

- [ ] **Step 1: Create blog_preview.rs**

Create `/home/dk/Documents/git/minion/src-tauri/src/blog_preview.rs`:

```rust
//! Markdown preview rendering for the blog editor.
//!
//! Uses pulldown-cmark with GFM extensions: tables, footnotes,
//! strikethrough, and task lists. Asset image paths (starting with
//! "asset://") are passed through unchanged. Relative local image
//! paths are left as-is for the frontend to resolve via Tauri's
//! asset protocol.

use pulldown_cmark::{html, Options, Parser};

#[tauri::command]
pub async fn blog_render_preview(markdown: String) -> Result<String, String> {
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_SMART_PUNCTUATION;

    let parser = Parser::new_ext(&markdown, opts);
    let mut html_output = String::with_capacity(markdown.len() * 2);
    html::push_html(&mut html_output, parser);
    Ok(html_output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn renders_gfm_table() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let html = blog_render_preview(md.to_string()).await.unwrap();
        assert!(html.contains("<table>"), "expected table tag, got: {html}");
        assert!(html.contains("<th>A</th>") || html.contains("<th>"), "expected th");
    }

    #[tokio::test]
    async fn renders_strikethrough() {
        let md = "~~deleted~~";
        let html = blog_render_preview(md.to_string()).await.unwrap();
        assert!(html.contains("<del>"), "expected del tag");
    }

    #[tokio::test]
    async fn renders_task_list() {
        let md = "- [x] done\n- [ ] todo";
        let html = blog_render_preview(md.to_string()).await.unwrap();
        assert!(html.contains("checkbox"), "expected checkbox input");
    }

    #[tokio::test]
    async fn fenced_code_block_has_language_class() {
        let md = "```rust\nfn main() {}\n```";
        let html = blog_render_preview(md.to_string()).await.unwrap();
        assert!(html.contains("language-rust"), "expected language-rust class");
    }

    #[tokio::test]
    async fn mermaid_fence_preserves_class() {
        let md = "```mermaid\ngraph LR\n  A --> B\n```";
        let html = blog_render_preview(md.to_string()).await.unwrap();
        assert!(html.contains("language-mermaid"), "expected language-mermaid class");
    }

    #[tokio::test]
    async fn empty_input_returns_empty() {
        let html = blog_render_preview("".to_string()).await.unwrap();
        assert!(html.trim().is_empty());
    }
}
```

- [ ] **Step 2: Run the tests — confirm they pass**

```bash
cd /home/dk/Documents/git/minion && cargo test -p minion-tauri-lib blog_render_preview 2>&1 | tail -10
```

If the crate name differs, run:
```bash
cd /home/dk/Documents/git/minion/src-tauri && cargo test blog_render_preview 2>&1 | tail -10
```

Expected: all 6 tests pass (once lib.rs is wired in Task 3 — run this after Task 3 if needed).

- [ ] **Step 3: Commit**

```bash
cd /home/dk/Documents/git/minion
git add src-tauri/src/blog_preview.rs
git commit -m "feat(blog): blog_render_preview — pulldown-cmark GFM with tables/tasks/strikethrough"
```

---

## Task 3: blog_update_draft command + lib.rs wiring

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add blog_update_draft to commands.rs**

Find `pub async fn blog_delete_post` in `src-tauri/src/commands.rs` (around line 5711). Add the following function immediately after it (after its closing brace):

```rust
#[tauri::command]
pub async fn blog_update_draft(
    state: State<'_, AppStateHandle>,
    post_id: String,
    draft_content: Option<String>,
) -> Result<(), String> {
    let st = state.read().await;
    let conn = st.db.get().map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE blog_posts SET draft_content = ?1, updated_at = ?2 WHERE id = ?3",
        rusqlite::params![draft_content, now, post_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 2: Wire both commands in lib.rs**

In `src-tauri/src/lib.rs`:

**2a.** After the existing `mod health_analysis;` / sysmon module declarations, add:

```rust
mod blog_preview;
```

**2b.** In `tauri::generate_handler![...]`, after the last `blog_publish::` entry, add:

```rust
// Blog v3 — rich preview
blog_preview::blog_render_preview,
commands::blog_update_draft,
```

- [ ] **Step 3: Build to confirm**

```bash
cd /home/dk/Documents/git/minion/src-tauri && cargo build 2>&1 | grep -E "^error" | head -20
```

Expected: no errors.

- [ ] **Step 4: Run blog_preview tests**

```bash
cd /home/dk/Documents/git/minion/src-tauri && cargo test blog_render_preview 2>&1 | tail -10
```

Expected: 6 tests pass.

- [ ] **Step 5: Commit**

```bash
cd /home/dk/Documents/git/minion
git add src-tauri/src/lib.rs src-tauri/src/commands.rs
git commit -m "feat(blog): blog_update_draft command + wire blog_preview module"
```

---

## Task 4: Install frontend dependencies

**Files:**
- Modify: `ui/package.json` (via pnpm)

- [ ] **Step 1: Install deps**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm add mermaid highlight.js dompurify && pnpm add -D @types/dompurify
```

Expected output: packages added, no errors.

- [ ] **Step 2: Verify TypeScript sees the types**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | grep -i "dompurify\|mermaid\|highlight" | head -5
```

Expected: no errors related to the new packages.

- [ ] **Step 3: Commit**

```bash
cd /home/dk/Documents/git/minion
git add ui/package.json ui/pnpm-lock.yaml
git commit -m "chore(blog): add mermaid, highlight.js, dompurify deps"
```

---

## Task 5: PreviewPane.tsx

**Files:**
- Create: `ui/src/pages/blog/PreviewPane.tsx`

- [ ] **Step 1: Create the file**

Create `/home/dk/Documents/git/minion/ui/src/pages/blog/PreviewPane.tsx`:

```tsx
import { Component, createEffect, onCleanup, useContext } from 'solid-js';
import DOMPurify from 'dompurify';

interface PreviewPaneProps {
  html: string; // already-rendered HTML from blog_render_preview
}

const DOMPURIFY_CONFIG: DOMPurify.Config = {
  USE_PROFILES: { html: true },
  ADD_TAGS: [
    'svg', 'path', 'circle', 'rect', 'line', 'polyline', 'polygon',
    'text', 'g', 'defs', 'clipPath', 'use', 'image', 'foreignObject',
    'ellipse', 'tspan', 'marker', 'linearGradient', 'radialGradient', 'stop',
  ],
  ADD_ATTR: [
    'viewBox', 'xmlns', 'd', 'fill', 'stroke', 'stroke-width', 'stroke-linecap',
    'stroke-linejoin', 'transform', 'cx', 'cy', 'r', 'rx', 'ry',
    'x', 'y', 'x1', 'y1', 'x2', 'y2', 'width', 'height',
    'points', 'clip-path', 'marker-end', 'marker-start',
    'text-anchor', 'dominant-baseline', 'font-size', 'font-family',
  ],
  FORBID_TAGS: ['script', 'style', 'iframe', 'object', 'embed', 'form'],
};

async function applyMermaid(container: HTMLElement): Promise<void> {
  const blocks = container.querySelectorAll('code.language-mermaid');
  if (blocks.length === 0) return;
  try {
    const mermaid = await import('mermaid');
    mermaid.default.initialize({ startOnLoad: false, theme: 'neutral', securityLevel: 'loose' });
    for (let i = 0; i < blocks.length; i++) {
      const el = blocks[i] as HTMLElement;
      const pre = el.closest('pre') ?? el;
      const graphDef = el.textContent ?? '';
      try {
        const id = `mermaid-${Date.now()}-${i}`;
        const { svg } = await mermaid.default.render(id, graphDef);
        const wrapper = document.createElement('div');
        wrapper.className = 'mermaid-rendered';
        wrapper.innerHTML = svg;
        pre.replaceWith(wrapper);
      } catch {
        // leave original code block on render failure
      }
    }
  } catch {
    // mermaid not available — leave code blocks as-is
  }
}

function applyHighlight(container: HTMLElement): void {
  // Lazy import highlight.js only for non-mermaid code blocks
  const blocks = container.querySelectorAll('pre code:not(.language-mermaid)');
  if (blocks.length === 0) return;
  import('highlight.js').then((hljs) => {
    blocks.forEach((block) => {
      hljs.default.highlightElement(block as HTMLElement);
    });
  }).catch(() => {});
}

const PreviewPane: Component<PreviewPaneProps> = (props) => {
  let containerRef: HTMLDivElement | undefined;

  createEffect(() => {
    const raw = props.html;
    if (!containerRef) return;

    const clean = DOMPurify.sanitize(raw, DOMPURIFY_CONFIG) as string;
    containerRef.innerHTML = clean;

    // Run async renderers after DOM is updated
    applyMermaid(containerRef);
    applyHighlight(containerRef);
  });

  onCleanup(() => {
    if (containerRef) containerRef.innerHTML = '';
  });

  return (
    <div
      ref={containerRef}
      class="prose prose-slate max-w-none h-full overflow-y-auto px-6 py-4
             prose-headings:font-bold prose-headings:text-slate-900
             prose-code:bg-slate-100 prose-code:px-1 prose-code:rounded
             prose-pre:bg-slate-900 prose-pre:text-slate-100
             prose-a:text-sky-600 prose-table:border-collapse
             prose-th:border prose-th:border-slate-300 prose-th:p-2 prose-th:bg-slate-50
             prose-td:border prose-td:border-slate-200 prose-td:p-2"
    />
  );
};

export default PreviewPane;
```

- [ ] **Step 2: Verify TypeScript compiles**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | grep "PreviewPane\|dompurify\|mermaid\|highlight" | head -10
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
cd /home/dk/Documents/git/minion
git add ui/src/pages/blog/PreviewPane.tsx
git commit -m "feat(blog): PreviewPane — DOMPurify SVG allowlist + Mermaid lazy-load + highlight.js"
```

---

## Task 6: Add split-view toggle to Blog.tsx editor

**Files:**
- Modify: `ui/src/pages/Blog.tsx`

- [ ] **Step 1: Add imports and view-mode signal**

At the top of `Blog.tsx`, add to the existing import line from `'solid-js'`:
- `onCleanup` (if not already there)

Add a new import after the existing imports:

```tsx
import { invoke } from '@tauri-apps/api/core'; // already present
import PreviewPane from './blog/PreviewPane';
```

In the component body, after the existing signals (around line 89), add:

```tsx
type ViewMode = 'editor' | 'split' | 'preview';
const [viewMode, setViewMode] = createSignal<ViewMode>('split');
const [previewHtml, setPreviewHtml] = createSignal('');
const [renderingPreview, setRenderingPreview] = createSignal(false);
```

- [ ] **Step 2: Add renderPreview helper with debounce**

After the `readingTime` helper function (around line 106), add:

```tsx
// Debounce timer ref
let previewDebounce: ReturnType<typeof setTimeout> | undefined;

const renderPreview = (markdown: string) => {
  if (previewDebounce) clearTimeout(previewDebounce);
  previewDebounce = setTimeout(async () => {
    if (!markdown.trim()) { setPreviewHtml(''); return; }
    setRenderingPreview(true);
    try {
      const html = await invoke<string>('blog_render_preview', { markdown });
      setPreviewHtml(html);
    } catch (e) {
      console.error('Preview render failed:', e);
    } finally {
      setRenderingPreview(false);
    }
  }, 400);
};

// Clean up debounce on unmount
onCleanup(() => { if (previewDebounce) clearTimeout(previewDebounce); });
```

- [ ] **Step 3: Wire renderPreview to content changes**

Find the existing content `onInput` handler inside the editor section (around line 441):

```tsx
onInput={(e) => setEdContent(e.currentTarget.value)}
```

Replace with:

```tsx
onInput={(e) => {
  const val = e.currentTarget.value;
  setEdContent(val);
  if (viewMode() !== 'editor') renderPreview(val);
}}
```

Also, find where `openPost` loads content (around line 155):

```tsx
setEdContent(full.content || '');
```

After that line add:

```tsx
renderPreview(full.content || '');
```

- [ ] **Step 4: Add view-mode toggle bar to editor tab**

Find the start of the editor Match block (around line 424):

```tsx
<Match when={tab() === 'editor'}>
  <div class="flex h-full">
    {/* Main editor area */}
```

Replace the opening with:

```tsx
<Match when={tab() === 'editor'}>
  <div class="flex flex-col h-full">
    {/* View mode toggle */}
    <div class="flex items-center gap-1 px-6 pt-3 pb-2 border-b border-gray-100 dark:border-gray-700 bg-white dark:bg-gray-800">
      <span class="text-xs text-gray-400 mr-2">View:</span>
      {(['editor', 'split', 'preview'] as ViewMode[]).map((mode) => (
        <button
          onClick={() => {
            setViewMode(mode);
            if (mode !== 'editor') renderPreview(edContent());
          }}
          class="px-3 py-1 rounded text-xs font-medium transition-colors"
          classList={{
            'bg-sky-500 text-white': viewMode() === mode,
            'bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300 hover:bg-gray-200': viewMode() !== mode,
          }}
        >
          {mode === 'editor' ? '✏️ Editor' : mode === 'split' ? '⬛ Split' : '👁 Preview'}
        </button>
      ))}
      <Show when={renderingPreview()}>
        <span class="text-xs text-gray-400 ml-2 animate-pulse">rendering…</span>
      </Show>
    </div>

    <div class="flex flex-1 overflow-hidden">
```

Then find the closing `</div>` that closes `<div class="flex h-full">` (the original wrapper) — it will be at the end of the editor Match block. Replace it with `</div></div>` (closing both the new inner flex div and the outer flex-col div).

- [ ] **Step 5: Wrap textarea in conditional show and add PreviewPane**

Find the "Main editor area" div:

```tsx
{/* Main editor area */}
<div class="flex-1 flex flex-col p-6 overflow-y-auto">
```

Replace it with:

```tsx
{/* Main editor area — shown in editor and split modes */}
<Show when={viewMode() !== 'preview'}>
  <div class={`flex flex-col p-6 overflow-y-auto ${viewMode() === 'split' ? 'w-1/2 border-r border-gray-200 dark:border-gray-700' : 'flex-1'}`}>
```

Then find the closing `</div>` of the "Main editor area" div and replace it with:

```tsx
  </div>
</Show>

{/* Preview pane — shown in split and preview modes */}
<Show when={viewMode() !== 'editor'}>
  <div class={viewMode() === 'split' ? 'w-1/2 overflow-y-auto' : 'flex-1 overflow-y-auto'}>
    <PreviewPane html={previewHtml()} />
  </div>
</Show>
```

- [ ] **Step 6: Verify TypeScript compiles**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | grep -E "error" | head -10
```

Expected: no errors.

- [ ] **Step 7: Commit**

```bash
cd /home/dk/Documents/git/minion
git add ui/src/pages/Blog.tsx
git commit -m "feat(blog): split-view editor with live preview toggle (Editor/Split/Preview)"
```

---

## Task 7: Debounced auto-save to draft_content

**Files:**
- Modify: `ui/src/pages/Blog.tsx`

- [ ] **Step 1: Add auto-save signal and timer**

In `Blog.tsx`, after the `previewDebounce` timer ref (added in Task 6), add:

```tsx
let autoSaveDebounce: ReturnType<typeof setTimeout> | undefined;
const [autoSaveStatus, setAutoSaveStatus] = createSignal<'saved' | 'saving' | 'unsaved' | 'idle'>('idle');

const triggerAutoSave = (postId: string, content: string) => {
  setAutoSaveStatus('unsaved');
  if (autoSaveDebounce) clearTimeout(autoSaveDebounce);
  autoSaveDebounce = setTimeout(async () => {
    if (!postId) return; // no post created yet — skip
    setAutoSaveStatus('saving');
    try {
      await invoke('blog_update_draft', { postId, draftContent: content });
      setAutoSaveStatus('saved');
      setTimeout(() => setAutoSaveStatus('idle'), 2000);
    } catch (e) {
      console.error('Auto-save failed:', e);
      setAutoSaveStatus('unsaved');
    }
  }, 2000);
};

onCleanup(() => { if (autoSaveDebounce) clearTimeout(autoSaveDebounce); });
```

- [ ] **Step 2: Wire auto-save to content/title changes**

Find the content `onInput` handler updated in Task 6:

```tsx
onInput={(e) => {
  const val = e.currentTarget.value;
  setEdContent(val);
  if (viewMode() !== 'editor') renderPreview(val);
}}
```

Replace with:

```tsx
onInput={(e) => {
  const val = e.currentTarget.value;
  setEdContent(val);
  if (viewMode() !== 'editor') renderPreview(val);
  const id = editingId();
  if (id) triggerAutoSave(id, val);
}}
```

- [ ] **Step 3: Add auto-save status indicator**

Find the bottom bar inside the editor area (around line 447):

```tsx
<div class="flex items-center justify-between mt-4 text-xs text-gray-400 dark:text-gray-500">
  <div class="flex gap-4">
    <span>{edWordCount()} words</span>
    <span>{edReadingTime()} min read</span>
  </div>
```

Replace with:

```tsx
<div class="flex items-center justify-between mt-4 text-xs text-gray-400 dark:text-gray-500">
  <div class="flex gap-4 items-center">
    <span>{edWordCount()} words</span>
    <span>{edReadingTime()} min read</span>
    <Show when={autoSaveStatus() === 'unsaved'}>
      <span class="text-amber-500">● Unsaved changes</span>
    </Show>
    <Show when={autoSaveStatus() === 'saving'}>
      <span class="animate-pulse">Saving…</span>
    </Show>
    <Show when={autoSaveStatus() === 'saved'}>
      <span class="text-emerald-500">✓ Draft saved</span>
    </Show>
  </div>
```

- [ ] **Step 4: Verify TypeScript**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | grep -E "error" | head -10
```

Expected: no errors.

- [ ] **Step 5: Commit**

```bash
cd /home/dk/Documents/git/minion
git add ui/src/pages/Blog.tsx
git commit -m "feat(blog): debounced auto-save to draft_content with unsaved indicator"
```

---

## Task 8: Move Import from tab to button next to New Post

**Files:**
- Modify: `ui/src/pages/Blog.tsx`

- [ ] **Step 1: Remove 'import' from the tabs array**

Find the tabs array (around line 259):

```tsx
const tabs: { id: TabId; label: string }[] = [
  { id: 'posts', label: 'Posts' },
  { id: 'editor', label: 'Editor' },
  { id: 'seo', label: 'SEO Tools' },
  { id: 'import', label: 'Import' },
  { id: 'publish', label: 'Publish' },
  { id: 'platforms', label: 'Platforms' },
  { id: 'assets', label: 'Assets' },
];
```

Replace with:

```tsx
const tabs: { id: TabId; label: string }[] = [
  { id: 'posts', label: 'Posts' },
  { id: 'editor', label: 'Editor' },
  { id: 'seo', label: 'SEO Tools' },
  { id: 'publish', label: 'Publish' },
  { id: 'platforms', label: 'Platforms' },
  { id: 'assets', label: 'Assets' },
];
```

- [ ] **Step 2: Add Import button next to New Post in header**

Find the header button section (around line 284):

```tsx
<Show when={tab() === 'posts'}>
  <button
    onClick={openNewPost}
    class="px-4 py-2 rounded-lg text-sm font-medium text-white bg-sky-500 hover:bg-sky-600 transition-colors"
  >
    New Post
  </button>
</Show>
```

Replace with:

```tsx
<Show when={tab() === 'posts' || tab() === 'editor'}>
  <div class="flex gap-2">
    <button
      onClick={() => setTab('import' as TabId)}
      class="px-4 py-2 rounded-lg text-sm font-medium text-gray-600 dark:text-gray-300 bg-gray-100 dark:bg-gray-700 hover:bg-gray-200 dark:hover:bg-gray-600 transition-colors flex items-center gap-1.5"
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
          d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-8l-4-4m0 0L8 8m4-4v12" />
      </svg>
      Import
    </button>
    <button
      onClick={openNewPost}
      class="px-4 py-2 rounded-lg text-sm font-medium text-white bg-sky-500 hover:bg-sky-600 transition-colors"
    >
      New Post
    </button>
  </div>
</Show>
```

- [ ] **Step 3: Update the TabId type to keep 'import' valid**

The `TabId` type at the top of the file currently includes `'import'`. It must stay (the import Match block still renders). No change needed to the type — only removing it from the visible tabs array.

- [ ] **Step 4: Verify TypeScript**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | grep -E "error" | head -10
```

Expected: no errors.

- [ ] **Step 5: Commit**

```bash
cd /home/dk/Documents/git/minion
git add ui/src/pages/Blog.tsx
git commit -m "feat(blog): move Import from tab to button next to New Post"
```

---

## Task 9: E2E smoke test

- [ ] **Step 1: Run all Rust tests**

```bash
cd /home/dk/Documents/git/minion && cargo test --workspace 2>&1 | grep -E "test result|FAILED" | head -20
```

Expected: all pass. Look for `test_migration_016_blog_draft_content ... ok` and all 6 `blog_render_preview` tests.

- [ ] **Step 2: Run Clippy**

```bash
cd /home/dk/Documents/git/minion && cargo clippy --workspace -- -D warnings 2>&1 | grep -E "^error" | head -10
```

Expected: no errors.

- [ ] **Step 3: Run frontend typecheck and lint**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | grep -E "error" | head -10
cd /home/dk/Documents/git/minion/ui && pnpm lint 2>&1 | grep -E "error" | head -10
```

Expected: no errors.

- [ ] **Step 4: Final commit if any cleanup needed**

```bash
cd /home/dk/Documents/git/minion
git add -A
git commit -m "fix(blog): Phase A smoke test cleanup"
git push origin main
```

---

## Self-Review Checklist

| Spec requirement | Task |
|---|---|
| pulldown-cmark GFM (tables, footnotes, strikethrough, tasklists) | Task 2 (`blog_render_preview` with Options flags) |
| Mermaid lazy-loaded only when diagrams present | Task 5 (`PreviewPane.tsx` — `import('mermaid')` gated on selector) |
| DOMPurify with SVG allowlist | Task 5 (`DOMPURIFY_CONFIG` with ADD_TAGS/ADD_ATTR) |
| Side-by-side split editor (Editor / Split / Preview toggle) | Task 6 |
| Live preview updates on keystroke (400ms debounce) | Task 6 (`renderPreview` with setTimeout) |
| Debounced auto-save to draft_content (2s) | Task 7 |
| Unsaved changes indicator | Task 7 |
| Import moved from tab to button next to New Post | Task 8 |
| Migration 016 `draft_content` column | Task 1 |
| `blog_render_preview` Tauri command registered | Task 3 |
| `blog_update_draft` Tauri command registered | Task 3 |
| highlight.js for syntax highlighting | Task 5 (`applyHighlight` in PreviewPane) |
