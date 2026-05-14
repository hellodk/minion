import { createSignal, createEffect, onCleanup, For, Show } from "solid-js";
import { createStore } from "solid-js/store";
import { listenToAgentEvents, type AgentEvent, type AgentName } from "../../lib/presentation-api";
import type { DeckPatch } from "../../lib/deck-patch";
import type { UnlistenFn } from "@tauri-apps/api/event";

interface AgentState { status: "waiting" | "running" | "done" | "error"; lastMessage: string }
interface Props { sessionId: string | null; onPatch: (p: DeckPatch) => void }

const LABELS: Record<AgentName, string> = {
  research: "Research", storyteller: "Storyteller", slide_planner: "Slide Planner",
  visual: "Visual", design_critic: "Design Critic",
};
const DOT: Record<AgentState["status"], string> = {
  waiting: "bg-gray-600", running: "bg-amber-400 animate-pulse",
  done: "bg-emerald-500", error: "bg-red-500",
};

export default function AgentSidebar(props: Props) {
  const [collapsed, setCollapsed] = createSignal(true);
  const [agents, setAgents] = createStore<Record<string, AgentState>>({});
  const [complete, setComplete] = createSignal(false);
  const [streamErr, setStreamErr] = createSignal<string | null>(null);

  createEffect(() => {
    const sid = props.sessionId;
    if (!sid) return;
    setComplete(false); setStreamErr(null);

    // Track whether cleanup ran before the promise resolved (race condition guard).
    let cancelled = false;
    let unlisten: UnlistenFn | null = null;

    listenToAgentEvents(sid, (ev: AgentEvent) => {
      if ("agent" in ev) {
        const n = ev.agent;
        switch (ev.kind) {
          case "started":    setAgents(n, { status: "running", lastMessage: "Starting…" }); break;
          case "progress":   setAgents(n, { status: "running", lastMessage: ev.data }); break;
          case "slide_ready":
            setAgents(n, "lastMessage", `Slide ${ev.slide_index + 1} ready`);
            props.onPatch(ev.patch); break;
          case "completed":  setAgents(n, { status: "done", lastMessage: "Done" }); break;
          case "error":      setAgents(n, { status: "error", lastMessage: ev.message }); break;
        }
      } else if (ev.kind === "stream_complete") {
        setComplete(true); setCollapsed(false);
      } else if (ev.kind === "stream_error") {
        setStreamErr(ev.message);
      }
    }).then(fn => {
      // If cleanup already ran before the promise resolved, unlisten immediately.
      if (cancelled) { fn(); } else { unlisten = fn; }
    });

    onCleanup(() => {
      cancelled = true;
      unlisten?.();
    });
  });

  const entries = () => Object.entries(agents) as [AgentName, AgentState][];

  return (
    <div class="relative flex h-full">
      <button onClick={() => setCollapsed(c => !c)}
        class="absolute top-4 -left-7 z-10 w-7 h-12 bg-[#1c1c24] border border-[#2a2a36] border-r-0 rounded-l-lg flex items-center justify-center text-gray-400 hover:text-white text-xs">
        {collapsed() ? "‹" : "›"}
      </button>
      <Show when={!collapsed()}>
        <div class="w-64 bg-[#13131a] border-l border-[#2a2a36] flex flex-col h-full overflow-y-auto">
          <div class="p-4 border-b border-[#2a2a36]">
            <p class="text-xs font-semibold text-gray-300 uppercase tracking-wider">Agent Activity</p>
            <Show when={complete()}>
              <span class="mt-1 inline-block px-2 py-0.5 bg-emerald-600/20 text-emerald-400 text-xs rounded-full border border-emerald-500/30">
                Generation complete
              </span>
            </Show>
            <Show when={streamErr()}><p class="mt-1 text-xs text-red-400">{streamErr()}</p></Show>
          </div>
          <div class="flex flex-col gap-2 p-3">
            <Show when={entries().length > 0}
              fallback={<p class="text-xs text-gray-600 text-center py-4">{props.sessionId ? "Waiting for agents…" : "No active session"}</p>}>
              <For each={entries()}>
                {([name, state]) => (
                  <div class="bg-[#1c1c24] rounded-lg p-3 border border-[#2a2a36]">
                    <div class="flex items-center gap-2 mb-1">
                      <span class={`w-2 h-2 rounded-full flex-shrink-0 ${DOT[state.status]}`} />
                      <span class="text-xs font-medium text-gray-200">{LABELS[name] ?? name}</span>
                    </div>
                    <p class="text-xs text-gray-500 truncate">{state.lastMessage}</p>
                  </div>
                )}
              </For>
            </Show>
          </div>
        </div>
      </Show>
    </div>
  );
}
