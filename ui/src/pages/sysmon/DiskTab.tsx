import { Component, createSignal, For, onMount } from 'solid-js';
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

const DiskTab: Component<{ disks: DiskInfo[] }> = (props) => {
  const [breakdown, setBreakdown] = createSignal<DirEntry[]>([]);
  const [scanning, setScanning] = createSignal(false);
  const [scanPath, setScanPath] = createSignal('/');

  const scan = async () => {
    setScanning(true);
    const result = await invoke<DirEntry[]>('sysmon_get_disk_breakdown', { path: scanPath() }).catch(() => []);
    setBreakdown(result);
    setScanning(false);
  };

  const drillInto = (path: string) => {
    setScanPath(path);
    scan();
  };

  onMount(scan);

  return (
    <div>
      <h3 style={{ margin: '0 0 12px', color: '#1e293b', 'font-size': '14px', 'font-weight': '600' }}>Disk Usage</h3>

      {/* Per-mount bars */}
      <div style={{ display: 'flex', 'flex-direction': 'column', gap: '10px', 'margin-bottom': '20px' }}>
        <For each={props.disks}>
          {(d) => {
            const pct = d.total_gb > 0 ? (d.used_gb / d.total_gb) * 100 : 0;
            const color = pct > 90 ? '#ef4444' : pct > 75 ? '#f97316' : '#22c55e';
            return (
              <div>
                <div style={{ display: 'flex', 'justify-content': 'space-between', 'margin-bottom': '4px', 'font-size': '12px' }}>
                  <span style={{ color: '#475569', 'font-weight': '500' }}>{d.mount}</span>
                  <span style={{ color: '#94a3b8' }}>
                    {d.used_gb.toFixed(1)} / {d.total_gb.toFixed(1)} GB ({pct.toFixed(0)}%)
                  </span>
                </div>
                <div style={{ height: '8px', background: '#f1f5f9', 'border-radius': '4px', overflow: 'hidden' }}>
                  <div style={{
                    width: `${pct}%`, height: '8px', background: color,
                    'border-radius': '4px', transition: 'width 0.3s',
                  }} />
                </div>
              </div>
            );
          }}
        </For>
      </div>

      {/* Directory breakdown */}
      <div style={{ display: 'flex', gap: '8px', 'margin-bottom': '12px', 'align-items': 'center' }}>
        <input
          value={scanPath()}
          onInput={(e) => setScanPath(e.currentTarget.value)}
          style={{
            flex: '1', padding: '6px 10px', border: '1px solid #e2e8f0',
            'border-radius': '6px', 'font-size': '12px', color: '#1e293b',
          }}
          placeholder="Path to scan..."
        />
        <button
          onClick={scan}
          disabled={scanning()}
          style={{
            padding: '6px 14px', background: '#3b82f6', color: '#fff',
            border: 'none', 'border-radius': '6px', 'font-size': '12px', cursor: 'pointer',
          }}
        >{scanning() ? 'Scanning...' : 'Scan'}</button>
      </div>

      <div style={{ overflow: 'auto', 'max-height': '300px' }}>
        <For each={breakdown()}>
          {(entry) => {
            const maxMb = breakdown()[0]?.size_mb ?? 1;
            const pct = (entry.size_mb / maxMb) * 100;
            return (
              <div
                style={{
                  display: 'flex', 'align-items': 'center', gap: '8px',
                  'margin-bottom': '6px', cursor: 'pointer',
                }}
                onClick={() => drillInto(entry.path)}
              >
                <div style={{
                  'font-size': '12px', color: '#475569', 'min-width': '120px',
                  overflow: 'hidden', 'text-overflow': 'ellipsis', 'white-space': 'nowrap',
                }}>
                  {entry.path.split('/').pop() || entry.path}
                </div>
                <div style={{ flex: '1', height: '6px', background: '#f1f5f9', 'border-radius': '3px' }}>
                  <div style={{ width: `${pct}%`, height: '6px', background: '#3b82f6', 'border-radius': '3px' }} />
                </div>
                <div style={{ 'font-size': '11px', color: '#94a3b8', 'min-width': '60px', 'text-align': 'right' }}>
                  {entry.size_mb > 1024 ? `${(entry.size_mb / 1024).toFixed(1)} GB` : `${entry.size_mb} MB`}
                </div>
              </div>
            );
          }}
        </For>
      </div>
    </div>
  );
};

export default DiskTab;
