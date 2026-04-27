import { Component, createSignal, For, Show, onMount } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

interface PrescriptionItemRow {
  id: string;
  drug_name: string;
  dosage: string | null;
  frequency: string | null;
  duration_days: number | null;
  instructions: string | null;
}

interface PrescriptionWithItems {
  id: string;
  patient_id: string;
  source_file_id: string | null;
  prescribed_date: string;
  prescriber_name: string | null;
  prescriber_specialty: string | null;
  facility_name: string | null;
  location_city: string | null;
  diagnosis_text: string | null;
  confirmed: boolean;
  created_at: string;
  items: PrescriptionItemRow[];
}

interface LabValueRow {
  id: string;
  test_name: string;
  value_text: string;
  value_numeric: number | null;
  unit: string | null;
  reference_low: number | null;
  reference_high: number | null;
  flag: string | null;
}

interface LabResultWithValues {
  id: string;
  patient_id: string;
  source_file_id: string | null;
  lab_name: string | null;
  report_date: string;
  location_city: string | null;
  confirmed: boolean;
  created_at: string;
  values: LabValueRow[];
  abnormal_count: number;
}

const flagColor = (flag: string | null) => {
  switch (flag) {
    case 'CRITICAL': return 'text-red-700 dark:text-red-400 font-bold';
    case 'HIGH': return 'text-red-500 dark:text-red-400';
    case 'LOW': return 'text-amber-600 dark:text-amber-400';
    case 'NORMAL': return 'text-green-600 dark:text-green-400';
    default: return 'text-gray-500 dark:text-gray-400';
  }
};

const flagBadge = (flag: string | null) => {
  switch (flag) {
    case 'CRITICAL': return 'bg-red-100 dark:bg-red-900/40 text-red-700 dark:text-red-400';
    case 'HIGH': return 'bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400';
    case 'LOW': return 'bg-amber-50 dark:bg-amber-900/20 text-amber-700 dark:text-amber-400';
    case 'NORMAL': return 'bg-green-50 dark:bg-green-900/20 text-green-700 dark:text-green-400';
    default: return 'bg-gray-100 dark:bg-gray-800 text-gray-500';
  }
};

