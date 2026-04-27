# Health Intelligence Phase A — Fitness Real Data Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace all 9 hardcoded mock-data constant blocks in `ui/src/pages/Fitness.tsx` with live derivations from the `metrics` signal already loaded from SQLite. Wire `SleepTab`, `HeartTab`, and `ActivityTab` to accept a `metrics` prop and compute their own 7-day views. Add a sync status bar shown when Google Fit is connected. Replace the AI score/recommendation/doctor blocks in `AiAnalysisTab` with a placeholder prompt to configure an AI endpoint.

**Architecture:** Pure frontend change. The Tauri backend already writes Google Fit data to `fitness_metrics` via `gfit_sync`; `fitness_get_metrics` already returns the last 30 days. The three tab components are currently closed over the module-level mock constants — they will instead receive `metrics: () => FitnessMetricResponse[]` as a SolidJS prop and derive all rendered values from it. An `EmptyState` sub-component is added for zero-data displays. A `SyncStatusBar` component reads a `gfit_get_sync_status` invocation on mount. The existing `FitnessMetricResponse` TypeScript interface is expanded to match the full backend schema (adding `heart_rate_min`, `heart_rate_max`, `distance_m`, `active_minutes`, `spo2_avg`, `calories_out`, `synced_at`; removing the unused `notes` / `created_at` that are absent from the backend struct spec).

**Tech Stack:** SolidJS + TypeScript. Single file: `ui/src/pages/Fitness.tsx`. Only `pnpm typecheck` is needed to verify correctness.

---

## File Map

| Action | Path | Responsibility |
|---|---|---|
| Modify | `ui/src/pages/Fitness.tsx` | Expand interface, add SyncStatusBar + EmptyState, wire SleepTab / HeartTab / ActivityTab to real data, remove 9 mock constants, replace AI blocks |

---

## Task 1: Expand FitnessMetricResponse interface

**Files:**
- Modify: `ui/src/pages/Fitness.tsx`

The existing interface (lines 22–35) only has the fields returned by the original `fitness_get_metrics` command, which does not include min/max HR, distance, active minutes, or SpO2. The backend Google Fit sync writes all those fields. We expand the TypeScript interface so the tab components can use them when present (they will be `null` if the backend has not yet returned them).

- [ ] **Step 1: Write the failing typecheck**

Verify that referencing `heart_rate_min` on a `FitnessMetricResponse` currently errors:

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | head -5
```

Expected: passes (no errors yet — the new fields are only referenced after Step 2 below changes the tab components). This step confirms the baseline is clean.

- [ ] **Step 2: Replace the FitnessMetricResponse interface**

In `/home/dk/Documents/git/minion/ui/src/pages/Fitness.tsx`, replace lines 22–35:

```tsx
interface FitnessMetricResponse {
  id: string;
  date: string;
  weight_kg: number | null;
  body_fat_pct: number | null;
  steps: number | null;
  heart_rate_avg: number | null;
  sleep_hours: number | null;
  sleep_quality: number | null;
  water_ml: number | null;
  calories_in: number | null;
  notes: string | null;
  created_at: string;
}
```

with:

```tsx
interface FitnessMetricResponse {
  id: string;
  date: string;                    // "YYYY-MM-DD"
  weight_kg: number | null;
  body_fat_pct: number | null;
  steps: number | null;
  heart_rate_avg: number | null;
  heart_rate_min: number | null;
  heart_rate_max: number | null;
  sleep_hours: number | null;
  sleep_quality: number | null;    // 0–100 score
  water_ml: number | null;
  calories_in: number | null;
  calories_out: number | null;
  distance_m: number | null;
  active_minutes: number | null;
  spo2_avg: number | null;
  source: string | null;
  synced_at: string | null;
}
```

- [ ] **Step 3: Add GfitSyncStatus interface**

Directly after the closing `}` of `FitnessMetricResponse`, add:

```tsx
interface GfitSyncStatus {
  last_synced: string | null;  // ISO datetime string or null
  days_count: number;
  running: boolean;
  pct: number;
  message: string;
}
```

- [ ] **Step 4: Run typecheck — confirm baseline still passes**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | grep -E "error TS" | head -10
```

Expected: no errors. (Nothing references the new fields yet.)

- [ ] **Step 5: Commit**

```bash
cd /home/dk/Documents/git/minion
git add ui/src/pages/Fitness.tsx
git commit -m "feat(fitness): expand FitnessMetricResponse — hr min/max, distance, active_minutes, spo2, GfitSyncStatus"
```

---

## Task 2: Add EmptyState and SyncStatusBar sub-components

**Files:**
- Modify: `ui/src/pages/Fitness.tsx`

These two helpers are shared by all three wired tabs. `EmptyState` renders a centred prompt whenever a tab has no rows. `SyncStatusBar` is shown above tab content when Google Fit is connected — it loads `gfit_get_sync_status` once on mount and exposes a manual "↻ Sync" button.

- [ ] **Step 1: Add EmptyState component**

Find the line:

```tsx
/** Mini inline progress bar */
const MiniBar: Component<{ value: number; max: number; colorClass?: string }> = (props) => {
```

Insert the following block immediately before it (after the closing `};` of `CircularProgress`):

```tsx
/** Empty state shown when a tab has no real data to display */
const EmptyState: Component<{ icon: string; message: string }> = (props) => (
  <div class="flex flex-col items-center justify-center py-16 gap-4 text-center">
    <div class="p-4 rounded-full bg-gray-100 dark:bg-gray-800 text-gray-400">
      <Icon name={props.icon} class="w-10 h-10" />
    </div>
    <p class="text-sm text-gray-500 dark:text-gray-400 max-w-xs leading-relaxed">
      {props.message}
    </p>
  </div>
);

```

- [ ] **Step 2: Add SyncStatusBar component**

After the `EmptyState` component (before `MiniBar`), add:

```tsx
/** Sync status bar shown above tab content when Google Fit is connected */
const SyncStatusBar: Component<{ onSync: () => Promise<void> }> = (props) => {
  const [status, setStatus] = createSignal<GfitSyncStatus | null>(null);
  const [syncing, setSyncing] = createSignal(false);

  const load = async () => {
    try {
      const s = await invoke<GfitSyncStatus>('gfit_get_sync_status');
      setStatus(s);
    } catch {
      // ignore — command may not exist in older builds
    }
  };

  onMount(load);

  const relativeTime = () => {
    const s = status();
    if (!s?.last_synced) return 'never';
    const diff = Date.now() - new Date(s.last_synced).getTime();
    const mins = Math.floor(diff / 60_000);
    if (mins < 1) return 'just now';
    if (mins < 60) return `${mins} min ago`;
    const hrs = Math.floor(mins / 60);
    if (hrs < 24) return `${hrs}h ago`;
    return `${Math.floor(hrs / 24)}d ago`;
  };

  const handleSync = async () => {
    setSyncing(true);
    try {
      await props.onSync();
      await load();
    } finally {
      setSyncing(false);
    }
  };

  return (
    <div class="flex items-center justify-between px-4 py-2 mb-4 rounded-lg
                bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800
                text-xs text-green-700 dark:text-green-300">
      <div class="flex items-center gap-2">
        <div class="w-1.5 h-1.5 rounded-full bg-green-500" />
        <span>
          Google Fit — last synced: <strong>{relativeTime()}</strong>
          <Show when={(status()?.days_count ?? 0) > 0}>
            {' '}({status()?.days_count} days)
          </Show>
        </span>
        <Show when={status()?.running}>
          <span class="animate-pulse ml-1">syncing…</span>
        </Show>
      </div>
      <button
        class="flex items-center gap-1 px-2 py-0.5 rounded bg-green-100 dark:bg-green-800
               hover:bg-green-200 dark:hover:bg-green-700 transition-colors font-medium"
        disabled={syncing() || status()?.running}
        onClick={handleSync}
        title="Sync Google Fit data now"
      >
        <svg
          class={`w-3 h-3 ${syncing() ? 'animate-spin' : ''}`}
          fill="none" stroke="currentColor" viewBox="0 0 24 24"
        >
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003
               8.003 0 01-15.357-2m15.357 2H15" />
        </svg>
        {syncing() ? 'Syncing…' : '↻ Sync'}
      </button>
    </div>
  );
};

```

