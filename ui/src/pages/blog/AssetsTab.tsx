import { Component, createSignal, For, Show, onMount } from 'solid-js';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';

interface BlogAsset {
  id: string;
  sha256: string;
  stored_path: string;
  original_filename: string | null;
  mime_type: string | null;
  width: number | null;
  height: number | null;
  size_bytes: number | null;
  created_at: string;
  use_count: number;
}

interface AssetUsage {
  post_id: string;
  post_title: string;
  post_slug: string;
  referenced_as: string;
}

interface DeleteResult {
  deleted_db_rows: number;
  deleted_files: number;
  freed_bytes: number;
  errors: string[];
}

function fmtBytes(b: number | null): string {
  if (b == null) return '—';
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
  if (b < 1024 * 1024 * 1024) return `${(b / (1024 * 1024)).toFixed(1)} MB`;
  return `${(b / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function extFromAsset(a: BlogAsset): string {
  const name = a.original_filename || a.stored_path;
  const m = /\.([a-z0-9]+)$/i.exec(name);
  return m ? m[1].toLowerCase() : 'bin';
}

const AssetsTab: Component = () => {
  const [assets, setAssets] = createSignal<BlogAsset[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [toast, setToast] = createSignal<string | null>(null);
  const [thumbs, setThumbs] = createSignal<Record<string, string>>({});
  const [uploading, setUploading] = createSignal(false);
  const [deletingOrphans, setDeletingOrphans] = createSignal(false);
  const [confirmDelete, setConfirmDelete] = createSignal(false);

  const [detail, setDetail] = createSignal<BlogAsset | null>(null);
  const [detailUsage, setDetailUsage] = createSignal<AssetUsage[]>([]);
  const [detailLoading, setDetailLoading] = createSignal(false);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 4000);
  };

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await invoke<BlogAsset[]>('blog_list_assets', {});
      setAssets(list);
      // Fetch thumbnail paths in parallel
      const next: Record<string, string> = { ...thumbs() };
      await Promise.all(
        list.map(async (a) => {
          if (next[a.id]) return;
          try {
            const p = await invoke<string>('blog_get_asset_path', { asset_id: a.id });
            next[a.id] = convertFileSrc(p);
          } catch {
            // leave empty
          }
        }),
      );
      setThumbs(next);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  onMount(load);

  const uploadAsset = async () => {
    setError(null);
    try {
      const selected = await openDialog({
        multiple: false,
        directory: false,
        title: 'Pick an image or file to upload',
      });
      if (!selected || typeof selected !== 'string') return;
      setUploading(true);
      await invoke<BlogAsset>('blog_upload_asset', { file_path: selected });
      showToast('Asset uploaded');
      await load();
    } catch (e) {
      setError(String(e));
    } finally {
      setUploading(false);
    }
  };

  const deleteOrphans = async () => {
    setConfirmDelete(false);
    setDeletingOrphans(true);
    setError(null);
    try {
      const res = await invoke<DeleteResult>('blog_delete_orphan_assets', {});
      showToast(
        `Deleted ${res.deleted_db_rows} rows, ${res.deleted_files} files, freed ${fmtBytes(
          res.freed_bytes,
        )}`,
      );
      if (res.errors.length > 0) {
        setError(res.errors.join('; '));
      }
      await load();
    } catch (e) {
      setError(String(e));
    } finally {
      setDeletingOrphans(false);
    }
  };

  const openDetail = async (a: BlogAsset) => {
    setDetail(a);
    setDetailUsage([]);
    setDetailLoading(true);
    try {
      const usage = await invoke<AssetUsage[]>('blog_get_asset_usage', { asset_id: a.id });
      setDetailUsage(usage);
    } catch (e) {
      setError(String(e));
    } finally {
      setDetailLoading(false);
    }
  };

  const closeDetail = () => {
    setDetail(null);
    setDetailUsage([]);
  };

  const copyMarkdown = async () => {
    const a = detail();
    if (!a) return;
    const ext = extFromAsset(a);
    const name = a.original_filename || `${a.sha256}.${ext}`;
    const md = `![${name}](assets/${a.sha256}.${ext})`;
    try {
      await navigator.clipboard.writeText(md);
      showToast('Markdown copied to clipboard');
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div class="px-8 py-6">
      <div class="flex items-center justify-between mb-4">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Assets</h2>
        <div class="flex gap-2">
          <button class="btn-primary text-sm" onClick={uploadAsset} disabled={uploading()}>
            {uploading() ? 'Uploading…' : 'Upload asset'}
          </button>
          <button
            class="px-3 py-1.5 text-sm border border-red-300 dark:border-red-700 text-red-700 dark:text-red-400 rounded hover:bg-red-50 dark:hover:bg-red-900/10"
            onClick={() => setConfirmDelete(true)}
            disabled={deletingOrphans()}
          >
            {deletingOrphans() ? 'Deleting…' : 'Delete orphans'}
          </button>
        </div>
      </div>

      <Show when={error()}>
        <div class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
          <p class="text-sm text-red-700 dark:text-red-300">{error()}</p>
        </div>
      </Show>
      <Show when={toast()}>
        <div class="mb-4 card p-3 text-sm text-green-700 dark:text-green-400 border-l-4 border-green-500">
          {toast()}
        </div>
      </Show>

      <Show when={loading() && assets().length === 0}>
        <div class="text-center py-12 text-gray-400">Loading…</div>
      </Show>

      <Show when={!loading() && assets().length === 0}>
        <div class="card p-10 text-center text-sm text-gray-500">
          No assets yet. Upload an image or reference an image from a post you import.
        </div>
      </Show>

      <Show when={assets().length > 0}>
        <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-4">
          <For each={assets()}>
            {(a) => (
              <button
                class="card p-3 text-left hover:border-minion-400 transition-colors flex flex-col"
                onClick={() => openDetail(a)}
              >
                <div class="w-full aspect-square bg-gray-100 dark:bg-gray-800 rounded flex items-center justify-center overflow-hidden">
                  <Show
                    when={
                      thumbs()[a.id] && (a.mime_type?.startsWith('image/') ?? true)
                    }
                    fallback={
                      <div class="text-xs text-gray-400 uppercase font-mono">
                        {extFromAsset(a)}
                      </div>
                    }
                  >
                    <img
                      src={thumbs()[a.id]}
                      alt={a.original_filename ?? a.sha256}
                      class="w-full h-full object-contain"
                      style={{ 'max-width': '160px', 'max-height': '160px' }}
                    />
                  </Show>
                </div>
                <div
                  class="mt-2 text-xs font-mono truncate"
                  title={a.original_filename ?? a.sha256}
                >
                  {a.original_filename ?? a.sha256.slice(0, 12)}
                </div>
                <div class="flex items-center justify-between text-[10px] text-gray-500 mt-1">
                  <span>{fmtBytes(a.size_bytes)}</span>
                  <span
                    class="px-1.5 py-0.5 rounded bg-gray-100 dark:bg-gray-800"
                    title="usage count"
                  >
                    {a.use_count} use{a.use_count === 1 ? '' : 's'}
                  </span>
                </div>
              </button>
            )}
          </For>
        </div>
      </Show>

      {/* Detail modal */}
      <Show when={detail()}>
        <div
          class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4"
          onClick={closeDetail}
        >
          <div
            class="bg-white dark:bg-gray-900 rounded-xl max-w-3xl w-full max-h-[90vh] overflow-y-auto"
            onClick={(e) => e.stopPropagation()}
          >
            <div class="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
              <h3 class="font-semibold">
                {detail()!.original_filename ?? detail()!.sha256.slice(0, 16)}
              </h3>
              <button
                class="text-gray-500 hover:text-gray-800 dark:hover:text-gray-200"
                onClick={closeDetail}
              >
                ✕
              </button>
            </div>
            <div class="p-4 grid md:grid-cols-2 gap-4">
              <div class="bg-gray-100 dark:bg-gray-800 rounded flex items-center justify-center p-2 min-h-[16rem]">
                <Show
                  when={
                    thumbs()[detail()!.id] &&
                    (detail()!.mime_type?.startsWith('image/') ?? true)
                  }
                  fallback={
                    <div class="text-sm text-gray-400 uppercase font-mono">
                      {extFromAsset(detail()!)} file
                    </div>
                  }
                >
                  <img
                    src={thumbs()[detail()!.id]}
                    alt={detail()!.original_filename ?? detail()!.sha256}
                    class="max-w-full max-h-[60vh] object-contain"
                  />
                </Show>
              </div>
              <div class="text-xs">
                <table class="w-full">
                  <tbody>
                    <tr class="border-b border-gray-100 dark:border-gray-800">
                      <td class="py-1 text-gray-500">sha256</td>
                      <td class="py-1 font-mono break-all">{detail()!.sha256}</td>
                    </tr>
                    <tr class="border-b border-gray-100 dark:border-gray-800">
                      <td class="py-1 text-gray-500">mime</td>
                      <td class="py-1 font-mono">{detail()!.mime_type ?? '—'}</td>
                    </tr>
                    <tr class="border-b border-gray-100 dark:border-gray-800">
                      <td class="py-1 text-gray-500">size</td>
                      <td class="py-1 font-mono">{fmtBytes(detail()!.size_bytes)}</td>
                    </tr>
                    <tr class="border-b border-gray-100 dark:border-gray-800">
                      <td class="py-1 text-gray-500">dimensions</td>
                      <td class="py-1 font-mono">
                        <Show
                          when={detail()!.width && detail()!.height}
                          fallback="—"
                        >
                          {detail()!.width} × {detail()!.height}
                        </Show>
                      </td>
                    </tr>
                    <tr class="border-b border-gray-100 dark:border-gray-800">
                      <td class="py-1 text-gray-500">stored</td>
                      <td class="py-1 font-mono break-all">{detail()!.stored_path}</td>
                    </tr>
                    <tr>
                      <td class="py-1 text-gray-500">uses</td>
                      <td class="py-1 font-mono">{detail()!.use_count}</td>
                    </tr>
                  </tbody>
                </table>
                <button
                  class="mt-3 btn-primary text-sm w-full"
                  onClick={copyMarkdown}
                >
                  Copy markdown
                </button>
              </div>
            </div>
            <div class="p-4 border-t border-gray-200 dark:border-gray-700">
              <h4 class="text-sm font-medium mb-2">Used by</h4>
              <Show when={detailLoading()}>
                <div class="text-xs text-gray-500">Loading…</div>
              </Show>
              <Show when={!detailLoading() && detailUsage().length === 0}>
                <div class="text-xs text-gray-500">
                  Not currently referenced by any post (orphan).
                </div>
              </Show>
              <Show when={!detailLoading() && detailUsage().length > 0}>
                <ul class="space-y-1">
                  <For each={detailUsage()}>
                    {(u) => (
                      <li class="flex items-start justify-between text-xs border-b border-gray-100 dark:border-gray-800 py-1">
                        <div>
                          <div class="font-medium">{u.post_title}</div>
                          <div class="text-gray-400 font-mono">/{u.post_slug}</div>
                        </div>
                        <code class="text-gray-500 text-[10px] ml-2">
                          {u.referenced_as}
                        </code>
                      </li>
                    )}
                  </For>
                </ul>
              </Show>
            </div>
          </div>
        </div>
      </Show>

      {/* Confirm-delete modal */}
      <Show when={confirmDelete()}>
        <div
          class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4"
          onClick={() => setConfirmDelete(false)}
        >
          <div
            class="bg-white dark:bg-gray-900 rounded-xl max-w-md w-full p-5"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 class="font-semibold mb-2">Delete orphan assets?</h3>
            <p class="text-sm text-gray-600 dark:text-gray-300 mb-4">
              This permanently removes any asset with zero post references, from the database
              and disk. Cannot be undone.
            </p>
            <div class="flex justify-end gap-2">
              <button
                class="px-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
                onClick={() => setConfirmDelete(false)}
              >
                Cancel
              </button>
              <button
                class="px-3 py-1.5 text-sm bg-red-600 hover:bg-red-700 text-white rounded"
                onClick={deleteOrphans}
              >
                Delete orphans
              </button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default AssetsTab;
