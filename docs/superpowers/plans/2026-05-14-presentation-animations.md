# Presentation — Plan D: Animation Rendering + Camera Transitions

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add CSS element entrance animations and slide transition effects to PresentationPlayer so decks feel cinematic rather than static.

**Architecture:** PresentationPlayer gets an animation system: per-slide element animation queue, click-to-reveal for OnClick triggers, CSS keyframe animations for AnimEffect variants, and CSS transition effects between slides. All implemented with pure CSS + SolidJS signals — no animation library needed.

**Tech Stack:** SolidJS, TypeScript, CSS animations (keyframes injected via `<style>`), deck-schema.ts.

---

## Files

- Create: `ui/src/lib/presentation-animations.ts` — keyframe CSS, `animationStyle()`, `transitionStyles()`, `injectKeyframes()`
- Modify: `ui/src/pages/presentation/PresentationPlayer.tsx` — element animations, click queue, transitions, typewriter
- Create: `ui/src/pages/presentation/AnimationPanel.tsx` — read-only animation info panel
- Modify: `ui/src/pages/presentation/DeckWorkspace.tsx` — wire AnimationPanel below canvas

---

## Task 1: CSS animation keyframes injection

- [ ] **Step 1: Create `ui/src/lib/presentation-animations.ts`**

```typescript
// ui/src/lib/presentation-animations.ts
import type { AnimPhase, TransitionKind, Direction } from "./deck-schema";

export const KEYFRAMES_CSS = `
@keyframes fade-in        { from{opacity:0} to{opacity:1} }
@keyframes fade-exit      { from{opacity:1} to{opacity:0} }
@keyframes slide-in-left  { from{opacity:0;transform:translateX(-40px)} to{opacity:1;transform:translateX(0)} }
@keyframes slide-in-right { from{opacity:0;transform:translateX(40px)}  to{opacity:1;transform:translateX(0)} }
@keyframes slide-in-up    { from{opacity:0;transform:translateY(40px)}  to{opacity:1;transform:translateY(0)} }
@keyframes slide-in-down  { from{opacity:0;transform:translateY(-40px)} to{opacity:1;transform:translateY(0)} }
@keyframes zoom-in-anim   { from{opacity:0;transform:scale(0.8)}   to{opacity:1;transform:scale(1)} }
@keyframes scale-up-anim  { from{opacity:0;transform:scale(0.6)}   to{opacity:1;transform:scale(1)} }
@keyframes spring-in-anim { from{opacity:0;transform:scale(0.75)}  to{opacity:1;transform:scale(1)} }
@keyframes blur-reveal-anim { from{opacity:0;filter:blur(8px)} to{opacity:1;filter:blur(0)} }
@keyframes zoom-exit  { from{opacity:1;transform:scale(1)}   to{opacity:0;transform:scale(1.15)} }
@keyframes zoom-enter { from{opacity:0;transform:scale(0.9)} to{opacity:1;transform:scale(1)} }
@keyframes slide-exit-left   { from{opacity:1;transform:translateX(0)}   to{opacity:0;transform:translateX(-60px)} }
@keyframes slide-exit-right  { from{opacity:1;transform:translateX(0)}   to{opacity:0;transform:translateX(60px)} }
@keyframes slide-enter-left  { from{opacity:0;transform:translateX(60px)}  to{opacity:1;transform:translateX(0)} }
@keyframes slide-enter-right { from{opacity:0;transform:translateX(-60px)} to{opacity:1;transform:translateX(0)} }
`;

const STYLE_TAG_ID = "minion-anim-keyframes";

export function injectKeyframes(): void {
  if (document.getElementById(STYLE_TAG_ID)) return;
  const style = document.createElement("style");
  style.id = STYLE_TAG_ID;
  style.textContent = KEYFRAMES_CSS;
  document.head.appendChild(style);
}

/** Returns a CSS `animation` shorthand string for the given AnimPhase. */
export function animationStyle(phase: AnimPhase | undefined): string {
  if (!phase) return "";
  const { delay_ms: d, duration_ms: dur, effect } = phase;
  const std    = "cubic-bezier(0.4,0,0.2,1)";
  const spring = "cubic-bezier(0.34,1.56,0.64,1)";
  switch (effect.kind) {
    case "fade":        return `fade-in ${dur}ms ${std} ${d}ms both`;
    case "slide_in": {
      const m: Record<string,string> = { left:"slide-in-left", right:"slide-in-right", up:"slide-in-up", down:"slide-in-down" };
      return `${m[effect.direction] ?? "slide-in-left"} ${dur}ms ${std} ${d}ms both`;
    }
    case "zoom_in":     return `zoom-in-anim ${dur}ms ${std} ${d}ms both`;
    case "scale_up":    return `scale-up-anim ${dur}ms ${std} ${d}ms both`;
    case "blur_reveal": return `blur-reveal-anim ${dur}ms ${std} ${d}ms both`;
    case "spring":      return `spring-in-anim ${dur}ms ${spring} ${d}ms both`;
    default:            return `fade-in ${dur}ms ${std} ${d}ms both`;
  }
}

export interface TransitionStyles { exiting: string; entering: string }

/** CSS animation shorthands for outgoing and incoming slide divs. */
export function transitionStyles(
  kind: TransitionKind, direction: Direction | undefined, duration_ms: number
): TransitionStyles {
  const dur  = duration_ms > 0 ? duration_ms : 300;
  const ease = "cubic-bezier(0.4,0,0.2,1)";
  switch (kind) {
    case "fade":
      return { exiting: `fade-exit ${dur}ms ${ease} both`, entering: `fade-in ${dur}ms ${ease} both` };
    case "push": {
      const ex = direction === "right" ? "slide-exit-right" : "slide-exit-left";
      const en = direction === "right" ? "slide-enter-right" : "slide-enter-left";
      return { exiting: `${ex} ${dur}ms ${ease} both`, entering: `${en} ${dur}ms ${ease} both` };
    }
    case "fly": {
      const en = direction === "right" ? "slide-enter-right" : "slide-enter-left";
      return { exiting: `fade-exit ${dur}ms ${ease} both`, entering: `${en} ${dur}ms ${ease} both` };
    }
    default: // zoom / morph / rotate3d / portal_zoom
      return { exiting: `zoom-exit ${dur}ms ${ease} both`, entering: `zoom-enter ${dur}ms ${ease} both` };
  }
}
```