- [ ] **Step 3: Run typecheck**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | grep -E "error TS" | head -10
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
cd /home/dk/Documents/git/minion
git add ui/src/pages/Fitness.tsx
git commit -m "feat(fitness): add EmptyState and SyncStatusBar sub-components"
```

---

## Task 3: Wire SleepTab to real data

**Files:**
- Modify: `ui/src/pages/Fitness.tsx`

**Current state (lines 628–749):**
- `SleepTab: Component` — no props, reads `WEEKLY_SLEEP` / `SLEEP_STAGES` / hardcoded numbers
- Called at line 2140 as `<SleepTab />`

**Target state:**
- `SleepTab: Component<{ metrics: () => FitnessMetricResponse[]; gfitConnected: () => boolean; onSync: () => Promise<void> }>` 
- Derives 7-day sleep bars from `metrics().slice(0, 7).reverse()`
- Shows `EmptyState` when no metrics have `sleep_hours` data
- Shows `SyncStatusBar` at the top when `gfitConnected()` is true
- Sleep Score CircularProgress uses `sleep_quality` of the most recent record (or computes from `sleep_hours` if null)
- Last Night Summary shows most recent record's `sleep_hours` formatted as `Xh Ym` (bedtime/wake are shown as "—" since the backend doesn't store them)
- Sleep Stages section is replaced with a "Sleep quality by night" bar list (each bar = that day's `sleep_quality` score; falls back to a `sleep_hours`-derived score if `sleep_quality` is null)
- Sleep Quality Factors section is replaced with a computed summary: avg, best, worst over the 7 days

- [ ] **Step 1: Replace the SleepTab component**

Find and replace the entire `SleepTab` component (lines 628–750 in the original file, from `const SleepTab: Component = () => {` to the closing `};`):

```tsx
const SleepTab: Component<{
  metrics: () => FitnessMetricResponse[];
  gfitConnected: () => boolean;
  onSync: () => Promise<void>;
}> = (props) => {
  // Last 7 days of metrics that have sleep data, oldest-first for left-to-right chart
  const sleepRows = () =>
    props
      .metrics()
      .filter((m) => m.sleep_hours !== null)
      .slice(0, 7)
      .reverse();

  const hasSleepData = () => sleepRows().length > 0;

  // Most recent record with sleep data
  const latest = () => sleepRows()[sleepRows().length - 1] ?? null;

  const maxSleep = () => Math.max(...sleepRows().map((m) => m.sleep_hours ?? 0), 1);

  // Sleep score: use sleep_quality if available, else derive from sleep_hours (7h = 85)
  const sleepScore = () => {
    const q = latest()?.sleep_quality;
    if (q !== null && q !== undefined) return Math.round(q);
    const h = latest()?.sleep_hours ?? 0;
    return Math.min(Math.round((h / 9) * 100), 100);
  };

  const sleepScoreLabel = () => {
    const s = sleepScore();
    if (s >= 80) return 'Excellent';
    if (s >= 65) return 'Good';
    if (s >= 50) return 'Fair';
    return 'Poor';
  };

  const fmtHours = (h: number | null) => {
    if (h === null) return '—';
    const hrs = Math.floor(h);
    const mins = Math.round((h - hrs) * 60);
    return `${hrs}h ${mins.toString().padStart(2, '0')}m`;
  };

  const avgSleep = () => {
    const rows = sleepRows();
    if (rows.length === 0) return 0;
    return rows.reduce((s, m) => s + (m.sleep_hours ?? 0), 0) / rows.length;
  };

  const bestSleep = () =>
    Math.max(...sleepRows().map((m) => m.sleep_hours ?? 0), 0);

  const worstSleep = () => {
    const vals = sleepRows().map((m) => m.sleep_hours ?? 0).filter((v) => v > 0);
    return vals.length > 0 ? Math.min(...vals) : 0;
  };

  const dayLabel = (dateStr: string) => {
    const days = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
    return days[new Date(dateStr).getDay()];
  };

  return (
    <div class="space-y-6">
      <Show when={props.gfitConnected()}>
        <SyncStatusBar onSync={props.onSync} />
      </Show>

      <Show
        when={hasSleepData()}
        fallback={
          <EmptyState
            icon="moon"
            message="No sleep data yet. Sync Google Fit or log sleep hours using the Log Today's Data form above."
          />
        }
      >
        {/* Top: Sleep Score + Last Night Summary */}
        <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* Sleep Score */}
          <div class="card p-6 flex flex-col items-center justify-center">
            <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
              Sleep Score
            </h3>
            <CircularProgress
              value={sleepScore()}
              max={100}
              size="w-40 h-40"
              colorClass="text-indigo-500"
              sublabel="/ 100"
            />
            <p class="mt-3 text-sm text-gray-500 dark:text-gray-400">{sleepScoreLabel()}</p>
          </div>

          {/* Last Night Summary */}
          <div class="lg:col-span-2 card p-6">
            <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
              Last Night Summary
            </h3>
            <div class="grid grid-cols-2 sm:grid-cols-4 gap-4 mb-6">
              <div>
                <p class="text-xs text-gray-400">Date</p>
                <p class="text-lg font-semibold text-gray-900 dark:text-white">
                  {latest()?.date ?? '—'}
                </p>
              </div>
              <div>
                <p class="text-xs text-gray-400">Duration</p>
                <p class="text-lg font-semibold text-gray-900 dark:text-white">
                  {fmtHours(latest()?.sleep_hours ?? null)}
                </p>
              </div>
              <div>
                <p class="text-xs text-gray-400">Quality Score</p>
                <p class="text-lg font-semibold text-gray-900 dark:text-white">
                  {latest()?.sleep_quality !== null && latest()?.sleep_quality !== undefined
                    ? `${latest()!.sleep_quality}/100`
                    : '—'}
                </p>
              </div>
              <div>
                <p class="text-xs text-gray-400">vs Target (7h)</p>
                <p
                  class="text-lg font-semibold"
                  classList={{
                    'text-green-500': (latest()?.sleep_hours ?? 0) >= 7,
                    'text-amber-500':
                      (latest()?.sleep_hours ?? 0) >= 6 && (latest()?.sleep_hours ?? 0) < 7,
                    'text-red-500': (latest()?.sleep_hours ?? 0) < 6,
                  }}
                >
                  {(latest()?.sleep_hours ?? 0) >= 7
                    ? 'Met'
                    : `−${fmtHours(7 - (latest()?.sleep_hours ?? 0))}`}
                </p>
              </div>
            </div>

            {/* Per-night quality bars */}
            <h4 class="text-xs text-gray-400 mb-3 uppercase tracking-wide">
              Sleep Quality by Night
            </h4>
            <div class="space-y-3">
              <For each={sleepRows()}>
                {(row) => {
                  const score = row.sleep_quality !== null
                    ? Math.round(row.sleep_quality)
                    : Math.min(Math.round(((row.sleep_hours ?? 0) / 9) * 100), 100);
                  return (
                    <div class="flex items-center gap-3">
                      <span class="text-sm text-gray-600 dark:text-gray-300 w-10 shrink-0">
                        {dayLabel(row.date)}
                      </span>
                      <div class="flex-1 h-4 rounded-full bg-gray-200 dark:bg-gray-700 overflow-hidden">
                        <div
                          class="h-full rounded-full bg-indigo-500 transition-all duration-500"
                          style={{ width: `${score}%` }}
                        />
                      </div>
                      <span class="text-sm text-gray-500 dark:text-gray-400 w-20 text-right shrink-0">
                        {fmtHours(row.sleep_hours)} {score > 0 ? `(${score})` : ''}
                      </span>
                    </div>
                  );
                }}
              </For>
            </div>
          </div>
        </div>

        {/* 7-Day Sleep Trend */}
        <div class="card p-6">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
            7-Day Sleep Trend
          </h3>
          <div class="flex items-end gap-3 h-40">
            <For each={sleepRows()}>
              {(entry) => (
                <div class="flex-1 flex flex-col items-center gap-1">
                  <span class="text-xs text-gray-400">
                    {(entry.sleep_hours ?? 0).toFixed(1)}h
                  </span>
                  <div class="w-full flex justify-center">
                    <div
                      class="w-full max-w-[40px] rounded-t-md bg-indigo-500 dark:bg-indigo-400 transition-all duration-500"
                      style={{
                        height: `${((entry.sleep_hours ?? 0) / maxSleep()) * 120}px`,
                      }}
                    />
                  </div>
                  <span class="text-xs font-medium text-gray-500 dark:text-gray-400">
                    {dayLabel(entry.date)}
                  </span>
                </div>
              )}
            </For>
          </div>
        </div>

        {/* Sleep Summary Stats */}
        <div class="card p-6">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
            7-Day Sleep Summary
          </h3>
          <div class="grid grid-cols-3 gap-4 text-center">
            <div>
              <p class="text-2xl font-bold text-gray-900 dark:text-white">
                {fmtHours(avgSleep())}
              </p>
              <p class="text-xs text-gray-400 mt-1">Average</p>
              <MiniBar
                value={avgSleep()}
                max={9}
                colorClass={avgSleep() >= 7 ? 'bg-green-500' : avgSleep() >= 6 ? 'bg-amber-500' : 'bg-red-500'}
              />
            </div>
            <div>
              <p class="text-2xl font-bold text-green-500">{fmtHours(bestSleep())}</p>
              <p class="text-xs text-gray-400 mt-1">Best night</p>
              <MiniBar value={bestSleep()} max={9} colorClass="bg-green-500" />
            </div>
            <div>
              <p class="text-2xl font-bold text-red-400">{fmtHours(worstSleep())}</p>
              <p class="text-xs text-gray-400 mt-1">Worst night</p>
              <MiniBar value={worstSleep()} max={9} colorClass="bg-red-400" />
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
};
```

- [ ] **Step 2: Update the SleepTab call site**

Find in the `<Switch>` block (around line 2139):

```tsx
          <Match when={activeTab() === 'sleep'}>
            <SleepTab />
          </Match>
