import { Component, createSignal, createEffect, For, Show } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

interface Episode {
  id: string;
  patient_id: string;
  name: string;
  description?: string;
  start_date: string;
  end_date?: string;
  primary_condition?: string;
  ai_generated: boolean;
  user_confirmed: boolean;
  created_at: string;
  event_count: number;
}

interface TimelineEvent {
  id: string;
  kind: string;
  layer: string;
  title: string;
  description?: string;
  date: string;
  value?: number;
  unit?: string;
  flag?: string;
  episode_id?: string;
}

function kindLabel(kind: string): string {
  return kind.replace(/_/g, ' ');
}

const EpisodesTab: Component<{ patientId: string }> = (props) => {
  const [episodes, setEpisodes] = createSignal<Episode[]>([]);
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [expanded, setExpanded] = createSignal<Record<string, TimelineEvent[]>>({});
  const [expandedLoading, setExpandedLoading] = createSignal<Record<string, boolean>>({});
  const [editing, setEditing] = createSignal<Episode | null>(null);
  const [creating, setCreating] = createSignal(false);
  const [autoLinking, setAutoLinking] = createSignal(false);
  const [toast, setToast] = createSignal<string | null>(null);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await invoke<Episode[]>('health_episode_list', {
        patient_id: props.patientId,
      });
      setEpisodes(data);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  createEffect(() => {
    props.patientId;
    load();
  });

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 4000);
  };

  const autoLink = async () => {
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

  const toggleExpand = async (ep: Episode) => {
    const cur = expanded();
    if (cur[ep.id]) {
      const next = { ...cur };
      delete next[ep.id];
      setExpanded(next);
      return;
    }
    setExpandedLoading((l) => ({ ...l, [ep.id]: true }));
    try {
      const all = await invoke<TimelineEvent[]>('health_timeline_get', {
        patient_id: props.patientId,
      });
      const members = all.filter((e) => e.episode_id === ep.id);
      setExpanded((x) => ({ ...x, [ep.id]: members }));
    } catch (e) {
      showToast(`Error loading events: ${e}`);
    } finally {
      setExpandedLoading((l) => {
        const next = { ...l };
        delete next[ep.id];
        return next;
      });
    }
  };

  const confirm_ = async (ep: Episode) => {
    try {
      await invoke('health_episode_update', { id: ep.id, user_confirmed: true });
      load();
    } catch (e) {
      showToast(`Error: ${e}`);
    }
  };

  const remove = async (ep: Episode) => {
    if (!confirm(`Delete episode "${ep.name}"?`)) return;
    try {
      await invoke('health_episode_delete', { id: ep.id });
      load();
    } catch (e) {
      showToast(`Error: ${e}`);
    }
  };

  return (
    <div>
      <div class="flex justify-between items-center mb-4">
        <div>
          <h2 class="text-lg font-semibold">Episodes</h2>
          <p class="text-xs text-gray-500">
            Group related events into a named clinical episode.
          </p>
        </div>
        <div class="flex gap-2">
          <button
            class="btn btn-secondary text-sm"
            onClick={autoLink}
            disabled={autoLinking()}
          >
            {autoLinking() ? 'Linking…' : 'Auto-link'}
          </button>
          <button class="btn btn-primary text-sm" onClick={() => setCreating(true)}>
            + New Episode
          </button>
        </div>
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

      <Show
        when={!loading()}
        fallback={<div class="text-center text-gray-500 py-12">Loading episodes…</div>}
      >
        <Show
          when={episodes().length > 0}
          fallback={
            <div class="card p-8 text-center text-gray-500">
              No episodes yet. Create one or run Auto-link.
            </div>
          }
        >
          <div class="space-y-2">
            <For each={episodes()}>
              {(ep) => (
                <div class="card p-4">
                  <div class="flex items-start justify-between gap-4">
                    <div class="flex-1 min-w-0">
                      <div class="flex items-center gap-2 flex-wrap">
                        <div class="font-semibold">{ep.name}</div>
                        <Show when={ep.ai_generated && !ep.user_confirmed}>
                          <span class="px-2 py-0.5 rounded-full bg-amber-100 dark:bg-amber-900/30 text-amber-700 dark:text-amber-300 text-[10px]">
                            AI · unconfirmed
                          </span>
                        </Show>
                        <Show when={ep.user_confirmed}>
                          <span class="px-2 py-0.5 rounded-full bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-300 text-[10px]">
                            confirmed
                          </span>
                        </Show>
                        <Show when={ep.primary_condition}>
                          <span class="px-2 py-0.5 rounded-full bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 text-[10px]">
                            {ep.primary_condition}
                          </span>
                        </Show>
                      </div>
                      <div class="text-xs text-gray-500 mt-1">
                        {ep.start_date.slice(0, 10)}
                        {ep.end_date ? ` → ${ep.end_date.slice(0, 10)}` : ' → ongoing'}
                        <span class="mx-2">·</span>
                        {ep.event_count} event{ep.event_count === 1 ? '' : 's'}
                      </div>
                      <Show when={ep.description}>
                        <div class="text-sm text-gray-600 dark:text-gray-400 mt-2">
                          {ep.description}
                        </div>
                      </Show>
                    </div>
                    <div class="flex flex-col gap-1 items-end shrink-0">
                      <button
                        class="text-xs text-minion-600 dark:text-minion-400 hover:underline"
                        onClick={() => toggleExpand(ep)}
                      >
                        {expanded()[ep.id] ? 'Hide events' : 'Show events'}
                      </button>
                      <Show when={ep.ai_generated && !ep.user_confirmed}>
                        <button
                          class="text-xs text-green-600 hover:underline"
                          onClick={() => confirm_(ep)}
                        >
                          Confirm
                        </button>
                      </Show>
                      <button
                        class="text-xs text-gray-600 hover:underline"
                        onClick={() => setEditing(ep)}
                      >
                        Edit
                      </button>
                      <button
                        class="text-xs text-red-500 hover:underline"
                        onClick={() => remove(ep)}
                      >
                        Delete
                      </button>
                    </div>
                  </div>

                  <Show when={expanded()[ep.id] !== undefined || expandedLoading()[ep.id]}>
                    <div class="mt-3 pt-3 border-t border-gray-200 dark:border-gray-700">
                      <Show
                        when={!expandedLoading()[ep.id]}
                        fallback={<div class="text-xs text-gray-500">Loading events…</div>}
                      >
                        <Show
                          when={(expanded()[ep.id] || []).length > 0}
                          fallback={
                            <div class="text-xs text-gray-500">
                              No events linked to this episode.
                            </div>
                          }
                        >
                          <div class="space-y-1">
                            <For each={expanded()[ep.id]}>
                              {(m) => (
                                <div class="flex items-center justify-between text-xs py-1.5 px-2 rounded bg-gray-50 dark:bg-gray-900/40">
                                  <div class="flex items-center gap-2 min-w-0">
                                    <span class="px-1.5 py-0.5 rounded text-[10px] bg-gray-200 dark:bg-gray-700 text-gray-700 dark:text-gray-300">
                                      {kindLabel(m.kind)}
                                    </span>
                                    <span class="truncate">{m.title}</span>
                                  </div>
                                  <span class="text-gray-500 shrink-0 ml-2">
                                    {m.date.slice(0, 10)}
                                  </span>
                                </div>
                              )}
                            </For>
                          </div>
                        </Show>
                      </Show>
                    </div>
                  </Show>
                </div>
              )}
            </For>
          </div>
        </Show>
      </Show>

      <Show when={creating()}>
        <EpisodeFormModal
          patientId={props.patientId}
          mode="create"
          onClose={() => setCreating(false)}
          onSaved={() => {
            setCreating(false);
            load();
          }}
        />
      </Show>

      <Show when={editing()}>
        <EpisodeFormModal
          patientId={props.patientId}
          mode="edit"
          episode={editing()!}
          onClose={() => setEditing(null)}
          onSaved={() => {
            setEditing(null);
            load();
          }}
        />
      </Show>
    </div>
  );
};

