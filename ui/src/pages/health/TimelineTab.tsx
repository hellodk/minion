import { Component, createSignal, createMemo, createEffect, For, Show } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

interface TimelineEvent {
  id: string;
  kind: string;
  layer: string;
  title: string;
  description?: string;
  category?: string;
  date: string;
  end_date?: string;
  value?: number;
  unit?: string;
  flag?: string;
  episode_id?: string;
}

interface Correlation {
  id: string;
  source_kind: string;
  source_id: string;
  source_title: string;
  source_date: string;
  target_kind: string;
  target_id: string;
  target_title: string;
  target_date: string;
  relation: string;
  confidence: number;
  delta_days: number;
}

type Preset = '3M' | '6M' | '12M' | 'all';

function isoDate(d: Date): string {
  return d.toISOString().slice(0, 10);
}

function parseDate(s: string): Date {
  return new Date(s.length === 10 ? s + 'T00:00:00' : s);
}

function monthTicks(start: Date, end: Date): Date[] {
  const ticks: Date[] = [];
  const cur = new Date(start.getFullYear(), start.getMonth(), 1);
  while (cur <= end) {
    if (cur >= start) ticks.push(new Date(cur));
    cur.setMonth(cur.getMonth() + 1);
  }
  return ticks;
}

function formatMonth(d: Date): string {
  return d.toLocaleString('en', { month: 'short', year: '2-digit' });
}

function eventColor(kind: string, flag?: string): string {
  switch (kind) {
    case 'medical_record':
      return '#3b82f6';
    case 'medication':
      return '#8b5cf6';
    case 'condition':
      return '#ef4444';
    case 'life_event':
      return '#10b981';
    case 'symptom':
      return '#f97316';
    case 'lab_test':
    case 'vital':
      if (flag === 'H' || flag === 'HH' || flag === 'L' || flag === 'LL') {
        return flag === 'HH' || flag === 'LL' ? '#dc2626' : '#eab308';
      }
      return '#9ca3af';
    default:
      return '#6b7280';
  }
}

function kindLabel(kind: string): string {
  return kind.replace(/_/g, ' ');
}

