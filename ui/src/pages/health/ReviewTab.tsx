import { Component, createSignal, createEffect, For, Show, onMount, onCleanup } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

interface Patient {
  id: string;
  full_name: string;
  [key: string]: any;
}

interface PendingReview {
  extraction_id: string;
  file_id: string;
  original_path: string;
  mime_type?: string;
  document_type: string;
  confidence: number;
  raw_text_preview: string;
  extracted_json: any;
}

interface LabTestRow {
  test_name?: string;
  value?: string | number;
  unit?: string;
  flag?: string;
  reference_low?: number | null;
  reference_high?: number | null;
  [key: string]: any;
}

interface MedicationRow {
  name?: string;
  dose?: string;
  frequency?: string;
  route?: string;
  indication?: string;
  [key: string]: any;
}

type ClassifyProgress = {
  processed: number;
  total: number;
  current?: string;
};

function baseName(p: string): string {
  if (!p) return '';
  const parts = p.split(/[/\\]/);
  return parts[parts.length - 1] || p;
}

function confidencePct(c: number): string {
  if (c <= 1) return `${Math.round(c * 100)}%`;
  return `${Math.round(c)}%`;
}

// -------------------------------------------------------------------
// Editors per document type
// -------------------------------------------------------------------

const LabReportEditor: Component<{
  data: any;
  onChange: (next: any) => void;
}> = (props) => {
  const rows = (): LabTestRow[] => {
    const d = props.data || {};
    if (Array.isArray(d.tests)) return d.tests;
    if (Array.isArray(d)) return d;
    return [];
  };

  const update = (i: number, key: keyof LabTestRow, value: string) => {
    const current = rows().slice();
    current[i] = { ...current[i], [key]: value };
    props.onChange({ ...(props.data || {}), tests: current });
  };

  const addRow = () => {
    const current = rows().slice();
    current.push({ test_name: '', value: '', unit: '', flag: '' });
    props.onChange({ ...(props.data || {}), tests: current });
  };

  const removeRow = (i: number) => {
    const current = rows().slice();
    current.splice(i, 1);
    props.onChange({ ...(props.data || {}), tests: current });
  };

  const topMeta = props.data || {};

  return (
    <div class="space-y-3">
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="block text-xs font-medium mb-1">Collected date</label>
          <input
            type="date"
            class="input w-full"
            value={topMeta.collected_at?.slice(0, 10) || ''}
            onInput={(e) =>
              props.onChange({ ...(props.data || {}), collected_at: e.currentTarget.value })
            }
          />
        </div>
        <div>
          <label class="block text-xs font-medium mb-1">Lab / Source</label>
          <input
            type="text"
            class="input w-full"
            value={topMeta.source || ''}
            onInput={(e) =>
              props.onChange({ ...(props.data || {}), source: e.currentTarget.value })
            }
          />
        </div>
      </div>
      <div>
        <div class="flex items-center justify-between mb-2">
          <label class="text-xs font-medium">Tests ({rows().length})</label>
          <button class="btn btn-secondary text-xs" onClick={addRow}>
            + Add row
          </button>
        </div>
        <div class="overflow-x-auto border border-gray-200 dark:border-gray-700 rounded">
          <table class="w-full text-sm">
            <thead class="bg-gray-50 dark:bg-gray-800">
              <tr>
                <th class="text-left p-2">Test</th>
                <th class="text-left p-2">Value</th>
                <th class="text-left p-2">Unit</th>
                <th class="text-left p-2">Flag</th>
                <th class="w-8"></th>
              </tr>
            </thead>
            <tbody>
              <For each={rows()}>
                {(t, i) => (
                  <tr class="border-t border-gray-100 dark:border-gray-800">
                    <td class="p-1">
                      <input
                        class="input w-full text-xs"
                        value={t.test_name || ''}
                        onInput={(e) => update(i(), 'test_name', e.currentTarget.value)}
                      />
                    </td>
                    <td class="p-1">
                      <input
                        class="input w-full text-xs"
                        value={t.value != null ? String(t.value) : ''}
                        onInput={(e) => update(i(), 'value', e.currentTarget.value)}
                      />
                    </td>
                    <td class="p-1">
                      <input
                        class="input w-full text-xs"
                        value={t.unit || ''}
                        onInput={(e) => update(i(), 'unit', e.currentTarget.value)}
                      />
                    </td>
                    <td class="p-1">
                      <select
                        class="input w-full text-xs"
                        value={t.flag || ''}
                        onChange={(e) => update(i(), 'flag', e.currentTarget.value)}
                      >
                        <option value="">—</option>
                        <option value="H">H</option>
                        <option value="L">L</option>
                        <option value="HH">HH</option>
                        <option value="LL">LL</option>
                        <option value="N">N</option>
                      </select>
                    </td>
                    <td class="p-1">
                      <button
                        class="text-xs text-red-500 hover:underline"
                        onClick={() => removeRow(i())}
                        title="Remove row"
                      >
                        ✕
                      </button>
                    </td>
                  </tr>
                )}
              </For>
              <Show when={rows().length === 0}>
                <tr>
                  <td colspan="5" class="p-3 text-center text-xs text-gray-500">
                    No tests extracted. Click "Add row" to add one manually.
                  </td>
                </tr>
              </Show>
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};

const PrescriptionEditor: Component<{
  data: any;
  onChange: (next: any) => void;
}> = (props) => {
  const rows = (): MedicationRow[] => {
    const d = props.data || {};
    if (Array.isArray(d.medications)) return d.medications;
    if (Array.isArray(d)) return d;
    return [];
  };

  const update = (i: number, key: keyof MedicationRow, value: string) => {
    const current = rows().slice();
    current[i] = { ...current[i], [key]: value };
    props.onChange({ ...(props.data || {}), medications: current });
  };

  const addRow = () => {
    const current = rows().slice();
    current.push({ name: '', dose: '', frequency: '', indication: '' });
    props.onChange({ ...(props.data || {}), medications: current });
  };

  const removeRow = (i: number) => {
    const current = rows().slice();
    current.splice(i, 1);
    props.onChange({ ...(props.data || {}), medications: current });
  };

  const topMeta = props.data || {};

  return (
    <div class="space-y-3">
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="block text-xs font-medium mb-1">Prescribed date</label>
          <input
            type="date"
            class="input w-full"
            value={topMeta.prescribed_at?.slice(0, 10) || ''}
            onInput={(e) =>
              props.onChange({ ...(props.data || {}), prescribed_at: e.currentTarget.value })
            }
          />
        </div>
        <div>
          <label class="block text-xs font-medium mb-1">Prescriber</label>
          <input
            type="text"
            class="input w-full"
            value={topMeta.prescriber || ''}
            onInput={(e) =>
              props.onChange({ ...(props.data || {}), prescriber: e.currentTarget.value })
            }
          />
        </div>
      </div>
      <div>
        <div class="flex items-center justify-between mb-2">
          <label class="text-xs font-medium">Medications ({rows().length})</label>
          <button class="btn btn-secondary text-xs" onClick={addRow}>
            + Add row
          </button>
        </div>
        <div class="overflow-x-auto border border-gray-200 dark:border-gray-700 rounded">
          <table class="w-full text-sm">
            <thead class="bg-gray-50 dark:bg-gray-800">
              <tr>
                <th class="text-left p-2">Name</th>
                <th class="text-left p-2">Dose</th>
                <th class="text-left p-2">Frequency</th>
                <th class="text-left p-2">Indication</th>
                <th class="w-8"></th>
              </tr>
            </thead>
            <tbody>
              <For each={rows()}>
                {(m, i) => (
                  <tr class="border-t border-gray-100 dark:border-gray-800">
                    <td class="p-1">
                      <input
                        class="input w-full text-xs"
                        value={m.name || ''}
                        onInput={(e) => update(i(), 'name', e.currentTarget.value)}
                      />
                    </td>
                    <td class="p-1">
                      <input
                        class="input w-full text-xs"
                        value={m.dose || ''}
                        onInput={(e) => update(i(), 'dose', e.currentTarget.value)}
                      />
                    </td>
                    <td class="p-1">
                      <input
                        class="input w-full text-xs"
                        value={m.frequency || ''}
                        onInput={(e) => update(i(), 'frequency', e.currentTarget.value)}
                      />
                    </td>
                    <td class="p-1">
                      <input
                        class="input w-full text-xs"
                        value={m.indication || ''}
                        onInput={(e) => update(i(), 'indication', e.currentTarget.value)}
                      />
                    </td>
                    <td class="p-1">
                      <button
                        class="text-xs text-red-500 hover:underline"
                        onClick={() => removeRow(i())}
                        title="Remove row"
                      >
                        ✕
                      </button>
                    </td>
                  </tr>
                )}
              </For>
              <Show when={rows().length === 0}>
                <tr>
                  <td colspan="5" class="p-3 text-center text-xs text-gray-500">
                    No medications extracted.
                  </td>
                </tr>
              </Show>
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};

const ImagingEditor: Component<{
  data: any;
  onChange: (next: any) => void;
}> = (props) => {
  const d = () => props.data || {};
  const set = (k: string, v: string) => props.onChange({ ...d(), [k]: v });
  return (
    <div class="space-y-3">
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="block text-xs font-medium mb-1">Modality</label>
          <input
            type="text"
            class="input w-full"
            value={d().modality || ''}
            onInput={(e) => set('modality', e.currentTarget.value)}
            placeholder="X-Ray / MRI / CT / Ultrasound"
          />
        </div>
        <div>
          <label class="block text-xs font-medium mb-1">Study date</label>
          <input
            type="date"
            class="input w-full"
            value={(d().study_date || '').slice(0, 10)}
            onInput={(e) => set('study_date', e.currentTarget.value)}
          />
        </div>
      </div>
      <div>
        <label class="block text-xs font-medium mb-1">Body part</label>
        <input
          type="text"
          class="input w-full"
          value={d().body_part || ''}
          onInput={(e) => set('body_part', e.currentTarget.value)}
        />
      </div>
      <div>
        <label class="block text-xs font-medium mb-1">Findings</label>
        <textarea
          class="input w-full h-24"
          value={d().findings || ''}
          onInput={(e) => set('findings', e.currentTarget.value)}
        />
      </div>
      <div>
        <label class="block text-xs font-medium mb-1">Impression</label>
        <textarea
          class="input w-full h-20"
          value={d().impression || ''}
          onInput={(e) => set('impression', e.currentTarget.value)}
        />
      </div>
    </div>
  );
};

const DischargeEditor: Component<{
  data: any;
  onChange: (next: any) => void;
}> = (props) => {
  const d = () => props.data || {};
  const set = (k: string, v: any) => props.onChange({ ...d(), [k]: v });
  const meds = (): MedicationRow[] => {
    const raw = d().medications;
    return Array.isArray(raw) ? raw : [];
  };
  const updateMed = (i: number, key: keyof MedicationRow, value: string) => {
    const current = meds().slice();
    current[i] = { ...current[i], [key]: value };
    set('medications', current);
  };
  const addMed = () => {
    set('medications', [...meds(), { name: '', dose: '', frequency: '' }]);
  };
  const removeMed = (i: number) => {
    const current = meds().slice();
    current.splice(i, 1);
    set('medications', current);
  };
  return (
    <div class="space-y-3">
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="block text-xs font-medium mb-1">Admit date</label>
          <input
            type="date"
            class="input w-full"
            value={(d().admit_date || '').slice(0, 10)}
            onInput={(e) => set('admit_date', e.currentTarget.value)}
          />
        </div>
        <div>
          <label class="block text-xs font-medium mb-1">Discharge date</label>
          <input
            type="date"
            class="input w-full"
            value={(d().discharge_date || '').slice(0, 10)}
            onInput={(e) => set('discharge_date', e.currentTarget.value)}
          />
        </div>
      </div>
      <div>
        <label class="block text-xs font-medium mb-1">Diagnosis</label>
        <textarea
          class="input w-full h-20"
          value={d().diagnosis || ''}
          onInput={(e) => set('diagnosis', e.currentTarget.value)}
        />
      </div>
      <div>
        <div class="flex items-center justify-between mb-1">
          <label class="text-xs font-medium">Discharge medications</label>
          <button class="btn btn-secondary text-xs" onClick={addMed}>
            + Add
          </button>
        </div>
        <div class="space-y-2">
          <For each={meds()}>
            {(m, i) => (
              <div class="flex gap-2 items-center">
                <input
                  class="input text-xs flex-1"
                  placeholder="Name"
                  value={m.name || ''}
                  onInput={(e) => updateMed(i(), 'name', e.currentTarget.value)}
                />
                <input
                  class="input text-xs w-24"
                  placeholder="Dose"
                  value={m.dose || ''}
                  onInput={(e) => updateMed(i(), 'dose', e.currentTarget.value)}
                />
                <input
                  class="input text-xs w-28"
                  placeholder="Frequency"
                  value={m.frequency || ''}
                  onInput={(e) => updateMed(i(), 'frequency', e.currentTarget.value)}
                />
                <button class="text-xs text-red-500" onClick={() => removeMed(i())}>
                  ✕
                </button>
              </div>
            )}
          </For>
        </div>
      </div>
      <div>
        <label class="block text-xs font-medium mb-1">Follow-ups</label>
        <textarea
          class="input w-full h-20"
          value={d().follow_ups || ''}
          onInput={(e) => set('follow_ups', e.currentTarget.value)}
        />
      </div>
    </div>
  );
};

const ConsultationEditor: Component<{
  data: any;
  onChange: (next: any) => void;
}> = (props) => {
  const d = () => props.data || {};
  const set = (k: string, v: string) => props.onChange({ ...d(), [k]: v });
  return (
    <div class="space-y-3">
      <div class="grid grid-cols-2 gap-3">
        <div>
          <label class="block text-xs font-medium mb-1">Doctor</label>
          <input
            type="text"
            class="input w-full"
            value={d().doctor || ''}
            onInput={(e) => set('doctor', e.currentTarget.value)}
          />
        </div>
        <div>
          <label class="block text-xs font-medium mb-1">Date</label>
          <input
            type="date"
            class="input w-full"
            value={(d().date || '').slice(0, 10)}
            onInput={(e) => set('date', e.currentTarget.value)}
          />
        </div>
      </div>
      <div>
        <label class="block text-xs font-medium mb-1">Complaint</label>
        <textarea
          class="input w-full h-16"
          value={d().complaint || ''}
          onInput={(e) => set('complaint', e.currentTarget.value)}
        />
      </div>
      <div>
        <label class="block text-xs font-medium mb-1">Assessment</label>
        <textarea
          class="input w-full h-20"
          value={d().assessment || ''}
          onInput={(e) => set('assessment', e.currentTarget.value)}
        />
      </div>
      <div>
        <label class="block text-xs font-medium mb-1">Plan</label>
        <textarea
          class="input w-full h-20"
          value={d().plan || ''}
          onInput={(e) => set('plan', e.currentTarget.value)}
        />
      </div>
    </div>
  );
};

const GenericJsonEditor: Component<{
  data: any;
  onChange: (next: any) => void;
}> = (props) => {
  const [text, setText] = createSignal<string>(JSON.stringify(props.data ?? {}, null, 2));
  const [err, setErr] = createSignal<string | null>(null);

  createEffect(() => {
    // Reset when underlying data changes via external source.
    setText(JSON.stringify(props.data ?? {}, null, 2));
    setErr(null);
  });

  const onInput = (value: string) => {
    setText(value);
    try {
      const parsed = value.trim() ? JSON.parse(value) : {};
      setErr(null);
      props.onChange(parsed);
    } catch (e) {
      setErr(String(e));
    }
  };

  return (
    <div>
      <label class="block text-xs font-medium mb-1">Extracted JSON (free-form)</label>
      <textarea
        class="input w-full h-64 font-mono text-xs"
        value={text()}
        onInput={(e) => onInput(e.currentTarget.value)}
      />
      <Show when={err()}>
        <p class="text-xs text-red-500 mt-1">{err()}</p>
      </Show>
    </div>
  );
};

// -------------------------------------------------------------------
// Main ReviewTab
// -------------------------------------------------------------------

const ReviewTab: Component<{
  activePatient: Patient;
}> = (props) => {
  const [pending, setPending] = createSignal<PendingReview[]>([]);
  const [current, setCurrent] = createSignal<PendingReview | null>(null);
  const [editedJson, setEditedJson] = createSignal<any>({});
  const [loading, setLoading] = createSignal(false);
  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [classifying, setClassifying] = createSignal(false);
  const [classifyProgress, setClassifyProgress] = createSignal<ClassifyProgress | null>(null);

  let unlisten: UnlistenFn | null = null;

  const loadPending = async () => {
    setLoading(true);
    setError(null);
    try {
      const items = await invoke<PendingReview[]>('health_list_pending_review', {
        patientId: props.activePatient.id,
      });
      setPending(items);
      if (items.length > 0 && !current()) {
        selectReview(items[0]);
      } else if (items.length === 0) {
        setCurrent(null);
        setEditedJson({});
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const selectReview = (r: PendingReview) => {
    setCurrent(r);
    setEditedJson(r.extracted_json || {});
  };

  onMount(async () => {
    await loadPending();
    try {
      unlisten = await listen<ClassifyProgress>('health-classify-progress', (event) => {
        setClassifyProgress(event.payload);
      });
    } catch (_) {
      // Event bus not available — OK.
    }
  });

  onCleanup(() => {
    if (unlisten) unlisten();
  });

  createEffect(() => {
    // Re-load whenever active patient changes.
    const p = props.activePatient;
    if (p) {
      setCurrent(null);
      void loadPending();
    }
  });

  const saveReview = async (accept: boolean, reject: boolean = false) => {
    const cur = current();
    if (!cur) return;
    setSaving(true);
    setError(null);
    try {
      await invoke('health_save_review', {
        extractionId: cur.extraction_id,
        corrections: reject ? null : editedJson(),
        accept,
      });
      // Refresh queue and move to next.
      const remaining = pending().filter((p) => p.extraction_id !== cur.extraction_id);
      setPending(remaining);
      if (remaining.length > 0) {
        selectReview(remaining[0]);
      } else {
        setCurrent(null);
        setEditedJson({});
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const skip = () => {
    const cur = current();
    if (!cur) return;
    const remaining = pending().filter((p) => p.extraction_id !== cur.extraction_id);
    if (remaining.length > 0) {
      selectReview(remaining[0]);
    } else {
      setCurrent(null);
    }
  };

  const runClassifier = async () => {
    setClassifying(true);
    setError(null);
    setClassifyProgress({ processed: 0, total: 0 });
    try {
      await invoke('health_classify_pending', { feature: null });
      await loadPending();
    } catch (e) {
      setError(String(e));
    } finally {
      setClassifying(false);
      setClassifyProgress(null);
    }
  };

  const renderEditor = () => {
    const cur = current();
    if (!cur) return null;
    const props2 = {
      data: editedJson(),
      onChange: setEditedJson,
    };
    switch (cur.document_type) {
      case 'lab_report':
        return <LabReportEditor {...props2} />;
      case 'prescription':
        return <PrescriptionEditor {...props2} />;
      case 'imaging_report':
        return <ImagingEditor {...props2} />;
      case 'discharge_summary':
        return <DischargeEditor {...props2} />;
      case 'consultation_note':
        return <ConsultationEditor {...props2} />;
      default:
        return <GenericJsonEditor {...props2} />;
    }
  };

  return (
    <div>
      <div class="flex items-center justify-between mb-4">
        <div>
          <h2 class="text-lg font-semibold">Review extractions</h2>
          <p class="text-xs text-gray-500">
            Verify the AI-extracted fields before they are merged into your health record.
          </p>
        </div>
        <div class="flex gap-2">
          <button
            class="btn btn-secondary text-sm"
            onClick={loadPending}
            disabled={loading()}
          >
            {loading() ? 'Loading…' : 'Refresh'}
          </button>
          <button
            class="btn btn-secondary text-sm"
            onClick={runClassifier}
            disabled={classifying()}
          >
            {classifying() ? 'Classifying…' : 'Run classifier'}
          </button>
        </div>
      </div>

      <Show when={error()}>
        <div class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
          <p class="text-sm text-red-700 dark:text-red-300">{error()}</p>
        </div>
      </Show>

      <Show when={classifyProgress() && classifying()}>
        <div class="card p-3 mb-4">
          <div class="text-xs text-gray-500 mb-1">
            Classifying: {classifyProgress()!.processed} / {classifyProgress()!.total}
          </div>
          <div class="h-2 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
            <div
              class="h-full bg-minion-500 transition-all"
              style={{
                width:
                  classifyProgress()!.total > 0
                    ? `${(classifyProgress()!.processed / classifyProgress()!.total) * 100}%`
                    : '0%',
              }}
            />
          </div>
          <Show when={classifyProgress()!.current}>
            <div class="text-xs mt-1 truncate font-mono">{classifyProgress()!.current}</div>
          </Show>
        </div>
      </Show>

      <Show
        when={pending().length > 0 || current()}
        fallback={
          <div class="card p-8 text-center">
            <p class="text-sm text-gray-500 mb-4">
              No extractions pending review. Import documents first, then run the classifier.
            </p>
          </div>
        }
      >
        <div class="grid grid-cols-12 gap-4">
          {/* Left: Queue */}
          <div class="col-span-3">
            <div class="card p-2 max-h-[70vh] overflow-y-auto">
              <div class="px-2 py-1 text-xs font-semibold text-gray-500">
                Queue ({pending().length})
              </div>
              <For each={pending()}>
                {(r) => (
                  <button
                    class="w-full text-left p-2 rounded mb-1 text-xs"
                    classList={{
                      'bg-minion-50 dark:bg-minion-900/30 border border-minion-300':
                        current()?.extraction_id === r.extraction_id,
                      'hover:bg-gray-50 dark:hover:bg-gray-800':
                        current()?.extraction_id !== r.extraction_id,
                    }}
                    onClick={() => selectReview(r)}
                  >
                    <div class="font-medium truncate" title={r.original_path}>
                      {baseName(r.original_path)}
                    </div>
                    <div class="mt-1 flex items-center gap-1">
                      <span class="px-1.5 py-0.5 bg-gray-100 dark:bg-gray-800 rounded text-[10px]">
                        {r.document_type}
                      </span>
                      <span class="text-[10px] text-gray-500">
                        {confidencePct(r.confidence)}
                      </span>
                    </div>
                  </button>
                )}
              </For>
              <Show when={pending().length === 0}>
                <p class="text-xs text-gray-500 p-2">Queue empty.</p>
              </Show>
            </div>
          </div>

          {/* Middle + Right: Two-pane */}
          <div class="col-span-9">
            <Show
              when={current()}
              fallback={
                <div class="card p-8 text-center text-sm text-gray-500">
                  Select an item from the queue to review.
                </div>
              }
            >
              {(cur) => (
                <div>
                  {/* Header row */}
                  <div class="card p-3 mb-3 flex items-center justify-between">
                    <div class="min-w-0">
                      <div class="text-sm font-semibold truncate" title={cur().original_path}>
                        {baseName(cur().original_path)}
                      </div>
                      <div class="text-xs text-gray-500 font-mono truncate">
                        {cur().original_path}
                      </div>
                    </div>
                    <div class="flex items-center gap-2 ml-3 flex-shrink-0">
                      <span class="px-2 py-1 bg-minion-100 dark:bg-minion-900/40 text-minion-700 dark:text-minion-300 rounded text-xs">
                        {cur().document_type} ({confidencePct(cur().confidence)})
                      </span>
                    </div>
                  </div>

                  <div class="grid grid-cols-2 gap-3">
                    {/* Left pane: preview */}
                    <div class="card p-3 max-h-[65vh] overflow-y-auto">
                      <div class="text-xs font-semibold text-gray-500 mb-2">
                        Original document
                      </div>
                      <pre class="text-xs whitespace-pre-wrap font-mono text-gray-800 dark:text-gray-200">
                        {cur().raw_text_preview || '(no text preview)'}
                      </pre>
                    </div>

                    {/* Right pane: editor */}
                    <div class="card p-3 max-h-[65vh] overflow-y-auto">
                      <div class="text-xs font-semibold text-gray-500 mb-2">
                        Extracted fields
                      </div>
                      {renderEditor()}
                    </div>
                  </div>

                  {/* Bottom action bar */}
                  <div class="card p-3 mt-3 flex items-center justify-between">
                    <div class="text-xs text-gray-500">
                      {pending().length} pending in queue
                    </div>
                    <div class="flex gap-2">
                      <button
                        class="btn btn-secondary text-sm"
                        onClick={skip}
                        disabled={saving()}
                      >
                        Skip
                      </button>
                      <button
                        class="btn btn-secondary text-sm text-red-600"
                        onClick={() => saveReview(false, true)}
                        disabled={saving()}
                      >
                        Reject
                      </button>
                      <button
                        class="btn btn-primary text-sm"
                        onClick={() => saveReview(true, false)}
                        disabled={saving()}
                      >
                        {saving() ? 'Saving…' : 'Save & Approve'}
                      </button>
                    </div>
                  </div>
                </div>
              )}
            </Show>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default ReviewTab;
