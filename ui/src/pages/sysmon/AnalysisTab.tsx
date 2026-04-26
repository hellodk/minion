import { Component, createSignal, For, onCleanup, onMount, Show } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

interface Analysis {
  id: string;
  created_at: string;
  trigger: string;
  question: string | null;
  response: string;
}

const AnalysisTab: Component = () => {
  const [analyses, setAnalyses] = createSignal<Analysis[]>([]);
  const [question, setQuestion] = createSignal('');
  const [running, setRunning] = createSignal(false);
  const [expanded, setExpanded] = createSignal<string | null>(null);

  const load = async () => {
    const result = await invoke<Analysis[]>('sysmon_list_analyses', { limit: 20 }).catch(() => []);
    setAnalyses(result);
  };

  const deepDive = async () => {
    setRunning(true);
    const result = await invoke<Analysis | null>('sysmon_run_analysis', {
      question: question() || null,
    }).catch(() => null);
    setRunning(false);
    setQuestion('');
    if (result) {
      setAnalyses(prev => [result, ...prev]);
      setExpanded(result.id);
    }
  };

  onMount(async () => {
    await load();
    const unlisten = await listen('sysmon-analysis-ready', () => { load(); });
    onCleanup(() => unlisten());
  });

  return (
    <div>
      {/* Deep Dive panel */}
      <div style={{
        background: '#f0f9ff', border: '1px solid #bae6fd',
        'border-radius': '8px', padding: '12px', 'margin-bottom': '16px',
      }}>
        <div style={{ 'font-size': '13px', 'font-weight': '600', color: '#0369a1', 'margin-bottom': '8px' }}>
          Deep Dive Analysis
        </div>
        <textarea
          value={question()}
          onInput={(e) => setQuestion(e.currentTarget.value)}
          placeholder="Optional: ask a specific question (e.g. 'Why is disk I/O spiking?')"
          rows={2}
          style={{
            width: '100%', padding: '8px', border: '1px solid #bae6fd',
            'border-radius': '6px', 'font-size': '12px', color: '#1e293b',
            resize: 'vertical', 'box-sizing': 'border-box',
          }}
        />
        <button
          onClick={deepDive}
          disabled={running()}
          style={{
            'margin-top': '8px', padding: '6px 16px',
            background: running() ? '#94a3b8' : '#0ea5e9',
            color: '#fff', border: 'none', 'border-radius': '6px',
            'font-size': '12px', cursor: running() ? 'not-allowed' : 'pointer',
          }}
        >{running() ? 'Analysing...' : 'Run Analysis'}</button>
      </div>

      {/* History */}
      <h3 style={{ margin: '0 0 10px', 'font-size': '13px', 'font-weight': '600', color: '#64748b' }}>
        Analysis History
      </h3>
      <Show when={analyses().length === 0}>
        <div style={{ color: '#94a3b8', 'font-size': '12px', padding: '20px', 'text-align': 'center' }}>
          No analyses yet. Click "Run Analysis" or wait for an auto-triggered analysis.
        </div>
      </Show>
      <For each={analyses()}>
        {(a) => (
          <div style={{ border: '1px solid #e2e8f0', 'border-radius': '8px', 'margin-bottom': '8px', overflow: 'hidden' }}>
            <div
              onClick={() => setExpanded(expanded() === a.id ? null : a.id)}
              style={{
                display: 'flex', 'justify-content': 'space-between', 'align-items': 'center',
                padding: '8px 12px', cursor: 'pointer', background: '#f8fafc',
              }}
            >
              <div>
                <span style={{ 'font-size': '12px', 'font-weight': '500', color: '#1e293b' }}>
                  {a.trigger === 'auto' ? 'Auto' : 'Manual'} — {new Date(a.created_at).toLocaleString()}
                </span>
                <Show when={a.question}>
                  <div style={{ 'font-size': '11px', color: '#64748b', 'margin-top': '2px' }}>{a.question}</div>
                </Show>
              </div>
              <span style={{ color: '#94a3b8', 'font-size': '12px' }}>{expanded() === a.id ? '▲' : '▼'}</span>
            </div>
            <Show when={expanded() === a.id}>
              <div style={{
                padding: '12px', 'font-size': '12px', color: '#334155',
                'white-space': 'pre-wrap', 'line-height': '1.6',
                'border-top': '1px solid #e2e8f0',
              }}>
                {a.response}
              </div>
            </Show>
          </div>
        )}
      </For>
    </div>
  );
};

export default AnalysisTab;
