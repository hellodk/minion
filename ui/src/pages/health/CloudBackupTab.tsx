import { Component, createSignal, Show, onMount } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';

interface DriveSyncStatus {
  enabled: boolean;
  connected: boolean;
  passphrase_set: boolean;
  last_synced_at: string | null;
  last_remote_etag: string | null;
  error: string | null;
  remote_file_id: string | null;
  client_id_set: boolean;
}

interface RestoreSummary {
  patients: number;
  records: number;
  labs: number;
  medications: number;
  conditions: number;
  vitals: number;
  family_history: number;
  life_events: number;
  symptoms: number;
  entities: number;
  episodes: number;
}

interface BackupResult {
  bytes_uploaded: number;
  remote_file_id: string;
  at: string;
}

function fmtBytes(b: number): string {
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
  if (b < 1024 * 1024 * 1024) return `${(b / (1024 * 1024)).toFixed(1)} MB`;
  return `${(b / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function fmtTimestamp(s: string | null): string {
  if (!s) return '—';
  try {
    return new Date(s).toLocaleString();
  } catch {
    return s;
  }
}

const CloudBackupTab: Component = () => {
  const [status, setStatus] = createSignal<DriveSyncStatus | null>(null);
  const [loadingStatus, setLoadingStatus] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [toast, setToast] = createSignal<string | null>(null);

  const [clientId, setClientId] = createSignal('');
  const [clientSecret, setClientSecret] = createSignal('');
  const [savingClientId, setSavingClientId] = createSignal(false);

  const [passphrase, setPassphrase] = createSignal('');
  const [passphraseConfirm, setPassphraseConfirm] = createSignal('');
  const [savingPassphrase, setSavingPassphrase] = createSignal(false);

  const [connecting, setConnecting] = createSignal(false);
  const [disconnecting, setDisconnecting] = createSignal(false);

  const [backupPassphrase, setBackupPassphrase] = createSignal('');
  const [backingUp, setBackingUp] = createSignal(false);

  const [restorePassphrase, setRestorePassphrase] = createSignal('');
  const [restoring, setRestoring] = createSignal(false);
  const [restoreSummary, setRestoreSummary] = createSignal<RestoreSummary | null>(null);

  const [localOpen, setLocalOpen] = createSignal(false);
  const [localPassphrase, setLocalPassphrase] = createSignal('');
  const [exporting, setExporting] = createSignal(false);
  const [importing, setImporting] = createSignal(false);
  const [localSummary, setLocalSummary] = createSignal<RestoreSummary | null>(null);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 5000);
  };

  const loadStatus = async () => {
    setLoadingStatus(true);
    setError(null);
    try {
      const st = await invoke<DriveSyncStatus>('health_drive_status', {});
      setStatus(st);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoadingStatus(false);
    }
  };

  onMount(() => {
    loadStatus();
  });

  const saveClientId = async () => {
    if (!clientId().trim()) {
      setError('Client ID is required.');
      return;
    }
    setSavingClientId(true);
    setError(null);
    try {
      await invoke<void>('health_drive_save_client_id', {
        client_id: clientId().trim(),
        client_secret: clientSecret().trim() || undefined,
      });
      setClientId('');
      setClientSecret('');
      showToast('OAuth client saved');
      loadStatus();
    } catch (e) {
      setError(String(e));
    } finally {
      setSavingClientId(false);
    }
  };

  const savePassphrase = async () => {
    if (passphrase().length < 8) {
      setError('Passphrase must be at least 8 characters.');
      return;
    }
    if (passphrase() !== passphraseConfirm()) {
      setError('Passphrase confirmation does not match.');
      return;
    }
    setSavingPassphrase(true);
    setError(null);
    try {
      await invoke<void>('health_drive_set_passphrase', { passphrase: passphrase() });
      setPassphrase('');
      setPassphraseConfirm('');
      showToast('Encryption passphrase set');
      loadStatus();
    } catch (e) {
      setError(String(e));
    } finally {
      setSavingPassphrase(false);
    }
  };

  const connect = async () => {
    setConnecting(true);
    setError(null);
    try {
      await invoke<void>('health_drive_connect', {});
      showToast('Connected to Google Drive');
      loadStatus();
    } catch (e) {
      setError(String(e));
    } finally {
      setConnecting(false);
    }
  };

  const disconnect = async () => {
    setDisconnecting(true);
    setError(null);
    try {
      await invoke<void>('health_drive_disconnect', {});
      showToast('Disconnected from Google Drive');
      loadStatus();
    } catch (e) {
      setError(String(e));
    } finally {
      setDisconnecting(false);
    }
  };

  const backupNow = async () => {
    if (!backupPassphrase()) {
      setError('Enter your passphrase to backup.');
      return;
    }
    setBackingUp(true);
    setError(null);
    try {
      const res = await invoke<BackupResult>('health_drive_backup_now', {
        passphrase: backupPassphrase(),
      });
      showToast(`Uploaded ${fmtBytes(res.bytes_uploaded)} to Drive at ${fmtTimestamp(res.at)}`);
      setBackupPassphrase('');
      loadStatus();
    } catch (e) {
      setError(String(e));
    } finally {
      setBackingUp(false);
    }
  };

  const restoreNow = async () => {
    if (!restorePassphrase()) {
      setError('Enter your passphrase to restore.');
      return;
    }
    const ok = window.confirm(
      'Restore will overwrite local rows that share IDs with the backup. Continue?',
    );
    if (!ok) return;
    setRestoring(true);
    setError(null);
    setRestoreSummary(null);
    try {
      const summary = await invoke<RestoreSummary>('health_drive_restore_now', {
        passphrase: restorePassphrase(),
      });
      setRestoreSummary(summary);
      showToast('Restore complete');
      setRestorePassphrase('');
      loadStatus();
    } catch (e) {
      setError(String(e));
    } finally {
      setRestoring(false);
    }
  };

  const exportLocal = async () => {
    if (!localPassphrase()) {
      setError('Enter a passphrase for the local export.');
      return;
    }
    try {
      const outPath = await saveDialog({
        title: 'Export encrypted health backup',
        defaultPath: `minion-health-${new Date().toISOString().slice(0, 10)}.bin`,
      });
      if (!outPath) return;
      setExporting(true);
      setError(null);
      const bytes = await invoke<number>('health_drive_export_local', {
        passphrase: localPassphrase(),
        output_path: outPath,
      });
      showToast(`Exported ${fmtBytes(bytes)} to ${outPath}`);
    } catch (e) {
      setError(String(e));
    } finally {
      setExporting(false);
    }
  };

  const importLocal = async () => {
    if (!localPassphrase()) {
      setError('Enter the backup passphrase to import.');
      return;
    }
    try {
      const inPath = await openDialog({
        title: 'Import encrypted health backup',
        multiple: false,
        directory: false,
      });
      if (!inPath || typeof inPath !== 'string') return;
      const ok = window.confirm(
        'Import will overwrite local rows that share IDs with the backup. Continue?',
      );
      if (!ok) return;
      setImporting(true);
      setError(null);
      setLocalSummary(null);
      const summary = await invoke<RestoreSummary>('health_drive_import_local', {
        passphrase: localPassphrase(),
        input_path: inPath,
      });
      setLocalSummary(summary);
      showToast('Local import complete');
    } catch (e) {
      setError(String(e));
    } finally {
      setImporting(false);
    }
  };

  const stepComplete = (n: number): boolean => {
    const s = status();
    if (!s) return false;
    if (n === 1) return s.client_id_set;
    if (n === 2) return s.passphrase_set;
    if (n === 3) return s.connected;
    return false;
  };

  const stepClass = (n: number) =>
    stepComplete(n)
      ? 'border-green-500 bg-green-50 dark:bg-green-900/10'
      : 'border-gray-300 dark:border-gray-700';

  return (
    <div class="space-y-4 max-w-3xl">
      {/* Last synced badge */}
      <div class="flex items-center justify-between">
        <div class="text-sm text-gray-600 dark:text-gray-400">
          Last synced: <span class="font-medium">{fmtTimestamp(status()?.last_synced_at ?? null)}</span>
          <Show when={status()?.remote_file_id}>
            <span class="ml-3 text-xs text-gray-500">
              Remote file: <span class="font-mono">{status()?.remote_file_id}</span>
            </span>
          </Show>
        </div>
        <button
          class="text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
          onClick={loadStatus}
          disabled={loadingStatus()}
        >
          {loadingStatus() ? 'Refreshing…' : 'Refresh'}
        </button>
      </div>

      {/* Error */}
      <Show when={error()}>
        <div class="card p-3 text-sm text-red-700 dark:text-red-400 border-l-4 border-red-500">
          {error()}
        </div>
      </Show>
      <Show when={status()?.error}>
        <div class="card p-3 text-sm text-amber-700 dark:text-amber-400 border-l-4 border-amber-500">
          Previous error: {status()!.error}
        </div>
      </Show>

      {/* Toast */}
      <Show when={toast()}>
        <div class="card p-3 text-sm text-green-700 dark:text-green-400 border-l-4 border-green-500">
          {toast()}
        </div>
      </Show>

      {/* Step 1 — OAuth client id */}
      <div class={`card p-4 border-l-4 ${stepClass(1)}`}>
        <div class="flex items-center justify-between mb-2">
          <div class="font-medium">
            1. Set OAuth client_id
            <Show when={stepComplete(1)}>
              <span class="ml-2 text-xs text-green-700 dark:text-green-400">✓ Set</span>
            </Show>
          </div>
        </div>
        <div class="text-xs text-gray-600 dark:text-gray-400 mb-2">
          Create a Desktop OAuth client at{' '}
          <span class="font-mono">console.cloud.google.com</span>, add{' '}
          <span class="font-mono">http://127.0.0.1:8746/</span> as redirect URI, enable scope{' '}
          <span class="font-mono">https://www.googleapis.com/auth/drive.appdata</span>. Optional:
          paste client_secret if your client requires one.
        </div>
        <div class="flex gap-2">
          <input
            type="text"
            placeholder="Client ID"
            class="flex-1 px-2 py-1 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
            value={clientId()}
            onInput={(e) => setClientId(e.currentTarget.value)}
          />
          <input
            type="text"
            placeholder="Client secret (optional)"
            class="flex-1 px-2 py-1 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
            value={clientSecret()}
            onInput={(e) => setClientSecret(e.currentTarget.value)}
          />
          <button
            class="btn-primary text-sm"
            onClick={saveClientId}
            disabled={savingClientId()}
          >
            {savingClientId() ? 'Saving…' : 'Save'}
          </button>
        </div>
      </div>

      {/* Step 2 — passphrase */}
      <div class={`card p-4 border-l-4 ${stepClass(2)}`}>
        <div class="flex items-center justify-between mb-2">
          <div class="font-medium">
            2. Set encryption passphrase
            <Show when={stepComplete(2)}>
              <span class="ml-2 text-xs text-green-700 dark:text-green-400">✓ Set</span>
            </Show>
          </div>
        </div>
        <div class="text-xs text-gray-600 dark:text-gray-400 mb-2">
          8+ chars. NEVER stored — you'll re-enter on every backup/restore.
        </div>
        <div class="flex gap-2">
          <input
            type="password"
            placeholder="Passphrase"
            class="flex-1 px-2 py-1 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
            value={passphrase()}
            onInput={(e) => setPassphrase(e.currentTarget.value)}
          />
          <input
            type="password"
            placeholder="Confirm"
            class="flex-1 px-2 py-1 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
            value={passphraseConfirm()}
            onInput={(e) => setPassphraseConfirm(e.currentTarget.value)}
          />
          <button
            class="btn-primary text-sm"
            onClick={savePassphrase}
            disabled={savingPassphrase()}
          >
            {savingPassphrase() ? 'Saving…' : 'Save'}
          </button>
        </div>
      </div>

      {/* Step 3 — Connect */}
      <div class={`card p-4 border-l-4 ${stepClass(3)}`}>
        <div class="flex items-center justify-between mb-2">
          <div class="font-medium">
            3. Connect Google Drive
            <Show when={stepComplete(3)}>
              <span class="ml-2 text-xs text-green-700 dark:text-green-400">✓ Connected</span>
            </Show>
          </div>
        </div>
        <div class="text-xs text-gray-600 dark:text-gray-400 mb-2">
          Opens an OAuth window in your browser. Authorize the app to access its appdata folder.
        </div>
        <div class="flex gap-2">
          <Show
            when={status()?.connected}
            fallback={
              <button
                class="btn-primary text-sm"
                onClick={connect}
                disabled={connecting() || !status()?.client_id_set}
              >
                {connecting() ? 'Opening…' : 'Connect'}
              </button>
            }
          >
            <button
              class="px-3 py-1.5 text-sm border border-red-300 dark:border-red-700 text-red-700 dark:text-red-400 rounded hover:bg-red-50 dark:hover:bg-red-900/10"
              onClick={disconnect}
              disabled={disconnecting()}
            >
              {disconnecting() ? 'Disconnecting…' : 'Disconnect'}
            </button>
          </Show>
        </div>
      </div>

      {/* Step 4 — Backup now */}
      <div class="card p-4 border-l-4 border-gray-300 dark:border-gray-700">
        <div class="font-medium mb-2">4. Backup now</div>
        <div class="text-xs text-gray-600 dark:text-gray-400 mb-2">
          Encrypts the health database with your passphrase and uploads to Drive.
        </div>
        <div class="flex gap-2">
          <input
            type="password"
            placeholder="Passphrase"
            class="flex-1 px-2 py-1 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
            value={backupPassphrase()}
            onInput={(e) => setBackupPassphrase(e.currentTarget.value)}
          />
          <button
            class="btn-primary text-sm"
            onClick={backupNow}
            disabled={
              backingUp() ||
              !status()?.connected ||
              !status()?.passphrase_set
            }
          >
            {backingUp() ? 'Uploading…' : 'Backup now'}
          </button>
        </div>
      </div>

      {/* Restore from cloud */}
      <div class="card p-4 space-y-2">
        <div class="font-medium">Restore from cloud</div>
        <div class="text-xs text-amber-700 dark:text-amber-400">
          Warning: Restore will overwrite local rows that share IDs with the backup.
        </div>
        <div class="flex gap-2">
          <input
            type="password"
            placeholder="Passphrase"
            class="flex-1 px-2 py-1 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
            value={restorePassphrase()}
            onInput={(e) => setRestorePassphrase(e.currentTarget.value)}
          />
          <button
            class="px-3 py-1.5 text-sm border border-amber-300 dark:border-amber-700 text-amber-700 dark:text-amber-400 rounded hover:bg-amber-50 dark:hover:bg-amber-900/10"
            onClick={restoreNow}
            disabled={restoring() || !status()?.connected}
          >
            {restoring() ? 'Restoring…' : 'Restore'}
          </button>
        </div>
        <Show when={restoreSummary()}>
          {(s) => <SummaryTable summary={s()} title="Cloud restore summary" />}
        </Show>
      </div>

      {/* Local export/import */}
      <div class="card p-0 overflow-hidden">
        <button
          class="w-full flex items-center justify-between px-4 py-3 text-sm font-medium hover:bg-gray-50 dark:hover:bg-gray-800"
          onClick={() => setLocalOpen(!localOpen())}
        >
          <span>Local export / import (no cloud)</span>
          <span class="text-gray-500">{localOpen() ? '▾' : '▸'}</span>
        </button>
        <Show when={localOpen()}>
          <div class="px-4 py-3 border-t border-gray-200 dark:border-gray-700 space-y-2">
            <div class="text-xs text-gray-600 dark:text-gray-400">
              Encrypt the health database to a local file, or import an encrypted file. Useful for
              portable backups or migrating between machines.
            </div>
            <div class="flex gap-2">
              <input
                type="password"
                placeholder="Passphrase"
                class="flex-1 px-2 py-1 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
                value={localPassphrase()}
                onInput={(e) => setLocalPassphrase(e.currentTarget.value)}
              />
              <button
                class="px-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
                onClick={exportLocal}
                disabled={exporting()}
              >
                {exporting() ? 'Exporting…' : 'Export…'}
              </button>
              <button
                class="px-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
                onClick={importLocal}
                disabled={importing()}
              >
                {importing() ? 'Importing…' : 'Import…'}
              </button>
            </div>
            <Show when={localSummary()}>
              {(s) => <SummaryTable summary={s()} title="Local import summary" />}
            </Show>
          </div>
        </Show>
      </div>
    </div>
  );
};

const SummaryTable: Component<{ summary: RestoreSummary; title: string }> = (props) => {
  const rows: Array<[string, keyof RestoreSummary]> = [
    ['Patients', 'patients'],
    ['Records', 'records'],
    ['Labs', 'labs'],
    ['Medications', 'medications'],
    ['Conditions', 'conditions'],
    ['Vitals', 'vitals'],
    ['Family history', 'family_history'],
    ['Life events', 'life_events'],
    ['Symptoms', 'symptoms'],
    ['Entities', 'entities'],
    ['Episodes', 'episodes'],
  ];
  return (
    <div class="mt-2 rounded border border-gray-200 dark:border-gray-700 overflow-hidden">
      <div class="px-3 py-2 text-xs font-medium bg-gray-50 dark:bg-gray-800">{props.title}</div>
      <table class="w-full text-xs">
        <tbody>
          {rows.map(([label, key]) => (
            <tr class="border-t border-gray-200 dark:border-gray-700">
              <td class="px-3 py-1 text-gray-600 dark:text-gray-400">{label}</td>
              <td class="px-3 py-1 text-right font-mono">{props.summary[key]}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
};

export default CloudBackupTab;
