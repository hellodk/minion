import { Component, createSignal, onMount, Show } from 'solid-js';
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
