# Presentation Module — Sub-Plan 3: Frontend Components

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the SolidJS UI that makes the presentation module usable: creation studio, live agent sidebar, spatial canvas renderer, slide player, and slide editor.

**Architecture:** All state lives in a SolidJS store (createStore). Components are purely reactive — they read from the store and dispatch patches back through the Tauri API. The spatial canvas uses CSS transforms (translate + scale) not SVG, with frustum culling for performance.

**Tech Stack:** SolidJS, TypeScript, TailwindCSS, @tauri-apps/api, presentation-api.ts, deck-schema.ts, deck-patch.ts.

## Task 1: Deck Store (`ui/src/store/deck-store.ts`)

- [ ] Create file with `createDeckStore()` → `[store, { setDeck, applyPatch, undo, redo, canUndo, canRedo }]`.
- [ ] `applyPatch` calls `applyPatch` from `deck-patch.ts`, pushes previous deck to undo stack (max 50), clears redo stack.
- [ ] `undo`/`redo` navigate the history arrays; `canUndo`/`canRedo` return booleans.
- [ ] Run `cd /home/dk/Documents/git/minion/ui && pnpm typecheck` — fix errors.
- [ ] Commit: `feat(presentation): add deck store with undo/redo history`

```typescript
// ui/src/store/deck-store.ts
import { createStore } from "solid-js/store";
import type { Deck } from "../lib/deck-schema";
import type { DeckPatch } from "../lib/deck-patch";
import { applyPatch as applyDeckPatch } from "../lib/deck-patch";
const MAX_HISTORY = 50;
export interface DeckStoreState { deck: Deck | null }
export interface DeckStoreActions {
  setDeck: (deck: Deck | null) => void;
  applyPatch: (patch: DeckPatch) => void;
  undo: () => void;
  redo: () => void;
  canUndo: () => boolean;
  canRedo: () => boolean;
}
export function createDeckStore(): [DeckStoreState, DeckStoreActions] {
  const [store, setStore] = createStore<DeckStoreState>({ deck: null });
  let undoStack: Deck[] = [];
  let redoStack: Deck[] = [];
  const actions: DeckStoreActions = {
    setDeck(deck) { undoStack = []; redoStack = []; setStore("deck", deck); },
    applyPatch(patch) {
      const current = store.deck;
      if (!current) return;
      undoStack = [...undoStack.slice(-(MAX_HISTORY - 1)), current];
      redoStack = [];
      setStore("deck", applyDeckPatch(current, patch));
    },
    undo() {
      const prev = undoStack.at(-1);
      if (!prev) return;
      if (store.deck) redoStack = [...redoStack, store.deck];
      undoStack = undoStack.slice(0, -1);
      setStore("deck", prev);
    },
    redo() {
      const next = redoStack.at(-1);
      if (!next) return;
      if (store.deck) undoStack = [...undoStack, store.deck];
      redoStack = redoStack.slice(0, -1);
      setStore("deck", next);
    },
    canUndo: () => undoStack.length > 0,
    canRedo: () => redoStack.length > 0,
  };
  return [store, actions];
}
```

## Task 2: CreationStudio (`ui/src/pages/presentation/CreationStudio.tsx`)

- [ ] Four tabs (Text | Files | URL | Git). Files tab: `<input type="file" multiple accept=".pdf,.docx,.md,.xlsx,.png,.jpg,.jpeg">`, collect `(file as File & { path?: string }).path ?? file.name`. URL tab: SSRF disclaimer _"Only fetch URLs you own or trust. Minion fetches server-side."_
- [ ] Audience chips: Engineering | Executive | Investor | General. Tone chips: Authoritative | Conversational | Technical | Inspirational. Language `<select>`: en-US, fr-FR, de-DE, es-ES, ja-JP, zh-CN, ar-SA.
- [ ] Generate button disabled when tab has no input. On click: build `InputSource[]`, `startGeneration(inputs, config)`, `props.onGenerated(sessionId)`. Show inline error on throw. Props: `onGenerated: (sessionId: string) => void; onBack: () => void`.
- [ ] Run `cd /home/dk/Documents/git/minion/ui && pnpm typecheck` — fix errors.
- [ ] Commit: `feat(presentation): add CreationStudio multi-tab input form`

