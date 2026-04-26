import { Component, createSignal, onMount, onCleanup, For, Show } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import ProcessTab from './sysmon/ProcessTab';
import DiskTab from './sysmon/DiskTab';
import AnalysisTab from './sysmon/AnalysisTab';
import SettingsTab from './sysmon/SettingsTab';

// ---------- Types ----------

interface DiskInfo {
  mount: string;
  used_gb: number;
  total_gb: number;
  read_bps: number;
  write_bps: number;
}

interface GpuInfo {
  name: string;
  util_pct: number;
  vram_used_mb: number;
  vram_total_mb: number;
  temp_c: number | null;
}

interface NetInfo {
  iface: string;
  rx_bps: number;
  tx_bps: number;
}

interface SystemSnapshot {
  cpu_pct: number;
  ram_used_mb: number;
  ram_total_mb: number;
  swap_used_mb: number;
  load_avg_1: number | null;
  disks: DiskInfo[];
  gpus: GpuInfo[];
  net: NetInfo[];
}

interface SysmonAlert {
  id: string;
  fired_at: string;
  metric: string;
  value: number;
  threshold: number;
  severity: string;
  detail: string | null;
  resolved_at: string | null;
}

type Tab = 'processes' | 'disk' | 'analyses' | 'settings';

// ---------- Sparkline component ----------

const Sparkline: Component<{
  values: number[];
  color: string;
  label: string;
  current: number;
}> = (props) => {
  const max = () => Math.max(...props.values, 1);
  return (
    <div style={{ display: 'flex', 'flex-direction': 'column', gap: '4px', 'min-width': '120px' }}>
      <div style={{ display: 'flex', 'justify-content': 'space-between', 'align-items': 'baseline' }}>
        <span style={{
          'font-size': '11px', color: '#64748b', 'font-weight': '600',
          'text-transform': 'uppercase', 'letter-spacing': '0.5px',
        }}>{props.label}</span>
        <span style={{ 'font-size': '15px', 'font-weight': '700', color: props.color }}>
          {props.current.toFixed(0)}%
        </span>
      </div>
      <div style={{ display: 'flex', 'align-items': 'flex-end', gap: '1px', height: '24px' }}>
        <For each={props.values.slice(-24)}>
          {(v) => (
            <div style={{
              flex: '1',
              height: `${Math.max(2, (v / max()) * 100)}%`,
              background: props.color,
              'border-radius': '1px',
              opacity: '0.7',
            }} />
          )}
        </For>
      </div>
      <div style={{ height: '2px', background: '#f1f5f9', 'border-radius': '1px' }}>
        <div style={{
          width: `${Math.min(props.current, 100)}%`,
          height: '2px',
          background: props.color,
          'border-radius': '1px',
        }} />
      </div>
    </div>
  );
};

// ---------- Main component ----------

