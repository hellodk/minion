import { Show } from "solid-js";
import type { Element as DeckElement } from "../../lib/deck-schema";

interface Props { element: DeckElement | null }

const EFFECT_LABELS: Record<string, string> = {
  fade: "Fade",
  slide_in: "Slide In",
  zoom_in: "Zoom In",
  zoom_out: "Zoom Out",
  spring: "Spring",
  particle_burst: "Particle Burst",
  typewriter_reveal: "Typewriter Reveal",
  blur_reveal: "Blur Reveal",
  scale_up: "Scale Up",
  glow: "Glow",
  shake: "Shake",
  pulse: "Pulse",
  motion_path: "Motion Path",
};

const TRIGGER_LABELS: Record<string, string> = {
  on_slide_enter: "On Slide Enter",
  on_click: "On Click",
  after_element: "After Element",
  with_element: "With Element",
  auto_after_ms: "Auto After",
};

function Row(p: { label: string; value: string }) {
  return (
    <div class="flex justify-between gap-2">
      <span class="text-gray-500">{p.label}</span>
      <span class="font-mono text-gray-200">{p.value}</span>
    </div>
  );
}

export default function AnimationPanel(props: Props) {
  return (
    <div class="border-t border-[#2a2a36] bg-[#0c0c12] px-4 py-3 flex-shrink-0">
      <p class="text-[10px] font-semibold text-gray-500 uppercase tracking-wider mb-2">Animation</p>
      <Show
        when={props.element}
        fallback={
          <p class="text-xs text-gray-600 italic">Select an element to see its animation settings.</p>
        }
      >
        {(el) => {
          const phase = () => el().animation.entrance;
          return (
            <div class="flex flex-col gap-1.5 text-xs text-gray-300">
              <Show
                when={phase()}
                fallback={<p class="text-gray-500">No entrance animation.</p>}
              >
                {(p) => (
                  <>
                    <Row label="Effect" value={EFFECT_LABELS[p().effect.kind] ?? p().effect.kind} />
                    <Row label="Delay" value={`${p().delay_ms} ms`} />
                    <Row label="Duration" value={`${p().duration_ms} ms`} />
                  </>
                )}
              </Show>
              <Row
                label="Trigger"
                value={TRIGGER_LABELS[el().animation.trigger.kind] ?? el().animation.trigger.kind}
              />
            </div>
          );
        }}
      </Show>
    </div>
  );
}