const TimelineTab: Component<{ patientId: string }> = (props) => {
  const [preset, setPreset] = createSignal<Preset>('12M');
  const [customFrom, setCustomFrom] = createSignal('');
  const [customTo, setCustomTo] = createSignal('');
  const [events, setEvents] = createSignal<TimelineEvent[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [selected, setSelected] = createSignal<TimelineEvent | null>(null);
  const [correlations, setCorrelations] = createSignal<Correlation[]>([]);
  const [loadingCorr, setLoadingCorr] = createSignal(false);
  const [correlating, setCorrelating] = createSignal(false);
  const [autoLinking, setAutoLinking] = createSignal(false);
  const [toast, setToast] = createSignal<string | null>(null);
  const [hovered, setHovered] = createSignal<{ ev: TimelineEvent; x: number; y: number } | null>(
    null,
  );

  const range = createMemo(() => {
    const now = new Date();
    let from: Date;
    let to = now;
    if (customFrom() && customTo()) {
      from = parseDate(customFrom());
      to = parseDate(customTo());
    } else {
      switch (preset()) {
        case '3M':
          from = new Date(now);
          from.setMonth(from.getMonth() - 3);
          break;
        case '6M':
          from = new Date(now);
          from.setMonth(from.getMonth() - 6);
          break;
        case 'all':
          from = new Date(2000, 0, 1);
          break;
        case '12M':
        default:
          from = new Date(now);
          from.setMonth(from.getMonth() - 12);
      }
    }
    return { from, to };
  });

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const { from, to } = range();
      const data = await invoke<TimelineEvent[]>('health_timeline_get', {
        patient_id: props.patientId,
        from: isoDate(from),
        to: isoDate(to),
      });
      setEvents(data);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  createEffect(() => {
    props.patientId;
    preset();
    customFrom();
    customTo();
    load();
  });

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 4000);
  };

  const recomputeCorrelations = async () => {
    setCorrelating(true);
    try {
      const res = await invoke<{ correlations_created: number; sources_scanned: number }>(
        'health_correlate',
        { patient_id: props.patientId, window_days: 30 },
      );
      showToast(
        `Scanned ${res.sources_scanned} events, created ${res.correlations_created} correlations`,
      );
      if (selected()) loadCorrelations(selected()!);
    } catch (e) {
      showToast(`Error: ${e}`);
    } finally {
      setCorrelating(false);
    }
  };

  const autoLinkEpisodes = async () => {
    setAutoLinking(true);
    try {
      const res = await invoke<{ episodes_created: number; events_linked: number }>(
        'health_episode_autolink',
        { patient_id: props.patientId, gap_days: 14 },
      );
      showToast(
        `Created ${res.episodes_created} episodes, linked ${res.events_linked} events`,
      );
      load();
    } catch (e) {
      showToast(`Error: ${e}`);
    } finally {
      setAutoLinking(false);
    }
  };

  const loadCorrelations = async (ev: TimelineEvent) => {
    setLoadingCorr(true);
    try {
      const data = await invoke<Correlation[]>('health_list_correlations', {
        patient_id: props.patientId,
        source_kind: ev.kind,
        source_id: ev.id,
        min_confidence: 0,
      });
      setCorrelations(data);
    } catch (e) {
      setCorrelations([]);
    } finally {
      setLoadingCorr(false);
    }
  };

  const selectEvent = (ev: TimelineEvent) => {
    setSelected(ev);
    loadCorrelations(ev);
  };

  const byLayer = createMemo(() => {
    const evs = events();
    return {
      events: evs.filter((e) => e.layer === 'events'),
      symptoms: evs.filter((e) => e.layer === 'symptoms'),
      labs: evs.filter((e) => e.layer === 'labs'),
    };
  });

  const xPos = (dateStr: string): number => {
    const { from, to } = range();
    const d = parseDate(dateStr).getTime();
    const f = from.getTime();
    const t = to.getTime();
    if (t <= f) return 0;
    return Math.max(0, Math.min(100, ((d - f) / (t - f)) * 100));
  };

  const symptomSize = (severity?: number): number => {
    const s = severity || 5;
    return 6 + (s / 10) * 10;
  };

  return (
    <div>
      <div class="flex flex-wrap items-center gap-2 mb-4">
        <div class="flex gap-1">
          <For each={['3M', '6M', '12M', 'all'] as Preset[]}>
            {(p) => (
              <button
                class="px-3 py-1.5 text-xs rounded-md border transition-colors"
                classList={{
                  'bg-minion-600 text-white border-minion-600': preset() === p && !customFrom(),
                  'bg-white dark:bg-gray-800 border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700':
                    preset() !== p || !!customFrom(),
                }}
                onClick={() => {
                  setCustomFrom('');
                  setCustomTo('');
                  setPreset(p);
                }}
              >
                {p === 'all' ? 'All' : `Last ${p}`}
              </button>
            )}
          </For>
        </div>
        <div class="flex items-center gap-1 text-xs">
          <span class="text-gray-500">From</span>
          <input
            type="date"
            class="input py-1 text-xs w-36"
            value={customFrom()}
            onInput={(e) => setCustomFrom(e.currentTarget.value)}
          />
          <span class="text-gray-500">to</span>
          <input
            type="date"
            class="input py-1 text-xs w-36"
            value={customTo()}
            onInput={(e) => setCustomTo(e.currentTarget.value)}
          />
        </div>
        <div class="flex-1" />
        <button
          class="btn btn-secondary text-xs"
          onClick={recomputeCorrelations}
          disabled={correlating()}
        >
          {correlating() ? 'Computing…' : 'Recompute correlations'}
        </button>
        <button
          class="btn btn-secondary text-xs"
          onClick={autoLinkEpisodes}
          disabled={autoLinking()}
        >
          {autoLinking() ? 'Linking…' : 'Auto-link episodes'}
        </button>
      </div>

      <Show when={toast()}>
        <div class="mb-3 px-3 py-2 rounded-lg bg-minion-50 dark:bg-minion-900/30 text-minion-700 dark:text-minion-300 text-sm">
          {toast()}
        </div>
      </Show>

      <Show when={error()}>
        <div class="mb-3 px-3 py-2 rounded-lg bg-red-50 dark:bg-red-900/30 text-red-700 dark:text-red-300 text-sm">
          {error()}
        </div>
      </Show>

      <div class="grid grid-cols-1 lg:grid-cols-[1fr_360px] gap-4">
        <div class="card p-4 relative">
          <Show
            when={!loading()}
            fallback={<div class="text-center text-gray-500 py-16">Loading timeline…</div>}
          >
            <Show
              when={events().length > 0}
              fallback={
                <div class="text-center text-gray-500 py-16">
                  No events in this date range. Add records or import documents.
                </div>
              }
            >
              <TimelineChart
                range={range()}
                byLayer={byLayer()}
                xPos={xPos}
                symptomSize={symptomSize}
                onSelect={selectEvent}
                onHover={setHovered}
                selectedId={selected()?.id}
              />
            </Show>
          </Show>

          <Show when={hovered()}>
            <div
              class="absolute pointer-events-none bg-gray-900 text-white text-xs rounded-md px-2 py-1 shadow-lg z-10"
              style={{
                left: `${hovered()!.x}px`,
                top: `${hovered()!.y}px`,
                transform: 'translate(-50%, -110%)',
              }}
            >
              <div class="font-semibold">{hovered()!.ev.title}</div>
              <div class="text-gray-300">
                {kindLabel(hovered()!.ev.kind)} · {hovered()!.ev.date.slice(0, 10)}
              </div>
              <Show when={hovered()!.ev.value !== undefined && hovered()!.ev.value !== null}>
                <div class="text-gray-300">
                  {hovered()!.ev.value} {hovered()!.ev.unit || ''}
                  {hovered()!.ev.flag && <span class="ml-1">[{hovered()!.ev.flag}]</span>}
                </div>
              </Show>
            </div>
          </Show>
        </div>

        <DetailPanel
          event={selected()}
          correlations={correlations()}
          loadingCorr={loadingCorr()}
          onClose={() => setSelected(null)}
        />
      </div>
    </div>
  );
};