```typescript
// ui/src/pages/presentation/CreationStudio.tsx
import { createSignal, Show, Switch, Match, For } from "solid-js";
import { startGeneration, type InputSource } from "../../lib/presentation-api";
import type { GenerationConfig } from "../../lib/deck-schema";
type Tab = "text" | "files" | "url" | "git";
type Audience = "Engineering" | "Executive" | "Investor" | "General";
type Tone = "Authoritative" | "Conversational" | "Technical" | "Inspirational";
const AUDIENCES: Audience[] = ["Engineering", "Executive", "Investor", "General"];
const TONES: Tone[] = ["Authoritative", "Conversational", "Technical", "Inspirational"];
const LANGS = [
  { code: "en-US", label: "English (US)" }, { code: "fr-FR", label: "French" },
  { code: "de-DE", label: "German" },       { code: "es-ES", label: "Spanish" },
  { code: "ja-JP", label: "Japanese" },     { code: "zh-CN", label: "Chinese (Simplified)" },
  { code: "ar-SA", label: "Arabic" },
];
interface Props { onGenerated: (sessionId: string) => void; onBack: () => void }
export default function CreationStudio(props: Props) {
  const [tab, setTab] = createSignal<Tab>("text");
  const [text, setText] = createSignal("");
  const [paths, setPaths] = createSignal<string[]>([]);
  const [url, setUrl] = createSignal("");
  const [git, setGit] = createSignal("");
  const [audience, setAudience] = createSignal<Audience>("General");
  const [tone, setTone] = createSignal<Tone>("Conversational");
  const [lang, setLang] = createSignal("en-US");
  const [busy, setBusy] = createSignal(false);
  const [err, setErr] = createSignal<string | null>(null);
  const hasInput = () => {
    switch (tab()) {
      case "text": return text().trim().length > 0;
      case "files": return paths().length > 0;
      case "url": return url().trim().length > 0;
      case "git": return git().trim().length > 0;
    }
  };
  const buildInputs = (): InputSource[] => {
    switch (tab()) {
      case "text": return [{ kind: "text", content: text() }];
      case "files": return paths().map(p => ({ kind: "file_path", content: p }));
      case "url": return [{ kind: "url", content: url() }];
      case "git": return [{ kind: "git_url", content: git() }];
    }
  };
  const generate = async () => {
    if (!hasInput() || busy()) return;
    setErr(null); setBusy(true);
    try {
      const config: GenerationConfig = {
        audience: audience(), tone: tone(), language: lang(),
        presentation_context: "live_talk",
      };
      props.onGenerated(await startGeneration(buildInputs(), config));
    } catch (e) { setErr(String(e)); } finally { setBusy(false); }
  };
  const tabCls = (t: Tab) =>
    `px-4 py-2 text-sm font-medium rounded-t-lg transition-colors ${tab() === t
      ? "bg-[#1c1c24] text-white border-b-2 border-indigo-500"
      : "text-gray-400 hover:text-white"}`;
  const chip = (on: boolean) =>
    `px-3 py-1 rounded-full text-xs font-medium border cursor-pointer transition-colors ${on
      ? "bg-indigo-600 border-indigo-500 text-white"
      : "bg-[#1c1c24] border-[#2a2a36] text-gray-400 hover:border-indigo-500/60"}`;
  return (
    <div class="flex flex-col h-full bg-[#0f0f14] text-white p-8 max-w-3xl mx-auto">
      <div class="flex items-center gap-4 mb-8">
        <button onClick={props.onBack} class="text-gray-400 hover:text-white text-sm">&#8592; Back</button>
        <h1 class="text-2xl font-bold">New Presentation</h1>
      </div>
      <div class="flex gap-1 border-b border-[#2a2a36]">
        {(["text","files","url","git"] as Tab[]).map(t =>
          <button class={tabCls(t)} onClick={() => setTab(t)}>{t[0].toUpperCase()+t.slice(1)}</button>
        )}
      </div>
      <div class="bg-[#1c1c24] rounded-b-xl rounded-tr-xl p-4 mb-6 border border-[#2a2a36] border-t-0">
        <Switch>
          <Match when={tab() === "text"}>
            <textarea class="w-full bg-transparent text-sm text-gray-200 placeholder-gray-600 resize-none outline-none"
              rows={12} placeholder="Paste notes, outline, or raw text…"
              value={text()} onInput={e => setText(e.currentTarget.value)} />
          </Match>
          <Match when={tab() === "files"}>
            <input type="file" multiple accept=".pdf,.docx,.md,.xlsx,.png,.jpg,.jpeg"
              class="text-sm text-gray-300 file:mr-3 file:py-1.5 file:px-3 file:rounded-lg file:border-0 file:bg-indigo-600 file:text-white file:text-xs"
              onChange={e => setPaths(Array.from(e.currentTarget.files ?? [])
                .map(f => (f as File & { path?: string }).path ?? f.name))} />
            <Show when={paths().length > 0}>
              <ul class="mt-2 text-xs text-gray-400 list-disc list-inside space-y-0.5">
                <For each={paths()}>{p => <li class="truncate">{p}</li>}</For>
              </ul>
            </Show>
          </Match>
          <Match when={tab() === "url"}>
            <input type="url" class="w-full bg-[#0f0f14] border border-[#2a2a36] rounded-lg px-3 py-2 text-sm text-gray-200 outline-none focus:border-indigo-500"
              placeholder="https://example.com/report" value={url()} onInput={e => setUrl(e.currentTarget.value)} />
            <p class="mt-2 text-xs text-amber-500/80">&#9888; Only fetch URLs you own or trust. Minion fetches server-side.</p>
          </Match>
          <Match when={tab() === "git"}>
            <input type="text" class="w-full bg-[#0f0f14] border border-[#2a2a36] rounded-lg px-3 py-2 text-sm text-gray-200 outline-none focus:border-indigo-500"
              placeholder="https://github.com/org/repo" value={git()} onInput={e => setGit(e.currentTarget.value)} />
          </Match>
        </Switch>
      </div>
      <div class="mb-4">
        <p class="text-xs text-gray-500 uppercase tracking-wider mb-2">Audience</p>
        <div class="flex gap-2 flex-wrap">
          <For each={AUDIENCES}>{a => <button class={chip(audience()===a)} onClick={() => setAudience(a)}>{a}</button>}</For>
        </div>
      </div>
      <div class="mb-4">
        <p class="text-xs text-gray-500 uppercase tracking-wider mb-2">Tone</p>
        <div class="flex gap-2 flex-wrap">
          <For each={TONES}>{t => <button class={chip(tone()===t)} onClick={() => setTone(t)}>{t}</button>}</For>
        </div>
      </div>
      <div class="mb-6">
        <p class="text-xs text-gray-500 uppercase tracking-wider mb-2">Language</p>
        <select class="bg-[#1c1c24] border border-[#2a2a36] rounded-lg px-3 py-2 text-sm text-gray-200 outline-none"
          value={lang()} onChange={e => setLang(e.currentTarget.value)}>
          <For each={LANGS}>{l => <option value={l.code}>{l.label}</option>}</For>
        </select>
      </div>
      <Show when={err()}><p class="text-red-400 text-sm mb-3">{err()}</p></Show>
      <button onClick={generate} disabled={!hasInput() || busy()}
        class="px-6 py-3 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-40 rounded-xl text-sm font-semibold transition-colors self-start">
        {busy() ? "Generating…" : "Generate Presentation"}
      </button>
    </div>
  );
}
```

