# Blog AI Generation + LinkedIn/Medium Export — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add AI content generation (blank canvas + improve existing) via a lightweight draft bar above the blog editor textarea, and add properly-formatted LinkedIn post and Medium article exports to the LlmAssistantPanel distribute tab.

**Architecture:** Two new Rust commands (`blog_llm_generate`, `blog_llm_improve`) stream generated content via the existing `llm-stream` Tauri event (same protocol as Explorer AI Fix). A new `AiDraftBar` component is inserted between the toolbar and textarea in `Blog.tsx`. LinkedIn/Medium sections are added to the distribute tab in `LlmAssistantPanel.tsx` by enhancing `blog_llm_adapt` with a `linkedin_post` arm and improving the `medium` arm.

**Tech Stack:** Rust/Tauri 2, SolidJS + TypeScript, existing `llm_router::stream_call`, existing `blog_post_variants` table, `@tauri-apps/api/event` `listen`.

---

## File Map

| File | Change |
|------|--------|
| `src-tauri/src/blog_llm.rs` | Add `blog_llm_generate`, `blog_llm_improve` commands; add `linkedin_post` arm and improve `medium` arm in `blog_llm_adapt` |
| `src-tauri/src/lib.rs` | Register 2 new commands |
| `ui/src/pages/Blog.tsx` | Add AI draft bar signals + `AiDraftBar` inner component above the textarea |
| `ui/src/pages/blog/LlmAssistantPanel.tsx` | Add LinkedIn Post + Medium Article sections in distribute tab |

No new files. No migrations.

---

## Task 1: `blog_llm_generate` — Rust command (stream blank-canvas generation)

**Files:**
- Modify: `src-tauri/src/blog_llm.rs` (append after `blog_get_snippets`)

- [ ] **Step 1: Add the command to `blog_llm.rs`**

Append this at the end of `src-tauri/src/blog_llm.rs`, before the final closing brace (there is none — just append):

```rust
#[tauri::command]
pub async fn blog_llm_generate(
    app: tauri::AppHandle,
    state: State<'_, AppStateHandle>,
    post_id: String,
    outline: Option<String>,
    tone: String,
    call_id: String,
) -> Result<(), String> {
    let db = { state.read().await.db.clone() };
    let (title, _) = {
        let c = db.get().map_err(|e| e.to_string())?;
        fetch_post(&c, &post_id)?
    };

    let tone_instruction = match tone.as_str() {
        "technical"      => "formal, precise, third-person, technical terminology, no contractions",
        "balanced"       => "clear and professional but approachable, occasional contractions",
        "conversational" => "casual, friendly, first-person, contractions, relatable analogies",
        _                => "clear and professional but approachable, occasional contractions",
    };

    let outline_section = outline
        .as_deref()
        .filter(|o| !o.trim().is_empty())
        .map(|o| format!("\n\nKey points / outline to cover:\n{}", o))
        .unwrap_or_default();

    let system = format!(
        "You are an expert technical writer. Write a complete, publication-ready blog post in Markdown.\n\
         Tone: {tone_instruction}.\n\
         Structure:\n\
         - # Title (reuse or improve the given title)\n\
         - Opening paragraph that hooks the reader\n\
         - 3-5 ## section headings with substantive content (200-400 words each)\n\
         - Code blocks with language fences where relevant\n\
         - Closing section with a concrete call-to-action\n\
         Return ONLY the Markdown content — no explanations, no surrounding fences."
    );
    let user = format!("Post title: {title}{outline_section}");

    crate::llm_router::stream_call(
        app,
        &state,
        "blog_llm_generate".to_string(),
        call_id,
        system,
        user,
        Some(0.7),
        Some(4096),
        String::new(),
    )
    .await
}

#[tauri::command]
pub async fn blog_llm_improve(
    app: tauri::AppHandle,
    state: State<'_, AppStateHandle>,
    post_id: String,
    call_id: String,
) -> Result<(), String> {
    let db = { state.read().await.db.clone() };
    let (title, content) = {
        let c = db.get().map_err(|e| e.to_string())?;
        fetch_post(&c, &post_id)?
    };

    let excerpt = if content.len() > 16_000 { &content[..16_000] } else { &content };

    let system = "You are an expert editor. Improve the following blog post:\n\
                  - Sharpen the opening to hook the reader immediately\n\
                  - Replace passive voice with active verbs\n\
                  - Remove filler words (very, just, really, quite)\n\
                  - Strengthen weak verbs (is/are/was/were → precise verbs)\n\
                  - Ensure all ## sections have a strong topic sentence\n\
                  - Improve flow between paragraphs\n\
                  - Preserve all code blocks, headings, and factual content exactly\n\
                  Return ONLY the improved Markdown — no explanations.";
    let user = format!("Post title: {title}\n\n{excerpt}");

    crate::llm_router::stream_call(
        app,
        &state,
        "blog_llm_improve".to_string(),
        call_id,
        system.to_string(),
        user,
        Some(0.4),
        Some(4096),
        String::new(),
    )
    .await
}
```