const StructuredRecordsTab: Component<{ patientId: string }> = (props) => {
  const [prescriptions, setPrescriptions] = createSignal<PrescriptionWithItems[]>([]);
  const [labResults, setLabResults] = createSignal<LabResultWithValues[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [expandedRx, setExpandedRx] = createSignal<Set<string>>(new Set());
  const [expandedLab, setExpandedLab] = createSignal<Set<string>>(new Set());
  const [error, setError] = createSignal('');

  const load = async () => {
    setLoading(true);
    setError('');
    try {
      const [rxs, labs] = await Promise.all([
        invoke<PrescriptionWithItems[]>('health_list_prescriptions', { patientId: props.patientId }),
        invoke<LabResultWithValues[]>('health_list_lab_results', { patientId: props.patientId }),
      ]);
      setPrescriptions(rxs);
      setLabResults(labs);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  onMount(load);

  const toggleRx = (id: string) => {
    setExpandedRx((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  };

  const toggleLab = (id: string) => {
    setExpandedLab((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  };

  const deleteRx = async (id: string) => {
    if (!confirm('Delete this prescription?')) return;
    try {
      await invoke('health_delete_prescription', { id });
      await load();
    } catch (e) { setError(String(e)); }
  };

  const deleteLab = async (id: string) => {
    if (!confirm('Delete this lab result?')) return;
    try {
      await invoke('health_delete_lab_result', { id });
      await load();
    } catch (e) { setError(String(e)); }
  };

  return (
    <div class="space-y-8 p-4">
      <Show when={error()}>
        <div class="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg text-sm text-red-700 dark:text-red-300">
          {error()}
        </div>
      </Show>

      {/* ── Prescriptions ──────────────────────────────────────── */}
      <div>
        <div class="flex items-center justify-between mb-3">
          <h2 class="text-base font-semibold text-gray-800 dark:text-gray-100">
            Prescriptions ({prescriptions().length})
          </h2>
          <button
            onClick={load}
            disabled={loading()}
            class="text-xs text-gray-400 hover:text-gray-600 dark:hover:text-gray-300"
          >
            ↻ Refresh
          </button>
        </div>

        <Show
          when={prescriptions().length > 0}
          fallback={
            <div class="text-center py-10 text-sm text-gray-400">
              No prescriptions extracted yet. Use the "Extract →" button in the Documents tab.
            </div>
          }
        >
          <div class="space-y-3">
            <For each={prescriptions()}>
              {(rx) => (
                <div class="card border border-gray-200 dark:border-gray-700 rounded-xl overflow-hidden">
                  {/* Card header */}
                  <div
                    class="flex items-start justify-between p-4 cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors"
                    onClick={() => toggleRx(rx.id)}
                  >
                    <div class="flex-1 min-w-0">
                      <div class="flex items-center gap-2 flex-wrap">
                        <span class="text-sm font-semibold text-gray-800 dark:text-gray-100">
                          {rx.prescribed_date}
                        </span>
                        <Show when={rx.prescriber_name}>
                          <span class="text-sm text-gray-500 dark:text-gray-400">
                            — {rx.prescriber_name}
                          </span>
                        </Show>
                        <Show when={rx.facility_name}>
                          <span class="text-xs px-2 py-0.5 bg-blue-50 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 rounded-full">
                            {rx.facility_name}
                          </span>
                        </Show>
                      </div>
                      <Show when={rx.diagnosis_text}>
                        <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5 truncate">
                          {rx.diagnosis_text}
                        </p>
                      </Show>
                      <p class="text-xs text-gray-400 mt-0.5">
                        {rx.items.length} medication{rx.items.length !== 1 ? 's' : ''}
                        <Show when={rx.location_city}> · {rx.location_city}</Show>
                      </p>
                    </div>
                    <div class="flex items-center gap-2 ml-2 shrink-0">
                      <span class="text-xs text-gray-400">{expandedRx().has(rx.id) ? '▲' : '▼'}</span>
                      <button
                        onClick={(e) => { e.stopPropagation(); deleteRx(rx.id); }}
                        class="text-xs text-red-400 hover:text-red-600 px-1"
                        title="Delete prescription"
                      >
                        ✕
                      </button>
                    </div>
                  </div>

                  {/* Expanded items */}
                  <Show when={expandedRx().has(rx.id)}>
                    <div class="border-t border-gray-100 dark:border-gray-700 px-4 pb-4 pt-3">
                      <table class="w-full text-xs">
                        <thead>
                          <tr class="text-gray-400 border-b border-gray-100 dark:border-gray-700">
                            <th class="text-left pb-1 font-medium">Drug</th>
                            <th class="text-left pb-1 font-medium">Dosage</th>
                            <th class="text-left pb-1 font-medium">Frequency</th>
                            <th class="text-left pb-1 font-medium">Duration</th>
                          </tr>
                        </thead>
                        <tbody>
                          <For each={rx.items}>
                            {(item) => (
                              <tr class="border-b border-gray-50 dark:border-gray-800 last:border-0">
                                <td class="py-1.5 font-medium text-gray-800 dark:text-gray-100">{item.drug_name}</td>
                                <td class="py-1.5 text-gray-600 dark:text-gray-300">{item.dosage ?? '—'}</td>
                                <td class="py-1.5 text-gray-600 dark:text-gray-300">{item.frequency ?? '—'}</td>
                                <td class="py-1.5 text-gray-600 dark:text-gray-300">
                                  {item.duration_days ? `${item.duration_days}d` : '—'}
                                </td>
                              </tr>
                            )}
                          </For>
                        </tbody>
                      </table>
                    </div>
                  </Show>
                </div>
              )}
            </For>
          </div>
        </Show>
      </div>

      {/* ── Lab Results ────────────────────────────────────────── */}
      <div>
        <h2 class="text-base font-semibold text-gray-800 dark:text-gray-100 mb-3">
          Lab Results ({labResults().length})
        </h2>

        <Show
          when={labResults().length > 0}
          fallback={
            <div class="text-center py-10 text-sm text-gray-400">
              No lab results extracted yet. Use the "Extract →" button in the Documents tab.
            </div>
          }
        >
          <div class="space-y-3">
            <For each={labResults()}>
              {(lab) => (
                <div class="card border border-gray-200 dark:border-gray-700 rounded-xl overflow-hidden">
                  {/* Card header */}
                  <div
                    class="flex items-start justify-between p-4 cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-800/50 transition-colors"
                    onClick={() => toggleLab(lab.id)}
                  >
                    <div class="flex-1 min-w-0">
                      <div class="flex items-center gap-2 flex-wrap">
                        <span class="text-sm font-semibold text-gray-800 dark:text-gray-100">
                          {lab.report_date}
                        </span>
                        <Show when={lab.lab_name}>
                          <span class="text-sm text-gray-500 dark:text-gray-400">— {lab.lab_name}</span>
                        </Show>
                        <Show when={lab.abnormal_count > 0}>
                          <span class="text-xs px-2 py-0.5 bg-red-50 dark:bg-red-900/20 text-red-600 dark:text-red-400 rounded-full">
                            {lab.abnormal_count} abnormal
                          </span>
                        </Show>
                      </div>
                      <p class="text-xs text-gray-400 mt-0.5">
                        {lab.values.length} test{lab.values.length !== 1 ? 's' : ''}
                        <Show when={lab.location_city}> · {lab.location_city}</Show>
                      </p>
                    </div>
                    <div class="flex items-center gap-2 ml-2 shrink-0">
                      <span class="text-xs text-gray-400">{expandedLab().has(lab.id) ? '▲' : '▼'}</span>
                      <button
                        onClick={(e) => { e.stopPropagation(); deleteLab(lab.id); }}
                        class="text-xs text-red-400 hover:text-red-600 px-1"
                        title="Delete lab result"
                      >
                        ✕
                      </button>
                    </div>
                  </div>

                  {/* Expanded values */}
                  <Show when={expandedLab().has(lab.id)}>
                    <div class="border-t border-gray-100 dark:border-gray-700 px-4 pb-4 pt-3">
                      <table class="w-full text-xs">
                        <thead>
                          <tr class="text-gray-400 border-b border-gray-100 dark:border-gray-700">
                            <th class="text-left pb-1 font-medium">Test</th>
                            <th class="text-right pb-1 font-medium">Value</th>
                            <th class="text-right pb-1 font-medium">Range</th>
                            <th class="text-center pb-1 font-medium">Flag</th>
                          </tr>
                        </thead>
                        <tbody>
                          <For each={lab.values}>
                            {(v) => (
                              <tr class="border-b border-gray-50 dark:border-gray-800 last:border-0">
                                <td class="py-1.5 text-gray-700 dark:text-gray-200">{v.test_name}</td>
                                <td class={`py-1.5 text-right font-medium ${flagColor(v.flag)}`}>
                                  {v.value_text} {v.unit ? <span class="text-gray-400 font-normal">{v.unit}</span> : ''}
                                </td>
                                <td class="py-1.5 text-right text-gray-400">
                                  {v.reference_low !== null || v.reference_high !== null
                                    ? `${v.reference_low ?? '?'}–${v.reference_high ?? '?'}`
                                    : '—'}
                                </td>
                                <td class="py-1.5 text-center">
                                  <Show when={v.flag}>
                                    <span class={`px-1.5 py-0.5 rounded text-[10px] font-semibold ${flagBadge(v.flag)}`}>
                                      {v.flag}
                                    </span>
                                  </Show>
                                </td>
                              </tr>
                            )}
                          </For>
                        </tbody>
                      </table>
                    </div>
                  </Show>
                </div>
              )}
            </For>
          </div>
        </Show>
      </div>
    </div>
  );
};

export default StructuredRecordsTab;
