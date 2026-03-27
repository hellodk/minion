import { Component, createSignal, For, Show, Switch, Match } from 'solid-js';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type TabId = 'dashboard' | 'sleep' | 'heart' | 'activity' | 'ai' | 'habits';

interface Habit {
  id: string;
  name: string;
  streak: number;
  completedToday: boolean;
}

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
    </Switch>
  );
};

// ---------------------------------------------------------------------------
// Tab content components
// ---------------------------------------------------------------------------

const DashboardTab: Component = () => {
  const maxSteps = Math.max(...WEEKLY_STEPS.map((d) => d.steps));

  return (
    <div class="space-y-6">
      {/* Top row: Health Score + Quick Stats */}
      <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Health Score */}
        <div class="card p-6 flex flex-col items-center justify-center">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
            Health Score
          </h3>
          <CircularProgress value={72} max={100} size="w-40 h-40" sublabel="/ 100" />
          <p class="mt-3 text-sm text-gray-500 dark:text-gray-400">Good - Keep it up!</p>
        </div>

        {/* Quick Stats Grid */}
        <div class="lg:col-span-2 grid grid-cols-1 sm:grid-cols-2 gap-4">
          {/* Steps Today */}
          <div class="card p-5">
            <div class="flex items-center gap-3 mb-3">
              <div class="p-2 rounded-lg bg-blue-100 dark:bg-blue-900/40 text-blue-600 dark:text-blue-400">
                <Icon name="steps" class="w-5 h-5" />
              </div>
              <span class="text-sm font-medium text-gray-500 dark:text-gray-400">Steps Today</span>
            </div>
            <p class="text-2xl font-bold text-gray-900 dark:text-white mb-1">
              8,432 <span class="text-sm font-normal text-gray-400">/ 10,000</span>
            </p>
            <MiniBar value={8432} max={10000} colorClass="bg-blue-500" />
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
              <p class="text-2xl font-bold text-gray-900 dark:text-white">72</p>
              <span class="text-sm text-gray-400 mb-1">BPM</span>
              <span class="text-green-500 ml-auto flex items-center gap-0.5 text-sm">
                <Icon name="arrow-down" class="w-3 h-3" /> 3
              </span>
            </div>
            <p class="text-xs text-gray-400 mt-1">Resting: 62 BPM</p>
          </div>

          {/* Sleep */}
          <div class="card p-5">
            <div class="flex items-center gap-3 mb-3">
              <div class="p-2 rounded-lg bg-indigo-100 dark:bg-indigo-900/40 text-indigo-500">
                <Icon name="moon" class="w-5 h-5" />
              </div>
              <span class="text-sm font-medium text-gray-500 dark:text-gray-400">Sleep Last Night</span>
            </div>
            <p class="text-2xl font-bold text-gray-900 dark:text-white mb-1">7h 23m</p>
            <div class="flex items-center gap-2">
              <span class="text-xs px-2 py-0.5 rounded-full bg-green-100 dark:bg-green-900/40 text-green-600 dark:text-green-400">
                Good quality
              </span>
            </div>
          </div>

          {/* Calories */}
          <div class="card p-5">
            <div class="flex items-center gap-3 mb-3">
              <div class="p-2 rounded-lg bg-orange-100 dark:bg-orange-900/40 text-orange-500">
                <Icon name="fire" class="w-5 h-5" />
              </div>
              <span class="text-sm font-medium text-gray-500 dark:text-gray-400">Calories Burned</span>
            </div>
            <p class="text-2xl font-bold text-gray-900 dark:text-white mb-1">
              1,847 <span class="text-sm font-normal text-gray-400">/ 2,200</span>
            </p>
            <MiniBar value={1847} max={2200} colorClass="bg-orange-500" />
          </div>
        </div>
      </div>

      {/* Weekly Activity Chart */}
      <div class="card p-6">
        <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
          Weekly Activity
        </h3>
        <div class="flex items-end gap-3 h-40">
          <For each={WEEKLY_STEPS}>
            {(entry) => (
              <div class="flex-1 flex flex-col items-center gap-1">
                <span class="text-xs text-gray-400">{(entry.steps / 1000).toFixed(1)}k</span>
                <div class="w-full flex justify-center">
                  <div
                    class="w-full max-w-[40px] rounded-t-md bg-minion-500 dark:bg-minion-400 transition-all duration-500"
                    style={{ height: `${(entry.steps / maxSteps) * 120}px` }}
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
              Your sleep quality has improved 12% this week. Consider maintaining your 10:30 PM
              bedtime routine. Your step count is trending 8% above last week's average, and your
              resting heart rate has decreased by 2 BPM -- a sign of improving cardiovascular fitness.
            </p>
          </div>
        </div>
      </div>
    </div>
  );
};

const SleepTab: Component = () => {
  const maxSleep = Math.max(...WEEKLY_SLEEP.map((d) => d.hours));
  const totalSleepMin = SLEEP_STAGES.reduce((a, s) => a + s.minutes, 0);

  const SLEEP_QUALITY_FACTORS = [
    { label: 'Consistency', score: 85, description: 'Regular bedtime schedule' },
    { label: 'Duration', score: 78, description: '7h 23m avg (target: 7-9h)' },
    { label: 'Efficiency', score: 93, description: '93% time asleep in bed' },
    { label: 'Timing', score: 72, description: 'Slightly late average bedtime' },
  ];

  return (
    <div class="space-y-6">
      {/* Top: Sleep Score + Last Night Summary */}
      <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Sleep Score */}
        <div class="card p-6 flex flex-col items-center justify-center">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
            Sleep Score
          </h3>
          <CircularProgress value={78} max={100} size="w-40 h-40" colorClass="text-indigo-500" sublabel="/ 100" />
          <p class="mt-3 text-sm text-gray-500 dark:text-gray-400">Good</p>
        </div>

        {/* Last Night Summary */}
        <div class="lg:col-span-2 card p-6">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
            Last Night Summary
          </h3>
          <div class="grid grid-cols-2 sm:grid-cols-4 gap-4 mb-6">
            <div>
              <p class="text-xs text-gray-400">Bedtime</p>
              <p class="text-lg font-semibold text-gray-900 dark:text-white">10:37 PM</p>
            </div>
            <div>
              <p class="text-xs text-gray-400">Wake Time</p>
              <p class="text-lg font-semibold text-gray-900 dark:text-white">6:00 AM</p>
            </div>
            <div>
              <p class="text-xs text-gray-400">Duration</p>
              <p class="text-lg font-semibold text-gray-900 dark:text-white">7h 23m</p>
            </div>
            <div>
              <p class="text-xs text-gray-400">Efficiency</p>
              <p class="text-lg font-semibold text-gray-900 dark:text-white">93%</p>
            </div>
          </div>

          {/* Sleep Stage Bars */}
          <h4 class="text-xs text-gray-400 mb-3 uppercase tracking-wide">Sleep Stages</h4>
          <div class="space-y-3">
            <For each={SLEEP_STAGES}>
              {(stage) => (
                <div class="flex items-center gap-3">
                  <span class="text-sm text-gray-600 dark:text-gray-300 w-24 shrink-0">
                    {stage.label}
                  </span>
                  <div class="flex-1 h-4 rounded-full bg-gray-200 dark:bg-gray-700 overflow-hidden">
                    <div
                      class={`h-full rounded-full ${stage.color} transition-all duration-500`}
                      style={{ width: `${(stage.minutes / totalSleepMin) * 100}%` }}
                    />
                  </div>
                  <span class="text-sm text-gray-500 dark:text-gray-400 w-16 text-right shrink-0">
                    {stage.duration}
                  </span>
                </div>
              )}
            </For>
          </div>
        </div>
      </div>

      {/* Sleep Trend */}
      <div class="card p-6">
        <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
          7-Day Sleep Trend
        </h3>
        <div class="flex items-end gap-3 h-40">
          <For each={WEEKLY_SLEEP}>
            {(entry) => (
              <div class="flex-1 flex flex-col items-center gap-1">
                <span class="text-xs text-gray-400">{entry.hours.toFixed(1)}h</span>
                <div class="w-full flex justify-center">
                  <div
                    class="w-full max-w-[40px] rounded-t-md bg-indigo-500 dark:bg-indigo-400 transition-all duration-500"
                    style={{ height: `${(entry.hours / maxSleep) * 120}px` }}
                  />
                </div>
                <span class="text-xs font-medium text-gray-500 dark:text-gray-400">{entry.day}</span>
              </div>
            )}
          </For>
        </div>
      </div>

      {/* Sleep Quality Factors */}
      <div class="card p-6">
        <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
          Sleep Quality Factors
        </h3>
        <div class="space-y-4">
          <For each={SLEEP_QUALITY_FACTORS}>
            {(factor) => (
              <div>
                <div class="flex items-center justify-between mb-1">
                  <span class="text-sm font-medium text-gray-700 dark:text-gray-200">
                    {factor.label}
                  </span>
                  <span class="text-sm font-semibold text-gray-900 dark:text-white">
                    {factor.score}/100
                  </span>
                </div>
                <MiniBar value={factor.score} max={100} colorClass="bg-indigo-500" />
                <p class="text-xs text-gray-400 mt-1">{factor.description}</p>
              </div>
            )}
          </For>
        </div>
      </div>
    </div>
  );
};

