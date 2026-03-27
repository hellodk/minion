import { Component, createSignal, onMount, For } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

interface ModuleInfo {
  id: string;
  name: string;
  enabled: boolean;
  status: string;
}

const Dashboard: Component = () => {
  const [modules, setModules] = createSignal<ModuleInfo[]>([]);

  onMount(async () => {
    try {
      const mods = await invoke<ModuleInfo[]>('list_modules');
      setModules(mods);
    } catch (e) {
      console.error('Failed to load modules:', e);
    }
  });

  return (
    <div class="p-6">
      <h1 class="text-2xl font-bold mb-6">Dashboard</h1>

      {/* Quick Stats */}
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-8">
        <div class="card p-4">
          <p class="text-sm text-gray-500 dark:text-gray-400">Total Files</p>
          <p class="text-2xl font-bold">-</p>
        </div>
        <div class="card p-4">
          <p class="text-sm text-gray-500 dark:text-gray-400">Duplicates Found</p>
          <p class="text-2xl font-bold">-</p>
        </div>
        <div class="card p-4">
          <p class="text-sm text-gray-500 dark:text-gray-400">Books in Library</p>
          <p class="text-2xl font-bold">-</p>
        </div>
        <div class="card p-4">
          <p class="text-sm text-gray-500 dark:text-gray-400">Reading Streak</p>
          <p class="text-2xl font-bold">-</p>
        </div>
      </div>

      {/* Modules Grid */}
      <h2 class="text-lg font-semibold mb-4">Modules</h2>
      <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        <For each={modules()}>
          {(module) => (
            <div class="card p-4 hover:shadow-md transition-shadow cursor-pointer">
              <div class="flex items-center justify-between mb-2">
                <h3 class="font-medium">{module.name}</h3>
                <span
                  class="px-2 py-1 text-xs rounded-full"
                  classList={{
                    'bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300': module.status === 'active',
                    'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300': module.status === 'inactive',
                  }}
                >
                  {module.status}
                </span>
              </div>
              <p class="text-sm text-gray-500 dark:text-gray-400">
                {module.enabled ? 'Enabled' : 'Disabled'}
              </p>
            </div>
          )}
        </For>
      </div>

      {/* Recent Activity */}
      <h2 class="text-lg font-semibold mt-8 mb-4">Recent Activity</h2>
      <div class="card p-4">
        <p class="text-gray-500 dark:text-gray-400 text-center py-8">
          No recent activity
        </p>
      </div>
    </div>
  );
};

export default Dashboard;
