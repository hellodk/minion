import {
  Component, createSignal, createEffect, For, Show, onMount, Switch, Match,
} from 'solid-js';
import { createStore } from 'solid-js/store';
import { invoke } from '@tauri-apps/api/core';
import { convertFileSrc } from '@tauri-apps/api/core';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface FvWorkspace { path: string; label: string; }
interface FvEntry {
  name: string; path: string; is_dir: boolean;
  extension: string | null; size: number; modified: string;
}
interface FvFileContent {
  text: string; size: number; is_binary: boolean;
  language: string; line_count: number;
}
interface ExplorerTab { path: string; name: string; loading: boolean; }
type ViewMode = 'source' | 'split' | 'preview';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const MAX_TABS = 20;

const IMAGE_EXTS = new Set(['png','jpg','jpeg','gif','svg','webp','ico','bmp','avif']);
const PREVIEW_EXTS = new Set(['md','markdown','mdx','html','htm']);

const EXT_COLOR: Record<string, string> = {
  ts:'text-blue-500', tsx:'text-blue-400', mts:'text-blue-500',
  js:'text-yellow-500', jsx:'text-yellow-400', mjs:'text-yellow-500',
  rs:'text-orange-500',
  py:'text-green-500',
  go:'text-cyan-500',
  html:'text-red-400', htm:'text-red-400',
  css:'text-purple-500', scss:'text-purple-400', sass:'text-purple-400',
  json:'text-yellow-300', toml:'text-orange-400', yaml:'text-orange-300', yml:'text-orange-300',
  md:'text-sky-400', markdown:'text-sky-400', mdx:'text-sky-400',
  sql:'text-blue-300',
  sh:'text-green-400', bash:'text-green-400', zsh:'text-green-400',
  java:'text-red-500', kt:'text-purple-400', cs:'text-green-400',
  cpp:'text-blue-400', c:'text-blue-300', h:'text-blue-300',
  rb:'text-red-400', php:'text-indigo-400', swift:'text-orange-400',
  xml:'text-gray-400',
};

function extColor(ext: string | null): string {
  return ext ? (EXT_COLOR[ext] ?? 'text-gray-400') : 'text-gray-400';
}

function isImage(ext: string | null): boolean {
  return ext ? IMAGE_EXTS.has(ext) : false;
}
function isPreviewable(path: string): boolean {
  const ext = path.split('.').pop()?.toLowerCase() ?? '';
  return PREVIEW_EXTS.has(ext);
}
function fileExt(path: string): string {
  return path.split('.').pop()?.toLowerCase() ?? '';
}
function fileName(path: string): string {
  return path.split(/[/\\]/).pop() ?? path;
}
function humanSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes/1024).toFixed(1)} KB`;
  return `${(bytes/1048576).toFixed(1)} MB`;
}

// ---------------------------------------------------------------------------
// Small SVG icons
// ---------------------------------------------------------------------------

const IconFolder = () => (
  <svg class="w-3.5 h-3.5 shrink-0 text-yellow-400" fill="currentColor" viewBox="0 0 20 20">
    <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z" />
  </svg>
);
const IconFolderOpen = () => (
  <svg class="w-3.5 h-3.5 shrink-0 text-yellow-300" fill="currentColor" viewBox="0 0 20 20">
    <path fill-rule="evenodd" d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v1H2V6zm-1 3a1 1 0 00-1 1v7a1 1 0 001 1h14a1 1 0 001-1V10a1 1 0 00-1-1H1z" clip-rule="evenodd" />
  </svg>
);
const IconFile = ({ ext }: { ext: string | null }) => (
  <svg class={`w-3.5 h-3.5 shrink-0 ${extColor(ext)}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
      d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
  </svg>
);
const IconChevronRight = () => (
  <svg class="w-3 h-3 shrink-0 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
  </svg>
);
const IconChevronDown = () => (
  <svg class="w-3 h-3 shrink-0 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
  </svg>
);
const IconPlus = () => (
  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
  </svg>
);
const IconX = ({ size = 3 }: { size?: number }) => (
  <svg class={`w-${size} h-${size}`} fill="none" stroke="currentColor" viewBox="0 0 24 24">
    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
  </svg>
);

// ---------------------------------------------------------------------------
// Explorer component
// ---------------------------------------------------------------------------

