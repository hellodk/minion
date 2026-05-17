# Blog AI Generation + LinkedIn/Medium Export — Design Spec

**Goal:** Let users generate full blog content with AI (blank canvas or improve existing) directly in the editor, and export properly formatted output for LinkedIn and Medium publishing.

**Architecture:** Two isolated additions — an AI Draft bar injected above the editor textarea, and LinkedIn/Medium sections added to the existing LlmAssistantPanel Distribute tab. No new pages, no new routes.

**Tech Stack:** SolidJS + TypeScript (UI), Rust/Tauri (backend), existing `llm_router`, Tauri event streaming (`blog-llm-stream`), `blog_post_variants` table (already exists).

---

## 1. AI Draft Bar (Editor Tab)

### Location
Rendered between the editor toolbar and the `<textarea>` in `Blog.tsx` whenever `editingId()` is non-null.

### Wire-frame (text)
```
[✨ Write]  [✨ Improve]                    (idle)
──────────────────────────────────────────
[✨ Write]  [✨ Improve]  Generating… 347 words  [✗ Cancel]  (streaming)
  ┌──── Write panel (only when Write expanded) ────┐
  │ Outline / key points (optional):               │
  │ ┌──────────────────────────────────────────┐   │
  │ │                                          │   │
  │ └──────────────────────────────────────────┘   │
  │ Tone: ● Conversational  ○ Balanced  ○ Technical │
  │                                [Cancel] [Write] │
  └────────────────────────────────────────────────┘
```

### Behaviour
- **✨ Write**: toggles inline panel below the bar. User optionally types an outline. Clicking "Write" calls `blog_llm_generate`. Streaming chunks are appended to `edContent()` in the editor in real time. On `done`, panel auto-collapses.
- **✨ Improve**: calls `blog_llm_improve` immediately (no panel). Streaming chunks replace `edContent()`. While streaming both buttons are disabled; a cancel button appears.
- **Cancel**: stops listening to the stream event; restores original `edContent()` from a snapshot taken at generation start.
- **Word count ticker**: shows live word count during streaming (recalculated on each chunk).
- Both buttons are disabled when no LLM is configured (checked via existing `blog_llm_status` Tauri command).

---

## 2. LinkedIn & Medium Export (LlmAssistantPanel — Distribute Tab)

### Location
Added below the existing TWITTER / LINKEDIN / SUBSTACK / GENERIC snippet cards in `LlmAssistantPanel.tsx` (the "distribute" PanelTab). Two new collapsible sections.

### LinkedIn Post Section
- Header: "LinkedIn Post (ready to paste)"
- Action button: "Generate LinkedIn Post"
- Output format rules enforced by the LLM prompt:
  - Hook sentence on its own line (3 lines max before implied "…see more" fold)
  - Empty line between every 2-3 sentences
  - `•` bullet characters (not `-` or `*`)
  - NO markdown headers, bold, or italic markers
  - 3-5 hashtags block on its own line at end (e.g. `#pnpm #nodejs #devtools`)
  - Target: 900–1,300 characters
  - End with a question to drive comments
- UI: textarea (read-only, monospace, 10 rows) + char count badge (green ≤1300, amber ≤2000, red >2000) + "Copy" button
- Saved as variant type `platform_linkedin_post` in `blog_post_variants`