- [ ] **Step 2: Add `linkedin_post` arm and improve `medium` arm in `blog_llm_adapt`**

In `src-tauri/src/blog_llm.rs`, find `blog_llm_adapt`. The `platform_instructions` match currently ends with `_ => return Err(...)`. Replace the entire match block (lines ~366–382) with:

```rust
    let platform_instructions = match platform.as_str() {
        "devto" => {
            "Dev.to style: technical, conversational, add a short TL;DR at top, use informal tone, \
             add canonical URL note at the bottom as: 'Originally published at [URL]'"
        }
        "hashnode" => {
            "Hashnode style: add a subtitle below the title, use numbered lists for steps, \
             include a 'Key takeaways' section at the end"
        }
        "medium" => {
            "Medium Article style:\n\
             1. Keep the # Title as the first line\n\
             2. Second line: an italic subtitle paragraph starting with * (e.g. *A deep dive into…*)\n\
             3. Use ## for all section headings (H2 only — no H3)\n\
             4. Extract one key insight as a > blockquote pull quote\n\
             5. All code blocks must have language fences (```js, ```rust, etc.)\n\
             6. End the post with a line: **Tags:** tag1, tag2, tag3, tag4, tag5 (max 5 relevant tags)\n\
             7. Final line: *Originally published at [your blog]*\n\
             Return only the Markdown — no preamble."
        }
        "substack" => {
            "Substack newsletter style: personal opener (e.g. 'Hey friends,'), \
             casual conversational tone, end with a personal sign-off and newsletter CTA"
        }
        "linkedin" => {
            "LinkedIn Article style: compress to key points, \
             add bold headers for each major point, \
             end with a question to drive comments"
        }
        "linkedin_post" => {
            "LinkedIn Post style (NOT an article — this is a short post for the feed):\n\
             1. First 1-2 lines are the HOOK — they must grab attention before the 'see more' fold\n\
             2. Use a blank line between every 2-3 sentences (LinkedIn renders line breaks)\n\
             3. Use • for bullet points — NOT hyphens or asterisks\n\
             4. NO markdown: no #headers, no **bold**, no *italic*\n\
             5. End with a 3-5 hashtag block on its own line (e.g. #pnpm #nodejs #devtools)\n\
             6. Final line: a question to invite comments (e.g. 'Have you made the switch yet?')\n\
             7. Target: 900–1,300 characters total\n\
             Return ONLY the plain text post — no markdown, no preamble."
        }
        _ => return Err(format!("Unknown platform: {}", platform)),
    };
```

- [ ] **Step 3: Build and verify compilation**

```bash
cargo build -p minion-tauri 2>&1 | grep -E "^error|Finished"
```
Expected: `Finished dev ...`

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/blog_llm.rs
git commit -m "feat(blog-llm): add generate, improve commands; linkedin_post + medium format"
```

---

## Task 2: Register new commands in `lib.rs`

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Register the two new commands**

In `src-tauri/src/lib.rs`, find the block where `blog_llm::blog_llm_adapt` is registered (around line 303). Add the two new commands after `blog_llm::blog_llm_adapt`:

```rust
            blog_llm::blog_llm_generate,
            blog_llm::blog_llm_improve,
