import { Component, createSignal, For, Show, onCleanup } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

interface FileInfo {
  path: string;
  name: string;
  size: number;
  modified: string;
  extension?: string;
}

interface DuplicateGroup {
  id: string;
  match_type: string;
  match_label: string;
  file_count: number;
  total_size: number;
  wasted_space: number;
  files: FileInfo[];
  hash?: string;
}

interface ScanProgress {
  task_id: string;
  status: string;
  files_scanned: number;
  total_files: number | null;
  progress_percent: number;
}

interface StorageAnalytics {
  total_files: number;
  total_size: number;
  by_extension: { extension: string; count: number; size: number }[];
  duplicates_found: number;
  duplicate_size: number;
}

interface ScanDir {
  id: string;
  path: string;
}

const Files: Component = () => {
  const [duplicates, setDuplicates] = createSignal<DuplicateGroup[]>([]);
  const [analytics, setAnalytics] = createSignal<StorageAnalytics | null>(null);
  const [scanning, setScanning] = createSignal(false);
  const [scanProgress, setScanProgress] = createSignal<ScanProgress | null>(null);
  const [activeTab, setActiveTab] = createSignal<'scan' | 'duplicates' | 'analytics'>('scan');
  const [expandedGroup, setExpandedGroup] = createSignal<string | null>(null);
  const [scanElapsed, setScanElapsed] = createSignal(0);
  const [bulkOperating, setBulkOperating] = createSignal(false);
  const [operationResult, setOperationResult] = createSignal<{ succeeded: number; failed: number; freed_bytes: number; action: string } | null>(null);

  const openFile = async (path: string) => {
    try {
      await invoke('files_open_file', { path });
    } catch (e) {
      console.error('Failed to open file:', e);
      alert(`Failed to open: ${e}`);
    }
  };

  const getAllCopyPaths = () => {
    const paths: string[] = [];
    for (const group of duplicates()) {
      // Skip first file (original), collect the rest (copies)
      for (let i = 1; i < group.files.length; i++) {
        paths.push(group.files[i].path);
      }
    }
    return paths;
  };

  const getGroupCopyPaths = (groupId: string) => {
    const group = duplicates().find((g) => g.id === groupId);
    if (!group) return [];
    return group.files.slice(1).map((f) => f.path);
  };

  const bulkDeleteAll = async () => {
    const paths = getAllCopyPaths();
    if (paths.length === 0) return;
    if (!confirm(`Delete ${paths.length} duplicate files? This cannot be undone.`)) return;

    setBulkOperating(true);
    setOperationResult(null);
    try {
      const result = await invoke<{ succeeded: number; failed: number; freed_bytes: number; errors: string[] }>('files_bulk_delete', { request: { paths } });
      setOperationResult({ ...result, action: 'deleted' });
      if (result.succeeded > 0) await loadDuplicates();
    } catch (e) {
      alert(`Error: ${e}`);
    } finally {
      setBulkOperating(false);
    }
  };

  const bulkDeleteGroup = async (groupId: string) => {
    const paths = getGroupCopyPaths(groupId);
    if (paths.length === 0) return;
    if (!confirm(`Delete ${paths.length} duplicate file${paths.length > 1 ? 's' : ''} in this group?`)) return;

    setBulkOperating(true);
    setOperationResult(null);
    try {
      const result = await invoke<{ succeeded: number; failed: number; freed_bytes: number; errors: string[] }>('files_bulk_delete', { request: { paths } });
      setOperationResult({ ...result, action: 'deleted' });
      if (result.succeeded > 0) await loadDuplicates();
    } catch (e) {
      alert(`Error: ${e}`);
    } finally {
      setBulkOperating(false);
    }
  };

  const bulkMoveAll = async () => {
    const paths = getAllCopyPaths();
    if (paths.length === 0) return;

    try {
      const dest = await open({ directory: true, multiple: false, title: 'Select destination for duplicate files' });
      if (!dest || typeof dest !== 'string') return;

      setBulkOperating(true);
      setOperationResult(null);
      const result = await invoke<{ succeeded: number; failed: number; freed_bytes: number; errors: string[] }>('files_bulk_move', { request: { paths, destination: dest } });
      setOperationResult({ ...result, action: 'moved' });
      if (result.succeeded > 0) await loadDuplicates();
    } catch (e) {
      alert(`Error: ${e}`);
    } finally {
      setBulkOperating(false);
    }
  };

  const bulkMoveGroup = async (groupId: string) => {
    const paths = getGroupCopyPaths(groupId);
    if (paths.length === 0) return;

    try {
      const dest = await open({ directory: true, multiple: false, title: 'Select destination' });
      if (!dest || typeof dest !== 'string') return;

      setBulkOperating(true);
      setOperationResult(null);
      const result = await invoke<{ succeeded: number; failed: number; freed_bytes: number; errors: string[] }>('files_bulk_move', { request: { paths, destination: dest } });
      setOperationResult({ ...result, action: 'moved' });
      if (result.succeeded > 0) await loadDuplicates();
    } catch (e) {
      alert(`Error: ${e}`);
    } finally {
      setBulkOperating(false);
    }
  };

  // Multi-directory state
  const [scanDirs, setScanDirs] = createSignal<ScanDir[]>([]);
  const [newPath, setNewPath] = createSignal('');

  // Persistent intervals - these survive tab switches
  let pollInterval: ReturnType<typeof setInterval> | undefined;
  let timerInterval: ReturnType<typeof setInterval> | undefined;

  // Only cleanup on component unmount (page navigation away), not tab switches
  onCleanup(() => {
    if (pollInterval) clearInterval(pollInterval);
    if (timerInterval) clearInterval(timerInterval);
  });

  const browseForDirectory = async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (selected && typeof selected === 'string') {
        setNewPath(selected);
      }
    } catch (e) {
      console.error('Failed to open folder dialog:', e);
    }
  };

  const addScanDir = () => {
    const path = newPath().trim();
    if (!path) return;
    // Don't add duplicates
    if (scanDirs().some((d) => d.path === path)) return;
    setScanDirs([...scanDirs(), { id: crypto.randomUUID(), path }]);
    setNewPath('');
  };

  const removeScanDir = (id: string) => {
    setScanDirs(scanDirs().filter((d) => d.id !== id));
  };

  const browseAndAdd = async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (selected && typeof selected === 'string') {
        if (!scanDirs().some((d) => d.path === selected)) {
          setScanDirs([...scanDirs(), { id: crypto.randomUUID(), path: selected }]);
        }
      }
    } catch (e) {
      console.error('Failed to open folder dialog:', e);
    }
  };

  const startScan = async () => {
    const paths = scanDirs().map((d) => d.path);

    // If no dirs in list but there's a typed path, use that
    if (paths.length === 0) {
      const typed = newPath().trim();
      if (!typed) {
        alert('Add at least one directory to scan');
        return;
      }
      paths.push(typed);
    }

    setScanning(true);
    setScanProgress(null);
    setScanElapsed(0);

    // Start elapsed timer
    if (timerInterval) clearInterval(timerInterval);
    timerInterval = setInterval(() => setScanElapsed((p) => p + 1), 1000);

    try {
      let progress: ScanProgress;

      if (paths.length === 1) {
        progress = await invoke<ScanProgress>('files_start_scan', { path: paths[0] });
      } else {
        progress = await invoke<ScanProgress>('files_start_multi_scan', { paths });
      }
      setScanProgress(progress);

      // Poll for progress
      if (pollInterval) clearInterval(pollInterval);
      pollInterval = setInterval(async () => {
        try {
          const updated = await invoke<ScanProgress>('files_get_scan_progress', {
            taskId: progress.task_id,
          });
          setScanProgress(updated);

          if (updated.status === 'completed' || updated.status.startsWith('failed')) {
            clearInterval(pollInterval);
            if (timerInterval) clearInterval(timerInterval);
            setScanning(false);

            if (updated.status === 'completed') {
              await loadDuplicates();
              await loadAnalytics();
            }
          }
        } catch (e) {
          console.error('Failed to get progress:', e);
        }
      }, 500);
    } catch (e) {
      console.error('Failed to start scan:', e);
      alert(`Error: ${e}`);
      setScanning(false);
      if (timerInterval) clearInterval(timerInterval);
    }
  };

  const loadDuplicates = async () => {
    try {
      const groups = await invoke<DuplicateGroup[]>('files_list_duplicates', {});
      setDuplicates(groups);
    } catch (e) {
      console.error('Failed to load duplicates:', e);
    }
  };

  const loadAnalytics = async () => {
    try {
      const data = await invoke<StorageAnalytics>('files_get_analytics', {});
      setAnalytics(data);
    } catch (e) {
      console.error('Failed to load analytics:', e);
    }
  };

  const formatSize = (bytes: number) => {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  const formatTime = (seconds: number) => {
    if (seconds < 60) return `${seconds}s`;
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return `${m}m ${s}s`;
  };

  const toggleGroup = (id: string) => {
    setExpandedGroup(expandedGroup() === id ? null : id);
  };

  return (
    <div class="p-6">
      <div class="flex items-center justify-between mb-6">
        <h1 class="text-2xl font-bold">File Intelligence</h1>
        <Show when={scanning()}>
          <div class="flex items-center gap-2 text-sm text-minion-600 dark:text-minion-400">
            <div class="w-4 h-4 rounded-full border-2 border-minion-200 border-t-minion-500" style={{ animation: 'spin 1s linear infinite' }} />
            Scanning... {scanProgress()?.files_scanned?.toLocaleString() ?? 0} files
          </div>
        </Show>
      </div>

      {/* Tabs */}
      <div class="flex border-b border-gray-200 dark:border-gray-700 mb-6">
        <button
          class="px-4 py-2 -mb-px border-b-2 transition-colors"
          classList={{
            'border-minion-500 text-minion-600 dark:text-minion-400': activeTab() === 'scan',
            'border-transparent text-gray-500 hover:text-gray-700 dark:hover:text-gray-300': activeTab() !== 'scan',
          }}
          onClick={() => setActiveTab('scan')}
        >
          Scan
          <Show when={scanning()}>
            <span class="ml-1.5 w-2 h-2 rounded-full bg-minion-500 inline-block" style={{ animation: 'pulse 1.5s infinite' }} />
          </Show>
        </button>
        <button
          class="px-4 py-2 -mb-px border-b-2 transition-colors"
          classList={{
            'border-minion-500 text-minion-600 dark:text-minion-400': activeTab() === 'duplicates',
            'border-transparent text-gray-500 hover:text-gray-700 dark:hover:text-gray-300': activeTab() !== 'duplicates',
          }}
          onClick={() => setActiveTab('duplicates')}
        >
          Duplicates ({duplicates().length})
        </button>
        <button
          class="px-4 py-2 -mb-px border-b-2 transition-colors"
          classList={{
            'border-minion-500 text-minion-600 dark:text-minion-400': activeTab() === 'analytics',
            'border-transparent text-gray-500 hover:text-gray-700 dark:hover:text-gray-300': activeTab() !== 'analytics',
          }}
          onClick={() => setActiveTab('analytics')}
        >
          Analytics
        </button>
      </div>

      {/* Scan Tab */}
      <Show when={activeTab() === 'scan'}>
        {/* Directory List */}
        <div class="card p-6 mb-4">
          <h3 class="font-medium mb-3">Directories to Compare</h3>
          <p class="text-xs text-gray-400 mb-4">
            Add one or more directories. Files across all directories will be scanned and compared for duplicates.
          </p>

          {/* Add directory input */}
          <div class="flex gap-2 mb-4">
            <input
              type="text"
              class="input flex-1"
              placeholder="Type a path or click Browse..."
              value={newPath()}
              onInput={(e) => setNewPath(e.currentTarget.value)}
              onKeyPress={(e) => {
                if (e.key === 'Enter') addScanDir();
              }}
              disabled={scanning()}
            />
            <button
              class="btn btn-secondary"
              onClick={browseForDirectory}
              disabled={scanning()}
              title="Browse for a folder"
            >
              <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
              </svg>
            </button>
            <button
              class="btn btn-secondary"
              onClick={addScanDir}
              disabled={scanning() || !newPath().trim()}
            >
              Add
            </button>
          </div>

          {/* Quick add button */}
          <button
            class="btn btn-secondary text-sm mb-4 w-full border-dashed border-2"
            onClick={browseAndAdd}
            disabled={scanning()}
          >
            + Browse and Add Directory
          </button>

          {/* Directory list */}
          <Show when={scanDirs().length > 0}>
            <div class="space-y-2 mb-4">
              <For each={scanDirs()}>
                {(dir) => (
                  <div class="flex items-center gap-3 p-3 bg-gray-50 dark:bg-gray-800 rounded-lg">
                    <div class="w-8 h-8 rounded-lg bg-minion-100 dark:bg-minion-900/30 flex items-center justify-center flex-shrink-0">
                      <svg class="w-4 h-4 text-minion-600 dark:text-minion-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                      </svg>
                    </div>
                    <span class="flex-1 text-sm truncate" title={dir.path}>{dir.path}</span>
                    <button
                      class="p-1 text-gray-400 hover:text-red-500 transition-colors"
                      onClick={() => removeScanDir(dir.id)}
                      disabled={scanning()}
                      title="Remove"
                    >
                      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                      </svg>
                    </button>
                  </div>
                )}
              </For>
            </div>
          </Show>

          {/* Scan button */}
          <button
            class="btn btn-primary w-full py-3 text-base"
            onClick={startScan}
            disabled={scanning() || (scanDirs().length === 0 && !newPath().trim())}
          >
            <Show
              when={!scanning()}
              fallback={
                <span class="flex items-center justify-center gap-2">
                  <div class="w-4 h-4 rounded-full border-2 border-white/30 border-t-white" style={{ animation: 'spin 1s linear infinite' }} />
                  Scanning...
                </span>
              }
            >
              {scanDirs().length > 1
                ? `Compare ${scanDirs().length} Directories`
                : 'Scan for Duplicates'}
            </Show>
          </button>
        </div>

        {/* Scanning Progress */}
        <Show when={scanning()}>
          <div class="card p-5 mb-4 border-minion-200 dark:border-minion-800 bg-minion-50/50 dark:bg-minion-900/10">
            <div class="flex items-center gap-3 mb-3">
              <div class="relative flex-shrink-0">
                <div class="w-12 h-12 rounded-full border-3 border-minion-200 dark:border-minion-800" style={{ 'border-width': '3px' }} />
                <div
                  class="absolute inset-0 w-12 h-12 rounded-full border-3 border-transparent border-t-minion-500"
                  style={{ 'border-width': '3px', animation: 'spin 1s linear infinite' }}
                />
              </div>
              <div class="flex-1 min-w-0">
                <p class="font-semibold text-minion-700 dark:text-minion-300">
                  Scanning {scanDirs().length > 1 ? `${scanDirs().length} directories` : 'directory'}...
                </p>
                <p class="text-sm text-gray-500 truncate">
                  {scanDirs().map((d) => d.path.split('/').pop()).join(', ') || newPath()}
                </p>
              </div>
              <div class="text-right flex-shrink-0">
                <p class="text-2xl font-bold text-minion-600 dark:text-minion-400">
                  {scanProgress()?.files_scanned?.toLocaleString() ?? '...'}
                </p>
                <p class="text-xs text-gray-500">files found</p>
              </div>
            </div>

            {/* Progress bar */}
            <div class="w-full bg-minion-100 dark:bg-minion-900/40 rounded-full h-2.5 mb-2 overflow-hidden">
              <Show
                when={scanProgress() && scanProgress()!.progress_percent > 0}
                fallback={
                  <div
                    class="h-2.5 rounded-full bg-minion-400"
                    style={{ width: '30%', animation: 'indeterminate 1.5s ease-in-out infinite' }}
                  />
                }
              >
                <div
                  class="bg-minion-500 h-2.5 rounded-full transition-all duration-500"
                  style={{ width: `${scanProgress()!.progress_percent}%` }}
                />
              </Show>
            </div>

            <div class="flex justify-between text-xs text-gray-500">
              <span>
                Status: <span class="font-medium text-minion-600 dark:text-minion-400">{scanProgress()?.status ?? 'starting'}</span>
              </span>
              <span>{formatTime(scanElapsed())}</span>
              <Show when={scanProgress() && scanProgress()!.progress_percent > 0}>
                <span>{Math.round(scanProgress()!.progress_percent)}%</span>
              </Show>
            </div>
          </div>
        </Show>

        {/* Scan Complete */}
        <Show when={!scanning() && scanProgress()?.status === 'completed'}>
          <div class="card p-5 mb-4 border-green-200 dark:border-green-800 bg-green-50/50 dark:bg-green-900/10">
            <div class="flex items-center gap-3">
              <div class="w-12 h-12 rounded-full bg-green-100 dark:bg-green-800 flex items-center justify-center flex-shrink-0">
                <svg class="w-7 h-7 text-green-600 dark:text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                </svg>
              </div>
              <div class="flex-1">
                <p class="font-semibold text-green-700 dark:text-green-300">Scan complete!</p>
                <p class="text-sm text-green-600/70 dark:text-green-400/70">
                  Found {scanProgress()!.files_scanned.toLocaleString()} files
                  {duplicates().length > 0 && ` with ${duplicates().length} duplicate group${duplicates().length > 1 ? 's' : ''}`}
                  {' '}in {formatTime(scanElapsed())}
                </p>
              </div>
              <Show when={duplicates().length > 0}>
                <button class="btn btn-primary text-sm" onClick={() => setActiveTab('duplicates')}>
                  View Duplicates
                </button>
              </Show>
            </div>
          </div>
        </Show>

        {/* Quick Stats */}
        <Show when={analytics()}>
          <div class="grid grid-cols-2 md:grid-cols-4 gap-4">
            <div class="card p-4 text-center">
              <p class="text-2xl font-bold text-minion-600 dark:text-minion-400">
                {analytics()!.total_files.toLocaleString()}
              </p>
              <p class="text-sm text-gray-500 dark:text-gray-400">Total Files</p>
            </div>
            <div class="card p-4 text-center">
              <p class="text-2xl font-bold text-minion-600 dark:text-minion-400">
                {formatSize(analytics()!.total_size)}
              </p>
              <p class="text-sm text-gray-500 dark:text-gray-400">Total Size</p>
            </div>
            <div class="card p-4 text-center">
              <p class="text-2xl font-bold text-orange-600 dark:text-orange-400">
                {analytics()!.duplicates_found}
              </p>
              <p class="text-sm text-gray-500 dark:text-gray-400">Duplicate Groups</p>
            </div>
            <div class="card p-4 text-center">
              <p class="text-2xl font-bold text-red-600 dark:text-red-400">
                {formatSize(analytics()!.duplicate_size)}
              </p>
              <p class="text-sm text-gray-500 dark:text-gray-400">Wasted Space</p>
            </div>
          </div>
        </Show>
      </Show>

      {/* Duplicates Tab */}
      <Show when={activeTab() === 'duplicates'}>
        <Show
          when={duplicates().length > 0}
          fallback={
            <div class="card p-8 text-center text-gray-500 dark:text-gray-400">
              <svg class="w-16 h-16 mx-auto mb-4 text-gray-300 dark:text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M8 7v8a2 2 0 002 2h6M8 7V5a2 2 0 012-2h4.586a1 1 0 01.707.293l4.414 4.414a1 1 0 01.293.707V15a2 2 0 01-2 2h-2M8 7H6a2 2 0 00-2 2v10a2 2 0 002 2h8a2 2 0 002-2v-2" />
              </svg>
              <p class="text-lg mb-2">No duplicates found</p>
              <p class="text-sm">Run a scan to detect duplicate files</p>
            </div>
          }
        >
          {/* Operation result banner */}
          <Show when={operationResult()}>
            <div class="card p-4 mb-4 border-green-200 dark:border-green-800 bg-green-50 dark:bg-green-900/10">
              <div class="flex items-center gap-2">
                <svg class="w-5 h-5 text-green-600 dark:text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                </svg>
                <span class="text-green-700 dark:text-green-300 font-medium">
                  {operationResult()!.succeeded} file{operationResult()!.succeeded !== 1 ? 's' : ''} {operationResult()!.action} ({formatSize(operationResult()!.freed_bytes)} freed)
                </span>
                <Show when={operationResult()!.failed > 0}>
                  <span class="text-red-500 ml-2">{operationResult()!.failed} failed</span>
                </Show>
                <button class="ml-auto text-gray-400 hover:text-gray-600" onClick={() => setOperationResult(null)}>
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>
            </div>
          </Show>

          {/* Summary bar with bulk actions */}
          <div class="card p-4 mb-4">
            <div class="flex items-center justify-between mb-3">
              <div>
                <span class="font-semibold text-lg">{duplicates().length}</span>
                <span class="text-gray-500 dark:text-gray-400 ml-1">duplicate groups</span>
                <span class="text-gray-400 mx-2">|</span>
                <span class="font-semibold">{duplicates().reduce((sum, g) => sum + g.file_count - 1, 0)}</span>
                <span class="text-gray-500 dark:text-gray-400 ml-1">copies</span>
                <span class="text-gray-400 mx-2">|</span>
                <span class="font-semibold text-red-600 dark:text-red-400">
                  {formatSize(duplicates().reduce((sum, g) => sum + g.wasted_space, 0))}
                </span>
                <span class="text-gray-500 dark:text-gray-400 ml-1">wasted</span>
              </div>
            </div>
            <div class="flex gap-2">
              <button
                class="btn bg-red-600 hover:bg-red-700 text-white text-sm flex-1"
                onClick={bulkDeleteAll}
                disabled={bulkOperating()}
              >
                {bulkOperating() ? 'Working...' : `Delete All Copies (${duplicates().reduce((s, g) => s + g.file_count - 1, 0)} files)`}
              </button>
              <button
                class="btn bg-orange-500 hover:bg-orange-600 text-white text-sm flex-1"
                onClick={bulkMoveAll}
                disabled={bulkOperating()}
              >
                Move All Copies to Folder
              </button>
            </div>
          </div>

          <div class="space-y-3">
            <For each={duplicates()}>
              {(group) => (
                <div class="card overflow-hidden">
                  {/* Group header */}
                  <div
                    class="p-4 cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-800 transition-colors"
                    onClick={() => toggleGroup(group.id)}
                  >
                    <div class="flex items-center justify-between">
                      <div class="flex items-center gap-3">
                        <span class="text-lg transition-transform" classList={{ 'rotate-90': expandedGroup() === group.id }}>
                          ▶
                        </span>
                        <div>
                          <p class="font-medium">
                            {group.files[0]?.name ?? 'Unknown'}
                            <span class="text-gray-400 font-normal ml-1">
                              + {group.file_count - 1} cop{group.file_count > 2 ? 'ies' : 'y'}
                            </span>
                          </p>
                          <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                            {group.match_label} • {formatSize(group.files[0]?.size ?? 0)} each
                          </p>
                        </div>
                      </div>
                      <div class="flex items-center gap-2">
                        {/* Per-group actions */}
                        <button
                          class="px-2 py-1 text-xs rounded bg-red-100 dark:bg-red-900/30 text-red-600 dark:text-red-400 hover:bg-red-200 transition-colors"
                          onClick={(e) => { e.stopPropagation(); bulkDeleteGroup(group.id); }}
                          disabled={bulkOperating()}
                          title="Delete all copies in this group"
                        >
                          Delete copies
                        </button>
                        <button
                          class="px-2 py-1 text-xs rounded bg-orange-100 dark:bg-orange-900/30 text-orange-600 dark:text-orange-400 hover:bg-orange-200 transition-colors"
                          onClick={(e) => { e.stopPropagation(); bulkMoveGroup(group.id); }}
                          disabled={bulkOperating()}
                          title="Move copies to another folder"
                        >
                          Move copies
                        </button>
                        <span class="px-2.5 py-1 bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-400 rounded-full text-xs font-medium">
                          {formatSize(group.wasted_space)}
                        </span>
                      </div>
                    </div>
                  </div>

                  {/* Expanded file list */}
                  <Show when={expandedGroup() === group.id}>
                    <div class="border-t border-gray-200 dark:border-gray-700">
                      {/* Match explanation */}
                      <div class="px-4 py-2 bg-blue-50 dark:bg-blue-900/10 text-xs text-blue-700 dark:text-blue-300 flex items-center gap-2">
                        <svg class="w-4 h-4 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                        </svg>
                        <span>{group.match_label}</span>
                        <Show when={group.hash}>
                          <span class="text-blue-500 dark:text-blue-400 font-mono ml-auto">
                            SHA256: {group.hash!.substring(0, 16)}...
                          </span>
                        </Show>
                      </div>

                      {/* File list */}
                      <div class="bg-gray-50 dark:bg-gray-800/50">
                        <For each={group.files}>
                          {(file, index) => (
                            <div class="px-4 py-3 border-b border-gray-200 dark:border-gray-700 last:border-b-0">
                              <div class="flex items-center gap-3">
                                <div class="w-8 h-8 rounded bg-gray-200 dark:bg-gray-700 flex items-center justify-center flex-shrink-0">
                                  <span class="text-[10px] font-bold text-gray-500 dark:text-gray-400 uppercase">
                                    {file.extension ?? '?'}
                                  </span>
                                </div>
                                <div class="flex-1 min-w-0">
                                  <div class="flex items-center gap-2">
                                    <p class="font-medium text-sm truncate" title={file.name}>{file.name}</p>
                                    <Show when={index() === 0}>
                                      <span class="text-[10px] px-1.5 py-0.5 bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400 rounded font-medium flex-shrink-0">
                                        KEEP
                                      </span>
                                    </Show>
                                    <Show when={index() > 0}>
                                      <span class="text-[10px] px-1.5 py-0.5 bg-orange-100 dark:bg-orange-900/30 text-orange-700 dark:text-orange-400 rounded font-medium flex-shrink-0">
                                        COPY
                                      </span>
                                    </Show>
                                  </div>
                                  <p class="text-xs text-gray-500 dark:text-gray-400 truncate" title={file.path}>
                                    {file.path}
                                  </p>
                                </div>
                                <span class="text-xs text-gray-500 dark:text-gray-400 flex-shrink-0">
                                  {formatSize(file.size)}
                                </span>
                                <button
                                  class="px-2.5 py-1 text-xs rounded bg-minion-100 dark:bg-minion-900/30 text-minion-700 dark:text-minion-300 hover:bg-minion-200 dark:hover:bg-minion-800/50 transition-colors flex-shrink-0"
                                  onClick={(e) => { e.stopPropagation(); openFile(file.path); }}
                                  title="Open with default application"
                                >
                                  Open
                                </button>
                              </div>
                            </div>
                          )}
                        </For>
                      </div>
                    </div>
                  </Show>
                </div>
              )}
            </For>
          </div>
        </Show>
      </Show>

      {/* Analytics Tab */}
      <Show when={activeTab() === 'analytics'}>
        <Show
          when={analytics()}
          fallback={
            <div class="card p-8 text-center text-gray-500 dark:text-gray-400">
              <svg class="w-16 h-16 mx-auto mb-4 text-gray-300 dark:text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z" />
              </svg>
              <p class="text-lg mb-2">No analytics available</p>
              <p class="text-sm">Run a scan to see storage analytics</p>
            </div>
          }
        >
          <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
            <div class="card p-4">
              <h3 class="font-medium mb-4">Storage by File Type</h3>
              <div class="space-y-3">
                <For each={analytics()!.by_extension.sort((a, b) => b.size - a.size).slice(0, 10)}>
                  {(ext) => {
                    const percentage = (ext.size / analytics()!.total_size) * 100;
                    return (
                      <div>
                        <div class="flex justify-between text-sm mb-1">
                          <span class="font-medium">.{ext.extension}</span>
                          <span class="text-gray-500 dark:text-gray-400">
                            {ext.count} files • {formatSize(ext.size)}
                          </span>
                        </div>
                        <div class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2">
                          <div class="bg-minion-500 h-2 rounded-full" style={{ width: `${percentage}%` }} />
                        </div>
                      </div>
                    );
                  }}
                </For>
              </div>
            </div>
            <div class="card p-4">
              <h3 class="font-medium mb-4">Summary</h3>
              <div class="space-y-4">
                <div class="flex justify-between items-center py-2 border-b border-gray-200 dark:border-gray-700">
                  <span class="text-gray-600 dark:text-gray-400">Total Files</span>
                  <span class="font-medium">{analytics()!.total_files.toLocaleString()}</span>
                </div>
                <div class="flex justify-between items-center py-2 border-b border-gray-200 dark:border-gray-700">
                  <span class="text-gray-600 dark:text-gray-400">Total Size</span>
                  <span class="font-medium">{formatSize(analytics()!.total_size)}</span>
                </div>
                <div class="flex justify-between items-center py-2 border-b border-gray-200 dark:border-gray-700">
                  <span class="text-gray-600 dark:text-gray-400">File Types</span>
                  <span class="font-medium">{analytics()!.by_extension.length}</span>
                </div>
                <div class="flex justify-between items-center py-2 border-b border-gray-200 dark:border-gray-700">
                  <span class="text-gray-600 dark:text-gray-400">Duplicate Groups</span>
                  <span class="font-medium text-orange-600 dark:text-orange-400">{analytics()!.duplicates_found}</span>
                </div>
                <div class="flex justify-between items-center py-2">
                  <span class="text-gray-600 dark:text-gray-400">Wasted Space</span>
                  <span class="font-medium text-red-600 dark:text-red-400">{formatSize(analytics()!.duplicate_size)}</span>
                </div>
              </div>
            </div>
          </div>
        </Show>
      </Show>

      <style>{`
        @keyframes spin { to { transform: rotate(360deg); } }
        @keyframes indeterminate { 0% { transform: translateX(-100%); } 100% { transform: translateX(400%); } }
        @keyframes pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.3; } }
      `}</style>
    </div>
  );
};

export default Files;
