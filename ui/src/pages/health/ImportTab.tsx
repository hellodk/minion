import { Component, createSignal, For, Show, onMount, onCleanup } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { open } from '@tauri-apps/plugin-dialog';

interface Patient {
  id: string;
  full_name: string;
  avatar_color?: string;
  [key: string]: any;
}

interface DiscoveredFile {
  path: string;
  size_bytes: number;
  extension: string;
  already_imported: boolean;
}

interface IngestionProgress {
  job_id: string;
  processed: number;
  total: number;
  skipped: number;
  failed: number;
  current?: string;
}

type Step = 'pick' | 'discover' | 'run' | 'done';

function fmtBytes(b: number): string {
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
  if (b < 1024 * 1024 * 1024) return `${(b / (1024 * 1024)).toFixed(1)} MB`;
  return `${(b / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function shortPath(path: string, max = 60): string {
  if (path.length <= max) return path;
  return '…' + path.slice(path.length - (max - 1));
}

const ImportTab: Component<{
  activePatient: Patient;
  onGoToReview?: () => void;
}> = (props) => {
  const [step, setStep] = createSignal<Step>('pick');
  const [folder, setFolder] = createSignal<string>('');
  const [discovering, setDiscovering] = createSignal(false);
  const [discovered, setDiscovered] = createSignal<DiscoveredFile[]>([]);
  const [selected, setSelected] = createSignal<Set<string>>(new Set());
  const [error, setError] = createSignal<string | null>(null);
  const [jobId, setJobId] = createSignal<string | null>(null);
  const [progress, setProgress] = createSignal<IngestionProgress>({
    job_id: '',
    processed: 0,
    total: 0,
    skipped: 0,
    failed: 0,
    current: '',
  });
  const [completed, setCompleted] = createSignal(false);

  let unlistenProgress: UnlistenFn | null = null;
  let unlistenComplete: UnlistenFn | null = null;

  onMount(async () => {
    unlistenProgress = await listen<IngestionProgress>('health-ingestion-progress', (event) => {
      setProgress(event.payload);
    });
    unlistenComplete = await listen<IngestionProgress>('health-ingestion-complete', (event) => {
      setProgress((p) => ({ ...p, ...event.payload }));
      setCompleted(true);
      setStep('done');
    });
  });

  onCleanup(() => {
    if (unlistenProgress) unlistenProgress();
    if (unlistenComplete) unlistenComplete();
  });

  const pickFolder = async () => {
    setError(null);
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select folder with medical documents',
      });
      if (typeof selected === 'string') {
        setFolder(selected);
        await discoverFolder(selected);
      }
    } catch (e) {
      setError(String(e));
    }
  };

  const discoverFolder = async (path: string) => {
    setDiscovering(true);
    setError(null);
    try {
      const files = await invoke<DiscoveredFile[]>('health_discover_folder', {
        folder: path,
      });
      setDiscovered(files);
      // Default: select all that are not already imported
      const initial = new Set(files.filter((f) => !f.already_imported).map((f) => f.path));
      setSelected(initial);
      setStep('discover');
    } catch (e) {
      setError(String(e));
    } finally {
      setDiscovering(false);
    }
  };

  const toggle = (path: string) => {
    const s = new Set(selected());
    if (s.has(path)) s.delete(path);
    else s.add(path);
    setSelected(s);
  };

  const selectAll = () => {
    setSelected(new Set(discovered().map((f) => f.path)));
  };

  const deselectAll = () => {
    setSelected(new Set<string>());
  };

  const selectNewOnly = () => {
    setSelected(new Set(discovered().filter((f) => !f.already_imported).map((f) => f.path)));
  };

  const startImport = async () => {
    setError(null);
    const paths = Array.from(selected());
    if (paths.length === 0) {
      setError('Select at least one file to import.');
      return;
    }
    try {
      const id = await invoke<string>('health_start_ingestion', {
        patientId: props.activePatient.id,
        sourceFolder: folder(),
        selectedPaths: paths,
      });
      setJobId(id);
      setProgress({
        job_id: id,
        processed: 0,
        total: paths.length,
        skipped: 0,
        failed: 0,
        current: '',
      });
      setCompleted(false);
      setStep('run');
    } catch (e) {
      setError(String(e));
    }
  };

  const reset = () => {
    setStep('pick');
    setFolder('');
    setDiscovered([]);
    setSelected(new Set<string>());
    setJobId(null);
    setCompleted(false);
    setProgress({
      job_id: '',
      processed: 0,
      total: 0,
      skipped: 0,
      failed: 0,
      current: '',
    });
    setError(null);
  };

  const progressPct = () => {
    const p = progress();
    if (p.total === 0) return 0;
    return Math.round((p.processed / p.total) * 100);
  };

  return (
    <div>
      <div class="flex items-center justify-between mb-4">
        <h2 class="text-lg font-semibold">Import medical documents</h2>
        <div class="flex items-center gap-2 text-xs text-gray-500">
          <span classList={{ 'font-semibold text-minion-600': step() === 'pick' }}>
            1. Pick folder
          </span>
          <span>→</span>
          <span classList={{ 'font-semibold text-minion-600': step() === 'discover' }}>
            2. Select files
          </span>
          <span>→</span>
          <span
            classList={{
              'font-semibold text-minion-600': step() === 'run' || step() === 'done',
            }}
          >
            3. Run import
          </span>
        </div>
      </div>

      <Show when={error()}>
        <div class="mb-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
          <p class="text-sm text-red-700 dark:text-red-300">{error()}</p>
        </div>
      </Show>

      {/* Step 1: Pick folder */}
      <Show when={step() === 'pick'}>
        <div class="card p-6 text-center">
          <div class="mb-4">
            <svg
              class="w-16 h-16 mx-auto text-gray-400"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
              />
            </svg>
          </div>
          <p class="text-sm text-gray-600 dark:text-gray-300 mb-4">
            Select a folder of medical documents (PDFs, images, text files). All files are
            hashed and checked for duplicates before being copied into your encrypted vault.
          </p>
          <button class="btn btn-primary" onClick={pickFolder} disabled={discovering()}>
            {discovering() ? 'Scanning…' : 'Pick folder'}
          </button>
        </div>
      </Show>

      {/* Step 2: Discover & select */}
      <Show when={step() === 'discover'}>
        <div class="card p-4 mb-4">
          <div class="flex items-center justify-between mb-3">
            <div>
              <div class="text-xs text-gray-500">Source folder</div>
              <div class="text-sm font-mono truncate" title={folder()}>
                {folder()}
              </div>
            </div>
            <button class="btn btn-secondary text-xs" onClick={reset}>
              Change folder
            </button>
          </div>
          <div class="flex flex-wrap gap-2 mb-3 text-xs">
            <span class="px-2 py-1 bg-gray-100 dark:bg-gray-800 rounded">
              Found: <strong>{discovered().length}</strong>
            </span>
            <span class="px-2 py-1 bg-gray-100 dark:bg-gray-800 rounded">
              Already imported:{' '}
              <strong>{discovered().filter((f) => f.already_imported).length}</strong>
            </span>
            <span class="px-2 py-1 bg-minion-50 dark:bg-minion-900/30 text-minion-700 dark:text-minion-300 rounded">
              Selected: <strong>{selected().size}</strong>
            </span>
          </div>
          <div class="flex gap-2 mb-3">
            <button class="btn btn-secondary text-xs" onClick={selectAll}>
              Select all
            </button>
            <button class="btn btn-secondary text-xs" onClick={deselectAll}>
              Deselect all
            </button>
            <button class="btn btn-secondary text-xs" onClick={selectNewOnly}>
              Only new
            </button>
          </div>
          <div class="overflow-x-auto max-h-96 overflow-y-auto border border-gray-200 dark:border-gray-700 rounded">
            <table class="w-full text-sm">
              <thead class="bg-gray-50 dark:bg-gray-800 sticky top-0">
                <tr>
                  <th class="w-10 p-2"></th>
                  <th class="text-left p-2">Path</th>
                  <th class="text-right p-2">Size</th>
                  <th class="text-left p-2">Ext</th>
                  <th class="text-left p-2">Status</th>
                </tr>
              </thead>
              <tbody>
                <For each={discovered()}>
                  {(f) => (
                    <tr class="border-t border-gray-100 dark:border-gray-800 hover:bg-gray-50 dark:hover:bg-gray-800/50">
                      <td class="p-2">
                        <input
                          type="checkbox"
                          checked={selected().has(f.path)}
                          onChange={() => toggle(f.path)}
                        />
                      </td>
                      <td class="p-2 font-mono text-xs" title={f.path}>
                        {shortPath(f.path, 70)}
                      </td>
                      <td class="p-2 text-right text-xs text-gray-500">
                        {fmtBytes(f.size_bytes)}
                      </td>
                      <td class="p-2 text-xs uppercase text-gray-500">{f.extension}</td>
                      <td class="p-2">
                        <Show
                          when={f.already_imported}
                          fallback={
                            <span class="text-xs text-green-600 dark:text-green-400">
                              New
                            </span>
                          }
                        >
                          <span class="text-xs px-1.5 py-0.5 bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300 rounded">
                            Already imported
                          </span>
                        </Show>
                      </td>
                    </tr>
                  )}
                </For>
                <Show when={discovered().length === 0}>
                  <tr>
                    <td colspan="5" class="p-6 text-center text-sm text-gray-500">
                      No supported files found in this folder.
                    </td>
                  </tr>
                </Show>
              </tbody>
            </table>
          </div>
          <div class="flex justify-end gap-2 mt-3">
            <button class="btn btn-secondary" onClick={reset}>
              Back
            </button>
            <button
              class="btn btn-primary"
              onClick={startImport}
              disabled={selected().size === 0}
            >
              Import {selected().size} file{selected().size === 1 ? '' : 's'}
            </button>
          </div>
        </div>
      </Show>

      {/* Step 3: Running import */}
      <Show when={step() === 'run'}>
        <div class="card p-6">
          <h3 class="text-base font-semibold mb-2">Importing…</h3>
          <p class="text-xs text-gray-500 mb-4">
            Job ID: <span class="font-mono">{jobId()}</span>
          </p>
          <div class="mb-4">
            <div class="flex justify-between text-xs text-gray-500 mb-1">
              <span>
                {progress().processed} / {progress().total}
              </span>
              <span>{progressPct()}%</span>
            </div>
            <div class="h-3 bg-gray-200 dark:bg-gray-700 rounded-full overflow-hidden">
              <div
                class="h-full bg-minion-500 transition-all duration-200"
                style={{ width: `${progressPct()}%` }}
              />
            </div>
          </div>
          <div class="grid grid-cols-3 gap-3 mb-4">
            <div class="card p-3">
              <div class="text-xs text-gray-500">Processed</div>
              <div class="text-xl font-bold text-minion-600">{progress().processed}</div>
            </div>
            <div class="card p-3">
              <div class="text-xs text-gray-500">Skipped</div>
              <div class="text-xl font-bold text-amber-600">{progress().skipped}</div>
            </div>
            <div class="card p-3">
              <div class="text-xs text-gray-500">Failed</div>
              <div class="text-xl font-bold text-red-600">{progress().failed}</div>
            </div>
          </div>
          <Show when={progress().current}>
            <div class="text-xs text-gray-500 mb-1">Current file:</div>
            <div class="text-sm font-mono truncate" title={progress().current}>
              {shortPath(progress().current || '', 80)}
            </div>
          </Show>
        </div>
      </Show>

      {/* Step 4: Done */}
      <Show when={step() === 'done'}>
        <div class="card p-6 text-center">
          <div class="mb-4">
            <svg
              class="w-16 h-16 mx-auto text-green-500"
              fill="none"
              stroke="currentColor"
              viewBox="0 0 24 24"
            >
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M5 13l4 4L19 7"
              />
            </svg>
          </div>
          <h3 class="text-lg font-bold mb-2">Import complete</h3>
          <div class="grid grid-cols-3 gap-3 my-4 max-w-md mx-auto">
            <div class="card p-3">
              <div class="text-xs text-gray-500">Processed</div>
              <div class="text-xl font-bold text-minion-600">{progress().processed}</div>
            </div>
            <div class="card p-3">
              <div class="text-xs text-gray-500">Skipped</div>
              <div class="text-xl font-bold text-amber-600">{progress().skipped}</div>
            </div>
            <div class="card p-3">
              <div class="text-xs text-gray-500">Failed</div>
              <div class="text-xl font-bold text-red-600">{progress().failed}</div>
            </div>
          </div>
          <div class="flex gap-2 justify-center mt-6">
            <button class="btn btn-secondary" onClick={reset}>
              Import more
            </button>
            <Show when={props.onGoToReview}>
              <button class="btn btn-primary" onClick={() => props.onGoToReview?.()}>
                Go to Review →
              </button>
            </Show>
          </div>
          <Show when={completed()}>
            <p class="text-xs text-gray-500 mt-4">
              Files are in your encrypted vault. Next, the AI will classify each document and
              extract structured fields. Review the extractions in the <strong>Review</strong> tab.
            </p>
          </Show>
        </div>
      </Show>
    </div>
  );
};

export default ImportTab;
