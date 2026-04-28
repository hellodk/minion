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

interface LlmEndpoint {
  id: string;
  name: string;
  default_model: string | null;
}

type PanelTab = 'lint' | 'writing' | 'seo' | 'distribute';

// ── Sub-components ────────────────────────────────────────────────────────────

const Spinner: Component = () => (
  <svg class="animate-spin h-3 w-3 text-current" viewBox="0 0 24 24" fill="none">
    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z" />
  </svg>
);

const ActionBtn: Component<{ label: string; loading: boolean; onClick: () => void; color?: string }> = (p) => (
  <button
    onClick={p.onClick}
    disabled={p.loading}
    class={`flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-lg border transition-colors cursor-pointer
            disabled:opacity-60 disabled:cursor-not-allowed
            ${(p.color ?? 'sky') === 'sky'
              ? 'bg-sky-50 dark:bg-sky-900/20 border-sky-200 dark:border-sky-800 text-sky-700 dark:text-sky-300 hover:bg-sky-100'
              : 'bg-gray-50 dark:bg-gray-800 border-gray-200 dark:border-gray-700 text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700'
            }`}
  >
    <Show when={p.loading} fallback={p.label}>
      <Spinner /><span>Working…</span>
    </Show>
  </button>
);

const ResultBox: Component<{ content: string; onCopy?: () => void; onApply?: () => void }> = (p) => (
  <div class="relative mt-2 p-3 bg-gray-50 dark:bg-gray-900 border border-gray-200 dark:border-gray-700 rounded-lg text-xs text-gray-700 dark:text-gray-300 whitespace-pre-wrap leading-relaxed max-h-48 overflow-y-auto">
    {p.content}
    <div class="absolute top-2 right-2 flex gap-1">
      <Show when={p.onApply}>
        <button onClick={p.onApply} class="px-2 py-0.5 text-[10px] bg-sky-500 text-white rounded hover:bg-sky-600">Append</button>
      </Show>
      <Show when={p.onCopy}>
        <button onClick={p.onCopy} class="px-2 py-0.5 text-[10px] bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-600 rounded text-gray-500 hover:text-gray-800 dark:hover:text-gray-200">Copy</button>
      </Show>
    </div>
  </div>
);

const NoEndpointBanner: Component = () => (
  <div class="p-3 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded-lg">
    <p class="text-xs font-medium text-amber-700 dark:text-amber-300 mb-1">No AI endpoint configured</p>
    <p class="text-[11px] text-amber-600 dark:text-amber-400">
      Go to <strong>Settings → AI Endpoints</strong> and add an Ollama or OpenAI-compatible endpoint to enable these features.
    </p>
  </div>
);

const ErrorNote: Component<{ msg: string }> = (p) => (
  <p class="text-[11px] text-red-500 dark:text-red-400 mt-1">⚠ {p.msg}</p>
);

const EmptyNote: Component<{ tried: boolean }> = (p) => (
  <Show when={p.tried}>
    <p class="text-[11px] text-gray-400 mt-1 italic">No response from AI. The model may need more context, or try again.</p>
  </Show>
);

// ── Main panel ────────────────────────────────────────────────────────────────

