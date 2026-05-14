import {
  createSignal, createMemo, createEffect, onMount, onCleanup, For, Show, Index
} from "solid-js";
import type { Deck, Slide, Element as DeckElement } from "../../lib/deck-schema";
import { allSlides, colorToCss } from "../../lib/deck-schema";
import { animationStyle, transitionStyles, injectKeyframes } from "../../lib/presentation-animations";

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

function TypewriterText(props: { text: string; duration_ms: number; delay_ms: number }) {
  const [displayed, setDisplayed] = createSignal("");
  onMount(() => {
    const chars = [...props.text]; if (!chars.length) return;
    const tickMs = Math.max(16, props.duration_ms / chars.length); let i = 0;
    const start = setTimeout(() => {
      const iv = setInterval(() => {
        i++;
        setDisplayed(chars.slice(0, i).join(""));
        if (i >= chars.length) clearInterval(iv);
      }, tickMs);
      onCleanup(() => clearInterval(iv));
    }, props.delay_ms);
    onCleanup(() => clearTimeout(start));
  });
  return <span>{displayed()}</span>;
}

function ElementRenderer(props: { el: DeckElement }) {
  const c = () => props.el.content;
  const isTw = () =>
    c().kind === "text" && props.el.animation.entrance?.effect.kind === "typewriter_reveal";
  return (
    <Show
      when={c().kind === "text"}
      fallback={
        <div class="w-full h-full flex items-center justify-center text-white/20 text-xs border border-white/10 rounded">
          {c().kind}
        </div>
      }
    >
      <div
        class="w-full h-full flex items-start overflow-hidden text-white"
        style={{ "font-size": "clamp(14px,2vw,28px)" }}
      >
        <Show
          when={isTw()}
          fallback={(c() as { kind: "text"; markdown: string }).markdown}
        >
          <TypewriterText
            text={(c() as { kind: "text"; markdown: string }).markdown}
            duration_ms={props.el.animation.entrance!.duration_ms}
            delay_ms={props.el.animation.entrance!.delay_ms}
          />
        </Show>
      </div>
    </Show>
  );
}

