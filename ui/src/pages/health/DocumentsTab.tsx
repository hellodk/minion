import { Component, createSignal, createMemo, createEffect, For, Show, onMount } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

interface Patient {
  id: string;
  full_name: string;
  [key: string]: any;
}

interface FileEntry {
  id: string;
  sha256: string;
  original_path: string;
  mime_type?: string;
  size_bytes: number;
  status: string;
  created_at: string;
}

interface ExtractionEntry {
  id: string;
  file_id: string;
  document_type?: string;
  classification_confidence?: number;
  raw_text?: string;
  extracted_json?: string;
  user_reviewed: boolean;
}

function fmtBytes(b: number): string {
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
  if (b < 1024 * 1024 * 1024) return `${(b / (1024 * 1024)).toFixed(1)} MB`;
  return `${(b / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function baseName(p: string): string {
  if (!p) return '';
  const parts = p.split(/[/\\]/);
  return parts[parts.length - 1] || p;
}

function countEntities(ex: ExtractionEntry | null): number {
  if (!ex || !ex.extracted_json) return 0;
  try {
    const parsed = JSON.parse(ex.extracted_json);
    if (!parsed) return 0;
    if (Array.isArray(parsed.tests)) return parsed.tests.length;
    if (Array.isArray(parsed.medications)) return parsed.medications.length;
    if (Array.isArray(parsed)) return parsed.length;
    return Object.keys(parsed).length;
  } catch (_) {
    return 0;
  }
}

function statusBadgeClass(status: string): string {
  switch (status) {
    case 'completed':
      return 'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300';
    case 'extracted':
      return 'bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300';
    case 'extracted_pending_review':
      return 'bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300';
    case 'pending':
    case 'extracting':
      return 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300';
    case 'failed':
      return 'bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300';
    default:
      return 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-400';
  }
}

const STATUS_OPTIONS = [
  { value: '', label: 'All statuses' },
  { value: 'pending', label: 'Pending' },
  { value: 'extracting', label: 'Extracting' },
  { value: 'extracted', label: 'Extracted' },
  { value: 'extracted_pending_review', label: 'Pending review' },
  { value: 'completed', label: 'Completed' },
  { value: 'failed', label: 'Failed' },
];

const DOC_TYPE_OPTIONS = [
  { value: '', label: 'All types' },
  { value: 'lab_report', label: 'Lab report' },
  { value: 'prescription', label: 'Prescription' },
  { value: 'imaging_report', label: 'Imaging report' },
  { value: 'discharge_summary', label: 'Discharge summary' },
  { value: 'consultation_note', label: 'Consultation note' },
  { value: 'unknown', label: 'Unknown' },
];

const DocumentsTab: Component<{
  activePatient: Patient;
}> = (props) => {
  const [files, setFiles] = createSignal<FileEntry[]>([]);
  const [extractions, setExtractions] = createSignal<Record<string, ExtractionEntry | null>>({});
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [search, setSearch] = createSignal('');
  const [statusFilter, setStatusFilter] = createSignal('');
  const [typeFilter, setTypeFilter] = createSignal('');
  const [viewFile, setViewFile] = createSignal<FileEntry | null>(null);
  const [reclassifyingId, setReclassifyingId] = createSignal<string | null>(null);

  const loadFiles = async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await invoke<FileEntry[]>('health_list_files', {
        patientId: props.activePatient.id,
      });
      setFiles(list);
      // Load extractions in parallel
      const entries = await Promise.all(
        list.map(async (f) => {
          try {
            const ex = await invoke<ExtractionEntry | null>('health_get_extraction', {
              fileId: f.id,
            });
            return [f.id, ex] as const;
          } catch (_) {
            return [f.id, null] as const;
          }
        }),
      );
      const map: Record<string, ExtractionEntry | null> = {};
      for (const [id, ex] of entries) map[id] = ex;
      setExtractions(map);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  onMount(() => {
    void loadFiles();
  });

  createEffect(() => {
    const p = props.activePatient;
    if (p) void loadFiles();
  });

  const filtered = createMemo(() => {
    const q = search().toLowerCase().trim();
    const st = statusFilter();
    const ty = typeFilter();
    return files().filter((f) => {
      if (q && !f.original_path.toLowerCase().includes(q)) return false;
      if (st && f.status !== st) return false;
      if (ty) {
        const ex = extractions()[f.id];
        const docType = ex?.document_type || 'unknown';
        if (docType !== ty) return false;
      }
      return true;
    });
  });

  const remove = async (file: FileEntry) => {
    if (!confirm(`Delete ${baseName(file.original_path)}? This removes the vault copy too.`)) {
      return;
    }
    try {
      await invoke('health_delete_file', { fileId: file.id });
      await loadFiles();
    } catch (e) {
      alert(String(e));
    }
  };

  const reclassify = async (file: FileEntry) => {
    setReclassifyingId(file.id);
    try {
      // The backend accepts an optional feature filter. A specific file id
      // isn't supported in the spec, so we re-run over all pending — that's
      // fine and idempotent.
      await invoke('health_classify_pending', { feature: null });
      await loadFiles();
    } catch (e) {
      alert(String(e));
    } finally {
      setReclassifyingId(null);
    }
  };

  return (
    <div>
      <div class="flex items-center justify-between mb-4">
        <div>
          <h2 class="text-lg font-semibold">Documents</h2>
          <p class="text-xs text-gray-500">
            All imported medical files for {props.activePatient.full_name}.
          </p>
        </div>
        <button class="btn btn-secondary text-sm" onClick={loadFiles} disabled={loading()}>
          {loading() ? 'Loading…' : 'Refresh'}
        </button>
      </div>

      <Show when={error()}>
        <div class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
          <p class="text-sm text-red-700 dark:text-red-300">{error()}</p>
        </div>
      </Show>

      {/* Filters */}
      <div class="card p-3 mb-3">
        <div class="grid grid-cols-1 md:grid-cols-3 gap-3">
          <div>
            <label class="block text-xs font-medium mb-1">Search (filename)</label>
            <input
              type="text"
              class="input w-full"
              placeholder="Search by name or path…"
              value={search()}
              onInput={(e) => setSearch(e.currentTarget.value)}
            />
          </div>
          <div>
            <label class="block text-xs font-medium mb-1">Status</label>
            <select
              class="input w-full"
              value={statusFilter()}
              onChange={(e) => setStatusFilter(e.currentTarget.value)}
            >
              <For each={STATUS_OPTIONS}>
                {(o) => <option value={o.value}>{o.label}</option>}
              </For>
            </select>
          </div>
          <div>
            <label class="block text-xs font-medium mb-1">Document type</label>
            <select
              class="input w-full"
              value={typeFilter()}
              onChange={(e) => setTypeFilter(e.currentTarget.value)}
            >
              <For each={DOC_TYPE_OPTIONS}>
                {(o) => <option value={o.value}>{o.label}</option>}
              </For>
            </select>
          </div>
        </div>
        <div class="text-xs text-gray-500 mt-2">
          Showing {filtered().length} of {files().length}
        </div>
      </div>

      {/* Table */}
      <div class="card p-0 overflow-x-auto">
        <table class="w-full text-sm">
          <thead class="bg-gray-50 dark:bg-gray-800">
            <tr>
              <th class="text-left p-2">Filename</th>
              <th class="text-left p-2">Type</th>
              <th class="text-right p-2">Size</th>
              <th class="text-left p-2">Status</th>
              <th class="text-left p-2">Classification</th>
              <th class="text-right p-2">Entities</th>
              <th class="text-left p-2">Imported</th>
              <th class="text-right p-2">Actions</th>
            </tr>
          </thead>
          <tbody>
            <For each={filtered()}>
              {(f) => {
                const ex = () => extractions()[f.id];
                return (
                  <tr class="border-t border-gray-100 dark:border-gray-800 hover:bg-gray-50 dark:hover:bg-gray-800/30">
                    <td class="p-2 max-w-xs">
                      <div class="font-medium truncate" title={f.original_path}>
                        {baseName(f.original_path)}
                      </div>
                      <div class="text-xs text-gray-500 font-mono truncate">
                        {f.original_path}
                      </div>
                    </td>
                    <td class="p-2 text-xs text-gray-500">{f.mime_type || '—'}</td>
                    <td class="p-2 text-right text-xs text-gray-500">
                      {fmtBytes(f.size_bytes)}
                    </td>
                    <td class="p-2">
                      <span
                        class={`px-2 py-0.5 rounded text-xs ${statusBadgeClass(f.status)}`}
                      >
                        {f.status}
                      </span>
                    </td>
                    <td class="p-2 text-xs">
                      <Show when={ex()?.document_type} fallback={<span class="text-gray-400">—</span>}>
                        <span class="font-medium">{ex()!.document_type}</span>
                        <Show when={ex()!.classification_confidence != null}>
                          <span class="text-gray-500 ml-1">
                            ({Math.round((ex()!.classification_confidence || 0) * 100)}%)
                          </span>
                        </Show>
                      </Show>
                    </td>
                    <td class="p-2 text-right text-xs">{countEntities(ex() || null)}</td>
                    <td class="p-2 text-xs text-gray-500">
                      {f.created_at.slice(0, 10)}
                    </td>
                    <td class="p-2 text-right">
                      <div class="flex justify-end gap-1">
                        <button
                          class="text-xs text-minion-600 hover:underline"
                          onClick={() => setViewFile(f)}
                          title="View raw text"
                        >
                          View
                        </button>
                        <button
                          class="text-xs text-minion-600 hover:underline disabled:opacity-50"
                          onClick={() => reclassify(f)}
                          disabled={reclassifyingId() === f.id}
                          title="Re-classify this document"
                        >
                          {reclassifyingId() === f.id ? '…' : 'Re-classify'}
                        </button>
                        <button
                          class="text-xs text-red-500 hover:underline"
                          onClick={() => remove(f)}
                          title="Delete file"
                        >
                          Delete
                        </button>
                      </div>
                    </td>
                  </tr>
                );
              }}
            </For>
            <Show when={filtered().length === 0 && !loading()}>
              <tr>
                <td colspan="8" class="p-6 text-center text-sm text-gray-500">
                  No documents match the current filters.
                </td>
              </tr>
            </Show>
          </tbody>
        </table>
      </div>

      {/* View modal */}
      <Show when={viewFile()}>
        <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4">
          <div class="card w-full max-w-4xl max-h-[90vh] overflow-hidden shadow-2xl flex flex-col">
            <div class="p-4 border-b border-gray-200 dark:border-gray-700 flex items-center justify-between">
              <div class="min-w-0">
                <div class="text-base font-semibold truncate">
                  {baseName(viewFile()!.original_path)}
                </div>
                <div class="text-xs text-gray-500 font-mono truncate">
                  {viewFile()!.original_path}
                </div>
              </div>
              <button
                class="btn btn-secondary text-sm"
                onClick={() => setViewFile(null)}
              >
                Close
              </button>
            </div>
            <div class="p-4 overflow-y-auto flex-1">
              <Show
                when={extractions()[viewFile()!.id]?.raw_text}
                fallback={
                  <p class="text-sm text-gray-500">
                    No extracted text available for this file.
                  </p>
                }
              >
                <pre class="text-xs whitespace-pre-wrap font-mono text-gray-800 dark:text-gray-200">
                  {extractions()[viewFile()!.id]!.raw_text}
                </pre>
              </Show>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default DocumentsTab;
