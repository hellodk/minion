import { Component, createSignal, createEffect, For, Show, onMount } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

type AnalysisMode = 'trend' | 'correlation' | 'lifestyle' | 'qa';

interface AnalysisRequest {
  patient_id: string;
  mode: AnalysisMode;
  question?: string;
  from?: string;
  to?: string;
  allow_cloud: boolean;
}

interface AnalysisResult {
  id: string;
  patient_id: string;
  mode: string;
  question: string | null;
  brief_text: string;
  response_text: string;
  model_used: string | null;
  cloud_used: boolean;
  created_at: string;
}

interface AnalysisEndpointStatus {
  configured: boolean;
  provider_type: string | null;
  base_url: string | null;
  model: string | null;
  is_cloud: boolean;
  user_cloud_consent: boolean;
}

type Preset = '3M' | '6M' | '12M' | 'all';

function isoDate(d: Date): string {
  return d.toISOString().slice(0, 10);
}

function presetRange(p: Preset): { from?: string; to?: string } {
  if (p === 'all') return {};
  const to = new Date();
  const from = new Date();
  if (p === '3M') from.setMonth(from.getMonth() - 3);
  else if (p === '6M') from.setMonth(from.getMonth() - 6);
  else if (p === '12M') from.setFullYear(from.getFullYear() - 1);
  return { from: isoDate(from), to: isoDate(to) };
}

function modeLabel(m: string): string {
  switch (m) {
    case 'trend':
      return 'Trend';
    case 'correlation':
      return 'Correlation';
    case 'lifestyle':
      return 'Lifestyle';
    case 'qa':
      return 'Q&A';
    default:
      return m;
  }
}

function fmtTimestamp(s: string): string {
  try {
    return new Date(s).toLocaleString();
  } catch {
    return s;
  }
}