export default function PresentationPlayer(props: Props) {
  injectKeyframes();
  const order = () => props.deck.play_order;
  const total = () => order().length;
  const [idx, setIdx] = createSignal(0);
  const [displayIdx, setDisplayIdx] = createSignal(0);
  const [prevIdx, setPrevIdx] = createSignal<number | null>(null);
  const [transitioning, setTransitioning] = createSignal(false);
  // Timer handle for the transition clearance — managed manually so it can be
  // cancelled on rapid slide changes without calling onCleanup outside reactive scope.
  let transitionTimer: ReturnType<typeof setTimeout> | undefined;

  function slideAt(i: number): Slide | undefined {
    const sid = order()[i];
    return sid ? allSlides(props.deck).find(s => s.id === sid) : undefined;
  }

  const slide     = createMemo<Slide | undefined>(() => slideAt(displayIdx()));
  const prevSlide = createMemo<Slide | undefined>(() =>
    prevIdx() !== null ? slideAt(prevIdx()!) : undefined
  );
  const txStyles = createMemo(() => {
    const s = slideAt(prevIdx() ?? displayIdx());
    if (!s) return { exiting: "", entering: "" };
    return transitionStyles(s.transition.kind, s.transition.direction, s.transition.duration_ms);
  });

  const sortedEls = createMemo<DeckElement[]>(() =>
    [...(slide()?.elements ?? [])].sort((a, b) => a.z_index - b.z_index)
  );
  const [clickQueue, setClickQueue] = createSignal<number[]>([]);
  const [visibleIds, setVisibleIds] = createSignal<Set<string>>(new Set());

  createEffect(() => {
    const els = sortedEls();
    const visible = new Set<string>();
    const queue: number[] = [];
    const timers: ReturnType<typeof setTimeout>[] = [];
    els.forEach((el, i) => {
      const t = el.animation.trigger;
      if (!el.animation.entrance || isAutoEnter(el)) {
        visible.add(el.id);
      } else if (t.kind === "on_click") {
        queue.push(i);
      } else if (t.kind === "auto_after_ms") {
        timers.push(
          setTimeout(
            () => setVisibleIds(p => { const s = new Set(p); s.add(el.id); return s; }),
            t.ms
          )
        );
      } else {
        visible.add(el.id);
      }
    });
    setVisibleIds(visible);
    setClickQueue(queue);
    onCleanup(() => timers.forEach(clearTimeout));
  });

  function advance(d: number) {
    if (d > 0 && clickQueue().length > 0) {
      const [next, ...rest] = clickQueue();
      const el = sortedEls()[next];
      if (el) {
        setVisibleIds(p => { const s = new Set(p); s.add(el.id); return s; });
        setClickQueue(rest);
        return;
      }
    }
    const next = Math.max(0, Math.min(total() - 1, idx() + d));
    if (next === idx()) return;
    const cur = displayIdx();
    setPrevIdx(cur);
    setTransitioning(true);
    setIdx(next);
    setDisplayIdx(next);
    const dur = slideAt(cur)?.transition.duration_ms ?? 300;
    // Cancel any in-flight transition before starting a new one.
    clearTimeout(transitionTimer);
    transitionTimer = setTimeout(() => {
      setPrevIdx(null);
      setTransitioning(false);
    }, dur + 16);
  }

  const notes = createMemo(() => slide()?.speaker_notes.talking_points ?? []);

  const onKey = (e: KeyboardEvent) => {
    if (e.key === "ArrowRight" || e.key === " ") { e.preventDefault(); advance(1); }
    else if (e.key === "ArrowLeft") { e.preventDefault(); advance(-1); }
    else if (e.key === "Escape") props.onClose();
  };
  onMount(() => window.addEventListener("keydown", onKey));
  onCleanup(() => {
    window.removeEventListener("keydown", onKey);
    clearTimeout(transitionTimer);
  });

  return (
    <div
      class="fixed inset-0 z-50 flex flex-col"
      style={{ background: slide() ? bgStyle(slide()!) : "#000" }}
    >
      <button
        onClick={props.onClose}
        class="absolute top-4 right-4 text-white/50 hover:text-white text-2xl w-8 h-8 flex items-center justify-center z-10"
      >
        &#x2715;
      </button>

      {/* Scale canvas: slides are in 1920×1080 logical units. We render them inside a
          container sized to maintain 16:9 and then scale the inner 1920×1080 space down
          to fit via CSS transform, so elements always appear at the right proportional
          position regardless of screen size. */}
      <div class="flex-1 relative overflow-hidden flex items-center justify-center">
        <Show
          when={slide()}
          fallback={
            <p class="text-white/40 text-xl">No slides in play order</p>
          }
        >
          {/* Viewport box: 16:9, fills available area */}
          <div class="relative w-full h-full"
            style={{ "aspect-ratio": "16/9", "max-width": "177.78vh", "max-height": "56.25vw" }}>
            {/* Coordinate-space scaler: maps 1920×1080 → 100% × 100% */}
            <div class="absolute inset-0" style={{ "overflow": "hidden" }}>
              <div class="absolute top-0 left-0 origin-top-left"
                style={{
                  width: `${slide()!.width || 1920}px`,
                  height: `${slide()!.height || 1080}px`,
                  transform: `scale(calc(min(100vw, 177.78vh) / ${slide()!.width || 1920}))`,
                }}>
                {/* Outgoing layer */}
                <Show when={transitioning() && prevSlide()}>
                  <div
                    class="absolute inset-0 pointer-events-none"
                    style={{
                      background: bgStyle(prevSlide()!),
                      animation: txStyles().exiting,
                      "z-index": 1,
                    }}
                  >
                    <Index each={[...(prevSlide()!.elements)].sort((a, b) => a.z_index - b.z_index)}>
                      {(el) => (
                        <div
                          class="absolute"
                          style={{
                            left: `${el().x}px`,
                            top: `${el().y}px`,
                            width: `${el().width}px`,
                            height: `${el().height}px`,
                            "z-index": el().z_index,
                            opacity: el().style.opacity,
                            "border-radius": `${el().style.border_radius}px`,
                          }}
                        >
                          <ElementRenderer el={el()} />
                        </div>
                      )}
                    </Index>
                  </div>
                </Show>

                {/* Incoming layer */}
                <div
                  class="absolute inset-0"
                  style={{
                    animation: transitioning() ? txStyles().entering : undefined,
                    "z-index": 2,
                  }}
                >
                  <Index each={sortedEls()}>
                    {(el) => (
                      <div
                        class="absolute"
                        style={{
                          left: `${el().x}px`,
                          top: `${el().y}px`,
                          width: `${el().width}px`,
                          height: `${el().height}px`,
                          "z-index": el().z_index,
                          opacity: visibleIds().has(el().id) ? el().style.opacity : 0,
                          animation: visibleIds().has(el().id)
                            ? animationStyle(el().animation.entrance)
                            : "none",
                          "border-radius": `${el().style.border_radius}px`,
                          "box-shadow": el().style.box_shadow ?? undefined,
                        }}
                      >
                        <ElementRenderer el={el()} />
                      </div>
                    )}
                  </Index>
                </div>
              </div>
            </div>
          </div>
        </Show>
      </div>

      <Show when={notes().length > 0}>
        <div class="border-t border-white/10 bg-black/40 px-8 py-3 flex gap-6 overflow-x-auto flex-shrink-0">
          <For each={notes()}>
            {(p) => (
              <p class="text-sm text-white/50 whitespace-nowrap flex-shrink-0 max-w-xs truncate">
                • {p}
              </p>
            )}
          </For>
        </div>
      </Show>

      <div class="absolute bottom-4 right-6 flex items-center gap-4">
        <button
          onClick={() => advance(-1)}
          disabled={idx() === 0}
          class="text-white/40 hover:text-white disabled:opacity-20 text-xl"
        >
          &#8592;
        </button>
        <span class="text-white/50 text-sm tabular-nums">{idx() + 1} / {total()}</span>
        <button
          onClick={() => advance(1)}
          disabled={idx() === total() - 1 && clickQueue().length === 0}
          class="text-white/40 hover:text-white disabled:opacity-20 text-xl"
        >
          &#8594;
        </button>
      </div>
    </div>
  );
}