```

Replace with:

```tsx
          <Match when={activeTab() === 'sleep'}>
            <SleepTab
              metrics={metrics}
              gfitConnected={gfitConnected}
              onSync={async () => { await invoke('gfit_sync'); await loadData(); }}
            />
          </Match>
```

- [ ] **Step 3: Run typecheck**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | grep -E "error TS" | head -15
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
cd /home/dk/Documents/git/minion
git add ui/src/pages/Fitness.tsx
git commit -m "feat(fitness): wire SleepTab to real metrics — 7-day trend, quality bars, summary stats, empty state"
```

---

## Task 4: Wire HeartTab to real data

**Files:**
- Modify: `ui/src/pages/Fitness.tsx`

**Current state (lines 752–925):**
- `HeartTab: Component` — no props, reads `WEEKLY_HEART_RATE` / `HEART_RATE_ZONES` / hardcoded BPM values
- Called at line 2143 as `<HeartTab />`

**Target state:**
- `HeartTab: Component<{ metrics: () => FitnessMetricResponse[]; gfitConnected: () => boolean; onSync: () => Promise<void> }>`
- 7-day HR line chart computed from `heart_rate_avg` / `heart_rate_min` / `heart_rate_max`
- Resting HR circular gauge shows the 7-day average of `heart_rate_avg` (excluding nulls)
- "Current heart rate" shows most recent day's `heart_rate_avg`
- HR zones block is removed (data not available from backend); replaced with a simple per-day min/max range chart
- EmptyState shown when no HR data

- [ ] **Step 1: Replace the HeartTab component**

Find and replace the entire `HeartTab` component (from `const HeartTab: Component = () => {` to its closing `};`):