```

- [ ] **Step 2: Build to confirm no registration errors**

```bash
cargo build -p minion-tauri 2>&1 | grep -E "^error|Finished"
```
Expected: `Finished dev ...`

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(blog-llm): register blog_llm_generate and blog_llm_improve commands"
```

---

## Task 3: `AiDraftBar` component in `Blog.tsx`

**Files:**
- Modify: `ui/src/pages/Blog.tsx`

The `AiDraftBar` lives entirely inside `Blog.tsx` as an inner component (same pattern as other inner components). It uses signals from the outer `Blog` scope via closure.

- [ ] **Step 1: Add streaming imports and AI draft bar signals**

At the top of `Blog.tsx`, the file already imports from `solid-js` and `@tauri-apps/api/core`. Add the event listener import:

Find:
```typescript
import { invoke } from '@tauri-apps/api/core';
```
Replace with:
```typescript
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
```

Then in the `Blog` component body, after the existing `const [showLlmPanel, setShowLlmPanel] = createSignal(false);` line, add:

```typescript
  // AI Draft Bar state
  const [aiDraftOpen, setAiDraftOpen] = createSignal(false);
  const [aiDraftOutline, setAiDraftOutline] = createSignal('');
  const [aiDraftTone, setAiDraftTone] = createSignal<'conversational' | 'balanced' | 'technical'>('conversational');
  const [aiDraftStreaming, setAiDraftStreaming] = createSignal(false);
  const [aiDraftStage, setAiDraftStage] = createSignal('');
  const [aiDraftWords, setAiDraftWords] = createSignal(0);
  const [aiDraftError, setAiDraftError] = createSignal<string | null>(null);
  let aiDraftUnlisten: (() => void) | undefined;
  let aiDraftOriginal = '';
```

- [ ] **Step 2: Add `startAiDraft` and `cancelAiDraft` functions**

After the `applySeoSuggestion` function (around line 250), add:

```typescript
  const LlmStreamEventSchema = {} as {
    call_id: string; stage: string; chunk?: string;
    content?: string; model?: string; elapsed_ms: number; error?: string;
  };
  type LlmStreamEvent = typeof LlmStreamEventSchema;

  const startAiDraft = async (mode: 'write' | 'improve') => {
    const id = editingId();
    if (!id) return;

    aiDraftOriginal = edContent();
    setAiDraftStreaming(true);
    setAiDraftStage('Connecting…');
    setAiDraftError(null);
    setAiDraftWords(0);

    const callId = `blog-draft-${Date.now()}`;
    let accumulated = '';

    if (aiDraftUnlisten) aiDraftUnlisten();
    aiDraftUnlisten = await listen<LlmStreamEvent>('llm-stream', (event) => {
      const ev = event.payload;
      if (ev.call_id !== callId) return;
      if (ev.stage === 'connecting') {
        setAiDraftStage('Connecting…');
      } else if (ev.stage === 'thinking' || ev.stage === 'generating') {
        setAiDraftStage('Generating…');
      } else if (ev.stage === 'chunk') {
        accumulated += ev.chunk ?? '';
        setEdContent(accumulated);
        setAiDraftWords(wordCount(accumulated));
        setAiDraftStage('Writing…');
      } else if (ev.stage === 'done') {
        const final = ev.content ?? accumulated;
        setEdContent(final);
        setAiDraftWords(wordCount(final));
        setAiDraftStreaming(false);
        setAiDraftStage('');
        setAiDraftOpen(false);
        if (aiDraftUnlisten) { aiDraftUnlisten(); aiDraftUnlisten = undefined; }
        // Auto-save the generated content
        invoke('blog_update_draft', { postId: id, draftContent: final }).catch(() => {});
      } else if (ev.stage === 'error') {
        setAiDraftError(ev.error ?? 'Generation failed');
        setEdContent(aiDraftOriginal);
        setAiDraftStreaming(false);
        setAiDraftStage('');
        if (aiDraftUnlisten) { aiDraftUnlisten(); aiDraftUnlisten = undefined; }
      }
    });

    try {
      if (mode === 'write') {
        await invoke('blog_llm_generate', {
          postId: id,
          outline: aiDraftOutline().trim() || null,
          tone: aiDraftTone(),
          callId,
        });
      } else {
        await invoke('blog_llm_improve', { postId: id, callId });
      }
    } catch (e) {
      setAiDraftError(`Failed to start: ${e}`);
      setEdContent(aiDraftOriginal);
      setAiDraftStreaming(false);
      setAiDraftStage('');
      if (aiDraftUnlisten) { aiDraftUnlisten(); aiDraftUnlisten = undefined; }
    }
  };

  const cancelAiDraft = () => {
    if (aiDraftUnlisten) { aiDraftUnlisten(); aiDraftUnlisten = undefined; }
    setEdContent(aiDraftOriginal);
    setAiDraftStreaming(false);
    setAiDraftStage('');
    setAiDraftOpen(false);
  };
```

