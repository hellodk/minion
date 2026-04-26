import { Component, createSignal, onMount } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

interface SysmonSettings {
  cpu_warn: number;
  cpu_critical: number;
  ram_warn: number;
  ram_critical: number;
  disk_warn: number;
  disk_critical: number;
  gpu_warn: number;
  gpu_critical: number;
}

const defaultSettings: SysmonSettings = {
  cpu_warn: 75, cpu_critical: 90,
  ram_warn: 80, ram_critical: 92,
  disk_warn: 80, disk_critical: 90,
  gpu_warn: 85, gpu_critical: 95,
};

const SettingsTab: Component = () => {
  const [settings, setSettings] = createSignal<SysmonSettings>(defaultSettings);
  const [saved, setSaved] = createSignal(false);

  onMount(async () => {
    const s = await invoke<SysmonSettings>('sysmon_get_settings').catch(() => defaultSettings);
    setSettings(s);
  });

  const save = async () => {
    await invoke('sysmon_save_settings', { settings: settings() }).catch(() => {});
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  const row = (label: string, warnKey: keyof SysmonSettings, critKey: keyof SysmonSettings) => (
    <tr style={{ 'border-bottom': '1px solid #f1f5f9' }}>
      <td style={{ padding: '8px', 'font-size': '13px', color: '#475569', 'font-weight': '500' }}>{label}</td>
      <td style={{ padding: '8px' }}>
        <input
          type="number"
          min={0}
          max={100}
          value={settings()[warnKey]}
          onInput={(e) => setSettings(s => ({ ...s, [warnKey]: +e.currentTarget.value }))}
          style={{
            width: '60px', padding: '4px 8px', border: '1px solid #e2e8f0',
            'border-radius': '6px', 'font-size': '12px',
          }}
        />
      </td>
      <td style={{ padding: '8px' }}>
        <input
          type="number"
          min={0}
          max={100}
          value={settings()[critKey]}
          onInput={(e) => setSettings(s => ({ ...s, [critKey]: +e.currentTarget.value }))}
          style={{
            width: '60px', padding: '4px 8px', border: '1px solid #fecaca',
            'border-radius': '6px', 'font-size': '12px',
          }}
        />
      </td>
    </tr>
  );

  return (
    <div>
      <h3 style={{ margin: '0 0 12px', 'font-size': '14px', 'font-weight': '600', color: '#1e293b' }}>
        Alert Thresholds
      </h3>
      <table style={{ width: '100%', 'border-collapse': 'collapse' }}>
        <thead>
          <tr style={{ background: '#f8fafc' }}>
            <th style={{ padding: '6px 8px', 'text-align': 'left', 'font-size': '12px', color: '#64748b' }}>Metric</th>
            <th style={{ padding: '6px 8px', 'text-align': 'left', 'font-size': '12px', color: '#f97316' }}>Warn %</th>
            <th style={{ padding: '6px 8px', 'text-align': 'left', 'font-size': '12px', color: '#ef4444' }}>Critical %</th>
          </tr>
        </thead>
        <tbody>
          {row('CPU', 'cpu_warn', 'cpu_critical')}
          {row('RAM', 'ram_warn', 'ram_critical')}
          {row('Disk', 'disk_warn', 'disk_critical')}
          {row('GPU', 'gpu_warn', 'gpu_critical')}
        </tbody>
      </table>
      <div style={{ 'margin-top': '8px', 'font-size': '11px', color: '#94a3b8' }}>
        Zombie processes always trigger a warn alert regardless of threshold.
      </div>
      <button
        onClick={save}
        style={{
          'margin-top': '16px', padding: '7px 20px',
          background: saved() ? '#22c55e' : '#3b82f6',
          color: '#fff', border: 'none', 'border-radius': '6px',
          'font-size': '13px', cursor: 'pointer',
        }}
      >{saved() ? '✓ Saved' : 'Save Settings'}</button>
    </div>
  );
};

export default SettingsTab;