## Task 3: AgentSidebar (`ui/src/pages/presentation/AgentSidebar.tsx`)

- [ ] Props: `sessionId: string | null; onPatch: (p: DeckPatch) => void`. `createEffect` on `sessionId()`: call `listenToAgentEvents`, store `UnlistenFn`, call on `onCleanup`. Track agent status via `createStore<Record<string, AgentState>>({})`.
- [ ] On `slide_ready`: call `props.onPatch(event.patch)`. On `stream_complete`: `setComplete(true)`. Collapsible via toggle; collapsed by default.
- [ ] Run `cd /home/dk/Documents/git/minion/ui && pnpm typecheck` — fix errors.
- [ ] Commit: `feat(presentation): add AgentSidebar with live event streaming`

```typescript
// ui/src/pages/presentation/AgentSidebar.tsx
import { createSignal, createEffect, onCleanup, For, Show } from "solid-js";
import { createStore } from "solid-js/store";
import { listenToAgentEvents, type AgentEvent, type AgentName } from "../../lib/presentation-api";
import type { DeckPatch } from "../../lib/deck-patch";
import type { UnlistenFn } from "@tauri-apps/api/event";
interface AgentState { status: "waiting"|"running"|"done"|"error"; lastMessage: string }
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
    }).then(fn => { unlisten = fn; });
    onCleanup(() => { unlisten?.(); });
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
```

