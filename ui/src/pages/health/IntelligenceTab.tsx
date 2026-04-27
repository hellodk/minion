import { Component, createSignal, For, Show, onMount } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

interface TimelineEvent {
  id: string;
  patient_id: string;
  event_date: string;
  category: string;
  title: string;
  description: string | null;
  source_type: string;
  source_id: string | null;
  severity: string | null;
  metadata_json: string | null;
}

interface AnomalyAlert {
  id: string;
  rule_name: string;
  severity: string;
  title: string;
  description: string;
  detected_at: string;
}

interface IntelligenceReport {
  id: string;
  patient_id: string;
  generated_at: string;
  model_used: string;
  report_text: string;
  anomalies_json: string | null;
}

const CATEGORY_COLORS: Record<string, string> = {
  lab: 'bg-red-500',
  prescription: 'bg-blue-500',
  fitness: 'bg-green-500',
  location: 'bg-amber-500',
  symptom: 'bg-purple-500',
  vital: 'bg-rose-500',
};

const SEVERITY_BORDER: Record<string, string> = {
  alert: 'border-l-4 border-red-500 bg-red-50 dark:bg-red-900/20',
  warning: 'border-l-4 border-amber-500 bg-amber-50 dark:bg-amber-900/20',
  info: 'border-l-4 border-blue-300 bg-blue-50 dark:bg-blue-900/10',
};

const FILTERS = ['All', 'lab', 'prescription', 'fitness', 'location', 'symptom', 'vital'];

