import { For } from "solid-js";
import { slideById, colorToCss } from "../../lib/deck-schema";
import type { Deck, SlideId, Background } from "../../lib/deck-schema";

interface Props {
  deck: Deck;
  selectedSlideId: SlideId | null;
  onSelectSlide: (id: SlideId) => void;
  onAddSlide: () => void;
}

function bgColor(bg: Background): string {
  switch (bg.kind) {
    case "solid":    return colorToCss(bg.color);
    case "gradient": return colorToCss(bg.from);
    default:         return "#1c1c24";
  }
}

export default function SlideTray(props: Props) {
  const orderedSlides = () =>
    props.deck.play_order
      .map((id) => slideById(props.deck, id))
      .filter((s): s is NonNullable<typeof s> => s !== undefined);

  return (
    <div class="flex-shrink-0 h-28 border-t border-[#2a2a36] bg-[#0a0a10] flex items-center gap-3 px-4 overflow-x-auto">
      <For each={orderedSlides()}>
        {(slide, idx) => (
          <button
            onClick={() => props.onSelectSlide(slide.id)}
            title={`Slide ${idx() + 1} — ${slide.layout}`}
            class={`flex-shrink-0 flex flex-col items-center gap-1.5 rounded-lg p-1 border-2 transition-colors ${
              props.selectedSlideId === slide.id
                ? "border-indigo-500"
                : "border-transparent hover:border-[#3a3a48]"
            }`}
          >
            <div
              class="w-40 h-[90px] rounded-md flex-shrink-0 flex items-end justify-start overflow-hidden relative"
              style={{ "background-color": bgColor(slide.background) }}
            >
              <span class="absolute top-1 left-1.5 text-[9px] font-mono text-white/40 leading-none">
                {idx() + 1}
              </span>
              <span class="absolute bottom-1 left-1.5 text-[9px] text-white/50 bg-black/30 rounded px-1 leading-none py-0.5">
                {slide.layout}
              </span>
            </div>
          </button>
        )}
      </For>
      <button
        onClick={() => props.onAddSlide()}
        class="flex-shrink-0 w-40 h-[90px] rounded-md border-2 border-dashed border-[#2a2a36] hover:border-indigo-500/60 text-gray-600 hover:text-indigo-400 text-xl transition-colors flex items-center justify-center"
        title="Add slide"
      >
        +
      </button>
    </div>
  );
}
