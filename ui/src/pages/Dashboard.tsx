import { Component, createSignal, onMount, For, Show } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { useNavigate } from '@solidjs/router';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface ModuleInfo {
  id: string;
  name: string;
  enabled: boolean;
  status: string;
}

interface FinancialSummary {
  net_worth: number;
  total_assets: number;
  total_liabilities: number;
  monthly_income: number;
  monthly_expenses: number;
  savings_rate: number;
  account_count: number;
  transaction_count: number;
}

interface FitnessDashboard {
  total_habits: number;
  habits_completed_today: number;
  current_streak: number;
  latest_weight_kg: number | null;
  avg_steps_7d: number | null;
  avg_sleep_7d: number | null;
  total_water_today: number | null;
  workouts_this_week: number;
}

interface ReaderBook {
  id: string;
  title: string | null;
  authors: string | null;
  file_path: string;
  format: string | null;
  cover_path: string | null;
  pages: number | null;
  current_position: string | null;
  progress: number;
  rating: number | null;
  favorite: boolean;
  tags: string | null;
  added_at: string;
  last_read_at: string | null;
}

interface FinanceTransaction {
  id: string;
  account_id: string;
  transaction_type: string;
  amount: number;
  description: string | null;
  category: string | null;
  tags: string | null;
  date: string;
  created_at: string;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const MODULE_ICONS: Record<string, string> = {
  files: 'M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z',
  reader:
    'M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253',
  finance:
    'M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z',
  fitness:
    'M4.318 6.318a4.5 4.5 0 000 6.364L12 20.364l7.682-7.682a4.5 4.5 0 00-6.364-6.364L12 7.636l-1.318-1.318a4.5 4.5 0 00-6.364 0z',
  media:
    'M15 10l4.553-2.276A1 1 0 0121 8.618v6.764a1 1 0 01-1.447.894L15 14M5 18h8a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z',
  blog: 'M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z',
};

function formatCurrency(value: number): string {
  if (Math.abs(value) >= 10_000_000) {
    return `${(value / 10_000_000).toFixed(2)} Cr`;
  }
  if (Math.abs(value) >= 100_000) {
    return `${(value / 100_000).toFixed(2)} L`;
  }
  return value.toLocaleString('en-IN', { maximumFractionDigits: 0 });
}

function relativeDate(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffDays = Math.floor(diffMs / 86_400_000);

  if (diffDays === 0) return 'Today';
  if (diffDays === 1) return 'Yesterday';
  if (diffDays < 7) return `${diffDays}d ago`;
  if (diffDays < 30) return `${Math.floor(diffDays / 7)}w ago`;
  return date.toLocaleDateString('en-IN', { month: 'short', day: 'numeric' });
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

const Dashboard: Component = () => {
  const navigate = useNavigate();

  const [loading, setLoading] = createSignal(true);
  const [modules, setModules] = createSignal<ModuleInfo[]>([]);
  const [financeSummary, setFinanceSummary] = createSignal<FinancialSummary | null>(null);
  const [fitnessDash, setFitnessDash] = createSignal<FitnessDashboard | null>(null);
  const [books, setBooks] = createSignal<ReaderBook[]>([]);
  const [recentTransactions, setRecentTransactions] = createSignal<FinanceTransaction[]>([]);
  const [filesScanned, setFilesScanned] = createSignal<number | null>(null);

  onMount(async () => {
    const results = await Promise.allSettled([
      invoke<ModuleInfo[]>('list_modules'),
      invoke<FinancialSummary>('finance_get_summary'),
      invoke<FitnessDashboard>('fitness_get_dashboard'),
      invoke<ReaderBook[]>('reader_get_library'),
      invoke<FinanceTransaction[]>('finance_list_transactions', {
        accountId: null,
        limit: 5,
      }),
    ]);

    if (results[0].status === 'fulfilled') setModules(results[0].value);
    if (results[1].status === 'fulfilled') setFinanceSummary(results[1].value);
    if (results[2].status === 'fulfilled') setFitnessDash(results[2].value);
    if (results[3].status === 'fulfilled') setBooks(results[3].value);
    if (results[4].status === 'fulfilled') setRecentTransactions(results[4].value);

    // Derive files scanned count from the files module status
    try {
      const analytics = await invoke<{ total_files: number }>('get_analytics');
      setFilesScanned(analytics.total_files);
    } catch {
      setFilesScanned(null);
    }

    setLoading(false);
  });

  // ----- Derived values -----
  const bookCount = () => books().length;
  const netWorth = () => financeSummary()?.net_worth ?? null;
  const streak = () => fitnessDash()?.current_streak ?? null;

  const recentBooks = () =>
    books()
      .filter((b) => b.last_read_at)
      .sort((a, b) => (b.last_read_at ?? '').localeCompare(a.last_read_at ?? ''))
      .slice(0, 5);

  // -------------------------------------------------------------------------
  // Render
  // -------------------------------------------------------------------------

  return (
    <div class="p-6 space-y-8">
      {/* Page header */}
      <div>
        <h1 class="text-2xl font-bold">Dashboard</h1>
        <p class="text-sm text-gray-500 dark:text-gray-400 mt-1">
          Your personal command centre at a glance.
        </p>
      </div>

      {/* ----------------------------------------------------------------- */}
      {/* Summary cards                                                      */}
      {/* ----------------------------------------------------------------- */}
      <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-5">
        {/* Books */}
        <div
          class="card p-5 cursor-pointer hover:shadow-md transition-shadow"
          onClick={() => navigate('/reader')}
        >
          <div class="flex items-center justify-between mb-3">
            <p class="text-sm font-medium text-gray-500 dark:text-gray-400">Books in Library</p>
            <span class="p-2 rounded-lg bg-indigo-50 dark:bg-indigo-900/30">
              <svg
                class="w-5 h-5 text-indigo-600 dark:text-indigo-400"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                viewBox="0 0 24 24"
              >
                <path stroke-linecap="round" stroke-linejoin="round" d={MODULE_ICONS.reader} />
              </svg>
            </span>
          </div>
          <p class="text-3xl font-bold">{loading() ? '...' : bookCount()}</p>
          <p class="text-xs text-gray-400 dark:text-gray-500 mt-1">
            {loading()
              ? ''
              : recentBooks().length > 0
                ? `${recentBooks().length} read recently`
                : 'No books read yet'}
          </p>
        </div>

        {/* Net Worth */}
        <div
          class="card p-5 cursor-pointer hover:shadow-md transition-shadow"
          onClick={() => navigate('/finance')}
        >
          <div class="flex items-center justify-between mb-3">
            <p class="text-sm font-medium text-gray-500 dark:text-gray-400">Net Worth</p>
            <span class="p-2 rounded-lg bg-emerald-50 dark:bg-emerald-900/30">
              <svg
                class="w-5 h-5 text-emerald-600 dark:text-emerald-400"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                viewBox="0 0 24 24"
              >
                <path stroke-linecap="round" stroke-linejoin="round" d={MODULE_ICONS.finance} />
              </svg>
            </span>
          </div>
          <p class="text-3xl font-bold">
            {loading() ? '...' : netWorth() !== null ? `₹${formatCurrency(netWorth()!)}` : '—'}
          </p>
          <Show when={!loading() && financeSummary()}>
            <p class="text-xs text-gray-400 dark:text-gray-500 mt-1">
              Savings rate {financeSummary()!.savings_rate.toFixed(0)}%
            </p>
          </Show>
        </div>

        {/* Fitness Streak */}
        <div
          class="card p-5 cursor-pointer hover:shadow-md transition-shadow"
          onClick={() => navigate('/fitness')}
        >
          <div class="flex items-center justify-between mb-3">
            <p class="text-sm font-medium text-gray-500 dark:text-gray-400">Fitness Streak</p>
            <span class="p-2 rounded-lg bg-rose-50 dark:bg-rose-900/30">
              <svg
                class="w-5 h-5 text-rose-600 dark:text-rose-400"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                viewBox="0 0 24 24"
              >
                <path stroke-linecap="round" stroke-linejoin="round" d={MODULE_ICONS.fitness} />
              </svg>
            </span>
          </div>
          <p class="text-3xl font-bold">
            {loading() ? '...' : streak() !== null ? `${streak()} days` : '—'}
          </p>
          <Show when={!loading() && fitnessDash()}>
            <p class="text-xs text-gray-400 dark:text-gray-500 mt-1">
              {fitnessDash()!.habits_completed_today}/{fitnessDash()!.total_habits} habits today
            </p>
          </Show>
        </div>

        {/* Files Scanned */}
        <div
          class="card p-5 cursor-pointer hover:shadow-md transition-shadow"
          onClick={() => navigate('/files')}
        >
          <div class="flex items-center justify-between mb-3">
            <p class="text-sm font-medium text-gray-500 dark:text-gray-400">Files Scanned</p>
            <span class="p-2 rounded-lg bg-amber-50 dark:bg-amber-900/30">
              <svg
                class="w-5 h-5 text-amber-600 dark:text-amber-400"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                viewBox="0 0 24 24"
              >
                <path stroke-linecap="round" stroke-linejoin="round" d={MODULE_ICONS.files} />
              </svg>
            </span>
          </div>
          <p class="text-3xl font-bold">
            {loading() ? '...' : filesScanned() !== null ? filesScanned()!.toLocaleString() : '—'}
          </p>
          <p class="text-xs text-gray-400 dark:text-gray-500 mt-1">
            {loading() ? '' : filesScanned() !== null ? 'From latest scan' : 'No scans yet'}
          </p>
        </div>
      </div>

      {/* ----------------------------------------------------------------- */}
      {/* Module status grid                                                 */}
      {/* ----------------------------------------------------------------- */}
      <div>
        <h2 class="text-lg font-semibold mb-4">Modules</h2>
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
          <For each={modules()}>
            {(mod) => (
              <div class="card p-4 hover:shadow-md transition-shadow">
                <div class="flex items-center gap-3">
                  <span class="p-2 rounded-lg bg-gray-100 dark:bg-gray-700">
                    <svg
                      class="w-5 h-5 text-gray-600 dark:text-gray-300"
                      fill="none"
                      stroke="currentColor"
                      stroke-width="1.5"
                      viewBox="0 0 24 24"
                    >
                      <path
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        d={
                          MODULE_ICONS[mod.id] ||
                          'M4 6h16M4 12h16M4 18h16'
                        }
                      />
                    </svg>
                  </span>
                  <div class="flex-1 min-w-0">
                    <h3 class="font-medium truncate">{mod.name}</h3>
                    <p class="text-xs text-gray-500 dark:text-gray-400">
                      {mod.enabled ? 'Enabled' : 'Disabled'}
                    </p>
                  </div>
                  <span
                    class="flex-shrink-0 px-2.5 py-0.5 text-xs font-medium rounded-full"
                    classList={{
                      'bg-green-100 text-green-700 dark:bg-green-900/40 dark:text-green-300':
                        mod.status === 'active',
                      'bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-300':
                        mod.status !== 'active',
                    }}
                  >
                    {mod.status}
                  </span>
                </div>
              </div>
            )}
          </For>
        </div>
      </div>

      {/* ----------------------------------------------------------------- */}
      {/* Recent Activity + Quick Actions                                    */}
      {/* ----------------------------------------------------------------- */}
      <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Recent Activity -- spans 2 cols */}
        <div class="lg:col-span-2 space-y-6">
          {/* Recent Transactions */}
          <div class="card">
            <div class="flex items-center justify-between px-5 pt-5 pb-3">
              <h3 class="font-semibold">Recent Transactions</h3>
              <button
                class="text-xs font-medium text-minion-600 dark:text-minion-400 hover:underline"
                onClick={() => navigate('/finance')}
              >
                View all
              </button>
            </div>
            <Show
              when={!loading() && recentTransactions().length > 0}
              fallback={
                <div class="px-5 pb-5">
                  <p class="text-sm text-gray-400 dark:text-gray-500 text-center py-6">
                    {loading() ? 'Loading...' : 'No transactions recorded yet.'}
                  </p>
                </div>
              }
            >
              <div class="divide-y divide-gray-100 dark:divide-gray-700">
                <For each={recentTransactions()}>
                  {(tx) => (
                    <div class="flex items-center justify-between px-5 py-3">
                      <div class="min-w-0 flex-1">
                        <p class="text-sm font-medium truncate">
                          {tx.description || 'Untitled transaction'}
                        </p>
                        <p class="text-xs text-gray-400 dark:text-gray-500">
                          {tx.category || 'Uncategorised'} &middot; {relativeDate(tx.date)}
                        </p>
                      </div>
                      <span
                        class="ml-4 text-sm font-semibold whitespace-nowrap"
                        classList={{
                          'text-green-600 dark:text-green-400': tx.transaction_type === 'income',
                          'text-red-600 dark:text-red-400': tx.transaction_type === 'expense',
                          'text-gray-700 dark:text-gray-300':
                            tx.transaction_type !== 'income' &&
                            tx.transaction_type !== 'expense',
                        }}
                      >
                        {tx.transaction_type === 'income' ? '+' : '-'}₹
                        {Math.abs(tx.amount).toLocaleString('en-IN', {
                          maximumFractionDigits: 0,
                        })}
                      </span>
                    </div>
                  )}
                </For>
              </div>
            </Show>
          </div>

          {/* Recent Books */}
          <div class="card">
            <div class="flex items-center justify-between px-5 pt-5 pb-3">
              <h3 class="font-semibold">Recently Read</h3>
              <button
                class="text-xs font-medium text-minion-600 dark:text-minion-400 hover:underline"
                onClick={() => navigate('/reader')}
              >
                View library
              </button>
            </div>
            <Show
              when={!loading() && recentBooks().length > 0}
              fallback={
                <div class="px-5 pb-5">
                  <p class="text-sm text-gray-400 dark:text-gray-500 text-center py-6">
                    {loading() ? 'Loading...' : 'No books read yet.'}
                  </p>
                </div>
              }
            >
              <div class="divide-y divide-gray-100 dark:divide-gray-700">
                <For each={recentBooks()}>
                  {(book) => (
                    <div class="flex items-center gap-4 px-5 py-3">
                      <div class="flex-shrink-0 w-8 h-10 rounded bg-gray-100 dark:bg-gray-700 flex items-center justify-center">
                        <svg
                          class="w-4 h-4 text-gray-400"
                          fill="none"
                          stroke="currentColor"
                          stroke-width="1.5"
                          viewBox="0 0 24 24"
                        >
                          <path
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            d={MODULE_ICONS.reader}
                          />
                        </svg>
                      </div>
                      <div class="min-w-0 flex-1">
                        <p class="text-sm font-medium truncate">
                          {book.title || 'Untitled'}
                        </p>
                        <p class="text-xs text-gray-400 dark:text-gray-500 truncate">
                          {book.authors || 'Unknown author'}
                        </p>
                      </div>
                      <div class="flex items-center gap-3 flex-shrink-0">
                        <div class="w-20 h-1.5 rounded-full bg-gray-200 dark:bg-gray-700 overflow-hidden">
                          <div
                            class="h-full rounded-full bg-indigo-500"
                            style={{ width: `${Math.round(book.progress * 100)}%` }}
                          />
                        </div>
                        <span class="text-xs text-gray-400 dark:text-gray-500 w-8 text-right">
                          {Math.round(book.progress * 100)}%
                        </span>
                      </div>
                    </div>
                  )}
                </For>
              </div>
            </Show>
          </div>
        </div>

        {/* Quick Actions */}
        <div class="card p-5 h-fit">
          <h3 class="font-semibold mb-4">Quick Actions</h3>
          <div class="space-y-3">
            <button
              class="w-full flex items-center gap-3 px-4 py-3 rounded-lg text-left
                     bg-gray-50 dark:bg-gray-700/50 hover:bg-gray-100 dark:hover:bg-gray-700
                     transition-colors"
              onClick={() => navigate('/files')}
            >
              <span class="flex-shrink-0 p-2 rounded-lg bg-amber-100 dark:bg-amber-900/40">
                <svg
                  class="w-4 h-4 text-amber-700 dark:text-amber-300"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  viewBox="0 0 24 24"
                >
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
                  />
                </svg>
              </span>
              <div>
                <p class="text-sm font-medium">Scan Files</p>
                <p class="text-xs text-gray-400 dark:text-gray-500">
                  Find duplicates and analyse storage
                </p>
              </div>
            </button>

            <button
              class="w-full flex items-center gap-3 px-4 py-3 rounded-lg text-left
                     bg-gray-50 dark:bg-gray-700/50 hover:bg-gray-100 dark:hover:bg-gray-700
                     transition-colors"
              onClick={() => navigate('/finance')}
            >
              <span class="flex-shrink-0 p-2 rounded-lg bg-emerald-100 dark:bg-emerald-900/40">
                <svg
                  class="w-4 h-4 text-emerald-700 dark:text-emerald-300"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  viewBox="0 0 24 24"
                >
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    d="M7 16V4m0 0L3 8m4-4l4 4m6 0v12m0 0l4-4m-4 4l-4-4"
                  />
                </svg>
              </span>
              <div>
                <p class="text-sm font-medium">Import CSV</p>
                <p class="text-xs text-gray-400 dark:text-gray-500">
                  Import bank transactions from CSV
                </p>
              </div>
            </button>

            <button
              class="w-full flex items-center gap-3 px-4 py-3 rounded-lg text-left
                     bg-gray-50 dark:bg-gray-700/50 hover:bg-gray-100 dark:hover:bg-gray-700
                     transition-colors"
              onClick={() => navigate('/reader')}
            >
              <span class="flex-shrink-0 p-2 rounded-lg bg-indigo-100 dark:bg-indigo-900/40">
                <svg
                  class="w-4 h-4 text-indigo-700 dark:text-indigo-300"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  viewBox="0 0 24 24"
                >
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    d={MODULE_ICONS.reader}
                  />
                </svg>
              </span>
              <div>
                <p class="text-sm font-medium">Open Book</p>
                <p class="text-xs text-gray-400 dark:text-gray-500">
                  Browse and read your library
                </p>
              </div>
            </button>

            <button
              class="w-full flex items-center gap-3 px-4 py-3 rounded-lg text-left
                     bg-gray-50 dark:bg-gray-700/50 hover:bg-gray-100 dark:hover:bg-gray-700
                     transition-colors"
              onClick={() => navigate('/fitness')}
            >
              <span class="flex-shrink-0 p-2 rounded-lg bg-rose-100 dark:bg-rose-900/40">
                <svg
                  class="w-4 h-4 text-rose-700 dark:text-rose-300"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="2"
                  viewBox="0 0 24 24"
                >
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    d="M13 10V3L4 14h7v7l9-11h-7z"
                  />
                </svg>
              </span>
              <div>
                <p class="text-sm font-medium">Log Workout</p>
                <p class="text-xs text-gray-400 dark:text-gray-500">
                  Track exercise and daily habits
                </p>
              </div>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default Dashboard;
