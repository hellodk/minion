import { Component, createSignal, For, onMount, Show } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import type { SystemInfo } from '../App';

const Settings: Component = () => {
  const [systemInfo, setSystemInfo] = createSignal<SystemInfo | null>(null);
  const [theme, setTheme] = createSignal('system');
  const [animations, setAnimations] = createSignal(true);
  const [ollamaUrl, setOllamaUrl] = createSignal('http://192.168.1.10:11434');
  const [aiModel, setAiModel] = createSignal('llama3.2:3b');
  const [aiTestStatus, setAiTestStatus] = createSignal<'idle' | 'testing' | 'success' | 'error'>('idle');
  const [aiTestMessage, setAiTestMessage] = createSignal('');
  const [aiSaving, setAiSaving] = createSignal(false);

  // Google Fit state
  const [gfitConnected, setGfitConnected] = createSignal(false);
  const [gfitClientId, setGfitClientId] = createSignal('');
  const [gfitAuthCode, setGfitAuthCode] = createSignal('');
  const [gfitStatus, setGfitStatus] = createSignal<'idle' | 'saving' | 'syncing' | 'success' | 'error'>('idle');
  const [gfitMessage, setGfitMessage] = createSignal('');

  type CalendarAccountRow = { id: string; provider: string; email: string | null };
  const [calAccounts, setCalAccounts] = createSignal<CalendarAccountRow[]>([]);
  const [outlookClientId, setOutlookClientId] = createSignal('');
  const [calIntMessage, setCalIntMessage] = createSignal('');

  // Zerodha Kite state
  const [zdApiKey, setZdApiKey] = createSignal('');
  const [zdApiSecret, setZdApiSecret] = createSignal('');
  const [zdAccessToken, setZdAccessToken] = createSignal('');
  const [zdStatus, setZdStatus] = createSignal<'idle' | 'saving' | 'syncing' | 'success' | 'error'>('idle');
  const [zdMessage, setZdMessage] = createSignal('');

  onMount(async () => {
    try {
      const info = await invoke<SystemInfo>('get_system_info');
      setSystemInfo(info);

      const config = await invoke<any>('get_config', { key: null });
      if (config.ui) {
        setTheme(config.ui.theme);
        setAnimations(config.ui.animations);
      }
      if (config.ai_ollama_url) setOllamaUrl(config.ai_ollama_url);
      if (config.ai_model) setAiModel(config.ai_model);

      try {
        const connected = await invoke<boolean>('gfit_check_connected');
        setGfitConnected(connected);
        const savedId = await invoke<string | null>('gfit_get_client_id');
        if (savedId) setGfitClientId(savedId);
        const oid = await invoke<string | null>('calendar_get_outlook_client_id');
        if (oid) setOutlookClientId(oid);
        const accounts = await invoke<CalendarAccountRow[]>('calendar_list_accounts');
        setCalAccounts(accounts);
      } catch (_) {
        // ignore
      }
    } catch (e) {
      console.error('Failed to load settings:', e);
    }
  });

  const updateTheme = async (newTheme: string) => {
    setTheme(newTheme);
    try {
      await invoke('set_config', { key: 'theme', value: newTheme });
    } catch (e) {
      console.error('Failed to save theme:', e);
    }
  };

  const updateAnimations = async (enabled: boolean) => {
    setAnimations(enabled);
    try {
      await invoke('set_config', { key: 'animations', value: enabled });
    } catch (e) {
      console.error('Failed to save animations setting:', e);
    }
  };

  return (
    <div class="p-6 max-w-3xl">
      <h1 class="text-2xl font-bold mb-6">Settings</h1>

      {/* Appearance */}
      <section class="card p-4 mb-6">
        <h2 class="text-lg font-medium mb-4">Appearance</h2>
        
        <div class="space-y-4">
          <div>
            <label class="block text-sm font-medium mb-2">Theme</label>
            <select
              class="input"
              value={theme()}
              onChange={(e) => updateTheme(e.currentTarget.value)}
            >
              <option value="system">System</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </div>
          
          <div class="flex items-center justify-between">
            <div>
              <p class="font-medium">Animations</p>
              <p class="text-sm text-gray-500 dark:text-gray-400">Enable UI animations</p>
            </div>
            <button
              class="relative w-11 h-6 rounded-full transition-colors"
              classList={{
                'bg-minion-600': animations(),
                'bg-gray-300 dark:bg-gray-600': !animations(),
              }}
              onClick={() => updateAnimations(!animations())}
            >
              <span
                class="absolute top-0.5 left-0.5 w-5 h-5 bg-white rounded-full shadow transition-transform"
                classList={{ 'translate-x-5': animations() }}
              />
            </button>
          </div>
        </div>
      </section>

      {/* Data & Privacy */}
      <section class="card p-4 mb-6">
        <h2 class="text-lg font-medium mb-4">Data & Privacy</h2>
        
        <div class="space-y-4">
          <Show when={systemInfo()}>
            <div>
              <label class="block text-sm font-medium mb-1">Data Directory</label>
              <p class="text-sm text-gray-600 dark:text-gray-400 font-mono bg-gray-100 dark:bg-gray-800 px-3 py-2 rounded">
                {systemInfo()!.data_dir}
              </p>
            </div>
          </Show>
          
          <div class="flex items-center justify-between">
            <div>
              <p class="font-medium">Telemetry</p>
              <p class="text-sm text-gray-500 dark:text-gray-400">Always disabled</p>
            </div>
            <span class="px-2 py-1 text-xs bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300 rounded">
              Off
            </span>
          </div>
          
          <button class="btn btn-secondary w-full">
            Export All Data
          </button>
        </div>
      </section>

      {/* AI / LLM */}
      <section class="card p-4 mb-6">
        <h2 class="text-lg font-medium mb-4">AI / LLM</h2>

        <div class="space-y-4">
          <div>
            <label class="block text-sm font-medium mb-2">Ollama URL</label>
            <input
              type="text"
              class="input w-full"
              value={ollamaUrl()}
              onInput={(e) => setOllamaUrl(e.currentTarget.value)}
              placeholder="http://192.168.1.10:11434"
            />
          </div>

          <div>
            <label class="block text-sm font-medium mb-2">Model Name</label>
            <input
              type="text"
              class="input w-full"
              value={aiModel()}
              onInput={(e) => setAiModel(e.currentTarget.value)}
              placeholder="llama3.2:3b"
            />
          </div>

          <div class="flex items-center gap-3">
            <button
              class="btn btn-secondary"
              disabled={aiTestStatus() === 'testing'}
              onClick={async () => {
                setAiTestStatus('testing');
                setAiTestMessage('');
                try {
                  const result = await invoke<string>('ai_test_connection', { url: ollamaUrl() });
                  const parsed = JSON.parse(result);
                  const models = parsed.models?.map((m: any) => m.name).join(', ') || 'none';
                  setAiTestMessage(`Connected! Available models: ${models}`);
                  setAiTestStatus('success');
                } catch (e: any) {
                  setAiTestMessage(String(e));
                  setAiTestStatus('error');
                }
              }}
            >
              {aiTestStatus() === 'testing' ? 'Testing...' : 'Test Connection'}
            </button>

            <button
              class="btn btn-primary"
              disabled={aiSaving()}
              onClick={async () => {
                setAiSaving(true);
                try {
                  await invoke('set_config', { key: 'ai_ollama_url', value: ollamaUrl() });
                  await invoke('set_config', { key: 'ai_model', value: aiModel() });
                  setAiTestMessage('Settings saved.');
                  setAiTestStatus('success');
                } catch (e: any) {
                  setAiTestMessage('Failed to save: ' + String(e));
                  setAiTestStatus('error');
                } finally {
                  setAiSaving(false);
                }
              }}
            >
              {aiSaving() ? 'Saving...' : 'Save'}
            </button>
          </div>

          <Show when={aiTestMessage()}>
            <p
              class="text-sm mt-1"
              classList={{
                'text-green-500': aiTestStatus() === 'success',
                'text-red-500': aiTestStatus() === 'error',
                'text-gray-500': aiTestStatus() === 'testing',
              }}
            >
              {aiTestMessage()}
            </p>
          </Show>
        </div>
      </section>

      {/* Google Fit */}
      <section class="card p-4 mb-6">
        <h2 class="text-lg font-medium mb-4">Google Fit</h2>

        <div class="space-y-4">
          {/* Connection status */}
          <div class="flex items-center justify-between">
            <div>
              <p class="font-medium">Connection Status</p>
              <p class="text-sm text-gray-500 dark:text-gray-400">
                {gfitConnected() ? 'Connected to Google Fit' : 'Not connected'}
              </p>
            </div>
            <span
              class="px-2 py-1 text-xs rounded"
              classList={{
                'bg-green-100 dark:bg-green-900 text-green-700 dark:text-green-300': gfitConnected(),
                'bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400': !gfitConnected(),
              }}
            >
              {gfitConnected() ? 'Connected' : 'Disconnected'}
            </span>
          </div>

          {/* Client ID + Connect */}
          <Show when={!gfitConnected()}>
            <div class="space-y-3">
              <div>
                <label class="block text-sm font-medium mb-1">OAuth Client ID</label>
                <input
                  type="text"
                  class="input w-full text-sm"
                  placeholder="your-client-id.apps.googleusercontent.com"
                  value={gfitClientId()}
                  onInput={(e) => setGfitClientId(e.currentTarget.value)}
                />
                <p class="text-xs text-gray-400 mt-1">
                  Google Cloud Console → APIs &amp; Services → Credentials → Create OAuth client ID (Desktop app).
                  Enable the Fitness API. Under Authorized redirect URIs add exactly{' '}
                  <code class="text-minion-600 dark:text-minion-400">http://127.0.0.1:8745/</code>
                </p>
              </div>
              <div class="flex gap-2">
                <button
                  class="btn btn-secondary flex-1"
                  disabled={!gfitClientId().trim()}
                  onClick={async () => {
                    try {
                      await invoke('gfit_save_client_id', { clientId: gfitClientId().trim() });
                      setGfitMessage('Client ID saved.');
                      setGfitStatus('success');
                    } catch (e: any) {
                      setGfitMessage('Failed: ' + String(e));
                      setGfitStatus('error');
                    }
                  }}
                >
                  Save Client ID
                </button>
                <button
                  class="btn btn-primary flex-1"
                  disabled={!gfitClientId().trim()}
                  onClick={async () => {
                    try {
                      await invoke('gfit_save_client_id', { clientId: gfitClientId().trim() });
                      await invoke('gfit_open_auth');
                      setGfitConnected(true);
                      setGfitAuthCode('');
                      setGfitMessage('Connected to Google Fit.');
                      setGfitStatus('success');
                    } catch (e: any) {
                      setGfitMessage(String(e));
                      setGfitStatus('error');
                    }
                  }}
                >
                  Connect Google Fit
                </button>
              </div>
            </div>
          </Show>

          {/* Optional: paste authorization code if the browser flow did not complete */}
          <Show when={!gfitConnected()}>
            <div>
              <label class="block text-sm font-medium mb-2">Authorization code (optional)</label>
              <input
                type="text"
                class="input w-full"
                value={gfitAuthCode()}
                onInput={(e) => setGfitAuthCode(e.currentTarget.value)}
                placeholder="Paste only if Connect did not finish automatically"
              />
            </div>
            <button
              class="btn btn-secondary"
              disabled={gfitStatus() === 'saving' || !gfitAuthCode().trim()}
              onClick={async () => {
                setGfitStatus('saving');
                setGfitMessage('');
                try {
                  await invoke('gfit_exchange_auth_code', { code: gfitAuthCode().trim() });
                  setGfitConnected(true);
                  setGfitAuthCode('');
                  setGfitMessage('Authorization code exchanged successfully.');
                  setGfitStatus('success');
                } catch (e: any) {
                  setGfitMessage('Failed: ' + String(e));
                  setGfitStatus('error');
                }
              }}
            >
              {gfitStatus() === 'saving' ? 'Exchanging...' : 'Exchange authorization code'}
            </button>
          </Show>

          {/* Sync & Disconnect */}
          <Show when={gfitConnected()}>
            <div class="flex items-center gap-3">
              <button
                class="btn btn-primary"
                disabled={gfitStatus() === 'syncing'}
                onClick={async () => {
                  setGfitStatus('syncing');
                  setGfitMessage('');
                  try {
                    const result = await invoke<string>('gfit_sync');
                    setGfitMessage(result);
                    setGfitStatus('success');
                  } catch (e: any) {
                    setGfitMessage(String(e));
                    setGfitStatus('error');
                  }
                }}
              >
                {gfitStatus() === 'syncing' ? 'Syncing...' : 'Sync Now'}
              </button>
              <button
                class="btn btn-secondary text-red-600 dark:text-red-400"
                onClick={async () => {
                  try {
                    await invoke('gfit_disconnect');
                    setGfitConnected(false);
                    setGfitMessage('Disconnected from Google Fit.');
                    setGfitStatus('idle');
                  } catch (e: any) {
                    setGfitMessage('Failed to disconnect: ' + String(e));
                    setGfitStatus('error');
                  }
                }}
              >
                Disconnect
              </button>
            </div>
          </Show>

          {/* Status message */}
          <Show when={gfitMessage()}>
            <p
              class="text-sm"
              classList={{
                'text-green-500': gfitStatus() === 'success',
                'text-red-500': gfitStatus() === 'error',
                'text-gray-500': gfitStatus() === 'syncing' || gfitStatus() === 'saving',
              }}
            >
              {gfitMessage()}
            </p>
          </Show>
        </div>
      </section>

      {/* Calendar Integrations */}
      <section class="card p-4 mb-6">
        <h2 class="text-lg font-medium mb-4">Calendar Integrations</h2>

        <p class="text-sm text-gray-500 dark:text-gray-400 mb-4">
          Add multiple Google and Outlook accounts. Google uses the same OAuth Client ID as Google Fit (must include
          Calendar API scope and redirect <code class="text-xs">http://127.0.0.1:8747/</code>).
          Outlook uses an Azure app with redirect <code class="text-xs">http://127.0.0.1:8748/</code> and PKCE.
        </p>

        <div class="space-y-6">
          {/* Google Calendar accounts */}
          <div>
            <div class="flex flex-wrap items-center justify-between gap-2 mb-2">
              <p class="font-medium">Google Calendar</p>
              <div class="flex gap-2">
                <button
                  type="button"
                  class="btn btn-secondary text-sm"
                  onClick={async () => {
                    setCalIntMessage('');
                    try {
                      await invoke('calendar_google_open_auth');
                      const accounts = await invoke<CalendarAccountRow[]>('calendar_list_accounts');
                      setCalAccounts(accounts);
                      setCalIntMessage('Google account added.');
                    } catch (e: any) {
                      setCalIntMessage(String(e));
                    }
                  }}
                >
                  Add Google account
                </button>
                <button
                  type="button"
                  class="btn btn-secondary text-sm"
                  onClick={async () => {
                    setCalIntMessage('');
                    try {
                      const msg = await invoke<string>('calendar_sync_google', { accountId: null });
                      setCalIntMessage(msg);
                    } catch (e: any) {
                      setCalIntMessage(String(e));
                    }
                  }}
                >
                  Sync all Google
                </button>
              </div>
            </div>
            <ul class="text-sm space-y-1 border border-gray-200 dark:border-gray-700 rounded px-3 py-2">
              <For
                each={calAccounts().filter((a) => a.provider === 'google')}
                fallback={<li class="text-gray-400">No Google Calendar accounts yet.</li>}
              >
                {(a) => (
                  <li class="flex flex-wrap items-center justify-between gap-2 py-1">
                    <span>{a.email ?? a.id.slice(0, 8) + '…'}</span>
                    <span class="flex gap-1">
                      <button
                        type="button"
                        class="text-xs px-2 py-0.5 rounded bg-blue-100 dark:bg-blue-900 text-blue-800 dark:text-blue-200"
                        onClick={async () => {
                          setCalIntMessage('');
                          try {
                            const msg = await invoke<string>('calendar_sync_google', { accountId: a.id });
                            setCalIntMessage(msg);
                          } catch (e: any) {
                            setCalIntMessage(String(e));
                          }
                        }}
                      >
                        Sync
                      </button>
                      <button
                        type="button"
                        class="text-xs px-2 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300"
                        onClick={async () => {
                          try {
                            await invoke('calendar_remove_account', { accountId: a.id });
                            setCalAccounts((prev) => prev.filter((x) => x.id !== a.id));
                          } catch (e: any) {
                            setCalIntMessage(String(e));
                          }
                        }}
                      >
                        Remove
                      </button>
                    </span>
                  </li>
                )}
              </For>
            </ul>
          </div>

          {/* Outlook */}
          <div>
            <p class="font-medium mb-2">Outlook Calendar</p>
            <div class="flex flex-wrap gap-2 mb-2">
              <input
                type="text"
                class="input flex-1 min-w-[200px]"
                placeholder="Azure Application (client) ID"
                value={outlookClientId()}
                onInput={(e) => setOutlookClientId(e.currentTarget.value)}
              />
              <button
                type="button"
                class="btn btn-secondary text-sm"
                onClick={async () => {
                  setCalIntMessage('');
                  try {
                    await invoke('calendar_save_outlook_client_id', { clientId: outlookClientId() });
                    setCalIntMessage('Outlook client ID saved.');
                  } catch (e: any) {
                    setCalIntMessage(String(e));
                  }
                }}
              >
                Save
              </button>
            </div>
            <div class="flex flex-wrap items-center justify-between gap-2 mb-2">
              <span class="text-sm text-gray-500 dark:text-gray-400">Connected accounts</span>
              <div class="flex gap-2">
                <button
                  type="button"
                  class="btn btn-secondary text-sm"
                  onClick={async () => {
                    setCalIntMessage('');
                    try {
                      await invoke('calendar_outlook_open_auth');
                      const accounts = await invoke<CalendarAccountRow[]>('calendar_list_accounts');
                      setCalAccounts(accounts);
                      setCalIntMessage('Outlook account added.');
                    } catch (e: any) {
                      setCalIntMessage(String(e));
                    }
                  }}
                >
                  Add Outlook account
                </button>
                <button
                  type="button"
                  class="btn btn-secondary text-sm"
                  onClick={async () => {
                    setCalIntMessage('');
                    try {
                      const msg = await invoke<string>('calendar_sync_outlook', { accountId: null });
                      setCalIntMessage(msg);
                    } catch (e: any) {
                      setCalIntMessage(String(e));
                    }
                  }}
                >
                  Sync all Outlook
                </button>
              </div>
            </div>
            <ul class="text-sm space-y-1 border border-gray-200 dark:border-gray-700 rounded px-3 py-2">
              <For
                each={calAccounts().filter((a) => a.provider === 'outlook')}
                fallback={<li class="text-gray-400">No Outlook accounts yet.</li>}
              >
                {(a) => (
                  <li class="flex flex-wrap items-center justify-between gap-2 py-1">
                    <span>{a.email ?? a.id.slice(0, 8) + '…'}</span>
                    <span class="flex gap-1">
                      <button
                        type="button"
                        class="text-xs px-2 py-0.5 rounded bg-blue-100 dark:bg-blue-900 text-blue-800 dark:text-blue-200"
                        onClick={async () => {
                          setCalIntMessage('');
                          try {
                            const msg = await invoke<string>('calendar_sync_outlook', { accountId: a.id });
                            setCalIntMessage(msg);
                          } catch (e: any) {
                            setCalIntMessage(String(e));
                          }
                        }}
                      >
                        Sync
                      </button>
                      <button
                        type="button"
                        class="text-xs px-2 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300"
                        onClick={async () => {
                          try {
                            await invoke('calendar_remove_account', { accountId: a.id });
                            setCalAccounts((prev) => prev.filter((x) => x.id !== a.id));
                          } catch (e: any) {
                            setCalIntMessage(String(e));
                          }
                        }}
                      >
                        Remove
                      </button>
                    </span>
                  </li>
                )}
              </For>
            </ul>
          </div>

          <Show when={calIntMessage()}>
            <p class="text-sm text-gray-600 dark:text-gray-400">{calIntMessage()}</p>
          </Show>
        </div>
      </section>

      {/* Zerodha Kite Connect */}
      <section class="card p-4 mb-6">
        <h2 class="text-lg font-medium mb-4">Zerodha Kite Connect</h2>

        <div class="space-y-4">
          <p class="text-sm text-gray-500 dark:text-gray-400">
            Connect your Zerodha Demat account to sync holdings into your MINION portfolio.
            You need a <a href="https://kite.trade" target="_blank" rel="noopener noreferrer" class="text-minion-600 dark:text-minion-400 underline">Kite Connect</a> API key.
          </p>

          {/* API Key */}
          <div>
            <label class="block text-sm font-medium mb-2">API Key</label>
            <input
              type="text"
              class="input w-full"
              value={zdApiKey()}
              onInput={(e) => setZdApiKey(e.currentTarget.value)}
              placeholder="Your Kite Connect API key"
            />
          </div>

          {/* API Secret */}
          <div>
            <label class="block text-sm font-medium mb-2">API Secret</label>
            <input
              type="password"
              class="input w-full"
              value={zdApiSecret()}
              onInput={(e) => setZdApiSecret(e.currentTarget.value)}
              placeholder="Your Kite Connect API secret"
            />
          </div>

          {/* Save config */}
          <button
            class="btn btn-primary"
            disabled={zdStatus() === 'saving' || !zdApiKey() || !zdApiSecret()}
            onClick={async () => {
              setZdStatus('saving');
              setZdMessage('');
              try {
                await invoke('zerodha_save_config', {
                  apiKey: zdApiKey(),
                  apiSecret: zdApiSecret(),
                });
                setZdMessage('API credentials saved.');
                setZdStatus('success');
              } catch (e: any) {
                setZdMessage('Failed to save: ' + String(e));
                setZdStatus('error');
              }
            }}
          >
            {zdStatus() === 'saving' ? 'Saving...' : 'Save Credentials'}
          </button>

          {/* Login button */}
          <div>
            <label class="block text-sm font-medium mb-2">Login</label>
            <button
              class="btn btn-secondary w-full"
              onClick={async () => {
                setZdMessage('');
                try {
                  await invoke('zerodha_open_login');
                } catch (e: any) {
                  setZdMessage(String(e));
                  setZdStatus('error');
                }
              }}
            >
              Login to Zerodha
            </button>
            <p class="text-xs text-gray-400 dark:text-gray-500 mt-1">
              Opens a Kite login window. After login, copy the access token below.
            </p>
          </div>

          {/* Access token input */}
          <div>
            <label class="block text-sm font-medium mb-2">Access Token</label>
            <input
              type="text"
              class="input w-full"
              value={zdAccessToken()}
              onInput={(e) => setZdAccessToken(e.currentTarget.value)}
              placeholder="Paste your Kite access token"
            />
          </div>
          <button
            class="btn btn-secondary"
            disabled={zdStatus() === 'saving' || !zdAccessToken()}
            onClick={async () => {
              setZdStatus('saving');
              setZdMessage('');
              try {
                await invoke('zerodha_save_token', { accessToken: zdAccessToken() });
                setZdAccessToken('');
                setZdMessage('Access token saved.');
                setZdStatus('success');
              } catch (e: any) {
                setZdMessage('Failed to save token: ' + String(e));
                setZdStatus('error');
              }
            }}
          >
            Save Token
          </button>

          {/* Sync holdings */}
          <button
            class="btn btn-secondary w-full"
            disabled={zdStatus() === 'syncing'}
            onClick={async () => {
              setZdStatus('syncing');
              setZdMessage('');
              try {
                const result = await invoke<string>('zerodha_sync_to_portfolio');
                setZdMessage(result);
                setZdStatus('success');
              } catch (e: any) {
                setZdMessage(String(e));
                setZdStatus('error');
              }
            }}
          >
            {zdStatus() === 'syncing' ? 'Syncing...' : 'Sync Holdings'}
          </button>

          {/* Status message */}
          <Show when={zdMessage()}>
            <p
              class="text-sm"
              classList={{
                'text-green-500': zdStatus() === 'success',
                'text-red-500': zdStatus() === 'error',
                'text-gray-500': zdStatus() === 'syncing' || zdStatus() === 'saving',
              }}
            >
              {zdMessage()}
            </p>
          </Show>
        </div>
      </section>

      {/* Modules */}
      <section class="card p-4 mb-6">
        <h2 class="text-lg font-medium mb-4">Modules</h2>
        
        <p class="text-sm text-gray-500 dark:text-gray-400">
          Enable or disable individual modules to customize your experience.
        </p>
        
        <button class="btn btn-secondary mt-4">
          Manage Modules
        </button>
      </section>

      {/* About */}
      <section class="card p-4">
        <h2 class="text-lg font-medium mb-4">About</h2>
        
        <Show when={systemInfo()}>
          <div class="space-y-2 text-sm">
            <div class="flex justify-between">
              <span class="text-gray-500 dark:text-gray-400">Version</span>
              <span class="font-mono">{systemInfo()!.version}</span>
            </div>
            <div class="flex justify-between">
              <span class="text-gray-500 dark:text-gray-400">Platform</span>
              <span class="font-mono">{systemInfo()!.platform}</span>
            </div>
            <div class="flex justify-between">
              <span class="text-gray-500 dark:text-gray-400">Architecture</span>
              <span class="font-mono">{systemInfo()!.arch}</span>
            </div>
          </div>
        </Show>
        
        <div class="mt-4 pt-4 border-t border-gray-200 dark:border-gray-700">
          <p class="text-sm text-gray-500 dark:text-gray-400">
            MINION - Modular Intelligence Network for Integrated Operations Natively
          </p>
          <p class="text-xs text-gray-400 dark:text-gray-500 mt-1">
            MIT License
          </p>
        </div>
      </section>
    </div>
  );
};

export default Settings;