```tsx
const HeartTab: Component<{
  metrics: () => FitnessMetricResponse[];
  gfitConnected: () => boolean;
  onSync: () => Promise<void>;
}> = (props) => {
  // Last 7 days that have HR data, oldest-first
  const hrRows = () =>
    props
      .metrics()
      .filter((m) => m.heart_rate_avg !== null)
      .slice(0, 7)
      .reverse();

  const hasHrData = () => hrRows().length > 0;

  const latest = () => hrRows()[hrRows().length - 1] ?? null;

  const avgRestingHr = () => {
    const rows = hrRows();
    if (rows.length === 0) return 0;
    return Math.round(
      rows.reduce((s, m) => s + (m.heart_rate_avg ?? 0), 0) / rows.length
    );
  };

  const dayLabel = (dateStr: string) => {
    const days = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
    return days[new Date(dateStr).getDay()];
  };

  // SVG chart helpers
  const chartWidth = 600;
  const chartHeight = 120;

  const allAvg = () => hrRows().map((d) => d.heart_rate_avg ?? 0);
  const allMin = () => hrRows().map((d) => d.heart_rate_min ?? d.heart_rate_avg ?? 0);
  const allMax = () => hrRows().map((d) => d.heart_rate_max ?? d.heart_rate_avg ?? 0);

  const hrFloor = () => Math.max(Math.min(...allMin()) - 5, 40);
  const hrCeil = () => Math.max(...allMax()) + 5;

  const scaleY = (v: number) => {
    const floor = hrFloor();
    const ceil = hrCeil();
    return chartHeight - ((v - floor) / (ceil - floor)) * chartHeight;
  };

  const pointsFor = (vals: () => number[]) => () =>
    vals()
      .map((v, i) => `${(i / Math.max(vals().length - 1, 1)) * chartWidth},${scaleY(v)}`)
      .join(' ');

  const avgPoints = pointsFor(allAvg);
  const minPoints = pointsFor(allMin);
  const maxPoints = pointsFor(allMax);

  const hrRangeLabel = () => {
    const hr = latest()?.heart_rate_avg;
    if (hr === null || hr === undefined) return '—';
    if (hr < 60) return 'Low';
    if (hr <= 100) return 'Normal range';
    return 'Elevated';
  };

  return (
    <div class="space-y-6">
      <Show when={props.gfitConnected()}>
        <SyncStatusBar onSync={props.onSync} />
      </Show>

      <Show
        when={hasHrData()}
        fallback={
          <EmptyState
            icon="heart"
            message="No heart rate data yet. Sync Google Fit or log your average BPM using the Log Today's Data form above."
          />
        }
      >
        {/* Current BPM + Resting HR */}
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-6">
          {/* Current BPM */}
          <div class="card p-6 flex flex-col items-center justify-center">
            <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
              Latest Heart Rate
            </h3>
            <div class="flex items-center gap-4">
              <div class="animate-pulse-heart text-red-500">
                <Icon name="heart" class="w-12 h-12" />
              </div>
              <div>
                <span class="text-5xl font-bold text-gray-900 dark:text-white">
                  {latest()?.heart_rate_avg ?? '—'}
                </span>
                <span class="text-lg text-gray-400 ml-1">BPM</span>
              </div>
            </div>
            <p class="text-sm text-gray-400 mt-3">{hrRangeLabel()}</p>
            <Show when={latest()?.heart_rate_min !== null && latest()?.heart_rate_max !== null}>
              <p class="text-xs text-gray-400 mt-1">
                Range: {latest()?.heart_rate_min}–{latest()?.heart_rate_max} BPM
              </p>
            </Show>
            {/* Inline keyframe style for pulse animation */}
            <style>{`
              @keyframes pulse-heart {
                0%, 100% { transform: scale(1); }
                15% { transform: scale(1.2); }
                30% { transform: scale(1); }
                45% { transform: scale(1.15); }
                60% { transform: scale(1); }
              }
              .animate-pulse-heart {
                animation: pulse-heart 1.2s ease-in-out infinite;
              }
            `}</style>
          </div>

          {/* 7-Day Average (resting HR proxy) */}
          <div class="card p-6 flex flex-col items-center justify-center">
            <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
              7-Day Avg Heart Rate
            </h3>
            <CircularProgress
              value={avgRestingHr()}
              max={200}
              size="w-36 h-36"
              colorClass="text-red-500"
              sublabel="BPM"
            />
            <p class="mt-3 text-sm text-gray-500 dark:text-gray-400">
              {hrRows().length} day{hrRows().length !== 1 ? 's' : ''} of data
            </p>
          </div>
        </div>

        {/* 7-Day Trend (SVG line chart) */}
        <div class="card p-6">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
            7-Day Heart Rate Trend
          </h3>
          <Show
            when={hrRows().length > 1}
            fallback={
              <p class="text-sm text-gray-400 text-center py-8">
                Need at least 2 days of data to draw a trend line.
              </p>
            }
          >
            <div class="overflow-x-auto">
              <svg
                viewBox={`-30 -10 ${chartWidth + 60} ${chartHeight + 40}`}
                class="w-full h-44"
              >
                {/* Y-axis reference lines at floor, mid, ceil */}
                {[hrFloor(), Math.round((hrFloor() + hrCeil()) / 2), hrCeil()].map((v) => (
                  <>
                    <text
                      x="-10"
                      y={scaleY(v) + 4}
                      class="fill-gray-400"
                      text-anchor="end"
                      font-size="10"
                    >
                      {v}
                    </text>
                    <line
                      x1="0"
                      y1={scaleY(v)}
                      x2={chartWidth}
                      y2={scaleY(v)}
                      stroke="currentColor"
                      class="text-gray-200 dark:text-gray-700"
                      stroke-dasharray="4"
                    />
                  </>
                ))}
                {/* Max line */}
                <Show when={hrRows().some((r) => r.heart_rate_max !== null)}>
                  <polyline
                    points={maxPoints()}
                    fill="none"
                    stroke="#fca5a5"
                    stroke-width="1.5"
                    stroke-dasharray="4"
                  />
                </Show>
                {/* Avg line */}
                <polyline
                  points={avgPoints()}
                  fill="none"
                  stroke="#ef4444"
                  stroke-width="2.5"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
                {/* Min line */}
                <Show when={hrRows().some((r) => r.heart_rate_min !== null)}>
                  <polyline
                    points={minPoints()}
                    fill="none"
                    stroke="#93c5fd"
                    stroke-width="1.5"
                    stroke-dasharray="4"
                  />
                </Show>
                {/* Dots on avg */}
                <For each={hrRows()}>
                  {(d, i) => (
                    <circle
                      cx={(i() / Math.max(hrRows().length - 1, 1)) * chartWidth}
                      cy={scaleY(d.heart_rate_avg ?? 0)}
                      r="4"
                      fill="#ef4444"
                    />
                  )}
                </For>
                {/* X-axis labels */}
                <For each={hrRows()}>
                  {(d, i) => (
                    <text
                      x={(i() / Math.max(hrRows().length - 1, 1)) * chartWidth}
                      y={chartHeight + 18}
                      text-anchor="middle"
                      class="fill-gray-400"
                      font-size="10"
                    >
                      {dayLabel(d.date)}
                    </text>
                  )}
                </For>
              </svg>
            </div>
            <div class="flex items-center justify-center gap-6 mt-2 text-xs text-gray-400">
              <div class="flex items-center gap-1">
                <div class="w-6 h-0.5 bg-red-500 rounded" />
                <span>Avg</span>
              </div>
              <div class="flex items-center gap-1">
                <div class="w-6 h-0.5 bg-red-300 rounded" />
                <span>Max</span>
              </div>
              <div class="flex items-center gap-1">
                <div class="w-6 h-0.5 bg-blue-300 rounded" />
                <span>Min</span>
              </div>
            </div>
          </Show>
        </div>

        {/* Per-day min/avg/max table */}
        <div class="card p-6">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
            Daily Breakdown
          </h3>
          <div class="overflow-x-auto">
            <table class="w-full text-sm text-left">
              <thead>
                <tr class="text-xs text-gray-400 border-b border-gray-200 dark:border-gray-700">
                  <th class="pb-2 font-medium">Day</th>
                  <th class="pb-2 font-medium text-right">Min</th>
                  <th class="pb-2 font-medium text-right">Avg</th>
                  <th class="pb-2 font-medium text-right">Max</th>
                </tr>
              </thead>
              <tbody>
                <For each={[...hrRows()].reverse()}>
                  {(row) => (
                    <tr class="border-b border-gray-100 dark:border-gray-800 last:border-0">
                      <td class="py-2 text-gray-700 dark:text-gray-200">{row.date}</td>
                      <td class="py-2 text-right text-blue-500">
                        {row.heart_rate_min ?? '—'}
                      </td>
                      <td class="py-2 text-right font-semibold text-gray-900 dark:text-white">
                        {row.heart_rate_avg ?? '—'}
                      </td>
                      <td class="py-2 text-right text-red-400">
                        {row.heart_rate_max ?? '—'}
                      </td>
                    </tr>
                  )}
                </For>
              </tbody>
            </table>
          </div>
        </div>
      </Show>
    </div>
  );
};
```