const AnalysisTab: Component<{ patientId: string }> = (props) => {
  const [mode, setMode] = createSignal<AnalysisMode>('trend');
  const [question, setQuestion] = createSignal('');
  const [from, setFrom] = createSignal('');
  const [to, setTo] = createSignal('');
  const [brief, setBrief] = createSignal<string | null>(null);
  const [briefOpen, setBriefOpen] = createSignal(false);
  const [loadingBrief, setLoadingBrief] = createSignal(false);
  const [result, setResult] = createSignal<AnalysisResult | null>(null);
  const [loadingRun, setLoadingRun] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [endpoint, setEndpoint] = createSignal<AnalysisEndpointStatus | null>(null);
  const [history, setHistory] = createSignal<AnalysisResult[]>([]);
  const [historyOpen, setHistoryOpen] = createSignal(true);
  const [confirmCloud, setConfirmCloud] = createSignal(false);

  const loadEndpoint = async () => {
    try {
      const st = await invoke<AnalysisEndpointStatus>('health_analysis_endpoint_status', {});
      setEndpoint(st);
    } catch (e) {
      setError(String(e));
    }
  };

  const loadHistory = async () => {
    try {
      const list = await invoke<AnalysisResult[]>('health_list_analyses', {
        patient_id: props.patientId,
        limit: 50,
      });
      setHistory(list);
    } catch (e) {
      setError(String(e));
    }
  };

  onMount(() => {
    loadEndpoint();
  });

  createEffect(() => {
    props.patientId;
    setResult(null);
    setBrief(null);
    setBriefOpen(false);
    setError(null);
    loadHistory();
  });

  const applyPreset = (p: Preset) => {
    const { from: f, to: t } = presetRange(p);
    setFrom(f ?? '');
    setTo(t ?? '');
  };

  const previewBrief = async () => {
    setLoadingBrief(true);
    setError(null);
    try {
      const text = await invoke<string>('health_preview_brief', {
        patient_id: props.patientId,
        from: from() || undefined,
        to: to() || undefined,
      });
      setBrief(text);
      setBriefOpen(true);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoadingBrief(false);
    }
  };

  const runAnalysis = async (allowCloud: boolean) => {
    setLoadingRun(true);
    setError(null);
    try {
      const req: AnalysisRequest = {
        patient_id: props.patientId,
        mode: mode(),
        allow_cloud: allowCloud,
      };
      if (mode() === 'qa') {
        if (!question().trim()) {
          setError('Please enter a question for Q&A mode.');
          setLoadingRun(false);
          return;
        }
        req.question = question().trim();
      }
      if (from()) req.from = from();
      if (to()) req.to = to();
      const res = await invoke<AnalysisResult>('health_run_analysis', { request: req });
      setResult(res);
      loadHistory();
    } catch (e) {
      setError(String(e));
    } finally {
      setLoadingRun(false);
    }
  };

  const handleRunClick = () => {
    const ep = endpoint();
    if (!ep || !ep.configured) {
      setError('Configure an LLM endpoint in Settings first.');
      return;
    }
    if (ep.is_cloud && !ep.user_cloud_consent) {
      setConfirmCloud(true);
      return;
    }
    runAnalysis(ep.is_cloud);
  };

  const confirmAndRun = () => {
    setConfirmCloud(false);
    runAnalysis(true);
  };

  const loadFromHistory = (r: AnalysisResult) => {
    setResult(r);
    setMode(r.mode as AnalysisMode);
    if (r.question) setQuestion(r.question);
    setBrief(r.brief_text);
  };

  const deleteFromHistory = async (id: string, e: MouseEvent) => {
    e.stopPropagation();
    try {
      await invoke<void>('health_delete_analysis', { id });
      if (result()?.id === id) setResult(null);
      loadHistory();
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div class="space-y-4">
      {/* Endpoint status banner */}
      <Show when={endpoint()}>
        {(ep) => (
          <div
            class="card p-3 text-sm flex items-center justify-between"
            classList={{
              'border-l-4 border-green-500': ep().configured && !ep().is_cloud,
              'border-l-4 border-amber-500': ep().configured && ep().is_cloud,
              'border-l-4 border-red-500': !ep().configured,
            }}
          >
            <div>
              <Show when={ep().configured && !ep().is_cloud}>
                <span class="font-medium text-green-700 dark:text-green-400">
                  Local LLM ready
                </span>
                <Show when={ep().model}>
                  <span class="ml-2 text-gray-500">
                    {ep().provider_type} / {ep().model}
                  </span>
                </Show>
              </Show>
              <Show when={ep().configured && ep().is_cloud}>
                <span class="font-medium text-amber-700 dark:text-amber-400">
                  Cloud LLM ({ep().provider_type}) — extra consent required per request
                </span>
                <Show when={ep().model}>
                  <span class="ml-2 text-gray-500">{ep().model}</span>
                </Show>
              </Show>
              <Show when={!ep().configured}>
                <span class="font-medium text-red-700 dark:text-red-400">
                  No LLM endpoint configured.
                </span>
                <a href="/settings" class="ml-2 text-minion-600 hover:underline">
                  Open Settings
                </a>
              </Show>
            </div>
            <button
              class="text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
              onClick={loadEndpoint}
            >
              Refresh
            </button>
          </div>
        )}
      </Show>

      <div class="flex gap-4">
        {/* Main column */}
        <div class="flex-1 space-y-4">
          {/* Mode picker */}
          <div class="card p-4">
            <div class="text-sm font-medium mb-2">Mode</div>
            <div class="flex gap-1">
              <For each={['trend', 'correlation', 'lifestyle', 'qa'] as AnalysisMode[]}>
                {(m) => (
                  <button
                    class="px-3 py-1.5 text-sm border rounded transition-colors"
                    classList={{
                      'bg-minion-500 text-white border-minion-500': mode() === m,
                      'border-gray-300 dark:border-gray-600 hover:bg-gray-100 dark:hover:bg-gray-800':
                        mode() !== m,
                    }}
                    onClick={() => setMode(m)}
                  >
                    {modeLabel(m)}
                  </button>
                )}
              </For>
            </div>
          </div>

          {/* Date range */}
          <div class="card p-4 space-y-2">
            <div class="text-sm font-medium">Date range</div>
            <div class="flex gap-2 items-center flex-wrap">
              <input
                type="date"
                class="px-2 py-1 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
                value={from()}
                onInput={(e) => setFrom(e.currentTarget.value)}
              />
              <span class="text-gray-500">to</span>
              <input
                type="date"
                class="px-2 py-1 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600"
                value={to()}
                onInput={(e) => setTo(e.currentTarget.value)}
              />
              <div class="flex gap-1 ml-2">
                <For each={['3M', '6M', '12M', 'all'] as Preset[]}>
                  {(p) => (
                    <button
                      class="px-2 py-1 text-xs border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
                      onClick={() => applyPreset(p)}
                    >
                      {p === 'all' ? 'All-time' : `Last ${p}`}
                    </button>
                  )}
                </For>
              </div>
            </div>
          </div>

          {/* Q&A question */}
          <Show when={mode() === 'qa'}>
            <div class="card p-4">
              <div class="text-sm font-medium mb-2">Your question</div>
              <textarea
                class="w-full px-3 py-2 text-sm border rounded bg-transparent border-gray-300 dark:border-gray-600 min-h-[80px]"
                placeholder="e.g., What might be driving my recent fatigue?"
                value={question()}
                onInput={(e) => setQuestion(e.currentTarget.value)}
              />
            </div>
          </Show>

          {/* Actions */}
          <div class="flex gap-2">
            <button
              class="px-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
              onClick={previewBrief}
              disabled={loadingBrief()}
            >
              {loadingBrief() ? 'Loading…' : 'Preview brief'}
            </button>
            <button
              class="btn-primary text-sm"
              onClick={handleRunClick}
              disabled={loadingRun()}
            >
              {loadingRun() ? 'Running…' : 'Run analysis'}
            </button>
          </div>

          {/* Error */}
          <Show when={error()}>
            <div class="card p-3 text-sm text-red-700 dark:text-red-400 border-l-4 border-red-500">
              {error()}
            </div>
          </Show>

          {/* Brief preview */}
          <Show when={brief() !== null}>
            <div class="card p-0 overflow-hidden">
              <button
                class="w-full flex items-center justify-between px-4 py-2 text-sm font-medium hover:bg-gray-50 dark:hover:bg-gray-800"
                onClick={() => setBriefOpen(!briefOpen())}
              >
                <span>Data brief (what will be sent to the model)</span>
                <span class="text-gray-500">{briefOpen() ? '▾' : '▸'}</span>
              </button>
              <Show when={briefOpen()}>
                <pre class="px-4 py-3 text-xs font-mono bg-gray-50 dark:bg-gray-900 overflow-x-auto whitespace-pre-wrap border-t border-gray-200 dark:border-gray-700">
                  {brief()}
                </pre>
              </Show>
            </div>
          </Show>

          {/* Result */}
          <Show when={result()}>
            {(r) => (
              <div class="card p-4 space-y-3">
                <div class="flex items-center gap-2 text-xs flex-wrap">
                  <span class="px-2 py-0.5 rounded bg-minion-100 dark:bg-minion-900/30 text-minion-700 dark:text-minion-300">
                    {modeLabel(r().mode)}
                  </span>
                  <Show when={r().model_used}>
                    <span class="px-2 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300">
                      {r().model_used}
                    </span>
                  </Show>
                  <span
                    class="px-2 py-0.5 rounded"
                    classList={{
                      'bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300':
                        r().cloud_used,
                      'bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300':
                        !r().cloud_used,
                    }}
                  >
                    {r().cloud_used ? 'Cloud' : 'Local'}
                  </span>
                  <span class="text-gray-500 ml-auto">{fmtTimestamp(r().created_at)}</span>
                </div>
                <Show when={r().question}>
                  <div class="text-sm italic text-gray-600 dark:text-gray-400">
                    Q: {r().question}
                  </div>
                </Show>
                <div class="text-sm whitespace-pre-wrap leading-relaxed">
                  {r().response_text}
                </div>
              </div>
            )}
          </Show>
        </div>

        {/* History sidebar */}
        <div
          class="card p-3 flex-shrink-0"
          classList={{
            'w-72': historyOpen(),
            'w-12': !historyOpen(),
          }}
        >
          <div class="flex items-center justify-between mb-2">
            <Show when={historyOpen()}>
              <div class="text-sm font-medium">History</div>
            </Show>
            <button
              class="text-xs text-gray-500 hover:text-gray-700 dark:hover:text-gray-300"
              onClick={() => setHistoryOpen(!historyOpen())}
              title={historyOpen() ? 'Collapse' : 'Expand history'}
            >
              {historyOpen() ? '›' : '‹'}
            </button>
          </div>
          <Show when={historyOpen()}>
            <Show
              when={history().length > 0}
              fallback={
                <div class="text-xs text-gray-500 py-4 text-center">No past analyses</div>
              }
            >
              <div class="space-y-1 max-h-[600px] overflow-y-auto">
                <For each={history()}>
                  {(h) => (
                    <div
                      class="group p-2 rounded border border-transparent hover:border-gray-300 dark:hover:border-gray-600 cursor-pointer"
                      classList={{
                        'bg-minion-50 dark:bg-minion-900/20 border-minion-300 dark:border-minion-700':
                          result()?.id === h.id,
                      }}
                      onClick={() => loadFromHistory(h)}
                    >
                      <div class="flex items-center justify-between gap-2">
                        <span class="text-xs font-medium">{modeLabel(h.mode)}</span>
                        <button
                          class="text-xs text-red-500 opacity-0 group-hover:opacity-100 hover:underline"
                          onClick={(e) => deleteFromHistory(h.id, e)}
                        >
                          Delete
                        </button>
                      </div>
                      <Show when={h.question}>
                        <div class="text-xs text-gray-600 dark:text-gray-400 truncate">
                          {h.question}
                        </div>
                      </Show>
                      <div class="text-xs text-gray-500 mt-0.5">
                        {fmtTimestamp(h.created_at)}
                        <Show when={h.cloud_used}>
                          <span class="ml-1 text-amber-600 dark:text-amber-400">cloud</span>
                        </Show>
                      </div>
                    </div>
                  )}
                </For>
              </div>
            </Show>
          </Show>
        </div>
      </div>

      {/* Cloud confirm modal */}
      <Show when={confirmCloud()}>
        <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div class="card p-5 max-w-md w-full mx-4 space-y-3">
            <div class="text-lg font-semibold">Send data to cloud?</div>
            <div class="text-sm text-gray-600 dark:text-gray-400">
              This will send health data to {endpoint()?.provider_type} (
              <span class="font-mono">{endpoint()?.base_url}</span>). Continue?
            </div>
            <div class="flex justify-end gap-2 pt-2">
              <button
                class="px-3 py-1.5 text-sm border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
                onClick={() => setConfirmCloud(false)}
              >
                Cancel
              </button>
              <button class="btn-primary text-sm" onClick={confirmAndRun}>
                Continue
              </button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
};

export default AnalysisTab;
