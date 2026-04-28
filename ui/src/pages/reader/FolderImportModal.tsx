import { Component, createSignal, For, Show, Accessor } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

// ---- Types ----

interface FolderFileCandidate {
  path: string;
  name: string;
  extension: string;
  size: number;
  already_imported: boolean;
}

interface Collection {
  id: string;
  name: string;
  description?: string;
  color: string;
  book_count: number;
  created_at: string;
}

// ---- Helpers ----

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kb = bytes / 1024;
  if (kb < 1024) return `${kb.toFixed(1)} KB`;
  const mb = kb / 1024;
  if (mb < 1024) return `${mb.toFixed(1)} MB`;
  return `${(mb / 1024).toFixed(2)} GB`;
}

// ---- Imperative API exposed to parent ----

export interface FolderImportModalApi {
  open: (folderPath: string, presetCollectionId?: string) => Promise<void>;
}

// ---- Props ----

interface FolderImportModalProps {
  collections: Accessor<Collection[]>;
  onImportComplete: () => void;
  onGeneratePdfThumbnails: () => void;
  apiRef: (api: FolderImportModalApi) => void;
}

// ---- Component ----

const FolderImportModal: Component<FolderImportModalProps> = (props) => {
  const [show, setShow] = createSignal(false);
  const [folderPath, setFolderPath] = createSignal('');
  const [candidates, setCandidates] = createSignal<FolderFileCandidate[]>([]);
  const [selected, setSelected] = createSignal<Set<string>>(new Set<string>());
  const [loading, setLoading] = createSignal(false);
  const [filter, setFilter] = createSignal('');
  const [targetCollection, setTargetCollection] = createSignal<string>('');
  const [importing, setImporting] = createSignal(false);

  // Expose imperative API to parent
  props.apiRef({
    open: async (path: string, presetCollectionId?: string) => {
      setFolderPath(path);
      setShow(true);
      setLoading(true);
      setCandidates([]);
      setSelected(new Set<string>());
      setFilter('');
      setTargetCollection(presetCollectionId ?? '');
      try {
        const files = await invoke<FolderFileCandidate[]>('reader_list_folder_files', { path });
        setCandidates(files);
        // Pre-select all files that aren't already imported
        const preSelected = new Set(
          files.filter((f) => !f.already_imported).map((f) => f.path)
        );
        setSelected(preSelected);
      } catch (e) {
        console.error('Failed to list folder files:', e);
        alert(`Failed to scan folder: ${e}`);
        setShow(false);
      } finally {
        setLoading(false);
      }
    },
  });

  // ---- Local helpers ----

  const filteredCandidates = () => {
    const q = filter().trim().toLowerCase();
    if (!q) return candidates();
    return candidates().filter(
      (c) => c.name.toLowerCase().includes(q) || c.extension.toLowerCase().includes(q)
    );
  };

  const toggleSelection = (path: string) => {
    const next = new Set<string>(selected());
    if (next.has(path)) next.delete(path);
    else next.add(path);
    setSelected(next);
  };

  const selectAll = () => {
    setSelected(new Set<string>(
      candidates().filter((c) => !c.already_imported).map((c) => c.path)
    ));
  };

  const deselectAll = () => {
    setSelected(new Set<string>());
  };

  const confirmImport = async () => {
    const paths = Array.from(selected());
    if (paths.length === 0) {
      alert('No files selected');
      return;
    }
    setImporting(true);
    try {
      const result = await invoke<{ imported: number; skipped: number; failed: number }>(
        'reader_import_paths',
        { paths, collectionId: targetCollection() || null }
      );
      props.onImportComplete();
      props.onGeneratePdfThumbnails();
      setShow(false);
      alert(
        `Imported: ${result.imported}\nSkipped (already exists): ${result.skipped}\nFailed: ${result.failed}`
      );
    } catch (e) {
      alert(`Import failed: ${e}`);
    } finally {
      setImporting(false);
    }
  };

  // ---- Render ----

  return (
    <Show when={show()}>
      <div
        class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm"
        onClick={(e) => {
          if (e.target === e.currentTarget) setShow(false);
        }}
      >
        <div class="card w-full max-w-3xl max-h-[85vh] flex flex-col shadow-2xl">
          {/* Modal header */}
          <div class="p-5 border-b border-gray-200 dark:border-gray-700">
            <div class="flex items-start justify-between mb-2">
              <div>
                <h2 class="text-xl font-bold">Import Books from Folder</h2>
                <p
                  class="text-sm text-gray-500 dark:text-gray-400 mt-1 truncate max-w-xl"
                  title={folderPath()}
                >
                  {folderPath()}
                </p>
              </div>
              <button
                class="p-1 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
                onClick={() => setShow(false)}
              >
                <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    stroke-width="2"
                    d="M6 18L18 6M6 6l12 12"
                  />
                </svg>
              </button>
            </div>

            <Show when={!loading() && candidates().length > 0}>
              <div class="flex items-center gap-2 flex-wrap">
                <input
                  type="text"
                  class="input text-sm flex-1 min-w-[180px]"
                  placeholder="Filter by filename or extension..."
                  value={filter()}
                  onInput={(e) => setFilter(e.currentTarget.value)}
                />
                <button class="btn btn-secondary text-xs" onClick={selectAll}>
                  Select All
                </button>
                <button class="btn btn-secondary text-xs" onClick={deselectAll}>
                  Clear
                </button>
                <span class="text-xs text-gray-500 ml-1">
                  {selected().size} / {candidates().filter((c) => !c.already_imported).length} selected
                </span>
              </div>
            </Show>
          </div>

          {/* Modal body: file list */}
          <div class="flex-1 overflow-auto p-2">
            <Show when={loading()}>
              <div class="text-center py-12 text-gray-500">
                <div
                  class="w-8 h-8 mx-auto mb-3 rounded-full border-2 border-gray-200 dark:border-gray-700 border-t-minion-500"
                  style={{ animation: 'spin 1s linear infinite' }}
                />
                Scanning folder...
              </div>
            </Show>

            <Show when={!loading() && candidates().length === 0}>
              <div class="text-center py-12 text-gray-500">
                <p>No supported book files found in this folder.</p>
                <p class="text-xs mt-1">Supported: EPUB, PDF, MOBI, TXT, MD, HTML</p>
              </div>
            </Show>

            <Show when={!loading() && candidates().length > 0}>
              <div class="space-y-0.5">
                <For each={filteredCandidates()}>
                  {(file) => (
                    <label
                      class="flex items-center gap-3 px-3 py-2 rounded hover:bg-gray-50 dark:hover:bg-gray-800/50 cursor-pointer"
                      classList={{ 'opacity-50': file.already_imported }}
                    >
                      <input
                        type="checkbox"
                        class="w-4 h-4 rounded"
                        checked={selected().has(file.path)}
                        disabled={file.already_imported}
                        onChange={() => toggleSelection(file.path)}
                      />
                      <span class="text-xs font-mono uppercase bg-gray-200 dark:bg-gray-700 rounded px-1.5 py-0.5 min-w-[44px] text-center">
                        {file.extension}
                      </span>
                      <div class="flex-1 min-w-0">
                        <p class="text-sm truncate" title={file.path}>
                          {file.name}
                        </p>
                        <Show when={file.already_imported}>
                          <p class="text-xs text-amber-600 dark:text-amber-400">Already in library</p>
                        </Show>
                      </div>
                      <span class="text-xs text-gray-500 dark:text-gray-400 whitespace-nowrap">
                        {formatBytes(file.size)}
                      </span>
                    </label>
                  )}
                </For>
              </div>
            </Show>
          </div>

          {/* Modal footer */}
          <div class="p-4 border-t border-gray-200 dark:border-gray-700 bg-gray-50/50 dark:bg-gray-800/30">
            <div class="flex items-center gap-3 mb-3">
              <label class="text-sm text-gray-600 dark:text-gray-300 whitespace-nowrap">
                Add to collection:
              </label>
              <select
                class="input text-sm flex-1"
                value={targetCollection()}
                onChange={(e) => setTargetCollection(e.currentTarget.value)}
              >
                <option value="">— None —</option>
                <For each={props.collections()}>
                  {(col) => <option value={col.id}>{col.name}</option>}
                </For>
              </select>
            </div>
            <div class="flex justify-end gap-2">
              <button
                class="btn btn-secondary text-sm"
                onClick={() => setShow(false)}
                disabled={importing()}
              >
                Cancel
              </button>
              <button
                class="btn btn-primary text-sm"
                onClick={confirmImport}
                disabled={importing() || selected().size === 0}
              >
                {importing()
                  ? 'Importing...'
                  : `Import ${selected().size} file${selected().size === 1 ? '' : 's'}`}
              </button>
            </div>
          </div>
        </div>
      </div>
    </Show>
  );
};

export { FolderImportModal };
export type { FolderImportModalProps, FolderFileCandidate, Collection };
