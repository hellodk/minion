import { Component, createSignal, createEffect, For, Show, onMount } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import ImportTab from './health/ImportTab';
import ReviewTab from './health/ReviewTab';
import DocumentsTab from './health/DocumentsTab';

// =====================================================================
// Types
// =====================================================================

interface Patient {
  id: string;
  phone_number: string;
  full_name: string;
  date_of_birth?: string;
  sex?: string;
  blood_group?: string;
  relationship: string;
  is_primary: boolean;
  avatar_color?: string;
  notes?: string;
  created_at: string;
  updated_at: string;
}

interface HealthConsent {
  id: number;
  accepted_at: string;
  version: string;
  local_only_mode: boolean;
  drive_sync_enabled: boolean;
  cloud_llm_allowed: boolean;
}

interface MedicalRecord {
  id: string;
  patient_id: string;
  record_type: string;
  title: string;
  description?: string;
  date: string;
  tags?: string;
  created_at: string;
}

interface LabTest {
  id: string;
  patient_id: string;
  test_name: string;
  canonical_name?: string;
  test_category?: string;
  value: number;
  unit?: string;
  reference_low?: number;
  reference_high?: number;
  reference_text?: string;
  flag?: string;
  collected_at: string;
  source?: string;
}

interface Medication {
  id: string;
  patient_id: string;
  name: string;
  generic_name?: string;
  dose?: string;
  frequency?: string;
  route?: string;
  start_date?: string;
  end_date?: string;
  indication?: string;
  notes?: string;
}

interface HealthCondition {
  id: string;
  patient_id: string;
  name: string;
  condition_type?: string;
  severity?: string;
  diagnosed_at?: string;
  resolved_at?: string;
  notes?: string;
}

interface Vital {
  id: string;
  patient_id: string;
  measurement_type: string;
  value: number;
  unit?: string;
  measured_at: string;
  context?: string;
  notes?: string;
}

interface FamilyHistoryEntry {
  id: string;
  patient_id: string;
  relation: string;
  condition: string;
  age_at_diagnosis?: number;
  notes?: string;
}

interface LifeEvent {
  id: string;
  patient_id: string;
  category: string;
  subcategory?: string;
  title: string;
  description?: string;
  intensity?: number;
  started_at: string;
  ended_at?: string;
  tags?: string;
}

interface Symptom {
  id: string;
  patient_id: string;
  description: string;
  canonical_name?: string;
  body_part?: string;
  severity?: number;
  first_noticed: string;
  resolved_at?: string;
  frequency?: string;
  notes?: string;
}

type HealthTab =
  | 'dashboard'
  | 'records'
  | 'labs'
  | 'medications'
  | 'conditions'
  | 'vitals'
  | 'life_events'
  | 'symptoms'
  | 'family'
  | 'import'
  | 'review'
  | 'documents';

// =====================================================================
// Life event categories (including yoga/meditation/spiritual)
// =====================================================================

const LIFE_EVENT_CATEGORIES: Array<{ value: string; label: string; emoji: string }> = [
  { value: 'work', label: 'Work', emoji: '💼' },
  { value: 'diet', label: 'Diet', emoji: '🥗' },
  { value: 'exercise', label: 'Exercise', emoji: '🏃' },
  { value: 'yoga', label: 'Yoga', emoji: '🧘' },
  { value: 'meditation', label: 'Meditation', emoji: '🕉️' },
  { value: 'spiritual', label: 'Spiritual', emoji: '🙏' },
  { value: 'travel', label: 'Travel', emoji: '✈️' },
  { value: 'relationship', label: 'Relationship', emoji: '❤️' },
  { value: 'stress', label: 'Stress', emoji: '😰' },
  { value: 'injury', label: 'Injury', emoji: '🩹' },
  { value: 'illness', label: 'Illness', emoji: '🤒' },
  { value: 'sleep', label: 'Sleep', emoji: '😴' },
  { value: 'habit', label: 'Habit', emoji: '🎯' },
  { value: 'environment', label: 'Environment', emoji: '🌍' },
  { value: 'other', label: 'Other', emoji: '📌' },
];

// =====================================================================
// Consent / First-run modal
// =====================================================================