const TimelineChart: Component<{
  range: { from: Date; to: Date };
  byLayer: { events: TimelineEvent[]; symptoms: TimelineEvent[]; labs: TimelineEvent[] };
  xPos: (date: string) => number;
  symptomSize: (severity?: number) => number;
  onSelect: (ev: TimelineEvent) => void;
  onHover: (h: { ev: TimelineEvent; x: number; y: number } | null) => void;
  selectedId?: string;
}> = (props) => {
  const ticks = createMemo(() => monthTicks(props.range.from, props.range.to));

  const hoverHandler = (ev: TimelineEvent) => (e: MouseEvent) => {
    const target = e.currentTarget as HTMLElement;
    const rect = target.getBoundingClientRect();
    const parentRect = target.closest('.card')!.getBoundingClientRect();
    props.onHover({
      ev,
      x: rect.left - parentRect.left + rect.width / 2,
      y: rect.top - parentRect.top,
    });
  };

  return (
    <div>
      <div class="relative h-6 border-b border-gray-200 dark:border-gray-700 mb-2">
        <For each={ticks()}>
          {(t) => (
            <div
              class="absolute top-0 text-[10px] text-gray-500"
              style={{ left: `${((t.getTime() - props.range.from.getTime()) / (props.range.to.getTime() - props.range.from.getTime())) * 100}%`, transform: 'translateX(-50%)' }}
            >
              <div class="border-l border-gray-300 dark:border-gray-600 h-2" />
              <div class="mt-0.5 whitespace-nowrap">{formatMonth(t)}</div>
            </div>
          )}
        </For>
      </div>

      <TimelineLayer
        title="Events"
        hint="records, medications, conditions, life events"
        items={props.byLayer.events}
        xPos={props.xPos}
        onSelect={props.onSelect}
        onHover={hoverHandler}
        onLeave={() => props.onHover(null)}
        selectedId={props.selectedId}
        renderItem={(ev) => (
          <div
            class="w-3 h-3 rounded-sm cursor-pointer border-2 transition-transform hover:scale-150"
            style={{
              'background-color': eventColor(ev.kind),
              'border-color': props.selectedId === ev.id ? '#000' : 'transparent',
            }}
          />
        )}
      />

      <TimelineLayer
        title="Symptoms"
        hint="severity shown by size"
        items={props.byLayer.symptoms}
        xPos={props.xPos}
        onSelect={props.onSelect}
        onHover={hoverHandler}
        onLeave={() => props.onHover(null)}
        selectedId={props.selectedId}
        renderItem={(ev) => {
          const size = props.symptomSize(ev.value);
          return (
            <div
              class="rounded-full cursor-pointer border-2 transition-transform hover:scale-125"
              style={{
                width: `${size}px`,
                height: `${size}px`,
                'background-color': eventColor('symptom'),
                'border-color': props.selectedId === ev.id ? '#000' : 'transparent',
              }}
            />
          );
        }}
      />

      <TimelineLayer
        title="Labs"
        hint="lab tests & vitals; red=abnormal"
        items={props.byLayer.labs}
        xPos={props.xPos}
        onSelect={props.onSelect}
        onHover={hoverHandler}
        onLeave={() => props.onHover(null)}
        selectedId={props.selectedId}
        renderItem={(ev) => (
          <div
            class="w-2.5 h-4 cursor-pointer border-2 transition-transform hover:scale-150"
            style={{
              'background-color': eventColor(ev.kind, ev.flag),
              'border-color': props.selectedId === ev.id ? '#000' : 'transparent',
            }}
          />
        )}
      />

      <div class="flex gap-3 mt-4 text-[10px] text-gray-500 flex-wrap">
        <LegendItem color="#3b82f6" label="Record" />
        <LegendItem color="#8b5cf6" label="Medication" />
        <LegendItem color="#ef4444" label="Condition" />
        <LegendItem color="#10b981" label="Life event" />
        <LegendItem color="#f97316" label="Symptom" />
        <LegendItem color="#9ca3af" label="Lab (normal)" />
        <LegendItem color="#eab308" label="Lab (H/L)" />
        <LegendItem color="#dc2626" label="Lab (HH/LL)" />
      </div>
    </div>
  );
};