const Explorer: Component = () => {
  const [workspaces, setWorkspaces] = createSignal<FvWorkspace[]>([]);
  const [tabs, setTabs] = createSignal<ExplorerTab[]>([]);
  const [activeTabPath, setActiveTabPath] = createSignal<string | null>(null);
  const [viewMode, setViewMode] = createSignal<ViewMode>('source');
  const [expandedDirs, setExpandedDirs] = createSignal<Set<string>>(new Set());
  const [loadingDirs, setLoadingDirs] = createSignal<Set<string>>(new Set());
  const [dirContents, setDirContents] = createStore<Record<string, FvEntry[]>>({});
  const [fileCache, setFileCache] = createStore<Record<string, FvFileContent | null>>({});
  const [previewCache, setPreviewCache] = createStore<Record<string, string>>({});
  const [addingFolder, setAddingFolder] = createSignal(false);

  onMount(async () => {
    try {
      const ws = await invoke<FvWorkspace[]>('fv_list_workspaces');
      setWorkspaces(ws);
      // Pre-load top-level of each workspace
      for (const w of ws) {
        void loadDir(w.path);
      }
    } catch { /* DB not ready yet — ignore */ }
  });

  // Reset view mode to 'source' when switching to a non-previewable file
  createEffect(() => {
    const p = activeTabPath();
    if (p && !isPreviewable(p) && viewMode() !== 'source') {
      setViewMode('source');
    }
  });

  // ---------------------------------------------------------------------------
  // Tree actions
  // ---------------------------------------------------------------------------

  async function loadDir(path: string) {
    if (dirContents[path]) return; // already loaded
    setLoadingDirs(s => new Set([...s, path]));
    try {
      const entries = await invoke<FvEntry[]>('fv_read_dir', { path });
      setDirContents(path, entries);
    } catch {
      setDirContents(path, []);
    } finally {
      setLoadingDirs(s => { const n = new Set(s); n.delete(path); return n; });
    }
  }

  function toggleDir(path: string) {
    const s = new Set(expandedDirs());
    if (s.has(path)) {
      s.delete(path);
    } else {
      s.add(path);
      void loadDir(path);
    }
    setExpandedDirs(s);
  }

  // ---------------------------------------------------------------------------
  // Workspace actions
  // ---------------------------------------------------------------------------

  async function addWorkspace() {
    setAddingFolder(true);
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selected = await open({ directory: true, multiple: false, title: 'Add Folder to Explorer' });
      if (!selected) return;
      const path = Array.isArray(selected) ? selected[0] : selected;
      const ws = await invoke<FvWorkspace>('fv_add_workspace', { path });
      setWorkspaces(prev => prev.some(w => w.path === ws.path) ? prev : [...prev, ws]);
      void loadDir(ws.path);
    } catch (e) {
      console.error('Add folder failed:', e);
    } finally {
      setAddingFolder(false);
    }
  }

  async function removeWorkspace(path: string) {
    await invoke('fv_remove_workspace', { path });
    setWorkspaces(prev => prev.filter(w => w.path !== path));
  }

  // ---------------------------------------------------------------------------
  // Tab management
  // ---------------------------------------------------------------------------

  async function openFile(entry: FvEntry) {
    if (entry.is_dir) { toggleDir(entry.path); return; }

    // Already open → just activate
    if (tabs().some(t => t.path === entry.path)) {
      setActiveTabPath(entry.path);
      return;
    }

    // Evict oldest if at limit
    const newTab: ExplorerTab = { path: entry.path, name: entry.name, loading: true };
    setTabs(prev => {
      const without = prev.filter(t => t.path !== entry.path);
      const trimmed = without.length >= MAX_TABS ? without.slice(1) : without;
      return [...trimmed, newTab];
    });
    setActiveTabPath(entry.path);

    // Skip binary detection for images — we show them directly
    if (isImage(entry.extension)) {
      setTabs(prev => prev.map(t => t.path === entry.path ? { ...t, loading: false } : t));
      return;
    }

    // Load file content
    try {
      const content = await invoke<FvFileContent>('fv_read_file', { path: entry.path });
      setFileCache(entry.path, content);
    } catch (e) {
      setFileCache(entry.path, { text: `Error: ${e}`, size: 0, is_binary: false, language: 'plaintext', line_count: 0 });
    } finally {
      setTabs(prev => prev.map(t => t.path === entry.path ? { ...t, loading: false } : t));
    }
  }

  function closeTab(path: string, e?: MouseEvent) {
    e?.stopPropagation();
    const current = tabs();
    const idx = current.findIndex(t => t.path === path);
    const next = current.filter(t => t.path !== path);
    setTabs(next);
    if (activeTabPath() === path) {
      const newActive = next[Math.max(0, idx - 1)]?.path ?? null;
      setActiveTabPath(newActive);
    }
  }

  // ---------------------------------------------------------------------------
  // Preview actions
  // ---------------------------------------------------------------------------

  async function renderPreview(path: string, text: string) {
    if (previewCache[path]) return;
    const ext = fileExt(path);
    if (ext === 'md' || ext === 'markdown' || ext === 'mdx') {
      try {
        const html = await invoke<string>('blog_render_preview', { markdown: text });
        setPreviewCache(path, html);
      } catch { setPreviewCache(path, '<p>Preview failed.</p>'); }
    } else if (ext === 'html' || ext === 'htm') {
      setPreviewCache(path, text);
    }
  }

  createEffect(() => {
    const path = activeTabPath();
    const mode = viewMode();
    if (!path || (mode !== 'preview' && mode !== 'split')) return;
    const content = fileCache[path];
    if (content && !content.is_binary && isPreviewable(path)) {
      void renderPreview(path, content.text);
    }
  });

  // ---------------------------------------------------------------------------
  // Sub-components (inner functions — hoisted, share outer state)
  // ---------------------------------------------------------------------------

  function FileRow(p: { entry: FvEntry; depth: number }) {
    const isActive = () => activeTabPath() === p.entry.path;
    return (
      <div
        class="flex items-center gap-1.5 py-0.5 cursor-pointer select-none rounded-sm"
        classList={{
          'bg-sky-100 dark:bg-sky-900/40 text-sky-900 dark:text-sky-100': isActive(),
          'text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800': !isActive(),
        }}
        style={{ 'padding-left': `${p.depth * 12 + 20}px`, 'padding-right': '8px' }}
        onClick={() => void openFile(p.entry)}
        title={p.entry.path}
      >
        <IconFile ext={p.entry.extension} />
        <span class="text-xs truncate">{p.entry.name}</span>
      </div>
    );
  }

  function DirRow(p: { entry: FvEntry; depth: number }) {
    const isExpanded = () => expandedDirs().has(p.entry.path);
    const isLoading = () => loadingDirs().has(p.entry.path);
    return (
      <>
        <div
          class="flex items-center gap-1.5 py-0.5 cursor-pointer select-none rounded-sm text-gray-700 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800"
          style={{ 'padding-left': `${p.depth * 12 + 6}px`, 'padding-right': '8px' }}
          onClick={() => toggleDir(p.entry.path)}
        >
          <span class="w-3 h-3 flex items-center justify-center shrink-0">
            {isExpanded() ? <IconChevronDown /> : <IconChevronRight />}
          </span>
          {isExpanded() ? <IconFolderOpen /> : <IconFolder />}
          <span class="text-xs truncate">{p.entry.name}</span>
          <Show when={isLoading()}>
            <span class="ml-1 text-[10px] text-gray-400 animate-pulse">…</span>
          </Show>
        </div>
        <Show when={isExpanded()}>
          <TreeLevel parentPath={p.entry.path} depth={p.depth + 1} />
        </Show>
      </>
    );
  }

  function TreeLevel(p: { parentPath: string; depth: number }) {
    const entries = () => dirContents[p.parentPath] ?? [];
    const isLoading = () => loadingDirs().has(p.parentPath) && entries().length === 0;
    return (
      <div>
        <Show when={isLoading()}>
          <div class="text-[11px] text-gray-400 py-0.5 animate-pulse"
            style={{ 'padding-left': `${p.depth * 12 + 20}px` }}>
            Loading…
          </div>
        </Show>
        <For each={entries()}>
          {(entry) => (
            <Show when={entry.is_dir}
              fallback={<FileRow entry={entry} depth={p.depth} />}>
              <DirRow entry={entry} depth={p.depth} />
            </Show>
          )}
        </For>
        <Show when={!isLoading() && entries().length === 0 && dirContents[p.parentPath] !== undefined}>
          <div class="text-[11px] text-gray-400 py-0.5 italic"
            style={{ 'padding-left': `${p.depth * 12 + 20}px` }}>
            Empty folder
          </div>
        </Show>
      </div>
    );
  }

  // ---------------------------------------------------------------------------
  // Source view with syntax highlighting
  // ---------------------------------------------------------------------------

  function SourcePane(p: { path: string; content: FvFileContent }) {
    // Signal-based ref so the createEffect can track when the element mounts.
    // A plain `let` ref would be undefined on the effect's first synchronous
    // run (before the DOM element exists), causing a silent early return with
    // no reactive dep to trigger a re-run. Bug #2 fix.
    const [codeEl, setCodeEl] = createSignal<HTMLElement | undefined>(undefined);

    createEffect(() => {
      const el = codeEl();          // tracked: re-runs when element mounts
      const text = p.content.text;  // tracked: re-runs when content changes
      const lang = p.content.language;
      if (!el || p.content.is_binary) return;

      el.textContent = text;        // set plain text immediately (no flash)

      if (lang === 'plaintext') return;

      // Bug #3 fix: capture `el` before the async gap; verify it's still
      // the current element after highlight.js loads.
      import('highlight.js').then((hljs) => {
        if (codeEl() !== el) return; // stale — element was replaced
        try {
          const safe = hljs.default.getLanguage(lang) ? lang : 'plaintext';
          if (safe !== 'plaintext') {
            el.innerHTML = hljs.default.highlight(text, { language: safe }).value;
          }
        } catch { /* leave as plain text */ }
      }).catch(() => {});
    });

    return (
      <div class="h-full overflow-auto bg-gray-50 dark:bg-gray-950">
        {/* Status bar */}
        <div class="sticky top-0 flex gap-4 px-4 py-1 text-[10px] text-gray-400 bg-gray-100 dark:bg-gray-900 border-b border-gray-200 dark:border-gray-800 font-mono select-none z-10">
          <span>{p.content.language}</span>
          <span>{p.content.line_count} lines</span>
          <span>{humanSize(p.content.size)}</span>
        </div>
        <pre class="m-0 p-4 text-[13px] leading-relaxed font-mono overflow-auto hljs"
          style={{ 'tab-size': '2', 'white-space': 'pre', 'min-height': 'calc(100% - 28px)' }}>
          <code ref={setCodeEl} class={`language-${p.content.language}`} />
        </pre>
      </div>
    );
  }

  // ---------------------------------------------------------------------------
  // Preview pane (MD rendered HTML or HTML iframe)
  // ---------------------------------------------------------------------------

  function PreviewPane(p: { path: string }) {
    const html = () => previewCache[p.path];
    const ext = fileExt(p.path);
    const isMd = ext === 'md' || ext === 'markdown' || ext === 'mdx';

    return (
      <div class="h-full overflow-auto bg-white dark:bg-gray-900">
        <Show when={html()} fallback={
          <div class="flex items-center justify-center h-full text-sm text-gray-400 animate-pulse">
            Rendering preview…
          </div>
        }>
          <Show when={isMd}
            fallback={
              // HTML files: sandboxed iframe
              <iframe
                srcdoc={html()}
                sandbox="allow-same-origin allow-scripts"
                class="w-full h-full border-none"
              />
            }
          >
            {/* MD: inline rendered HTML using same preview styles as Blog */}
            <div
              class="prose prose-slate dark:prose-invert max-w-none p-6
                     prose-headings:font-bold prose-code:bg-gray-100 dark:prose-code:bg-gray-800
                     prose-code:px-1 prose-code:rounded prose-pre:bg-gray-900
                     prose-a:text-sky-600 prose-table:border-collapse
                     prose-th:border prose-th:p-2 prose-td:border prose-td:p-2"
              innerHTML={html()}
            />
          </Show>
        </Show>
      </div>
    );
  }

  // ---------------------------------------------------------------------------
  // Active file content view
  // ---------------------------------------------------------------------------

  function ContentView(p: { path: string }) {
    const tab = () => tabs().find(t => t.path === p.path);
    const content = () => fileCache[p.path];
    const ext = fileExt(p.path);
    const imgSrc = isImage(ext ? ext : null) ? convertFileSrc(p.path) : null;

    return (
      <div class="h-full overflow-hidden">
        <Show when={tab()?.loading}>
          <div class="flex items-center justify-center h-full text-sm text-gray-400 animate-pulse">
            Loading {fileName(p.path)}…
          </div>
        </Show>

        <Show when={!tab()?.loading}>
          {/* Image viewer */}
          <Show when={imgSrc}>
            <div class="flex items-center justify-center h-full bg-gray-100 dark:bg-gray-900 overflow-auto p-4">
              <img src={imgSrc!} alt={fileName(p.path)}
                class="max-w-full max-h-full object-contain rounded shadow-lg" />
            </div>
          </Show>

          {/* Binary file */}
          <Show when={!imgSrc && content()?.is_binary}>
            <div class="flex flex-col items-center justify-center h-full gap-3 text-gray-400">
              <svg class="w-12 h-12 opacity-40" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"
                  d="M9 17v-2m3 2v-4m3 4v-6m2 10H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
              </svg>
              <p class="text-sm font-medium">Binary file — cannot preview</p>
              <p class="text-xs">{humanSize(content()?.size ?? 0)}</p>
            </div>
          </Show>

          {/* Text / code file — Bug #1 fix: use Switch+Match (not Switch+Show).
               Switch with Show children is a SolidJS no-op: Switch only
               processes Match children; Show children are silently discarded. */}
          <Show when={!imgSrc && content() && !content()!.is_binary}>
            <Switch fallback={null}>
              <Match when={viewMode() === 'source'}>
                <SourcePane path={p.path} content={content()!} />
              </Match>
              <Match when={viewMode() === 'preview'}>
                <PreviewPane path={p.path} />
              </Match>
              <Match when={viewMode() === 'split'}>
                <div class="flex h-full">
                  <div class="w-1/2 border-r border-gray-200 dark:border-gray-700 overflow-hidden">
                    <SourcePane path={p.path} content={content()!} />
                  </div>
                  <div class="w-1/2 overflow-hidden">
                    <PreviewPane path={p.path} />
                  </div>
                </div>
              </Match>
            </Switch>
          </Show>

          {/* No content yet (shouldn't happen, but guard) */}
          <Show when={!imgSrc && !content()}>
            <div class="flex items-center justify-center h-full text-sm text-gray-400">
              File content unavailable
            </div>
          </Show>
        </Show>
      </div>
    );
  }

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  const activeFile = () => tabs().find(t => t.path === activeTabPath());

  return (
    <div class="flex h-full overflow-hidden text-sm bg-white dark:bg-gray-900">

      {/* ── Left sidebar ── */}
      <div class="w-64 flex-none flex flex-col bg-gray-50 dark:bg-gray-900 border-r border-gray-200 dark:border-gray-700 overflow-hidden">
        {/* Sidebar header */}
        <div class="flex-none flex items-center justify-between px-3 py-2 border-b border-gray-200 dark:border-gray-700">
          <span class="text-[10px] font-semibold uppercase tracking-widest text-gray-500 dark:text-gray-400">
            Explorer
          </span>
          <button
            onClick={addWorkspace}
            disabled={addingFolder()}
            title="Add folder to workspace"
            class="p-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-500 dark:text-gray-400 disabled:opacity-50 transition-colors"
          >
            <IconPlus />
          </button>
        </div>

        {/* Tree area */}
        <div class="flex-1 overflow-y-auto py-1">
          <Show when={workspaces().length === 0}>
            <div class="px-4 pt-8 text-center">
              <svg class="w-10 h-10 mx-auto mb-3 text-gray-300 dark:text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"
                  d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
              </svg>
              <p class="text-xs text-gray-400 dark:text-gray-500">No folders open</p>
              <button
                onClick={addWorkspace}
                class="mt-3 text-xs text-sky-600 dark:text-sky-400 hover:underline"
              >
                Add a folder
              </button>
            </div>
          </Show>

          <For each={workspaces()}>
            {(ws) => (
              <div class="mb-1">
                {/* Workspace root header */}
                <div class="group flex items-center gap-1.5 px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-800 cursor-pointer"
                  onClick={() => toggleDir(ws.path)}>
                  <span class="w-3 h-3 flex items-center justify-center shrink-0">
                    {expandedDirs().has(ws.path) ? <IconChevronDown /> : <IconChevronRight />}
                  </span>
                  <span class="flex-1 text-[11px] font-semibold uppercase tracking-wider text-gray-600 dark:text-gray-300 truncate"
                    title={ws.path}>
                    {ws.label}
                  </span>
                  <button
                    onClick={(e) => { e.stopPropagation(); void removeWorkspace(ws.path); }}
                    title="Remove folder from workspace"
                    class="opacity-0 group-hover:opacity-100 p-0.5 rounded hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-400 transition-all"
                  >
                    <IconX size={3} />
                  </button>
                </div>

                <Show when={expandedDirs().has(ws.path)}>
                  <TreeLevel parentPath={ws.path} depth={0} />
                </Show>
              </div>
            )}
          </For>
        </div>
      </div>

      {/* ── Right content area ── */}
      <div class="flex-1 flex flex-col overflow-hidden">

        {/* Tab bar */}
        <div class="flex-none flex bg-gray-100 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 overflow-x-auto min-h-[36px]">
          <For each={tabs()}>
            {(tab) => (
              <button
                class="flex-none flex items-center gap-1.5 px-3 py-1.5 text-xs border-r border-gray-200 dark:border-gray-700 transition-colors min-w-0 max-w-[180px] group relative"
                classList={{
                  'bg-white dark:bg-gray-900 text-gray-900 dark:text-white': activeTabPath() === tab.path,
                  'text-gray-500 dark:text-gray-400 hover:bg-gray-50 dark:hover:bg-gray-800 hover:text-gray-700 dark:hover:text-gray-200': activeTabPath() !== tab.path,
                }}
                onClick={() => setActiveTabPath(tab.path)}
                title={tab.path}
              >
                <Show when={activeTabPath() === tab.path}>
                  <div class="absolute bottom-0 left-0 right-0 h-0.5 bg-sky-500" />
                </Show>
                <IconFile ext={fileExt(tab.path) || null} />
                <span class="truncate">{tab.name}</span>
                <span
                  onClick={(e) => closeTab(tab.path, e)}
                  class="shrink-0 opacity-0 group-hover:opacity-100 hover:bg-gray-200 dark:hover:bg-gray-700 rounded p-0.5 transition-all"
                >
                  <IconX size={3} />
                </span>
              </button>
            )}
          </For>

          {/* Spacer + tab count when many open */}
          <Show when={tabs().length === 0}>
            <div class="flex items-center px-4 text-xs text-gray-400 italic select-none">
              Open a file from the Explorer
            </div>
          </Show>
        </div>

        {/* View-mode toggle bar — only for MD / HTML */}
        <Show when={activeFile() && !activeFile()!.loading && isPreviewable(activeTabPath()!)}>
          <div class="flex-none flex items-center gap-1 px-3 py-1.5 bg-white dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700">
            <span class="text-[10px] text-gray-400 mr-1">View:</span>
            {(['source', 'split', 'preview'] as ViewMode[]).map((m) => (
              <button
                onClick={() => setViewMode(m)}
                class="px-2.5 py-0.5 rounded text-xs font-medium transition-colors"
                classList={{
                  'bg-sky-500 text-white': viewMode() === m,
                  'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800': viewMode() !== m,
                }}
              >
                {m === 'source' ? '✏️ Source' : m === 'split' ? '⬛ Split' : '👁 Preview'}
              </button>
            ))}
          </div>
        </Show>

        {/* Content area */}
        <div class="flex-1 overflow-hidden">
          <Show when={activeTabPath()} fallback={
            /* Welcome / empty state */
            <div class="flex flex-col items-center justify-center h-full gap-4 text-gray-400 select-none">
              <svg class="w-16 h-16 opacity-20" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1"
                  d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4" />
              </svg>
              <div class="text-center">
                <p class="text-sm font-medium text-gray-500 dark:text-gray-400">No file open</p>
                <p class="text-xs mt-1">Click a file in the Explorer to open it</p>
              </div>
              <div class="flex gap-3 text-xs text-gray-300 dark:text-gray-600 mt-2">
                <span class="bg-gray-100 dark:bg-gray-800 rounded px-2 py-1">Syntax highlighting</span>
                <span class="bg-gray-100 dark:bg-gray-800 rounded px-2 py-1">MD &amp; HTML preview</span>
                <span class="bg-gray-100 dark:bg-gray-800 rounded px-2 py-1">Split view</span>
              </div>
            </div>
          }>
            <ContentView path={activeTabPath()!} />
          </Show>
        </div>
      </div>

    </div>
  );
};

export default Explorer;