- [ ] **Step 2: Update the HeartTab call site**

Find (around line 2143):

```tsx
          <Match when={activeTab() === 'heart'}>
            <HeartTab />
          </Match>
```

Replace with:

```tsx
          <Match when={activeTab() === 'heart'}>
            <HeartTab
              metrics={metrics}
              gfitConnected={gfitConnected}
              onSync={async () => { await invoke('gfit_sync'); await loadData(); }}
            />
          </Match>
```

- [ ] **Step 3: Run typecheck**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | grep -E "error TS" | head -15
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
cd /home/dk/Documents/git/minion
git add ui/src/pages/Fitness.tsx
git commit -m "feat(fitness): wire HeartTab to real metrics — 7-day SVG trend, daily breakdown, empty state"
```

---

## Task 5: Wire ActivityTab to real data

**Files:**
- Modify: `ui/src/pages/Fitness.tsx`

**Current state (lines 927–1036):**
- `ActivityTab: Component` — no props, reads `WEEKLY_STEPS` / hardcoded distances / active minutes / calorie breakdown
- Called at line 2145 as `<ActivityTab />`

**Target state:**
- `ActivityTab: Component<{ metrics: () => FitnessMetricResponse[]; gfitConnected: () => boolean; onSync: () => Promise<void> }>`
- 7-day step bars from `steps` field
- Today's card shows most recent record's `steps`, `distance_m` (converted to km), `active_minutes`
- Weekly totals / daily avg computed from the 7 rows
- Calorie breakdown uses `calories_out` (total burned) from most recent record; basal is estimated as `calories_out - (calories_out * 0.2)` if no separate field exists
- EmptyState when no activity data

- [ ] **Step 1: Replace the ActivityTab component**

Find and replace the entire `ActivityTab` component (from `const ActivityTab: Component = () => {` to its closing `};`):

```tsx
const ActivityTab: Component<{
  metrics: () => FitnessMetricResponse[];
  gfitConnected: () => boolean;
  onSync: () => Promise<void>;
}> = (props) => {
  // Last 7 days that have at least steps or distance, oldest-first
  const activityRows = () =>
    props
      .metrics()
      .filter((m) => m.steps !== null || m.distance_m !== null || m.active_minutes !== null)
      .slice(0, 7)
      .reverse();

  const hasActivityData = () => activityRows().length > 0;

  const latest = () => activityRows()[activityRows().length - 1] ?? null;

  const maxSteps = () => Math.max(...activityRows().map((m) => m.steps ?? 0), 1);

  const weeklySteps = () => activityRows().reduce((s, m) => s + (m.steps ?? 0), 0);
  const dailyAvgSteps = () => {
    const rows = activityRows();
    return rows.length > 0 ? Math.round(weeklySteps() / rows.length) : 0;
  };

  const distanceKm = () =>
    latest()?.distance_m !== null && latest()?.distance_m !== undefined
      ? ((latest()!.distance_m ?? 0) / 1000).toFixed(2)
      : null;

  const activeMin = () => latest()?.active_minutes ?? null;

  const dayLabel = (dateStr: string) => {
    const days = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
    return days[new Date(dateStr).getDay()];
  };

  const isToday = (dateStr: string) =>
    dateStr === new Date().toISOString().slice(0, 10);

  // Calorie breakdown from most recent record
  const caloriesOut = () => latest()?.calories_out ?? null;
  const caloriesActive = () =>
    caloriesOut() !== null ? Math.round((caloriesOut() ?? 0) * 0.2) : null;
  const caloriesBasal = () =>
    caloriesOut() !== null
      ? Math.round((caloriesOut() ?? 0) - (caloriesActive() ?? 0))
      : null;

  return (
    <div class="space-y-6">
      <Show when={props.gfitConnected()}>
        <SyncStatusBar onSync={props.onSync} />
      </Show>

      <Show
        when={hasActivityData()}
        fallback={
          <EmptyState
            icon="steps"
            message="No activity data yet. Sync Google Fit or log steps using the Log Today's Data form above."
          />
        }
      >
        {/* Top metrics */}
        <div class="grid grid-cols-2 lg:grid-cols-4 gap-4">
          {/* Steps */}
          <div class="card p-5 flex flex-col items-center">
            <CircularProgress
              value={latest()?.steps ?? 0}
              max={10000}
              size="w-24 h-24"
              colorClass="text-blue-500"
              sublabel="steps"
            />
            <p class="mt-2 text-sm font-medium text-gray-600 dark:text-gray-300">Steps</p>
            <p class="text-xs text-gray-400">Goal: 10,000</p>
          </div>

          {/* Distance */}
          <div class="card p-5 flex flex-col items-center">
            <div class="p-3 rounded-full bg-green-100 dark:bg-green-900/40 text-green-500 mb-2">
              <Icon name="distance" class="w-8 h-8" />
            </div>
            <Show
              when={distanceKm() !== null}
              fallback={<p class="text-2xl font-bold text-gray-400">—</p>}
            >
              <p class="text-2xl font-bold text-gray-900 dark:text-white">{distanceKm()}</p>
              <p class="text-sm text-gray-400">km</p>
              <MiniBar value={parseFloat(distanceKm() ?? '0')} max={8} colorClass="bg-green-500" />
            </Show>
          </div>

          {/* Active Minutes */}
          <div class="card p-5 flex flex-col items-center">
            <div class="p-3 rounded-full bg-orange-100 dark:bg-orange-900/40 text-orange-500 mb-2">
              <Icon name="clock" class="w-8 h-8" />
            </div>
            <Show
              when={activeMin() !== null}
              fallback={<p class="text-2xl font-bold text-gray-400">—</p>}
            >
              <p class="text-2xl font-bold text-gray-900 dark:text-white">{activeMin()}</p>
              <p class="text-sm text-gray-400">active min</p>
              <MiniBar value={activeMin() ?? 0} max={60} colorClass="bg-orange-500" />
            </Show>
          </div>

          {/* Calories Out */}
          <div class="card p-5 flex flex-col items-center">
            <div class="p-3 rounded-full bg-red-100 dark:bg-red-900/40 text-red-500 mb-2">
              <Icon name="fire" class="w-8 h-8" />
            </div>
            <Show
              when={caloriesOut() !== null}
              fallback={<p class="text-2xl font-bold text-gray-400">—</p>}
            >
              <p class="text-2xl font-bold text-gray-900 dark:text-white">
                {Math.round(caloriesOut() ?? 0).toLocaleString()}
              </p>
              <p class="text-sm text-gray-400">cal burned</p>
              <MiniBar value={caloriesOut() ?? 0} max={3000} colorClass="bg-red-500" />
            </Show>
          </div>
        </div>

        {/* Weekly Activity Bars */}
        <div class="card p-6">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
            Weekly Steps
          </h3>
          <div class="flex items-end gap-3 h-44">
            <For each={activityRows()}>
              {(entry) => {
                const today = isToday(entry.date);
                return (
                  <div class="flex-1 flex flex-col items-center gap-1">
                    <span class="text-xs text-gray-400">
                      {((entry.steps ?? 0) / 1000).toFixed(1)}k
                    </span>
                    <div class="w-full flex justify-center">
                      <div
                        class={`w-full max-w-[40px] rounded-t-md transition-all duration-500 ${
                          today
                            ? 'bg-minion-500 dark:bg-minion-400'
                            : 'bg-gray-300 dark:bg-gray-600'
                        }`}
                        style={{ height: `${((entry.steps ?? 0) / maxSteps()) * 140}px` }}
                      />
                    </div>
                    <span
                      class={`text-xs font-medium ${
                        today
                          ? 'text-minion-600 dark:text-minion-400'
                          : 'text-gray-500 dark:text-gray-400'
                      }`}
                    >
                      {dayLabel(entry.date)}
                    </span>
                  </div>
                );
              }}
            </For>
          </div>
          <div class="flex items-center justify-between mt-4 text-sm text-gray-500 dark:text-gray-400">
            <span>
              Weekly total:{' '}
              <strong class="text-gray-900 dark:text-white">
                {weeklySteps().toLocaleString()}
              </strong>{' '}
              steps
            </span>
            <span>
              Daily avg:{' '}
              <strong class="text-gray-900 dark:text-white">
                {dailyAvgSteps().toLocaleString()}
              </strong>{' '}
              steps
            </span>
          </div>
        </div>

        {/* Calorie Breakdown */}
        <Show when={caloriesOut() !== null}>
          <div class="card p-6">
            <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
              Calorie Breakdown (Latest Day)
            </h3>
            <div class="grid grid-cols-3 gap-4 text-center">
              <div>
                <p class="text-3xl font-bold text-gray-900 dark:text-white">
                  {(caloriesBasal() ?? 0).toLocaleString()}
                </p>
                <p class="text-sm text-gray-400">Basal (est.)</p>
              </div>
              <div>
                <p class="text-3xl font-bold text-orange-500">
                  {(caloriesActive() ?? 0).toLocaleString()}
                </p>
                <p class="text-sm text-gray-400">Active (est.)</p>
              </div>
              <div>
                <p class="text-3xl font-bold text-gray-900 dark:text-white">
                  {Math.round(caloriesOut() ?? 0).toLocaleString()}
                </p>
                <p class="text-sm text-gray-400">Total</p>
              </div>
            </div>
          </div>
        </Show>
      </Show>
    </div>
  );
};
```

- [ ] **Step 2: Update the ActivityTab call site**

Find (around line 2145):

```tsx
          <Match when={activeTab() === 'activity'}>
            <ActivityTab />
          </Match>
