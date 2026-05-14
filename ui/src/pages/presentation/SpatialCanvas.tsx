import { createSignal, For } from "solid-js";
import type { Deck, Slide } from "../../lib/deck-schema";
import { allSlides, colorToCss } from "../../lib/deck-schema";

interface Props {
  deck: Deck;
  selectedSlideId: string | null;
  onSelectSlide: (id: string) => void;
  pan: { x: number; y: number };
  zoom: number;
  onPanChange: (p: { x: number; y: number }) => void;
  onZoomChange: (z: number) => void;
}

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
  const [drag, setDrag] = createSignal(false);
  let ref: HTMLDivElement | undefined;
  let last = {x:0,y:0};

  const vp = () => ({w: ref?.clientWidth??800, h: ref?.clientHeight??600});

  const onWheel = (e: WheelEvent) => {
    e.preventDefault();
    props.onZoomChange(Math.min(20, Math.max(0.05, props.zoom*(1-e.deltaY*0.001))));
  };
  const onDown = (e: MouseEvent) => { setDrag(true); last={x:e.clientX,y:e.clientY}; };
  const onMove = (e: MouseEvent) => {
    if (!drag()) return;
    const dx=e.clientX-last.x, dy=e.clientY-last.y; last={x:e.clientX,y:e.clientY};
    props.onPanChange({x:props.pan.x+dx,y:props.pan.y+dy});
  };
  const onUp = () => setDrag(false);

  const slides = () => {
    const {w,h}=vp();
    return allSlides(props.deck).filter(s=>visible(s,props.pan,props.zoom,w,h));
  };

  return (
    <div ref={ref} class="relative overflow-hidden w-full h-full select-none"
      style={{cursor:drag()?"grabbing":"grab",background:"#090910"}}
      onWheel={onWheel} onMouseDown={onDown} onMouseMove={onMove}
      onMouseUp={onUp} onMouseLeave={onUp}>
      {/* Dot grid background */}
      <div class="absolute inset-0 pointer-events-none"
        style={{"background-image":"radial-gradient(circle,#2a2a3a 1px,transparent 1px)",
          "background-size":`${32*props.zoom}px ${32*props.zoom}px`,
          "background-position":`${props.pan.x%(32*props.zoom)}px ${props.pan.y%(32*props.zoom)}px`}} />
      {/* Transform layer */}
      <div class="absolute" style={{"transform-origin":"0 0",
        transform:`translate(${props.pan.x}px,${props.pan.y}px) scale(${props.zoom})`}}>
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
        {Math.round(props.zoom*100)}%
      </div>
    </div>
  );
}