const ConsentModal: Component<{
  onAccept: (patient: Patient) => void;
}> = (props) => {
  const [name, setName] = createSignal('');
  const [phone, setPhone] = createSignal('');
  const [dob, setDob] = createSignal('');
  const [sex, setSex] = createSignal('M');
  const [localOnly, setLocalOnly] = createSignal(true);
  const [cloudLlm, setCloudLlm] = createSignal(false);
  const [accepted, setAccepted] = createSignal(false);
  const [submitting, setSubmitting] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  const submit = async () => {
    if (!name().trim() || !phone().trim() || !accepted()) {
      setError('Please fill in required fields and accept the terms.');
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      await invoke('health_accept_consent', {
        localOnlyMode: localOnly(),
        driveSyncEnabled: !localOnly(),
        cloudLlmAllowed: cloudLlm(),
      });
      const patient = await invoke<Patient>('health_create_patient', {
        request: {
          phone_number: phone().trim(),
          full_name: name().trim(),
          date_of_birth: dob() || null,
          sex: sex(),
          blood_group: null,
          relationship: 'self',
          is_primary: true,
          avatar_color: '#0ea5e9',
          notes: null,
        },
      });
      props.onAccept(patient);
    } catch (e) {
      setError(String(e));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4">
      <div class="card w-full max-w-xl max-h-[90vh] overflow-y-auto shadow-2xl">
        <div class="p-6">
          <h2 class="text-2xl font-bold mb-2">Health Vault — First-time Setup</h2>
          <p class="text-sm text-gray-500 dark:text-gray-400 mb-6">
            Welcome to MINION Health. Your medical records will be stored locally on
            your device, encrypted with AES-256-GCM.
          </p>

          {/* Patient info */}
          <div class="mb-6">
            <h3 class="text-sm font-semibold mb-3 text-gray-700 dark:text-gray-200">
              1. WHO ARE YOU?
            </h3>
            <div class="space-y-3">
              <div>
                <label class="block text-xs font-medium mb-1">Full name *</label>
                <input
                  type="text"
                  class="input w-full"
                  placeholder="Your full name"
                  value={name()}
                  onInput={(e) => setName(e.currentTarget.value)}
                />
              </div>
              <div>
                <label class="block text-xs font-medium mb-1">Phone number * (unique)</label>
                <input
                  type="tel"
                  class="input w-full"
                  placeholder="+91 9876543210"
                  value={phone()}
                  onInput={(e) => setPhone(e.currentTarget.value)}
                />
              </div>
              <div class="grid grid-cols-2 gap-3">
                <div>
                  <label class="block text-xs font-medium mb-1">Date of birth</label>
                  <input
                    type="date"
                    class="input w-full"
                    value={dob()}
                    onInput={(e) => setDob(e.currentTarget.value)}
                  />
                </div>
                <div>
                  <label class="block text-xs font-medium mb-1">Sex</label>
                  <select
                    class="input w-full"
                    value={sex()}
                    onChange={(e) => setSex(e.currentTarget.value)}
                  >
                    <option value="M">Male</option>
                    <option value="F">Female</option>
                    <option value="other">Other</option>
                  </select>
                </div>
              </div>
              <p class="text-xs text-gray-400">You can add family members later.</p>
            </div>
          </div>

          {/* Privacy mode */}
          <div class="mb-6">
            <h3 class="text-sm font-semibold mb-3 text-gray-700 dark:text-gray-200">
              2. PRIVACY MODE
            </h3>
            <div class="space-y-2">
              <label class="flex items-start gap-3 cursor-pointer">
                <input
                  type="radio"
                  checked={localOnly()}
                  onChange={() => setLocalOnly(true)}
                  class="mt-1"
                />
                <div>
                  <div class="text-sm font-medium">Local only</div>
                  <div class="text-xs text-gray-500">No data leaves your device.</div>
                </div>
              </label>
              <label class="flex items-start gap-3 cursor-pointer">
                <input
                  type="radio"
                  checked={!localOnly()}
                  onChange={() => setLocalOnly(false)}
                  class="mt-1"
                />
                <div>
                  <div class="text-sm font-medium">Local + Google Drive backup</div>
                  <div class="text-xs text-gray-500">
                    Encrypted backup to your Drive (hidden app folder).
                  </div>
                </div>
              </label>
            </div>
          </div>

          {/* AI */}
          <div class="mb-6">
            <h3 class="text-sm font-semibold mb-3 text-gray-700 dark:text-gray-200">
              3. AI ANALYSIS
            </h3>
            <div class="space-y-2">
              <label class="flex items-start gap-3 cursor-pointer">
                <input
                  type="radio"
                  checked={!cloudLlm()}
                  onChange={() => setCloudLlm(false)}
                  class="mt-1"
                />
                <div>
                  <div class="text-sm font-medium">Local LLM only</div>
                  <div class="text-xs text-gray-500">Ollama / llama.cpp / AirLLM</div>
                </div>
              </label>
              <label class="flex items-start gap-3 cursor-pointer">
                <input
                  type="radio"
                  checked={cloudLlm()}
                  onChange={() => setCloudLlm(true)}
                  class="mt-1"
                />
                <div>
                  <div class="text-sm font-medium">Allow cloud LLM</div>
                  <div class="text-xs text-gray-500">
                    Per-analysis consent required each time.
                  </div>
                </div>
              </label>
            </div>
          </div>

          {/* Disclaimer */}
          <div class="mb-6 p-3 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 rounded-lg">
            <p class="text-xs text-amber-900 dark:text-amber-200">
              ⚠ MINION's AI analysis is <strong>EDUCATIONAL ONLY</strong>, not medical advice.
              Always consult a licensed physician for medical decisions.
            </p>
          </div>

          <label class="flex items-start gap-3 mb-6 cursor-pointer">
            <input
              type="checkbox"
              checked={accepted()}
              onChange={(e) => setAccepted(e.currentTarget.checked)}
              class="mt-1"
            />
            <span class="text-sm">I understand and accept these terms</span>
          </label>

          <Show when={error()}>
            <div class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
              <p class="text-sm text-red-700 dark:text-red-300">{error()}</p>
            </div>
          </Show>

          <div class="flex gap-2 justify-end">
            <button
              class="btn btn-primary"
              disabled={!accepted() || submitting()}
              onClick={submit}
            >
              {submitting() ? 'Creating...' : 'Create Vault'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

// =====================================================================
// Patient switcher (simple dropdown)
// =====================================================================

const PatientSwitcher: Component<{
  patients: Patient[];
  activePatient: Patient | null;
  onSelect: (p: Patient) => void;
  onAddPatient: () => void;
}> = (props) => {
  const [open, setOpen] = createSignal(false);

  return (
    <div class="relative">
      <button
        class="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-gray-100 dark:bg-gray-800 hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors"
        onClick={() => setOpen(!open())}
      >
        <Show when={props.activePatient}>
          <div
            class="w-6 h-6 rounded-full flex items-center justify-center text-white text-xs font-bold"
            style={{ background: props.activePatient!.avatar_color || '#0ea5e9' }}
          >
            {props.activePatient!.full_name.charAt(0).toUpperCase()}
          </div>
          <span class="text-sm font-medium">{props.activePatient!.full_name}</span>
        </Show>
        <svg class="w-4 h-4 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
        </svg>
      </button>
      <Show when={open()}>
        <div class="absolute right-0 mt-1 w-64 card p-1 shadow-xl z-40">
          <For each={props.patients}>
            {(p) => (
              <button
                class="w-full flex items-center gap-3 px-3 py-2 rounded hover:bg-gray-50 dark:hover:bg-gray-800 text-left"
                onClick={() => {
                  props.onSelect(p);
                  setOpen(false);
                }}
              >
                <div
                  class="w-7 h-7 rounded-full flex items-center justify-center text-white text-xs font-bold flex-shrink-0"
                  style={{ background: p.avatar_color || '#0ea5e9' }}
                >
                  {p.full_name.charAt(0).toUpperCase()}
                </div>
                <div class="flex-1 min-w-0">
                  <div class="text-sm font-medium truncate">{p.full_name}</div>
                  <div class="text-xs text-gray-500 capitalize">{p.relationship}</div>
                </div>
                <Show when={p.is_primary}>
                  <span class="text-xs text-minion-500">★</span>
                </Show>
              </button>
            )}
          </For>
          <div class="border-t border-gray-200 dark:border-gray-700 my-1" />
          <button
            class="w-full flex items-center gap-2 px-3 py-2 rounded hover:bg-gray-50 dark:hover:bg-gray-800 text-sm text-minion-600"
            onClick={() => {
              props.onAddPatient();
              setOpen(false);
            }}
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
            </svg>
            Add family member
          </button>
        </div>
      </Show>
    </div>
  );
};

// =====================================================================
// Labs timeline chart (SVG, no deps)
// =====================================================================

const LabsTimelineChart: Component<{
  tests: LabTest[];
  testName: string;
}> = (props) => {
  const width = 700;
  const height = 200;
  const padding = { top: 20, right: 30, bottom: 30, left: 50 };

  const points = () => {
    const filtered = props.tests
      .filter((t) => (t.canonical_name || t.test_name) === props.testName)
      .sort((a, b) => a.collected_at.localeCompare(b.collected_at));
    return filtered;
  };

  const chart = () => {
    const pts = points();
    if (pts.length === 0) return null;

    const values = pts.map((p) => p.value);
    const refLows = pts
      .map((p) => p.reference_low)
      .filter((v): v is number => v != null);
    const refHighs = pts
      .map((p) => p.reference_high)
      .filter((v): v is number => v != null);

    const allVals = [...values, ...refLows, ...refHighs];
    const vMin = Math.min(...allVals);
    const vMax = Math.max(...allVals);
    const vRange = vMax - vMin || 1;
    const vPad = vRange * 0.1;

    const yScale = (v: number) => {
      const yMin = padding.top;
      const yMax = height - padding.bottom;
      return yMax - ((v - vMin + vPad) / (vRange + 2 * vPad)) * (yMax - yMin);
    };

    const xScale = (i: number) => {
      const xMin = padding.left;
      const xMax = width - padding.right;
      if (pts.length === 1) return (xMin + xMax) / 2;
      return xMin + (i / (pts.length - 1)) * (xMax - xMin);
    };

    const path = pts
      .map((p, i) => `${i === 0 ? 'M' : 'L'} ${xScale(i)} ${yScale(p.value)}`)
      .join(' ');

    // Reference range band (if all tests share same ref)
    const refLow = pts[0]?.reference_low ?? null;
    const refHigh = pts[0]?.reference_high ?? null;
    const bandY1 = refHigh != null ? yScale(refHigh) : null;
    const bandY2 = refLow != null ? yScale(refLow) : null;

    return {
      pts,
      path,
      yScale,
      xScale,
      bandY1,
      bandY2,
      unit: pts[0]?.unit || '',
    };
  };

  return (
    <div>
      <Show when={chart()} fallback={
        <div class="text-sm text-gray-500 text-center py-8">No data for {props.testName}</div>
      }>
        {(c) => (
          <svg width={width} height={height} class="w-full max-w-full h-auto">
            {/* Reference range band */}
            <Show when={c().bandY1 != null && c().bandY2 != null}>
              <rect
                x={padding.left}
                y={c().bandY1!}
                width={width - padding.left - padding.right}
                height={c().bandY2! - c().bandY1!}
                fill="rgb(34 197 94 / 0.15)"
              />
            </Show>
            {/* Axes */}
            <line
              x1={padding.left}
              y1={height - padding.bottom}
              x2={width - padding.right}
              y2={height - padding.bottom}
              stroke="currentColor"
              stroke-opacity="0.2"
            />
            <line
              x1={padding.left}
              y1={padding.top}
              x2={padding.left}
              y2={height - padding.bottom}
              stroke="currentColor"
              stroke-opacity="0.2"
            />
            {/* Line */}
            <path d={c().path} stroke="#0ea5e9" stroke-width="2" fill="none" />
            {/* Dots */}
            <For each={c().pts}>
              {(p, i) => (
                <g>
                  <circle
                    cx={c().xScale(i())}
                    cy={c().yScale(p.value)}
                    r="4"
                    fill={p.flag === 'H' || p.flag === 'L' ? '#ef4444' : '#0ea5e9'}
                  />
                  <title>
                    {p.collected_at.slice(0, 10)}: {p.value} {p.unit || ''}
                    {p.flag ? ` [${p.flag}]` : ''}
                  </title>
                </g>
              )}
            </For>
            {/* X-axis labels (first + last date) */}
            <Show when={c().pts.length > 0}>
              <text
                x={padding.left}
                y={height - 8}
                font-size="10"
                fill="currentColor"
                fill-opacity="0.5"
              >
                {c().pts[0].collected_at.slice(0, 10)}
              </text>
              <Show when={c().pts.length > 1}>
                <text
                  x={width - padding.right}
                  y={height - 8}
                  font-size="10"
                  fill="currentColor"
                  fill-opacity="0.5"
                  text-anchor="end"
                >
                  {c().pts[c().pts.length - 1].collected_at.slice(0, 10)}
                </text>
              </Show>
            </Show>
            {/* Unit label */}
            <text
              x={padding.left - 8}
              y={padding.top + 4}
              font-size="10"
              fill="currentColor"
              fill-opacity="0.5"
              text-anchor="end"
            >
              {c().unit}
            </text>
          </svg>
        )}
      </Show>
    </div>
  );
};

// =====================================================================
// MAIN COMPONENT
// =====================================================================

const Health: Component = () => {
  const [consent, setConsent] = createSignal<HealthConsent | null>(null);
  const [patients, setPatients] = createSignal<Patient[]>([]);
  const [activePatient, setActivePatient] = createSignal<Patient | null>(null);
  const [activeTab, setActiveTab] = createSignal<HealthTab>('dashboard');
  const [showConsent, setShowConsent] = createSignal(false);
  const [showAddPatient, setShowAddPatient] = createSignal(false);
  const [loading, setLoading] = createSignal(true);

  // Entity lists
  const [records, setRecords] = createSignal<MedicalRecord[]>([]);
  const [labTests, setLabTests] = createSignal<LabTest[]>([]);
  const [medications, setMedications] = createSignal<Medication[]>([]);
  const [conditions, setConditions] = createSignal<HealthCondition[]>([]);
  const [vitals, setVitals] = createSignal<Vital[]>([]);
  const [lifeEvents, setLifeEvents] = createSignal<LifeEvent[]>([]);
  const [symptoms, setSymptoms] = createSignal<Symptom[]>([]);
  const [familyHistory, setFamilyHistory] = createSignal<FamilyHistoryEntry[]>([]);
  const [testNames, setTestNames] = createSignal<string[]>([]);
  const [selectedTestName, setSelectedTestName] = createSignal<string>('');

  const loadConsent = async () => {
    try {
      const c = await invoke<HealthConsent | null>('health_get_consent');
      setConsent(c);
      if (!c) setShowConsent(true);
    } catch (e) {
      console.error('Failed to load consent', e);
    }
  };

  const loadPatients = async () => {
    try {
      const ps = await invoke<Patient[]>('health_list_patients');
      setPatients(ps);
      if (ps.length > 0 && !activePatient()) {
        const primary = ps.find((p) => p.is_primary) || ps[0];
        setActivePatient(primary);
      }
    } catch (e) {
      console.error('Failed to load patients', e);
    }
  };

  const loadPatientData = async () => {
    const p = activePatient();
    if (!p) return;
    try {
      const [r, l, m, c, v, le, s, fh, tn] = await Promise.all([
        invoke<MedicalRecord[]>('health_list_records', { patientId: p.id }),
        invoke<LabTest[]>('health_list_lab_tests', { patientId: p.id, category: null }),
        invoke<Medication[]>('health_list_medications', { patientId: p.id, activeOnly: false }),
        invoke<HealthCondition[]>('health_list_conditions', { patientId: p.id }),
        invoke<Vital[]>('health_list_vitals', { patientId: p.id, measurementType: null }),
        invoke<LifeEvent[]>('health_list_life_events', { patientId: p.id, category: null }),
        invoke<Symptom[]>('health_list_symptoms', { patientId: p.id }),
        invoke<FamilyHistoryEntry[]>('health_list_family_history', { patientId: p.id }),
        invoke<string[]>('health_list_test_names', { patientId: p.id }),
      ]);
      setRecords(r);
      setLabTests(l);
      setMedications(m);
      setConditions(c);
      setVitals(v);
      setLifeEvents(le);
      setSymptoms(s);
      setFamilyHistory(fh);
      setTestNames(tn);
      if (tn.length > 0 && !selectedTestName()) {
        setSelectedTestName(tn[0]);
      }
    } catch (e) {
      console.error('Failed to load patient data', e);
    }
  };

  onMount(async () => {
    await loadConsent();
    await loadPatients();
    setLoading(false);
  });

  createEffect(() => {
    if (activePatient()) {
      void loadPatientData();
    }
  });

  const onConsentAccepted = async (patient: Patient) => {
    setShowConsent(false);
    await loadConsent();
    await loadPatients();
    setActivePatient(patient);
  };

  // --- Render ---

  return (
    <div class="p-6 max-w-6xl mx-auto">
      <Show when={showConsent()}>
        <ConsentModal onAccept={onConsentAccepted} />
      </Show>

      <Show
        when={!loading() && consent() && activePatient()}
        fallback={
          <Show when={!loading() && !consent()}>
            <div class="text-center py-12">
              <h1 class="text-2xl font-bold mb-2">Health Vault</h1>
              <p class="text-gray-500">Setting up your vault...</p>
            </div>
          </Show>
        }
      >
        {/* Header */}
        <div class="flex items-center justify-between mb-6">
          <div>
            <h1 class="text-2xl font-bold">Health Vault</h1>
            <p class="text-sm text-gray-500">
              Longitudinal medical records for {activePatient()!.full_name}
            </p>
          </div>
          <PatientSwitcher
            patients={patients()}
            activePatient={activePatient()}
            onSelect={setActivePatient}
            onAddPatient={() => setShowAddPatient(true)}
          />
        </div>

        {/* Tabs */}
        <div class="flex gap-1 border-b border-gray-200 dark:border-gray-700 mb-6 overflow-x-auto">
          {(
            [
              ['dashboard', 'Dashboard'],
              ['records', 'Records'],
              ['labs', 'Labs'],
              ['medications', 'Medications'],
              ['conditions', 'Conditions'],
              ['vitals', 'Vitals'],
              ['life_events', 'Life Events'],
              ['symptoms', 'Symptoms'],
              ['family', 'Family History'],
              ['import', 'Import'],
              ['review', 'Review'],
              ['documents', 'Documents'],
            ] as const
          ).map(([tab, label]) => (
            <button
              class="px-4 py-2 text-sm font-medium border-b-2 transition-colors whitespace-nowrap"
              classList={{
                'border-minion-500 text-minion-600 dark:text-minion-400': activeTab() === tab,
                'border-transparent text-gray-500 hover:text-gray-700 dark:hover:text-gray-300':
                  activeTab() !== tab,
              }}
              onClick={() => setActiveTab(tab)}
            >
              {label}
            </button>
          ))}
        </div>

        {/* ============== DASHBOARD ============== */}
        <Show when={activeTab() === 'dashboard'}>
          <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
            <div class="card p-4">
              <div class="text-sm text-gray-500">Records</div>
              <div class="text-2xl font-bold mt-1">{records().length}</div>
            </div>
            <div class="card p-4">
              <div class="text-sm text-gray-500">Lab tests</div>
              <div class="text-2xl font-bold mt-1">{labTests().length}</div>
            </div>
            <div class="card p-4">
              <div class="text-sm text-gray-500">Active medications</div>
              <div class="text-2xl font-bold mt-1">
                {medications().filter((m) => !m.end_date).length}
              </div>
            </div>
            <div class="card p-4">
              <div class="text-sm text-gray-500">Active symptoms</div>
              <div class="text-2xl font-bold mt-1">
                {symptoms().filter((s) => !s.resolved_at).length}
              </div>
            </div>
          </div>
          <div class="card p-6 mt-4">
            <h2 class="text-sm font-semibold mb-3 text-gray-700 dark:text-gray-200">
              Recent activity
            </h2>
            <Show
              when={records().length > 0 || labTests().length > 0}
              fallback={
                <p class="text-sm text-gray-500 text-center py-6">
                  Nothing logged yet. Start by adding a record or lab result.
                </p>
              }
            >
              <div class="space-y-2">
                <For each={records().slice(0, 5)}>
                  {(r) => (
                    <div class="flex justify-between items-center py-2 border-b border-gray-100 dark:border-gray-800 last:border-0">
                      <div>
                        <div class="text-sm font-medium">{r.title}</div>
                        <div class="text-xs text-gray-500 capitalize">{r.record_type}</div>
                      </div>
                      <div class="text-xs text-gray-500">{r.date.slice(0, 10)}</div>
                    </div>
                  )}
                </For>
              </div>
            </Show>
          </div>
        </Show>

        {/* ============== RECORDS ============== */}
        <Show when={activeTab() === 'records'}>
          <RecordsTab
            patientId={activePatient()!.id}
            records={records()}
            onReload={loadPatientData}
          />
        </Show>

        {/* ============== LABS ============== */}
        <Show when={activeTab() === 'labs'}>
          <LabsTab
            patientId={activePatient()!.id}
            tests={labTests()}
            testNames={testNames()}
            selectedTestName={selectedTestName()}
            onSelectTest={setSelectedTestName}
            onReload={loadPatientData}
          />
        </Show>

        {/* ============== MEDICATIONS ============== */}
        <Show when={activeTab() === 'medications'}>
          <MedicationsTab
            patientId={activePatient()!.id}
            medications={medications()}
            onReload={loadPatientData}
          />
        </Show>

        {/* ============== CONDITIONS ============== */}
        <Show when={activeTab() === 'conditions'}>
          <ConditionsTab
            patientId={activePatient()!.id}
            conditions={conditions()}
            onReload={loadPatientData}
          />
        </Show>

        {/* ============== VITALS ============== */}
        <Show when={activeTab() === 'vitals'}>
          <VitalsTab
            patientId={activePatient()!.id}
            vitals={vitals()}
            onReload={loadPatientData}
          />
        </Show>

        {/* ============== LIFE EVENTS ============== */}
        <Show when={activeTab() === 'life_events'}>
          <LifeEventsTab
            patientId={activePatient()!.id}
            events={lifeEvents()}
            onReload={loadPatientData}
          />
        </Show>

        {/* ============== SYMPTOMS ============== */}
        <Show when={activeTab() === 'symptoms'}>
          <SymptomsTab
            patientId={activePatient()!.id}
            symptoms={symptoms()}
            onReload={loadPatientData}
          />
        </Show>

        {/* ============== FAMILY HISTORY ============== */}
        <Show when={activeTab() === 'family'}>
          <FamilyHistoryTab
            patientId={activePatient()!.id}
            entries={familyHistory()}
            onReload={loadPatientData}
          />
        </Show>

        {/* ============== IMPORT ============== */}
        <Show when={activeTab() === 'import'}>
          <ImportTab
            activePatient={activePatient()!}
            onGoToReview={() => setActiveTab('review')}
          />
        </Show>

        {/* ============== REVIEW ============== */}
        <Show when={activeTab() === 'review'}>
          <ReviewTab activePatient={activePatient()!} />
        </Show>

        {/* ============== DOCUMENTS ============== */}
        <Show when={activeTab() === 'documents'}>
          <DocumentsTab activePatient={activePatient()!} />
        </Show>

        {/* Add patient modal */}
        <Show when={showAddPatient()}>
          <AddPatientModal
            onClose={() => setShowAddPatient(false)}
            onCreated={async (p) => {
              setShowAddPatient(false);
              await loadPatients();
              setActivePatient(p);
            }}
          />
        </Show>
      </Show>

      {/* Floating labs chart export at bottom of page for reference */}
      <Show when={activeTab() === 'labs' && testNames().length > 0 && selectedTestName()}>
        <div class="card p-4 mt-4">
          <div class="flex items-center justify-between mb-3">
            <h3 class="text-sm font-semibold">Trend: {selectedTestName()}</h3>
          </div>
          <LabsTimelineChart tests={labTests()} testName={selectedTestName()} />
        </div>
      </Show>
    </div>
  );
};

// =====================================================================
// TAB COMPONENTS
// =====================================================================

const RecordsTab: Component<{
  patientId: string;
  records: MedicalRecord[];
  onReload: () => void;
}> = (props) => {
  const [showForm, setShowForm] = createSignal(false);
  const [title, setTitle] = createSignal('');
  const [recordType, setRecordType] = createSignal('visit');
  const [date, setDate] = createSignal(new Date().toISOString().slice(0, 10));
  const [description, setDescription] = createSignal('');

  const save = async () => {
    if (!title().trim()) return;
    try {
      await invoke('health_create_record', {
        request: {
          patient_id: props.patientId,
          record_type: recordType(),
          title: title().trim(),
          description: description() || null,
          date: date(),
          tags: null,
        },
      });
      setTitle('');
      setDescription('');
      setShowForm(false);
      props.onReload();
    } catch (e) {
      alert(String(e));
    }
  };

  const remove = async (id: string) => {
    if (!confirm('Delete this record?')) return;
    await invoke('health_delete_record', { recordId: id });
    props.onReload();
  };

  return (
    <div>
      <div class="flex justify-between items-center mb-4">
        <h2 class="text-lg font-semibold">Medical Records</h2>
        <button class="btn btn-primary text-sm" onClick={() => setShowForm(!showForm())}>
          {showForm() ? 'Cancel' : '+ Add Record'}
        </button>
      </div>
      <Show when={showForm()}>
        <div class="card p-4 mb-4 space-y-3">
          <div class="grid grid-cols-2 gap-3">
            <div>
              <label class="block text-xs font-medium mb-1">Type</label>
              <select
                class="input w-full"
                value={recordType()}
                onChange={(e) => setRecordType(e.currentTarget.value)}
              >
                <option value="visit">Visit</option>
                <option value="diagnosis">Diagnosis</option>
                <option value="procedure">Procedure</option>
                <option value="prescription">Prescription</option>
                <option value="imaging">Imaging</option>
                <option value="vaccination">Vaccination</option>
              </select>
            </div>
            <div>
              <label class="block text-xs font-medium mb-1">Date</label>
              <input
                type="date"
                class="input w-full"
                value={date()}
                onInput={(e) => setDate(e.currentTarget.value)}
              />
            </div>
          </div>
          <div>
            <label class="block text-xs font-medium mb-1">Title *</label>
            <input
              type="text"
              class="input w-full"
              placeholder="e.g., Annual checkup with Dr. Rao"
              value={title()}
              onInput={(e) => setTitle(e.currentTarget.value)}
            />
          </div>
          <div>
            <label class="block text-xs font-medium mb-1">Description</label>
            <textarea
              class="input w-full"
              rows="3"
              placeholder="Notes, observations, etc."
              value={description()}
              onInput={(e) => setDescription(e.currentTarget.value)}
            />
          </div>
          <button class="btn btn-primary w-full" onClick={save}>
            Save Record
          </button>
        </div>
      </Show>
      <Show
        when={props.records.length > 0}
        fallback={
          <div class="card p-8 text-center text-gray-500">
            No records yet. Click "+ Add Record" to log one.
          </div>
        }
      >
        <div class="space-y-2">
          <For each={props.records}>
            {(r) => (
              <div class="card p-4 flex justify-between items-start">
                <div>
                  <div class="flex items-center gap-2 mb-1">
                    <span class="text-xs uppercase px-2 py-0.5 bg-gray-100 dark:bg-gray-800 rounded">
                      {r.record_type}
                    </span>
                    <span class="text-xs text-gray-500">{r.date.slice(0, 10)}</span>
                  </div>
                  <div class="font-medium">{r.title}</div>
                  <Show when={r.description}>
                    <div class="text-sm text-gray-600 dark:text-gray-400 mt-1">
                      {r.description}
                    </div>
                  </Show>
                </div>
                <button
                  class="text-xs text-red-500 hover:underline"
                  onClick={() => remove(r.id)}
                >
                  Delete
                </button>
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
};

const LabsTab: Component<{
  patientId: string;
  tests: LabTest[];
  testNames: string[];
  selectedTestName: string;
  onSelectTest: (name: string) => void;
  onReload: () => void;
}> = (props) => {
  const [showForm, setShowForm] = createSignal(false);
  const [testName, setTestName] = createSignal('');
  const [category, setCategory] = createSignal('metabolic');
  const [value, setValue] = createSignal('');
  const [unit, setUnit] = createSignal('');
  const [refLow, setRefLow] = createSignal('');
  const [refHigh, setRefHigh] = createSignal('');
  const [collectedAt, setCollectedAt] = createSignal(new Date().toISOString().slice(0, 10));

  const save = async () => {
    if (!testName().trim() || !value().trim()) return;
    const num = parseFloat(value());
    if (Number.isNaN(num)) {
      alert('Value must be a number');
      return;
    }
    try {
      await invoke('health_create_lab_test', {
        request: {
          patient_id: props.patientId,
          test_name: testName().trim(),
          canonical_name: testName().trim(),
          test_category: category(),
          value: num,
          unit: unit() || null,
          reference_low: refLow() ? parseFloat(refLow()) : null,
          reference_high: refHigh() ? parseFloat(refHigh()) : null,
          reference_text: null,
          flag: null,
          collected_at: collectedAt(),
        },
      });
      setTestName('');
      setValue('');
      setUnit('');
      setRefLow('');
      setRefHigh('');
      setShowForm(false);
      props.onReload();
    } catch (e) {
      alert(String(e));
    }
  };

  const remove = async (id: string) => {
    if (!confirm('Delete this lab test?')) return;
    await invoke('health_delete_lab_test', { testId: id });
    props.onReload();
  };

  return (
    <div>
      <div class="flex justify-between items-center mb-4">
        <h2 class="text-lg font-semibold">Lab Tests</h2>
        <button class="btn btn-primary text-sm" onClick={() => setShowForm(!showForm())}>
          {showForm() ? 'Cancel' : '+ Add Lab Result'}
        </button>
      </div>
      <Show when={showForm()}>
        <div class="card p-4 mb-4 space-y-3">
          <div class="grid grid-cols-2 gap-3">
            <div>
              <label class="block text-xs font-medium mb-1">Test name *</label>
              <input
                type="text"
                class="input w-full"
                placeholder="e.g., HbA1c, LDL Cholesterol"
                value={testName()}
                onInput={(e) => setTestName(e.currentTarget.value)}
              />
            </div>
            <div>
              <label class="block text-xs font-medium mb-1">Category</label>
              <select
                class="input w-full"
                value={category()}
                onChange={(e) => setCategory(e.currentTarget.value)}
              >
                <option value="metabolic">Metabolic</option>
                <option value="lipid">Lipid</option>
                <option value="cbc">CBC</option>
                <option value="thyroid">Thyroid</option>
                <option value="liver">Liver</option>
                <option value="kidney">Kidney</option>
                <option value="hormonal">Hormonal</option>
                <option value="other">Other</option>
              </select>
            </div>
          </div>
          <div class="grid grid-cols-3 gap-3">
            <div>
              <label class="block text-xs font-medium mb-1">Value *</label>
              <input
                type="number"
                step="any"
                class="input w-full"
                value={value()}
                onInput={(e) => setValue(e.currentTarget.value)}
              />
            </div>
            <div>
              <label class="block text-xs font-medium mb-1">Unit</label>
              <input
                type="text"
                class="input w-full"
                placeholder="mg/dL, %"
                value={unit()}
                onInput={(e) => setUnit(e.currentTarget.value)}
              />
            </div>
            <div>
              <label class="block text-xs font-medium mb-1">Collected</label>
              <input
                type="date"
                class="input w-full"
                value={collectedAt()}
                onInput={(e) => setCollectedAt(e.currentTarget.value)}
              />
            </div>
          </div>
          <div class="grid grid-cols-2 gap-3">
            <div>
              <label class="block text-xs font-medium mb-1">Ref. low</label>
              <input
                type="number"
                step="any"
                class="input w-full"
                value={refLow()}
                onInput={(e) => setRefLow(e.currentTarget.value)}
              />
            </div>
            <div>
              <label class="block text-xs font-medium mb-1">Ref. high</label>
              <input
                type="number"
                step="any"
                class="input w-full"
                value={refHigh()}
                onInput={(e) => setRefHigh(e.currentTarget.value)}
              />
            </div>
          </div>
          <button class="btn btn-primary w-full" onClick={save}>
            Save Lab Result
          </button>
        </div>
      </Show>

      <Show when={props.testNames.length > 0}>
        <div class="flex gap-2 mb-4 flex-wrap">
          <span class="text-xs text-gray-500 self-center">Trend:</span>
          <For each={props.testNames}>
            {(name) => (
              <button
                class="text-xs px-3 py-1 rounded-full border transition-colors"
                classList={{
                  'bg-minion-500 text-white border-minion-500':
                    props.selectedTestName === name,
                  'border-gray-300 dark:border-gray-600 hover:border-minion-400':
                    props.selectedTestName !== name,
                }}
                onClick={() => props.onSelectTest(name)}
              >
                {name}
              </button>
            )}
          </For>
        </div>
      </Show>

      <Show
        when={props.tests.length > 0}
        fallback={
          <div class="card p-8 text-center text-gray-500">
            No lab tests yet.
          </div>
        }
      >
        <div class="space-y-2">
          <For each={props.tests}>
            {(t) => (
              <div class="card p-3 flex justify-between items-center">
                <div class="flex-1">
                  <div class="flex items-center gap-2">
                    <span class="font-medium">{t.test_name}</span>
                    <Show when={t.test_category}>
                      <span class="text-xs px-1.5 py-0.5 bg-gray-100 dark:bg-gray-800 rounded capitalize">
                        {t.test_category}
                      </span>
                    </Show>
                    <Show when={t.flag}>
                      <span
                        class="text-xs px-1.5 py-0.5 rounded font-bold"
                        classList={{
                          'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400':
                            t.flag === 'H' || t.flag === 'HH',
                          'bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400':
                            t.flag === 'L' || t.flag === 'LL',
                        }}
                      >
                        {t.flag}
                      </span>
                    </Show>
                  </div>
                  <div class="text-xs text-gray-500">
                    {t.collected_at.slice(0, 10)}
                    {t.reference_low != null && t.reference_high != null && (
                      <>
                        {' · '}
                        Ref: {t.reference_low}–{t.reference_high} {t.unit}
                      </>
                    )}
                  </div>
                </div>
                <div class="text-right mr-4">
                  <div class="text-lg font-bold">
                    {t.value} <span class="text-xs text-gray-500">{t.unit}</span>
                  </div>
                </div>
                <button
                  class="text-xs text-red-500 hover:underline"
                  onClick={() => remove(t.id)}
                >
                  Delete
                </button>
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
};

const MedicationsTab: Component<{
  patientId: string;
  medications: Medication[];
  onReload: () => void;
}> = (props) => {
  const [showForm, setShowForm] = createSignal(false);
  const [name, setName] = createSignal('');
  const [dose, setDose] = createSignal('');
  const [frequency, setFrequency] = createSignal('');
  const [startDate, setStartDate] = createSignal(new Date().toISOString().slice(0, 10));
  const [indication, setIndication] = createSignal('');

  const save = async () => {
    if (!name().trim()) return;
    try {
      await invoke('health_create_medication', {
        request: {
          patient_id: props.patientId,
          name: name().trim(),
          generic_name: null,
          dose: dose() || null,
          frequency: frequency() || null,
          route: null,
          start_date: startDate() || null,
          end_date: null,
          indication: indication() || null,
          notes: null,
        },
      });
      setName('');
      setDose('');
      setFrequency('');
      setIndication('');
      setShowForm(false);
      props.onReload();
    } catch (e) {
      alert(String(e));
    }
  };

  const remove = async (id: string) => {
    if (!confirm('Delete this medication?')) return;
    await invoke('health_delete_medication', { medicationId: id });
    props.onReload();
  };

  const active = () => props.medications.filter((m) => !m.end_date);
  const past = () => props.medications.filter((m) => m.end_date);

  return (
    <div>
      <div class="flex justify-between items-center mb-4">
        <h2 class="text-lg font-semibold">Medications</h2>
        <button class="btn btn-primary text-sm" onClick={() => setShowForm(!showForm())}>
          {showForm() ? 'Cancel' : '+ Add Medication'}
        </button>
      </div>
      <Show when={showForm()}>
        <div class="card p-4 mb-4 space-y-3">
          <div>
            <label class="block text-xs font-medium mb-1">Name *</label>
            <input
              type="text"
              class="input w-full"
              placeholder="Metformin"
              value={name()}
              onInput={(e) => setName(e.currentTarget.value)}
            />
          </div>
          <div class="grid grid-cols-2 gap-3">
            <div>
              <label class="block text-xs font-medium mb-1">Dose</label>
              <input
                type="text"
                class="input w-full"
                placeholder="500mg"
                value={dose()}
                onInput={(e) => setDose(e.currentTarget.value)}
              />
            </div>
            <div>
              <label class="block text-xs font-medium mb-1">Frequency</label>
              <input
                type="text"
                class="input w-full"
                placeholder="Twice daily"
                value={frequency()}
                onInput={(e) => setFrequency(e.currentTarget.value)}
              />
            </div>
          </div>
          <div class="grid grid-cols-2 gap-3">
            <div>
              <label class="block text-xs font-medium mb-1">Start date</label>
              <input
                type="date"
                class="input w-full"
                value={startDate()}
                onInput={(e) => setStartDate(e.currentTarget.value)}
              />
            </div>
            <div>
              <label class="block text-xs font-medium mb-1">Indication</label>
              <input
                type="text"
                class="input w-full"
                placeholder="Type 2 diabetes"
                value={indication()}
                onInput={(e) => setIndication(e.currentTarget.value)}
              />
            </div>
          </div>
          <button class="btn btn-primary w-full" onClick={save}>
            Save Medication
          </button>
        </div>
      </Show>

      <Show when={active().length > 0}>
        <h3 class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">Active</h3>
        <div class="space-y-2 mb-4">
          <For each={active()}>
            {(m) => (
              <div class="card p-3 flex justify-between items-center">
                <div>
                  <div class="font-medium">
                    {m.name}
                    <Show when={m.dose}>
                      <span class="text-sm text-gray-500 ml-2">{m.dose}</span>
                    </Show>
                  </div>
                  <div class="text-xs text-gray-500">
                    {m.frequency || ''}
                    {m.indication && <span> · {m.indication}</span>}
                    {m.start_date && <span> · Started {m.start_date.slice(0, 10)}</span>}
                  </div>
                </div>
                <button
                  class="text-xs text-red-500 hover:underline"
                  onClick={() => remove(m.id)}
                >
                  Delete
                </button>
              </div>
            )}
          </For>
        </div>
      </Show>

      <Show when={past().length > 0}>
        <h3 class="text-sm font-semibold text-gray-700 dark:text-gray-300 mb-2">Past</h3>
        <div class="space-y-2">
          <For each={past()}>
            {(m) => (
              <div class="card p-3 flex justify-between items-center opacity-70">
                <div>
                  <div class="font-medium">{m.name}</div>
                  <div class="text-xs text-gray-500">
                    {m.start_date?.slice(0, 10)} → {m.end_date?.slice(0, 10)}
                  </div>
                </div>
                <button
                  class="text-xs text-red-500 hover:underline"
                  onClick={() => remove(m.id)}
                >
                  Delete
                </button>
              </div>
            )}
          </For>
        </div>
      </Show>

      <Show when={props.medications.length === 0}>
        <div class="card p-8 text-center text-gray-500">No medications logged.</div>
      </Show>
    </div>
  );
};

const ConditionsTab: Component<{
  patientId: string;
  conditions: HealthCondition[];
  onReload: () => void;
}> = (props) => {
  const [showForm, setShowForm] = createSignal(false);
  const [name, setName] = createSignal('');
  const [type, setType] = createSignal('chronic');
  const [severity, setSeverity] = createSignal('moderate');
  const [diagnosedAt, setDiagnosedAt] = createSignal('');

  const save = async () => {
    if (!name().trim()) return;
    try {
      await invoke('health_create_condition', {
        request: {
          patient_id: props.patientId,
          name: name().trim(),
          condition_type: type(),
          severity: severity(),
          diagnosed_at: diagnosedAt() || null,
          resolved_at: null,
          notes: null,
        },
      });
      setName('');
      setDiagnosedAt('');
      setShowForm(false);
      props.onReload();
    } catch (e) {
      alert(String(e));
    }
  };

  const remove = async (id: string) => {
    if (!confirm('Delete this condition?')) return;
    await invoke('health_delete_condition', { conditionId: id });
    props.onReload();
  };

  return (
    <div>
      <div class="flex justify-between items-center mb-4">
        <h2 class="text-lg font-semibold">Conditions & Allergies</h2>
        <button class="btn btn-primary text-sm" onClick={() => setShowForm(!showForm())}>
          {showForm() ? 'Cancel' : '+ Add Condition'}
        </button>
      </div>
      <Show when={showForm()}>
        <div class="card p-4 mb-4 space-y-3">
          <div>
            <label class="block text-xs font-medium mb-1">Name *</label>
            <input
              type="text"
              class="input w-full"
              placeholder="Hypertension, Peanut allergy, ..."
              value={name()}
              onInput={(e) => setName(e.currentTarget.value)}
            />
          </div>
          <div class="grid grid-cols-2 gap-3">
            <div>
              <label class="block text-xs font-medium mb-1">Type</label>
              <select
                class="input w-full"
                value={type()}
                onChange={(e) => setType(e.currentTarget.value)}
              >
                <option value="chronic">Chronic</option>
                <option value="allergy">Allergy</option>
                <option value="surgery">Surgery</option>
                <option value="past">Past</option>
              </select>
            </div>
            <div>
              <label class="block text-xs font-medium mb-1">Severity</label>
              <select
                class="input w-full"
                value={severity()}
                onChange={(e) => setSeverity(e.currentTarget.value)}
              >
                <option value="mild">Mild</option>
                <option value="moderate">Moderate</option>
                <option value="severe">Severe</option>
              </select>
            </div>
          </div>
          <div>
            <label class="block text-xs font-medium mb-1">Diagnosed at</label>
            <input
              type="date"
              class="input w-full"
              value={diagnosedAt()}
              onInput={(e) => setDiagnosedAt(e.currentTarget.value)}
            />
          </div>
          <button class="btn btn-primary w-full" onClick={save}>
            Save Condition
          </button>
        </div>
      </Show>
      <Show
        when={props.conditions.length > 0}
        fallback={
          <div class="card p-8 text-center text-gray-500">No conditions logged.</div>
        }
      >
        <div class="space-y-2">
          <For each={props.conditions}>
            {(c) => (
              <div class="card p-3 flex justify-between items-center">
                <div>
                  <div class="flex items-center gap-2">
                    <span class="font-medium">{c.name}</span>
                    <Show when={c.condition_type}>
                      <span class="text-xs px-1.5 py-0.5 bg-gray-100 dark:bg-gray-800 rounded capitalize">
                        {c.condition_type}
                      </span>
                    </Show>
                    <Show when={c.severity}>
                      <span class="text-xs px-1.5 py-0.5 bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300 rounded capitalize">
                        {c.severity}
                      </span>
                    </Show>
                  </div>
                  <Show when={c.diagnosed_at}>
                    <div class="text-xs text-gray-500">Diagnosed: {c.diagnosed_at?.slice(0, 10)}</div>
                  </Show>
                </div>
                <button
                  class="text-xs text-red-500 hover:underline"
                  onClick={() => remove(c.id)}
                >
                  Delete
                </button>
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
};

const VitalsTab: Component<{
  patientId: string;
  vitals: Vital[];
  onReload: () => void;
}> = (props) => {
  const [showForm, setShowForm] = createSignal(false);
  const [type, setType] = createSignal('bp_systolic');
  const [value, setValue] = createSignal('');
  const [unit, setUnit] = createSignal('mmHg');
  const [measuredAt, setMeasuredAt] = createSignal(new Date().toISOString().slice(0, 16));
  const [context, setContext] = createSignal('');

  const save = async () => {
    if (!value().trim()) return;
    try {
      await invoke('health_create_vital', {
        request: {
          patient_id: props.patientId,
          measurement_type: type(),
          value: parseFloat(value()),
          unit: unit() || null,
          measured_at: measuredAt(),
          context: context() || null,
          notes: null,
        },
      });
      setValue('');
      setContext('');
      setShowForm(false);
      props.onReload();
    } catch (e) {
      alert(String(e));
    }
  };

  const remove = async (id: string) => {
    if (!confirm('Delete this reading?')) return;
    await invoke('health_delete_vital', { vitalId: id });
    props.onReload();
  };

  return (
    <div>
      <div class="flex justify-between items-center mb-4">
        <h2 class="text-lg font-semibold">Vitals</h2>
        <button class="btn btn-primary text-sm" onClick={() => setShowForm(!showForm())}>
          {showForm() ? 'Cancel' : '+ Add Reading'}
        </button>
      </div>
      <Show when={showForm()}>
        <div class="card p-4 mb-4 space-y-3">
          <div class="grid grid-cols-2 gap-3">
            <div>
              <label class="block text-xs font-medium mb-1">Type</label>
              <select
                class="input w-full"
                value={type()}
                onChange={(e) => setType(e.currentTarget.value)}
              >
                <option value="bp_systolic">BP Systolic</option>
                <option value="bp_diastolic">BP Diastolic</option>
                <option value="glucose">Glucose</option>
                <option value="temperature">Temperature</option>
                <option value="spo2">SpO2</option>
                <option value="pulse">Pulse</option>
              </select>
            </div>
            <div>
              <label class="block text-xs font-medium mb-1">When</label>
              <input
                type="datetime-local"
                class="input w-full"
                value={measuredAt()}
                onInput={(e) => setMeasuredAt(e.currentTarget.value)}
              />
            </div>
          </div>
          <div class="grid grid-cols-2 gap-3">
            <div>
              <label class="block text-xs font-medium mb-1">Value *</label>
              <input
                type="number"
                step="any"
                class="input w-full"
                value={value()}
                onInput={(e) => setValue(e.currentTarget.value)}
              />
            </div>
            <div>
              <label class="block text-xs font-medium mb-1">Unit</label>
              <input
                type="text"
                class="input w-full"
                value={unit()}
                onInput={(e) => setUnit(e.currentTarget.value)}
              />
            </div>
          </div>
          <div>
            <label class="block text-xs font-medium mb-1">Context</label>
            <input
              type="text"
              class="input w-full"
              placeholder="fasting, after meal, morning, ..."
              value={context()}
              onInput={(e) => setContext(e.currentTarget.value)}
            />
          </div>
          <button class="btn btn-primary w-full" onClick={save}>
            Save Reading
          </button>
        </div>
      </Show>
      <Show
        when={props.vitals.length > 0}
        fallback={<div class="card p-8 text-center text-gray-500">No vitals logged.</div>}
      >
        <div class="space-y-2">
          <For each={props.vitals}>
            {(v) => (
              <div class="card p-3 flex justify-between items-center">
                <div>
                  <div class="font-medium capitalize">{v.measurement_type.replace(/_/g, ' ')}</div>
                  <div class="text-xs text-gray-500">
                    {v.measured_at.slice(0, 16).replace('T', ' ')}
                    {v.context && <span> · {v.context}</span>}
                  </div>
                </div>
                <div class="text-right mr-4">
                  <div class="text-lg font-bold">
                    {v.value} <span class="text-xs text-gray-500">{v.unit}</span>
                  </div>
                </div>
                <button
                  class="text-xs text-red-500 hover:underline"
                  onClick={() => remove(v.id)}
                >
                  Delete
                </button>
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
};

const LifeEventsTab: Component<{
  patientId: string;
  events: LifeEvent[];
  onReload: () => void;
}> = (props) => {
  const [showForm, setShowForm] = createSignal(false);
  const [category, setCategory] = createSignal('work');
  const [title, setTitle] = createSignal('');
  const [description, setDescription] = createSignal('');
  const [intensity, setIntensity] = createSignal(5);
  const [startedAt, setStartedAt] = createSignal(new Date().toISOString().slice(0, 10));
  const [endedAt, setEndedAt] = createSignal('');

  const save = async () => {
    if (!title().trim()) return;
    try {
      await invoke('health_create_life_event', {
        request: {
          patient_id: props.patientId,
          category: category(),
          subcategory: null,
          title: title().trim(),
          description: description() || null,
          intensity: intensity(),
          started_at: startedAt(),
          ended_at: endedAt() || null,
          tags: null,
        },
      });
      setTitle('');
      setDescription('');
      setShowForm(false);
      props.onReload();
    } catch (e) {
      alert(String(e));
    }
  };

  const remove = async (id: string) => {
    if (!confirm('Delete this event?')) return;
    await invoke('health_delete_life_event', { eventId: id });
    props.onReload();
  };

  const catMeta = (cat: string) =>
    LIFE_EVENT_CATEGORIES.find((c) => c.value === cat) || { emoji: '📌', label: cat };

  return (
    <div>
      <div class="flex justify-between items-center mb-4">
        <h2 class="text-lg font-semibold">Life Events</h2>
        <button class="btn btn-primary text-sm" onClick={() => setShowForm(!showForm())}>
          {showForm() ? 'Cancel' : '+ Add Event'}
        </button>
      </div>
      <p class="text-xs text-gray-500 mb-4">
        These correlate with your lab values and symptoms during AI analysis.
      </p>
      <Show when={showForm()}>
        <div class="card p-4 mb-4 space-y-3">
          <div>
            <label class="block text-xs font-medium mb-1">Category</label>
            <div class="grid grid-cols-3 gap-1">
              <For each={LIFE_EVENT_CATEGORIES}>
                {(cat) => (
                  <button
                    class="text-xs px-2 py-1.5 rounded border flex items-center gap-1 transition-colors"
                    classList={{
                      'bg-minion-500 text-white border-minion-500': category() === cat.value,
                      'border-gray-300 dark:border-gray-600 hover:border-minion-400':
                        category() !== cat.value,
                    }}
                    onClick={() => setCategory(cat.value)}
                  >
                    <span>{cat.emoji}</span>
                    <span>{cat.label}</span>
                  </button>
                )}
              </For>
            </div>
          </div>
          <div>
            <label class="block text-xs font-medium mb-1">Title *</label>
            <input
              type="text"
              class="input w-full"
              placeholder="e.g., Started Shambhavi Mahamudra daily practice"
              value={title()}
              onInput={(e) => setTitle(e.currentTarget.value)}
            />
          </div>
          <div>
            <label class="block text-xs font-medium mb-1">Description</label>
            <textarea
              class="input w-full"
              rows="2"
              value={description()}
              onInput={(e) => setDescription(e.currentTarget.value)}
            />
          </div>
          <div class="grid grid-cols-3 gap-3">
            <div>
              <label class="block text-xs font-medium mb-1">Started</label>
              <input
                type="date"
                class="input w-full"
                value={startedAt()}
                onInput={(e) => setStartedAt(e.currentTarget.value)}
              />
            </div>
            <div>
              <label class="block text-xs font-medium mb-1">Ended (optional)</label>
              <input
                type="date"
                class="input w-full"
                value={endedAt()}
                onInput={(e) => setEndedAt(e.currentTarget.value)}
              />
            </div>
            <div>
              <label class="block text-xs font-medium mb-1">
                Intensity: {intensity()}
              </label>
              <input
                type="range"
                min="1"
                max="10"
                class="w-full"
                value={intensity()}
                onInput={(e) => setIntensity(parseInt(e.currentTarget.value))}
              />
            </div>
          </div>
          <button class="btn btn-primary w-full" onClick={save}>
            Save Event
          </button>
        </div>
      </Show>
      <Show
        when={props.events.length > 0}
        fallback={<div class="card p-8 text-center text-gray-500">No life events logged.</div>}
      >
        <div class="space-y-2">
          <For each={props.events}>
            {(e) => {
              const meta = catMeta(e.category);
              return (
                <div class="card p-3 flex justify-between items-start">
                  <div class="flex-1">
                    <div class="flex items-center gap-2">
                      <span class="text-lg">{meta.emoji}</span>
                      <span class="font-medium">{e.title}</span>
                      <span class="text-xs px-1.5 py-0.5 bg-gray-100 dark:bg-gray-800 rounded capitalize">
                        {meta.label}
                      </span>
                      <Show when={e.intensity}>
                        <span class="text-xs text-amber-600">
                          intensity {e.intensity}/10
                        </span>
                      </Show>
                    </div>
                    <div class="text-xs text-gray-500 mt-0.5">
                      {e.started_at.slice(0, 10)}
                      {e.ended_at && <span> → {e.ended_at.slice(0, 10)}</span>}
                      {!e.ended_at && <span> → ongoing</span>}
                    </div>
                    <Show when={e.description}>
                      <div class="text-sm text-gray-600 dark:text-gray-400 mt-1">
                        {e.description}
                      </div>
                    </Show>
                  </div>
                  <button
                    class="text-xs text-red-500 hover:underline"
                    onClick={() => remove(e.id)}
                  >
                    Delete
                  </button>
                </div>
              );
            }}
          </For>
        </div>
      </Show>
    </div>
  );
};

const SymptomsTab: Component<{
  patientId: string;
  symptoms: Symptom[];
  onReload: () => void;
}> = (props) => {
  const [showForm, setShowForm] = createSignal(false);
  const [description, setDescription] = createSignal('');
  const [severity, setSeverity] = createSignal(5);
  const [firstNoticed, setFirstNoticed] = createSignal(new Date().toISOString().slice(0, 10));
  const [frequency, setFrequency] = createSignal('intermittent');

  const save = async () => {
    if (!description().trim()) return;
    try {
      await invoke('health_create_symptom', {
        request: {
          patient_id: props.patientId,
          description: description().trim(),
          severity: severity(),
          first_noticed: firstNoticed(),
          frequency: frequency(),
          notes: null,
        },
      });
      setDescription('');
      setShowForm(false);
      props.onReload();
    } catch (e) {
      alert(String(e));
    }
  };

  const remove = async (id: string) => {
    if (!confirm('Delete this symptom?')) return;
    await invoke('health_delete_symptom', { symptomId: id });
    props.onReload();
  };

  const resolve = async (id: string) => {
    const today = new Date().toISOString().slice(0, 10);
    await invoke('health_resolve_symptom', { symptomId: id, resolvedAt: today });
    props.onReload();
  };

  return (
    <div>
      <div class="flex justify-between items-center mb-4">
        <h2 class="text-lg font-semibold">Symptoms</h2>
        <button class="btn btn-primary text-sm" onClick={() => setShowForm(!showForm())}>
          {showForm() ? 'Cancel' : '+ Log Symptom'}
        </button>
      </div>
      <p class="text-xs text-gray-500 mb-4">
        Describe symptoms in your own words. MINION will correlate them with labs and life
        events during AI analysis.
      </p>
      <Show when={showForm()}>
        <div class="card p-4 mb-4 space-y-3">
          <div>
            <label class="block text-xs font-medium mb-1">Describe the symptom *</label>
            <textarea
              class="input w-full"
              rows="3"
              placeholder="e.g., dull pain on left side of chest after long walks, usually after climbing stairs"
              value={description()}
              onInput={(e) => setDescription(e.currentTarget.value)}
            />
          </div>
          <div class="grid grid-cols-3 gap-3">
            <div>
              <label class="block text-xs font-medium mb-1">First noticed</label>
              <input
                type="date"
                class="input w-full"
                value={firstNoticed()}
                onInput={(e) => setFirstNoticed(e.currentTarget.value)}
              />
            </div>
            <div>
              <label class="block text-xs font-medium mb-1">Severity: {severity()}/10</label>
              <input
                type="range"
                min="1"
                max="10"
                class="w-full"
                value={severity()}
                onInput={(e) => setSeverity(parseInt(e.currentTarget.value))}
              />
            </div>
            <div>
              <label class="block text-xs font-medium mb-1">Frequency</label>
              <select
                class="input w-full"
                value={frequency()}
                onChange={(e) => setFrequency(e.currentTarget.value)}
              >
                <option value="constant">Constant</option>
                <option value="daily">Daily</option>
                <option value="weekly">Weekly</option>
                <option value="intermittent">Intermittent</option>
              </select>
            </div>
          </div>
          <button class="btn btn-primary w-full" onClick={save}>
            Save Symptom
          </button>
        </div>
      </Show>
      <Show
        when={props.symptoms.length > 0}
        fallback={<div class="card p-8 text-center text-gray-500">No symptoms logged.</div>}
      >
        <div class="space-y-2">
          <For each={props.symptoms}>
            {(s) => (
              <div
                class="card p-3"
                classList={{ 'opacity-60': !!s.resolved_at }}
              >
                <div class="flex justify-between items-start">
                  <div class="flex-1">
                    <div class="text-sm">{s.description}</div>
                    <div class="text-xs text-gray-500 mt-1">
                      First noticed {s.first_noticed.slice(0, 10)}
                      {s.severity && <span> · severity {s.severity}/10</span>}
                      {s.frequency && <span> · {s.frequency}</span>}
                      {s.resolved_at && (
                        <span> · resolved {s.resolved_at.slice(0, 10)}</span>
                      )}
                    </div>
                  </div>
                  <div class="flex gap-2 ml-2">
                    <Show when={!s.resolved_at}>
                      <button
                        class="text-xs text-green-600 hover:underline"
                        onClick={() => resolve(s.id)}
                      >
                        Resolved
                      </button>
                    </Show>
                    <button
                      class="text-xs text-red-500 hover:underline"
                      onClick={() => remove(s.id)}
                    >
                      Delete
                    </button>
                  </div>
                </div>
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
};

const FamilyHistoryTab: Component<{
  patientId: string;
  entries: FamilyHistoryEntry[];
  onReload: () => void;
}> = (props) => {
  const [showForm, setShowForm] = createSignal(false);
  const [relation, setRelation] = createSignal('father');
  const [condition, setCondition] = createSignal('');
  const [age, setAge] = createSignal('');
  const [notes, setNotes] = createSignal('');

  const save = async () => {
    if (!condition().trim()) return;
    try {
      await invoke('health_create_family_history', {
        request: {
          patient_id: props.patientId,
          relation: relation(),
          condition: condition().trim(),
          age_at_diagnosis: age() ? parseInt(age()) : null,
          notes: notes() || null,
        },
      });
      setCondition('');
      setAge('');
      setNotes('');
      setShowForm(false);
      props.onReload();
    } catch (e) {
      alert(String(e));
    }
  };

  const remove = async (id: string) => {
    if (!confirm('Delete this entry?')) return;
    await invoke('health_delete_family_history', { entryId: id });
    props.onReload();
  };

  return (
    <div>
      <div class="flex justify-between items-center mb-4">
        <h2 class="text-lg font-semibold">Family History</h2>
        <button class="btn btn-primary text-sm" onClick={() => setShowForm(!showForm())}>
          {showForm() ? 'Cancel' : '+ Add Entry'}
        </button>
      </div>
      <Show when={showForm()}>
        <div class="card p-4 mb-4 space-y-3">
          <div class="grid grid-cols-2 gap-3">
            <div>
              <label class="block text-xs font-medium mb-1">Relation</label>
              <select
                class="input w-full"
                value={relation()}
                onChange={(e) => setRelation(e.currentTarget.value)}
              >
                <option value="father">Father</option>
                <option value="mother">Mother</option>
                <option value="sibling">Sibling</option>
                <option value="grandparent">Grandparent</option>
                <option value="aunt_uncle">Aunt/Uncle</option>
                <option value="cousin">Cousin</option>
              </select>
            </div>
            <div>
              <label class="block text-xs font-medium mb-1">Age at diagnosis</label>
              <input
                type="number"
                class="input w-full"
                value={age()}
                onInput={(e) => setAge(e.currentTarget.value)}
              />
            </div>
          </div>
          <div>
            <label class="block text-xs font-medium mb-1">Condition *</label>
            <input
              type="text"
              class="input w-full"
              placeholder="Type 2 diabetes"
              value={condition()}
              onInput={(e) => setCondition(e.currentTarget.value)}
            />
          </div>
          <div>
            <label class="block text-xs font-medium mb-1">Notes</label>
            <input
              type="text"
              class="input w-full"
              value={notes()}
              onInput={(e) => setNotes(e.currentTarget.value)}
            />
          </div>
          <button class="btn btn-primary w-full" onClick={save}>
            Save Entry
          </button>
        </div>
      </Show>
      <Show
        when={props.entries.length > 0}
        fallback={
          <div class="card p-8 text-center text-gray-500">No family history logged.</div>
        }
      >
        <div class="space-y-2">
          <For each={props.entries}>
            {(e) => (
              <div class="card p-3 flex justify-between items-center">
                <div>
                  <div class="font-medium">
                    <span class="capitalize text-sm text-gray-500 mr-2">{e.relation}:</span>
                    {e.condition}
                  </div>
                  <Show when={e.age_at_diagnosis}>
                    <div class="text-xs text-gray-500">
                      Diagnosed at age {e.age_at_diagnosis}
                    </div>
                  </Show>
                  <Show when={e.notes}>
                    <div class="text-xs text-gray-500 mt-1">{e.notes}</div>
                  </Show>
                </div>
                <button
                  class="text-xs text-red-500 hover:underline"
                  onClick={() => remove(e.id)}
                >
                  Delete
                </button>
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
};

const AddPatientModal: Component<{
  onClose: () => void;
  onCreated: (p: Patient) => void;
}> = (props) => {
  const [name, setName] = createSignal('');
  const [phone, setPhone] = createSignal('');
  const [dob, setDob] = createSignal('');
  const [sex, setSex] = createSignal('M');
  const [relationship, setRelationship] = createSignal('spouse');
  const [submitting, setSubmitting] = createSignal(false);

  const save = async () => {
    if (!name().trim() || !phone().trim()) return;
    setSubmitting(true);
    try {
      const p = await invoke<Patient>('health_create_patient', {
        request: {
          phone_number: phone().trim(),
          full_name: name().trim(),
          date_of_birth: dob() || null,
          sex: sex(),
          blood_group: null,
          relationship: relationship(),
          is_primary: false,
          avatar_color: '#10b981',
          notes: null,
        },
      });
      props.onCreated(p);
    } catch (e) {
      alert(String(e));
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4">
      <div class="card w-full max-w-md shadow-2xl">
        <div class="p-6">
          <h3 class="text-lg font-bold mb-4">Add family member</h3>
          <div class="space-y-3">
            <div>
              <label class="block text-xs font-medium mb-1">Name *</label>
              <input
                type="text"
                class="input w-full"
                value={name()}
                onInput={(e) => setName(e.currentTarget.value)}
              />
            </div>
            <div>
              <label class="block text-xs font-medium mb-1">Phone *</label>
              <input
                type="tel"
                class="input w-full"
                value={phone()}
                onInput={(e) => setPhone(e.currentTarget.value)}
              />
            </div>
            <div class="grid grid-cols-2 gap-3">
              <div>
                <label class="block text-xs font-medium mb-1">DOB</label>
                <input
                  type="date"
                  class="input w-full"
                  value={dob()}
                  onInput={(e) => setDob(e.currentTarget.value)}
                />
              </div>
              <div>
                <label class="block text-xs font-medium mb-1">Sex</label>
                <select
                  class="input w-full"
                  value={sex()}
                  onChange={(e) => setSex(e.currentTarget.value)}
                >
                  <option value="M">Male</option>
                  <option value="F">Female</option>
                  <option value="other">Other</option>
                </select>
              </div>
            </div>
            <div>
              <label class="block text-xs font-medium mb-1">Relationship</label>
              <select
                class="input w-full"
                value={relationship()}
                onChange={(e) => setRelationship(e.currentTarget.value)}
              >
                <option value="spouse">Spouse</option>
                <option value="child">Child</option>
                <option value="parent">Parent</option>
                <option value="dependent">Dependent</option>
                <option value="sibling">Sibling</option>
                <option value="other">Other</option>
              </select>
            </div>
          </div>
          <div class="flex gap-2 justify-end mt-6">
            <button class="btn btn-secondary" onClick={props.onClose}>
              Cancel
            </button>
            <button class="btn btn-primary" disabled={submitting()} onClick={save}>
              {submitting() ? 'Creating...' : 'Create'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default Health;