- [ ] **Step 2: Typecheck + commit**
```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | grep "error TS" | head -10
cd /home/dk/Documents/git/minion && git add ui/src/lib/presentation-animations.ts
git commit -m "feat(presentation): add CSS keyframe + transition animation helpers"
```

---

## Task 2: Animated element rendering in PresentationPlayer

Replace the static headline/body render with a per-element animated render. Entrance-triggered elements visible on mount; `on_click` elements queue one-at-a-time per advance press; `auto_after_ms` elements use `setTimeout`.

- [ ] **Step 1: Replace `ui/src/pages/presentation/PresentationPlayer.tsx`**

```tsx
// ui/src/pages/presentation/PresentationPlayer.tsx
import { createSignal, createMemo, createEffect, onMount, onCleanup, For, Show, Index } from "solid-js";
import type { Deck, Slide, Element as DeckElement } from "../../lib/deck-schema";
import { allSlides, colorToCss } from "../../lib/deck-schema";
import { animationStyle, injectKeyframes } from "../../lib/presentation-animations";

interface Props { deck: Deck; onClose: () => void }

function bgStyle(s: Slide): string {
  const bg = s.background;
  if (bg.kind === "solid")    return colorToCss(bg.color);
  if (bg.kind === "gradient") return `linear-gradient(${bg.angle_deg}deg,${colorToCss(bg.from)},${colorToCss(bg.to)})`;
  return "#111";
}
function isAutoEnter(el: DeckElement): boolean {
  const k = el.animation.trigger.kind;
  return k === "on_slide_enter" || k === "after_element" || k === "with_element";
}

export default function PresentationPlayer(props: Props) {
  injectKeyframes();
  const order = () => props.deck.play_order;
  const total = () => order().length;
  const [idx, setIdx] = createSignal(0);
  const slide = createMemo<Slide | undefined>(() => {
    const sid = order()[idx()]; return sid ? allSlides(props.deck).find(s => s.id === sid) : undefined;
  });
  const sortedEls = createMemo<DeckElement[]>(() =>
    [...(slide()?.elements ?? [])].sort((a, b) => a.z_index - b.z_index));
  const [clickQueue, setClickQueue] = createSignal<number[]>([]);
  const [visibleIds, setVisibleIds] = createSignal<Set<string>>(new Set());

  createEffect(() => {
    const els = sortedEls(); const visible = new Set<string>(); const queue: number[] = [];
    const timers: ReturnType<typeof setTimeout>[] = [];
    els.forEach((el, i) => {
      const t = el.animation.trigger;
      if (!el.animation.entrance || isAutoEnter(el)) { visible.add(el.id); }
      else if (t.kind === "on_click") { queue.push(i); }
      else if (t.kind === "auto_after_ms") {
        timers.push(setTimeout(() =>
          setVisibleIds(p => { const s = new Set(p); s.add(el.id); return s; }), t.ms));
      } else { visible.add(el.id); }
    });
    setVisibleIds(visible); setClickQueue(queue);
    onCleanup(() => timers.forEach(clearTimeout));
  });

  function advance(d: number) {
    if (d > 0 && clickQueue().length > 0) {
      const [next, ...rest] = clickQueue(); const el = sortedEls()[next];
      if (el) { setVisibleIds(p => { const s = new Set(p); s.add(el.id); return s; }); setClickQueue(rest); return; }
    }
    setIdx(i => Math.max(0, Math.min(total() - 1, i + d)));
  }

  const notes = createMemo(() => slide()?.speaker_notes.talking_points ?? []);
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "ArrowRight" || e.key === " ") { e.preventDefault(); advance(1); }
    else if (e.key === "ArrowLeft") { e.preventDefault(); advance(-1); }
    else if (e.key === "Escape") props.onClose();
  };
  onMount(() => window.addEventListener("keydown", onKey));
  onCleanup(() => window.removeEventListener("keydown", onKey));

  return (
    <div class="fixed inset-0 z-50 flex flex-col" style={{ background: slide() ? bgStyle(slide()!) : "#000" }}>
      <button onClick={props.onClose} class="absolute top-4 right-4 text-white/50 hover:text-white text-2xl w-8 h-8 flex items-center justify-center z-10">&#x2715;</button>
      <div class="flex-1 relative overflow-hidden">
        <Show when={slide()} fallback={<p class="absolute inset-0 flex items-center justify-center text-white/40 text-xl">No slides in play order</p>}>
          <Index each={sortedEls()}>
            {(el) => (
              <div class="absolute" style={{
                left:`${el().x}px`, top:`${el().y}px`, width:`${el().width}px`, height:`${el().height}px`,
                "z-index":el().z_index, opacity:visibleIds().has(el().id) ? el().style.opacity : 0,
                animation:visibleIds().has(el().id) ? animationStyle(el().animation.entrance) : "none",
                "border-radius":`${el().style.border_radius}px`, "box-shadow":el().style.box_shadow ?? undefined,
              }}><ElementRenderer el={el()} /></div>
            )}
          </Index>
        </Show>
      </div>
      <Show when={notes().length > 0}>
        <div class="border-t border-white/10 bg-black/40 px-8 py-3 flex gap-6 overflow-x-auto flex-shrink-0">
          <For each={notes()}>{p => <p class="text-sm text-white/50 whitespace-nowrap flex-shrink-0 max-w-xs truncate">• {p}</p>}</For>
        </div>
      </Show>
      <div class="absolute bottom-4 right-6 flex items-center gap-4">
        <button onClick={() => advance(-1)} disabled={idx() === 0} class="text-white/40 hover:text-white disabled:opacity-20 text-xl">&#8592;</button>
        <span class="text-white/50 text-sm tabular-nums">{idx() + 1} / {total()}</span>
        <button onClick={() => advance(1)} disabled={idx() === total() - 1 && clickQueue().length === 0} class="text-white/40 hover:text-white disabled:opacity-20 text-xl">&#8594;</button>
      </div>
    </div>
  );
}

function ElementRenderer(props: { el: DeckElement }) {
  const c = () => props.el.content;
  return (
    <Show when={c().kind === "text"}
      fallback={<div class="w-full h-full flex items-center justify-center text-white/20 text-xs border border-white/10 rounded">{c().kind}</div>}>
      <div class="w-full h-full flex items-start overflow-hidden text-white" style={{ "font-size":"clamp(14px,2vw,28px)" }}>
        {c().kind === "text" ? (c() as {kind:"text";markdown:string}).markdown : ""}
      </div>
    </Show>
  );
}
```