const HeartTab: Component = () => {
  // SVG line chart points for 7 day heart rate
  const chartWidth = 600;
  const chartHeight = 120;
  const minHR = Math.min(...WEEKLY_HEART_RATE.map((d) => d.min)) - 5;
  const maxHR = Math.max(...WEEKLY_HEART_RATE.map((d) => d.max)) + 5;
  const scaleY = (v: number) => chartHeight - ((v - minHR) / (maxHR - minHR)) * chartHeight;
  const avgPoints = WEEKLY_HEART_RATE.map(
    (d, i) => `${(i / (WEEKLY_HEART_RATE.length - 1)) * chartWidth},${scaleY(d.avg)}`
  ).join(' ');
  const minPoints = WEEKLY_HEART_RATE.map(
    (d, i) => `${(i / (WEEKLY_HEART_RATE.length - 1)) * chartWidth},${scaleY(d.min)}`
  ).join(' ');
  const maxPoints = WEEKLY_HEART_RATE.map(
    (d, i) => `${(i / (WEEKLY_HEART_RATE.length - 1)) * chartWidth},${scaleY(d.max)}`
  ).join(' ');

  return (
    <div class="space-y-6">
      {/* Current BPM + Resting */}
      <div class="grid grid-cols-1 sm:grid-cols-2 gap-6">
        {/* Current BPM with pulse animation */}
        <div class="card p-6 flex flex-col items-center justify-center">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
            Current Heart Rate
          </h3>
          <div class="flex items-center gap-4">
            <div class="animate-pulse-heart text-red-500">
              <Icon name="heart" class="w-12 h-12" />
            </div>
            <div>
              <span class="text-5xl font-bold text-gray-900 dark:text-white">72</span>
              <span class="text-lg text-gray-400 ml-1">BPM</span>
            </div>
          </div>
          <p class="text-sm text-gray-400 mt-3">Normal range</p>
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

        {/* Resting Heart Rate */}
        <div class="card p-6 flex flex-col items-center justify-center">
          <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
            Resting Heart Rate
          </h3>
          <CircularProgress value={62} max={100} size="w-36 h-36" colorClass="text-red-500" sublabel="BPM" />
          <div class="flex items-center gap-1 mt-3 text-sm text-green-500">
            <Icon name="arrow-down" class="w-3 h-3" />
            <span>2 BPM from last week</span>
          </div>
        </div>
      </div>

      {/* Heart Rate Zones */}
      <div class="card p-6">
        <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
          Heart Rate Zones (Today)
        </h3>
        <div class="space-y-4">
          <For each={HEART_RATE_ZONES}>
            {(zone) => (
              <div>
                <div class="flex items-center justify-between mb-1">
                  <div class="flex items-center gap-2">
                    <div class={`w-3 h-3 rounded-full ${zone.color}`} />
                    <span class="text-sm font-medium text-gray-700 dark:text-gray-200">
                      {zone.label}
                    </span>
                    <span class="text-xs text-gray-400">({zone.bpmRange} BPM)</span>
                  </div>
                  <span class="text-sm text-gray-500 dark:text-gray-400">
                    {zone.minutes >= 60
                      ? `${Math.floor(zone.minutes / 60)}h ${zone.minutes % 60}m`
                      : `${zone.minutes}m`}
                  </span>
                </div>
                <div class="w-full h-3 rounded-full bg-gray-200 dark:bg-gray-700 overflow-hidden">
                  <div
                    class={`h-full rounded-full ${zone.color} transition-all duration-500`}
                    style={{ width: `${(zone.minutes / zone.maxMinutes) * 100}%` }}
                  />
                </div>
              </div>
            )}
          </For>
        </div>
      </div>

      {/* 7-Day Trend (SVG line chart) */}
      <div class="card p-6">
        <h3 class="text-sm font-medium text-gray-500 dark:text-gray-400 mb-4 uppercase tracking-wide">
          7-Day Heart Rate Trend
        </h3>
        <div class="overflow-x-auto">
          <svg viewBox={`-30 -10 ${chartWidth + 60} ${chartHeight + 40}`} class="w-full h-44">
            {/* Y-axis labels */}
            <text x="-10" y={scaleY(60) + 4} class="fill-gray-400 text-xs" text-anchor="end"
              font-size="10">60</text>
            <text x="-10" y={scaleY(100) + 4} class="fill-gray-400 text-xs" text-anchor="end"
              font-size="10">100</text>
            <text x="-10" y={scaleY(140) + 4} class="fill-gray-400 text-xs" text-anchor="end"
              font-size="10">140</text>
            {/* Grid lines */}
            <line x1="0" y1={scaleY(60)} x2={chartWidth} y2={scaleY(60)}
              stroke="currentColor" class="text-gray-200 dark:text-gray-700" stroke-dasharray="4" />
            <line x1="0" y1={scaleY(100)} x2={chartWidth} y2={scaleY(100)}
              stroke="currentColor" class="text-gray-200 dark:text-gray-700" stroke-dasharray="4" />
            <line x1="0" y1={scaleY(140)} x2={chartWidth} y2={scaleY(140)}
              stroke="currentColor" class="text-gray-200 dark:text-gray-700" stroke-dasharray="4" />
            {/* Max line */}
            <polyline points={maxPoints} fill="none" stroke="#fca5a5" stroke-width="1.5"
              stroke-dasharray="4" />
            {/* Avg line */}
            <polyline points={avgPoints} fill="none" stroke="#ef4444" stroke-width="2.5"
              stroke-linecap="round" stroke-linejoin="round" />
            {/* Min line */}
            <polyline points={minPoints} fill="none" stroke="#93c5fd" stroke-width="1.5"
              stroke-dasharray="4" />
            {/* Dots on avg */}
            <For each={WEEKLY_HEART_RATE}>
              {(d, i) => (
                <circle
                  cx={(i() / (WEEKLY_HEART_RATE.length - 1)) * chartWidth}
                  cy={scaleY(d.avg)}
                  r="4"
                  fill="#ef4444"
                />
              )}
            </For>
            {/* X-axis labels */}
            <For each={WEEKLY_HEART_RATE}>
              {(d, i) => (
                <text
                  x={(i() / (WEEKLY_HEART_RATE.length - 1)) * chartWidth}
                  y={chartHeight + 18}
                  text-anchor="middle"
                  class="fill-gray-400"
                  font-size="10"
                >
                  {d.day}
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
            <div class="w-6 h-0.5 bg-red-300 rounded border-dashed" />
            <span>Max</span>
          </div>
          <div class="flex items-center gap-1">
            <div class="w-6 h-0.5 bg-blue-300 rounded" />
            <span>Min</span>
          </div>
        </div>
      </div>
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

const AiAnalysisTab: Component = () => {
  return (
    <div class="space-y-6">
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
}> = (props) => {
  const toggleHabit = (id: string) => {
    props.setHabits((prev: Habit[]) =>
      prev.map((h) =>
        h.id === id
          ? {
              ...h,
              completedToday: !h.completedToday,
              streak: !h.completedToday ? h.streak + 1 : Math.max(h.streak - 1, 0),
            }
          : h
      )
    );
  };

  const addHabit = () => {
    const name = prompt('Enter a new habit name:');
    if (!name || !name.trim()) return;
    const id = String(Date.now());
    props.setHabits((prev: Habit[]) => [...prev, { id, name: name.trim(), streak: 0, completedToday: false }]);
  };

  const completedCount = () => props.habits().filter((h) => h.completedToday).length;

  return (
    <div class="space-y-6">
      {/* Summary */}
      <div class="card p-6 flex items-center gap-6">
        <CircularProgress
          value={completedCount()}
          max={props.habits().length}
          size="w-28 h-28"
          colorClass="text-green-500"
          sublabel={`/ ${props.habits().length}`}
        />
        <div>
          <h3 class="text-lg font-semibold text-gray-900 dark:text-white">Today's Progress</h3>
          <p class="text-sm text-gray-500 dark:text-gray-400">
            {completedCount()} of {props.habits().length} habits completed
          </p>
          <Show when={completedCount() === props.habits().length}>
            <p class="text-sm text-green-500 font-medium mt-1">All habits done -- great job!</p>
          </Show>
        </div>
      </div>

      {/* Habit List */}
      <div class="card p-4">
        <div class="flex items-center justify-between mb-4">
          <h3 class="font-medium text-gray-900 dark:text-white">Today's Habits</h3>
          <button
            onClick={addHabit}
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
                onClick={() => toggleHabit(habit.id)}
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
// Google Fit Onboarding
// ---------------------------------------------------------------------------

const GoogleFitOnboarding: Component<{ onConnect: () => void }> = (props) => {
  return (
    <div class="flex items-center justify-center min-h-[60vh]">
      <div class="card p-8 max-w-md w-full text-center">
        {/* Google Fit Logo Placeholder */}
        <div class="mx-auto mb-6 w-20 h-20 rounded-full bg-gradient-to-br from-blue-400 via-green-400 to-red-400 flex items-center justify-center">
          <svg class="w-10 h-10 text-white" fill="currentColor" viewBox="0 0 24 24">
            <path d="M4.318 6.318a4.5 4.5 0 000 6.364L12 20.364l7.682-7.682a4.5 4.5 0 00-6.364-6.364L12 7.636l-1.318-1.318a4.5 4.5 0 00-6.364 0z" />
          </svg>
        </div>

        <h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">
          Connect Google Fit
        </h2>
        <p class="text-gray-500 dark:text-gray-400 mb-6 leading-relaxed">
          Sync your health and fitness data from Google Fit to get personalized insights,
          AI-powered health analysis, and comprehensive wellness tracking.
        </p>

        <div class="space-y-3 text-left mb-8">
          {[
            'Steps, heart rate, and activity data',
            'Sleep tracking and analysis',
            'AI-powered health recommendations',
            'Personalized supplement and nutrition advice',
          ].map((item) => (
            <div class="flex items-center gap-3 text-sm text-gray-600 dark:text-gray-300">
              <div class="w-5 h-5 rounded-full bg-green-100 dark:bg-green-900/40 text-green-500 flex items-center justify-center shrink-0">
                <Icon name="check" class="w-3 h-3" />
              </div>
              <span>{item}</span>
            </div>
          ))}
        </div>

        <button
          onClick={props.onConnect}
          class="w-full px-6 py-3 rounded-lg bg-minion-600 text-white font-medium hover:bg-minion-700 transition-colors flex items-center justify-center gap-2"
        >
          <Icon name="google-fit" class="w-5 h-5" />
          Connect Google Fit
        </button>

        <p class="text-xs text-gray-400 mt-4">
          Your data is encrypted and stored locally. We never share your health data.
        </p>
      </div>
    </div>
  );
};

// ---------------------------------------------------------------------------
// Main Fitness Component
// ---------------------------------------------------------------------------

const TABS: { id: TabId; label: string }[] = [
  { id: 'dashboard', label: 'Dashboard' },
  { id: 'sleep', label: 'Sleep' },
  { id: 'heart', label: 'Heart' },
  { id: 'activity', label: 'Activity' },
  { id: 'ai', label: 'AI Analysis' },
  { id: 'habits', label: 'Habits' },
];

const Fitness: Component = () => {
  const [connected, setConnected] = createSignal(true);
  const [activeTab, setActiveTab] = createSignal<TabId>('dashboard');
  const [habits, setHabits] = createSignal<Habit[]>(DEFAULT_HABITS);

  return (
    <div class="p-6">
      <div class="flex items-center justify-between mb-6">
        <h1 class="text-2xl font-bold text-gray-900 dark:text-white">Health &amp; Fitness</h1>
        <Show when={connected()}>
          <div class="flex items-center gap-2 text-sm text-green-500">
            <div class="w-2 h-2 rounded-full bg-green-500" />
            Google Fit Connected
          </div>
        </Show>
      </div>

      <Show when={!connected()}>
        <GoogleFitOnboarding onConnect={() => setConnected(true)} />
      </Show>

      <Show when={connected()}>
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
            <DashboardTab />
          </Match>
          <Match when={activeTab() === 'sleep'}>
            <SleepTab />
          </Match>
          <Match when={activeTab() === 'heart'}>
            <HeartTab />
          </Match>
          <Match when={activeTab() === 'activity'}>
            <ActivityTab />
          </Match>
          <Match when={activeTab() === 'ai'}>
            <AiAnalysisTab />
          </Match>
          <Match when={activeTab() === 'habits'}>
            <HabitsTab habits={habits} setHabits={setHabits} />
          </Match>
        </Switch>
      </Show>
    </div>
  );
};

export default Fitness;
