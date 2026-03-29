import { Component, createSignal, createMemo, For, Show, onMount } from 'solid-js';
import { invoke } from '@tauri-apps/api/core';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type ViewMode = 'month' | 'week' | 'agenda';

interface CalendarEvent {
  id: string;
  title: string;
  description: string | null;
  start_time: string;
  end_time: string | null;
  all_day: boolean;
  location: string | null;
  color: string;
  source: string;
  calendar_name: string | null;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const WEEKDAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];

function startOfMonth(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth(), 1);
}

function endOfMonth(d: Date): Date {
  return new Date(d.getFullYear(), d.getMonth() + 1, 0);
}

function isSameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}

function formatTime(iso: string): string {
  const d = new Date(iso);
  return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

function formatDate(d: Date): string {
  return d.toLocaleDateString(undefined, {
    weekday: 'long',
    year: 'numeric',
    month: 'long',
    day: 'numeric',
  });
}

function monthLabel(d: Date): string {
  return d.toLocaleDateString(undefined, { year: 'numeric', month: 'long' });
}

/** Get ISO date string in YYYY-MM-DD */
function toDateStr(d: Date): string {
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, '0');
  const day = String(d.getDate()).padStart(2, '0');
  return `${y}-${m}-${day}`;
}

/** Monday-based day of week (0=Mon .. 6=Sun) */
function dayOfWeekMon(d: Date): number {
  return (d.getDay() + 6) % 7;
}