Also add cleanup in `onCleanup` — find the existing `onCleanup(() => { if (autoSaveDebounce) clearTimeout(autoSaveDebounce); });` and add after it:

```typescript
  onCleanup(() => { if (aiDraftUnlisten) aiDraftUnlisten(); });
```

- [ ] **Step 3: Add `AiDraftBar` inner component JSX**

Find the `{/* Content */}` comment in the editor tab (before the `<textarea` for `edContent`). The current structure is:

```
{/* Content */}
<textarea
  placeholder="Start writing your post in markdown..."
```

Insert the AI Draft Bar **between** the title `<input>` and the content `<textarea>`. Add this block after the closing `/>` of the title input and before the `{/* Content */}` comment:

```tsx
                {/* AI Draft Bar */}
                <Show when={editingId()}>
                  <div class="mb-3">
                    <div class="flex items-center gap-2 p-2 bg-gray-50 dark:bg-gray-900/50 border border-gray-200 dark:border-gray-700 rounded-lg">
                      <Show when={!aiDraftStreaming()} fallback={
                        <>
                          <span class="text-xs text-gray-500 animate-pulse flex-1">{aiDraftStage()} {aiDraftWords() > 0 ? `· ${aiDraftWords()} words` : ''}</span>
                          <button
                            onClick={cancelAiDraft}
                            class="px-2 py-1 text-xs bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-red-600 dark:text-red-400 rounded hover:bg-red-100 transition-colors"
                          >
                            ✕ Cancel
                          </button>
                        </>
                      }>
                        <button
                          onClick={() => { setAiDraftOpen(v => !v); setAiDraftError(null); }}
                          class="flex items-center gap-1 px-2.5 py-1 text-xs font-medium bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-600 rounded-md hover:border-sky-300 hover:text-sky-600 transition-colors text-gray-600 dark:text-gray-300"
                        >
                          ✨ Write
                        </button>
                        <button
                          onClick={() => startAiDraft('improve')}
                          disabled={!edContent().trim()}
                          class="flex items-center gap-1 px-2.5 py-1 text-xs font-medium bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-600 rounded-md hover:border-sky-300 hover:text-sky-600 transition-colors text-gray-600 dark:text-gray-300 disabled:opacity-40 disabled:cursor-not-allowed"
                        >
                          ✨ Improve
                        </button>
                        <span class="text-[10px] text-gray-400 ml-auto">AI drafting</span>
                      </Show>
                    </div>

                    <Show when={aiDraftError()}>
                      <p class="mt-1 text-xs text-red-500 dark:text-red-400">⚠ {aiDraftError()}</p>
                    </Show>

                    <Show when={aiDraftOpen() && !aiDraftStreaming()}>
                      <div class="mt-2 p-3 bg-white dark:bg-gray-900 border border-sky-200 dark:border-sky-800 rounded-lg space-y-2">
                        <div>
                          <label class="block text-[11px] font-medium text-gray-500 dark:text-gray-400 mb-1">
                            Key points / outline (optional)
                          </label>
                          <textarea
                            placeholder="e.g.&#10;- What is pnpm&#10;- Content-addressable store&#10;- Hard links explained"
                            value={aiDraftOutline()}
                            onInput={(e) => setAiDraftOutline(e.currentTarget.value)}
                            rows={4}
                            class="w-full px-2 py-1.5 text-xs border border-gray-200 dark:border-gray-700 rounded bg-gray-50 dark:bg-gray-800 text-gray-800 dark:text-gray-200 resize-none outline-none focus:border-sky-300"
                          />
                        </div>
                        <div>
                          <label class="block text-[11px] font-medium text-gray-500 dark:text-gray-400 mb-1">Tone</label>
                          <div class="flex gap-3">
                            <For each={(['conversational', 'balanced', 'technical'] as const)}>
                              {(t) => (
                                <label class="flex items-center gap-1 text-xs text-gray-600 dark:text-gray-300 cursor-pointer">
                                  <input
                                    type="radio"
                                    name="ai-draft-tone"
                                    value={t}
                                    checked={aiDraftTone() === t}
                                    onChange={() => setAiDraftTone(t)}
                                    class="accent-sky-500"
                                  />
                                  {t.charAt(0).toUpperCase() + t.slice(1)}
                                </label>
                              )}
                            </For>
                          </div>
                        </div>
                        <div class="flex justify-end gap-2 pt-1">
                          <button
                            onClick={() => setAiDraftOpen(false)}
                            class="px-3 py-1 text-xs text-gray-500 border border-gray-200 dark:border-gray-700 rounded hover:bg-gray-50 dark:hover:bg-gray-800"
                          >
                            Cancel
                          </button>
                          <button
                            onClick={() => startAiDraft('write')}
                            disabled={!edTitle().trim()}
                            class="px-3 py-1 text-xs font-medium bg-sky-500 text-white rounded hover:bg-sky-600 disabled:opacity-40 disabled:cursor-not-allowed"
                          >
                            ✨ Generate
                          </button>
                        </div>
                      </div>
                    </Show>
                  </div>
                </Show>
```