const LlmAssistantPanel: Component<{
  postId: string | null;
  onClose: () => void;
  onApply?: (type: 'title' | 'append', value: string) => void;
}> = (props) => {
  const [tab, setTab] = createSignal<PanelTab>('lint');
  const [hasEndpoint, setHasEndpoint] = createSignal<boolean | null>(null); // null = checking
  const [modelLabel, setModelLabel] = createSignal<string | null>(null);

  // ── Lint ──────────────────────────────────────────────────────────────────
  const [lintIssues, setLintIssues] = createSignal<LintIssue[]>([]);
  const [lintLoading, setLintLoading] = createSignal(false);
  const [lintError, setLintError] = createSignal('');
  const [lintRan, setLintRan] = createSignal(false);

  const runLint = async () => {
    if (!props.postId) return;
    setLintLoading(true); setLintError('');
    try {
      const issues = await invoke<LintIssue[]>('blog_lint', { postId: props.postId });
      setLintIssues(issues);
      setLintRan(true);
    } catch (e) {
      setLintError(String(e));
      setLintIssues([]);
    } finally { setLintLoading(false); }
  };

  // ── Writing ───────────────────────────────────────────────────────────────
  const [titles, setTitles] = createSignal<TitleSuggestion[]>([]);
  const [titlesLoading, setTitlesLoading] = createSignal(false);
  const [titlesError, setTitlesError] = createSignal('');
  const [titlesTried, setTitlesTried] = createSignal(false);

  const [hook, setHook] = createSignal<string[]>([]);
  const [hookLoading, setHookLoading] = createSignal(false);
  const [hookError, setHookError] = createSignal('');
  const [hookTried, setHookTried] = createSignal(false);

  const [conclusion, setConclusion] = createSignal('');
  const [conclusionLoading, setConclusionLoading] = createSignal(false);
  const [conclusionError, setConclusionError] = createSignal('');
  const [conclusionTried, setConclusionTried] = createSignal(false);

  const [grammar, setGrammar] = createSignal<string[]>([]);
  const [grammarLoading, setGrammarLoading] = createSignal(false);
  const [grammarError, setGrammarError] = createSignal('');
  const [grammarTried, setGrammarTried] = createSignal(false);

  const runTitles = async () => {
    if (!props.postId) return;
    setTitlesLoading(true); setTitlesError(''); setTitlesTried(false);
    try {
      const res = await invoke<TitleSuggestion[] | null>('blog_llm_titles', { postId: props.postId });
      setTitles(res ?? []);
    } catch (e) { setTitlesError(String(e)); setTitles([]); }
    finally { setTitlesLoading(false); setTitlesTried(true); }
  };

  const runHook = async () => {
    if (!props.postId) return;
    setHookLoading(true); setHookError(''); setHookTried(false);
    try {
      const res = await invoke<string[] | null>('blog_llm_hook', { postId: props.postId });
      setHook(res ?? []);
    } catch (e) { setHookError(String(e)); setHook([]); }
    finally { setHookLoading(false); setHookTried(true); }
  };

  const runConclusion = async () => {
    if (!props.postId) return;
    setConclusionLoading(true); setConclusionError(''); setConclusionTried(false);
    try {
      const res = await invoke<string | null>('blog_llm_conclusion', { postId: props.postId });
      setConclusion(res ?? '');
    } catch (e) { setConclusionError(String(e)); setConclusion(''); }
    finally { setConclusionLoading(false); setConclusionTried(true); }
  };

  const runGrammar = async () => {
    if (!props.postId) return;
    setGrammarLoading(true); setGrammarError(''); setGrammarTried(false);
    try {
      const res = await invoke<string[] | null>('blog_llm_grammar', { postId: props.postId });
      setGrammar(res ?? []);
    } catch (e) { setGrammarError(String(e)); setGrammar([]); }
    finally { setGrammarLoading(false); setGrammarTried(true); }
  };

  // ── SEO ───────────────────────────────────────────────────────────────────
  const [metaDesc, setMetaDesc] = createSignal('');
  const [metaLoading, setMetaLoading] = createSignal(false);
  const [metaError, setMetaError] = createSignal('');
  const [metaTried, setMetaTried] = createSignal(false);
  const [metaSaved, setMetaSaved] = createSignal(false);

  const [tags, setTags] = createSignal<string[]>([]);
  const [tagsLoading, setTagsLoading] = createSignal(false);
  const [tagsError, setTagsError] = createSignal('');
  const [tagsTried, setTagsTried] = createSignal(false);

  const runMetaDesc = async (save: boolean) => {
    if (!props.postId) return;
    setMetaLoading(true); setMetaError(''); setMetaTried(false); setMetaSaved(false);
    try {
      const res = await invoke<string | null>('blog_llm_meta_description', { postId: props.postId, saveToExcerpt: save });
      setMetaDesc(res ?? '');
      if (save && res) setMetaSaved(true);
    } catch (e) { setMetaError(String(e)); setMetaDesc(''); }
    finally { setMetaLoading(false); setMetaTried(true); }
  };

  const runTags = async () => {
    if (!props.postId) return;
    setTagsLoading(true); setTagsError(''); setTagsTried(false);
    try {
      const res = await invoke<string[] | null>('blog_llm_tags', { postId: props.postId });
      setTags(res ?? []);
    } catch (e) { setTagsError(String(e)); setTags([]); }
    finally { setTagsLoading(false); setTagsTried(true); }
  };

  // ── Distribute ────────────────────────────────────────────────────────────
  const [snippets, setSnippets] = createSignal<Record<string, string>>({});
  const [snippetsLoading, setSnippetsLoading] = createSignal(false);
  const [snippetsError, setSnippetsError] = createSignal('');
  const [snippetsTried, setSnippetsTried] = createSignal(false);

  const [variants, setVariants] = createSignal<BlogVariant[]>([]);
  const [adaptLoading, setAdaptLoading] = createSignal(false);
  const [adaptError, setAdaptError] = createSignal('');
  const [adaptPlatform, setAdaptPlatform] = createSignal('devto');

  const [toneLoading, setToneLoading] = createSignal(false);
  const [toneError, setToneError] = createSignal('');
  const [tonePlatform, setTonePlatform] = createSignal('balanced');

  const runSnippets = async () => {
    if (!props.postId) return;
    setSnippetsLoading(true); setSnippetsError(''); setSnippetsTried(false);
    try {
      const s = await invoke<Record<string, string> | null>('blog_llm_snippets', { postId: props.postId });
      setSnippets(s ?? {});
    } catch (e) { setSnippetsError(String(e)); setSnippets({}); }
    finally { setSnippetsLoading(false); setSnippetsTried(true); }
  };

  const loadVariants = async () => {
    if (!props.postId) return;
    try { setVariants(await invoke<BlogVariant[]>('blog_get_variants', { postId: props.postId })); }
    catch { setVariants([]); }
  };

  const runAdapt = async () => {
    if (!props.postId) return;
    setAdaptLoading(true); setAdaptError('');
    try {
      await invoke('blog_llm_adapt', { postId: props.postId, platform: adaptPlatform() });
      await loadVariants();
    } catch (e) { setAdaptError(String(e)); }
    finally { setAdaptLoading(false); }
  };

  const runTone = async () => {
    if (!props.postId) return;
    setToneLoading(true); setToneError('');
    try {
      await invoke('blog_llm_tone', { postId: props.postId, target: tonePlatform() });
      await loadVariants();
    } catch (e) { setToneError(String(e)); }
    finally { setToneLoading(false); }
  };

  const deleteVariant = async (id: string) => {
    try { await invoke('blog_delete_variant', { variantId: id }); }
    catch { /* ignore */ }
    await loadVariants();
  };

  onMount(async () => {
    // Check if any LLM endpoint is configured
    try {
      const endpoints = await invoke<LlmEndpoint[]>('llm_list_endpoints');
      setHasEndpoint(endpoints.length > 0);
      if (endpoints.length > 0) {
        const ep = endpoints[0];
        setModelLabel(ep.default_model ? `${ep.name} · ${ep.default_model}` : ep.name);
      }
    } catch { setHasEndpoint(false); }

    // Load lint (rule-based, always works)
    await runLint();

    // Load cached data
    if (props.postId) {
      try {
        const s = await invoke<Record<string, string> | null>('blog_get_snippets', { postId: props.postId });
        if (s) setSnippets(s);
      } catch { /* ignore */ }
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
  const noLlm = () => hasEndpoint() === false;

  return (
    <div class="flex flex-col h-full bg-white dark:bg-gray-800 border-l border-gray-200 dark:border-gray-700 w-80 shrink-0">
      {/* Header */}
      <div class="flex items-center justify-between px-3 py-2 border-b border-gray-200 dark:border-gray-700 shrink-0">
        <div>
          <span class="text-sm font-semibold text-gray-800 dark:text-gray-200">✨ AI Assistant</span>
          <Show when={modelLabel()}>
            <span class="ml-1.5 text-[10px] text-gray-400 dark:text-gray-500">{modelLabel()}</span>
          </Show>
        </div>
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

        {/* ── LINT TAB ──────────────────────────────────────────────────── */}
        <Show when={tab() === 'lint' && !noPost()}>
          <div class="flex items-center justify-between">
            <span class="text-xs font-semibold text-gray-600 dark:text-gray-300">Rule-Based Checks</span>
            <ActionBtn label="↻ Re-run" loading={lintLoading()} onClick={runLint} color="gray" />
          </div>
          <Show when={lintError()}>
            <ErrorNote msg={lintError()} />
          </Show>
          <Show when={lintRan() && lintIssues().length === 0 && !lintLoading() && !lintError()}>
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

        {/* ── WRITING TAB ───────────────────────────────────────────────── */}
        <Show when={tab() === 'writing' && !noPost()}>
          <Show when={noLlm()}>
            <NoEndpointBanner />
          </Show>

          {/* Title Generator */}
          <div>
            <div class="flex items-center justify-between mb-2">
              <span class="text-xs font-semibold text-gray-600 dark:text-gray-300">Title Generator</span>
              <ActionBtn label="Generate" loading={titlesLoading()} onClick={runTitles} />
            </div>
            <Show when={titlesError()}><ErrorNote msg={titlesError()} /></Show>
            <EmptyNote tried={titlesTried() && titles().length === 0 && !titlesError()} />
            <Show when={titles().length > 0}>
              <div class="space-y-2">
                <For each={titles()}>
                  {(t) => (
                    <div class="p-2 border border-gray-200 dark:border-gray-700 rounded-lg">
                      <div class="flex justify-between items-start gap-1 mb-1">
                        <span class="text-[10px] px-1 py-0.5 bg-sky-100 dark:bg-sky-900/40 text-sky-600 dark:text-sky-400 rounded">{t.style}</span>
                        <div class="flex gap-1.5">
                          <button onClick={() => navigator.clipboard.writeText(t.title)} class="text-[10px] text-gray-400 hover:text-gray-600">Copy</button>
                          <Show when={props.onApply}>
                            <button onClick={() => props.onApply!('title', t.title)} class="text-[10px] text-sky-600 hover:text-sky-800 font-medium">Apply</button>
                          </Show>
                        </div>
                      </div>
                      <p class="text-xs font-medium text-gray-800 dark:text-gray-200">{t.title}</p>
                      <p class="text-[10px] text-gray-400 mt-0.5">{t.rationale}</p>
                    </div>
                  )}
                </For>
              </div>
            </Show>
          </div>

          {/* Hook Rewriter */}
          <div class="border-t border-gray-100 dark:border-gray-700 pt-3">
            <div class="flex items-center justify-between mb-2">
              <span class="text-xs font-semibold text-gray-600 dark:text-gray-300">Hook Rewriter</span>
              <ActionBtn label="Rewrite" loading={hookLoading()} onClick={runHook} />
            </div>
            <Show when={hookError()}><ErrorNote msg={hookError()} /></Show>
            <EmptyNote tried={hookTried() && hook().length === 0 && !hookError()} />
            <Show when={hook().length > 0}>
              <div class="space-y-2">
                <For each={hook()}>
                  {(v, i) => (
                    <div class="p-2 border border-gray-200 dark:border-gray-700 rounded-lg">
                      <div class="flex justify-between mb-1">
                        <span class="text-[10px] text-gray-400">Variant {i() + 1}</span>
                        <div class="flex gap-1.5">
                          <button onClick={() => navigator.clipboard.writeText(v)} class="text-[10px] text-gray-400 hover:text-gray-600">Copy</button>
                          <Show when={props.onApply}>
                            <button onClick={() => props.onApply!('append', v)} class="text-[10px] text-sky-600 hover:text-sky-800 font-medium">Append</button>
                          </Show>
                        </div>
                      </div>
                      <p class="text-xs text-gray-700 dark:text-gray-300">{v}</p>
                    </div>
                  )}
                </For>
              </div>
            </Show>
          </div>

          {/* Conclusion + CTA */}
          <div class="border-t border-gray-100 dark:border-gray-700 pt-3">
            <div class="flex items-center justify-between mb-2">
              <span class="text-xs font-semibold text-gray-600 dark:text-gray-300">Conclusion + CTA</span>
              <ActionBtn label="Generate" loading={conclusionLoading()} onClick={runConclusion} />
            </div>
            <Show when={conclusionError()}><ErrorNote msg={conclusionError()} /></Show>
            <EmptyNote tried={conclusionTried() && !conclusion() && !conclusionError()} />
            <Show when={conclusion()}>
              <ResultBox content={conclusion()} onCopy={() => navigator.clipboard.writeText(conclusion())} onApply={props.onApply ? () => props.onApply!('append', conclusion()) : undefined} />
            </Show>
          </div>

          {/* Grammar & Style */}
          <div class="border-t border-gray-100 dark:border-gray-700 pt-3">
            <div class="flex items-center justify-between mb-2">
              <span class="text-xs font-semibold text-gray-600 dark:text-gray-300">Grammar & Style</span>
              <ActionBtn label="Analyse" loading={grammarLoading()} onClick={runGrammar} />
            </div>
            <Show when={grammarError()}><ErrorNote msg={grammarError()} /></Show>
            <EmptyNote tried={grammarTried() && grammar().length === 0 && !grammarError()} />
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

        {/* ── SEO TAB ───────────────────────────────────────────────────── */}
        <Show when={tab() === 'seo' && !noPost()}>
          <Show when={noLlm()}>
            <NoEndpointBanner />
          </Show>

          {/* Meta Description */}
          <div>
            <div class="flex items-center justify-between mb-1">
              <span class="text-xs font-semibold text-gray-600 dark:text-gray-300">Meta Description</span>
              <div class="flex gap-1">
                <ActionBtn label="Generate" loading={metaLoading()} onClick={() => runMetaDesc(false)} color="gray" />
                <ActionBtn label="Generate & Save" loading={metaLoading()} onClick={() => runMetaDesc(true)} />
              </div>
            </div>
            <p class="text-[10px] text-gray-400 mb-2">"Generate & Save" overwrites the post excerpt.</p>
            <Show when={metaError()}><ErrorNote msg={metaError()} /></Show>
            <EmptyNote tried={metaTried() && !metaDesc() && !metaError()} />
            <Show when={metaDesc()}>
              <ResultBox content={metaDesc()} onCopy={() => navigator.clipboard.writeText(metaDesc())} />
              <div class="flex items-center justify-between mt-1">
                <span class={`text-[10px] ${metaDesc().length > 160 ? 'text-red-500' : metaDesc().length >= 140 ? 'text-green-600' : 'text-gray-400'}`}>
                  {metaDesc().length} chars {metaDesc().length > 160 ? '(too long)' : metaDesc().length >= 140 ? '(ideal)' : '(short)'}
                </span>
                <Show when={metaSaved()}>
                  <span class="text-[10px] text-green-600 dark:text-green-400">✓ Saved to excerpt</span>
                </Show>
              </div>
            </Show>
          </div>

          {/* Tag Suggestions */}
          <div class="border-t border-gray-100 dark:border-gray-700 pt-3">
            <div class="flex items-center justify-between mb-2">
              <span class="text-xs font-semibold text-gray-600 dark:text-gray-300">Tag Suggestions</span>
              <ActionBtn label="Suggest" loading={tagsLoading()} onClick={runTags} />
            </div>
            <Show when={tagsError()}><ErrorNote msg={tagsError()} /></Show>
            <EmptyNote tried={tagsTried() && tags().length === 0 && !tagsError()} />
            <Show when={tags().length > 0}>
              <div class="flex flex-wrap gap-1 mt-2">
                <For each={tags()}>
                  {(tag) => (
                    <button
                      onClick={() => navigator.clipboard.writeText(tag)}
                      title="Click to copy"
                      class="px-2 py-0.5 bg-sky-50 dark:bg-sky-900/30 border border-sky-200 dark:border-sky-800 text-sky-700 dark:text-sky-300 text-[11px] rounded-full hover:bg-sky-100 cursor-copy"
                    >
                      {tag}
                    </button>
                  )}
                </For>
              </div>
              <p class="text-[10px] text-gray-400 mt-1">Click any tag to copy it.</p>
            </Show>
          </div>
        </Show>

        {/* ── DISTRIBUTE TAB ────────────────────────────────────────────── */}
        <Show when={tab() === 'distribute' && !noPost()}>
          <Show when={noLlm()}>
            <NoEndpointBanner />
          </Show>

          {/* Social Snippets */}
          <div>
            <div class="flex items-center justify-between mb-2">
              <span class="text-xs font-semibold text-gray-600 dark:text-gray-300">Social Snippets</span>
              <ActionBtn label="Generate All" loading={snippetsLoading()} onClick={runSnippets} />
            </div>
            <Show when={snippetsError()}><ErrorNote msg={snippetsError()} /></Show>
            <EmptyNote tried={snippetsTried() && Object.keys(snippets()).length === 0 && !snippetsError()} />
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

          {/* Platform Adapter */}
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
              <ActionBtn label="Adapt" loading={adaptLoading()} onClick={runAdapt} />
            </div>
            <Show when={adaptError()}><ErrorNote msg={adaptError()} /></Show>
            <Show when={adaptLoading()}>
              <p class="text-[10px] text-gray-400 mt-1 italic">Rewriting post… this may take up to 90 seconds.</p>
            </Show>
          </div>

          {/* Tone Rewrite */}
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
              <ActionBtn label="Rewrite" loading={toneLoading()} onClick={runTone} />
            </div>
            <Show when={toneError()}><ErrorNote msg={toneError()} /></Show>
            <Show when={toneLoading()}>
              <p class="text-[10px] text-gray-400 mt-1 italic">Rewriting post… this may take up to 90 seconds.</p>
            </Show>
          </div>

          {/* Saved Variants */}
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
                        <Show when={props.onApply}>
                          <button onClick={() => props.onApply!('append', v.content)} class="text-[10px] text-green-600 hover:text-green-800 font-medium">Apply</button>
                        </Show>
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
