import { Component, createSignal, createMemo, For, Show, Switch, Match, onMount, onCleanup } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import ImportTab from './blog/ImportTab';
import AssetsTab from './blog/AssetsTab';
import PublishTab from './blog/PublishTab';
import PlatformsTab from './blog/PlatformsTab';
import PreviewPane from './blog/PreviewPane';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type TabId = 'posts' | 'editor' | 'seo' | 'import' | 'assets' | 'publish' | 'platforms';

interface BlogPost {
  id: string;
  title: string;
  slug: string;
  content: string | null;
  excerpt: string | null;
  status: string;
  author: string | null;
  tags: string | null;
  seo_score: number | null;
  word_count: number | null;
  reading_time: number | null;
  created_at: string;
  updated_at: string;
  published_at: string | null;
}

interface SeoAnalysis {
  score: number;
  title_length: number;
  keyword_density: number;
  heading_structure: boolean;
  word_count: number;
  suggestions: string[];
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatDate(iso: string): string {
  if (!iso) return '';
  const d = new Date(iso);
  return d.toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });
}

function statusColor(status: string): string {
  switch (status) {
    case 'draft': return 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300';
    case 'review': return 'bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-300';
    case 'published': return 'bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-300';
    case 'archived': return 'bg-slate-100 text-slate-600 dark:bg-slate-700 dark:text-slate-400';
    default: return 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300';
  }
}

function scoreColor(score: number): string {
  if (score >= 80) return '#22c55e';
  if (score >= 60) return '#eab308';
  if (score >= 40) return '#f97316';
  return '#ef4444';
}

function wordCount(text: string): number {
  return text.trim() ? text.trim().split(/\s+/).length : 0;
}