const LegendItem: Component<{ color: string; label: string }> = (props) => (
  <div class="flex items-center gap-1">
    <div class="w-3 h-3 rounded-sm" style={{ 'background-color': props.color }} />
    <span>{props.label}</span>
  </div>
);

const TimelineLayer: Component<{
  title: string;
  hint: string;
  items: TimelineEvent[];
  xPos: (date: string) => number;
  onSelect: (ev: TimelineEvent) => void;
  onHover: (ev: TimelineEvent) => (e: MouseEvent) => void;
  onLeave: () => void;
  selectedId?: string;
  renderItem: (ev: TimelineEvent) => any;
}> = (props) => {
  return (
    <div class="mb-3">
      <div class="flex items-baseline gap-2 mb-1">
        <div class="text-xs font-semibold text-gray-700 dark:text-gray-300">{props.title}</div>
        <div class="text-[10px] text-gray-400">{props.hint}</div>
        <div class="text-[10px] text-gray-400 ml-auto">{props.items.length}</div>
      </div>
      <div class="relative h-10 bg-gray-50 dark:bg-gray-900/40 rounded-md border border-gray-200 dark:border-gray-700">
        <For each={props.items}>
          {(ev) => (
            <div
              class="absolute top-1/2"
              style={{ left: `${props.xPos(ev.date)}%`, transform: 'translate(-50%, -50%)' }}
              onClick={() => props.onSelect(ev)}
              onMouseEnter={props.onHover(ev)}
              onMouseLeave={props.onLeave}
            >
              {props.renderItem(ev)}
            </div>
          )}
        </For>
      </div>
    </div>
  );
};

