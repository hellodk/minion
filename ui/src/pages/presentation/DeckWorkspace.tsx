import { createSignal, onMount, Show } from "solid-js";
import { getDeck, saveDeckPatch } from "../../lib/presentation-api";
import type { DeckPatch } from "../../lib/deck-patch";
import { createDeckStore } from "../../store/deck-store";
import { slideById } from "../../lib/deck-schema";
import type { Element as DeckElement } from "../../lib/deck-schema";
import SpatialCanvas from "./SpatialCanvas";
import SlideEditor from "./SlideEditor";
import AgentSidebar from "./AgentSidebar";
import PresentationPlayer from "./PresentationPlayer";
import ExportDialog from "./ExportDialog";
import SlideTray from "./SlideTray";
import AnimationPanel from "./AnimationPanel";

interface Props { deckId: string; onBack: () => void; initialSessionId?: string }

export default function DeckWorkspace(props: Props) {
  const [store, actions] = createDeckStore();
  const [selected, setSelected] = createSignal<string | null>(null);
  const [playerOpen, setPlayerOpen] = createSignal(false);
  const [exportOpen, setExportOpen] = createSignal(false);
  const [loadErr, setLoadErr] = createSignal<string | null>(null);
  const [sessionId] = createSignal<string | null>(props.initialSessionId ?? null);
  const [pan, setPan] = createSignal<{ x: number; y: number }>({ x: 40, y: 40 });
  const [zoom, setZoom] = createSignal(0.3);
  const [selectedElement, setSelectedElement] = createSignal<DeckElement | null>(null);

  onMount(async () => {
    if (!props.deckId) return;
    try { actions.setDeck(await getDeck(props.deckId)); }
    catch (e) { setLoadErr(String(e)); }
  });

  const handlePatch = (p: DeckPatch) => {
    actions.applyPatch(p);
    saveDeckPatch(props.deckId, [p]).catch(e => console.error("[DeckWorkspace]", e));
  };

  const handleAddSlide = () => {
    console.log("[DeckWorkspace] onAddSlide — not yet implemented");
  };

  return (
    <div class="flex flex-col h-full w-full bg-[#090910] text-white">
      {/* Toolbar */}
      <div class="flex items-center gap-3 px-4 py-2 border-b border-[#2a2a36] bg-[#0f0f14] flex-shrink-0">
        <button onClick={props.onBack} class="text-gray-400 hover:text-white text-sm">&#8592; Back</button>
        <div class="w-px h-4 bg-[#2a2a36]" />
        <h1 class="text-sm font-medium flex-1 truncate">{store.deck?.meta.title ?? "Loading…"}</h1>
        <button onClick={actions.undo} disabled={!actions.canUndo()}
          class="px-2 py-1 text-xs text-gray-400 hover:text-white disabled:opacity-30" title="Undo">&#8617;</button>
        <button onClick={actions.redo} disabled={!actions.canRedo()}
          class="px-2 py-1 text-xs text-gray-400 hover:text-white disabled:opacity-30" title="Redo">&#8618;</button>
        <button onClick={() => setExportOpen(true)}
          class="px-3 py-1.5 text-xs text-gray-400 border border-[#2a2a36] hover:border-gray-500 rounded-lg">Export</button>
        <button onClick={() => setPlayerOpen(true)} disabled={!store.deck}
          class="px-3 py-1.5 text-xs bg-indigo-600 hover:bg-indigo-500 disabled:opacity-40 rounded-lg font-medium">
          &#9654; Present
        </button>
      </div>

      {/* Body */}
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
              <div class="flex flex-col flex-1 overflow-hidden">
                <div class="flex-1 overflow-hidden relative">
                  <SpatialCanvas
                    deck={deck()}
                    selectedSlideId={selected()}
                    onSelectSlide={setSelected}
                    pan={pan()}
                    zoom={zoom()}
                    onPanChange={setPan}
                    onZoomChange={setZoom}
                  />
                  <Show when={selected() !== null && slideById(deck(), selected()!)}>
                    {(slide) => (
                      <SlideEditor
                        slide={slide()}
                        zoom={zoom()}
                        slideScreenX={slide().canvas_x * zoom() + pan().x}
                        slideScreenY={slide().canvas_y * zoom() + pan().y}
                        onPatch={handlePatch}
                        onElementSelect={setSelectedElement}
                      />
                    )}
                  </Show>
                </div>
                <AnimationPanel element={selectedElement()} />
              </div>
              <AgentSidebar sessionId={sessionId()} onPatch={handlePatch} />
            </>
          )}
        </Show>
      </div>

      {/* Slide tray */}
      <Show when={store.deck}>
        {(deck) => (
          <SlideTray
            deck={deck()}
            selectedSlideId={selected()}
            onSelectSlide={setSelected}
            onAddSlide={handleAddSlide}
          />
        )}
      </Show>

      {/* Player overlay */}
      <Show when={playerOpen() && store.deck}>
        <PresentationPlayer deck={store.deck!} onClose={() => setPlayerOpen(false)} />
      </Show>
      <Show when={exportOpen() && store.deck}>
        <ExportDialog deck={store.deck!} deckId={props.deckId} onClose={() => setExportOpen(false)} />
      </Show>
    </div>
  );
}