- [ ] **Step 2: Typecheck + commit**
```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | grep "error TS" | head -10
cd /home/dk/Documents/git/minion && git add ui/src/pages/presentation/PresentationPlayer.tsx
git commit -m "feat(presentation): animate elements on entrance with click-to-reveal queue"
```

---

## Task 3: Slide transition effects

Two-layer crossfade: outgoing slide plays its exit animation, incoming plays its entrance animation simultaneously.

- [ ] **Step 1: Patch `ui/src/pages/presentation/PresentationPlayer.tsx` with transition state**

Update the import to include `transitionStyles`:

```typescript
import { animationStyle, transitionStyles, injectKeyframes } from "../../lib/presentation-animations";
```

After `const [idx, setIdx] = createSignal(0);` add:

```typescript
const [displayIdx, setDisplayIdx] = createSignal(0);
const [prevIdx, setPrevIdx] = createSignal<number | null>(null);
const [transitioning, setTransitioning] = createSignal(false);
function slideAt(i: number): Slide | undefined {
  const sid = order()[i]; return sid ? allSlides(props.deck).find(s => s.id === sid) : undefined;
}
const prevSlide = createMemo(() => prevIdx() !== null ? slideAt(prevIdx()!) : undefined);
const txStyles = createMemo(() => {
  const s = slideAt(prevIdx() ?? displayIdx());
  if (!s) return { exiting: "", entering: "" };
  return transitionStyles(s.transition.kind, s.transition.direction, s.transition.duration_ms);
});
```