const DetailPanel: Component<{
  event: TimelineEvent | null;
  correlations: Correlation[];
  loadingCorr: boolean;
  onClose: () => void;
}> = (props) => {
  const grouped = createMemo(() => {
    const by: Record<string, Correlation[]> = { precedes: [], follows: [], concurrent: [] };
    for (const c of props.correlations) {
      if (by[c.relation]) by[c.relation].push(c);
    }
    return by;
  });

  return (
    <div class="card p-4 h-fit">
      <Show
        when={props.event}
        fallback={
          <div class="text-sm text-gray-500 text-center py-8">
            Click an event on the timeline to see details and correlations.
          </div>
        }
      >
        <div class="flex justify-between items-start mb-3">
          <div class="flex-1">
            <div class="text-xs uppercase tracking-wide text-gray-500">
              {kindLabel(props.event!.kind)}
            </div>
            <div class="font-semibold text-sm">{props.event!.title}</div>
          </div>
          <button
            class="text-xs text-gray-500 hover:text-gray-900 dark:hover:text-gray-100"
            onClick={props.onClose}
          >
            ✕
          </button>
        </div>

        <div class="space-y-1 text-xs">
          <div>
            <span class="text-gray-500">Date:</span>{' '}
            <span class="font-medium">{props.event!.date.slice(0, 10)}</span>
            <Show when={props.event!.end_date}>
              <span class="text-gray-500"> → {props.event!.end_date!.slice(0, 10)}</span>
            </Show>
          </div>
          <Show when={props.event!.category}>
            <div>
              <span class="text-gray-500">Category:</span> {props.event!.category}
            </div>
          </Show>
          <Show when={props.event!.value !== undefined && props.event!.value !== null}>
            <div>
              <span class="text-gray-500">Value:</span>{' '}
              <span class="font-medium">
                {props.event!.value} {props.event!.unit || ''}
              </span>
              <Show when={props.event!.flag}>
                <span class="ml-1 px-1.5 py-0.5 rounded bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-300 text-[10px]">
                  {props.event!.flag}
                </span>
              </Show>
            </div>
          </Show>
          <Show when={props.event!.description}>
            <div class="text-gray-600 dark:text-gray-400 pt-1">{props.event!.description}</div>
          </Show>
          <Show when={props.event!.episode_id}>
            <div>
              <span class="inline-block mt-1 px-2 py-0.5 rounded-full bg-purple-100 dark:bg-purple-900/30 text-purple-700 dark:text-purple-300 text-[10px]">
                Episode: {props.event!.episode_id!.slice(0, 8)}
              </span>
            </div>
          </Show>
        </div>

        <div class="mt-4 border-t border-gray-200 dark:border-gray-700 pt-3">
          <div class="text-xs font-semibold mb-2 text-gray-700 dark:text-gray-300">
            Correlations
          </div>
          <Show
            when={!props.loadingCorr}
            fallback={<div class="text-xs text-gray-500">Loading…</div>}
          >
            <Show
              when={props.correlations.length > 0}
              fallback={
                <div class="text-xs text-gray-500">
                  No correlations. Try "Recompute correlations".
                </div>
              }
            >
              <For each={['precedes', 'follows', 'concurrent']}>
                {(rel) => (
                  <Show when={grouped()[rel]?.length > 0}>
                    <div class="mb-3">
                      <div class="text-[10px] uppercase tracking-wide text-gray-500 mb-1">
                        {rel}
                      </div>
                      <div class="space-y-1">
                        <For each={grouped()[rel]}>
                          {(c) => (
                            <div class="p-2 rounded-md bg-gray-50 dark:bg-gray-900/40 border border-gray-200 dark:border-gray-700">
                              <div class="text-xs font-medium">{c.target_title}</div>
                              <div class="text-[10px] text-gray-500 flex items-center justify-between">
                                <span>
                                  {kindLabel(c.target_kind)} · {c.target_date.slice(0, 10)} ·{' '}
                                  Δ {c.delta_days}d
                                </span>
                              </div>
                              <div class="mt-1 flex items-center gap-1">
                                <div class="flex-1 h-1 bg-gray-200 dark:bg-gray-700 rounded overflow-hidden">
                                  <div
                                    class="h-full bg-minion-500"
                                    style={{ width: `${Math.round(c.confidence * 100)}%` }}
                                  />
                                </div>
                                <span class="text-[10px] text-gray-500 w-8 text-right">
                                  {Math.round(c.confidence * 100)}%
                                </span>
                              </div>
                            </div>
                          )}
                        </For>
                      </div>
                    </div>
                  </Show>
                )}
              </For>
            </Show>
          </Show>
        </div>
      </Show>
    </div>
  );
};

export default TimelineTab;