## Task 4: SpatialCanvas (`ui/src/pages/presentation/SpatialCanvas.tsx`)

- [ ] Props: `deck: Deck; selectedSlideId: string | null; onSelectSlide: (id: string) => void`. Pan `{x,y}` + zoom (0.05–20) signals. Wheel: `z*(1-deltaY*0.001)`. Mousedown/move/up drag. Cursor grab/grabbing.
- [ ] Frustum culling: only render slides whose screen rect overlaps viewport padded 1× each side. Transform layer: `transform-origin:0 0; transform:translate(${pan.x}px,${pan.y}px) scale(${zoom})`. Slides at `canvas_x,canvas_y`. Selected slide: indigo ring. Zoom % label bottom-right.
- [ ] Run `cd /home/dk/Documents/git/minion/ui && pnpm typecheck` — fix errors.
- [ ] Commit: `feat(presentation): add SpatialCanvas with frustum culling and pan/zoom`

```typescript
// ui/src/pages/presentation/SpatialCanvas.tsx
import { createSignal, For } from "solid-js";
import type { Deck, Slide } from "../../lib/deck-schema";
import { allSlides, colorToCss } from "../../lib/deck-schema";
interface Props { deck: Deck; selectedSlideId: string | null; onSelectSlide: (id: string) => void }
function slideBg(slide: Slide): string {
  const bg = slide.background;
  if (bg.kind === "solid") return colorToCss(bg.color);
  if (bg.kind === "gradient") return `linear-gradient(${bg.angle_deg}deg,${colorToCss(bg.from)},${colorToCss(bg.to)})`;
  return "#1c1c24";
}
function visible(s: Slide, pan: {x:number;y:number}, z: number, w: number, h: number) {
  const sx = s.canvas_x*z+pan.x, sy = s.canvas_y*z+pan.y;
  const sw = s.width*z, sh = s.height*z;
  return sx+sw>-w && sx<2*w && sy+sh>-h && sy<2*h;
}
export default function SpatialCanvas(props: Props) {
  const [pan, setPan] = createSignal({x:40,y:40});
  const [zoom, setZoom] = createSignal(1);
  const [drag, setDrag] = createSignal(false);
  let ref: HTMLDivElement | undefined;
  let last = {x:0,y:0};
  const vp = () => ({w: ref?.clientWidth??800, h: ref?.clientHeight??600});
  const onWheel = (e: WheelEvent) => {
    e.preventDefault();
    setZoom(z => Math.min(20, Math.max(0.05, z*(1-e.deltaY*0.001))));
  };
  const onDown = (e: MouseEvent) => { setDrag(true); last={x:e.clientX,y:e.clientY}; };
  const onMove = (e: MouseEvent) => {
    if (!drag()) return;
    const dx=e.clientX-last.x, dy=e.clientY-last.y; last={x:e.clientX,y:e.clientY};
    setPan(p=>({x:p.x+dx,y:p.y+dy}));
  };
  const onUp = () => setDrag(false);
  const slides = () => {
    const {w,h}=vp();
    return allSlides(props.deck).filter(s=>visible(s,pan(),zoom(),w,h));
  };
  return (
    <div ref={ref} class="relative overflow-hidden w-full h-full select-none"
      style={{cursor:drag()?"grabbing":"grab",background:"#090910"}}
      onWheel={onWheel} onMouseDown={onDown} onMouseMove={onMove}
      onMouseUp={onUp} onMouseLeave={onUp}>
      <div class="absolute inset-0 pointer-events-none"
        style={{"background-image":"radial-gradient(circle,#2a2a3a 1px,transparent 1px)",
          "background-size":`${32*zoom()}px ${32*zoom()}px`,
          "background-position":`${pan().x%(32*zoom())}px ${pan().y%(32*zoom())}px`}} />
      <div class="absolute" style={{"transform-origin":"0 0",
        transform:`translate(${pan().x}px,${pan().y}px) scale(${zoom()})`}}>
        <For each={slides()}>
          {(slide) => (
            <div class="absolute border"
              style={{left:`${slide.canvas_x}px`,top:`${slide.canvas_y}px`,
                width:`${slide.width}px`,height:`${slide.height}px`,
                background:slideBg(slide),
                "box-shadow":props.selectedSlideId===slide.id?"0 0 0 3px #6366f1":"0 2px 8px rgba(0,0,0,0.5)",
                "border-color":props.selectedSlideId===slide.id?"#6366f1":"rgba(255,255,255,0.06)"}}
              onClick={e=>{e.stopPropagation();props.onSelectSlide(slide.id);}}>
              <span class="absolute top-2 left-2 px-1.5 py-0.5 bg-black/40 text-white/60 text-[10px] rounded font-mono">
                {slide.layout}
              </span>
              <For each={slide.elements}>
                {(el) => (
                  <div class="absolute rounded-sm opacity-60"
                    style={{left:`${el.x}px`,top:`${el.y}px`,width:`${el.width}px`,height:`${el.height}px`,
                      background:el.content.kind==="text"?"rgba(255,255,255,0.15)":
                        el.content.kind==="image"?"rgba(99,102,241,0.3)":"rgba(255,255,255,0.08)"}} />
                )}
              </For>
            </div>
          )}
        </For>
      </div>
      <div class="absolute bottom-3 right-3 px-2 py-1 bg-black/50 text-gray-400 text-xs rounded font-mono pointer-events-none">
        {Math.round(zoom()*100)}%
      </div>
    </div>
  );
}
```

