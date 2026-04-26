import { Component, createSignal, For, Show, onMount } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

interface DiskInfo {
  mount: string;
  used_gb: number;
  total_gb: number;
  read_bps: number;
  write_bps: number;
}

interface DirEntry {
  path: string;
  size_mb: number;
}

interface RedundantEntry {
  path: string;
  size_mb: number;
  category: string;
  description: string;
}

type DiskView = 'explorer' | 'redundant';

const CATEGORY_LABELS: Record<string, string> = {
  node_modules: '📦 node_modules',
  rust_target: '🦀 Rust target',
  python_cache: '🐍 __pycache__',
  gradle_cache: '🐘 Gradle',
  maven_cache: '☕ Maven',
  next_cache: '▲ Next.js',
  nuxt_cache: '💚 Nuxt',
  dist_build: '📤 dist/',
  jest_cache: '🃏 Jest cache',
  cargo_debug: '🦀 Cargo debug',
  venv: '🐍 venv',
  venv2: '🐍 venv',
  ds_store: '🍎 .DS_Store',
};

function fmtSize(mb: number): string {
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
  if (mb >= 1) return `${mb} MB`;
  return '< 1 MB';
}

const DiskTab: Component<{ disks: DiskInfo[] }> = (props) => {
  const [view, setView] = createSignal<DiskView>('explorer');

  // ── Explorer state ────────────────────────────────────────────────────
  const [breadcrumbs, setBreadcrumbs] = createSignal<string[]>([]);
  const [currentPath, setCurrentPath] = createSignal('/home');
  const [entries, setEntries] = createSignal<DirEntry[]>([]);
  const [scanning, setScanning] = createSignal(false);
  const [deleting, setDeleting] = createSignal<string | null>(null);
  const [confirmDelete, setConfirmDelete] = createSignal<DirEntry | null>(null);
  const [deleteError, setDeleteError] = createSignal<string | null>(null);

  // ── Redundant state ───────────────────────────────────────────────────
  const [redundantRoot, setRedundantRoot] = createSignal('/home');
  const [redundant, setRedundant] = createSignal<RedundantEntry[]>([]);
  const [scanning2, setScanning2] = createSignal(false);
  const [selected, setSelected] = createSignal<Set<string>>(new Set());
  const [deleting2, setDeleting2] = createSignal(false);
  const [cleanedMb, setCleanedMb] = createSignal(0);

  // ── Explorer functions ────────────────────────────────────────────────
  const scan = async (path?: string) => {
    const p = path ?? currentPath();
    setScanning(true);
    setDeleteError(null);
    const result = await invoke<DirEntry[]>('sysmon_get_disk_breakdown', { path: p }).catch(() => []);
    setEntries(result);
    setScanning(false);
  };

  const navigateTo = (path: string) => {
    setCurrentPath(path);
    // Build breadcrumbs from path
    const parts = path.split('/').filter(Boolean);
    setBreadcrumbs(['/', ...parts.map((_, i) => '/' + parts.slice(0, i + 1).join('/'))]);
    scan(path);
  };

  const drillInto = (entry: DirEntry) => {
    navigateTo(entry.path);
  };

  const goUp = () => {
    const parts = currentPath().split('/').filter(Boolean);
    if (parts.length === 0) return;
    const parent = parts.length === 1 ? '/' : '/' + parts.slice(0, -1).join('/');
    navigateTo(parent);
  };

  const doDelete = async () => {
    const entry = confirmDelete();
    if (!entry) return;
    setDeleting(entry.path);
    setConfirmDelete(null);
    setDeleteError(null);
    try {
      await invoke('sysmon_delete_path', { path: entry.path });
      await scan();
    } catch (e) {
      setDeleteError(String(e));
    } finally {
      setDeleting(null);
    }
  };

  // ── Redundant functions ───────────────────────────────────────────────
  const scanRedundant = async () => {
    setScanning2(true);
    setSelected(new Set());
    setCleanedMb(0);
    const result = await invoke<RedundantEntry[]>('sysmon_find_redundant', { root: redundantRoot() }).catch(() => []);
    setRedundant(result);
    setScanning2(false);
  };

  const toggleSelect = (path: string) => {
    setSelected(prev => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path); else next.add(path);
      return next;
    });
  };

  const selectAll = () => setSelected(new Set(redundant().map(e => e.path)));
  const selectNone = () => setSelected(new Set());

  const selectedMb = () => redundant()
    .filter(e => selected().has(e.path))
    .reduce((acc, e) => acc + e.size_mb, 0);

  const deleteSelected = async () => {
    setDeleting2(true);
    let freed = 0;
    for (const path of selected()) {
      try {
        await invoke('sysmon_delete_path', { path });
        const entry = redundant().find(e => e.path === path);
        if (entry) freed += entry.size_mb;
      } catch { /* skip protected paths */ }
    }
    setCleanedMb(freed);
    setSelected(new Set());
    await scanRedundant();
    setDeleting2(false);
  };

  onMount(() => scan('/home'));

  return (
    <div>
      {/* Per-mount bars */}
      <div style={{ display: 'flex', 'flex-direction': 'column', gap: '8px', 'margin-bottom': '16px' }}>
        <For each={props.disks}>
          {(d) => {
            const pct = d.total_gb > 0 ? (d.used_gb / d.total_gb) * 100 : 0;
            const color = pct > 90 ? '#ef4444' : pct > 75 ? '#f97316' : '#22c55e';
            return (
              <div>
                <div style={{ display: 'flex', 'justify-content': 'space-between', 'margin-bottom': '3px', 'font-size': '12px' }}>
                  <span style={{ color: '#475569', 'font-weight': '500' }}
                    onClick={() => navigateTo(d.mount)}
                    title="Explore this mount"
                    class="cursor-pointer hover:text-sky-600"
                  >{d.mount}</span>
                  <span style={{ color: '#94a3b8' }}>
                    {d.used_gb.toFixed(1)} / {d.total_gb.toFixed(1)} GB ({pct.toFixed(0)}%)
                  </span>
                </div>
                <div style={{ height: '7px', background: '#f1f5f9', 'border-radius': '4px', overflow: 'hidden' }}>
                  <div style={{ width: `${pct}%`, height: '7px', background: color, 'border-radius': '4px', transition: 'width 0.3s' }} />
                </div>
              </div>
            );
          }}
        </For>
      </div>

      {/* View tabs */}
      <div style={{ display: 'flex', gap: '6px', 'margin-bottom': '12px' }}>
        {(['explorer', 'redundant'] as DiskView[]).map(v => (
          <button
            onClick={() => setView(v)}
            style={{
              padding: '5px 14px', 'border-radius': '6px', 'font-size': '12px',
              'font-weight': '500', cursor: 'pointer', border: 'none',
              background: view() === v ? '#3b82f6' : '#f1f5f9',
              color: view() === v ? '#fff' : '#475569',
            }}
          >
            {v === 'explorer' ? '📁 Explorer' : '🗑 Space Wasters'}
          </button>
        ))}
      </div>

      {/* ── EXPLORER VIEW ──────────────────────────────────────────────── */}
      <Show when={view() === 'explorer'}>
        {/* Breadcrumb + up button */}
        <div style={{ display: 'flex', 'align-items': 'center', gap: '6px', 'margin-bottom': '8px', 'flex-wrap': 'wrap' }}>
          <button
            onClick={goUp}
            disabled={currentPath() === '/'}
            style={{
              padding: '3px 8px', 'border-radius': '4px', 'font-size': '11px',
              cursor: 'pointer', border: '1px solid #e2e8f0', background: '#f8fafc', color: '#475569',
            }}
          >↑ Up</button>
          <div style={{ 'font-size': '11px', color: '#64748b', 'font-family': 'monospace', background: '#f8fafc', padding: '3px 8px', 'border-radius': '4px', 'border': '1px solid #e2e8f0' }}>
            {currentPath()}
          </div>
          <button
            onClick={() => scan()}
            disabled={scanning()}
            style={{
              padding: '3px 8px', 'border-radius': '4px', 'font-size': '11px',
              cursor: 'pointer', border: '1px solid #e2e8f0', background: '#f8fafc', color: '#475569',
            }}
          >{scanning() ? '…' : '↻'}</button>
        </div>

        {/* Custom path input */}
        <div style={{ display: 'flex', gap: '6px', 'margin-bottom': '10px' }}>
          <input
            value={currentPath()}
            onInput={(e) => setCurrentPath(e.currentTarget.value)}
            onKeyDown={(e) => e.key === 'Enter' && navigateTo(currentPath())}
            style={{ flex: '1', padding: '5px 8px', border: '1px solid #e2e8f0', 'border-radius': '6px', 'font-size': '12px', 'font-family': 'monospace' }}
            placeholder="/path/to/scan"
          />
          <button
            onClick={() => navigateTo(currentPath())}
            disabled={scanning()}
            style={{ padding: '5px 12px', background: '#3b82f6', color: '#fff', border: 'none', 'border-radius': '6px', 'font-size': '12px', cursor: 'pointer' }}
          >{scanning() ? 'Scanning…' : 'Go'}</button>
        </div>

        <Show when={deleteError()}>
          <div style={{ background: '#fef2f2', border: '1px solid #fecaca', 'border-radius': '6px', padding: '6px 10px', 'margin-bottom': '8px', 'font-size': '11px', color: '#dc2626' }}>
            {deleteError()}
          </div>
        </Show>

        <div style={{ overflow: 'auto', 'max-height': '320px' }}>
          <For each={entries()}>
            {(entry) => {
              const maxMb = entries()[0]?.size_mb ?? 1;
              const pct = Math.min(100, (entry.size_mb / maxMb) * 100);
              const name = entry.path.split('/').pop() || entry.path;
              const isDeleting = deleting() === entry.path;
              return (
                <div style={{ display: 'flex', 'align-items': 'center', gap: '8px', 'margin-bottom': '5px', padding: '3px 0' }}>
                  {/* Name + drill-in */}
                  <div
                    onClick={() => drillInto(entry)}
                    title={entry.path}
                    style={{
                      'min-width': '140px', 'max-width': '140px', 'font-size': '12px',
                      color: '#1e293b', cursor: 'pointer', overflow: 'hidden',
                      'text-overflow': 'ellipsis', 'white-space': 'nowrap',
                    }}
                    class="hover:text-sky-600"
                  >📁 {name}</div>
                  {/* Bar */}
                  <div style={{ flex: '1', height: '6px', background: '#f1f5f9', 'border-radius': '3px' }}>
                    <div style={{ width: `${pct}%`, height: '6px', background: '#3b82f6', 'border-radius': '3px' }} />
                  </div>
                  {/* Size */}
                  <div style={{ 'font-size': '11px', color: '#64748b', 'min-width': '58px', 'text-align': 'right' }}>
                    {fmtSize(entry.size_mb)}
                  </div>
                  {/* Delete button */}
                  <button
                    disabled={isDeleting}
                    onClick={() => { setDeleteError(null); setConfirmDelete(entry); }}
                    title="Delete"
                    style={{
                      padding: '2px 7px', 'border-radius': '4px', 'font-size': '11px',
                      cursor: 'pointer', border: '1px solid #fecaca',
                      background: '#fef2f2', color: '#ef4444',
                    }}
                  >{isDeleting ? '…' : '🗑'}</button>
                </div>
              );
            }}
          </For>
          <Show when={entries().length === 0 && !scanning()}>
            <div style={{ 'font-size': '12px', color: '#94a3b8', padding: '16px 0', 'text-align': 'center' }}>
              No entries — empty directory or permission denied.
            </div>
          </Show>
        </div>
      </Show>

      {/* ── REDUNDANT VIEW ─────────────────────────────────────────────── */}
      <Show when={view() === 'redundant'}>
        <div style={{ display: 'flex', gap: '6px', 'margin-bottom': '10px', 'align-items': 'center' }}>
          <input
            value={redundantRoot()}
            onInput={(e) => setRedundantRoot(e.currentTarget.value)}
            style={{ flex: '1', padding: '5px 8px', border: '1px solid #e2e8f0', 'border-radius': '6px', 'font-size': '12px', 'font-family': 'monospace' }}
            placeholder="/home or project directory"
          />
          <button
            onClick={scanRedundant}
            disabled={scanning2()}
            style={{ padding: '5px 14px', background: '#3b82f6', color: '#fff', border: 'none', 'border-radius': '6px', 'font-size': '12px', cursor: 'pointer' }}
          >{scanning2() ? 'Scanning…' : 'Scan'}</button>
        </div>

        <Show when={cleanedMb() > 0}>
          <div style={{ background: '#f0fdf4', border: '1px solid #bbf7d0', 'border-radius': '6px', padding: '6px 10px', 'margin-bottom': '8px', 'font-size': '12px', color: '#166534' }}>
            ✓ Freed {fmtSize(cleanedMb())}
          </div>
        </Show>

        <Show when={redundant().length > 0}>
          <div style={{ display: 'flex', 'align-items': 'center', gap: '8px', 'margin-bottom': '8px' }}>
            <button onClick={selectAll} style={{ padding: '3px 8px', 'font-size': '11px', cursor: 'pointer', border: '1px solid #e2e8f0', 'border-radius': '4px', background: '#f8fafc', color: '#475569' }}>Select All</button>
            <button onClick={selectNone} style={{ padding: '3px 8px', 'font-size': '11px', cursor: 'pointer', border: '1px solid #e2e8f0', 'border-radius': '4px', background: '#f8fafc', color: '#475569' }}>None</button>
            <Show when={selected().size > 0}>
              <button
                onClick={deleteSelected}
                disabled={deleting2()}
                style={{
                  padding: '3px 10px', 'font-size': '11px', cursor: 'pointer',
                  border: 'none', 'border-radius': '4px',
                  background: '#ef4444', color: '#fff', 'font-weight': '600',
                }}
              >{deleting2() ? 'Deleting…' : `🗑 Delete ${selected().size} items (${fmtSize(selectedMb())})`}</button>
            </Show>
            <span style={{ 'margin-left': 'auto', 'font-size': '11px', color: '#94a3b8' }}>
              {redundant().length} items · {fmtSize(redundant().reduce((a, e) => a + e.size_mb, 0))} total
            </span>
          </div>

          <div style={{ overflow: 'auto', 'max-height': '340px' }}>
            <For each={redundant()}>
              {(entry) => {
                const isSelected = selected().has(entry.path);
                return (
                  <div
                    onClick={() => toggleSelect(entry.path)}
                    style={{
                      display: 'flex', 'align-items': 'flex-start', gap: '8px',
                      padding: '6px 8px', 'margin-bottom': '3px', cursor: 'pointer',
                      'border-radius': '6px', 'border': '1px solid',
                      'border-color': isSelected ? '#bfdbfe' : '#f1f5f9',
                      background: isSelected ? '#eff6ff' : '#fff',
                    }}
                  >
                    <input type="checkbox" checked={isSelected} style={{ 'margin-top': '1px', cursor: 'pointer' }} />
                    <div style={{ flex: '1', 'min-width': '0' }}>
                      <div style={{ 'font-size': '11px', 'font-weight': '600', color: '#1e293b', 'margin-bottom': '1px' }}>
                        {CATEGORY_LABELS[entry.category] ?? entry.category}
                        <span style={{ 'font-weight': '400', color: '#64748b', 'margin-left': '6px' }}>
                          {fmtSize(entry.size_mb)}
                        </span>
                      </div>
                      <div style={{ 'font-size': '10px', color: '#64748b', overflow: 'hidden', 'text-overflow': 'ellipsis', 'white-space': 'nowrap' }}>
                        {entry.path}
                      </div>
                      <div style={{ 'font-size': '10px', color: '#94a3b8', 'margin-top': '1px' }}>{entry.description}</div>
                    </div>
                  </div>
                );
              }}
            </For>
          </div>
        </Show>

        <Show when={redundant().length === 0 && !scanning2()}>
          <div style={{ 'font-size': '12px', color: '#94a3b8', padding: '24px 0', 'text-align': 'center' }}>
            Click Scan to find space-wasting files (node_modules, build artifacts, caches…)
          </div>
        </Show>
      </Show>

      {/* ── DELETE CONFIRMATION MODAL ───────────────────────────────────── */}
      <Show when={confirmDelete() !== null}>
        <div style={{
          position: 'fixed', inset: '0', 'z-index': '50',
          background: 'rgba(0,0,0,0.4)', display: 'flex',
          'align-items': 'center', 'justify-content': 'center', padding: '16px',
        }}>
          <div style={{ background: '#fff', 'border-radius': '12px', padding: '20px', 'max-width': '400px', width: '100%', 'box-shadow': '0 20px 60px rgba(0,0,0,0.2)' }}>
            <h3 style={{ margin: '0 0 8px', 'font-size': '15px', 'font-weight': '700', color: '#0f172a' }}>
              Delete permanently?
            </h3>
            <p style={{ 'font-size': '12px', color: '#64748b', 'margin-bottom': '12px', 'word-break': 'break-all' }}>
              {confirmDelete()?.path}
            </p>
            <p style={{ 'font-size': '12px', color: '#f97316', 'margin-bottom': '16px' }}>
              {fmtSize(confirmDelete()?.size_mb ?? 0)} will be freed. This cannot be undone.
            </p>
            <div style={{ display: 'flex', gap: '8px', 'justify-content': 'flex-end' }}>
              <button
                onClick={() => setConfirmDelete(null)}
                style={{ padding: '7px 16px', 'border-radius': '6px', 'font-size': '13px', cursor: 'pointer', border: '1px solid #e2e8f0', background: '#f8fafc', color: '#475569' }}
              >Cancel</button>
              <button
                onClick={doDelete}
                style={{ padding: '7px 16px', 'border-radius': '6px', 'font-size': '13px', cursor: 'pointer', border: 'none', background: '#ef4444', color: '#fff', 'font-weight': '600' }}
              >Delete</button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default DiskTab;