const IntelligenceTab: Component<{ patientId: string }> = (props) => {
  // Anomaly state
  const [anomalies, setAnomalies] = createSignal<AnomalyAlert[]>([]);
  const [anomalyLoading, setAnomalyLoading] = createSignal(false);
  const [anomalyError, setAnomalyError] = createSignal('');
  const [anomalyCollapsed, setAnomalyCollapsed] = createSignal(false);

  // Timeline state
  const [events, setEvents] = createSignal<TimelineEvent[]>([]);
  const [timelineLoading, setTimelineLoading] = createSignal(false);
  const [timelineError, setTimelineError] = createSignal('');
  const [offset, setOffset] = createSignal(0);
  const [hasMore, setHasMore] = createSignal(false);
  const [categoryFilter, setCategoryFilter] = createSignal<string | null>(null);
  const [rebuildLoading, setRebuildLoading] = createSignal(false);
  const PAGE = 50;

  // AI report state
  const [reports, setReports] = createSignal<IntelligenceReport[]>([]);
  const [reportLoading, setReportLoading] = createSignal(false);
  const [reportError, setReportError] = createSignal('');
  const [consentChecked, setConsentChecked] = createSignal(false);
  const [expandedReport, setExpandedReport] = createSignal<string | null>(null);

  const loadAnomalies = async () => {
    setAnomalyLoading(true); setAnomalyError('');
    try {
      setAnomalies(await invoke<AnomalyAlert[]>('health_detect_anomalies', { patientId: props.patientId }));
    } catch (e) { setAnomalyError(String(e)); }
    finally { setAnomalyLoading(false); }
  };

  const loadTimeline = async (reset = false) => {
    setTimelineLoading(true); setTimelineError('');
    try {
      const off = reset ? 0 : offset();
      const filter = categoryFilter();
      const newEvents = await invoke<TimelineEvent[]>('health_get_timeline', {
        patientId: props.patientId,
        limit: PAGE + 1,
        offset: off,
        categoryFilter: filter,
      });
      const page = newEvents.slice(0, PAGE);
      setHasMore(newEvents.length > PAGE);
      if (reset) { setEvents(page); setOffset(PAGE); }
      else { setEvents((prev) => [...prev, ...page]); setOffset(off + PAGE); }
    } catch (e) { setTimelineError(String(e)); }
    finally { setTimelineLoading(false); }
  };

  const rebuildTimeline = async () => {
    setRebuildLoading(true);
    try {
      const count = await invoke<number>('health_rebuild_timeline', { patientId: props.patientId });
      await loadTimeline(true);
      setTimelineError(`✓ Timeline rebuilt with ${count} events.`);
    } catch (e) { setTimelineError(String(e)); }
    finally { setRebuildLoading(false); }
  };

  const loadReports = async () => {
    try { setReports(await invoke<IntelligenceReport[]>('health_list_reports', { patientId: props.patientId })); }
    catch { /* ignore */ }
  };

  const generateReport = async () => {
    if (!consentChecked()) return;
    setReportLoading(true); setReportError('');
    try {
      const r = await invoke<IntelligenceReport>('health_generate_report', {
        patientId: props.patientId,
        consentConfirmed: true,
      });
      setReports((prev) => [r, ...prev]);
      setExpandedReport(r.id);
    } catch (e) { setReportError(String(e)); }
    finally { setReportLoading(false); }
  };

  const deleteReport = async (id: string) => {
    if (!confirm('Delete this report?')) return;
    try {
      await invoke('health_delete_report', { id });
      setReports((prev) => prev.filter((r) => r.id !== id));
      if (expandedReport() === id) setExpandedReport(null);
    } catch (e) { setReportError(String(e)); }
  };

  onMount(async () => {
    await Promise.all([loadAnomalies(), loadTimeline(true), loadReports()]);
  });

  // Group events by month for timeline headers
  const groupedEvents = () => {
    const groups: { month: string; items: TimelineEvent[] }[] = [];
    let currentMonth = '';
    for (const ev of events()) {
      const month = ev.event_date.slice(0, 7); // YYYY-MM
      if (month !== currentMonth) {
        currentMonth = month;
        groups.push({ month, items: [] });
      }
      groups[groups.length - 1].items.push(ev);
    }
    return groups;
  };

  const fmtMonth = (ym: string) => {
    const [year, month] = ym.split('-');
    const names = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec'];
    return `${names[parseInt(month) - 1]} ${year}`;
  };

  return (
    <div class="space-y-6 p-4 max-w-4xl mx-auto">

      {/* ── Section 1: Anomaly Alerts ──────────────────────────── */}
      <div class="card">
        <div
          class="flex items-center justify-between p-4 cursor-pointer"
          onClick={() => setAnomalyCollapsed((v) => !v)}
        >
          <div class="flex items-center gap-3">
            <span class="text-base font-semibold text-gray-800 dark:text-gray-100">
              Anomaly Alerts
            </span>
            <Show when={anomalies().length > 0}>
              <span class="px-2 py-0.5 text-xs bg-red-100 dark:bg-red-900/40 text-red-700 dark:text-red-300 rounded-full font-medium">
                {anomalies().length}
              </span>
            </Show>
          </div>
          <div class="flex items-center gap-2">
            <button
              onClick={(e) => { e.stopPropagation(); loadAnomalies(); }}
              disabled={anomalyLoading()}
              class="text-xs text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 px-2 py-1 rounded hover:bg-gray-100 dark:hover:bg-gray-700"
            >
              {anomalyLoading() ? 'Detecting…' : '↻ Run Detection'}
            </button>
            <span class="text-xs text-gray-400">{anomalyCollapsed() ? '▼' : '▲'}</span>
          </div>
        </div>

        <Show when={!anomalyCollapsed()}>
          <div class="px-4 pb-4 space-y-2">
            <Show when={anomalyError()}>
              <p class="text-sm text-red-500">{anomalyError()}</p>
            </Show>
            <Show
              when={anomalies().length > 0}
              fallback={
                <p class="text-sm text-gray-400 text-center py-4">
                  No anomalies detected.
                </p>
              }
            >
              <For each={anomalies()}>
                {(alert) => (
                  <div class={`p-3 rounded-lg ${SEVERITY_BORDER[alert.severity] ?? SEVERITY_BORDER.info}`}>
                    <div class="flex items-start gap-2">
                      <div class="flex-1 min-w-0">
                        <p class="text-sm font-semibold text-gray-800 dark:text-gray-100">{alert.title}</p>
                        <p class="text-xs text-gray-600 dark:text-gray-300 mt-0.5">{alert.description}</p>
                        <p class="text-[10px] text-gray-400 mt-1">Detected: {alert.detected_at}</p>
                      </div>
                    </div>
                  </div>
                )}
              </For>
            </Show>
          </div>
        </Show>
      </div>

      {/* ── Section 2: Unified Timeline ────────────────────────── */}
      <div class="card">
        <div class="flex items-center justify-between p-4 border-b border-gray-100 dark:border-gray-700">
          <span class="text-base font-semibold text-gray-800 dark:text-gray-100">Health Timeline</span>
          <button
            onClick={rebuildTimeline}
            disabled={rebuildLoading()}
            class="text-xs text-gray-400 hover:text-gray-600 dark:hover:text-gray-300 px-2 py-1 rounded hover:bg-gray-100 dark:hover:bg-gray-700"
          >
            {rebuildLoading() ? 'Rebuilding…' : '↻ Rebuild'}
          </button>
        </div>

        {/* Filter chips */}
        <div class="flex gap-1.5 px-4 py-3 flex-wrap border-b border-gray-100 dark:border-gray-700">
          <For each={FILTERS}>
            {(f) => {
              const active = () => (f === 'All' ? categoryFilter() === null : categoryFilter() === f);
              return (
                <button
                  onClick={async () => {
                    setCategoryFilter(f === 'All' ? null : f);
                    await loadTimeline(true);
                  }}
                  class={`px-2.5 py-1 text-xs rounded-full border transition-colors ${
                    active()
                      ? 'bg-sky-500 text-white border-sky-500'
                      : 'border-gray-200 dark:border-gray-600 text-gray-600 dark:text-gray-300 hover:border-sky-400'
                  }`}
                >
                  {f}
                </button>
              );
            }}
          </For>
        </div>

        <div class="px-4 py-3">
          <Show when={timelineError()}>
            <p class="text-xs text-gray-500 mb-2">{timelineError()}</p>
          </Show>

          <Show
            when={events().length > 0}
            fallback={
              <p class="text-sm text-gray-400 text-center py-8">
                No events found. Click "↻ Rebuild" to populate the timeline from your health data.
              </p>
            }
          >
            <div class="space-y-0">
              <For each={groupedEvents()}>
                {(group) => (
                  <div>
                    <div class="text-xs font-bold uppercase text-gray-400 tracking-wider py-2 sticky top-0 bg-white dark:bg-gray-800">
                      {fmtMonth(group.month)}
                    </div>
                    <For each={group.items}>
                      {(ev) => (
                        <div class="flex gap-3 pb-4">
                          <div class="flex flex-col items-center mt-1">
                            <div class={`w-2.5 h-2.5 rounded-full shrink-0 ${CATEGORY_COLORS[ev.category] ?? 'bg-gray-400'}`} />
                            <div class="w-px flex-1 bg-gray-200 dark:bg-gray-700 mt-1" />
                          </div>
                          <div class="flex-1 min-w-0 pb-2">
                            <div class="flex items-start gap-2">
                              <div class="flex-1 min-w-0">
                                <p class="text-sm font-medium text-gray-800 dark:text-gray-100 truncate">{ev.title}</p>
                                <Show when={ev.description}>
                                  <p class="text-xs text-gray-500 dark:text-gray-400 mt-0.5">{ev.description}</p>
                                </Show>
                                <p class="text-[10px] text-gray-400 mt-0.5">{ev.event_date} · {ev.category}</p>
                              </div>
                              <Show when={ev.severity === 'alert' || ev.severity === 'warning'}>
                                <span class={`text-[10px] px-1.5 py-0.5 rounded font-medium shrink-0 ${
                                  ev.severity === 'alert'
                                    ? 'bg-red-100 dark:bg-red-900/40 text-red-600 dark:text-red-400'
                                    : 'bg-amber-100 dark:bg-amber-900/40 text-amber-600 dark:text-amber-400'
                                }`}>
                                  {ev.severity}
                                </span>
                              </Show>
                            </div>
                          </div>
                        </div>
                      )}
                    </For>
                  </div>
                )}
              </For>
            </div>

            <Show when={hasMore()}>
              <button
                onClick={() => loadTimeline(false)}
                disabled={timelineLoading()}
                class="w-full py-2 text-xs text-sky-600 hover:text-sky-800 dark:text-sky-400"
              >
                {timelineLoading() ? 'Loading…' : '↓ Load more'}
              </button>
            </Show>
          </Show>
        </div>
      </div>

      {/* ── Section 3: AI Analysis Panel ───────────────────────── */}
      <div class="card p-4">
        <h2 class="text-base font-semibold text-gray-800 dark:text-gray-100 mb-4">AI Analysis</h2>

        {/* Generate button + consent */}
        <div class="p-4 bg-gray-50 dark:bg-gray-900 rounded-xl mb-4">
          <p class="text-sm font-medium text-gray-700 dark:text-gray-200 mb-2">Generate Doctor Report</p>
          <p class="text-xs text-gray-500 dark:text-gray-400 mb-3">
            Creates a 300–500 word health summary from your last 6 months of data, suitable for sharing with your GP.
            <strong class="text-amber-600"> Requires sending your health summary to the configured AI endpoint.</strong>
          </p>
          <label class="flex items-start gap-2 cursor-pointer mb-3">
            <input
              type="checkbox"
              class="mt-0.5"
              checked={consentChecked()}
              onChange={(e) => setConsentChecked(e.currentTarget.checked)}
            />
            <span class="text-xs text-gray-600 dark:text-gray-300">
              I understand my health summary will be sent to the AI endpoint configured in Settings → AI Endpoints.
            </span>
          </label>
          <button
            onClick={generateReport}
            disabled={!consentChecked() || reportLoading()}
            class="px-4 py-2 text-sm font-medium rounded-lg bg-sky-500 text-white hover:bg-sky-600 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {reportLoading() ? 'Generating… (up to 90s)' : 'Generate Report →'}
          </button>
          <Show when={reportError()}>
            <p class="text-xs text-red-500 mt-2">{reportError()}</p>
          </Show>
        </div>

        {/* Previous reports */}
        <Show when={reports().length > 0}>
          <h3 class="text-sm font-semibold text-gray-600 dark:text-gray-300 mb-2">
            Previous Reports ({reports().length})
          </h3>
          <div class="space-y-2">
            <For each={reports()}>
              {(r) => (
                <div class="border border-gray-200 dark:border-gray-700 rounded-xl overflow-hidden">
                  <div
                    class="flex items-center justify-between p-3 cursor-pointer hover:bg-gray-50 dark:hover:bg-gray-800/50"
                    onClick={() => setExpandedReport((prev) => prev === r.id ? null : r.id)}
                  >
                    <div>
                      <p class="text-sm font-medium text-gray-700 dark:text-gray-200">
                        {new Date(r.generated_at).toLocaleDateString()}
                      </p>
                      <p class="text-xs text-gray-400">{r.model_used}</p>
                    </div>
                    <div class="flex items-center gap-2">
                      <button
                        onClick={(e) => { e.stopPropagation(); navigator.clipboard.writeText(r.report_text); }}
                        class="text-xs text-sky-600 hover:text-sky-800 dark:text-sky-400 px-2 py-0.5 rounded hover:bg-sky-50"
                      >
                        Copy
                      </button>
                      <button
                        onClick={(e) => { e.stopPropagation(); deleteReport(r.id); }}
                        class="text-xs text-red-400 hover:text-red-600 px-2 py-0.5 rounded hover:bg-red-50"
                      >
                        Delete
                      </button>
                      <span class="text-xs text-gray-400">{expandedReport() === r.id ? '▲' : '▼'}</span>
                    </div>
                  </div>
                  <Show when={expandedReport() === r.id}>
                    <div class="border-t border-gray-100 dark:border-gray-700 p-4">
                      <p class="text-sm text-gray-700 dark:text-gray-200 whitespace-pre-wrap leading-relaxed">
                        {r.report_text}
                      </p>
                    </div>
                  </Show>
                </div>
              )}
            </For>
          </div>
        </Show>
      </div>
    </div>
  );
};

export default IntelligenceTab;
