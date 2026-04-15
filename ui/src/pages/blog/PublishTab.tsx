import { Component, createSignal, createMemo, For, Show, onMount } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { open as openUrl } from '@tauri-apps/plugin-shell';

interface BlogPost { id: string; title: string; slug: string; status: string; updated_at: string; }
interface PlatformAccount {
  id: string; platform: string; account_label: string | null; base_url: string | null;
  publication_id: string | null; default_tags: string[] | null;
  enabled: boolean; created_at: string; has_key: boolean;
}
interface Publication {
  id: string; post_id: string; platform: string; account_id: string | null;
  status: string | null; remote_id: string | null; remote_url: string | null;
  canonical_url: string | null; published_at: string | null;
  last_synced_at: string | null; error: string | null;
}
interface ExportPayload {
  platform: string; format: 'markdown' | 'html' | 'text';
  copy_text: string; open_url: string | null;
}

const AUTO_PLATFORMS = ['wordpress', 'devto', 'hashnode'];
const MANUAL_PLATFORMS = ['linkedin', 'medium', 'substack', 'twitter'];

function normalisePlatform(p: string): string {
  return p === 'dev_to' ? 'devto' : p;
}

const PLATFORM_LABELS: Record<string, string> = {
  wordpress: 'WordPress',
  devto: 'Dev.to',
  hashnode: 'Hashnode',
  medium: 'Medium',
  substack: 'Substack',
  linkedin: 'LinkedIn',
  twitter: 'Twitter / X',
};
const platformLabel = (p: string) => PLATFORM_LABELS[normalisePlatform(p)] ?? p;
const fmtTimestamp = (s: string | null) => {
  if (!s) return '—';
  try { return new Date(s).toLocaleString(); } catch { return s; }
};

function statusClass(status: string | null | undefined): string {
  switch (status) {
    case 'published':
      return 'bg-emerald-100 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-300';
    case 'exported':
      return 'bg-sky-100 dark:bg-sky-900/30 text-sky-700 dark:text-sky-300';
    case 'pending':
    case 'draft':
      return 'bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300';
    case 'failed':
    case 'error':
      return 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300';
    default:
      return 'bg-gray-100 dark:bg-gray-800 text-gray-500';
  }
}

interface Row {
  key: string; platform: string; label: string;
  account: PlatformAccount | null; publication: Publication | null; auto: boolean;
}