- [ ] **Step 4: Also add `For` to the solid-js import if not present**

Check the top of `Blog.tsx` — `For` should already be imported (it is used in the file). Confirm:
```bash
grep "^import.*For" ui/src/pages/Blog.tsx
```
Expected: `import { Component, createSignal, createMemo, For, Show, Switch, Match, onMount, onCleanup } from 'solid-js';`

If `For` is missing, add it to the destructured import list.

- [ ] **Step 5: Typecheck**

```bash
cd ui && pnpm typecheck 2>&1 | grep -E "error TS|Error" | head -20
```
Expected: no errors related to `Blog.tsx` or the new signals.

- [ ] **Step 6: Commit**

```bash
git add ui/src/pages/Blog.tsx
git commit -m "feat(blog-ui): add AI draft bar with generate and improve streaming"
```

---

## Task 4: LinkedIn Post + Medium Article in LlmAssistantPanel distribute tab

**Files:**
- Modify: `ui/src/pages/blog/LlmAssistantPanel.tsx`

- [ ] **Step 1: Add signals for LinkedIn Post and Medium Article**

In `LlmAssistantPanel.tsx`, find the `// ── Distribute` comment section (around line 219). After `const [snippetsTried, ...]`, add:

```typescript
  // ── LinkedIn Post ─────────────────────────────────────────────────────────
  const [linkedinPost, setLinkedinPost] = createSignal('');
  const [linkedinLoading, setLinkedinLoading] = createSignal(false);
  const [linkedinError, setLinkedinError] = createSignal('');

  // ── Medium Article ────────────────────────────────────────────────────────
  const [mediumArticle, setMediumArticle] = createSignal('');
  const [mediumLoading, setMediumLoading] = createSignal(false);
  const [mediumError, setMediumError] = createSignal('');
```

- [ ] **Step 2: Add `runLinkedinPost` and `runMediumArticle` async functions**

Find the existing `runAdapt` function in the file. Add these two new functions alongside it:

