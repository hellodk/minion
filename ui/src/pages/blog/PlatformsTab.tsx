import { Component, createSignal, For, Show, onMount } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

interface PlatformAccount {
  id: string;
  platform: string;
  account_label: string | null;
  base_url: string | null;
  publication_id: string | null;
  default_tags: string[] | null;
  enabled: boolean;
  created_at: string;
  has_key: boolean;
}

const ALL_PLATFORMS = [
  { value: 'wordpress', label: 'WordPress', auto: true },
  { value: 'devto', label: 'Dev.to', auto: true },
  { value: 'hashnode', label: 'Hashnode', auto: true },
  { value: 'medium', label: 'Medium', auto: false },
  { value: 'substack', label: 'Substack', auto: false },
  { value: 'linkedin', label: 'LinkedIn', auto: false },
  { value: 'twitter', label: 'Twitter / X', auto: false },
];

function isAutoPlatform(p: string): boolean {
  const n = p === 'dev_to' ? 'devto' : p;
  return ALL_PLATFORMS.find((pp) => pp.value === n)?.auto ?? false;
}

function platformLabel(p: string): string {
  const n = p === 'dev_to' ? 'devto' : p;
  return ALL_PLATFORMS.find((pp) => pp.value === n)?.label ?? p;
}

const PlatformsTab: Component = () => {
  const [accounts, setAccounts] = createSignal<PlatformAccount[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [toast, setToast] = createSignal<string | null>(null);
  const [testStatus, setTestStatus] = createSignal<Record<string, 'pending' | 'ok' | 'fail'>>(
    {},
  );

  const [showAdd, setShowAdd] = createSignal(false);
  const [formPlatform, setFormPlatform] = createSignal<string>('wordpress');
  const [formLabel, setFormLabel] = createSignal('');
  const [formBaseUrl, setFormBaseUrl] = createSignal('');
  const [formApiKey, setFormApiKey] = createSignal('');
  const [formPubId, setFormPubId] = createSignal('');
  const [formDefaultTags, setFormDefaultTags] = createSignal('');
  const [saving, setSaving] = createSignal(false);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 4000);
  };

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await invoke<PlatformAccount[]>('blog_list_platform_accounts', {});
      setAccounts(list);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  onMount(load);

  const resetForm = () => {
    setFormPlatform('wordpress');
    setFormLabel('');
    setFormBaseUrl('');
    setFormApiKey('');
    setFormPubId('');
    setFormDefaultTags('');
  };

  const openAdd = () => {
    resetForm();
    setShowAdd(true);
  };

  const closeAdd = () => {
    setShowAdd(false);
    resetForm();
  };

  const formValid = (): boolean => {
    const p = formPlatform();
    if (p === 'wordpress') {
      return (
        formBaseUrl().trim().length > 0 &&
        formLabel().trim().length > 0 &&
        formApiKey().trim().length > 0
      );
    }
    if (p === 'devto') return formApiKey().trim().length > 0;
    if (p === 'hashnode')
      return formApiKey().trim().length > 0 && formPubId().trim().length > 0;
    return formLabel().trim().length > 0;
  };

  const runTest = async (id: string) => {
    setTestStatus((prev) => ({ ...prev, [id]: 'pending' }));
    try {
      const ok = await invoke<boolean>('blog_test_platform_connection', {
        account_id: id,
      });
      setTestStatus((prev) => ({ ...prev, [id]: ok ? 'ok' : 'fail' }));
      setTimeout(() => {
        setTestStatus((prev) => {
          const n = { ...prev };
          delete n[id];
          return n;
        });
      }, 5000);
    } catch (e) {
      setTestStatus((prev) => ({ ...prev, [id]: 'fail' }));
      setError(String(e));
    }
  };

  const saveAccount = async () => {
    if (!formValid()) {
      setError('Please fill in the required fields for this platform.');
      return;
    }
    setSaving(true);
    setError(null);
    try {
      const defaultTags = formDefaultTags()
        .split(',')
        .map((t) => t.trim())
        .filter(Boolean);
      const account = {
        platform: formPlatform(),
        account_label: formLabel().trim() || null,
        base_url: formBaseUrl().trim() || null,
        api_key: formApiKey().trim() || null,
        publication_id: formPubId().trim() || null,
        default_tags: defaultTags.length > 0 ? defaultTags : null,
      };
      const created = await invoke<PlatformAccount>('blog_create_platform_account', {
        account,
      });
      showToast(`Account for ${platformLabel(created.platform)} created`);
      closeAdd();
      await load();
      if (isAutoPlatform(created.platform) && created.has_key) {
        runTest(created.id);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const deleteAccount = async (id: string) => {
    const ok = window.confirm('Delete this platform account?');
    if (!ok) return;
    try {
      await invoke<void>('blog_delete_platform_account', { id });
      showToast('Account deleted');
      await load();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div class="px-8 py-6 max-w-4xl">
      <div class="flex items-center justify-between mb-4">
        <h2 class="text-lg font-semibold text-gray-900 dark:text-white">Platform accounts</h2>
        <button class="btn-primary text-sm" onClick={openAdd}>
          + Add account
        </button>
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

      <Show when={loading() && accounts().length === 0}>
        <div class="text-center py-12 text-gray-400">Loading…</div>
      </Show>

      <Show when={!loading() && accounts().length === 0}>
        <div class="card p-10 text-center text-sm text-gray-500">
          No platform accounts yet. Add one to enable auto-publishing or to track manual
          exports.
        </div>
      </Show>

      <Show when={accounts().length > 0}>
        <div class="card p-0 overflow-hidden">
          <table class="w-full text-sm">
            <thead class="bg-gray-50 dark:bg-gray-800 text-xs uppercase text-gray-500">
              <tr>
                <th class="text-left p-3">Platform</th>
                <th class="text-left p-3">Label</th>
                <th class="text-left p-3">Mode</th>
                <th class="text-left p-3">Key</th>
                <th class="text-left p-3">Status</th>
                <th class="text-right p-3">Actions</th>
              </tr>
            </thead>
            <tbody>
              <For each={accounts()}>
                {(a) => {
                  const auto = isAutoPlatform(a.platform);
                  return (
                    <tr class="border-t border-gray-100 dark:border-gray-800">
                      <td class="p-3 font-medium">{platformLabel(a.platform)}</td>
                      <td class="p-3 text-gray-600 dark:text-gray-300">
                        {a.account_label ?? '—'}
                        <Show when={a.base_url}>
                          <div class="text-[10px] text-gray-400 font-mono truncate max-w-[14rem]">
                            {a.base_url}
                          </div>
                        </Show>
                      </td>
                      <td class="p-3">
                        <span
                          class="text-[10px] px-1.5 py-0.5 rounded"
                          classList={{
                            'bg-emerald-100 dark:bg-emerald-900/30 text-emerald-700 dark:text-emerald-300':
                              auto,
                            'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300':
                              !auto,
                          }}
                        >
                          {auto ? 'Auto' : 'Manual'}
                        </span>
                      </td>
                      <td class="p-3">
                        <Show
                          when={a.has_key}
                          fallback={
                            <span class="text-[10px] text-gray-400">no key</span>
                          }
                        >
                          <span class="text-[10px] text-emerald-600">key set</span>
                        </Show>
                      </td>
                      <td class="p-3">
                        <Show when={auto && a.has_key}>
                          <div class="flex items-center gap-2">
                            <button
                              class="text-xs text-minion-600 hover:underline"
                              onClick={() => runTest(a.id)}
                              disabled={testStatus()[a.id] === 'pending'}
                            >
                              {testStatus()[a.id] === 'pending' ? 'Testing…' : 'Test'}
                            </button>
                            <Show when={testStatus()[a.id] === 'ok'}>
                              <span class="text-xs text-emerald-600">✓</span>
                            </Show>
                            <Show when={testStatus()[a.id] === 'fail'}>
                              <span class="text-xs text-red-600">✗</span>
                            </Show>
                          </div>
                        </Show>
                      </td>
                      <td class="p-3 text-right">
                        <button
                          class="text-xs text-red-600 hover:underline"
                          onClick={() => deleteAccount(a.id)}
                        >
                          Delete
                        </button>
                      </td>
                    </tr>
                  );
                }}
              </For>
            </tbody>
          </table>
        </div>
      </Show>

      {/* Add modal */}
      <Show when={showAdd()}>
        <div
          class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4"
          onClick={closeAdd}
        >
          <div
            class="bg-white dark:bg-gray-900 rounded-xl max-w-lg w-full p-5 max-h-[90vh] overflow-y-auto"
            onClick={(e) => e.stopPropagation()}
          >
            <h3 class="font-semibold mb-4">Add platform account</h3>
            <div class="space-y-3">
              <div>
                <label class="block text-xs text-gray-500 mb-1">Platform</label>
                <select
                  class="w-full px-2 py-1.5 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
                  value={formPlatform()}
                  onChange={(e) => setFormPlatform(e.currentTarget.value)}
                >
                  <For each={ALL_PLATFORMS}>
                    {(p) => (
                      <option value={p.value}>
                        {p.label} {p.auto ? '(auto)' : '(manual)'}
                      </option>
                    )}
                  </For>
                </select>
              </div>

              <Show when={formPlatform() === 'wordpress'}>
                <div>
                  <label class="block text-xs text-gray-500 mb-1">
                    Base URL <span class="text-red-500">*</span>
                  </label>
                  <input
                    type="text"
                    placeholder="https://yourblog.wordpress.com"
                    value={formBaseUrl()}
                    onInput={(e) => setFormBaseUrl(e.currentTarget.value)}
                    class="w-full px-2 py-1.5 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
                  />
                </div>
                <div>
                  <label class="block text-xs text-gray-500 mb-1">
                    Username <span class="text-red-500">*</span>
                  </label>
                  <input
                    type="text"
                    value={formLabel()}
                    onInput={(e) => setFormLabel(e.currentTarget.value)}
                    class="w-full px-2 py-1.5 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
                  />
                </div>
                <div>
                  <label class="block text-xs text-gray-500 mb-1">
                    Application password <span class="text-red-500">*</span>
                  </label>
                  <input
                    type="password"
                    value={formApiKey()}
                    onInput={(e) => setFormApiKey(e.currentTarget.value)}
                    class="w-full px-2 py-1.5 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
                  />
                </div>
              </Show>

              <Show when={formPlatform() === 'devto'}>
                <div>
                  <label class="block text-xs text-gray-500 mb-1">
                    API key <span class="text-red-500">*</span>
                  </label>
                  <input
                    type="password"
                    value={formApiKey()}
                    onInput={(e) => setFormApiKey(e.currentTarget.value)}
                    class="w-full px-2 py-1.5 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
                  />
                </div>
                <div>
                  <label class="block text-xs text-gray-500 mb-1">Label (optional)</label>
                  <input
                    type="text"
                    value={formLabel()}
                    onInput={(e) => setFormLabel(e.currentTarget.value)}
                    class="w-full px-2 py-1.5 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
                  />
                </div>
              </Show>

              <Show when={formPlatform() === 'hashnode'}>
                <div>
                  <label class="block text-xs text-gray-500 mb-1">
                    API key <span class="text-red-500">*</span>
                  </label>
                  <input
                    type="password"
                    value={formApiKey()}
                    onInput={(e) => setFormApiKey(e.currentTarget.value)}
                    class="w-full px-2 py-1.5 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
                  />
                </div>
                <div>
                  <label class="block text-xs text-gray-500 mb-1">
                    Publication ID <span class="text-red-500">*</span>
                  </label>
                  <input
                    type="text"
                    value={formPubId()}
                    onInput={(e) => setFormPubId(e.currentTarget.value)}
                    class="w-full px-2 py-1.5 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
                  />
                </div>
                <div>
                  <label class="block text-xs text-gray-500 mb-1">Label (optional)</label>
                  <input
                    type="text"
                    value={formLabel()}
                    onInput={(e) => setFormLabel(e.currentTarget.value)}
                    class="w-full px-2 py-1.5 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
                  />
                </div>
              </Show>

              <Show
                when={
                  formPlatform() !== 'wordpress' &&
                  formPlatform() !== 'devto' &&
                  formPlatform() !== 'hashnode'
                }
              >
                <div>
                  <label class="block text-xs text-gray-500 mb-1">
                    Label <span class="text-red-500">*</span>
                  </label>
                  <input
                    type="text"
                    placeholder="e.g. @yourhandle"
                    value={formLabel()}
                    onInput={(e) => setFormLabel(e.currentTarget.value)}
                    class="w-full px-2 py-1.5 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
                  />
                </div>
                <p class="text-xs text-gray-500">
                  This platform only supports manual export. No API key required — you'll copy
                  the post and paste into the editor.
                </p>
              </Show>

              <div>
                <label class="block text-xs text-gray-500 mb-1">
                  Default tags (optional)
                </label>
                <input
                  type="text"
                  placeholder="rust, programming"
                  value={formDefaultTags()}
                  onInput={(e) => setFormDefaultTags(e.currentTarget.value)}
                  class="w-full px-2 py-1.5 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
                />
              </div>
            </div>
            <div class="flex justify-end gap-2 mt-5">
              <button
                class="px-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
                onClick={closeAdd}
              >
                Cancel
              </button>
              <button
                class="btn-primary text-sm"
                onClick={saveAccount}
                disabled={saving() || !formValid()}
              >
                {saving() ? 'Saving…' : 'Save'}
              </button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default PlatformsTab;
