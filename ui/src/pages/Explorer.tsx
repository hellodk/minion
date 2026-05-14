import 'highlight.js/styles/github.css';
import {
  Component, createSignal, createEffect, createMemo, For, Show, onMount, onCleanup,
  Switch, Match,
} from 'solid-js';
import { diffLines } from 'diff';
import type { Change } from 'diff';
import { createStore, produce } from 'solid-js/store';
import { invoke } from '@tauri-apps/api/core';
import DOMPurify from 'dompurify';
import { open as shellOpen } from '@tauri-apps/plugin-shell';

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
interface ExplorerTab { path: string; name: string; }
type ViewMode = 'source' | 'split' | 'preview' | 'diff';

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
  return PREVIEW_EXTS.has(fileExt(path));
}
function fileExt(path: string): string {
  const base = path.split(/[/\\]/).pop() ?? '';
  const dot = base.lastIndexOf('.');
  if (dot <= 0) return '';
  return base.slice(dot + 1).toLowerCase();
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
// Context menu type
// ---------------------------------------------------------------------------

type ContextMenuTarget = { entry: FvEntry; x: number; y: number } | null;

// ---------------------------------------------------------------------------
// Explorer component
// ---------------------------------------------------------------------------

const Explorer: Component = () => {
  const [workspaces, setWorkspaces] = createSignal<FvWorkspace[]>([]);
  const [tabs, setTabs] = createSignal<ExplorerTab[]>([]);
  const [activeTabPath, setActiveTabPath] = createSignal<string | null>(null);

  // Per-tab view mode store (replaces global viewMode signal)
  const [tabViewModes, setTabViewModes] = createStore<Record<string, ViewMode>>({});
  const viewMode = () => activeTabPath() ? (tabViewModes[activeTabPath()!] ?? 'source') : 'source';
  const setViewMode = (m: ViewMode) => { const p = activeTabPath(); if (p) setTabViewModes(p, m); };

  const [expandedDirs, setExpandedDirs] = createSignal<Set<string>>(new Set());
  const [loadingDirs, setLoadingDirs] = createSignal<Set<string>>(new Set());
  const [dirContents, setDirContents] = createStore<Record<string, FvEntry[]>>({});
  const [fileCache, setFileCache] = createStore<Record<string, FvFileContent | null>>({});
  const [previewCache, setPreviewCache] = createStore<Record<string, string>>({});
  const [previewInFlight, setPreviewInFlight] = createSignal<Set<string>>(new Set());
  const [addingFolder, setAddingFolder] = createSignal(false);
  const [showHidden, setShowHidden] = createSignal(false);
  const [failedDirs, setFailedDirs] = createSignal<Set<string>>(new Set());
  const [aiError, setAiError] = createSignal<string | null>(null);

  const [sidebarOpen, setSidebarOpen] = createSignal(true);

  // CR17: Persist sidebar width across sessions
  const [sidebarWidth, setSidebarWidth] = createSignal(
    parseInt(localStorage.getItem('explorer-sidebar-width') ?? '256', 10)
  );
  createEffect(() => {
    localStorage.setItem('explorer-sidebar-width', String(sidebarWidth()));
  });

  const [gitStatus, setGitStatus] = createStore<Record<string, string>>({});

  const [contextMenu, setContextMenu] = createSignal<ContextMenuTarget>(null);

  // Tab eviction toast
  const [evictedTab, setEvictedTab] = createSignal<string | null>(null);
  let evictToastTimer: ReturnType<typeof setTimeout> | undefined;

  // Restore expanded dirs from previous session
  {
    const saved = localStorage.getItem('explorer-expanded');
    if (saved) {
      try { setExpandedDirs(new Set(JSON.parse(saved) as string[])); } catch { /* ignore */ }
    }
  }
  // CR13: Debounce localStorage writes for expandedDirs
  let _saveTimer: ReturnType<typeof setTimeout> | undefined;
  createEffect(() => {
    const dirs = [...expandedDirs()]; // access signal to track it
    clearTimeout(_saveTimer);
    _saveTimer = setTimeout(() => {
      localStorage.setItem('explorer-expanded', JSON.stringify(dirs));
    }, 400);
  });

  // File search
  const [searchQuery, setSearchQuery] = createSignal('');
  const [searchActive, setSearchActive] = createSignal(false);

  const searchResults = createMemo(() => {
    const q = searchQuery().trim().toLowerCase();
    if (!q) return [];
    const results: FvEntry[] = [];
    for (const key of Object.keys(dirContents)) {
      for (const entry of (dirContents[key] ?? [])) {
        if (!entry.is_dir && entry.name.toLowerCase().includes(q)) {
          results.push(entry);
        }
      }
    }
    return results.slice(0, 80);
  });

  // Image cache: path → base64 data URI (loaded on demand via fv_read_image_base64)
  const [imageCache, setImageCache] = createStore<Record<string, string | null>>({});

  // AI Format MD
  const [aiWorking, setAiWorking] = createSignal(false);
  const [aiOriginals, setAiOriginals] = createStore<Record<string, string>>({});

  // Git status refresh
  async function refreshGitStatus(wsPath: string) {
    try {
      const status = await invoke<Record<string, string>>('fv_git_status', { workspacePath: wsPath });
      setGitStatus(produce(d => { Object.assign(d, status); }));
    } catch { /* git not available */ }
  }

  onMount(async () => {
    try {
      const ws = await invoke<FvWorkspace[]>('fv_list_workspaces');
      setWorkspaces(ws);
      // Auto-expand all workspaces on mount
      setExpandedDirs(s => { const n = new Set(s); ws.forEach(w => n.add(w.path)); return n; });
      for (const w of ws) {
        void loadDir(w.path);
        void refreshGitStatus(w.path);
      }
    } catch { /* DB not ready yet — ignore */ }
  });

  // Keyboard shortcuts
  {
    const handler = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key === 'w') {
        const p = activeTabPath();
        if (p) { e.preventDefault(); closeTab(p); }
      } else if (e.ctrlKey && e.key === 'Tab') {
        e.preventDefault();
        const ts = tabs();
        if (ts.length < 2) return;
        const idx = ts.findIndex(t => t.path === activeTabPath());
        setActiveTabPath(ts[(idx + (e.shiftKey ? -1 : 1) + ts.length) % ts.length].path);
      } else if (e.ctrlKey && (e.key === 'f' || e.key === 'p')) {
        e.preventDefault();
        setSearchActive(true);
        setSidebarOpen(true);
      }
    };
    window.addEventListener('keydown', handler);
    onCleanup(() => window.removeEventListener('keydown', handler));
  }

  // Context menu dismiss
  {
    const dismiss = () => setContextMenu(null);
    window.addEventListener('click', dismiss);
    onCleanup(() => window.removeEventListener('click', dismiss));
  }

  // Reset view mode to 'source' when switching to a non-previewable file
  createEffect(() => {
    const p = activeTabPath();
    if (p && !isPreviewable(p) && viewMode() !== 'source' && viewMode() !== 'diff') {
      setViewMode('source');
    }
  });

  // ---------------------------------------------------------------------------
  // Tree actions
  // ---------------------------------------------------------------------------

  async function loadDir(path: string) {
    // Allow reload if previously failed; skip if already loading or successfully cached
    const alreadyLoaded = dirContents[path] !== undefined && !failedDirs().has(path);
    if (alreadyLoaded || loadingDirs().has(path)) return;
    setLoadingDirs(s => new Set([...s, path]));
    setFailedDirs(s => { const n = new Set(s); n.delete(path); return n; });
    try {
      const entries = await invoke<FvEntry[]>('fv_read_dir', { path, showHidden: showHidden() });
      setDirContents(path, entries);
    } catch {
      setDirContents(path, []);
      setFailedDirs(s => new Set([...s, path]));
    } finally {
      setLoadingDirs(s => { const n = new Set(s); n.delete(path); return n; });
    }
  }

  function reloadDir(path: string) {
    setDirContents(produce(d => { delete d[path]; }));
    setFailedDirs(s => { const n = new Set(s); n.delete(path); return n; });
    void loadDir(path);
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

  /** Expand every directory under `rootPath` up to `depth` levels deep. */
  async function expandAll(rootPath: string, maxDepth = 4) {
    const queue: Array<{ path: string; depth: number }> = [{ path: rootPath, depth: 0 }];
    setExpandedDirs(s => new Set([...s, rootPath]));

    while (queue.length > 0) {
      const { path, depth } = queue.shift()!;
      if (depth >= maxDepth) continue;
      await loadDir(path);
      const newDirs = (dirContents[path] ?? []).filter(e => e.is_dir);
      if (newDirs.length > 0) {
        setExpandedDirs(s => {
          const next = new Set(s);
          for (const d of newDirs) next.add(d.path);
          return next;
        });
        for (const d of newDirs) queue.push({ path: d.path, depth: depth + 1 });
      }
    }
  }

  /** Collapse every directory under `rootPath` (including root itself). */
  function collapseAll(rootPath: string) {
    const s = new Set(expandedDirs());
    // Remove rootPath and everything nested under it
    for (const p of s) {
      if (p === rootPath || p.startsWith(rootPath + '/') || p.startsWith(rootPath + '\\')) {
        s.delete(p);
      }
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
      void refreshGitStatus(ws.path);
      setExpandedDirs(s => new Set([...s, ws.path]));
    } catch (e) {
      console.error('Add folder failed:', e);
    } finally {
      setAddingFolder(false);
    }
  }

  async function removeWorkspace(wsPath: string) {
    try {
      await invoke('fv_remove_workspace', { path: wsPath });
    } catch (e) {
      console.error('Failed to remove workspace from DB:', e);
      // Continue with UI cleanup regardless
    }
    setWorkspaces(prev => prev.filter(w => w.path !== wsPath));

    // Close tabs for files under this workspace
    const affectedTabs = tabs().filter(
      t => t.path === wsPath || t.path.startsWith(wsPath + '/'),
    );
    if (affectedTabs.length > 0) {
      const newTabs = tabs().filter(
        t => t.path !== wsPath && !t.path.startsWith(wsPath + '/'),
      );
      setTabs(newTabs);
      if (activeTabPath() &&
          (activeTabPath()! === wsPath || activeTabPath()!.startsWith(wsPath + '/'))) {
        setActiveTabPath(newTabs[0]?.path ?? null);
      }
    }

    // Clear cached dir tree, file contents, and previews for this workspace
    setDirContents(produce(d => {
      for (const key of Object.keys(d)) {
        if (key === wsPath || key.startsWith(wsPath + '/')) delete d[key];
      }
    }));
    setFileCache(produce(d => {
      for (const key of Object.keys(d)) {
        if (key === wsPath || key.startsWith(wsPath + '/')) delete d[key];
      }
    }));
    setPreviewCache(produce(d => {
      for (const key of Object.keys(d)) {
        if (key === wsPath || key.startsWith(wsPath + '/')) delete d[key];
      }
    }));
    setImageCache(produce(d => {
      for (const key of Object.keys(d)) {
        if (key === wsPath || key.startsWith(wsPath + '/')) delete d[key];
      }
    }));
  }

  // ---------------------------------------------------------------------------
  // Tab management
  // ---------------------------------------------------------------------------

  async function openFile(entry: FvEntry) {
    if (entry.is_dir) { toggleDir(entry.path); return; }

    if (tabs().some(t => t.path === entry.path)) {
      setActiveTabPath(entry.path);
      return;
    }

    const newTab: ExplorerTab = { path: entry.path, name: entry.name };

    // Tab eviction toast
    setTabs(prev => {
      const without = prev.filter(t => t.path !== entry.path);
      if (without.length >= MAX_TABS) {
        const evicted = without[0];
        clearTimeout(evictToastTimer);
        setEvictedTab(evicted.name);
        evictToastTimer = setTimeout(() => setEvictedTab(null), 3000);
        return [...without.slice(1), newTab];
      }
      return [...without, newTab];
    });
    setActiveTabPath(entry.path);

    // PDF: extract text via backend
    if (entry.extension === 'pdf') {
      setFileCache(entry.path, null);
      invoke<FvFileContent>('fv_extract_pdf', { path: entry.path })
        .then(r => setFileCache(entry.path, r))
        .catch(e => setFileCache(entry.path, {
          text: `⚠ PDF extraction failed\n\n${e}`,
          size: 0, is_binary: false, language: 'plaintext', line_count: 0,
        }));
      return;
    }

    // Images are loaded as base64 data URIs to avoid asset-protocol scope issues.
    if (isImage(entry.extension)) {
      if (!imageCache[entry.path]) {
        setImageCache(entry.path, null); // null = loading
        const imgTimeout = new Promise<never>((_, r) =>
          setTimeout(() => r(new Error('timeout')), 15_000));
        Promise.race([
          invoke<string>('fv_read_image_base64', { path: entry.path }),
          imgTimeout,
        ])
          .then(uri => setImageCache(entry.path, uri as string))
          .catch(() => setImageCache(entry.path, ''));
      }
      return;
    }

    setFileCache(entry.path, null);

    let timeoutId: ReturnType<typeof setTimeout> | undefined;
    const failSafe = new Promise<never>((_, reject) => {
      timeoutId = setTimeout(
        () => reject(new Error('File load timed out (15 s). Click "Retry load" to try again.')),
        15_000,
      );
    });

    try {
      const content = await Promise.race([
        invoke<FvFileContent>('fv_read_file', { path: entry.path }),
        failSafe,
      ]);
      clearTimeout(timeoutId);
      setFileCache(entry.path, content);
    } catch (e) {
      clearTimeout(timeoutId);
      setFileCache(entry.path, {
        text: `⚠ Could not load file\n\n${e}`,
        size: 0, is_binary: false, language: 'plaintext', line_count: 0,
      });
    }
  }

  function closeTab(path: string, e?: MouseEvent) {
    e?.stopPropagation();
    const current = tabs();
    const idx = current.findIndex(t => t.path === path);
    const next = current.filter(t => t.path !== path);
    setTabs(next);
    // Clean up per-tab state
    if (previewCache[path]) setPreviewCache(produce(d => { delete d[path]; }));
    if (tabViewModes[path]) setTabViewModes(produce(d => { delete d[path]; }));
    if (aiOriginals[path]) setAiOriginals(produce(d => { delete d[path]; }));
    if (activeTabPath() === path) {
      const newActive = next[Math.min(idx, next.length - 1)]?.path ?? null;
      setActiveTabPath(newActive);
    }
  }

  // ---------------------------------------------------------------------------
  // Preview actions
  // ---------------------------------------------------------------------------

  // Resolve all relative/absolute local img src values in rendered HTML to
  // base64 data URIs so images display correctly in the WebView preview.
  async function resolveImagesInHtml(html: string, mdFilePath: string): Promise<string> {
    const baseDir = mdFilePath.includes('/') || mdFilePath.includes('\\')
      ? mdFilePath.substring(0, Math.max(mdFilePath.lastIndexOf('/'), mdFilePath.lastIndexOf('\\')))
      : '';

    // Only match src attributes inside <img> tags
    const imgTagRe = /(<img\b[^>]*?\bsrc=")([^"]+)(")/gi;
    const matches: Array<{ full: string; prefix: string; src: string; suffix: string }> = [];
    let m: RegExpExecArray | null;
    while ((m = imgTagRe.exec(html)) !== null) {
      matches.push({ full: m[0], prefix: m[1], src: m[2], suffix: m[3] });
    }

    let result = html;
    for (const { full, prefix, src, suffix } of matches) {
      if (src.startsWith('data:') || src.startsWith('http://') || src.startsWith('https://')) continue;
      const absPath = src.startsWith('/') ? src : `${baseDir}/${src}`;
      try {
        const uri = await invoke<string>('fv_read_image_base64', { path: absPath });
        result = result.replace(full, `${prefix}${uri}${suffix}`);
      } catch { /* skip unresolvable paths */ }
    }
    return result;
  }

  const DOMPURIFY_CONFIG = {
    ADD_TAGS: ['svg', 'use', 'path', 'circle', 'rect', 'line', 'polyline', 'polygon', 'g', 'defs', 'clipPath', 'text', 'tspan'],
    ADD_ATTR: ['viewBox', 'fill', 'stroke', 'stroke-width', 'd', 'cx', 'cy', 'r', 'x', 'y', 'width', 'height', 'transform', 'xmlns'],
  };

  async function renderPreview(path: string, text: string) {
    if (previewCache[path] || previewInFlight().has(path)) return;
    setPreviewInFlight(s => new Set([...s, path]));
    const ext = fileExt(path);
    try {
      if (ext === 'md' || ext === 'markdown' || ext === 'mdx') {
        const rawHtml = await invoke<string>('blog_render_preview', { markdown: text });
        const html = await resolveImagesInHtml(rawHtml, path);
        setPreviewCache(path, DOMPurify.sanitize(html, DOMPURIFY_CONFIG));
      } else if (ext === 'html' || ext === 'htm') {
        const html = await resolveImagesInHtml(text, path);
        setPreviewCache(path, DOMPurify.sanitize(html, DOMPURIFY_CONFIG));
      }
    } catch {
      setPreviewCache(path, '<p>Preview failed.</p>');
    } finally {
      setPreviewInFlight(s => { const n = new Set(s); n.delete(path); return n; });
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
        onContextMenu={(e: MouseEvent) => {
          e.preventDefault();
          e.stopPropagation();
          setContextMenu({ entry: p.entry, x: e.clientX, y: e.clientY });
        }}
        title={p.entry.path}
      >
        <IconFile ext={p.entry.extension} />
        <span class="text-xs truncate">{p.entry.name}</span>
        {/* Git status badge */}
        <Show when={gitStatus[p.entry.path]}>
          <span
            class={`text-[9px] font-bold ml-auto shrink-0 ${
              gitStatus[p.entry.path]?.includes('?') ? 'text-green-500' :
              gitStatus[p.entry.path]?.trim() === 'A' ? 'text-green-400' :
              'text-amber-400'
            }`}
            title={`Git: ${gitStatus[p.entry.path]}`}
          >
            {gitStatus[p.entry.path]?.includes('?') ? 'U' : gitStatus[p.entry.path]?.trim()[0]}
          </span>
        </Show>
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
          <Show when={failedDirs().has(p.entry.path)}>
            <span class="ml-1 text-[10px] text-red-400" title="Failed to load — click to retry">⚠</span>
            <button
              onClick={(e) => { e.stopPropagation(); reloadDir(p.entry.path); }}
              class="text-[10px] text-gray-400 hover:text-gray-600 ml-0.5"
              title="Reload directory"
            >↻</button>
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
  // Source view with syntax highlighting + line number gutter
  // ---------------------------------------------------------------------------

  function SourcePane(p: { path: string; content: FvFileContent }) {
    // Signal-based ref so the createEffect can track when the element mounts.
    const [codeEl, setCodeEl] = createSignal<HTMLElement | undefined>(undefined);

    createEffect(() => {
      const el = codeEl();
      const text = p.content.text;
      const lang = p.content.language;
      if (!el || p.content.is_binary) return;

      el.textContent = text;

      if (lang === 'plaintext') return;

      import('highlight.js').then((hljs) => {
        if (codeEl() !== el) return;
        try {
          const safe = hljs.default.getLanguage(lang) ? lang : 'plaintext';
          if (safe !== 'plaintext') {
            el.innerHTML = hljs.default.highlight(text, { language: safe }).value;
          }
        } catch { /* leave as plain text */ }
      }).catch(() => {});
    });

    return (
      <div class="h-full overflow-hidden flex flex-col">
        {/* Status bar */}
        <div class="flex-none sticky top-0 flex gap-4 px-4 py-1 text-[10px] text-gray-400 bg-gray-100 dark:bg-gray-900 border-b border-gray-200 dark:border-gray-800 font-mono select-none z-10">
          <span>{p.content.language}</span>
          <span>{p.content.line_count} lines</span>
          <span>{humanSize(p.content.size)}</span>
          <button onClick={() => navigator.clipboard.writeText(p.content.text)} title="Copy to clipboard" class="ml-auto px-1.5 py-0.5 rounded text-[10px] text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700 hover:text-gray-600 dark:hover:text-gray-200 transition-colors">Copy</button>
        </div>
        <div class="flex flex-1 overflow-auto">
          {/* Line numbers */}
          <div class="select-none text-right text-gray-400 dark:text-gray-600 font-mono text-[13px] leading-relaxed py-4 px-2 bg-gray-50 dark:bg-gray-950 border-r border-gray-200 dark:border-gray-800 shrink-0 min-w-[2.5rem] sticky left-0 z-10">
            <For each={Array.from({length: p.content.line_count || 1}, (_, i) => i + 1)}>
              {(n) => <div class="leading-relaxed">{n}</div>}
            </For>
          </div>
          {/* Code */}
          <pre class="m-0 p-4 text-[13px] leading-relaxed font-mono flex-1 min-w-0" style={{ 'tab-size': '2', 'white-space': 'pre' }}>
            <code ref={setCodeEl} class={`language-${p.content.language}`} />
          </pre>
        </div>
      </div>
    );
  }

  // ---------------------------------------------------------------------------
  // Preview pane (MD rendered HTML or HTML iframe)
  // ---------------------------------------------------------------------------

  function PreviewPane(p: { path: string }) {
    const html = () => previewCache[p.path];
    const isMd = () => { const e = fileExt(p.path); return e === 'md' || e === 'markdown' || e === 'mdx'; };
    const [previewEl, setPreviewEl] = createSignal<HTMLDivElement>();

    createEffect(() => {
      const el = previewEl();
      const h = html();
      if (!el || !h || !isMd()) return;
      queueMicrotask(() => {
        import('highlight.js').then(hljs => {
          el.querySelectorAll('pre code[class*="language-"]').forEach(block => {
            if (!(block as HTMLElement).dataset['highlighted']) {
              hljs.default.highlightElement(block as HTMLElement);
            }
          });
        }).catch(() => {});
      });
    });

    // Mermaid diagram rendering
    createEffect(() => {
      const el = previewEl();
      const h = html();
      if (!el || !h || !isMd()) return;
      queueMicrotask(async () => {
        const mermaidBlocks = el.querySelectorAll('pre code.language-mermaid, pre code[class*="language-mermaid"]');
        if (mermaidBlocks.length === 0) return;
        try {
          const mermaid = (await import('mermaid')).default;
          mermaid.initialize({ startOnLoad: false, theme: 'neutral', securityLevel: 'strict' });
          mermaidBlocks.forEach((block, i) => {
            const pre = block.parentElement;
            if (!pre) return;
            const container = document.createElement('div');
            container.className = 'mermaid-diagram';
            container.id = `mermaid-${Date.now()}-${i}`;
            container.setAttribute('data-mermaid', block.textContent || '');
            pre.replaceWith(container);
          });
          for (const container of el.querySelectorAll('.mermaid-diagram')) {
            const code = container.getAttribute('data-mermaid') || '';
            if (!code.trim()) continue;
            try {
              const { svg } = await mermaid.render(container.id + '-svg', code);
              container.innerHTML = svg;
            } catch { container.innerHTML = '<p style="color:red;font-size:12px">Mermaid render error</p>'; }
          }
        } catch { /* mermaid not available */ }
      });
    });

    return (
      <div class="h-full overflow-auto bg-white dark:bg-gray-900">
        <Show when={html()} fallback={
          <div class="flex items-center justify-center h-full text-sm text-gray-400 animate-pulse">
            Rendering preview…
          </div>
        }>
          <Show when={isMd()}
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
              ref={setPreviewEl}
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
  // Diff pane for AI-modified files
  // ---------------------------------------------------------------------------

  function DiffPane(p: { path: string }) {
    const original = () => aiOriginals[p.path] ?? '';
    const current = () => (fileCache[p.path] as FvFileContent)?.text ?? '';

    const [diffResult, setDiffResult] = createSignal<Change[]>([]);

    createEffect(() => {
      const orig = original();
      const cur = current();
      if (!orig && !cur) return;
      setDiffResult(diffLines(orig, cur));
    });

    return (
      <div class="h-full overflow-auto font-mono text-[12px] p-4 bg-gray-50 dark:bg-gray-950">
        <For each={diffResult()}>
          {(part) => (
            <For each={(part.value ?? '').split('\n').slice(0, part.value?.endsWith('\n') ? -1 : undefined)}>
              {(line) => (
                <div class={`px-2 leading-relaxed ${
                  part.added ? 'bg-green-50 dark:bg-green-900/30 text-green-800 dark:text-green-200' :
                  part.removed ? 'bg-red-50 dark:bg-red-900/30 text-red-800 dark:text-red-200' :
                  'text-gray-700 dark:text-gray-300'
                }`}>
                  <span class="select-none text-gray-400 w-4 inline-block">
                    {part.added ? '+' : part.removed ? '-' : ' '}
                  </span>
                  {line}
                </div>
              )}
            </For>
          )}
        </For>
      </div>
    );
  }

  // ---------------------------------------------------------------------------
  // Active file content view
  // ---------------------------------------------------------------------------

  function ContentView(p: { path: string }) {
    const cacheEntry = () => fileCache[p.path];
    // Image files use imageCache (base64 data URI); null = still loading, '' = failed
    const imageCacheEntry = () => imageCache[p.path];
    const ext = () => fileExt(p.path);
    const isImg = () => isImage(ext());
    // For images: loading while imageCache entry is null (undefined = not yet requested,
    // which won't happen because openFile always seeds it; null = fetch in flight)
    const isLoading = () => isImg()
      ? (imageCacheEntry() === null || imageCacheEntry() === undefined)
      : (cacheEntry() === null || cacheEntry() === undefined);
    // Use ?? undefined to convert null sentinel to undefined, making the type FvFileContent | undefined
    const content = () => cacheEntry() ?? undefined;
    const imgSrc = () => isImg() ? (imageCacheEntry() || null) : null;

    return (
      <div class="h-full overflow-hidden">
        <Switch>
          <Match when={imgSrc()}>
            <div class="flex items-center justify-center h-full bg-gray-100 dark:bg-gray-900 overflow-auto p-4">
              <img src={imgSrc()!} alt={fileName(p.path)}
                class="max-w-full max-h-full object-contain rounded shadow-lg" />
            </div>
          </Match>

          <Match when={isLoading()}>
            <div class="flex flex-col items-center justify-center h-full gap-3">
              <div class="text-sm text-gray-400 animate-pulse">Loading {fileName(p.path)}…</div>
              <button
                class="text-xs text-sky-500 underline mt-1"
                onClick={async () => {
                  setFileCache(p.path, null);
                  let tid: ReturnType<typeof setTimeout> | undefined;
                  const fs = new Promise<never>((_, r) => { tid = setTimeout(() => r(new Error('timeout')), 15_000); });
                  try {
                    const r = await Promise.race([invoke<FvFileContent>('fv_read_file', { path: p.path }), fs]);
                    clearTimeout(tid);
                    setFileCache(p.path, r);
                  } catch(e) {
                    clearTimeout(tid);
                    setFileCache(p.path, { text: `⚠ Retry failed: ${e}`, size: 0, is_binary: false, language: 'plaintext', line_count: 0 });
                  }
                }}
              >Retry load</button>
            </div>
          </Match>

          <Match when={content()?.is_binary}>
            <div class="flex flex-col items-center justify-center h-full gap-3 text-gray-400">
              <svg class="w-12 h-12 opacity-40" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"
                  d="M9 17v-2m3 2v-4m3 4v-6m2 10H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
              </svg>
              <p class="text-sm font-medium">Binary file — cannot preview</p>
              <p class="text-xs">{humanSize(content()?.size ?? 0)}</p>
              <button
                onClick={() => shellOpen(p.path).catch(() => {})}
                class="text-xs text-sky-500 hover:underline"
              >
                Open with system app
              </button>
            </div>
          </Match>

          <Match when={content()}>
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
              {/* Diff view */}
              <Match when={viewMode() === 'diff'}>
                <DiffPane path={p.path} />
              </Match>
            </Switch>
          </Match>
        </Switch>
      </div>
    );
  }

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  return (
    <div class="flex h-full overflow-hidden text-sm bg-white dark:bg-gray-900">

      {/* ── Left sidebar ── */}
      <Show when={sidebarOpen()} fallback={
        <div class="w-8 flex-none flex flex-col bg-gray-50 dark:bg-gray-900 border-r border-gray-200 dark:border-gray-700">
          <button onClick={() => setSidebarOpen(true)} class="p-1 m-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-500">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 5l7 7-7 7M5 5l7 7-7 7" />
            </svg>
          </button>
        </div>
      }>
        {/* Resizable sidebar — width driven by sidebarWidth signal */}
        <div
          class="flex-none flex flex-col bg-gray-50 dark:bg-gray-900 border-r border-gray-200 dark:border-gray-700 overflow-hidden"
          style={{ width: `${sidebarWidth()}px` }}
        >
          {/* Sidebar header */}
          <div class="flex-none flex items-center gap-1 px-3 py-2 border-b border-gray-200 dark:border-gray-700 overflow-hidden">
            <span class="text-[10px] font-semibold uppercase tracking-widest text-gray-500 dark:text-gray-400 truncate min-w-0 flex-1">
              Explorer
            </span>
            <div class="flex-none flex items-center gap-0.5">
              {/* Show hidden files toggle */}
              <button
                onClick={() => {
                  const newVal = !showHidden();
                  setShowHidden(newVal);
                  // Clear all dir caches so they reload with new hidden setting
                  setDirContents(produce(d => { Object.keys(d).forEach(k => { delete d[k]; }); }));
                  setFailedDirs(new Set<string>());
                  for (const p of expandedDirs()) void loadDir(p);
                }}
                title={showHidden() ? 'Hide hidden files' : 'Show hidden files (. files)'}
                class={`p-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors ${showHidden() ? 'text-sky-500 dark:text-sky-400' : 'text-gray-500 dark:text-gray-400'}`}
              >
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                    d={showHidden()
                      ? "M15 12a3 3 0 11-6 0 3 3 0 016 0zM2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z"
                      : "M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l3.59 3.59m0 0A9.953 9.953 0 0112 5c4.478 0 8.268 2.943 9.542 7a10.025 10.025 0 01-4.132 5.411m0 0L21 21"}
                  />
                </svg>
              </button>
              {/* Search toggle button */}
              <button
                onClick={() => { setSearchActive(s => !s); if (searchActive()) setSearchQuery(''); }}
                title="Search files"
                class="p-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-500 dark:text-gray-400 transition-colors"
              >
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                </svg>
              </button>
              {/* Add folder button */}
              <button
                onClick={addWorkspace}
                disabled={addingFolder()}
                title="Add folder to workspace"
                class="p-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-500 dark:text-gray-400 disabled:opacity-50 transition-colors"
              >
                <IconPlus />
              </button>
              {/* Sidebar close button */}
              <button
                onClick={() => setSidebarOpen(false)}
                title="Close sidebar"
                class="p-1 rounded hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-500 dark:text-gray-400 transition-colors"
              >
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 19l-7-7 7-7m8 14l-7-7 7-7" />
                </svg>
              </button>
            </div>
          </div>

          {/* Search input */}
          <Show when={searchActive()}>
            <div class="px-2 py-1.5 border-b border-gray-200 dark:border-gray-700">
              <div class="flex items-center gap-1 bg-white dark:bg-gray-800 border border-gray-300 dark:border-gray-600 rounded px-2 py-0.5">
                <svg class="w-3 h-3 text-gray-400 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
                </svg>
                <input
                  type="text"
                  placeholder="Search files…"
                  value={searchQuery()}
                  onInput={(e) => setSearchQuery(e.currentTarget.value)}
                  class="flex-1 bg-transparent text-xs outline-none text-gray-700 dark:text-gray-200 placeholder-gray-400"
                  autofocus
                />
                <Show when={searchQuery()}>
                  <button onClick={() => setSearchQuery('')} class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-200">
                    <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                    </svg>
                  </button>
                </Show>
              </div>
            </div>
          </Show>

          {/* Tree area */}
          <div class="flex-1 overflow-y-auto py-1">
            {/* Search results */}
            <Show when={searchActive()}>
              <div class="py-1">
                <Show when={searchQuery().trim() && searchResults().length === 0}>
                  <p class="px-4 py-3 text-xs text-gray-400 italic">No files found</p>
                </Show>
                <Show when={!searchQuery().trim()}>
                  <p class="px-4 py-3 text-xs text-gray-400 italic">Type to search…</p>
                </Show>
                <For each={searchResults()}>
                  {(entry) => (
                    <div
                      class="flex flex-col px-3 py-1.5 cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-800 group"
                      onClick={() => { void openFile(entry); }}
                    >
                      <span class="text-xs text-gray-800 dark:text-gray-200 truncate">{entry.name}</span>
                      <span class="text-[10px] text-gray-400 truncate">{entry.path}</span>
                    </div>
                  )}
                </For>
                <Show when={searchResults().length === 0 && workspaces().length > 0}>
                  <div class="px-3 py-1 text-[10px] text-gray-400 italic border-t border-gray-100 dark:border-gray-800 mt-1">
                    Tip: Expand folders to include their files in search
                  </div>
                </Show>
              </div>
            </Show>

            {/* Normal workspace tree */}
            <Show when={!searchActive()}>
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
                    <div class="group flex items-center gap-1 px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-800 cursor-pointer"
                      onClick={() => toggleDir(ws.path)}>
                      <span class="w-3 h-3 flex items-center justify-center shrink-0">
                        {expandedDirs().has(ws.path) ? <IconChevronDown /> : <IconChevronRight />}
                      </span>
                      <span class="flex-1 text-[11px] font-semibold uppercase tracking-wider text-gray-600 dark:text-gray-300 truncate"
                        title={ws.path}>
                        {ws.label}
                      </span>
                      {/* Expand / Collapse all */}
                      <Show when={expandedDirs().has(ws.path)}>
                        <button
                          onClick={(e) => { e.stopPropagation(); void expandAll(ws.path); }}
                          title="Expand all (up to 4 levels)"
                          class="p-0.5 rounded hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-400 transition-all"
                        >
                          <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                              d="M19 9l-7 7-7-7" />
                          </svg>
                        </button>
                        <button
                          onClick={(e) => { e.stopPropagation(); collapseAll(ws.path); }}
                          title="Collapse all"
                          class="p-0.5 rounded hover:bg-gray-200 dark:hover:bg-gray-700 text-gray-400 transition-all"
                        >
                          <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                              d="M5 15l7-7 7 7" />
                          </svg>
                        </button>
                      </Show>
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
            </Show>
          </div>
        </div>
      </Show>

      {/* Resize handle between sidebar and content */}
      <Show when={sidebarOpen()}>
        <div
          class="w-1 flex-none cursor-col-resize bg-gray-200 dark:bg-gray-700 hover:bg-sky-400 dark:hover:bg-sky-600 transition-colors"
          onMouseDown={(startE: MouseEvent) => {
            startE.preventDefault();
            const startX = startE.clientX;
            const startW = sidebarWidth();
            const onMove = (e: MouseEvent) => setSidebarWidth(Math.max(160, Math.min(600, startW + e.clientX - startX)));
            const onUp = () => {
              window.removeEventListener('mousemove', onMove);
              window.removeEventListener('mouseup', onUp);
            };
            window.addEventListener('mousemove', onMove);
            window.addEventListener('mouseup', onUp);
          }}
        />
      </Show>

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
                {/* Modified dot indicator */}
                <Show when={aiOriginals[tab.path]}>
                  <span class="w-1.5 h-1.5 rounded-full bg-amber-400 shrink-0" title="Modified by AI (unsaved)" />
                </Show>
                <span
                  role="button"
                  onClick={(e) => closeTab(tab.path, e as unknown as MouseEvent)}
                  class="shrink-0 opacity-0 group-hover:opacity-100 hover:bg-gray-200 dark:hover:bg-gray-700 rounded p-0.5 transition-all leading-none"
                  title="Close tab"
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

        {/* Path bar + Reload button — always visible when a tab is open */}
        <Show when={activeTabPath()}>
          <div class="flex-none flex items-center gap-2 px-3 py-0.5 bg-white dark:bg-gray-900 border-b border-gray-100 dark:border-gray-800 min-h-[20px]">
            {/* Reload button — always visible when a tab is open */}
            <button
              onClick={() => {
                const p = activeTabPath();
                if (!p) return;
                setFileCache(p, null);
                if (previewCache[p]) setPreviewCache(produce(d => { delete d[p]; }));
                invoke<FvFileContent>('fv_read_file', { path: p })
                  .then(r => setFileCache(p, r))
                  .catch(e => setFileCache(p, { text: `⚠ Could not load file\n\n${e}`, size: 0, is_binary: false, language: 'plaintext', line_count: 0 }));
              }}
              title="Reload from disk"
              class="shrink-0 p-0.5 rounded text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
            >
              <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
              </svg>
            </button>
            <span class="text-[10px] text-gray-400 font-mono truncate select-all flex-1" title={activeTabPath()!}>
              {activeTabPath()}
            </span>
            <button
              onClick={() => {
                const p = activeTabPath();
                if (p) navigator.clipboard.writeText(p);
              }}
              title="Copy path"
              class="shrink-0 text-[10px] text-gray-400 hover:text-gray-600 dark:hover:text-gray-200"
            >
              ⎘
            </button>
          </div>
        </Show>

        {/* View-mode toggle bar + AI button — only when a file with content is active */}
        <Show when={activeTabPath() && !!fileCache[activeTabPath()!]}>
          <div class="flex-none flex items-center justify-between px-3 py-1.5 bg-white dark:bg-gray-900 border-b border-gray-200 dark:border-gray-700">
            {/* View mode buttons */}
            <div class="flex items-center gap-1">
              {/* View mode buttons — only for previewable files */}
              <Show when={isPreviewable(activeTabPath()!)}>
                <span class="text-[10px] text-gray-400 mr-1 ml-1">View:</span>
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
              </Show>
              {/* Diff button — only when file has AI modifications */}
              <Show when={activeTabPath() && aiOriginals[activeTabPath()!]}>
                <button
                  onClick={() => setViewMode('diff')}
                  class="px-2.5 py-0.5 rounded text-xs font-medium transition-colors"
                  classList={{
                    'bg-sky-500 text-white': viewMode() === 'diff',
                    'text-gray-500 dark:text-gray-400 hover:bg-gray-100 dark:hover:bg-gray-800': viewMode() !== 'diff',
                  }}
                >⊕ Diff</button>
              </Show>
            </div>

            {/* AI Fix button — only for MD files */}
            <Show when={activeTabPath() && !!fileCache[activeTabPath()!] && (fileExt(activeTabPath()!) === 'md' || fileExt(activeTabPath()!) === 'markdown' || fileExt(activeTabPath()!) === 'mdx')}>
              <div class="flex items-center gap-1">
                <Show when={aiError()}>
                  <span class="text-xs text-red-500 ml-1">{aiError()}</span>
                </Show>
                <Show when={aiOriginals[activeTabPath()!]}>
                  <button
                    onClick={() => {
                      const p = activeTabPath()!;
                      const orig = aiOriginals[p];
                      if (orig) {
                        const cur = fileCache[p];
                        if (cur) setFileCache(p, { ...cur, text: orig });
                        setAiOriginals(produce(d => { delete d[p]; }));
                      }
                    }}
                    class="px-2 py-0.5 text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
                  >↩ Revert</button>
                </Show>
                <button
                  disabled={aiWorking()}
                  onClick={async () => {
                    const p = activeTabPath();
                    if (!p) return;
                    const cur = fileCache[p];
                    if (!cur || cur.is_binary) return;
                    setAiError(null);
                    setAiWorking(true);
                    try {
                      const fixed = await invoke<string>('fv_format_md', { markdown: cur.text });
                      if (!aiOriginals[p]) setAiOriginals(p, cur.text);
                      const lines = fixed.split('\n').length;
                      setFileCache(p, { ...cur, text: fixed, line_count: lines });
                      if (previewCache[p]) setPreviewCache(produce(d => { delete d[p]; }));
                    } catch (e) {
                      setAiError(`AI fix failed: ${e}`);
                    } finally {
                      setAiWorking(false);
                    }
                  }}
                  class="flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium bg-purple-100 dark:bg-purple-900/40 text-purple-700 dark:text-purple-300 hover:bg-purple-200 dark:hover:bg-purple-800 disabled:opacity-50 transition-colors"
                >
                  <Show when={aiWorking()} fallback={<span>✨ AI Fix</span>}>
                    <span class="animate-pulse">⏳ Fixing…</span>
                  </Show>
                </button>
              </div>
            </Show>
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

      {/* Tab eviction toast */}
      <Show when={evictedTab()}>
        <div class="fixed bottom-4 left-1/2 -translate-x-1/2 z-50 bg-gray-800 text-white text-xs px-4 py-2 rounded shadow-lg">
          Tab "{evictedTab()}" closed (max {MAX_TABS} tabs)
        </div>
      </Show>

      {/* Context menu */}
      <Show when={contextMenu()}>
        <div
          class="fixed z-[100] bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded shadow-xl py-1 min-w-[160px] text-xs"
          style={{ left: `${contextMenu()!.x}px`, top: `${contextMenu()!.y}px` }}
          onClick={(e) => e.stopPropagation()}
        >
          <button class="w-full text-left px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700"
            onClick={() => { void openFile(contextMenu()!.entry); setContextMenu(null); }}>
            Open
          </button>
          <button class="w-full text-left px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700"
            onClick={() => { navigator.clipboard.writeText(contextMenu()!.entry.path); setContextMenu(null); }}>
            Copy Path
          </button>
          <button class="w-full text-left px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700"
            onClick={() => {
              const entryPath = contextMenu()!.entry.path;
              const ws = workspaces().find(w => entryPath.startsWith(w.path));
              const rel = ws ? entryPath.slice(ws.path.length + 1) : entryPath;
              navigator.clipboard.writeText(rel);
              setContextMenu(null);
            }}>
            Copy Relative Path
          </button>
          <div class="border-t border-gray-200 dark:border-gray-700 my-1" />
          <button class="w-full text-left px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700"
            onClick={() => {
              const entry = contextMenu()!.entry;
              const dir = entry.is_dir ? entry.path : entry.path.substring(0, entry.path.lastIndexOf('/'));
              shellOpen(dir).catch(() => {});
              setContextMenu(null);
            }}>
            Reveal in File Manager
          </button>
          <Show when={!contextMenu()!.entry.is_dir}>
            <button class="w-full text-left px-3 py-1.5 hover:bg-gray-100 dark:hover:bg-gray-700"
              onClick={() => { shellOpen(contextMenu()!.entry.path).catch(() => {}); setContextMenu(null); }}>
              Open with System App
            </button>
          </Show>
        </div>
      </Show>

    </div>
  );
};

export default Explorer;