```

Replace with:

```tsx
          <Match when={activeTab() === 'activity'}>
            <ActivityTab
              metrics={metrics}
              gfitConnected={gfitConnected}
              onSync={async () => { await invoke('gfit_sync'); await loadData(); }}
            />
          </Match>
```

- [ ] **Step 3: Run typecheck**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | grep -E "error TS" | head -15
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
cd /home/dk/Documents/git/minion
git add ui/src/pages/Fitness.tsx
git commit -m "feat(fitness): wire ActivityTab to real metrics — steps/distance/active-min/calories, weekly bars, empty state"
```

---

## Task 6: Remove hardcoded constants and update AiAnalysisTab

**Files:**
- Modify: `ui/src/pages/Fitness.tsx`

This task removes the 9 mock-data constants (lines 114–223 in the original file) and replaces the `AI_HEALTH_SCORES` / `AI_RECOMMENDATIONS` / `DOCTOR_SUGGESTIONS` rendering blocks inside `AiAnalysisTab` with a placeholder card.

- [ ] **Step 1: Delete the 9 mock-data constant blocks**

Remove the entire block from the comment line `// ---------------------------------------------------------------------------` that precedes `const WEEKLY_STEPS` through the closing `];` of `DEFAULT_HABITS` (original lines 110–223). The block to delete is:

```tsx
// ---------------------------------------------------------------------------
// Mock data (structured for future Google Fit API integration)
// ---------------------------------------------------------------------------

const WEEKLY_STEPS = [
  { day: 'Mon', steps: 9200 },
  { day: 'Tue', steps: 7800 },
  { day: 'Wed', steps: 11400 },
  { day: 'Thu', steps: 6500 },
  { day: 'Fri', steps: 8900 },
  { day: 'Sat', steps: 12100 },
  { day: 'Sun', steps: 8432 },
];

const WEEKLY_SLEEP = [
  { day: 'Mon', hours: 7.2 },
  { day: 'Tue', hours: 6.8 },
  { day: 'Wed', hours: 7.5 },
  { day: 'Thu', hours: 8.1 },
  { day: 'Fri', hours: 6.5 },
  { day: 'Sat', hours: 7.8 },
  { day: 'Sun', hours: 7.4 },
];

const SLEEP_STAGES: SleepStage[] = [
  { label: 'Deep Sleep', duration: '1h 42m', minutes: 102, color: 'bg-indigo-600' },
  { label: 'Light Sleep', duration: '3h 18m', minutes: 198, color: 'bg-blue-400' },
  { label: 'REM', duration: '1h 53m', minutes: 113, color: 'bg-purple-500' },
  { label: 'Awake', duration: '0h 30m', minutes: 30, color: 'bg-gray-400' },
];

const HEART_RATE_ZONES: HeartRateZone[] = [
  { label: 'Rest', bpmRange: '< 100', minutes: 1120, maxMinutes: 1440, color: 'bg-blue-400' },
  { label: 'Fat Burn', bpmRange: '100-140', minutes: 185, maxMinutes: 1440, color: 'bg-green-500' },
  { label: 'Cardio', bpmRange: '140-170', minutes: 95, maxMinutes: 1440, color: 'bg-orange-500' },
  { label: 'Peak', bpmRange: '170+', minutes: 40, maxMinutes: 1440, color: 'bg-red-500' },
];

const WEEKLY_HEART_RATE = [
  { day: 'Mon', avg: 68, min: 58, max: 155 },
  { day: 'Tue', avg: 72, min: 60, max: 162 },
  { day: 'Wed', avg: 65, min: 56, max: 148 },
  { day: 'Thu', avg: 70, min: 59, max: 158 },
  { day: 'Fri', avg: 74, min: 61, max: 170 },
  { day: 'Sat', avg: 66, min: 55, max: 145 },
  { day: 'Sun', avg: 69, min: 57, max: 152 },
];

const AI_HEALTH_SCORES = [
  { label: 'Sleep Quality', score: 78, color: 'bg-indigo-500' },
  { label: 'Cardiovascular', score: 82, color: 'bg-red-500' },
  { label: 'Activity Level', score: 65, color: 'bg-green-500' },
  { label: 'Recovery', score: 71, color: 'bg-yellow-500' },
  { label: 'Consistency', score: 88, color: 'bg-blue-500' },
];

const AI_RECOMMENDATIONS: AiRecommendation[] = [
  {
    category: 'Supplements',
    icon: 'pill',
    text: 'Based on your activity level, consider Vitamin D3 (2000 IU) and Magnesium Glycinate (400mg) before bed for improved recovery and sleep quality.',
    color: 'text-purple-500',
  },
  {
    category: 'Nutrition',
    icon: 'nutrition',
    text: 'Your recovery metrics suggest increasing protein intake to 1.6g/kg body weight. Add more omega-3 rich foods like salmon, walnuts, and flaxseed.',
    color: 'text-green-500',
  },
  {
    category: 'Exercise',
    icon: 'exercise',
    text: 'Your heart rate recovery is improving. You\'re ready to increase cardio intensity by 10%. Consider adding interval training 2x per week.',
    color: 'text-orange-500',
  },
  {
    category: 'Sleep',
    icon: 'sleep',
    text: 'Your deep sleep percentage is below optimal (23% vs 25% target). Avoid screens 1 hour before bed and keep room temperature at 18-19 C.',
    color: 'text-indigo-500',
  },
  {
    category: 'Medical',
    icon: 'medical',
    text: 'Your resting heart rate trend is healthy (62 BPM avg). Schedule an annual checkup if not done in the last 12 months.',
    color: 'text-red-500',
  },
];

const DOCTOR_SUGGESTIONS: DoctorSuggestion[] = [
  {
    specialty: 'Nutritionist',
    reason: 'Optimize diet based on your activity level and recovery needs',
    icon: 'nutrition',
  },
  {
    specialty: 'Sports Medicine',
    reason: 'Fine-tune training intensity and prevent overtraining',
    icon: 'exercise',
  },
  {
    specialty: 'Sleep Specialist',
    reason: 'Address below-optimal deep sleep percentage',
    icon: 'sleep',
  },
];

const DEFAULT_HABITS: Habit[] = [
  { id: '1', name: 'Exercise 30 min', streak: 12, completedToday: true },
  { id: '2', name: 'Drink 8 glasses of water', streak: 5, completedToday: false },
  { id: '3', name: 'Read for 30 minutes', streak: 8, completedToday: true },
  { id: '4', name: 'Meditate 10 min', streak: 3, completedToday: false },
  { id: '5', name: 'No sugar after 6 PM', streak: 15, completedToday: true },
];
```

