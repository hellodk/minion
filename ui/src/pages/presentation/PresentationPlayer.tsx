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