```typescript
  const runLinkedinPost = async () => {
    if (!props.postId) return;
    setLinkedinLoading(true);
    setLinkedinError('');
    try {
      const v = await invoke<{ content: string } | null>('blog_llm_adapt', {
        postId: props.postId,
        platform: 'linkedin_post',
      });
      setLinkedinPost(v?.content ?? '');
      if (!v) setLinkedinError('No output from AI.');
    } catch (e) {
      setLinkedinError(String(e));
    } finally {
      setLinkedinLoading(false);
    }
  };

  const runMediumArticle = async () => {
    if (!props.postId) return;
    setMediumLoading(true);
    setMediumError('');
    try {
      const v = await invoke<{ content: string } | null>('blog_llm_adapt', {
        postId: props.postId,
        platform: 'medium',
      });
      setMediumArticle(v?.content ?? '');
      if (!v) setMediumError('No output from AI.');
    } catch (e) {
      setMediumError(String(e));
    } finally {
      setMediumLoading(false);
    }
  };
```

- [ ] **Step 3: Add LinkedIn Post and Medium Article UI sections to the distribute tab**

In the distribute tab JSX (around line 541), find the closing `</Show>` of the `{/* Saved Variants */}` block (the last section before the closing `</Show>` of `tab() === 'distribute'`). Insert **before** the Saved Variants section:

```tsx
          {/* LinkedIn Post */}
          <div class="border-t border-gray-100 dark:border-gray-700 pt-3">
            <div class="flex items-center justify-between mb-2">
              <span class="text-xs font-semibold text-gray-600 dark:text-gray-300">LinkedIn Post</span>
              <ActionBtn label="Generate" loading={linkedinLoading()} onClick={runLinkedinPost} />
            </div>
            <Show when={linkedinError()}><ErrorNote msg={linkedinError()} /></Show>
            <Show when={linkedinLoading()}>
              <p class="text-[10px] text-gray-400 italic">Formatting for LinkedIn feed… ~30s</p>
            </Show>
            <Show when={linkedinPost()}>
              <div class="relative">
                <textarea
                  readOnly
                  value={linkedinPost()}
                  rows={8}
                  class="w-full text-xs font-mono p-2 border border-gray-200 dark:border-gray-700 rounded-lg bg-gray-50 dark:bg-gray-900 text-gray-700 dark:text-gray-300 resize-none"
                />
                <div class="flex items-center justify-between mt-1">
                  <span
                    class="text-[10px]"
                    classList={{
                      'text-emerald-600': linkedinPost().length <= 1300,
                      'text-amber-500': linkedinPost().length > 1300 && linkedinPost().length <= 2000,
                      'text-red-500': linkedinPost().length > 2000,
                    }}
                  >
                    {linkedinPost().length} chars {linkedinPost().length > 1300 ? '(over recommended 1,300)' : ''}
                  </span>
                  <button
                    onClick={() => navigator.clipboard.writeText(linkedinPost())}
                    class="text-[10px] text-sky-600 hover:text-sky-800 font-medium"
                  >
                    Copy
                  </button>
                </div>
              </div>
            </Show>
          </div>

          {/* Medium Article */}
          <div class="border-t border-gray-100 dark:border-gray-700 pt-3">
            <div class="flex items-center justify-between mb-2">
              <span class="text-xs font-semibold text-gray-600 dark:text-gray-300">Medium Article</span>
              <ActionBtn label="Generate" loading={mediumLoading()} onClick={runMediumArticle} />
            </div>
            <Show when={mediumError()}><ErrorNote msg={mediumError()} /></Show>
            <Show when={mediumLoading()}>
              <p class="text-[10px] text-gray-400 italic">Formatting for Medium… ~30s</p>
            </Show>
            <Show when={mediumArticle()}>
              <div class="relative">
                <textarea
                  readOnly
                  value={mediumArticle()}
                  rows={10}
                  class="w-full text-xs font-mono p-2 border border-gray-200 dark:border-gray-700 rounded-lg bg-gray-50 dark:bg-gray-900 text-gray-700 dark:text-gray-300 resize-none"
                />
                <div class="flex justify-end mt-1 gap-2">
                  <button
                    onClick={() => navigator.clipboard.writeText(mediumArticle())}
                    class="text-[10px] text-sky-600 hover:text-sky-800 font-medium"
                  >
                    Copy Markdown
                  </button>
                </div>
              </div>
            </Show>
          </div>
```

- [ ] **Step 4: Typecheck**