Replace the entire deleted block with just:

```tsx
// ---------------------------------------------------------------------------
// Mock data removed — all data is now derived from the metrics signal.
// DEFAULT_HABITS kept below for the initial empty-habits state only.
// ---------------------------------------------------------------------------

const DEFAULT_HABITS: Habit[] = [];
```

Note: `DEFAULT_HABITS` is still referenced in `const [habits, setHabits] = createSignal<Habit[]>(DEFAULT_HABITS)` (line ~1777). Keeping it as an empty array is correct: real habits are loaded from the backend via `fitness_list_habits`; the mock entries are never shown.

- [ ] **Step 2: Remove now-unused interfaces and TypeScript types**

Now that `SLEEP_STAGES`, `HEART_RATE_ZONES`, `AI_RECOMMENDATIONS`, and `DOCTOR_SUGGESTIONS` are gone, their companion interfaces `SleepStage`, `HeartRateZone`, `AiRecommendation`, and `DoctorSuggestion` are also unused. Remove them from the types section (original lines 53–79):

```tsx
interface SleepStage {
  label: string;
  duration: string;
  minutes: number;
  color: string;
}

interface HeartRateZone {
  label: string;
  bpmRange: string;
  minutes: number;
  maxMinutes: number;
  color: string;
}

interface AiRecommendation {
  category: string;
  icon: string;
  text: string;
  color: string;
}

interface DoctorSuggestion {
  specialty: string;
  reason: string;
  icon: string;
}
```

Replace with just a blank line (delete the four interfaces entirely).

- [ ] **Step 3: Replace AI_HEALTH_SCORES block in AiAnalysisTab**

In `AiAnalysisTab`, find the "Health Score Breakdown" card (original lines 1113–1156):

```tsx
      {/* Health Score Breakdown */}
      <div class="card p-6">
        <div class="flex items-center gap-2 mb-6">
          <Icon name="sparkle" class="w-5 h-5 text-minion-500" />
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide">
            Health Score Breakdown
          </h3>
        </div>
        <div class="space-y-4">
          <For each={AI_HEALTH_SCORES}>
            {(item) => (
              <div>
                <div class="flex items-center justify-between mb-1">
                  <span class="text-sm font-medium text-gray-700 dark:text-gray-200">
                    {item.label}
                  </span>
                  <span
                    class="text-sm font-bold"
                    classList={{
                      'text-green-500': item.score >= 70,
                      'text-yellow-500': item.score >= 40 && item.score < 70,
                      'text-red-500': item.score < 40,
                    }}
                  >
                    {item.score}/100
                  </span>
                </div>
                <div class="w-full h-3 rounded-full bg-gray-200 dark:bg-gray-700 overflow-hidden">
                  <div
                    class={`h-full rounded-full ${item.color} transition-all duration-700`}
                    style={{ width: `${item.score}%` }}
                  />
                </div>
              </div>
            )}
          </For>
        </div>
        <div class="mt-6 pt-4 border-t border-gray-200 dark:border-gray-700 flex items-center justify-between">
          <span class="text-sm text-gray-500 dark:text-gray-400">Overall Score</span>
          <span class="text-2xl font-bold text-gray-900 dark:text-white">
            {Math.round(AI_HEALTH_SCORES.reduce((a, s) => a + s.score, 0) / AI_HEALTH_SCORES.length)}/100
          </span>
        </div>
      </div>
```

Replace with:

```tsx
      {/* AI Health Score Breakdown — placeholder until AI endpoint is configured */}
      <div class="card p-6">
        <div class="flex items-center gap-2 mb-4">
          <Icon name="sparkle" class="w-5 h-5 text-minion-500" />
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide">
            Health Score Breakdown
          </h3>
        </div>
        <div class="flex flex-col items-center justify-center py-8 gap-3 text-center">
          <Icon name="sparkle" class="w-10 h-10 text-gray-300 dark:text-gray-600" />
          <p class="text-sm text-gray-500 dark:text-gray-400 max-w-sm leading-relaxed">
            Connect an AI endpoint in{' '}
            <strong class="text-gray-700 dark:text-gray-300">Settings → AI Endpoints</strong>{' '}
            to generate personalised health scores from your real data.
          </p>
        </div>
      </div>
```

- [ ] **Step 4: Replace AI_RECOMMENDATIONS block in AiAnalysisTab**

Find the "AI Recommendations" block (original lines 1158–1192):

```tsx
      {/* AI Recommendations */}
      <div>
        <div class="flex items-center gap-2 mb-4">
          <Icon name="sparkle" class="w-5 h-5 text-minion-500" />
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide">
            AI Recommendations
          </h3>
        </div>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
          <For each={AI_RECOMMENDATIONS}>
            {(rec) => (
              <div class="card p-5">
                <div class="flex items-center gap-3 mb-3">
                  <div
                    class={`p-2 rounded-lg ${rec.color}`}
                    classList={{
                      'bg-purple-100 dark:bg-purple-900/40': rec.category === 'Supplements',
                      'bg-green-100 dark:bg-green-900/40': rec.category === 'Nutrition',
                      'bg-orange-100 dark:bg-orange-900/40': rec.category === 'Exercise',
                      'bg-indigo-100 dark:bg-indigo-900/40': rec.category === 'Sleep',
                      'bg-red-100 dark:bg-red-900/40': rec.category === 'Medical',
                    }}
                  >
                    <Icon name={rec.icon} class="w-5 h-5" />
                  </div>
                  <h4 class="font-medium text-gray-900 dark:text-white">{rec.category}</h4>
                </div>
                <p class="text-sm text-gray-600 dark:text-gray-300 leading-relaxed">
                  {rec.text}
                </p>
              </div>
            )}
          </For>
        </div>
      </div>
```