const PublishTab: Component = () => {
  const [posts, setPosts] = createSignal<BlogPost[]>([]);
  const [postId, setPostId] = createSignal<string>('');
  const [accounts, setAccounts] = createSignal<PlatformAccount[]>([]);
  const [publications, setPublications] = createSignal<Publication[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [busy, setBusy] = createSignal<Record<string, boolean>>({});
  const [error, setError] = createSignal<string | null>(null);
  const [toast, setToast] = createSignal<string | null>(null);

  const [exportModal, setExportModal] = createSignal<ExportPayload | null>(null);
  const [exportUrl, setExportUrl] = createSignal('');
  const [exportPlatform, setExportPlatform] = createSignal('');
  const [marking, setMarking] = createSignal(false);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 4000);
  };

  const loadPostsAndAccounts = async () => {
    try {
      const [pList, aList] = await Promise.all([
        invoke<BlogPost[]>('blog_list_posts', { status: null }),
        invoke<PlatformAccount[]>('blog_list_platform_accounts', {}),
      ]);
      setPosts(pList);
      setAccounts(aList);
      if (!postId() && pList.length > 0) {
        setPostId(pList[0].id);
      }
    } catch (e) {
      setError(String(e));
    }
  };

  const loadPublications = async () => {
    const id = postId();
    if (!id) {
      setPublications([]);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const pubs = await invoke<Publication[]>('blog_list_publications', { post_id: id });
      setPublications(pubs);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  onMount(async () => {
    await loadPostsAndAccounts();
    await loadPublications();
  });

  const onSelectPost = async (id: string) => {
    setPostId(id);
    await loadPublications();
  };

  const rows = createMemo<Row[]>(() => {
    const out: Row[] = [];
    const pubs = publications();
    for (const a of accounts()) {
      const p = normalisePlatform(a.platform);
      const pub =
        pubs.find((x) => x.account_id === a.id) ??
        pubs.find((x) => normalisePlatform(x.platform) === p && x.account_id === null) ??
        null;
      out.push({
        key: `account:${a.id}`, platform: p,
        label: platformLabel(a.platform) + (a.account_label ? ` (${a.account_label})` : ''),
        account: a, publication: pub, auto: AUTO_PLATFORMS.includes(p),
      });
    }
    for (const mp of MANUAL_PLATFORMS) {
      if (accounts().some((a) => normalisePlatform(a.platform) === mp)) continue;
      const pub = pubs.find((x) => normalisePlatform(x.platform) === mp) ?? null;
      out.push({
        key: `manual:${mp}`, platform: mp, label: platformLabel(mp),
        account: null, publication: pub, auto: false,
      });
    }
    return out;
  });

  const setBusyKey = (key: string, v: boolean) =>
    setBusy((prev) => {
      const n = { ...prev };
      if (v) n[key] = true;
      else delete n[key];
      return n;
    });

  const doPublish = async (row: Row) => {
    if (!row.account) return;
    setBusyKey(row.key, true);
    setError(null);
    try {
      const res = await invoke<{ remote_url: string | null; status: string }>(
        'blog_publish_to_platform',
        { post_id: postId(), account_id: row.account.id },
      );
      showToast(
        res.remote_url
          ? `Published to ${platformLabel(row.platform)} (${res.status})`
          : `Published: ${res.status}`,
      );
      await loadPublications();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusyKey(row.key, false);
    }
  };

  const doExport = async (row: Row) => {
    setBusyKey(row.key, true);
    setError(null);
    try {
      const payload = await invoke<ExportPayload>('blog_export_for_platform', {
        post_id: postId(),
        platform: row.platform,
      });
      setExportPlatform(row.platform);
      setExportUrl('');
      setExportModal(payload);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusyKey(row.key, false);
    }
  };

  const doUnpublish = async (row: Row) => {
    if (!row.publication) return;
    const ok = window.confirm(
      `Unpublish from ${platformLabel(row.platform)}? This removes the publication record.`,
    );
    if (!ok) return;
    setBusyKey(row.key, true);
    setError(null);
    try {
      await invoke<void>('blog_unpublish', { publication_id: row.publication.id });
      showToast('Unpublished');
      await loadPublications();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusyKey(row.key, false);
    }
  };

  const copyExport = async () => {
    const p = exportModal();
    if (!p) return;
    try {
      await navigator.clipboard.writeText(p.copy_text);
      showToast('Copied to clipboard');
    } catch (e) {
      setError(String(e));
    }
  };

  const openExportUrl = async () => {
    const p = exportModal();
    if (!p?.open_url) return;
    try {
      await openUrl(p.open_url);
    } catch (e) {
      setError(String(e));
    }
  };

  const markExported = async () => {
    const platform = exportPlatform();
    if (!platform) return;
    setMarking(true);
    try {
      await invoke<Publication>('blog_mark_exported', {
        post_id: postId(),
        platform,
        remote_url: exportUrl().trim() || null,
      });
      showToast('Marked as exported');
      setExportModal(null);
      setExportPlatform('');
      setExportUrl('');
      await loadPublications();
    } catch (e) {
      setError(String(e));
    } finally {
      setMarking(false);
    }
  };

  return (
    <div class="px-8 py-6">
      <div class="flex items-center justify-between mb-4">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Publish</h2>
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

      <div class="card p-4 mb-4">
        <label class="block text-xs text-gray-500 mb-1">Post</label>
        <Show
          when={posts().length > 0}
          fallback={<div class="text-sm text-gray-400">No posts yet.</div>}
        >
          <select
            class="w-full max-w-xl px-2 py-1.5 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
            value={postId()}
            onChange={(e) => onSelectPost(e.currentTarget.value)}
          >
            <For each={posts()}>
              {(p) => (
                <option value={p.id}>
                  {p.title} · {p.status}
                </option>
              )}
            </For>
          </select>
        </Show>
      </div>

      <Show when={postId()}>
        <div class="card p-0 overflow-hidden">
          <table class="w-full text-sm">
            <thead class="bg-gray-50 dark:bg-gray-800 text-xs uppercase text-gray-500">
              <tr>
                <th class="text-left p-3">Platform</th>
                <th class="text-left p-3">Status</th>
                <th class="text-left p-3">Remote URL</th>
                <th class="text-left p-3">Last synced</th>
                <th class="text-right p-3">Action</th>
              </tr>
            </thead>
            <tbody>
              <Show when={loading()}>
                <tr>
                  <td colspan="5" class="p-6 text-center text-sm text-gray-400">
                    Loading…
                  </td>
                </tr>
              </Show>
              <Show when={!loading() && rows().length === 0}>
                <tr>
                  <td colspan="5" class="p-6 text-center text-sm text-gray-500">
                    No platforms configured. Add a platform account in the Platforms tab.
                  </td>
                </tr>
              </Show>
              <For each={rows()}>
                {(row) => {
                  const pub = () => row.publication;
                  const canAutoPublish = () =>
                    row.auto && row.account?.has_key;
                  return (
                    <tr class="border-t border-gray-100 dark:border-gray-800">
                      <td class="p-3">
                        <div class="font-medium">{row.label}</div>
                        <div class="text-[10px] text-gray-400">
                          {row.auto ? 'Auto-publish' : 'Manual export'}
                        </div>
                      </td>
                      <td class="p-3">
                        <Show
                          when={pub()}
                          fallback={
                            <span class={`text-[10px] px-1.5 py-0.5 rounded ${statusClass(null)}`}>
                              not published
                            </span>
                          }
                        >
                          <span class={`text-[10px] px-1.5 py-0.5 rounded ${statusClass(pub()!.status)}`}>
                            {pub()!.status ?? 'unknown'}
                          </span>
                          <Show when={pub()!.error}>
                            <div
                              class="text-[10px] text-red-500 mt-1 max-w-[16rem] truncate"
                              title={pub()!.error ?? ''}
                            >
                              {pub()!.error}
                            </div>
                          </Show>
                        </Show>
                      </td>
                      <td class="p-3">
                        <Show when={pub()?.remote_url} fallback={<span class="text-gray-400">—</span>}>
                          <a
                            href={pub()!.remote_url!}
                            target="_blank"
                            rel="noreferrer"
                            class="text-minion-600 hover:underline text-xs font-mono truncate inline-block max-w-[20rem]"
                          >
                            {pub()!.remote_url}
                          </a>
                        </Show>
                      </td>
                      <td class="p-3 text-xs text-gray-500">
                        {fmtTimestamp(pub()?.last_synced_at ?? pub()?.published_at ?? null)}
                      </td>
                      <td class="p-3 text-right space-x-2 whitespace-nowrap">
                        <Show when={canAutoPublish()}>
                          <button
                            class="btn-primary text-xs"
                            onClick={() => doPublish(row)}
                            disabled={busy()[row.key]}
                          >
                            {busy()[row.key]
                              ? 'Publishing…'
                              : pub()?.status === 'published'
                              ? 'Re-publish'
                              : 'Publish'}
                          </button>
                        </Show>
                        <Show when={!canAutoPublish()}>
                          <button
                            class="px-2 py-1 text-xs border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
                            onClick={() => doExport(row)}
                            disabled={busy()[row.key]}
                          >
                            {busy()[row.key] ? 'Exporting…' : 'Export'}
                          </button>
                        </Show>
                        <Show when={pub()}>
                          <button
                            class="px-2 py-1 text-xs border border-red-300 dark:border-red-700 text-red-700 dark:text-red-400 rounded hover:bg-red-50 dark:hover:bg-red-900/10"
                            onClick={() => doUnpublish(row)}
                            disabled={busy()[row.key]}
                          >
                            Unpublish
                          </button>
                        </Show>
                      </td>
                    </tr>
                  );
                }}
              </For>
            </tbody>
          </table>
        </div>
      </Show>

      {/* Export modal */}
      <Show when={exportModal()}>
        <div
          class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4"
          onClick={() => setExportModal(null)}
        >
          <div
            class="bg-white dark:bg-gray-900 rounded-xl max-w-3xl w-full max-h-[90vh] overflow-y-auto"
            onClick={(e) => e.stopPropagation()}
          >
            <div class="flex items-center justify-between p-4 border-b border-gray-200 dark:border-gray-700">
              <h3 class="font-semibold">
                Export for {platformLabel(exportModal()!.platform)}
              </h3>
              <button
                class="text-gray-500 hover:text-gray-800 dark:hover:text-gray-200"
                onClick={() => setExportModal(null)}
              >
                ✕
              </button>
            </div>
            <div class="p-4 space-y-3">
              <div class="text-xs text-gray-500">
                Format: <span class="font-mono">{exportModal()!.format}</span>
              </div>
              <pre class="text-xs bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded p-3 max-h-80 overflow-auto whitespace-pre-wrap font-mono">
                {exportModal()!.copy_text}
              </pre>
              <div class="flex flex-wrap gap-2">
                <button class="btn-primary text-sm" onClick={copyExport}>
                  Copy to clipboard
                </button>
                <Show when={exportModal()!.open_url}>
                  <button
                    class="px-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
                    onClick={openExportUrl}
                  >
                    Open editor
                  </button>
                </Show>
              </div>
              <div class="pt-3 border-t border-gray-200 dark:border-gray-700">
                <label class="block text-xs text-gray-500 mb-1">
                  Public URL (optional, for your records)
                </label>
                <input
                  type="text"
                  placeholder="https://…"
                  value={exportUrl()}
                  onInput={(e) => setExportUrl(e.currentTarget.value)}
                  class="w-full px-2 py-1.5 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
                />
                <div class="flex justify-end gap-2 mt-3">
                  <button
                    class="px-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
                    onClick={() => setExportModal(null)}
                  >
                    Close
                  </button>
                  <button
                    class="btn-primary text-sm"
                    onClick={markExported}
                    disabled={marking()}
                  >
                    {marking() ? 'Saving…' : 'Mark as exported'}
                  </button>
                </div>
              </div>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default PublishTab;