function startOfWeek(d: Date): Date {
  const diff = dayOfWeekMon(d);
  const r = new Date(d);
  r.setDate(r.getDate() - diff);
  return r;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

const Calendar: Component = () => {
  const [viewMode, setViewMode] = createSignal<ViewMode>('month');
  const [currentDate, setCurrentDate] = createSignal(new Date());
  const [selectedDate, setSelectedDate] = createSignal(new Date());
  const [events, setEvents] = createSignal<CalendarEvent[]>([]);
  const [showAddForm, setShowAddForm] = createSignal(false);

  // Integration state
  const [googleConnected, setGoogleConnected] = createSignal(false);
  const [outlookConnected, setOutlookConnected] = createSignal(false);
  void setOutlookConnected; // will be used once Outlook OAuth flow is complete
  const [syncStatus, setSyncStatus] = createSignal<'idle' | 'syncing' | 'success' | 'error'>('idle');
  const [syncMessage, setSyncMessage] = createSignal('');

  // Add-event form state
  const [formTitle, setFormTitle] = createSignal('');
  const [formDate, setFormDate] = createSignal('');
  const [formTime, setFormTime] = createSignal('09:00');
  const [formDuration, setFormDuration] = createSignal('60');
  const [formColor, setFormColor] = createSignal('#0ea5e9');
  const [formDescription, setFormDescription] = createSignal('');
  const [formLocation, setFormLocation] = createSignal('');
  const [formAllDay, setFormAllDay] = createSignal(false);

  // ----- Data fetching -----

  const fetchEvents = async () => {
    try {
      const cur = currentDate();
      const from = new Date(cur.getFullYear(), cur.getMonth() - 1, 1);
      const to = new Date(cur.getFullYear(), cur.getMonth() + 2, 0);
      const result = await invoke<CalendarEvent[]>('calendar_list_events', {
        from: from.toISOString(),
        to: to.toISOString(),
      });
      setEvents(result);
    } catch (e) {
      console.error('Failed to fetch calendar events:', e);
    }
  };

  onMount(async () => {
    setFormDate(toDateStr(new Date()));
    await fetchEvents();
    // Check Google connection (reuses Fit token)
    try {
      const connected = await invoke<boolean>('gfit_check_connected');
      setGoogleConnected(connected);
    } catch (_) {
      // ignore
    }
  });

  // Refetch when month changes
  const navigateMonth = (delta: number) => {
    const cur = currentDate();
    setCurrentDate(new Date(cur.getFullYear(), cur.getMonth() + delta, 1));
    fetchEvents();
  };

  const goToday = () => {
    const today = new Date();
    setCurrentDate(today);
    setSelectedDate(today);
    fetchEvents();
  };

  // ----- Derived data -----

  const today = new Date();

  const calendarGrid = createMemo(() => {
    const cur = currentDate();
    const first = startOfMonth(cur);
    const last = endOfMonth(cur);
    const startDow = dayOfWeekMon(first);

    const cells: { date: Date; inMonth: boolean }[] = [];

    // Days from previous month to fill the first row
    for (let i = startDow - 1; i >= 0; i--) {
      const d = new Date(first);
      d.setDate(d.getDate() - i - 1);
      cells.push({ date: d, inMonth: false });
    }

    // Days of the current month
    for (let d = 1; d <= last.getDate(); d++) {
      cells.push({
        date: new Date(cur.getFullYear(), cur.getMonth(), d),
        inMonth: true,
      });
    }

    // Fill the last row
    while (cells.length % 7 !== 0) {
      const d = new Date(last);
      d.setDate(d.getDate() + (cells.length - (startDow + last.getDate())) + 1);
      cells.push({ date: d, inMonth: false });
    }

    return cells;
  });

  const eventsForDate = (d: Date): CalendarEvent[] => {
    return events().filter((ev) => {
      const evDate = new Date(ev.start_time);
      return isSameDay(evDate, d);
    });
  };

  const selectedDayEvents = createMemo(() => eventsForDate(selectedDate()));

  const weekDays = createMemo(() => {
    const start = startOfWeek(selectedDate());
    const days: Date[] = [];
    for (let i = 0; i < 7; i++) {
      const d = new Date(start);
      d.setDate(d.getDate() + i);
      days.push(d);
    }
    return days;
  });

  const agendaEvents = createMemo(() => {
    const now = new Date();
    return events()
      .filter((ev) => new Date(ev.start_time) >= now)
      .sort((a, b) => new Date(a.start_time).getTime() - new Date(b.start_time).getTime())
      .slice(0, 30);
  });

  // ----- Actions -----

  const addEvent = async () => {
    if (!formTitle().trim()) return;
    try {
      const startStr = formAllDay()
        ? `${formDate()}T00:00:00`
        : `${formDate()}T${formTime()}:00`;
      const dur = parseInt(formDuration()) || 60;
      const startDt = new Date(startStr);
      const endDt = new Date(startDt.getTime() + dur * 60_000);

      await invoke('calendar_add_event', {
        title: formTitle(),
        startTime: startDt.toISOString(),
        endTime: formAllDay() ? null : endDt.toISOString(),
        allDay: formAllDay(),
        location: formLocation() || null,
        color: formColor(),
        description: formDescription() || null,
      });

      setFormTitle('');
      setFormDescription('');
      setFormLocation('');
      setShowAddForm(false);
      await fetchEvents();
    } catch (e) {
      console.error('Failed to add event:', e);
    }
  };

  const deleteEvent = async (id: string) => {
    try {
      await invoke('calendar_delete_event', { eventId: id });
      await fetchEvents();
    } catch (e) {
      console.error('Failed to delete event:', e);
    }
  };

  const syncGoogle = async () => {
    setSyncStatus('syncing');
    setSyncMessage('');
    try {
      const result = await invoke<string>('calendar_sync_google');
      setSyncMessage(result);
      setSyncStatus('success');
      await fetchEvents();
    } catch (e: any) {
      setSyncMessage(String(e));
      setSyncStatus('error');
    }
  };

  const openOutlookAuth = async () => {
    try {
      await invoke('calendar_open_outlook_auth');
    } catch (e: any) {
      setSyncMessage('Failed to open Outlook auth: ' + String(e));
      setSyncStatus('error');
    }
  };

  // =========================================================================
  // Render
  // =========================================================================

  return (
    <div class="p-6 max-w-full">
      {/* Header */}
      <div class="flex items-center justify-between mb-6">
        <h1 class="text-2xl font-bold">Calendar</h1>
        <div class="flex items-center gap-2">
          <button class="btn btn-primary text-sm" onClick={() => { setFormDate(toDateStr(selectedDate())); setShowAddForm(!showAddForm()); }}>
            + Add Event
          </button>
        </div>
      </div>

      {/* Integrations bar */}
      <div class="card p-3 mb-4 flex items-center gap-4 flex-wrap">
        <span class="text-sm font-medium text-gray-500 dark:text-gray-400">Integrations:</span>

        {/* Google Calendar */}
        <div class="flex items-center gap-2">
          <span class="w-2 h-2 rounded-full" classList={{ 'bg-green-500': googleConnected(), 'bg-gray-400': !googleConnected() }} />
          <span class="text-sm">Google Calendar</span>
          <Show when={googleConnected()} fallback={
            <span class="text-xs text-gray-400">Not connected</span>
          }>
            <button
              class="text-xs px-2 py-0.5 rounded bg-blue-100 dark:bg-blue-900 text-blue-700 dark:text-blue-300 hover:bg-blue-200 dark:hover:bg-blue-800"
              disabled={syncStatus() === 'syncing'}
              onClick={syncGoogle}
            >
              {syncStatus() === 'syncing' ? 'Syncing...' : 'Sync'}
            </button>
          </Show>
        </div>

        <div class="w-px h-5 bg-gray-200 dark:bg-gray-700" />

        {/* Outlook Calendar */}
        <div class="flex items-center gap-2">
          <span class="w-2 h-2 rounded-full" classList={{ 'bg-green-500': outlookConnected(), 'bg-gray-400': !outlookConnected() }} />
          <span class="text-sm">Outlook</span>
          <Show when={!outlookConnected()}>
            <button
              class="text-xs px-2 py-0.5 rounded bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700"
              onClick={openOutlookAuth}
            >
              Connect
            </button>
          </Show>
        </div>

        <Show when={syncMessage()}>
          <span
            class="text-xs ml-auto"
            classList={{
              'text-green-500': syncStatus() === 'success',
              'text-red-500': syncStatus() === 'error',
              'text-gray-400': syncStatus() === 'syncing',
            }}
          >
            {syncMessage()}
          </span>
        </Show>
      </div>

      {/* Controls row */}
      <div class="flex items-center justify-between mb-4">
        <div class="flex items-center gap-2">
          <button class="btn btn-secondary text-sm px-3" onClick={() => navigateMonth(-1)}>
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" /></svg>
          </button>
          <h2 class="text-lg font-semibold min-w-[180px] text-center">{monthLabel(currentDate())}</h2>
          <button class="btn btn-secondary text-sm px-3" onClick={() => navigateMonth(1)}>
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" /></svg>
          </button>
          <button class="btn btn-secondary text-sm ml-2" onClick={goToday}>Today</button>
        </div>

        <div class="flex rounded-lg overflow-hidden border border-gray-200 dark:border-gray-700">
          <button
            class="px-3 py-1 text-sm"
            classList={{
              'bg-minion-600 text-white': viewMode() === 'month',
              'bg-white dark:bg-gray-800 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700': viewMode() !== 'month',
            }}
            onClick={() => setViewMode('month')}
          >
            Month
          </button>
          <button
            class="px-3 py-1 text-sm border-x border-gray-200 dark:border-gray-700"
            classList={{
              'bg-minion-600 text-white': viewMode() === 'week',
              'bg-white dark:bg-gray-800 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700': viewMode() !== 'week',
            }}
            onClick={() => setViewMode('week')}
          >
            Week
          </button>
          <button
            class="px-3 py-1 text-sm"
            classList={{
              'bg-minion-600 text-white': viewMode() === 'agenda',
              'bg-white dark:bg-gray-800 text-gray-700 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700': viewMode() !== 'agenda',
            }}
            onClick={() => setViewMode('agenda')}
          >
            Agenda
          </button>
        </div>
      </div>

      {/* Main layout: calendar + sidebar */}
      <div class="flex gap-4">
        {/* Calendar area */}
        <div class="flex-1 min-w-0">
          {/* --- Month View --- */}
          <Show when={viewMode() === 'month'}>
            <div class="card overflow-hidden">
              {/* Weekday header */}
              <div class="grid grid-cols-7 border-b border-gray-200 dark:border-gray-700">
                <For each={WEEKDAYS}>
                  {(d) => (
                    <div class="py-2 text-center text-xs font-medium text-gray-500 dark:text-gray-400 uppercase">
                      {d}
                    </div>
                  )}
                </For>
              </div>

              {/* Day cells */}
              <div class="grid grid-cols-7">
                <For each={calendarGrid()}>
                  {(cell) => {
                    const dayEvents = () => eventsForDate(cell.date);
                    const isToday = isSameDay(cell.date, today);
                    const isSelected = () => isSameDay(cell.date, selectedDate());

                    return (
                      <div
                        class="min-h-[80px] p-1 border-b border-r border-gray-100 dark:border-gray-800 cursor-pointer transition-colors hover:bg-gray-50 dark:hover:bg-gray-800/50"
                        classList={{
                          'bg-blue-50/50 dark:bg-blue-900/10': isSelected(),
                          'opacity-40': !cell.inMonth,
                        }}
                        onClick={() => setSelectedDate(cell.date)}
                      >
                        <div class="flex items-center justify-center mb-0.5">
                          <span
                            class="w-7 h-7 flex items-center justify-center rounded-full text-sm"
                            classList={{
                              'bg-minion-600 text-white font-bold': isToday,
                              'font-medium': !isToday && cell.inMonth,
                            }}
                          >
                            {cell.date.getDate()}
                          </span>
                        </div>

                        {/* Event dots / pills */}
                        <div class="space-y-0.5">
                          <For each={dayEvents().slice(0, 3)}>
                            {(ev) => (
                              <div
                                class="text-[10px] leading-tight px-1 rounded truncate text-white"
                                style={{ 'background-color': ev.color }}
                                title={ev.title}
                              >
                                {ev.title}
                              </div>
                            )}
                          </For>
                          <Show when={dayEvents().length > 3}>
                            <div class="text-[10px] text-gray-500 dark:text-gray-400 px-1">
                              +{dayEvents().length - 3} more
                            </div>
                          </Show>
                        </div>
                      </div>
                    );
                  }}
                </For>
              </div>
            </div>
          </Show>

          {/* --- Week View --- */}
          <Show when={viewMode() === 'week'}>
            <div class="card overflow-hidden">
              <div class="grid grid-cols-7 border-b border-gray-200 dark:border-gray-700">
                <For each={weekDays()}>
                  {(d) => {
                    const isToday = isSameDay(d, today);
                    return (
                      <div
                        class="py-2 text-center border-r border-gray-100 dark:border-gray-800 last:border-r-0"
                        classList={{ 'bg-blue-50 dark:bg-blue-900/20': isToday }}
                      >
                        <div class="text-xs text-gray-500 dark:text-gray-400 uppercase">
                          {WEEKDAYS[dayOfWeekMon(d)]}
                        </div>
                        <div
                          class="text-lg font-semibold"
                          classList={{ 'text-minion-600': isToday }}
                        >
                          {d.getDate()}
                        </div>
                      </div>
                    );
                  }}
                </For>
              </div>

              {/* Hour rows */}
              <div class="max-h-[500px] overflow-y-auto">
                <For each={Array.from({ length: 16 }, (_, i) => i + 6)}>
                  {(hour) => (
                    <div class="grid grid-cols-7 border-b border-gray-100 dark:border-gray-800">
                      <For each={weekDays()}>
                        {(d) => {
                          const hourEvents = () =>
                            eventsForDate(d).filter((ev) => {
                              const h = new Date(ev.start_time).getHours();
                              return h === hour;
                            });
                          return (
                            <div class="min-h-[40px] p-0.5 border-r border-gray-100 dark:border-gray-800 last:border-r-0 relative">
                              <Show when={dayOfWeekMon(d) === 0}>
                                <span class="absolute left-[-2px] top-0 text-[10px] text-gray-400 select-none -translate-x-full pr-1">
                                  {String(hour).padStart(2, '0')}:00
                                </span>
                              </Show>
                              <For each={hourEvents()}>
                                {(ev) => (
                                  <div
                                    class="text-[10px] leading-tight px-1 rounded truncate text-white mb-0.5"
                                    style={{ 'background-color': ev.color }}
                                    title={ev.title}
                                  >
                                    {formatTime(ev.start_time)} {ev.title}
                                  </div>
                                )}
                              </For>
                            </div>
                          );
                        }}
                      </For>
                    </div>
                  )}
                </For>
              </div>
            </div>
          </Show>

          {/* --- Agenda View --- */}
          <Show when={viewMode() === 'agenda'}>
            <div class="card divide-y divide-gray-100 dark:divide-gray-800">
              <Show when={agendaEvents().length === 0}>
                <div class="p-8 text-center text-gray-400">No upcoming events.</div>
              </Show>
              <For each={agendaEvents()}>
                {(ev) => {
                  const d = new Date(ev.start_time);
                  return (
                    <div class="flex items-start gap-3 p-3 hover:bg-gray-50 dark:hover:bg-gray-800/50">
                      <div
                        class="w-1 self-stretch rounded-full flex-shrink-0"
                        style={{ 'background-color': ev.color }}
                      />
                      <div class="flex-1 min-w-0">
                        <div class="font-medium truncate">{ev.title}</div>
                        <div class="text-xs text-gray-500 dark:text-gray-400">
                          {formatDate(d)}
                          {!ev.all_day && ` at ${formatTime(ev.start_time)}`}
                          {ev.end_time && !ev.all_day && ` - ${formatTime(ev.end_time)}`}
                        </div>
                        <Show when={ev.location}>
                          <div class="text-xs text-gray-400 mt-0.5">{ev.location}</div>
                        </Show>
                      </div>
                      <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400 capitalize flex-shrink-0">
                        {ev.source}
                      </span>
                    </div>
                  );
                }}
              </For>
            </div>
          </Show>
        </div>

        {/* Sidebar */}
        <div class="w-72 flex-shrink-0 space-y-4">
          {/* Selected day events */}
          <div class="card p-4">
            <h3 class="text-sm font-semibold mb-3">{formatDate(selectedDate())}</h3>
            <Show
              when={selectedDayEvents().length > 0}
              fallback={<p class="text-sm text-gray-400">No events this day.</p>}
            >
              <div class="space-y-2">
                <For each={selectedDayEvents()}>
                  {(ev) => (
                    <div class="flex items-start gap-2 group">
                      <div
                        class="w-2 h-2 rounded-full mt-1.5 flex-shrink-0"
                        style={{ 'background-color': ev.color }}
                      />
                      <div class="flex-1 min-w-0">
                        <div class="text-sm font-medium truncate">{ev.title}</div>
                        <div class="text-xs text-gray-500 dark:text-gray-400">
                          {ev.all_day ? 'All day' : formatTime(ev.start_time)}
                          {ev.end_time && !ev.all_day && ` - ${formatTime(ev.end_time)}`}
                        </div>
                        <Show when={ev.location}>
                          <div class="text-xs text-gray-400">{ev.location}</div>
                        </Show>
                        <Show when={ev.description}>
                          <div class="text-xs text-gray-400 mt-0.5 line-clamp-2">
                            {ev.description}
                          </div>
                        </Show>
                      </div>
                      <Show when={ev.source === 'local'}>
                        <button
                          class="opacity-0 group-hover:opacity-100 text-red-400 hover:text-red-600 text-xs transition-opacity"
                          onClick={() => deleteEvent(ev.id)}
                          title="Delete event"
                        >
                          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                          </svg>
                        </button>
                      </Show>
                    </div>
                  )}
                </For>
              </div>
            </Show>
          </div>

          {/* Add event form */}
          <Show when={showAddForm()}>
            <div class="card p-4">
              <h3 class="text-sm font-semibold mb-3">New Event</h3>
              <div class="space-y-3">
                <div>
                  <label class="block text-xs font-medium mb-1">Title</label>
                  <input
                    type="text"
                    class="input w-full text-sm"
                    value={formTitle()}
                    onInput={(e) => setFormTitle(e.currentTarget.value)}
                    placeholder="Event title"
                  />
                </div>

                <div>
                  <label class="block text-xs font-medium mb-1">Date</label>
                  <input
                    type="date"
                    class="input w-full text-sm"
                    value={formDate()}
                    onInput={(e) => setFormDate(e.currentTarget.value)}
                  />
                </div>

                <div class="flex items-center gap-2">
                  <input
                    type="checkbox"
                    id="cal-allday"
                    checked={formAllDay()}
                    onChange={(e) => setFormAllDay(e.currentTarget.checked)}
                    class="rounded"
                  />
                  <label for="cal-allday" class="text-xs">All day</label>
                </div>

                <Show when={!formAllDay()}>
                  <div class="flex gap-2">
                    <div class="flex-1">
                      <label class="block text-xs font-medium mb-1">Time</label>
                      <input
                        type="time"
                        class="input w-full text-sm"
                        value={formTime()}
                        onInput={(e) => setFormTime(e.currentTarget.value)}
                      />
                    </div>
                    <div class="w-20">
                      <label class="block text-xs font-medium mb-1">Min</label>
                      <input
                        type="number"
                        class="input w-full text-sm"
                        value={formDuration()}
                        onInput={(e) => setFormDuration(e.currentTarget.value)}
                        min="15"
                        step="15"
                      />
                    </div>
                  </div>
                </Show>

                <div>
                  <label class="block text-xs font-medium mb-1">Location</label>
                  <input
                    type="text"
                    class="input w-full text-sm"
                    value={formLocation()}
                    onInput={(e) => setFormLocation(e.currentTarget.value)}
                    placeholder="Optional"
                  />
                </div>

                <div>
                  <label class="block text-xs font-medium mb-1">Color</label>
                  <div class="flex gap-1.5">
                    <For each={['#0ea5e9', '#22c55e', '#ef4444', '#f59e0b', '#8b5cf6', '#ec4899', '#6b7280']}>
                      {(c) => (
                        <button
                          class="w-6 h-6 rounded-full border-2 transition-transform"
                          classList={{
                            'border-gray-900 dark:border-white scale-110': formColor() === c,
                            'border-transparent': formColor() !== c,
                          }}
                          style={{ 'background-color': c }}
                          onClick={() => setFormColor(c)}
                        />
                      )}
                    </For>
                  </div>
                </div>

                <div>
                  <label class="block text-xs font-medium mb-1">Description</label>
                  <textarea
                    class="input w-full text-sm"
                    rows={2}
                    value={formDescription()}
                    onInput={(e) => setFormDescription(e.currentTarget.value)}
                    placeholder="Optional"
                  />
                </div>

                <div class="flex gap-2">
                  <button class="btn btn-primary text-sm flex-1" onClick={addEvent}>
                    Save
                  </button>
                  <button class="btn btn-secondary text-sm" onClick={() => setShowAddForm(false)}>
                    Cancel
                  </button>
                </div>
              </div>
            </div>
          </Show>
        </div>
      </div>
    </div>
  );
};

export default Calendar;