Replace `const slide = createMemo(...)` with `const slide = createMemo(() => slideAt(displayIdx()));`.

Replace `advance`:

```typescript
function advance(d: number) {
  if (d > 0 && clickQueue().length > 0) {
    const [next, ...rest] = clickQueue(); const el = sortedEls()[next];
    if (el) { setVisibleIds(p => { const s = new Set(p); s.add(el.id); return s; }); setClickQueue(rest); return; }
  }
  const next = Math.max(0, Math.min(total() - 1, idx() + d));
  if (next === idx()) return;
  const cur = displayIdx(); setPrevIdx(cur); setTransitioning(true); setIdx(next); setDisplayIdx(next);
  const t = setTimeout(() => { setPrevIdx(null); setTransitioning(false); }, (slideAt(cur)?.transition.duration_ms ?? 300) + 16);
  onCleanup(() => clearTimeout(t));
}
```

Replace the `<div class="flex-1 relative overflow-hidden">` body with two absolutely-positioned layers:

```tsx
<div class="flex-1 relative overflow-hidden">
  {/* Outgoing layer — snapshot of previous slide, plays exit animation */}
  <Show when={transitioning() && prevSlide()}>
    <div class="absolute inset-0 pointer-events-none"
      style={{ background: bgStyle(prevSlide()!), animation: txStyles().exiting, "z-index": 1 }}>
      <Index each={[...(prevSlide()!.elements)].sort((a,b) => a.z_index - b.z_index)}>
        {(el) => (
          <div class="absolute" style={{ left:`${el().x}px`, top:`${el().y}px`,
            width:`${el().width}px`, height:`${el().height}px`,
            "z-index":el().z_index, opacity:el().style.opacity,
            "border-radius":`${el().style.border_radius}px` }}>
            <ElementRenderer el={el()} />
          </div>
        )}
      </Index>
    </div>
  </Show>
  {/* Incoming layer — active slide, plays entrance animation */}
  <Show when={slide()}>
    <div class="absolute inset-0"
      style={{ animation: transitioning() ? txStyles().entering : undefined, "z-index": 2 }}>
      <Index each={sortedEls()}>
        {(el) => (
          <div class="absolute" style={{ left:`${el().x}px`, top:`${el().y}px`,
            width:`${el().width}px`, height:`${el().height}px`, "z-index":el().z_index,
            opacity: visibleIds().has(el().id) ? el().style.opacity : 0,
            animation: visibleIds().has(el().id) ? animationStyle(el().animation.entrance) : "none",
            "border-radius":`${el().style.border_radius}px`, "box-shadow":el().style.box_shadow ?? undefined }}>
            <ElementRenderer el={el()} />
          </div>
        )}
      </Index>
    </div>
  </Show>
</div>
```

- [ ] **Step 2: Typecheck + commit**
```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | grep "error TS" | head -10
cd /home/dk/Documents/git/minion && git add ui/src/pages/presentation/PresentationPlayer.tsx
git commit -m "feat(presentation): add two-layer CSS slide transition effects"
```

---

## Task 4: TypewriterReveal + AnimationPanel stub

### 4a — TypewriterText component

- [ ] **Step 1: Add `TypewriterText` and update `ElementRenderer` in PresentationPlayer.tsx**

Place `TypewriterText` above `ElementRenderer`. Replace `ElementRenderer` entirely.