const EpisodeFormModal: Component<{
  patientId: string;
  mode: 'create' | 'edit';
  episode?: Episode;
  onClose: () => void;
  onSaved: () => void;
}> = (props) => {
  const [name, setName] = createSignal(props.episode?.name ?? '');
  const [description, setDescription] = createSignal(props.episode?.description ?? '');
  const [startDate, setStartDate] = createSignal(
    props.episode?.start_date.slice(0, 10) ?? new Date().toISOString().slice(0, 10),
  );
  const [endDate, setEndDate] = createSignal(props.episode?.end_date?.slice(0, 10) ?? '');
  const [primaryCondition, setPrimaryCondition] = createSignal(
    props.episode?.primary_condition ?? '',
  );
  const [saving, setSaving] = createSignal(false);
  const [err, setErr] = createSignal<string | null>(null);

  const save = async () => {
    if (!name().trim()) return;
    setSaving(true);
    setErr(null);
    try {
      if (props.mode === 'create') {
        await invoke<string>('health_episode_create', {
          patient_id: props.patientId,
          name: name().trim(),
          description: description().trim() || null,
          start_date: startDate(),
          end_date: endDate() || null,
          primary_condition: primaryCondition().trim() || null,
        });
      } else {
        await invoke('health_episode_update', {
          id: props.episode!.id,
          name: name().trim(),
          description: description().trim() || null,
          end_date: endDate() || null,
          primary_condition: primaryCondition().trim() || null,
        });
      }
      props.onSaved();
    } catch (e) {
      setErr(String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4">
      <div class="card w-full max-w-md shadow-2xl">
        <div class="p-6">
          <h3 class="text-lg font-bold mb-4">
            {props.mode === 'create' ? 'New episode' : 'Edit episode'}
          </h3>
          <div class="space-y-3">
            <div>
              <label class="block text-xs font-medium mb-1">Name *</label>
              <input
                type="text"
                class="input w-full"
                value={name()}
                onInput={(e) => setName(e.currentTarget.value)}
              />
            </div>
            <div>
              <label class="block text-xs font-medium mb-1">Description</label>
              <textarea
                class="input w-full"
                rows="3"
                value={description()}
                onInput={(e) => setDescription(e.currentTarget.value)}
              />
            </div>
            <div class="grid grid-cols-2 gap-3">
              <div>
                <label class="block text-xs font-medium mb-1">Start date *</label>
                <input
                  type="date"
                  class="input w-full"
                  value={startDate()}
                  onInput={(e) => setStartDate(e.currentTarget.value)}
                  disabled={props.mode === 'edit'}
                />
              </div>
              <div>
                <label class="block text-xs font-medium mb-1">End date</label>
                <input
                  type="date"
                  class="input w-full"
                  value={endDate()}
                  onInput={(e) => setEndDate(e.currentTarget.value)}
                />
              </div>
            </div>
            <div>
              <label class="block text-xs font-medium mb-1">Primary condition</label>
              <input
                type="text"
                class="input w-full"
                placeholder="e.g., hypertension, viral fever"
                value={primaryCondition()}
                onInput={(e) => setPrimaryCondition(e.currentTarget.value)}
              />
            </div>
          </div>
          <Show when={err()}>
            <div class="mt-3 text-xs text-red-600 dark:text-red-400">{err()}</div>
          </Show>
          <div class="flex gap-2 justify-end mt-6">
            <button class="btn btn-secondary" onClick={props.onClose} disabled={saving()}>
              Cancel
            </button>
            <button
              class="btn btn-primary"
              onClick={save}
              disabled={saving() || !name().trim()}
            >
              {saving() ? 'Saving…' : 'Save'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};

export default EpisodesTab;