## Task 5: PresentationPlayer (`ui/src/pages/presentation/PresentationPlayer.tsx`)

- [ ] Props: `deck: Deck; onClose: () => void`. Fixed overlay (z-50). `idx` signal over `play_order`. ArrowRight/Space → advance; ArrowLeft → back; Escape → close. Register/remove handler via `onMount`/`onCleanup`.
- [ ] Headline = first text element by z_index; body = second. Slide counter bottom-right. `talking_points` in scrollable bottom bar.
- [ ] Run `cd /home/dk/Documents/git/minion/ui && pnpm typecheck` — fix errors.
- [ ] Commit: `feat(presentation): add PresentationPlayer full-screen playback`

```typescript
// ui/src/pages/presentation/PresentationPlayer.tsx
import { createSignal, createMemo, onMount, onCleanup, For, Show } from "solid-js";
import type { Deck, Slide } from "../../lib/deck-schema";
import { allSlides, colorToCss } from "../../lib/deck-schema";
interface Props { deck: Deck; onClose: () => void }
function bgStyle(s: Slide) {
  const bg = s.background;
  if (bg.kind==="solid") return colorToCss(bg.color);
  if (bg.kind==="gradient") return `linear-gradient(${bg.angle_deg}deg,${colorToCss(bg.from)},${colorToCss(bg.to)})`;
  return "#111";
}
export default function PresentationPlayer(props: Props) {
  const order = () => props.deck.play_order;
  const [idx, setIdx] = createSignal(0);
  const total = () => order().length;
  const advance = (d: number) => setIdx(i=>Math.max(0,Math.min(total()-1,i+d)));
  const slide = createMemo<Slide|undefined>(() => {
    const sid = order()[idx()];
    return sid ? allSlides(props.deck).find(s=>s.id===sid) : undefined;
  });
  const texts = createMemo(()=>
    [...(slide()?.elements??[])].filter(e=>e.content.kind==="text").sort((a,b)=>a.z_index-b.z_index)
  );
  const headline = createMemo(()=>{const e=texts()[0]; return e?.content.kind==="text"?e.content.markdown:"";});
  const body     = createMemo(()=>{const e=texts()[1]; return e?.content.kind==="text"?e.content.markdown:"";});
  const notes    = createMemo(()=>slide()?.speaker_notes.talking_points??[]);
  const onKey=(e:KeyboardEvent)=>{
    if(e.key==="ArrowRight"||e.key===" "){e.preventDefault();advance(1);}
    else if(e.key==="ArrowLeft"){e.preventDefault();advance(-1);}
    else if(e.key==="Escape") props.onClose();
  };
  onMount(()=>window.addEventListener("keydown",onKey));
  onCleanup(()=>window.removeEventListener("keydown",onKey));
  return (
    <div class="fixed inset-0 z-50 flex flex-col"
      style={{background:slide()?bgStyle(slide()!):"#000"}}>
      <button onClick={props.onClose}
        class="absolute top-4 right-4 text-white/50 hover:text-white text-2xl w-8 h-8 flex items-center justify-center">
        &#x2715;
      </button>
      <div class="flex-1 flex flex-col items-center justify-center px-16 text-center gap-6">
        <Show when={slide()} fallback={<p class="text-white/40 text-xl">No slides in play order</p>}>
          <h1 class="text-5xl font-bold text-white leading-tight max-w-5xl">{headline()}</h1>
          <Show when={body()}>
            <p class="text-xl text-white/70 max-w-3xl leading-relaxed">{body()}</p>
          </Show>
        </Show>
      </div>
      <Show when={notes().length>0}>
        <div class="border-t border-white/10 bg-black/40 px-8 py-3 flex gap-6 overflow-x-auto">
          <For each={notes()}>
            {p=><p class="text-sm text-white/50 whitespace-nowrap flex-shrink-0 max-w-xs truncate">&#x2022; {p}</p>}
          </For>
        </div>
      </Show>
      <div class="absolute bottom-4 right-6 flex items-center gap-4">
        <button onClick={()=>advance(-1)} disabled={idx()===0}
          class="text-white/40 hover:text-white disabled:opacity-20 text-xl">&#8592;</button>
        <span class="text-white/50 text-sm tabular-nums">{idx()+1} / {total()}</span>
        <button onClick={()=>advance(1)} disabled={idx()===total()-1}
          class="text-white/40 hover:text-white disabled:opacity-20 text-xl">&#8594;</button>
      </div>
    </div>
  );
}
```

