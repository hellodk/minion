import { Component, createSignal, For, Show, Switch, Match, onMount, onCleanup } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type TabId = 'dashboard' | 'sleep' | 'heart' | 'activity' | 'ai' | 'habits' | 'workouts' | 'nutrition';

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

export interface GfitSyncStatus {
  last_synced: string | null;  // ISO datetime string or null
  days_count: number;
  running: boolean;
  pct: number;
  message: string;
}

interface FitnessHabitResponse {
  id: string;
  name: string;
  description: string | null;
  frequency: string;
  created_at: string;
  completed_today: boolean;
}

interface Habit {
  id: string;
  name: string;
  streak: number;
  completedToday: boolean;
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

interface WorkoutResponse {
  id: string;
  name: string;
  exercises: string | null;
  duration_minutes: number;
  calories_burned: number | null;
  date: string;
  notes: string | null;
}

interface NutritionResponse {
  id: string;
  name: string;
  calories: number;
  protein_g: number;
  carbs_g: number;
  fat_g: number;
  meal_type: string;
  date: string;
}

interface NutritionDaySummary {
  total_calories: number;
  total_protein: number;
  total_carbs: number;
  total_fat: number;
  meals: NutritionResponse[];
}

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

// ---------------------------------------------------------------------------
// Reusable sub-components
// ---------------------------------------------------------------------------

/** SVG circular progress indicator */
const CircularProgress: Component<{
  value: number;
  max: number;
  size?: string;
  strokeWidth?: number;
  colorClass?: string;
  label?: string;
  sublabel?: string;
}> = (props) => {
  const pct = () => Math.min((props.value / props.max) * 100, 100);
  const scoreColor = () => {
    if (props.colorClass) return props.colorClass;
    if (pct() >= 70) return 'text-green-500';
    if (pct() >= 40) return 'text-yellow-500';
    return 'text-red-500';
  };
  const sizeClass = () => props.size ?? 'w-32 h-32';

  return (
    <div class="flex flex-col items-center gap-1">
      <div class="relative">
        <svg class={sizeClass()} viewBox="0 0 36 36">
          <path
            class="text-gray-200 dark:text-gray-700"
            stroke="currentColor"
            stroke-width={props.strokeWidth ?? 3}
            fill="none"
            d="M18 2.0845 a 15.9155 15.9155 0 0 1 0 31.831 a 15.9155 15.9155 0 0 1 0 -31.831"
          />
          <path
            class={scoreColor()}
            stroke="currentColor"
            stroke-width={props.strokeWidth ?? 3}
            fill="none"
            stroke-linecap="round"
            stroke-dasharray={`${pct()}, 100`}
            d="M18 2.0845 a 15.9155 15.9155 0 0 1 0 31.831 a 15.9155 15.9155 0 0 1 0 -31.831"
          />
        </svg>
        <div class="absolute inset-0 flex flex-col items-center justify-center">
          <span class="text-2xl font-bold text-gray-900 dark:text-white">
            {props.value}
          </span>
          <Show when={props.sublabel}>
            <span class="text-xs text-gray-500 dark:text-gray-400">
              {props.sublabel}
            </span>
          </Show>
        </div>
      </div>
      <Show when={props.label}>
        <span class="text-sm font-medium text-gray-600 dark:text-gray-300">
          {props.label}
        </span>
      </Show>
    </div>
  );
};

export const EmptyState: Component<{ icon: string; message: string }> = (props) => (
  <div class="flex flex-col items-center justify-center py-16 gap-4 text-center">
    <div class="p-4 rounded-full bg-gray-100 dark:bg-gray-800 text-gray-400">
      <Icon name={props.icon} class="w-10 h-10" />
    </div>
    <p class="text-sm text-gray-500 dark:text-gray-400 max-w-xs leading-relaxed">
      {props.message}
    </p>
  </div>
);

export const SyncStatusBar: Component<{ onSync: () => Promise<void> }> = (props) => {
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

/** Mini inline progress bar */
const MiniBar: Component<{ value: number; max: number; colorClass?: string }> = (props) => {
  const pct = () => Math.min((props.value / props.max) * 100, 100);
  return (
    <div class="w-full h-2 rounded-full bg-gray-200 dark:bg-gray-700 overflow-hidden">
      <div
        class={`h-full rounded-full transition-all duration-500 ${props.colorClass ?? 'bg-minion-500'}`}
        style={{ width: `${pct()}%` }}
      />
    </div>
  );
};

/** Icon helper -- simple inline SVG icons keyed by name */
const Icon: Component<{ name: string; class?: string }> = (props) => {
  const cls = () => props.class ?? 'w-5 h-5';

  return (
    <Switch>
      <Match when={props.name === 'steps'}>
        <svg class={cls()} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M13 7h8m0 0v8m0-8l-8 8-4-4-6 6" />
        </svg>
      </Match>
      <Match when={props.name === 'heart'}>
        <svg class={cls()} fill="currentColor" viewBox="0 0 24 24">
          <path d="M4.318 6.318a4.5 4.5 0 000 6.364L12 20.364l7.682-7.682a4.5 4.5 0 00-6.364-6.364L12 7.636l-1.318-1.318a4.5 4.5 0 00-6.364 0z" />
        </svg>
      </Match>
      <Match when={props.name === 'moon'}>
        <svg class={cls()} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" />
        </svg>
      </Match>
      <Match when={props.name === 'fire'}>
        <svg class={cls()} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M17.657 18.657A8 8 0 016.343 7.343S7 9 9 10c0-2 .5-5 2.986-7C14 5 16.09 5.777 17.656 7.343A7.975 7.975 0 0120 13a7.975 7.975 0 01-2.343 5.657z" />
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M9.879 16.121A3 3 0 1012.015 11L11 14H9c0 .768.293 1.536.879 2.121z" />
        </svg>
      </Match>
      <Match when={props.name === 'sparkle'}>
        <svg class={cls()} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M9.813 15.904L9 18.75l-.813-2.846a4.5 4.5 0 00-3.09-3.09L2.25 12l2.846-.813a4.5 4.5 0 003.09-3.09L9 5.25l.813 2.846a4.5 4.5 0 003.09 3.09L15.75 12l-2.846.813a4.5 4.5 0 00-3.09 3.09zM18.259 8.715L18 9.75l-.259-1.035a3.375 3.375 0 00-2.455-2.456L14.25 6l1.036-.259a3.375 3.375 0 002.455-2.456L18 2.25l.259 1.035a3.375 3.375 0 002.455 2.456L21.75 6l-1.036.259a3.375 3.375 0 00-2.455 2.456z" />
        </svg>
      </Match>
      <Match when={props.name === 'pill'}>
        <svg class={cls()} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M4.5 12.75l6-6a4.243 4.243 0 016 6l-6 6a4.243 4.243 0 01-6-6z" />
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7.5 9.75l6 6" />
        </svg>
      </Match>
      <Match when={props.name === 'nutrition'}>
        <svg class={cls()} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
      </Match>
      <Match when={props.name === 'exercise'}>
        <svg class={cls()} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M3.75 13.5l10.5-11.25L12 10.5h8.25L9.75 21.75 12 13.5H3.75z" />
        </svg>
      </Match>
      <Match when={props.name === 'sleep'}>
        <svg class={cls()} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" />
        </svg>
      </Match>
      <Match when={props.name === 'medical'}>
        <svg class={cls()} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M9 12h6m-3-3v6m-7.5 3V6.75A2.25 2.25 0 016.75 4.5h10.5a2.25 2.25 0 012.25 2.25v10.5a2.25 2.25 0 01-2.25 2.25H6.75a2.25 2.25 0 01-2.25-2.25z" />
        </svg>
      </Match>
      <Match when={props.name === 'distance'}>
        <svg class={cls()} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M15 10.5a3 3 0 11-6 0 3 3 0 016 0z" />
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M19.5 10.5c0 7.142-7.5 11.25-7.5 11.25S4.5 17.642 4.5 10.5a7.5 7.5 0 1115 0z" />
        </svg>
      </Match>
      <Match when={props.name === 'clock'}>
        <svg class={cls()} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M12 6v6h4.5m4.5 0a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
      </Match>
      <Match when={props.name === 'stairs'}>
        <svg class={cls()} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M3 21h4V15h4V9h4V3h6" />
        </svg>
      </Match>
      <Match when={props.name === 'google-fit'}>
        <svg class={cls()} viewBox="0 0 24 24" fill="none">
          <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2z"
            fill="#4285F4" opacity="0.2" />
          <path d="M7.5 12l3 3 6-6" stroke="#4285F4" stroke-width="2.5"
            stroke-linecap="round" stroke-linejoin="round" />
        </svg>
      </Match>
      <Match when={props.name === 'plus'}>
        <svg class={cls()} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
        </svg>
      </Match>
      <Match when={props.name === 'check'}>
        <svg class={cls()} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
        </svg>
      </Match>
      <Match when={props.name === 'arrow-up'}>
        <svg class={cls()} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M5 10l7-7m0 0l7 7m-7-7v18" />
        </svg>
      </Match>
      <Match when={props.name === 'arrow-down'}>
        <svg class={cls()} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M19 14l-7 7m0 0l-7-7m7 7V3" />
        </svg>
      </Match>
      <Match when={props.name === 'trash'}>
        <svg class={cls()} fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
        </svg>
      </Match>
    </Switch>
  );
};

// ---------------------------------------------------------------------------
// Tab content components
// ---------------------------------------------------------------------------

const DashboardTab: Component<{
  dashboard: () => FitnessDashboard | null;
  metrics: () => FitnessMetricResponse[];
  hasRealData: () => boolean;
}> = (props) => {
  // Derive values from real data or fall back to mock
  const stepsToday = () => {
    if (!props.hasRealData()) return 8432;
    const todayMetric = props.metrics().find(
      (m) => m.date === new Date().toISOString().slice(0, 10)
    );
    return todayMetric?.steps ?? props.dashboard()?.avg_steps_7d ?? 0;
  };

  const heartRate = () => {
    if (!props.hasRealData()) return 72;
    const todayMetric = props.metrics().find(
      (m) => m.date === new Date().toISOString().slice(0, 10)
    );
    return todayMetric?.heart_rate_avg ?? 0;
  };

  const sleepHours = () => {
    if (!props.hasRealData()) return 7.38;
    return props.dashboard()?.avg_sleep_7d ?? 0;
  };

  const waterToday = () => {
    if (!props.hasRealData()) return 1847;
    return props.dashboard()?.total_water_today ?? 0;
  };

  const latestWeight = () => {
    if (!props.hasRealData()) return null;
    return props.dashboard()?.latest_weight_kg ?? null;
  };

  const currentStreak = () => props.dashboard()?.current_streak ?? 0;

  // Build weekly steps from recent metrics
  const weeklyStepsData = () => {
    if (!props.hasRealData()) return WEEKLY_STEPS;
    const days = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
    const last7 = props.metrics().slice(0, 7).reverse();
    if (last7.length === 0) return WEEKLY_STEPS;
    return last7.map((m) => {
      const d = new Date(m.date);
      return { day: days[d.getDay()], steps: m.steps ?? 0 };
    });
  };

  const maxSteps = () => Math.max(...weeklyStepsData().map((d) => d.steps), 1);

  const sleepLabel = () => {
    const h = sleepHours();
    const hrs = Math.floor(h);
    const mins = Math.round((h - hrs) * 60);
    return `${hrs}h ${mins.toString().padStart(2, '0')}m`;
  };

  return (
    <div class="space-y-6">
      {/* Top row: Health Score + Quick Stats */}
      <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Health Score */}
        <div class="card p-6 flex flex-col items-center justify-center">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
            Health Score
          </h3>
          <CircularProgress value={props.hasRealData() ? Math.min(Math.round((stepsToday() / 10000) * 50 + (sleepHours() / 8) * 50), 100) : 72} max={100} size="w-40 h-40" sublabel="/ 100" />
          <p class="mt-3 text-sm text-gray-500 dark:text-gray-400">
            {props.hasRealData() && currentStreak() > 0 ? `${currentStreak()} day streak` : 'Good - Keep it up!'}
          </p>
        </div>

        {/* Quick Stats Grid */}
        <div class="lg:col-span-2 grid grid-cols-1 sm:grid-cols-2 gap-4">
          {/* Steps Today */}
          <div class="card p-5">
            <div class="flex items-center gap-3 mb-3">
              <div class="p-2 rounded-lg bg-blue-100 dark:bg-blue-900/40 text-blue-600 dark:text-blue-400">
                <Icon name="steps" class="w-5 h-5" />
              </div>
              <span class="text-sm font-medium text-gray-500 dark:text-gray-400">Steps (avg 7d)</span>
            </div>
            <p class="text-2xl font-bold text-gray-900 dark:text-white mb-1">
              {Math.round(stepsToday()).toLocaleString()} <span class="text-sm font-normal text-gray-400">/ 10,000</span>
            </p>
            <MiniBar value={stepsToday()} max={10000} colorClass="bg-blue-500" />
          </div>

          {/* Heart Rate */}
          <div class="card p-5">
            <div class="flex items-center gap-3 mb-3">
              <div class="p-2 rounded-lg bg-red-100 dark:bg-red-900/40 text-red-500">
                <Icon name="heart" class="w-5 h-5" />
              </div>
              <span class="text-sm font-medium text-gray-500 dark:text-gray-400">Heart Rate</span>
            </div>
            <div class="flex items-end gap-2">
              <p class="text-2xl font-bold text-gray-900 dark:text-white">{heartRate()}</p>
              <span class="text-sm text-gray-400 mb-1">BPM</span>
            </div>
            <Show when={latestWeight() !== null}>
              <p class="text-xs text-gray-400 mt-1">Weight: {latestWeight()?.toFixed(1)} kg</p>
            </Show>
          </div>

          {/* Sleep */}
          <div class="card p-5">
            <div class="flex items-center gap-3 mb-3">
              <div class="p-2 rounded-lg bg-indigo-100 dark:bg-indigo-900/40 text-indigo-500">
                <Icon name="moon" class="w-5 h-5" />
              </div>
              <span class="text-sm font-medium text-gray-500 dark:text-gray-400">Sleep (avg 7d)</span>
            </div>
            <p class="text-2xl font-bold text-gray-900 dark:text-white mb-1">{sleepLabel()}</p>
            <div class="flex items-center gap-2">
              <span class="text-xs px-2 py-0.5 rounded-full bg-green-100 dark:bg-green-900/40 text-green-600 dark:text-green-400">
                {sleepHours() >= 7 ? 'Good quality' : sleepHours() >= 6 ? 'Fair' : 'Needs improvement'}
              </span>
            </div>
          </div>

          {/* Water / Calories */}
          <div class="card p-5">
            <div class="flex items-center gap-3 mb-3">
              <div class="p-2 rounded-lg bg-orange-100 dark:bg-orange-900/40 text-orange-500">
                <Icon name="fire" class="w-5 h-5" />
              </div>
              <span class="text-sm font-medium text-gray-500 dark:text-gray-400">Water Today</span>
            </div>
            <p class="text-2xl font-bold text-gray-900 dark:text-white mb-1">
              {waterToday().toLocaleString()} <span class="text-sm font-normal text-gray-400">ml</span>
            </p>
            <MiniBar value={waterToday()} max={2500} colorClass="bg-orange-500" />
          </div>
        </div>
      </div>

      {/* Weekly Activity Chart */}
      <div class="card p-6">
        <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
          Weekly Activity
        </h3>
        <div class="flex items-end gap-3 h-40">
          <For each={weeklyStepsData()}>
            {(entry) => (
              <div class="flex-1 flex flex-col items-center gap-1">
                <span class="text-xs text-gray-400">{(entry.steps / 1000).toFixed(1)}k</span>
                <div class="w-full flex justify-center">
                  <div
                    class="w-full max-w-[40px] rounded-t-md bg-minion-500 dark:bg-minion-400 transition-all duration-500"
                    style={{ height: `${(entry.steps / maxSteps()) * 120}px` }}
                  />
                </div>
                <span class="text-xs font-medium text-gray-500 dark:text-gray-400">{entry.day}</span>
              </div>
            )}
          </For>
        </div>
      </div>

      {/* AI Insight Card */}
      <div class="card p-6 border-l-4 border-l-minion-500">
        <div class="flex items-start gap-3">
          <div class="p-2 rounded-lg bg-minion-100 dark:bg-minion-900/40 text-minion-600 dark:text-minion-400 shrink-0">
            <Icon name="sparkle" class="w-5 h-5" />
          </div>
          <div>
            <h3 class="font-medium text-gray-900 dark:text-white mb-1">AI Insight</h3>
            <p class="text-sm text-gray-600 dark:text-gray-300 leading-relaxed">
              <Show when={props.hasRealData()} fallback={
                <>Your sleep quality has improved 12% this week. Consider maintaining your 10:30 PM
                bedtime routine. Your step count is trending 8% above last week's average.</>
              }>
                {sleepHours() >= 7
                  ? 'Your sleep average is in a healthy range. '
                  : 'Your sleep average is below the recommended 7 hours. Consider adjusting your routine. '}
                {stepsToday() >= 8000
                  ? 'Great step count -- you are meeting activity targets.'
                  : 'Try to increase your daily steps toward the 10,000 goal.'}
                {currentStreak() > 3
                  ? ` You have a ${currentStreak()}-day habit streak going -- keep it up!`
                  : ''}
              </Show>
            </p>
          </div>
        </div>
      </div>
    </div>
  );
};

const SleepTab: Component<{
  metrics: () => FitnessMetricResponse[];
  gfitConnected: () => boolean;
  onSync: () => Promise<void>;
}> = (props) => {
  const sleepRows = () =>
    props.metrics().filter((m) => m.sleep_hours !== null).slice(0, 7).reverse();
  const hasSleepData = () => sleepRows().length > 0;
  const latest = () => sleepRows()[sleepRows().length - 1] ?? null;
  const maxSleep = () => Math.max(...sleepRows().map((m) => m.sleep_hours ?? 0), 1);
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
  const bestSleep = () => Math.max(...sleepRows().map((m) => m.sleep_hours ?? 0), 0);
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
        <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
          <div class="card p-6 flex flex-col items-center justify-center">
            <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">Sleep Score</h3>
            <CircularProgress value={sleepScore()} max={100} size="w-40 h-40" colorClass="text-indigo-500" sublabel="/ 100" />
            <p class="mt-3 text-sm text-gray-500 dark:text-gray-400">{sleepScoreLabel()}</p>
          </div>
          <div class="lg:col-span-2 card p-6">
            <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">Last Night Summary</h3>
            <div class="grid grid-cols-2 sm:grid-cols-4 gap-4 mb-6">
              <div>
                <p class="text-xs text-gray-400">Date</p>
                <p class="text-lg font-semibold text-gray-900 dark:text-white">{latest()?.date ?? '—'}</p>
              </div>
              <div>
                <p class="text-xs text-gray-400">Duration</p>
                <p class="text-lg font-semibold text-gray-900 dark:text-white">{fmtHours(latest()?.sleep_hours ?? null)}</p>
              </div>
              <div>
                <p class="text-xs text-gray-400">Quality Score</p>
                <p class="text-lg font-semibold text-gray-900 dark:text-white">
                  {latest()?.sleep_quality !== null && latest()?.sleep_quality !== undefined ? `${latest()!.sleep_quality}/100` : '—'}
                </p>
              </div>
              <div>
                <p class="text-xs text-gray-400">vs Target (7h)</p>
                <p class="text-lg font-semibold"
                  classList={{
                    'text-green-500': (latest()?.sleep_hours ?? 0) >= 7,
                    'text-amber-500': (latest()?.sleep_hours ?? 0) >= 6 && (latest()?.sleep_hours ?? 0) < 7,
                    'text-red-500': (latest()?.sleep_hours ?? 0) < 6,
                  }}
                >
                  {(latest()?.sleep_hours ?? 0) >= 7 ? 'Met' : `−${fmtHours(7 - (latest()?.sleep_hours ?? 0))}`}
                </p>
              </div>
            </div>
            <h4 class="text-xs text-gray-400 mb-3 uppercase tracking-wide">Sleep Quality by Night</h4>
            <div class="space-y-3">
              <For each={sleepRows()}>
                {(row) => {
                  const score = row.sleep_quality !== null
                    ? Math.round(row.sleep_quality)
                    : Math.min(Math.round(((row.sleep_hours ?? 0) / 9) * 100), 100);
                  return (
                    <div class="flex items-center gap-3">
                      <span class="text-sm text-gray-600 dark:text-gray-300 w-10 shrink-0">{dayLabel(row.date)}</span>
                      <div class="flex-1 h-4 rounded-full bg-gray-200 dark:bg-gray-700 overflow-hidden">
                        <div class="h-full rounded-full bg-indigo-500 transition-all duration-500" style={{ width: `${score}%` }} />
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
        <div class="card p-6">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">7-Day Sleep Trend</h3>
          <div class="flex items-end gap-3 h-40">
            <For each={sleepRows()}>
              {(entry) => (
                <div class="flex-1 flex flex-col items-center gap-1">
                  <span class="text-xs text-gray-400">{(entry.sleep_hours ?? 0).toFixed(1)}h</span>
                  <div class="w-full flex justify-center">
                    <div class="w-full max-w-[40px] rounded-t-md bg-indigo-500 dark:bg-indigo-400 transition-all duration-500"
                      style={{ height: `${((entry.sleep_hours ?? 0) / maxSleep()) * 120}px` }} />
                  </div>
                  <span class="text-xs font-medium text-gray-500 dark:text-gray-400">{dayLabel(entry.date)}</span>
                </div>
              )}
            </For>
          </div>
        </div>
        <div class="card p-6">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">7-Day Sleep Summary</h3>
          <div class="grid grid-cols-3 gap-4 text-center">
            <div>
              <p class="text-2xl font-bold text-gray-900 dark:text-white">{fmtHours(avgSleep())}</p>
              <p class="text-xs text-gray-400 mt-1">Average</p>
              <MiniBar value={avgSleep()} max={9} colorClass={avgSleep() >= 7 ? 'bg-green-500' : avgSleep() >= 6 ? 'bg-amber-500' : 'bg-red-500'} />
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

const HeartTab: Component<{
  metrics: () => FitnessMetricResponse[];
  gfitConnected: () => boolean;
  onSync: () => Promise<void>;
}> = (props) => {
  const hrRows = () =>
    props.metrics().filter((m) => m.heart_rate_avg !== null).slice(0, 7).reverse();
  const hasHrData = () => hrRows().length > 0;
  const latest = () => hrRows()[hrRows().length - 1] ?? null;
  const avgRestingHr = () => {
    const rows = hrRows();
    if (rows.length === 0) return 0;
    return Math.round(rows.reduce((s, m) => s + (m.heart_rate_avg ?? 0), 0) / rows.length);
  };
  const dayLabel = (dateStr: string) => {
    const days = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
    return days[new Date(dateStr).getDay()];
  };
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
    vals().map((v, i) => `${(i / Math.max(vals().length - 1, 1)) * chartWidth},${scaleY(v)}`).join(' ');
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
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-6">
          <div class="card p-6 flex flex-col items-center justify-center">
            <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">Latest Heart Rate</h3>
            <div class="flex items-center gap-4">
              <div class="text-red-500">
                <Icon name="heart" class="w-12 h-12" />
              </div>
              <div>
                <span class="text-5xl font-bold text-gray-900 dark:text-white">{latest()?.heart_rate_avg ?? '—'}</span>
                <span class="text-lg text-gray-400 ml-1">BPM</span>
              </div>
            </div>
            <p class="text-sm text-gray-400 mt-3">{hrRangeLabel()}</p>
            <Show when={latest()?.heart_rate_min !== null && latest()?.heart_rate_max !== null}>
              <p class="text-xs text-gray-400 mt-1">Range: {latest()?.heart_rate_min}–{latest()?.heart_rate_max} BPM</p>
            </Show>
          </div>
          <div class="card p-6 flex flex-col items-center justify-center">
            <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">7-Day Avg Heart Rate</h3>
            <CircularProgress value={avgRestingHr()} max={200} size="w-36 h-36" colorClass="text-red-500" sublabel="BPM" />
            <p class="mt-3 text-sm text-gray-500 dark:text-gray-400">{hrRows().length} day{hrRows().length !== 1 ? 's' : ''} of data</p>
          </div>
        </div>
        <div class="card p-6">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">7-Day Heart Rate Trend</h3>
          <Show
            when={hrRows().length > 1}
            fallback={<p class="text-sm text-gray-400 text-center py-8">Need at least 2 days of data to draw a trend line.</p>}
          >
            <div class="overflow-x-auto">
              <svg viewBox={`-30 -10 ${chartWidth + 60} ${chartHeight + 40}`} class="w-full h-44">
                <Show when={hrRows().some((r) => r.heart_rate_max !== null)}>
                  <polyline points={maxPoints()} fill="none" stroke="#fca5a5" stroke-width="1.5" stroke-dasharray="4" />
                </Show>
                <polyline points={avgPoints()} fill="none" stroke="#ef4444" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" />
                <Show when={hrRows().some((r) => r.heart_rate_min !== null)}>
                  <polyline points={minPoints()} fill="none" stroke="#93c5fd" stroke-width="1.5" stroke-dasharray="4" />
                </Show>
                <For each={hrRows()}>
                  {(d, i) => (
                    <circle cx={(i() / Math.max(hrRows().length - 1, 1)) * chartWidth} cy={scaleY(d.heart_rate_avg ?? 0)} r="4" fill="#ef4444" />
                  )}
                </For>
                <For each={hrRows()}>
                  {(d, i) => (
                    <text x={(i() / Math.max(hrRows().length - 1, 1)) * chartWidth} y={chartHeight + 18} text-anchor="middle" class="fill-gray-400" font-size="10">
                      {dayLabel(d.date)}
                    </text>
                  )}
                </For>
              </svg>
            </div>
            <div class="flex items-center justify-center gap-6 mt-2 text-xs text-gray-400">
              <div class="flex items-center gap-1"><div class="w-6 h-0.5 bg-red-500 rounded" /><span>Avg</span></div>
              <div class="flex items-center gap-1"><div class="w-6 h-0.5 bg-red-300 rounded" /><span>Max</span></div>
              <div class="flex items-center gap-1"><div class="w-6 h-0.5 bg-blue-300 rounded" /><span>Min</span></div>
            </div>
          </Show>
        </div>
        <div class="card p-6">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">Daily Breakdown</h3>
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
                      <td class="py-2 text-right text-blue-500">{row.heart_rate_min ?? '—'}</td>
                      <td class="py-2 text-right font-semibold text-gray-900 dark:text-white">{row.heart_rate_avg ?? '—'}</td>
                      <td class="py-2 text-right text-red-400">{row.heart_rate_max ?? '—'}</td>
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

const ActivityTab: Component = () => {
  const maxSteps = Math.max(...WEEKLY_STEPS.map((d) => d.steps));

  return (
    <div class="space-y-6">
      {/* Top metrics */}
      <div class="grid grid-cols-2 lg:grid-cols-4 gap-4">
        {/* Steps */}
        <div class="card p-5 flex flex-col items-center">
          <CircularProgress value={8432} max={10000} size="w-24 h-24" colorClass="text-blue-500" sublabel="steps" />
          <p class="mt-2 text-sm font-medium text-gray-600 dark:text-gray-300">Steps</p>
          <p class="text-xs text-gray-400">Goal: 10,000</p>
        </div>

        {/* Distance */}
        <div class="card p-5 flex flex-col items-center">
          <div class="p-3 rounded-full bg-green-100 dark:bg-green-900/40 text-green-500 mb-2">
            <Icon name="distance" class="w-8 h-8" />
          </div>
          <p class="text-2xl font-bold text-gray-900 dark:text-white">6.2</p>
          <p class="text-sm text-gray-400">km</p>
          <MiniBar value={6.2} max={8} colorClass="bg-green-500" />
        </div>

        {/* Active Minutes */}
        <div class="card p-5 flex flex-col items-center">
          <div class="p-3 rounded-full bg-orange-100 dark:bg-orange-900/40 text-orange-500 mb-2">
            <Icon name="clock" class="w-8 h-8" />
          </div>
          <p class="text-2xl font-bold text-gray-900 dark:text-white">47</p>
          <p class="text-sm text-gray-400">active min</p>
          <MiniBar value={47} max={60} colorClass="bg-orange-500" />
        </div>

        {/* Flights Climbed */}
        <div class="card p-5 flex flex-col items-center">
          <div class="p-3 rounded-full bg-purple-100 dark:bg-purple-900/40 text-purple-500 mb-2">
            <Icon name="stairs" class="w-8 h-8" />
          </div>
          <p class="text-2xl font-bold text-gray-900 dark:text-white">14</p>
          <p class="text-sm text-gray-400">flights</p>
          <MiniBar value={14} max={20} colorClass="bg-purple-500" />
        </div>
      </div>

      {/* Weekly Activity Bars */}
      <div class="card p-6">
        <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
          Weekly Steps
        </h3>
        <div class="flex items-end gap-3 h-44">
          <For each={WEEKLY_STEPS}>
            {(entry) => {
              const isToday = entry.day === 'Sun';
              return (
                <div class="flex-1 flex flex-col items-center gap-1">
                  <span class="text-xs text-gray-400">{(entry.steps / 1000).toFixed(1)}k</span>
                  <div class="w-full flex justify-center">
                    <div
                      class={`w-full max-w-[40px] rounded-t-md transition-all duration-500 ${
                        isToday
                          ? 'bg-minion-500 dark:bg-minion-400'
                          : 'bg-gray-300 dark:bg-gray-600'
                      }`}
                      style={{ height: `${(entry.steps / maxSteps) * 140}px` }}
                    />
                  </div>
                  <span
                    class={`text-xs font-medium ${
                      isToday
                        ? 'text-minion-600 dark:text-minion-400'
                        : 'text-gray-500 dark:text-gray-400'
                    }`}
                  >
                    {entry.day}
                  </span>
                </div>
              );
            }}
          </For>
        </div>
        <div class="flex items-center justify-between mt-4 text-sm text-gray-500 dark:text-gray-400">
          <span>Weekly total: <strong class="text-gray-900 dark:text-white">64,332</strong> steps</span>
          <span>Daily avg: <strong class="text-gray-900 dark:text-white">9,190</strong> steps</span>
        </div>
      </div>

      {/* Calorie Breakdown */}
      <div class="card p-6">
        <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
          Calorie Breakdown
        </h3>
        <div class="grid grid-cols-3 gap-4 text-center">
          <div>
            <p class="text-3xl font-bold text-gray-900 dark:text-white">1,547</p>
            <p class="text-sm text-gray-400">Basal</p>
          </div>
          <div>
            <p class="text-3xl font-bold text-orange-500">300</p>
            <p class="text-sm text-gray-400">Active</p>
          </div>
          <div>
            <p class="text-3xl font-bold text-gray-900 dark:text-white">1,847</p>
            <p class="text-sm text-gray-400">Total</p>
          </div>
        </div>
      </div>
    </div>
  );
};

const AiAnalysisTab: Component<{
  dashboard: () => FitnessDashboard | null;
  metrics: () => FitnessMetricResponse[];
}> = (props) => {
  const [aiLoading, setAiLoading] = createSignal(false);
  const [aiResponse, setAiResponse] = createSignal('');
  const [aiError, setAiError] = createSignal('');

  const handleAnalyze = async () => {
    setAiLoading(true);
    setAiResponse('');
    setAiError('');
    try {
      const metricsData = {
        dashboard: props.dashboard(),
        recentMetrics: props.metrics().slice(0, 14),
      };
      const result = await invoke<string>('ai_analyze_health', {
        metricsJson: JSON.stringify(metricsData, null, 2),
      });
      setAiResponse(result);
    } catch (e: any) {
      setAiError(String(e));
    } finally {
      setAiLoading(false);
    }
  };

  return (
    <div class="space-y-6">
      {/* AI Analyze Button */}
      <div class="card p-6">
        <div class="flex items-center gap-2 mb-4">
          <Icon name="sparkle" class="w-5 h-5 text-minion-500" />
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 uppercase tracking-wide">
            AI Health Analysis (Ollama)
          </h3>
        </div>
        <p class="text-sm text-gray-500 dark:text-gray-400 mb-4">
          Send your health metrics to your local LLM for personalized analysis and recommendations.
          Configure the Ollama connection in Settings.
        </p>
        <button
          class="btn btn-primary flex items-center gap-2"
          disabled={aiLoading()}
          onClick={handleAnalyze}
        >
          <Show when={aiLoading()} fallback={<Icon name="sparkle" class="w-4 h-4" />}>
            <svg class="animate-spin w-4 h-4" fill="none" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" />
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
            </svg>
          </Show>
          {aiLoading() ? 'Analyzing...' : 'Analyze My Health with AI'}
        </button>

        <Show when={aiError()}>
          <div class="mt-4 p-4 rounded-lg bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-sm text-red-700 dark:text-red-300">
            {aiError()}
          </div>
        </Show>

        <Show when={aiResponse()}>
          <div class="mt-4 card p-5 border-l-4 border-l-minion-500 bg-minion-50 dark:bg-minion-900/10">
            <div class="flex items-center gap-2 mb-3">
              <Icon name="sparkle" class="w-4 h-4 text-minion-500" />
              <h4 class="font-medium text-gray-900 dark:text-white text-sm">AI Analysis Result</h4>
            </div>
            <div class="text-sm text-gray-700 dark:text-gray-300 leading-relaxed whitespace-pre-wrap">
              {aiResponse()}
            </div>
          </div>
        </Show>
      </div>

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
    </div>
  );
};

const HabitsTab: Component<{
  habits: () => Habit[];
  setHabits: (v: Habit[] | ((prev: Habit[]) => Habit[])) => void;
  onToggle: (id: string) => void;
  onAdd: () => void;
}> = (props) => {
  const completedCount = () => props.habits().filter((h) => h.completedToday).length;

  return (
    <div class="space-y-6">
      {/* Summary */}
      <div class="card p-6 flex items-center gap-6">
        <CircularProgress
          value={completedCount()}
          max={props.habits().length || 1}
          size="w-28 h-28"
          colorClass="text-green-500"
          sublabel={`/ ${props.habits().length}`}
        />
        <div>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Today's Progress</h3>
          <p class="text-sm text-gray-500 dark:text-gray-400">
            {completedCount()} of {props.habits().length} habits completed
          </p>
          <Show when={props.habits().length > 0 && completedCount() === props.habits().length}>
            <p class="text-sm text-green-500 font-medium mt-1">All habits done -- great job!</p>
          </Show>
        </div>
      </div>

      {/* Habit List */}
      <div class="card p-4">
        <div class="flex items-center justify-between mb-4">
          <h3 class="font-medium text-gray-900 dark:text-white">Today's Habits</h3>
          <button
            onClick={props.onAdd}
            class="flex items-center gap-1 px-3 py-1.5 text-sm rounded-lg bg-minion-600 text-white hover:bg-minion-700 transition-colors"
          >
            <Icon name="plus" class="w-4 h-4" />
            Add Habit
          </button>
        </div>
        <div class="space-y-2">
          <For each={props.habits()}>
            {(habit) => (
              <div
                class="flex items-center justify-between p-3 rounded-lg bg-gray-50 dark:bg-gray-800/50 cursor-pointer hover:bg-gray-100 dark:hover:bg-gray-700/50 transition-colors"
                onClick={() => props.onToggle(habit.id)}
              >
                <div class="flex items-center gap-3">
                  <div
                    class="w-6 h-6 rounded-full border-2 flex items-center justify-center transition-colors"
                    classList={{
                      'border-green-500 bg-green-500': habit.completedToday,
                      'border-gray-300 dark:border-gray-600': !habit.completedToday,
                    }}
                  >
                    <Show when={habit.completedToday}>
                      <Icon name="check" class="w-4 h-4 text-white" />
                    </Show>
                  </div>
                  <span
                    class="text-sm"
                    classList={{
                      'line-through text-gray-400': habit.completedToday,
                      'text-gray-700 dark:text-gray-200': !habit.completedToday,
                    }}
                  >
                    {habit.name}
                  </span>
                </div>
                <div class="flex items-center gap-2">
                  <Show when={habit.streak > 0}>
                    <span class="text-xs px-2 py-0.5 rounded-full bg-orange-100 dark:bg-orange-900/40 text-orange-600 dark:text-orange-400">
                      {habit.streak} day streak
                    </span>
                  </Show>
                </div>
              </div>
            )}
          </For>
        </div>
      </div>
    </div>
  );
};

// ---------------------------------------------------------------------------
// Workouts Tab
// ---------------------------------------------------------------------------

const WorkoutsTab: Component = () => {
  const [workouts, setWorkouts] = createSignal<WorkoutResponse[]>([]);
  const [formOpen, setFormOpen] = createSignal(false);
  const [wName, setWName] = createSignal('');
  const [wDuration, setWDuration] = createSignal('');
  const [wCalories, setWCalories] = createSignal('');
  const [wNotes, setWNotes] = createSignal('');
  const [saving, setSaving] = createSignal(false);
  const [msg, setMsg] = createSignal('');

  const loadWorkouts = async () => {
    try {
      const data = await invoke<WorkoutResponse[]>('fitness_list_workouts', { limit: 50 });
      setWorkouts(data);
    } catch (e) {
      console.error('Failed to load workouts:', e);
    }
  };

  onMount(loadWorkouts);

  const handleSave = async () => {
    if (!wName().trim() || !wDuration().trim()) {
      setMsg('Name and duration are required');
      return;
    }
    setSaving(true);
    setMsg('');
    try {
      await invoke<WorkoutResponse>('fitness_log_workout', {
        name: wName().trim(),
        exercises: null,
        durationMinutes: parseFloat(wDuration()),
        caloriesBurned: wCalories() ? parseFloat(wCalories()) : null,
        notes: wNotes().trim() || null,
      });
      setWName(''); setWDuration(''); setWCalories(''); setWNotes('');
      setMsg('Workout logged!');
      setFormOpen(false);
      await loadWorkouts();
    } catch (e: any) {
      setMsg('Error: ' + String(e));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await invoke('fitness_delete_workout', { workoutId: id });
      setWorkouts((prev) => prev.filter((w) => w.id !== id));
    } catch (e) {
      console.error('Failed to delete workout:', e);
    }
  };

  // Weekly summary computed from workouts in last 7 days
  const weeklySummary = () => {
    const now = new Date();
    const weekAgo = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 7);
    const recent = workouts().filter((w) => new Date(w.date) >= weekAgo);
    return {
      count: recent.length,
      totalDuration: recent.reduce((s, w) => s + w.duration_minutes, 0),
      totalCalories: recent.reduce((s, w) => s + (w.calories_burned ?? 0), 0),
    };
  };

  return (
    <div class="space-y-6">
      {/* Weekly Summary */}
      <div class="grid grid-cols-3 gap-4">
        <div class="card p-4 text-center">
          <p class="text-2xl font-bold text-gray-900 dark:text-white">{weeklySummary().count}</p>
          <p class="text-xs text-gray-500 dark:text-gray-400">Workouts this week</p>
        </div>
        <div class="card p-4 text-center">
          <p class="text-2xl font-bold text-gray-900 dark:text-white">{Math.round(weeklySummary().totalDuration)}</p>
          <p class="text-xs text-gray-500 dark:text-gray-400">Total minutes</p>
        </div>
        <div class="card p-4 text-center">
          <p class="text-2xl font-bold text-gray-900 dark:text-white">{Math.round(weeklySummary().totalCalories)}</p>
          <p class="text-xs text-gray-500 dark:text-gray-400">Calories burned</p>
        </div>
      </div>

      {/* Log Workout button + form */}
      <div>
        <button
          class="flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-lg bg-minion-600 text-white hover:bg-minion-700 transition-colors"
          onClick={() => { setFormOpen(!formOpen()); setMsg(''); }}
        >
          <Icon name="plus" class="w-4 h-4" />
          {formOpen() ? 'Cancel' : 'Log Workout'}
        </button>

        <Show when={formOpen()}>
          <div class="card p-5 mt-3">
            <h3 class="font-medium text-gray-900 dark:text-white mb-4">Log Workout</h3>
            <div class="grid grid-cols-2 gap-4">
              <div>
                <label class="block text-xs font-medium text-gray-500 mb-1">Name *</label>
                <input type="text" class="input w-full" value={wName()}
                  onInput={(e) => setWName(e.currentTarget.value)} placeholder="e.g. Morning Run" />
              </div>
              <div>
                <label class="block text-xs font-medium text-gray-500 mb-1">Duration (min) *</label>
                <input type="number" class="input w-full" value={wDuration()}
                  onInput={(e) => setWDuration(e.currentTarget.value)} placeholder="e.g. 45" />
              </div>
              <div>
                <label class="block text-xs font-medium text-gray-500 mb-1">Calories Burned</label>
                <input type="number" class="input w-full" value={wCalories()}
                  onInput={(e) => setWCalories(e.currentTarget.value)} placeholder="e.g. 350" />
              </div>
              <div>
                <label class="block text-xs font-medium text-gray-500 mb-1">Notes</label>
                <input type="text" class="input w-full" value={wNotes()}
                  onInput={(e) => setWNotes(e.currentTarget.value)} placeholder="Optional notes" />
              </div>
            </div>
            <div class="flex items-center gap-3 mt-4">
              <button class="btn btn-primary" disabled={saving()} onClick={handleSave}>
                {saving() ? 'Saving...' : 'Save Workout'}
              </button>
              <Show when={msg()}>
                <span class="text-sm" classList={{
                  'text-green-500': msg().startsWith('Workout'),
                  'text-red-500': msg().startsWith('Error') || msg().startsWith('Name'),
                }}>{msg()}</span>
              </Show>
            </div>
          </div>
        </Show>
      </div>

      {/* Recent Workouts */}
      <div class="card p-4">
        <h3 class="font-medium text-gray-900 dark:text-white mb-4">Recent Workouts</h3>
        <Show when={workouts().length > 0} fallback={
          <p class="text-sm text-gray-500 dark:text-gray-400">No workouts logged yet. Start by logging your first workout above.</p>
        }>
          <div class="space-y-2">
            <For each={workouts()}>
              {(w) => (
                <div class="flex items-center justify-between p-3 rounded-lg bg-gray-50 dark:bg-gray-800/50">
                  <div class="flex-1 min-w-0">
                    <div class="flex items-center gap-2">
                      <span class="text-sm font-medium text-gray-900 dark:text-white truncate">{w.name}</span>
                      <span class="text-xs text-gray-400">{w.date}</span>
                    </div>
                    <div class="flex items-center gap-3 mt-1 text-xs text-gray-500 dark:text-gray-400">
                      <span>{w.duration_minutes} min</span>
                      <Show when={w.calories_burned != null}>
                        <span>{w.calories_burned} cal</span>
                      </Show>
                      <Show when={w.notes}>
                        <span class="truncate">{w.notes}</span>
                      </Show>
                    </div>
                  </div>
                  <button
                    class="ml-2 p-1.5 rounded text-gray-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
                    onClick={() => handleDelete(w.id)}
                    title="Delete workout"
                  >
                    <Icon name="trash" class="w-4 h-4" />
                  </button>
                </div>
              )}
            </For>
          </div>
        </Show>
      </div>
    </div>
  );
};

// ---------------------------------------------------------------------------
// Nutrition Tab
// ---------------------------------------------------------------------------

const MEAL_TYPES = ['Breakfast', 'Lunch', 'Dinner', 'Snack'] as const;

const NutritionTab: Component = () => {
  const today = () => new Date().toISOString().slice(0, 10);
  const [selectedDate, setSelectedDate] = createSignal(today());
  const [summary, setSummary] = createSignal<NutritionDaySummary | null>(null);
  const [formOpen, setFormOpen] = createSignal(false);
  const [nName, setNName] = createSignal('');
  const [nCalories, setNCalories] = createSignal('');
  const [nProtein, setNProtein] = createSignal('');
  const [nCarbs, setNCarbs] = createSignal('');
  const [nFat, setNFat] = createSignal('');
  const [nMealType, setNMealType] = createSignal<string>('Lunch');
  const [saving, setSaving] = createSignal(false);
  const [msg, setMsg] = createSignal('');

  const loadNutrition = async () => {
    try {
      const data = await invoke<NutritionDaySummary>('fitness_nutrition_summary', {
        date: selectedDate(),
      });
      setSummary(data);
    } catch (e) {
      console.error('Failed to load nutrition:', e);
    }
  };

  onMount(loadNutrition);

  const handleDateChange = (newDate: string) => {
    setSelectedDate(newDate);
    loadNutrition();
  };

  const handleSave = async () => {
    if (!nName().trim() || !nCalories().trim()) {
      setMsg('Name and calories are required');
      return;
    }
    setSaving(true);
    setMsg('');
    try {
      await invoke<NutritionResponse>('fitness_log_food', {
        name: nName().trim(),
        calories: parseFloat(nCalories()),
        proteinG: parseFloat(nProtein() || '0'),
        carbsG: parseFloat(nCarbs() || '0'),
        fatG: parseFloat(nFat() || '0'),
        mealType: nMealType(),
        date: selectedDate(),
      });
      setNName(''); setNCalories(''); setNProtein(''); setNCarbs(''); setNFat('');
      setMsg('Meal logged!');
      setFormOpen(false);
      await loadNutrition();
    } catch (e: any) {
      setMsg('Error: ' + String(e));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await invoke('fitness_delete_nutrition', { entryId: id });
      await loadNutrition();
    } catch (e) {
      console.error('Failed to delete entry:', e);
    }
  };

  const mealsByType = (type: string) =>
    (summary()?.meals ?? []).filter((m) => m.meal_type === type);

  // Macro bar max = total calories goal rough estimate (2200)
  const CALORIE_GOAL = 2200;

  return (
    <div class="space-y-6">
      {/* Date Picker */}
      <div class="flex items-center gap-3">
        <label class="text-sm font-medium text-gray-700 dark:text-gray-300">Date:</label>
        <input
          type="date"
          class="input"
          value={selectedDate()}
          onInput={(e) => handleDateChange(e.currentTarget.value)}
        />
        <Show when={selectedDate() !== today()}>
          <button
            class="text-xs text-minion-600 hover:underline"
            onClick={() => handleDateChange(today())}
          >
            Go to today
          </button>
        </Show>
      </div>

      {/* Daily Summary */}
      <div class="card p-6">
        <h3 class="font-medium text-gray-900 dark:text-white mb-4">Daily Summary</h3>
        <div class="grid grid-cols-4 gap-4 mb-4">
          <div class="text-center">
            <p class="text-2xl font-bold text-gray-900 dark:text-white">
              {Math.round(summary()?.total_calories ?? 0)}
            </p>
            <p class="text-xs text-gray-500 dark:text-gray-400">Calories</p>
          </div>
          <div class="text-center">
            <p class="text-2xl font-bold text-blue-600">{Math.round(summary()?.total_protein ?? 0)}g</p>
            <p class="text-xs text-gray-500 dark:text-gray-400">Protein</p>
          </div>
          <div class="text-center">
            <p class="text-2xl font-bold text-yellow-600">{Math.round(summary()?.total_carbs ?? 0)}g</p>
            <p class="text-xs text-gray-500 dark:text-gray-400">Carbs</p>
          </div>
          <div class="text-center">
            <p class="text-2xl font-bold text-red-500">{Math.round(summary()?.total_fat ?? 0)}g</p>
            <p class="text-xs text-gray-500 dark:text-gray-400">Fat</p>
          </div>
        </div>
        {/* Calorie progress bar */}
        <div>
          <div class="flex justify-between text-xs text-gray-500 mb-1">
            <span>{Math.round(summary()?.total_calories ?? 0)} cal</span>
            <span>{CALORIE_GOAL} cal goal</span>
          </div>
          <MiniBar value={summary()?.total_calories ?? 0} max={CALORIE_GOAL} colorClass="bg-green-500" />
        </div>
        {/* Macro bars */}
        <div class="grid grid-cols-3 gap-4 mt-4">
          <div>
            <div class="flex justify-between text-xs text-gray-500 mb-1">
              <span>Protein</span>
              <span>{Math.round(summary()?.total_protein ?? 0)}g</span>
            </div>
            <MiniBar value={summary()?.total_protein ?? 0} max={150} colorClass="bg-blue-500" />
          </div>
          <div>
            <div class="flex justify-between text-xs text-gray-500 mb-1">
              <span>Carbs</span>
              <span>{Math.round(summary()?.total_carbs ?? 0)}g</span>
            </div>
            <MiniBar value={summary()?.total_carbs ?? 0} max={275} colorClass="bg-yellow-500" />
          </div>
          <div>
            <div class="flex justify-between text-xs text-gray-500 mb-1">
              <span>Fat</span>
              <span>{Math.round(summary()?.total_fat ?? 0)}g</span>
            </div>
            <MiniBar value={summary()?.total_fat ?? 0} max={75} colorClass="bg-red-500" />
          </div>
        </div>
      </div>

      {/* Log Meal button + form */}
      <div>
        <button
          class="flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-lg bg-minion-600 text-white hover:bg-minion-700 transition-colors"
          onClick={() => { setFormOpen(!formOpen()); setMsg(''); }}
        >
          <Icon name="plus" class="w-4 h-4" />
          {formOpen() ? 'Cancel' : 'Log Meal'}
        </button>

        <Show when={formOpen()}>
          <div class="card p-5 mt-3">
            <h3 class="font-medium text-gray-900 dark:text-white mb-4">Log Meal</h3>
            <div class="grid grid-cols-2 sm:grid-cols-3 gap-4">
              <div>
                <label class="block text-xs font-medium text-gray-500 mb-1">Food Name *</label>
                <input type="text" class="input w-full" value={nName()}
                  onInput={(e) => setNName(e.currentTarget.value)} placeholder="e.g. Chicken Breast" />
              </div>
              <div>
                <label class="block text-xs font-medium text-gray-500 mb-1">Calories *</label>
                <input type="number" class="input w-full" value={nCalories()}
                  onInput={(e) => setNCalories(e.currentTarget.value)} placeholder="e.g. 250" />
              </div>
              <div>
                <label class="block text-xs font-medium text-gray-500 mb-1">Meal Type</label>
                <select class="input w-full" value={nMealType()}
                  onChange={(e) => setNMealType(e.currentTarget.value)}>
                  <For each={[...MEAL_TYPES]}>{(t) => <option value={t}>{t}</option>}</For>
                </select>
              </div>
              <div>
                <label class="block text-xs font-medium text-gray-500 mb-1">Protein (g)</label>
                <input type="number" step="0.1" class="input w-full" value={nProtein()}
                  onInput={(e) => setNProtein(e.currentTarget.value)} placeholder="0" />
              </div>
              <div>
                <label class="block text-xs font-medium text-gray-500 mb-1">Carbs (g)</label>
                <input type="number" step="0.1" class="input w-full" value={nCarbs()}
                  onInput={(e) => setNCarbs(e.currentTarget.value)} placeholder="0" />
              </div>
              <div>
                <label class="block text-xs font-medium text-gray-500 mb-1">Fat (g)</label>
                <input type="number" step="0.1" class="input w-full" value={nFat()}
                  onInput={(e) => setNFat(e.currentTarget.value)} placeholder="0" />
              </div>
            </div>
            <div class="flex items-center gap-3 mt-4">
              <button class="btn btn-primary" disabled={saving()} onClick={handleSave}>
                {saving() ? 'Saving...' : 'Save Meal'}
              </button>
              <Show when={msg()}>
                <span class="text-sm" classList={{
                  'text-green-500': msg().startsWith('Meal'),
                  'text-red-500': msg().startsWith('Error') || msg().startsWith('Name'),
                }}>{msg()}</span>
              </Show>
            </div>
          </div>
        </Show>
      </div>

      {/* Meals grouped by type */}
      <For each={[...MEAL_TYPES]}>
        {(type) => (
          <Show when={mealsByType(type).length > 0}>
            <div class="card p-4">
              <h4 class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-3">{type}</h4>
              <div class="space-y-2">
                <For each={mealsByType(type)}>
                  {(meal) => (
                    <div class="flex items-center justify-between p-3 rounded-lg bg-gray-50 dark:bg-gray-800/50">
                      <div class="flex-1 min-w-0">
                        <span class="text-sm font-medium text-gray-900 dark:text-white">{meal.name}</span>
                        <div class="flex items-center gap-3 mt-1 text-xs text-gray-500 dark:text-gray-400">
                          <span>{meal.calories} cal</span>
                          <span>P: {meal.protein_g}g</span>
                          <span>C: {meal.carbs_g}g</span>
                          <span>F: {meal.fat_g}g</span>
                        </div>
                      </div>
                      <button
                        class="ml-2 p-1.5 rounded text-gray-400 hover:text-red-500 hover:bg-red-50 dark:hover:bg-red-900/20 transition-colors"
                        onClick={() => handleDelete(meal.id)}
                        title="Delete entry"
                      >
                        <Icon name="trash" class="w-4 h-4" />
                      </button>
                    </div>
                  )}
                </For>
              </div>
            </div>
          </Show>
        )}
      </For>

      {/* Empty state */}
      <Show when={(summary()?.meals ?? []).length === 0}>
        <div class="card p-6 text-center">
          <p class="text-sm text-gray-500 dark:text-gray-400">
            No meals logged for {selectedDate()}. Start tracking by logging a meal above.
          </p>
        </div>
      </Show>
    </div>
  );
};

// ---------------------------------------------------------------------------
// Main Fitness Component
// ---------------------------------------------------------------------------

const TABS: { id: TabId; label: string }[] = [
  { id: 'dashboard', label: 'Dashboard' },
  { id: 'workouts', label: 'Workouts' },
  { id: 'nutrition', label: 'Nutrition' },
  { id: 'sleep', label: 'Sleep' },
  { id: 'heart', label: 'Heart' },
  { id: 'activity', label: 'Activity' },
  { id: 'ai', label: 'AI Analysis' },
  { id: 'habits', label: 'Habits' },
];

const Fitness: Component = () => {
  const [activeTab, setActiveTab] = createSignal<TabId>('dashboard');
  const [habits, setHabits] = createSignal<Habit[]>(DEFAULT_HABITS);

  // Real data from backend
  const [dashboard, setDashboard] = createSignal<FitnessDashboard | null>(null);
  const [metrics, setMetrics] = createSignal<FitnessMetricResponse[]>([]);
  const [hasRealData, setHasRealData] = createSignal(false);
  const [loading, setLoading] = createSignal(true);

  // Google Fit connection status
  const [gfitConnected, setGfitConnected] = createSignal(false);
  const [gfitClientId, setGfitClientId] = createSignal('');
  const [gfitConnectBusy, setGfitConnectBusy] = createSignal(false);
  const [gfitConnectMessage, setGfitConnectMessage] = createSignal('');

  // Log form state
  const [logFormOpen, setLogFormOpen] = createSignal(false);
  const [logWeight, setLogWeight] = createSignal('');
  const [logSteps, setLogSteps] = createSignal('');
  const [logHeartRate, setLogHeartRate] = createSignal('');
  const [logSleepHours, setLogSleepHours] = createSignal('');
  const [logWater, setLogWater] = createSignal('');
  const [logCalories, setLogCalories] = createSignal('');
  const [logSaving, setLogSaving] = createSignal(false);
  const [logMessage, setLogMessage] = createSignal('');

  const loadData = async () => {
    try {
      const [dash, mets, habs] = await Promise.all([
        invoke<FitnessDashboard>('fitness_get_dashboard'),
        invoke<FitnessMetricResponse[]>('fitness_get_metrics', { days: 30 }),
        invoke<FitnessHabitResponse[]>('fitness_list_habits'),
      ]);

      try {
        const gfit = await invoke<boolean>('gfit_check_connected');
        setGfitConnected(gfit);
        const savedId = await invoke<string | null>('gfit_get_client_id');
        if (savedId) setGfitClientId(savedId);
      } catch (_) {
        // ignore
      }
      setDashboard(dash);
      setMetrics(mets);
      // Check if any real data exists
      const hasData =
        mets.length > 0 ||
        dash.total_habits > 0 ||
        dash.latest_weight_kg !== null ||
        dash.avg_steps_7d !== null;
      setHasRealData(hasData);
      // Map backend habits into local Habit format
      if (habs.length > 0) {
        setHabits(
          habs.map((h) => ({
            id: h.id,
            name: h.name,
            streak: 0,
            completedToday: h.completed_today,
          }))
        );
      }
    } catch (e) {
      console.error('Failed to load fitness data:', e);
    } finally {
      setLoading(false);
    }
  };

  onMount(loadData);

  onMount(async () => {
    const unlisten = await listen('fitness-data-updated', () => { loadData(); });
    onCleanup(() => unlisten());
  });

  const handleLogMetric = async () => {
    setLogSaving(true);
    setLogMessage('');
    try {
      const metric: any = {};
      if (logWeight()) metric.weight_kg = parseFloat(logWeight());
      if (logSteps()) metric.steps = parseInt(logSteps(), 10);
      if (logHeartRate()) metric.heart_rate_avg = parseInt(logHeartRate(), 10);
      if (logSleepHours()) metric.sleep_hours = parseFloat(logSleepHours());
      if (logWater()) metric.water_ml = parseInt(logWater(), 10);
      if (logCalories()) metric.calories_in = parseInt(logCalories(), 10);

      await invoke('fitness_log_metric', { metric });
      setLogMessage('Metric logged successfully!');
      // Clear form
      setLogWeight(''); setLogSteps(''); setLogHeartRate('');
      setLogSleepHours(''); setLogWater(''); setLogCalories('');
      // Reload data
      await loadData();
    } catch (e: any) {
      setLogMessage('Error: ' + String(e));
    } finally {
      setLogSaving(false);
    }
  };

  const handleToggleHabit = async (habitId: string) => {
    try {
      const nowCompleted = await invoke<boolean>('fitness_toggle_habit', { habitId });
      setHabits((prev) =>
        prev.map((h) =>
          h.id === habitId ? { ...h, completedToday: nowCompleted } : h
        )
      );
      // Refresh dashboard for updated counts
      const dash = await invoke<FitnessDashboard>('fitness_get_dashboard');
      setDashboard(dash);
    } catch (e) {
      console.error('Failed to toggle habit:', e);
    }
  };

  const handleAddHabit = async () => {
    const name = prompt('Enter a new habit name:');
    if (!name || !name.trim()) return;
    try {
      const habit = await invoke<FitnessHabitResponse>('fitness_add_habit', {
        name: name.trim(),
        frequency: null,
        description: null,
      });
      setHabits((prev) => [
        ...prev,
        { id: habit.id, name: habit.name, streak: 0, completedToday: false },
      ]);
      const dash = await invoke<FitnessDashboard>('fitness_get_dashboard');
      setDashboard(dash);
    } catch (e) {
      console.error('Failed to add habit:', e);
    }
  };

  return (
    <div class="p-6">
      <div class="flex items-center justify-between mb-6">
        <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Health &amp; Fitness</h1>
        <Show when={gfitConnected()}>
          <div class="flex items-center gap-2 text-sm text-green-500">
            <div class="w-2 h-2 rounded-full bg-green-500" />
            Google Fit connected
          </div>
        </Show>
      </div>

      {/* Google Fit: connect on this page (same flow as Settings) */}
      <Show when={!loading() && !gfitConnected()}>
        <div class="card p-5 mb-6 max-w-xl">
          <h2 class="text-lg font-semibold text-gray-900 dark:text-white mb-1">Google Fit</h2>
          <p class="text-sm text-gray-500 dark:text-gray-400 mb-4">
            Enter your Desktop OAuth Client ID from Google Cloud Console, then use Connect Google Fit.
            Add redirect URI{' '}
            <code class="text-xs text-minion-600 dark:text-minion-400">http://127.0.0.1:8745/</code>
            {' '}and enable the Fitness API.
          </p>
          <label class="block text-xs font-medium text-gray-500 mb-1">OAuth Client ID</label>
          <input
            type="text"
            class="input w-full text-sm mb-3"
            placeholder="your-id.apps.googleusercontent.com"
            value={gfitClientId()}
            onInput={(e) => setGfitClientId(e.currentTarget.value)}
          />
          <div class="flex flex-wrap gap-2">
            <button
              type="button"
              class="btn btn-secondary"
              disabled={!gfitClientId().trim() || gfitConnectBusy()}
              onClick={async () => {
                try {
                  setGfitConnectMessage('');
                  await invoke('gfit_save_client_id', { clientId: gfitClientId().trim() });
                  setGfitConnectMessage('Client ID saved.');
                } catch (e) {
                  setGfitConnectMessage(String(e));
                }
              }}
            >
              Save Client ID
            </button>
            <button
              type="button"
              class="btn btn-primary inline-flex items-center gap-2"
              disabled={!gfitClientId().trim() || gfitConnectBusy()}
              onClick={async () => {
                setGfitConnectBusy(true);
                setGfitConnectMessage('');
                try {
                  await invoke('gfit_save_client_id', { clientId: gfitClientId().trim() });
                  await invoke('gfit_open_auth');
                  setGfitConnected(true);
                  setGfitConnectMessage('Connected to Google Fit.');
                  await loadData();
                } catch (e) {
                  setGfitConnectMessage(String(e));
                } finally {
                  setGfitConnectBusy(false);
                }
              }}
            >
              <Icon name="google-fit" class="w-4 h-4" />
              {gfitConnectBusy() ? 'Connecting…' : 'Connect Google Fit'}
            </button>
          </div>
          <Show when={gfitConnectMessage()}>
            <p class="text-sm mt-3 text-gray-600 dark:text-gray-300">{gfitConnectMessage()}</p>
          </Show>
        </div>
      </Show>

      <Show when={gfitConnected()}>
        <div class="mb-4">
          <span class="inline-flex items-center gap-2 px-3 py-1.5 rounded-full text-xs font-medium
                       bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400">
            <div class="w-2 h-2 rounded-full bg-green-500" />
            Synced with Google Fit
          </span>
        </div>
      </Show>

      {/* Demo data banner */}
      <Show when={!loading() && !hasRealData()}>
        <div class="mb-4 p-3 rounded-lg bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 text-sm text-amber-700 dark:text-amber-300">
          Showing demo data. Log your first metric to see real data.
        </div>
      </Show>

      {/* Log Today's Data - Collapsible */}
      <div class="mb-6">
        <button
          class="flex items-center gap-2 px-4 py-2 text-sm font-medium rounded-lg bg-minion-600 text-white hover:bg-minion-700 transition-colors"
          onClick={() => setLogFormOpen(!logFormOpen())}
        >
          <Icon name="plus" class="w-4 h-4" />
          {logFormOpen() ? 'Hide Log Form' : "Log Today's Data"}
        </button>

        <Show when={logFormOpen()}>
          <div class="card p-5 mt-3">
            <h3 class="font-medium text-gray-900 dark:text-white mb-4">Log Today's Metrics</h3>
            <div class="grid grid-cols-2 sm:grid-cols-3 gap-4">
              <div>
                <label class="block text-xs font-medium text-gray-500 mb-1">Weight (kg)</label>
                <input
                  type="number"
                  step="0.1"
                  class="input w-full"
                  value={logWeight()}
                  onInput={(e) => setLogWeight(e.currentTarget.value)}
                  placeholder="e.g. 75.5"
                />
              </div>
              <div>
                <label class="block text-xs font-medium text-gray-500 mb-1">Steps</label>
                <input
                  type="number"
                  class="input w-full"
                  value={logSteps()}
                  onInput={(e) => setLogSteps(e.currentTarget.value)}
                  placeholder="e.g. 8000"
                />
              </div>
              <div>
                <label class="block text-xs font-medium text-gray-500 mb-1">Heart Rate (avg BPM)</label>
                <input
                  type="number"
                  class="input w-full"
                  value={logHeartRate()}
                  onInput={(e) => setLogHeartRate(e.currentTarget.value)}
                  placeholder="e.g. 72"
                />
              </div>
              <div>
                <label class="block text-xs font-medium text-gray-500 mb-1">Sleep (hours)</label>
                <input
                  type="number"
                  step="0.1"
                  class="input w-full"
                  value={logSleepHours()}
                  onInput={(e) => setLogSleepHours(e.currentTarget.value)}
                  placeholder="e.g. 7.5"
                />
              </div>
              <div>
                <label class="block text-xs font-medium text-gray-500 mb-1">Water (ml)</label>
                <input
                  type="number"
                  class="input w-full"
                  value={logWater()}
                  onInput={(e) => setLogWater(e.currentTarget.value)}
                  placeholder="e.g. 2000"
                />
              </div>
              <div>
                <label class="block text-xs font-medium text-gray-500 mb-1">Calories In</label>
                <input
                  type="number"
                  class="input w-full"
                  value={logCalories()}
                  onInput={(e) => setLogCalories(e.currentTarget.value)}
                  placeholder="e.g. 2200"
                />
              </div>
            </div>
            <div class="flex items-center gap-3 mt-4">
              <button
                class="btn btn-primary"
                disabled={logSaving()}
                onClick={handleLogMetric}
              >
                {logSaving() ? 'Saving...' : 'Save Metric'}
              </button>
              <Show when={logMessage()}>
                <span
                  class="text-sm"
                  classList={{
                    'text-green-500': logMessage().startsWith('Metric'),
                    'text-red-500': logMessage().startsWith('Error'),
                  }}
                >
                  {logMessage()}
                </span>
              </Show>
            </div>
          </div>
        </Show>
      </div>

      {/* Tabs */}
      <div class="flex border-b border-gray-200 dark:border-gray-700 mb-6 overflow-x-auto">
          <For each={TABS}>
            {(tab) => (
              <button
                class="px-4 py-2 -mb-px border-b-2 transition-colors whitespace-nowrap text-sm font-medium"
                classList={{
                  'border-minion-500 text-minion-600 dark:text-minion-400': activeTab() === tab.id,
                  'border-transparent text-gray-500 hover:text-gray-700 dark:hover:text-gray-300':
                    activeTab() !== tab.id,
                }}
                onClick={() => setActiveTab(tab.id)}
              >
                {tab.label}
              </button>
            )}
          </For>
        </div>

        {/* Tab Content */}
        <Switch>
          <Match when={activeTab() === 'dashboard'}>
            <DashboardTab dashboard={dashboard} metrics={metrics} hasRealData={hasRealData} />
          </Match>
          <Match when={activeTab() === 'workouts'}>
            <WorkoutsTab />
          </Match>
          <Match when={activeTab() === 'nutrition'}>
            <NutritionTab />
          </Match>
          <Match when={activeTab() === 'sleep'}>
            <SleepTab
              metrics={metrics}
              gfitConnected={gfitConnected}
              onSync={async () => { await invoke('gfit_sync'); await loadData(); }}
            />
          </Match>
          <Match when={activeTab() === 'heart'}>
            <HeartTab
              metrics={metrics}
              gfitConnected={gfitConnected}
              onSync={async () => { await invoke('gfit_sync'); await loadData(); }}
            />
          </Match>
          <Match when={activeTab() === 'activity'}>
            <ActivityTab />
          </Match>
          <Match when={activeTab() === 'ai'}>
            <AiAnalysisTab dashboard={dashboard} metrics={metrics} />
          </Match>
          <Match when={activeTab() === 'habits'}>
            <HabitsTab
              habits={habits}
              setHabits={setHabits}
              onToggle={handleToggleHabit}
              onAdd={handleAddHabit}
            />
          </Match>
        </Switch>
    </div>
  );
};

export default Fitness;