```tsx
function TypewriterText(props: { text: string; duration_ms: number; delay_ms: number }) {
  const [displayed, setDisplayed] = createSignal("");
  onMount(() => {
    const chars = [...props.text]; if (!chars.length) return;
    const tickMs = Math.max(16, props.duration_ms / chars.length); let i = 0;
    const start = setTimeout(() => {
      const iv = setInterval(() => { i++; setDisplayed(chars.slice(0, i).join("")); if (i >= chars.length) clearInterval(iv); }, tickMs);
      onCleanup(() => clearInterval(iv));
    }, props.delay_ms);
    onCleanup(() => clearTimeout(start));
  });
  return <span>{displayed()}</span>;
}

function ElementRenderer(props: { el: DeckElement }) {
  const c = () => props.el.content;
  const isTw = () => c().kind === "text" && props.el.animation.entrance?.effect.kind === "typewriter_reveal";
  return (
    <Show when={c().kind === "text"}
      fallback={<div class="w-full h-full flex items-center justify-center text-white/20 text-xs border border-white/10 rounded">{c().kind}</div>}>
      <div class="w-full h-full flex items-start overflow-hidden text-white" style={{ "font-size":"clamp(14px,2vw,28px)" }}>
        <Show when={isTw()} fallback={(c() as {kind:"text";markdown:string}).markdown}>
          <TypewriterText text={(c() as {kind:"text";markdown:string}).markdown}
            duration_ms={props.el.animation.entrance!.duration_ms} delay_ms={props.el.animation.entrance!.delay_ms} />
        </Show>
      </div>
    </Show>
  );
}
```

### 4b — AnimationPanel

- [ ] **Step 2: Create `ui/src/pages/presentation/AnimationPanel.tsx`**

```tsx
// ui/src/pages/presentation/AnimationPanel.tsx
import { Show } from "solid-js";
import type { Element as DeckElement } from "../../lib/deck-schema";

interface Props { element: DeckElement | null }

const EL: Record<string,string> = { fade:"Fade", slide_in:"Slide In", zoom_in:"Zoom In", zoom_out:"Zoom Out",
  spring:"Spring", particle_burst:"Particle Burst", typewriter_reveal:"Typewriter Reveal",
  blur_reveal:"Blur Reveal", scale_up:"Scale Up", glow:"Glow", shake:"Shake", pulse:"Pulse", motion_path:"Motion Path" };
const TL: Record<string,string> = { on_slide_enter:"On Slide Enter", on_click:"On Click",
  after_element:"After Element", with_element:"With Element", auto_after_ms:"Auto After" };

const Row = (p: {label:string;value:string}) => (
  <div class="flex justify-between gap-2"><span class="text-gray-500">{p.label}</span><span class="font-mono text-gray-200">{p.value}</span></div>
);

export default function AnimationPanel(props: Props) {
  return (
    <div class="border-t border-[#2a2a36] bg-[#0c0c12] px-4 py-3 flex-shrink-0">
      <p class="text-[10px] font-semibold text-gray-500 uppercase tracking-wider mb-2">Animation</p>
      <Show when={props.element} fallback={<p class="text-xs text-gray-600 italic">Select an element to see its animation settings.</p>}>
        {(el) => {
          const phase = () => el().animation.entrance;
          return (
            <div class="flex flex-col gap-1.5 text-xs text-gray-300">
              <Show when={phase()} fallback={<p class="text-gray-500">No entrance animation.</p>}>
                {(p) => (<><Row label="Effect" value={EL[p().effect.kind] ?? p().effect.kind} />
                  <Row label="Delay" value={`${p().delay_ms} ms`} /><Row label="Duration" value={`${p().duration_ms} ms`} /></>)}
              </Show>
              <Row label="Trigger" value={TL[el().animation.trigger.kind] ?? el().animation.trigger.kind} />
            </div>
          );
        }}
      </Show>
    </div>
  );
}
```

### 4c — Wire into DeckWorkspace

- [ ] **Step 3: Patch DeckWorkspace.tsx** — add imports, signal, and wrap the canvas:

```typescript
// Add near top imports:
import type { Element as DeckElement } from "../../lib/deck-schema";
import AnimationPanel from "./AnimationPanel";
// Add signal inside component:
const [selectedElement, setSelectedElement] = createSignal<DeckElement | null>(null);
```

Replace the current `<div class="flex-1 overflow-hidden">` canvas wrapper:

```tsx
<div class="flex flex-col flex-1 overflow-hidden">
  <div class="flex-1 overflow-hidden">
    <SpatialCanvas deck={deck()} selectedSlideId={selected()} onSelectSlide={setSelected} />
  </div>
  <AnimationPanel element={selectedElement()} />
</div>
```

- [ ] **Step 4: Typecheck, lint, commit**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck 2>&1 | grep "error TS" | head -10
cd /home/dk/Documents/git/minion/ui && pnpm lint 2>&1 | grep " error " | head -10
cd /home/dk/Documents/git/minion && git add ui/src/pages/presentation/PresentationPlayer.tsx \
  ui/src/pages/presentation/AnimationPanel.tsx ui/src/pages/presentation/DeckWorkspace.tsx
git commit -m "feat(presentation): typewriter reveal + AnimationPanel stub wired into DeckWorkspace"
```