## Task 6: DeckWorkspace + Wire `Presentation.tsx`

- [ ] Create `DeckWorkspace.tsx`. Props: `deckId: string; onBack: () => void; initialSessionId?: string`. Use `createDeckStore()`. On mount: `getDeck(deckId)` → `actions.setDeck`.
- [ ] Toolbar: Back, title, Undo/Redo (disabled when `!canUndo/Redo()`), Export stub, Present button. Body: `SpatialCanvas` (flex-1) + `AgentSidebar`. `onPatch`: `actions.applyPatch(p)` + `saveDeckPatch(deckId,[p])` (fire-and-forget, log errors). `PresentationPlayer` overlay when `playerOpen && store.deck`.
- [ ] Update `Presentation.tsx`: studio `<Match>` → `<CreationStudio>`; workspace `<Match>` → `<Show when={activeDeckId()||initialSessionId()}><DeckWorkspace .../></Show>`.
- [ ] Run `cd /home/dk/Documents/git/minion/ui && pnpm typecheck` — zero errors.
- [ ] Run `cd /home/dk/Documents/git/minion/ui && pnpm lint` — zero errors.
- [ ] Commit: `feat(presentation): wire DeckWorkspace and replace Presentation.tsx placeholders`

```typescript
// ui/src/pages/presentation/DeckWorkspace.tsx
import { createSignal, onMount, Show } from "solid-js";
import { getDeck, saveDeckPatch } from "../../lib/presentation-api";
import type { DeckPatch } from "../../lib/deck-patch";
import { createDeckStore } from "../../store/deck-store";
import SpatialCanvas from "./SpatialCanvas";
import AgentSidebar from "./AgentSidebar";
import PresentationPlayer from "./PresentationPlayer";
interface Props { deckId: string; onBack: () => void; initialSessionId?: string }
export default function DeckWorkspace(props: Props) {
  const [store, actions] = createDeckStore();
  const [selected, setSelected] = createSignal<string | null>(null);
  const [playerOpen, setPlayerOpen] = createSignal(false);
  const [loadErr, setLoadErr] = createSignal<string | null>(null);
  const [sessionId] = createSignal<string | null>(props.initialSessionId ?? null);
  onMount(async () => {
    if (!props.deckId) return;
    try { actions.setDeck(await getDeck(props.deckId)); }
    catch (e) { setLoadErr(String(e)); }
  });
  const handlePatch = (p: DeckPatch) => {
    actions.applyPatch(p);
    saveDeckPatch(props.deckId, [p]).catch(e => console.error("[DeckWorkspace]", e));
  };
  return (
    <div class="flex flex-col h-full w-full bg-[#090910] text-white">
      <div class="flex items-center gap-3 px-4 py-2 border-b border-[#2a2a36] bg-[#0f0f14] flex-shrink-0">
        <button onClick={props.onBack} class="text-gray-400 hover:text-white text-sm">&#8592; Back</button>
        <div class="w-px h-4 bg-[#2a2a36]" />
        <h1 class="text-sm font-medium flex-1 truncate">{store.deck?.meta.title ?? "Loading…"}</h1>
        <button onClick={actions.undo} disabled={!actions.canUndo()}
          class="px-2 py-1 text-xs text-gray-400 hover:text-white disabled:opacity-30" title="Undo">&#8617;</button>
        <button onClick={actions.redo} disabled={!actions.canRedo()}
          class="px-2 py-1 text-xs text-gray-400 hover:text-white disabled:opacity-30" title="Redo">&#8618;</button>
        <button onClick={() => console.log("export stub")}
          class="px-3 py-1.5 text-xs text-gray-400 border border-[#2a2a36] hover:border-gray-500 rounded-lg">Export</button>
        <button onClick={() => setPlayerOpen(true)} disabled={!store.deck}
          class="px-3 py-1.5 text-xs bg-indigo-600 hover:bg-indigo-500 disabled:opacity-40 rounded-lg font-medium">
          &#9654; Present
        </button>
      </div>
      <div class="flex flex-1 overflow-hidden relative">
        <Show when={loadErr()}>
          <div class="flex-1 flex items-center justify-center text-red-400 text-sm">{loadErr()}</div>
        </Show>
        <Show when={!loadErr() && !store.deck}>
          <div class="flex-1 flex items-center justify-center text-gray-500 text-sm">Loading deck…</div>
        </Show>
        <Show when={store.deck}>
          {(deck) => (
            <>
              <div class="flex-1 overflow-hidden">
                <SpatialCanvas deck={deck()} selectedSlideId={selected()} onSelectSlide={setSelected} />
              </div>
              <AgentSidebar sessionId={sessionId()} onPatch={handlePatch} />
            </>
          )}
        </Show>
      </div>
      <Show when={playerOpen() && store.deck}>
        <PresentationPlayer deck={store.deck!} onClose={() => setPlayerOpen(false)} />
      </Show>
    </div>
  );
}
```