const Sysmon: Component = () => {
  const [snapshot, setSnapshot] = createSignal<SystemSnapshot | null>(null);
  const [alerts, setAlerts] = createSignal<SysmonAlert[]>([]);
  const [latestAnalysis, setLatestAnalysis] = createSignal<string | null>(null);
  const [tab, setTab] = createSignal<Tab>('processes');

  // Rolling history for sparklines (last 60 points = 5 min at 5 s interval)
  const [cpuHistory, setCpuHistory] = createSignal<number[]>([]);
  const [ramHistory, setRamHistory] = createSignal<number[]>([]);
  const [diskHistory, setDiskHistory] = createSignal<number[]>([]);
  const [gpuHistory, setGpuHistory] = createSignal<number[]>([]);

  const appendHistory = (
    setter: (fn: (prev: number[]) => number[]) => void,
    val: number,
  ) => {
    setter(prev => [...prev.slice(-59), val]);
  };

  const ramPct = () => {
    const s = snapshot();
    if (!s || s.ram_total_mb === 0) return 0;
    return (s.ram_used_mb / s.ram_total_mb) * 100;
  };

  const maxDiskPct = () => {
    const s = snapshot();
    if (!s || s.disks.length === 0) return 0;
    return Math.max(...s.disks.map(d => d.total_gb > 0 ? (d.used_gb / d.total_gb) * 100 : 0));
  };

  const maxGpuPct = () => {
    const s = snapshot();
    if (!s || s.gpus.length === 0) return 0;
    return Math.max(...s.gpus.map(g => g.util_pct));
  };

  onMount(async () => {
    // Load initial state
    const current = await invoke<{ snapshot: SystemSnapshot | null; processes: unknown[] }>(
      'sysmon_get_current',
    ).catch(() => null);
    if (current?.snapshot) setSnapshot(current.snapshot);

    const alertList = await invoke<SysmonAlert[]>('sysmon_list_alerts', { limit: 50 }).catch(() => []);
    setAlerts(alertList);

    // Load most recent analysis
    const analyses = await invoke<Array<{ response: string }>>('sysmon_list_analyses', { limit: 1 }).catch(() => []);
    if (analyses.length > 0) setLatestAnalysis(analyses[0].response);

    // Live event listeners
    const unlistenSnapshot = await listen<SystemSnapshot>('sysmon-snapshot', (e) => {
      const s = e.payload;
      setSnapshot(s);
      appendHistory(setCpuHistory, s.cpu_pct);
      appendHistory(setRamHistory, s.ram_total_mb > 0 ? (s.ram_used_mb / s.ram_total_mb) * 100 : 0);
      const dp = s.disks.length > 0
        ? Math.max(...s.disks.map(d => d.total_gb > 0 ? (d.used_gb / d.total_gb) * 100 : 0))
        : 0;
      appendHistory(setDiskHistory, dp);
      const gp = s.gpus.length > 0 ? Math.max(...s.gpus.map(g => g.util_pct)) : 0;
      appendHistory(setGpuHistory, gp);
    });

    const unlistenAlert = await listen<SysmonAlert>('sysmon-alert', (e) => {
      setAlerts(prev => [e.payload, ...prev.slice(0, 49)]);
    });

    const unlistenAnalysis = await listen<{ id: string; trigger: string; response: string }>(
      'sysmon-analysis-ready',
      (e) => {
        setLatestAnalysis(e.payload.response);
      },
    );

    onCleanup(() => {
      unlistenSnapshot();
      unlistenAlert();
      unlistenAnalysis();
    });
  });

  const tabs: { key: Tab; label: string }[] = [
    { key: 'processes', label: 'Processes' },
    { key: 'disk', label: 'Disk' },
    { key: 'analyses', label: 'Analyses' },
    { key: 'settings', label: 'Settings' },
  ];

  return (
    <div style={{
      padding: '20px', 'max-width': '900px', margin: '0 auto',
      'font-family': 'system-ui, sans-serif',
    }}>
      <h2 style={{ margin: '0 0 16px', 'font-size': '18px', 'font-weight': '700', color: '#0f172a' }}>
        System Monitor
      </h2>

      {/* Sparkline header — 4 metrics side by side */}
      <div style={{
        display: 'grid', 'grid-template-columns': 'repeat(4, 1fr)', gap: '16px',
        background: '#fff', border: '1px solid #e2e8f0', 'border-radius': '12px',
        padding: '16px', 'margin-bottom': '16px',
      }}>
        <Sparkline values={cpuHistory()} color="#f97316" label="CPU" current={snapshot()?.cpu_pct ?? 0} />
        <Sparkline values={ramHistory()} color="#3b82f6" label="RAM" current={ramPct()} />
        <Sparkline values={diskHistory()} color="#ef4444" label="Disk" current={maxDiskPct()} />
        <Sparkline values={gpuHistory()} color="#a855f7" label="GPU" current={maxGpuPct()} />
      </div>

      {/* Event timeline */}
      <div style={{
        background: '#fff', border: '1px solid #e2e8f0', 'border-radius': '12px',
        padding: '16px', 'margin-bottom': '16px',
      }}>
        <h3 style={{
          margin: '0 0 10px', 'font-size': '13px', 'font-weight': '600',
          color: '#64748b', 'text-transform': 'uppercase', 'letter-spacing': '0.5px',
        }}>Event Timeline</h3>
        <Show when={alerts().length === 0}>
          <div style={{ color: '#94a3b8', 'font-size': '12px', padding: '8px 0' }}>
            No alerts — system healthy.
          </div>
        </Show>
        <div style={{
          'max-height': '180px', overflow: 'auto', display: 'flex',
          'flex-direction': 'column', gap: '6px',
        }}>
          <For each={alerts().slice(0, 20)}>
            {(a) => {
              const isCrit = a.severity === 'critical';
              const dotColor = isCrit ? '#ef4444' : '#f59e0b';
              return (
                <div style={{ display: 'flex', 'align-items': 'flex-start', gap: '8px', 'font-size': '12px' }}>
                  <div style={{
                    width: '8px', height: '8px', 'border-radius': '50%',
                    background: dotColor, 'margin-top': '3px', 'flex-shrink': '0',
                  }} />
                  <div>
                    <span style={{ color: '#94a3b8', 'margin-right': '6px' }}>
                      {new Date(a.fired_at).toLocaleTimeString()}
                    </span>
                    <span style={{ color: '#1e293b', 'font-weight': '500' }}>
                      {a.metric.toUpperCase()} {isCrit ? '[CRIT]' : '[WARN]'} {a.value.toFixed(1)}%
                    </span>
                    <Show when={a.detail}>
                      <span style={{ color: '#64748b' }}> — {a.detail}</span>
                    </Show>
                    <Show when={a.resolved_at}>
                      <span style={{ color: '#22c55e', 'margin-left': '6px' }}>(resolved)</span>
                    </Show>
                  </div>
                </div>
              );
            }}
          </For>
        </div>
      </div>

      {/* LLM Insight panel */}
      <div style={{
        background: '#f0f9ff', border: '1px solid #bae6fd',
        'border-radius': '12px', padding: '14px', 'margin-bottom': '16px',
      }}>
        <div style={{
          display: 'flex', 'justify-content': 'space-between',
          'align-items': 'center', 'margin-bottom': '8px',
        }}>
          <span style={{ 'font-size': '13px', 'font-weight': '600', color: '#0369a1' }}>LLM Insight</span>
          <button
            onClick={() => setTab('analyses')}
            style={{
              'font-size': '12px', color: '#0284c7',
              background: 'none', border: 'none', cursor: 'pointer', 'text-decoration': 'underline',
            }}
          >Deep Dive →</button>
        </div>
        <Show
          when={latestAnalysis()}
          fallback={
            <div style={{ 'font-size': '12px', color: '#64748b' }}>
              No analysis yet. Analyses run automatically when alerts fire, or click Deep Dive.
            </div>
          }
        >
          <div style={{
            'font-size': '12px', color: '#0c4a6e', 'white-space': 'pre-wrap',
            'line-height': '1.6', 'max-height': '100px', overflow: 'auto',
          }}>
            {latestAnalysis()}
          </div>
        </Show>
      </div>

      {/* Tabs */}
      <div style={{ background: '#fff', border: '1px solid #e2e8f0', 'border-radius': '12px', overflow: 'hidden' }}>
        <div style={{ display: 'flex', 'border-bottom': '1px solid #e2e8f0' }}>
          <For each={tabs}>
            {(t) => (
              <button
                onClick={() => setTab(t.key)}
                style={{
                  flex: '1', padding: '10px', border: 'none', cursor: 'pointer',
                  'font-size': '13px',
                  background: tab() === t.key ? '#f0f9ff' : '#fff',
                  color: tab() === t.key ? '#0284c7' : '#64748b',
                  'border-bottom': tab() === t.key ? '2px solid #0284c7' : '2px solid transparent',
                  'font-weight': tab() === t.key ? '600' : '400',
                }}
              >{t.label}</button>
            )}
          </For>
        </div>
        <div style={{ padding: '16px' }}>
          <Show when={tab() === 'processes'}>
            <ProcessTab />
          </Show>
          <Show when={tab() === 'disk'}>
            <DiskTab disks={snapshot()?.disks ?? []} />
          </Show>
          <Show when={tab() === 'analyses'}>
            <AnalysisTab />
          </Show>
          <Show when={tab() === 'settings'}>
            <SettingsTab />
          </Show>
        </div>
      </div>
    </div>
  );
};

export default Sysmon;