Replace with:

```tsx
      {/* AI Recommendations — placeholder */}
      <div class="card p-6">
        <div class="flex items-center gap-2 mb-4">
          <Icon name="sparkle" class="w-5 h-5 text-minion-500" />
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide">
            AI Recommendations
          </h3>
        </div>
        <div class="flex flex-col items-center justify-center py-8 gap-3 text-center">
          <Icon name="sparkle" class="w-10 h-10 text-gray-300 dark:text-gray-600" />
          <p class="text-sm text-gray-500 dark:text-gray-400 max-w-sm leading-relaxed">
            Connect an AI endpoint in{' '}
            <strong class="text-gray-700 dark:text-gray-300">Settings → AI Endpoints</strong>{' '}
            to generate personalised supplement, nutrition, and exercise recommendations.
          </p>
        </div>
      </div>
```

- [ ] **Step 5: Replace DOCTOR_SUGGESTIONS block in AiAnalysisTab**

Find the "Suggested Consultations" card (original lines 1194–1219):

```tsx
      {/* Suggested Doctors */}
      <div class="card p-6">
        <div class="flex items-center gap-2 mb-4">
          <Icon name="medical" class="w-5 h-5 text-minion-500" />
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide">
            Suggested Consultations
          </h3>
        </div>
        <p class="text-sm text-gray-500 dark:text-gray-400 mb-4">
          Based on your health profile, consider consulting:
        </p>
        <div class="grid grid-cols-1 sm:grid-cols-3 gap-4">
          <For each={DOCTOR_SUGGESTIONS}>
            {(doc) => (
              <div class="p-4 rounded-lg border border-gray-200 dark:border-gray-700 hover:border-minion-300 dark:hover:border-minion-600 transition-colors">
                <div class="flex items-center gap-2 mb-2">
                  <Icon name={doc.icon} class="w-5 h-5 text-minion-500" />
                  <h4 class="font-medium text-gray-900 dark:text-white text-sm">{doc.specialty}</h4>
                </div>
                <p class="text-xs text-gray-500 dark:text-gray-400 leading-relaxed">{doc.reason}</p>
              </div>
            )}
          </For>
        </div>
      </div>
```

Replace with:

```tsx
      {/* Suggested Consultations — placeholder */}
      <div class="card p-6">
        <div class="flex items-center gap-2 mb-4">
          <Icon name="medical" class="w-5 h-5 text-minion-500" />
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide">
            Suggested Consultations
          </h3>
        </div>
        <div class="flex flex-col items-center justify-center py-8 gap-3 text-center">
          <Icon name="medical" class="w-10 h-10 text-gray-300 dark:text-gray-600" />
          <p class="text-sm text-gray-500 dark:text-gray-400 max-w-sm leading-relaxed">
            Connect an AI endpoint in{' '}
            <strong class="text-gray-700 dark:text-gray-300">Settings → AI Endpoints</strong>{' '}
            to receive personalised specialist consultation recommendations based on your health trends.
          </p>
        </div>
      </div>
```

- [ ] **Step 6: Run typecheck**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | grep -E "error TS" | head -20
```

Expected: no errors. If there are "cannot find name" errors for `WEEKLY_STEPS`, `WEEKLY_SLEEP`, etc., those constants were not fully removed — re-check the deletion in Step 1.

- [ ] **Step 7: Run ESLint**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm lint 2>&1 | grep -E "error" | head -20
```

Expected: no errors.

- [ ] **Step 8: Commit**

```bash
cd /home/dk/Documents/git/minion
git add ui/src/pages/Fitness.tsx
git commit -m "feat(fitness): remove 9 mock constants, replace AI score/rec/doctor blocks with AI endpoint prompt"
```

---

## Task 7: Final smoke test

**Files:**
- Read-only

- [ ] **Step 1: Full typecheck**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | tail -5
```

Expected: `Found 0 errors.`

- [ ] **Step 2: ESLint**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm lint 2>&1 | grep -E "^.*error" | head -10
```

Expected: no errors.

- [ ] **Step 3: Rust workspace build (no frontend changes, but verify nothing broke)**

```bash
cd /home/dk/Documents/git/minion && cargo build -p minion-tauri-lib 2>&1 | grep -E "^error" | head -10
```

Expected: no errors (Rust is unchanged).

- [ ] **Step 4: Verify all mock constants are gone**

```bash
grep -n "WEEKLY_STEPS\|WEEKLY_SLEEP\|SLEEP_STAGES\|HEART_RATE_ZONES\|WEEKLY_HEART_RATE\|AI_HEALTH_SCORES\|AI_RECOMMENDATIONS\|DOCTOR_SUGGESTIONS" /home/dk/Documents/git/minion/ui/src/pages/Fitness.tsx
```

Expected: no output (all constants removed).

- [ ] **Step 5: Verify props are wired on all three tabs**

```bash
grep -n "SleepTab\|HeartTab\|ActivityTab" /home/dk/Documents/git/minion/ui/src/pages/Fitness.tsx
```

Expected output should show three call sites each with `metrics={metrics}` and `gfitConnected={gfitConnected}` props.

- [ ] **Step 6: Verify EmptyState and SyncStatusBar are defined**

```bash
grep -n "const EmptyState\|const SyncStatusBar" /home/dk/Documents/git/minion/ui/src/pages/Fitness.tsx
```

Expected: two lines, one for each component.

- [ ] **Step 7: Final cleanup commit (if any)**

Only if there are minor formatting or lint fixes to address:

```bash
cd /home/dk/Documents/git/minion
git add ui/src/pages/Fitness.tsx
git commit -m "fix(fitness): Phase A smoke test cleanup"
```

---

## Self-Review Checklist

| Spec requirement | Task |
|---|---|
| Add sync status bar when gfitConnected is true | Task 2 (`SyncStatusBar` component with `↻ Sync` button and last-synced relative time) |
| Wire SleepTab to real data | Task 3 (7-day sleep bars, per-night quality, EmptyState, SyncStatusBar at top) |
| Wire HeartTab to real data | Task 4 (7-day SVG line chart avg/min/max, daily breakdown table, EmptyState) |
| Wire ActivityTab to real data | Task 5 (steps/distance/active-min/calories, weekly bar chart, empty state) |
| Show empty state when no data | Tasks 3–5 (`EmptyState` component, shown when filter returns 0 rows) |
| Remove 9 hardcoded constant blocks | Task 6 (WEEKLY_STEPS, WEEKLY_SLEEP, SLEEP_STAGES, HEART_RATE_ZONES, WEEKLY_HEART_RATE, AI_HEALTH_SCORES, AI_RECOMMENDATIONS, DOCTOR_SUGGESTIONS, DEFAULT_HABITS) |
| Replace AI blocks with placeholder | Task 6 (all three AI sections → "Connect AI endpoint in Settings" card) |
| Expand FitnessMetricResponse interface | Task 1 (add heart_rate_min/max, distance_m, active_minutes, spo2_avg, calories_out, source, synced_at) |
| Add GfitSyncStatus interface | Task 1 |
| Only file changed is ui/src/pages/Fitness.tsx | All tasks touch only this file |
| pnpm typecheck passes after every task | Verified in steps 4, 3, 3, 3, 6, 1 of Tasks 1–7 |