### Medium Article Section
- Header: "Medium Article (Markdown)"
- Action button: "Generate Medium Draft"
- Output format rules:
  - `# Title` (H1)
  - First paragraph: italic subtitle (e.g. `*A deep dive into…*`)
  - `## Section` headers (H2) only — no H3 inside
  - At least one `> Pull quote` blockquote extracted from key insight
  - Code blocks with language fences (\`\`\`js etc.)
  - Bottom line: `**Tags:** tag1, tag2, tag3, tag4, tag5` (max 5)
  - Bottom line: `*Originally published at [your blog]*`
  - Clean, no trailing spaces, single blank line between sections
- UI: textarea (read-only, monospace, 15 rows) + "Copy" button + "Save as Variant" button
- Saved as variant type `platform_medium` in `blog_post_variants`

---

## 3. New Rust Commands

### `blog_llm_generate`
```
Input:  post_id: String, outline: Option<String>, tone: String
Output: streams via Tauri event "blog-llm-stream"
```
- Reads `title` from DB (post must exist, even as empty draft)
- System prompt builds a structured blog post: intro, 3-5 H2 sections, conclusion with CTA
- Tone maps to: "conversational/first-person/contractions", "balanced/clear/professional", "technical/formal/precise"
- Streams chunks via `app.emit("blog-llm-stream", LlmStreamEvent { event_type, data })`
- On completion, writes full content to `draft_content` column

### `blog_llm_improve`
```
Input:  post_id: String
Output: streams via Tauri event "blog-llm-stream"
```
- Reads `COALESCE(draft_content, content)` from DB
- Prompt: preserve structure, fix passive voice, strengthen verbs, sharpen opening, improve flow
- Same streaming protocol as `blog_llm_generate`
- On completion, writes improved content to `draft_content`

### Stream event type (already defined in `llm_router.rs`)
```rust
pub struct LlmStreamEvent {
    pub event_type: String,  // "chunk" | "done" | "error"
    pub data: String,
}
```

### Enhanced `blog_llm_adapt` — LinkedIn and Medium
The existing `blog_llm_adapt` command already handles `"linkedin"` and `"medium"` as platforms. Enhance the prompt strings for both to enforce the format rules above. No signature change needed.

For the LinkedIn Post (short format, distinct from LinkedIn Article): add a new match arm `"linkedin_post"` to `blog_llm_adapt` that generates the short-form LinkedIn post (900-1,300 chars, no markdown). This is separate from the existing `"linkedin"` arm which generates a full LinkedIn article.

---

## 4. Frontend Changes

### `Blog.tsx`
- Add `AiDraftBar` inner component rendered between toolbar and textarea
- Signals: `aiDraftOpen`, `aiDraftOutline`, `aiDraftTone`, `aiStreaming`, `aiStreamUnlisten`, `aiOriginalContent`
- `startGenerate(mode: 'write' | 'improve')`: snapshot current content → call appropriate command → subscribe to `blog-llm-stream` → append/replace `edContent()` live
- `cancelGenerate()`: call `aiStreamUnlisten()` → restore `aiOriginalContent`

### `LlmAssistantPanel.tsx`
- In the `distribute` PanelTab, after the snippet cards, add `LinkedInSection` and `MediumSection` sub-components
- Each has: loading state, generated content signal, copy handler, save-as-variant handler

---

## 5. Error Handling & Graceful Degradation
- If no LLM configured: Draft bar buttons are disabled with tooltip "Configure an LLM endpoint in Settings"
- If LLM call fails mid-stream: emit `{type: "error", data: "..."}` → toast error → restore original content
- LinkedIn char limit: warn if >2,000 chars (don't block, just badge turns red)
- Medium: if content has no H2 sections, AI is prompted to create them; worst case user sees plain paragraphs

---

## 6. What Does NOT Change
- No new DB migrations
- No new routes or pages
- No changes to `blog_export.rs`, `blog_lint.rs`, `blog_publish.rs`
- `blog_llm_adapt` existing arms (`devto`, `hashnode`, `substack`) are unchanged
- The `blog-llm-stream` Tauri event key must not conflict with other stream events — it is scoped to blog commands only (Explorer uses `llm-stream`)

---

## 7. Spec Self-Review

**Placeholder scan:** None found. All sections specify exact prompts, types, event names, and DB columns.

**Internal consistency:**
- Stream event name: `blog-llm-stream` used consistently (different from Explorer's `llm-stream`)
- Variant types: `platform_linkedin_post`, `platform_medium` — distinct from existing `platform_linkedin` (article)
- `draft_content` column exists (migration 016); no new column needed

**Scope:** Focused on two UI surfaces and two Rust commands. Does not touch export, publish, or import flows.

**Ambiguity:**
- "Improve" replaces `edContent()` entirely (not a diff/merge) — this is intentional and matches user expectation of a rewrite
- LinkedIn "Post" vs "Article": spec explicitly distinguishes short-form post (new arm `linkedin_post`) from existing article adaptation (`linkedin`)
