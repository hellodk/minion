import { Component, createSignal, For, Show } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

interface ProcessInfo {
  pid: number;
  name: string;
  cpu_pct: number;
  ram_mb: number;
  status: string;
  user_name: string | null;
}

const ProcessTab: Component = () => {
  const [procs, setProcs] = createSignal<ProcessInfo[]>([]);
  const [sortBy, setSortBy] = createSignal<'cpu' | 'ram'>('cpu');
  const [killing, setKilling] = createSignal<number | null>(null);

  const refresh = async () => {
    const result = await invoke<ProcessInfo[]>('sysmon_list_processes').catch(() => []);
    setProcs(result);
  };

  const sorted = () => {
    const col = sortBy();
    return [...procs()].sort((a, b) =>
      col === 'cpu' ? b.cpu_pct - a.cpu_pct : b.ram_mb - a.ram_mb
    );
  };

  const kill = async (pid: number) => {
    setKilling(pid);
    await invoke('sysmon_kill_process', { pid }).catch(() => {});
    setKilling(null);
    await refresh();
  };

  // Initial load
  refresh();

  return (
    <div>
      <div style={{ display: 'flex', 'justify-content': 'space-between', 'align-items': 'center', 'margin-bottom': '12px' }}>
        <h3 style={{ margin: '0', color: '#1e293b', 'font-size': '14px', 'font-weight': '600' }}>Processes</h3>
        <div style={{ display: 'flex', gap: '8px' }}>
          <button
            onClick={() => setSortBy('cpu')}
            style={{
              padding: '4px 10px', 'border-radius': '6px', 'font-size': '12px', cursor: 'pointer',
              border: sortBy() === 'cpu' ? '1px solid #3b82f6' : '1px solid #e2e8f0',
              background: sortBy() === 'cpu' ? '#eff6ff' : '#fff',
              color: sortBy() === 'cpu' ? '#1d4ed8' : '#64748b',
            }}
          >Sort CPU</button>
          <button
            onClick={() => setSortBy('ram')}
            style={{
              padding: '4px 10px', 'border-radius': '6px', 'font-size': '12px', cursor: 'pointer',
              border: sortBy() === 'ram' ? '1px solid #3b82f6' : '1px solid #e2e8f0',
              background: sortBy() === 'ram' ? '#eff6ff' : '#fff',
              color: sortBy() === 'ram' ? '#1d4ed8' : '#64748b',
            }}
          >Sort RAM</button>
          <button
            onClick={refresh}
            style={{ padding: '4px 10px', 'border-radius': '6px', 'font-size': '12px', cursor: 'pointer', border: '1px solid #e2e8f0', background: '#fff', color: '#64748b' }}
          >&#8635; Refresh</button>
        </div>
      </div>

      <div style={{ overflow: 'auto', 'max-height': '400px' }}>
        <table style={{ width: '100%', 'border-collapse': 'collapse', 'font-size': '12px' }}>
          <thead>
            <tr style={{ background: '#f8fafc', 'border-bottom': '1px solid #e2e8f0' }}>
              <th style={{ padding: '6px 8px', 'text-align': 'left', color: '#64748b', 'font-weight': '600' }}>PID</th>
              <th style={{ padding: '6px 8px', 'text-align': 'left', color: '#64748b', 'font-weight': '600' }}>Name</th>
              <th style={{ padding: '6px 8px', 'text-align': 'right', color: '#64748b', 'font-weight': '600' }}>CPU%</th>
              <th style={{ padding: '6px 8px', 'text-align': 'right', color: '#64748b', 'font-weight': '600' }}>RAM</th>
              <th style={{ padding: '6px 8px', 'text-align': 'left', color: '#64748b', 'font-weight': '600' }}>Status</th>
              <th style={{ padding: '6px 8px' }}></th>
            </tr>
          </thead>
          <tbody>
            <For each={sorted()}>
              {(p) => {
                const isZombie = p.status === 'zombie';
                return (
                  <tr style={{
                    'border-bottom': '1px solid #f1f5f9',
                    background: isZombie ? '#fef2f2' : 'transparent',
                  }}>
                    <td style={{ padding: '5px 8px', color: '#94a3b8' }}>{p.pid}</td>
                    <td style={{
                      padding: '5px 8px',
                      color: isZombie ? '#dc2626' : '#1e293b',
                      'font-weight': isZombie ? '600' : 'normal',
                    }}>
                      {isZombie ? '⚠ ' : ''}{p.name}
                    </td>
                    <td style={{ padding: '5px 8px', 'text-align': 'right', color: p.cpu_pct > 50 ? '#f97316' : '#475569' }}>
                      {p.cpu_pct.toFixed(1)}%
                    </td>
                    <td style={{ padding: '5px 8px', 'text-align': 'right', color: '#475569' }}>
                      {p.ram_mb > 1024 ? `${(p.ram_mb / 1024).toFixed(1)}G` : `${p.ram_mb}M`}
                    </td>
                    <td style={{ padding: '5px 8px' }}>
                      <span style={{
                        padding: '2px 6px', 'border-radius': '4px', 'font-size': '11px',
                        background: isZombie ? '#fee2e2' : '#f0fdf4',
                        color: isZombie ? '#dc2626' : '#16a34a',
                      }}>{p.status}</span>
                    </td>
                    <td style={{ padding: '5px 8px', 'text-align': 'right' }}>
                      <Show when={isZombie || p.cpu_pct > 80}>
                        <button
                          onClick={() => kill(p.pid)}
                          disabled={killing() === p.pid}
                          style={{
                            padding: '2px 8px', 'border-radius': '4px', 'font-size': '11px',
                            background: '#fef2f2', color: '#dc2626',
                            border: '1px solid #fecaca', cursor: 'pointer',
                          }}
                        >{killing() === p.pid ? '...' : 'Kill'}</button>
                      </Show>
                    </td>
                  </tr>
                );
              }}
            </For>
          </tbody>
        </table>
      </div>
    </div>
  );
};

export default ProcessTab;