function readingTime(text: string): number {
  const wc = wordCount(text);
  if (wc === 0) return 0;
  return Math.max(1, Math.ceil(wc / 200));
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

const Blog: Component = () => {
  const [tab, setTab] = createSignal<TabId>('posts');
  const [posts, setPosts] = createSignal<BlogPost[]>([]);
  const [statusFilter, setStatusFilter] = createSignal<string | null>(null);
  const [loading, setLoading] = createSignal(false);

  // Editor state
  const [editingId, setEditingId] = createSignal<string | null>(null);
  const [edTitle, setEdTitle] = createSignal('');
  const [edContent, setEdContent] = createSignal('');
  const [edStatus, setEdStatus] = createSignal('draft');
  const [edTags, setEdTags] = createSignal('');
  const [edAuthor, setEdAuthor] = createSignal('');
  const [edSeoResult, setEdSeoResult] = createSignal<SeoAnalysis | null>(null);
  const [saving, setSaving] = createSignal(false);

  // SEO tool state
  const [seoTitle, setSeoTitle] = createSignal('');
  const [seoContent, setSeoContent] = createSignal('');
  const [seoKeywords, setSeoKeywords] = createSignal('');
  const [seoResult, setSeoResult] = createSignal<SeoAnalysis | null>(null);
  const [analyzing, setAnalyzing] = createSignal(false);

  // Split-view state
  type ViewMode = 'editor' | 'split' | 'preview';
  const [viewMode, setViewMode] = createSignal<ViewMode>('split');
  const [previewHtml, setPreviewHtml] = createSignal('');
  const [renderingPreview, setRenderingPreview] = createSignal(false);

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

  onCleanup(() => { if (previewDebounce) clearTimeout(previewDebounce); });

  let autoSaveDebounce: ReturnType<typeof setTimeout> | undefined;
  const [autoSaveStatus, setAutoSaveStatus] = createSignal<'saved' | 'saving' | 'unsaved' | 'idle'>('idle');

  const triggerAutoSave = (postId: string, content: string) => {
    setAutoSaveStatus('unsaved');
    if (autoSaveDebounce) clearTimeout(autoSaveDebounce);
    autoSaveDebounce = setTimeout(async () => {
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

  // Derived
  const edWordCount = createMemo(() => wordCount(edContent()));
  const edReadingTime = createMemo(() => readingTime(edContent()));
  const edSlug = createMemo(() => {
    const t = edTitle();
    return t
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-|-$/g, '');
  });

  // ---------------------------------------------------------------------------
  // Data loading
  // ---------------------------------------------------------------------------

  const loadPosts = async () => {
    setLoading(true);
    try {
      const result = await invoke<BlogPost[]>('blog_list_posts', {
        status: statusFilter(),
      });
      setPosts(result);
    } catch (e) {
      console.error('Failed to load posts:', e);
    } finally {
      setLoading(false);
    }
  };

  onMount(loadPosts);

  // ---------------------------------------------------------------------------
  // Editor actions
  // ---------------------------------------------------------------------------

  const openNewPost = () => {
    setEditingId(null);
    setEdTitle('');
    setEdContent('');
    setEdStatus('draft');
    setEdTags('');
    setEdAuthor('');
    setEdSeoResult(null);
    setTab('editor');
  };

  const openPost = async (post: BlogPost) => {
    try {
      const full = await invoke<BlogPost>('blog_get_post', { postId: post.id });
      setEditingId(full.id);
      setEdTitle(full.title);
      setEdContent(full.content || '');
      renderPreview(full.content || '');
      setEdStatus(full.status);
      setEdTags(full.tags || '');
      setEdAuthor(full.author || '');
      setEdSeoResult(null);
      setTab('editor');
    } catch (e) {
      console.error('Failed to load post:', e);
    }
  };

  const savePost = async () => {
    setSaving(true);
    try {
      const id = editingId();
      if (id) {
        await invoke('blog_update_post', {
          postId: id,
          title: edTitle(),
          content: edContent(),
          status: edStatus(),
          tags: edTags() || null,
        });
      } else {
        const created = await invoke<BlogPost>('blog_create_post', {
          title: edTitle(),
          content: edContent(),
          author: edAuthor() || null,
        });
        setEditingId(created.id);
        // Apply status/tags if not default
        if (edStatus() !== 'draft' || edTags()) {
          await invoke('blog_update_post', {
            postId: created.id,
            status: edStatus(),
            tags: edTags() || null,
          });
        }
      }
      await loadPosts();
    } catch (e) {
      console.error('Failed to save post:', e);
    } finally {
      setSaving(false);
    }
  };

  const deletePost = async (id: string) => {
    try {
      await invoke('blog_delete_post', { postId: id });
      await loadPosts();
      if (editingId() === id) {
        openNewPost();
      }
    } catch (e) {
      console.error('Failed to delete post:', e);
    }
  };

  const analyzeEditorSeo = async () => {
    try {
      const keywords = edTags()
        .split(',')
        .map((k) => k.trim())
        .filter(Boolean);
      const result = await invoke<SeoAnalysis>('blog_analyze_seo', {
        title: edTitle(),
        content: edContent(),
        keywords,
      });
      setEdSeoResult(result);
    } catch (e) {
      console.error('SEO analysis failed:', e);
    }
  };

  // ---------------------------------------------------------------------------
  // SEO tool actions
  // ---------------------------------------------------------------------------

  const runSeoAnalysis = async () => {
    setAnalyzing(true);
    try {
      const keywords = seoKeywords()
        .split(',')
        .map((k) => k.trim())
        .filter(Boolean);
      const result = await invoke<SeoAnalysis>('blog_analyze_seo', {
        title: seoTitle(),
        content: seoContent(),
        keywords,
      });
      setSeoResult(result);
    } catch (e) {
      console.error('SEO analysis failed:', e);
    } finally {
      setAnalyzing(false);
    }
  };

  // ---------------------------------------------------------------------------
  // Tab bar
  // ---------------------------------------------------------------------------

  const tabs: { id: TabId; label: string }[] = [
    { id: 'posts', label: 'Posts' },
    { id: 'editor', label: 'Editor' },
    { id: 'seo', label: 'SEO Tools' },
    { id: 'publish', label: 'Publish' },
    { id: 'platforms', label: 'Platforms' },
    { id: 'assets', label: 'Assets' },
  ];

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  return (
    <div class="h-full flex flex-col">
      {/* Header */}
      <div class="px-8 pt-6 pb-0">
        <div class="flex items-center justify-between mb-4">
          <div>
            <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Blog Engine</h1>
            <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
              Write, manage, and optimize your blog content
            </p>
          </div>
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
        </div>

        {/* Tabs */}
        <div class="flex gap-1 border-b border-gray-200 dark:border-gray-700">
          <For each={tabs}>
            {(t) => (
              <button
                onClick={() => setTab(t.id)}
                class="px-4 py-2.5 text-sm font-medium transition-colors relative"
                classList={{
                  'text-sky-600 dark:text-sky-400': tab() === t.id,
                  'text-gray-500 dark:text-gray-400 hover:text-gray-700 dark:hover:text-gray-300': tab() !== t.id,
                }}
              >
                {t.label}
                <Show when={tab() === t.id}>
                  <div class="absolute bottom-0 left-0 right-0 h-0.5 bg-sky-500 rounded-t" />
                </Show>
              </button>
            )}
          </For>
        </div>
      </div>

      {/* Content */}
      <div class="flex-1 overflow-y-auto">
        <Switch>
          {/* ==================== POSTS TAB ==================== */}
          <Match when={tab() === 'posts'}>
            <div class="px-8 py-6">
              {/* Status filter */}
              <div class="flex gap-2 mb-6">
                {[
                  { label: 'All', value: null },
                  { label: 'Draft', value: 'draft' },
                  { label: 'Review', value: 'review' },
                  { label: 'Published', value: 'published' },
                ].map((f) => (
                  <button
                    onClick={() => {
                      setStatusFilter(f.value);
                      loadPosts();
                    }}
                    class="px-3 py-1.5 rounded-lg text-xs font-medium transition-colors"
                    classList={{
                      'bg-sky-500 text-white': statusFilter() === f.value,
                      'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700':
                        statusFilter() !== f.value,
                    }}
                  >
                    {f.label}
                  </button>
                ))}
              </div>

              <Show when={loading()}>
                <div class="text-center py-12 text-gray-400">Loading...</div>
              </Show>

              <Show when={!loading() && posts().length === 0}>
                <div class="text-center py-16">
                  <div class="text-4xl mb-3 text-gray-300 dark:text-gray-600">
                    <svg class="w-12 h-12 mx-auto" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
                    </svg>
                  </div>
                  <p class="text-gray-500 dark:text-gray-400 font-medium">No posts yet</p>
                  <p class="text-sm text-gray-400 dark:text-gray-500 mt-1">
                    Create your first blog post to get started
                  </p>
                  <button
                    onClick={openNewPost}
                    class="mt-4 px-4 py-2 rounded-lg text-sm font-medium text-white bg-sky-500 hover:bg-sky-600 transition-colors"
                  >
                    New Post
                  </button>
                </div>
              </Show>

              <Show when={!loading() && posts().length > 0}>
                <div class="space-y-3">
                  <For each={posts()}>
                    {(post) => (
                      <div
                        class="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-5 hover:border-sky-300 dark:hover:border-sky-700 transition-colors cursor-pointer group"
                        onClick={() => openPost(post)}
                      >
                        <div class="flex items-start justify-between gap-4">
                          <div class="flex-1 min-w-0">
                            <div class="flex items-center gap-3 mb-1.5">
                              <h3 class="font-semibold text-gray-900 dark:text-white truncate">
                                {post.title}
                              </h3>
                              <span class={`px-2 py-0.5 rounded-full text-[11px] font-medium uppercase tracking-wide ${statusColor(post.status)}`}>
                                {post.status}
                              </span>
                            </div>
                            <div class="flex items-center gap-4 text-xs text-gray-400 dark:text-gray-500">
                              <Show when={post.word_count}>
                                <span>{post.word_count} words</span>
                              </Show>
                              <Show when={post.reading_time}>
                                <span>{post.reading_time} min read</span>
                              </Show>
                              <span>{formatDate(post.updated_at)}</span>
                              <Show when={post.author}>
                                <span>by {post.author}</span>
                              </Show>
                            </div>
                          </div>
                          <button
                            onClick={(e) => {
                              e.stopPropagation();
                              deletePost(post.id);
                            }}
                            class="p-1.5 rounded-lg text-gray-300 dark:text-gray-600 hover:text-red-500 dark:hover:text-red-400 hover:bg-red-50 dark:hover:bg-red-900/20 opacity-0 group-hover:opacity-100 transition-all"
                            title="Delete post"
                          >
                            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                            </svg>
                          </button>
                        </div>
                      </div>
                    )}
                  </For>
                </div>
              </Show>
            </div>
          </Match>

          {/* ==================== EDITOR TAB ==================== */}
          <Match when={tab() === 'editor'}>
            <div class="flex flex-col h-full">
              {/* View mode toggle */}
              <div class="flex items-center gap-1 px-6 pt-3 pb-2 border-b border-gray-100 dark:border-gray-700 bg-white dark:bg-gray-800 shrink-0">
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
              <div class="flex flex-1 overflow-hidden min-h-0">
              {/* Main editor area — shown in editor and split modes */}
              <Show when={viewMode() !== 'preview'}>
                <div class={`flex flex-col p-6 overflow-y-auto ${viewMode() === 'split' ? 'w-1/2 border-r border-gray-200 dark:border-gray-700' : 'flex-1'}`}>
                {/* Title */}
                <input
                  type="text"
                  placeholder="Post title..."
                  value={edTitle()}
                  onInput={(e) => setEdTitle(e.currentTarget.value)}
                  class="w-full text-2xl font-bold bg-transparent border-none outline-none placeholder-gray-300 dark:placeholder-gray-600 text-gray-900 dark:text-white mb-4"
                />

                {/* Content */}
                <textarea
                  placeholder="Start writing your post in markdown..."
                  value={edContent()}
                  onInput={(e) => {
                    const val = e.currentTarget.value;
                    setEdContent(val);
                    if (viewMode() !== 'editor') renderPreview(val);
                    const id = editingId();
                    if (id) triggerAutoSave(id, val);
                  }}
                  class="w-full flex-1 min-h-[400px] bg-transparent border border-gray-200 dark:border-gray-700 rounded-lg p-4 outline-none resize-none text-gray-800 dark:text-gray-200 placeholder-gray-300 dark:placeholder-gray-600 focus:border-sky-300 dark:focus:border-sky-700 transition-colors"
                  style={{ 'font-family': 'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace', 'font-size': '14px', 'line-height': '1.7' }}
                />

                {/* Bottom bar */}
                <div class="flex items-center justify-between mt-4 text-xs text-gray-400 dark:text-gray-500">
                  <div class="flex gap-4 items-center">
                    <span>{edWordCount()} words</span>
                    <span>{edReadingTime()} min read</span>
                    <Show when={autoSaveStatus() === 'unsaved'}>
                      <span class="text-amber-500">● Unsaved changes</span>
                    </Show>
                    <Show when={autoSaveStatus() === 'saving'}>
                      <span class="animate-pulse text-gray-400">Saving…</span>
                    </Show>
                    <Show when={autoSaveStatus() === 'saved'}>
                      <span class="text-emerald-500">✓ Draft saved</span>
                    </Show>
                  </div>
                  <Show when={edSeoResult()}>
                    <div class="flex items-center gap-2">
                      <span>SEO Score:</span>
                      <span
                        class="font-bold"
                        style={{ color: scoreColor(edSeoResult()!.score) }}
                      >
                        {edSeoResult()!.score}/100
                      </span>
                    </div>
                  </Show>
                </div>
              </div>
              </Show>

              {/* Preview pane — shown in split and preview modes */}
              <Show when={viewMode() !== 'editor'}>
                <div class={viewMode() === 'split' ? 'w-1/2 overflow-y-auto' : 'flex-1 overflow-y-auto'}>
                  <PreviewPane html={previewHtml()} />
                </div>
              </Show>

              {/* Side panel */}
              <div class="w-72 border-l border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 p-5 overflow-y-auto flex flex-col gap-5">
                {/* Status */}
                <div>
                  <label class="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1.5">Status</label>
                  <select
                    value={edStatus()}
                    onChange={(e) => setEdStatus(e.currentTarget.value)}
                    class="w-full px-3 py-2 rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900 text-sm text-gray-800 dark:text-gray-200 outline-none focus:border-sky-300"
                  >
                    <option value="draft">Draft</option>
                    <option value="review">Review</option>
                    <option value="published">Published</option>
                    <option value="archived">Archived</option>
                  </select>
                </div>

                {/* Author */}
                <div>
                  <label class="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1.5">Author</label>
                  <input
                    type="text"
                    placeholder="Author name"
                    value={edAuthor()}
                    onInput={(e) => setEdAuthor(e.currentTarget.value)}
                    class="w-full px-3 py-2 rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900 text-sm text-gray-800 dark:text-gray-200 outline-none focus:border-sky-300"
                  />
                </div>

                {/* Tags */}
                <div>
                  <label class="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1.5">Tags</label>
                  <input
                    type="text"
                    placeholder="rust, programming, web"
                    value={edTags()}
                    onInput={(e) => setEdTags(e.currentTarget.value)}
                    class="w-full px-3 py-2 rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900 text-sm text-gray-800 dark:text-gray-200 outline-none focus:border-sky-300"
                  />
                  <p class="text-[10px] text-gray-400 mt-1">Comma-separated</p>
                </div>

                {/* Slug preview */}
                <div>
                  <label class="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1.5">Slug</label>
                  <p class="text-sm text-gray-600 dark:text-gray-300 font-mono bg-gray-50 dark:bg-gray-900 rounded-lg px-3 py-2 break-all">
                    {edSlug() || 'post-slug'}
                  </p>
                </div>

                {/* Stats */}
                <div class="grid grid-cols-2 gap-3">
                  <div class="bg-gray-50 dark:bg-gray-900 rounded-lg p-3 text-center">
                    <div class="text-lg font-bold text-gray-900 dark:text-white">{edWordCount()}</div>
                    <div class="text-[10px] text-gray-400 uppercase tracking-wide">Words</div>
                  </div>
                  <div class="bg-gray-50 dark:bg-gray-900 rounded-lg p-3 text-center">
                    <div class="text-lg font-bold text-gray-900 dark:text-white">{edReadingTime()}</div>
                    <div class="text-[10px] text-gray-400 uppercase tracking-wide">Min read</div>
                  </div>
                </div>

                {/* Actions */}
                <div class="flex flex-col gap-2 mt-auto pt-4 border-t border-gray-100 dark:border-gray-700">
                  <button
                    onClick={savePost}
                    disabled={saving() || !edTitle().trim() || !edContent().trim()}
                    class="w-full px-4 py-2.5 rounded-lg text-sm font-medium text-white bg-sky-500 hover:bg-sky-600 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                  >
                    {saving() ? 'Saving...' : editingId() ? 'Update Post' : 'Create Post'}
                  </button>
                  <button
                    onClick={analyzeEditorSeo}
                    disabled={!edContent().trim()}
                    class="w-full px-4 py-2 rounded-lg text-sm font-medium text-sky-600 dark:text-sky-400 bg-sky-50 dark:bg-sky-900/20 hover:bg-sky-100 dark:hover:bg-sky-900/40 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                  >
                    Analyze SEO
                  </button>
                </div>

                {/* Inline SEO result */}
                <Show when={edSeoResult()}>
                  <div class="bg-gray-50 dark:bg-gray-900 rounded-lg p-4">
                    <div class="flex items-center gap-2 mb-3">
                      <div
                        class="w-10 h-10 rounded-full flex items-center justify-center text-white font-bold text-sm"
                        style={{ background: scoreColor(edSeoResult()!.score) }}
                      >
                        {edSeoResult()!.score}
                      </div>
                      <span class="text-xs text-gray-500">SEO Score</span>
                    </div>
                    <Show when={edSeoResult()!.suggestions.length > 0}>
                      <ul class="space-y-1">
                        <For each={edSeoResult()!.suggestions}>
                          {(s) => (
                            <li class="text-[11px] text-gray-500 dark:text-gray-400 flex items-start gap-1.5">
                              <span class="text-amber-500 mt-0.5 shrink-0">!</span>
                              {s}
                            </li>
                          )}
                        </For>
                      </ul>
                    </Show>
                  </div>
                </Show>
              </div>
              </div>
            </div>
          </Match>

          {/* ==================== SEO TOOLS TAB ==================== */}
          <Match when={tab() === 'seo'}>
            <div class="px-8 py-6 max-w-4xl">
              <div class="bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
                <h2 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">SEO Analyzer</h2>
                <p class="text-sm text-gray-500 dark:text-gray-400 mb-6">
                  Analyze your content against SEO best practices. Provide a title, content, and target keywords.
                </p>

                <div class="space-y-4">
                  {/* Title */}
                  <div>
                    <label class="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1.5">Title</label>
                    <input
                      type="text"
                      placeholder="Blog post title"
                      value={seoTitle()}
                      onInput={(e) => setSeoTitle(e.currentTarget.value)}
                      class="w-full px-3 py-2 rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900 text-sm text-gray-800 dark:text-gray-200 outline-none focus:border-sky-300"
                    />
                  </div>

                  {/* Content */}
                  <div>
                    <label class="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1.5">Content</label>
                    <textarea
                      placeholder="Paste your blog content here..."
                      value={seoContent()}
                      onInput={(e) => setSeoContent(e.currentTarget.value)}
                      rows={10}
                      class="w-full px-3 py-2 rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900 text-sm text-gray-800 dark:text-gray-200 outline-none focus:border-sky-300 resize-none"
                      style={{ 'font-family': 'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace' }}
                    />
                  </div>

                  {/* Keywords */}
                  <div>
                    <label class="block text-xs font-medium text-gray-500 dark:text-gray-400 mb-1.5">
                      Target Keywords
                    </label>
                    <input
                      type="text"
                      placeholder="rust, web development, programming"
                      value={seoKeywords()}
                      onInput={(e) => setSeoKeywords(e.currentTarget.value)}
                      class="w-full px-3 py-2 rounded-lg border border-gray-200 dark:border-gray-700 bg-gray-50 dark:bg-gray-900 text-sm text-gray-800 dark:text-gray-200 outline-none focus:border-sky-300"
                    />
                    <p class="text-[10px] text-gray-400 mt-1">Comma-separated keywords</p>
                  </div>

                  <button
                    onClick={runSeoAnalysis}
                    disabled={analyzing() || !seoContent().trim()}
                    class="px-5 py-2.5 rounded-lg text-sm font-medium text-white bg-sky-500 hover:bg-sky-600 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                  >
                    {analyzing() ? 'Analyzing...' : 'Analyze'}
                  </button>
                </div>
              </div>

              {/* Results */}
              <Show when={seoResult()}>
                <div class="mt-6 bg-white dark:bg-gray-800 rounded-xl border border-gray-200 dark:border-gray-700 p-6">
                  <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-5">Analysis Results</h3>

                  {/* Score gauge */}
                  <div class="flex items-center gap-6 mb-6 pb-6 border-b border-gray-100 dark:border-gray-700">
                    <div class="relative w-24 h-24">
                      <svg class="w-24 h-24 -rotate-90" viewBox="0 0 100 100">
                        <circle cx="50" cy="50" r="42" fill="none" stroke-width="8" class="stroke-gray-100 dark:stroke-gray-700" />
                        <circle
                          cx="50" cy="50" r="42" fill="none" stroke-width="8"
                          stroke={scoreColor(seoResult()!.score)}
                          stroke-dasharray={`${(seoResult()!.score / 100) * 264} 264`}
                          stroke-linecap="round"
                        />
                      </svg>
                      <div class="absolute inset-0 flex items-center justify-center">
                        <span class="text-2xl font-bold text-gray-900 dark:text-white">{seoResult()!.score}</span>
                      </div>
                    </div>
                    <div>
                      <div class="text-sm font-medium text-gray-700 dark:text-gray-300">
                        {seoResult()!.score >= 80 ? 'Excellent' :
                         seoResult()!.score >= 60 ? 'Good' :
                         seoResult()!.score >= 40 ? 'Needs Work' : 'Poor'}
                      </div>
                      <div class="text-xs text-gray-400 mt-1">Overall SEO Score</div>
                    </div>
                  </div>

                  {/* Checklist */}
                  <div class="space-y-3 mb-6">
                    {/* Title length */}
                    <div class="flex items-center gap-3">
                      <div class={`w-5 h-5 rounded-full flex items-center justify-center text-white text-xs ${seoResult()!.title_length >= 50 && seoResult()!.title_length <= 60 ? 'bg-emerald-500' : seoResult()!.title_length >= 40 && seoResult()!.title_length <= 70 ? 'bg-amber-500' : 'bg-red-500'}`}>
                        {seoResult()!.title_length >= 50 && seoResult()!.title_length <= 60 ? '✓' : '!'}
                      </div>
                      <div class="flex-1">
                        <div class="text-sm text-gray-700 dark:text-gray-300">Title Length</div>
                        <div class="text-xs text-gray-400">{seoResult()!.title_length} characters (ideal: 50-60)</div>
                      </div>
                    </div>

                    {/* Keyword density */}
                    <div class="flex items-center gap-3">
                      <div class={`w-5 h-5 rounded-full flex items-center justify-center text-white text-xs ${seoResult()!.keyword_density >= 1 && seoResult()!.keyword_density <= 3 ? 'bg-emerald-500' : seoResult()!.keyword_density > 0 ? 'bg-amber-500' : 'bg-red-500'}`}>
                        {seoResult()!.keyword_density >= 1 && seoResult()!.keyword_density <= 3 ? '✓' : '!'}
                      </div>
                      <div class="flex-1">
                        <div class="text-sm text-gray-700 dark:text-gray-300">Keyword Density</div>
                        <div class="text-xs text-gray-400">{seoResult()!.keyword_density.toFixed(1)}% (ideal: 1-3%)</div>
                      </div>
                    </div>

                    {/* Heading structure */}
                    <div class="flex items-center gap-3">
                      <div class={`w-5 h-5 rounded-full flex items-center justify-center text-white text-xs ${seoResult()!.heading_structure ? 'bg-emerald-500' : 'bg-red-500'}`}>
                        {seoResult()!.heading_structure ? '✓' : '!'}
                      </div>
                      <div class="flex-1">
                        <div class="text-sm text-gray-700 dark:text-gray-300">Heading Structure</div>
                        <div class="text-xs text-gray-400">
                          {seoResult()!.heading_structure ? 'Has proper markdown headings' : 'Missing ## headings'}
                        </div>
                      </div>
                    </div>

                    {/* Content length */}
                    <div class="flex items-center gap-3">
                      <div class={`w-5 h-5 rounded-full flex items-center justify-center text-white text-xs ${seoResult()!.word_count >= 300 ? 'bg-emerald-500' : seoResult()!.word_count >= 150 ? 'bg-amber-500' : 'bg-red-500'}`}>
                        {seoResult()!.word_count >= 300 ? '✓' : '!'}
                      </div>
                      <div class="flex-1">
                        <div class="text-sm text-gray-700 dark:text-gray-300">Content Length</div>
                        <div class="text-xs text-gray-400">{seoResult()!.word_count} words (recommended: 300+)</div>
                      </div>
                    </div>
                  </div>

                  {/* Suggestions */}
                  <Show when={seoResult()!.suggestions.length > 0}>
                    <div>
                      <h4 class="text-sm font-medium text-gray-700 dark:text-gray-300 mb-3">Improvement Suggestions</h4>
                      <ul class="space-y-2">
                        <For each={seoResult()!.suggestions}>
                          {(suggestion) => (
                            <li class="flex items-start gap-2 text-sm text-gray-600 dark:text-gray-400">
                              <svg class="w-4 h-4 text-amber-500 mt-0.5 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5z" />
                              </svg>
                              {suggestion}
                            </li>
                          )}
                        </For>
                      </ul>
                    </div>
                  </Show>

                  <Show when={seoResult()!.suggestions.length === 0}>
                    <div class="flex items-center gap-2 text-sm text-emerald-600 dark:text-emerald-400">
                      <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
                      </svg>
                      All SEO checks passed. Great job!
                    </div>
                  </Show>
                </div>
              </Show>
            </div>
          </Match>

          {/* ==================== IMPORT TAB ==================== */}
          <Match when={tab() === 'import'}>
            <ImportTab onDone={() => { setTab('posts'); loadPosts(); }} />
          </Match>

          {/* ==================== PUBLISH TAB ==================== */}
          <Match when={tab() === 'publish'}>
            <PublishTab />
          </Match>

          {/* ==================== PLATFORMS TAB ==================== */}
          <Match when={tab() === 'platforms'}>
            <PlatformsTab />
          </Match>

          {/* ==================== ASSETS TAB ==================== */}
          <Match when={tab() === 'assets'}>
            <AssetsTab />
          </Match>
        </Switch>
      </div>
    </div>
  );
};

export default Blog;
