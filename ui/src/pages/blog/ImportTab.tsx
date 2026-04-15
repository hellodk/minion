import { Component, createSignal, For, Show } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

interface ImportPreview {
  preview_id: string;
  source_path: string;
  title: string;
  slug: string;
  content: string;
  excerpt: string | null;
  date: string | null;
  status: string;
  canonical_url: string | null;
  suggested_tags: string[];
  image_references: string[];
  had_frontmatter: boolean;
  already_exists: boolean;
  body_char_count: number;
}

interface ConfirmImportEntry {
  preview_id: string;
  source_path: string;
  title: string;
  slug: string;
  content: string;
  excerpt?: string | null;
  date?: string | null;
  status: string;
  canonical_url?: string | null;
  tags: string[];
  author?: string | null;
}

interface ImportResult {
  imported: number;
  skipped: number;
  failed: number;
  post_ids: string[];
  errors: string[];
}

type Step = 'pick' | 'review' | 'done';

interface RowEdits {
  selected: boolean;
  title: string;
  tagsText: string;
}

function shortPath(path: string, max = 60): string {
  if (path.length <= max) return path;
  return '…' + path.slice(path.length - (max - 1));
}

const ImportTab: Component<{ onDone?: () => void }> = (props) => {
  const [step, setStep] = createSignal<Step>('pick');
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [previews, setPreviews] = createSignal<ImportPreview[]>([]);
  const [edits, setEdits] = createSignal<Record<string, RowEdits>>({});
  const [author, setAuthor] = createSignal('');
  const [importing, setImporting] = createSignal(false);
  const [result, setResult] = createSignal<ImportResult | null>(null);

  const applyPreviews = (list: ImportPreview[]) => {
    const e: Record<string, RowEdits> = {};
    for (const p of list) {
      e[p.preview_id] = {
        selected: !p.already_exists,
        title: p.title,
        tagsText: p.suggested_tags.join(', '),
      };
    }
    setPreviews(list);
    setEdits(e);
    setStep('review');
  };

  const pickFiles = async () => {
    setError(null);
    try {
      const selected = await open({
        multiple: true,
        title: 'Pick markdown / HTML files to import',
        filters: [
          { name: 'Markdown/HTML', extensions: ['md', 'markdown', 'txt', 'html'] },
        ],
      });
      if (!selected) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      if (paths.length === 0) return;
      setLoading(true);
      const list = await invoke<ImportPreview[]>('blog_import_files', { paths });
      applyPreviews(list);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const pickFolder = async () => {
    setError(null);
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Pick folder to import posts from',
      });
      if (!selected || typeof selected !== 'string') return;
      setLoading(true);
      const list = await invoke<ImportPreview[]>('blog_import_folder', { path: selected });
      applyPreviews(list);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const updateEdit = (id: string, patch: Partial<RowEdits>) => {
    setEdits((prev) => ({ ...prev, [id]: { ...prev[id], ...patch } }));
  };

  const selectedCount = () =>
    Object.values(edits()).filter((e) => e.selected).length;

  const selectAll = () => {
    setEdits((prev) => {
      const next: Record<string, RowEdits> = {};
      for (const [k, v] of Object.entries(prev)) next[k] = { ...v, selected: true };
      return next;
    });
  };

  const deselectAll = () => {
    setEdits((prev) => {
      const next: Record<string, RowEdits> = {};
      for (const [k, v] of Object.entries(prev)) next[k] = { ...v, selected: false };
      return next;
    });
  };

  const runImport = async () => {
    setError(null);
    setImporting(true);
    try {
      const entries: ConfirmImportEntry[] = [];
      for (const p of previews()) {
        const e = edits()[p.preview_id];
        if (!e || !e.selected) continue;
        const tags = e.tagsText
          .split(',')
          .map((t) => t.trim())
          .filter(Boolean);
        entries.push({
          preview_id: p.preview_id,
          source_path: p.source_path,
          title: e.title.trim() || p.title,
          slug: p.slug,
          content: p.content,
          excerpt: p.excerpt,
          date: p.date,
          status: p.status,
          canonical_url: p.canonical_url,
          tags,
        });
      }
      if (entries.length === 0) {
        setError('Select at least one post to import.');
        setImporting(false);
        return;
      }
      const res = await invoke<ImportResult>('blog_confirm_import', {
        entries,
        author: author().trim() || null,
      });
      setResult(res);
      setStep('done');
    } catch (e) {
      setError(String(e));
    } finally {
      setImporting(false);
    }
  };

  const reset = () => {
    setStep('pick');
    setPreviews([]);
    setEdits({});
    setResult(null);
    setError(null);
  };

  return (
    <div class="px-8 py-6">
      <div class="flex items-center justify-between mb-4">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Import posts</h2>
        <div class="flex items-center gap-2 text-xs text-gray-500">
          <span classList={{ 'font-semibold text-minion-600': step() === 'pick' }}>
            1. Pick source
          </span>
          <span>→</span>
          <span classList={{ 'font-semibold text-minion-600': step() === 'review' }}>
            2. Review
          </span>
          <span>→</span>
          <span classList={{ 'font-semibold text-minion-600': step() === 'done' }}>
            3. Done
          </span>
        </div>
      </div>

      <Show when={error()}>
        <div class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
          <p class="text-sm text-red-700 dark:text-red-300">{error()}</p>
        </div>
      </Show>

      <Show when={step() === 'pick'}>
        <div class="card p-6 text-center">
          <p class="text-sm text-gray-600 dark:text-gray-300 mb-4">
            Import markdown or HTML files individually, or scan a folder recursively. Files
            with YAML frontmatter will preserve their metadata.
          </p>
          <div class="flex gap-2 justify-center">
            <button class="btn-primary text-sm" onClick={pickFiles} disabled={loading()}>
              {loading() ? 'Scanning…' : 'Pick files'}
            </button>
            <button
              class="px-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
              onClick={pickFolder}
              disabled={loading()}
            >
              {loading() ? 'Scanning…' : 'Pick folder'}
            </button>
          </div>
        </div>
      </Show>

      <Show when={step() === 'review'}>
        <div class="card p-4">
          <div class="flex flex-wrap items-center justify-between gap-2 mb-3">
            <div class="flex flex-wrap gap-2 text-xs">
              <span class="px-2 py-1 bg-gray-100 dark:bg-gray-800 rounded">
                Found: <strong>{previews().length}</strong>
              </span>
              <span class="px-2 py-1 bg-gray-100 dark:bg-gray-800 rounded">
                Duplicates:{' '}
                <strong>{previews().filter((p) => p.already_exists).length}</strong>
              </span>
              <span class="px-2 py-1 bg-minion-50 dark:bg-minion-900/30 text-minion-700 dark:text-minion-300 rounded">
                Selected: <strong>{selectedCount()}</strong>
              </span>
            </div>
            <div class="flex gap-2">
              <button
                class="px-2 py-1 text-xs border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
                onClick={selectAll}
              >
                Select all
              </button>
              <button
                class="px-2 py-1 text-xs border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
                onClick={deselectAll}
              >
                Deselect all
              </button>
            </div>
          </div>

          <div class="mb-3 flex items-center gap-2">
            <label class="text-xs text-gray-500">Author (applied to all):</label>
            <input
              type="text"
              placeholder="optional"
              value={author()}
              onInput={(e) => setAuthor(e.currentTarget.value)}
              class="flex-1 max-w-xs px-2 py-1 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
            />
          </div>

          <div class="overflow-x-auto max-h-[28rem] overflow-y-auto border border-gray-200 dark:border-gray-700 rounded">
            <table class="w-full text-sm">
              <thead class="bg-gray-50 dark:bg-gray-800 sticky top-0">
                <tr>
                  <th class="w-10 p-2"></th>
                  <th class="text-left p-2">Title</th>
                  <th class="text-left p-2">Tags</th>
                  <th class="text-left p-2">Badges</th>
                  <th class="text-left p-2">Source</th>
                </tr>
              </thead>
              <tbody>
                <For each={previews()}>
                  {(p) => {
                    const e = () => edits()[p.preview_id];
                    return (
                      <tr class="border-t border-gray-100 dark:border-gray-800 hover:bg-gray-50 dark:hover:bg-gray-800/50 align-top">
                        <td class="p-2">
                          <input
                            type="checkbox"
                            checked={e()?.selected ?? false}
                            onChange={(ev) =>
                              updateEdit(p.preview_id, {
                                selected: ev.currentTarget.checked,
                              })
                            }
                          />
                        </td>
                        <td class="p-2 min-w-[16rem]">
                          <input
                            type="text"
                            value={e()?.title ?? p.title}
                            onInput={(ev) =>
                              updateEdit(p.preview_id, { title: ev.currentTarget.value })
                            }
                            class="w-full px-2 py-1 text-sm border rounded bg-transparent border-gray-200 dark:border-gray-700"
                          />
                          <div class="text-[10px] text-gray-400 mt-1 font-mono">
                            /{p.slug}
                          </div>
                        </td>
                        <td class="p-2 min-w-[12rem]">
                          <input
                            type="text"
                            value={e()?.tagsText ?? ''}
                            onInput={(ev) =>
                              updateEdit(p.preview_id, {
                                tagsText: ev.currentTarget.value,
                              })
                            }
                            placeholder="comma, separated"
                            class="w-full px-2 py-1 text-sm border rounded bg-transparent border-gray-200 dark:border-gray-700"
                          />
                        </td>
                        <td class="p-2 space-x-1 whitespace-nowrap">
                          <Show when={p.had_frontmatter}>
                            <span class="inline-block text-[10px] px-1.5 py-0.5 rounded bg-sky-100 dark:bg-sky-900/30 text-sky-700 dark:text-sky-300">
                              frontmatter ✓
                            </span>
                          </Show>
                          <Show when={p.already_exists}>
                            <span class="inline-block text-[10px] px-1.5 py-0.5 rounded bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300">
                              duplicate
                            </span>
                          </Show>
                          <Show when={p.image_references.length > 0}>
                            <span
                              class="inline-block text-[10px] px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300"
                              title={p.image_references.join('\n')}
                            >
                              {p.image_references.length} img
                            </span>
                          </Show>
                          <span class="inline-block text-[10px] px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-500">
                            {p.body_char_count} ch
                          </span>
                        </td>
                        <td
                          class="p-2 text-xs font-mono text-gray-500"
                          title={p.source_path}
                        >
                          {shortPath(p.source_path, 50)}
                        </td>
                      </tr>
                    );
                  }}
                </For>
                <Show when={previews().length === 0}>
                  <tr>
                    <td colspan="5" class="p-6 text-center text-sm text-gray-500">
                      No importable files found.
                    </td>
                  </tr>
                </Show>
              </tbody>
            </table>
          </div>

          <div class="flex justify-end gap-2 mt-3">
            <button
              class="px-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
              onClick={reset}
            >
              Back
            </button>
            <button
              class="btn-primary text-sm"
              onClick={runImport}
              disabled={importing() || selectedCount() === 0}
            >
              {importing() ? 'Importing…' : `Import ${selectedCount()} selected`}
            </button>
          </div>
        </div>
      </Show>

      <Show when={step() === 'done' && result()}>
        <div class="card p-6 text-center max-w-2xl mx-auto">
          <div class="mb-4">
            <svg
              class="w-14 h-14 mx-auto text-green-500"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M5 13l4 4L19 7"
              />
            </svg>
          </div>
          <h3 class="text-lg font-bold mb-4">Import complete</h3>
          <div class="grid grid-cols-3 gap-3 mb-4">
            <div class="card p-3">
              <div class="text-xs text-gray-500">Imported</div>
              <div class="text-xl font-bold text-minion-600">{result()!.imported}</div>
            </div>
            <div class="card p-3">
              <div class="text-xs text-gray-500">Skipped</div>
              <div class="text-xl font-bold text-amber-600">{result()!.skipped}</div>
            </div>
            <div class="card p-3">
              <div class="text-xs text-gray-500">Failed</div>
              <div class="text-xl font-bold text-red-600">{result()!.failed}</div>
            </div>
          </div>
          <Show when={result()!.errors.length > 0}>
            <div class="text-left mt-3 border border-red-200 dark:border-red-800 rounded p-2 max-h-40 overflow-y-auto">
              <div class="text-xs font-medium text-red-600 mb-1">Errors</div>
              <ul class="text-xs text-red-600 dark:text-red-400 space-y-1 font-mono">
                <For each={result()!.errors}>{(e) => <li>{e}</li>}</For>
              </ul>
            </div>
          </Show>
          <div class="flex gap-2 justify-center mt-6">
            <button
              class="px-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
              onClick={reset}
            >
              Import more
            </button>
            <Show when={props.onDone}>
              <button class="btn-primary text-sm" onClick={() => props.onDone?.()}>
                View posts →
              </button>
            </Show>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default ImportTab;