```typescript
// ui/src/pages/Presentation.tsx (updated)
import { createSignal, Switch, Match, Show } from "solid-js";
import PresentationLibrary from "./presentation/PresentationLibrary";
import CreationStudio from "./presentation/CreationStudio";
import DeckWorkspace from "./presentation/DeckWorkspace";
type View = "library" | "studio" | "workspace";
export default function PresentationPage() {
  const [view, setView] = createSignal<View>("library");
  const [activeDeckId, setActiveDeckId] = createSignal<string | null>(null);
  const [initialSessionId, setInitialSessionId] = createSignal<string | undefined>(undefined);
  return (
    <div class="h-full w-full">
      <Switch>
        <Match when={view() === "library"}>
          <PresentationLibrary
            onOpenDeck={(id) => { setActiveDeckId(id); setInitialSessionId(undefined); setView("workspace"); }}
            onNewDeck={() => setView("studio")} />
        </Match>
        <Match when={view() === "studio"}>
          <CreationStudio
            onBack={() => setView("library")}
            onGenerated={(sid) => { setInitialSessionId(sid); setActiveDeckId(null); setView("workspace"); }} />
        </Match>
        <Match when={view() === "workspace"}>
          <Show when={activeDeckId() || initialSessionId()}
            fallback={
              <div class="flex items-center justify-center h-full bg-[#0f0f14]">
                <button onClick={() => setView("library")} class="px-4 py-2 bg-[#1c1c24] rounded-lg text-sm text-white">
                  &#8592; Back to Library
                </button>
              </div>
            }>
            <DeckWorkspace deckId={activeDeckId() ?? ""} onBack={() => setView("library")}
              initialSessionId={initialSessionId()} />
          </Show>
        </Match>
      </Switch>
    </div>
  );
}
```

---

## Verification Checklist

- [ ] `cd /home/dk/Documents/git/minion/ui && pnpm typecheck` — zero errors; `pnpm lint` — zero errors
- [ ] All 7 files present: `ui/src/store/deck-store.ts`, `CreationStudio.tsx`, `AgentSidebar.tsx`, `SpatialCanvas.tsx`, `PresentationPlayer.tsx`, `DeckWorkspace.tsx`, `Presentation.tsx` (updated)
- [ ] No `any` types; SolidJS primitives only (`createSignal`, `createStore`, `createMemo`, `createEffect`, `onMount`, `onCleanup`, `For`, `Show`, `Switch/Match`)
- [ ] `AgentSidebar` and `PresentationPlayer` clean up listeners on `onCleanup`
- [ ] `DeckWorkspace` saves patches fire-and-forget with error logging