```bash
cd ui && pnpm typecheck 2>&1 | grep -E "error TS|Error" | head -20
```
Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add ui/src/pages/blog/LlmAssistantPanel.tsx
git commit -m "feat(blog-ui): add LinkedIn post + Medium article export in distribute tab"
```

---

## Task 5: End-to-end smoke test + audit

**Files:** No code changes — verification only.

- [ ] **Step 1: Build the full app**

```bash
cd /home/dk/Documents/git/minion
cargo build -p minion-tauri 2>&1 | grep -E "^error|Finished"
cd ui && pnpm build 2>&1 | grep -E "error|Error|✓" | tail -10
```
Expected: both succeed with no errors.

- [ ] **Step 2: Run Rust tests**

```bash
cd /home/dk/Documents/git/minion
cargo test --workspace 2>&1 | tail -10
```
Expected: all tests pass (test count ≥ 623).

- [ ] **Step 3: Manual smoke test — AI Generate (Write)**
- Launch app: `cargo tauri dev`
- Go to Blog → Editor tab → create or open a post with a title but empty content
- Confirm AI Draft Bar appears below the title input
- Click `✨ Write` → panel expands
- Enter outline: `- What is pnpm\n- Hard links\n- Benefits`
- Select Tone: Conversational
- Click Generate
- Confirm content streams into editor live (words counter increments)
- Confirm panel collapses when done
- Confirm `draft_content` was auto-saved (check autoSaveStatus)

- [ ] **Step 4: Manual smoke test — AI Improve**
- With content in the editor, click `✨ Improve`
- Confirm stage shows "Connecting… → Writing…"
- Confirm content updates live during streaming
- Click Cancel mid-stream → content reverts to original

- [ ] **Step 5: Manual smoke test — LinkedIn Post**
- Open a post with substantial content
- Open AI panel (✨ AI button)
- Go to Distribute tab
- Click "Generate" under "LinkedIn Post"
- Confirm output is plain text (no # headers, uses • bullets, has hashtag line)
- Confirm char count badge shows color correctly
- Click Copy → paste into a text editor and verify format

- [ ] **Step 6: Manual smoke test — Medium Article**
- In Distribute tab, click "Generate" under "Medium Article"
- Confirm output has `# Title`, `*italic subtitle*`, `## Section` headings, `>` pull quote, and `**Tags:**` line
- Click "Copy Markdown"

- [ ] **Step 7: Commit with final verification note**

```bash
git add -A
git commit -m "feat(blog): AI generate + improve + LinkedIn/Medium export — complete"
```

---

## Self-Review

**Spec coverage:**
- ✅ AI Draft bar above textarea with Write + Improve buttons (Task 3)
- ✅ Streaming via `llm-stream` event with call_id filtering (Tasks 1 + 3)
- ✅ Cancel restores original content (Task 3 `cancelAiDraft`)
- ✅ Auto-save generated content to `draft_content` (Task 3 `done` handler)
- ✅ `blog_llm_generate` blank canvas with outline + tone (Task 1)
- ✅ `blog_llm_improve` existing content rewrite (Task 1)
- ✅ `linkedin_post` arm in `blog_llm_adapt` (Task 1)
- ✅ Enhanced `medium` arm with subtitle + pull quote + tags (Task 1)
- ✅ LinkedIn Post UI with char count badge + copy (Task 4)
- ✅ Medium Article UI with monospace preview + copy (Task 4)
- ✅ Registered in `lib.rs` (Task 2)
- ✅ Disabled when no LLM: `✨ Improve` disabled when no content; `✨ Write` requires a title — both show natural disabled state. The panel-level `noLlm()` check in LlmAssistantPanel covers LinkedIn/Medium sections via `NoEndpointBanner`.

**Placeholder scan:** None found. All code blocks are complete.

**Type consistency:**
- `callId` parameter: Tauri 2 converts camelCase to snake_case → command receives `call_id`. All invocations use `callId` in JS which maps to `call_id` in Rust. ✅
- `LlmStreamEvent` type in Blog.tsx uses `call_id` (snake_case) matching the Tauri payload. ✅
- `blog_llm_adapt` returns `Option<BlogVariant>` → JS receives `{ content: string } | null` — the `BlogVariant` struct has a `content` field. ✅
