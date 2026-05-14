import { createSignal, For, Show } from "solid-js";
import type { Element, Slide } from "../../lib/deck-schema";
import type { DeckPatch } from "../../lib/deck-patch";

interface Props {
  slide: Slide;
  zoom: number;
  slideScreenX: number;
  slideScreenY: number;
  onPatch: (p: DeckPatch) => void;
  onElementSelect?: (el: Element | null) => void;
}

function elementBg(el: Element): string {
  if (el.content.kind === "text")  return "rgba(255,255,255,0.08)";
  if (el.content.kind === "image") return "rgba(99,102,241,0.25)";
  return "rgba(255,255,255,0.05)";
}

interface DragState {
  elId: string; startMouseX: number; startMouseY: number;
  startElX: number; startElY: number;
}

export default function SlideEditor(props: Props) {
  const [selectedId, setSelectedId] = createSignal<string | null>(null);

  function selectElement(el: Element | null) {
    setSelectedId(el?.id ?? null);
    props.onElementSelect?.(el);
  }
  const [editingId, setEditingId] = createSignal<string | null>(null);
  const [dragState, setDragState] = createSignal<DragState | null>(null);
  const [dragOffset, setDragOffset] = createSignal<{ dx: number; dy: number }>({ dx: 0, dy: 0 });

  function handleTextCommit(el: Element, div: HTMLDivElement) {
    const newText = div.innerText;
    if (el.content.kind !== "text") return;
    setEditingId(null);
    if (newText === el.content.markdown) return;
    props.onPatch({
      op: "upsert_element", slide_id: props.slide.id,
      element: { ...el, content: { kind: "text", markdown: newText } },
    });
  }

  function onContainerMouseMove(e: MouseEvent) {
    const ds = dragState(); if (!ds) return;
    setDragOffset({
      dx: (e.clientX - ds.startMouseX) / props.zoom,
      dy: (e.clientY - ds.startMouseY) / props.zoom,
    });
  }

  function onContainerMouseUp(e: MouseEvent) {
    const ds = dragState(); if (!ds) return;
    const newX = Math.round(ds.startElX + (e.clientX - ds.startMouseX) / props.zoom);
    const newY = Math.round(ds.startElY + (e.clientY - ds.startMouseY) / props.zoom);
    const el = props.slide.elements.find(el => el.id === ds.elId);
    if (el) props.onPatch({ op: "upsert_element", slide_id: props.slide.id,
      element: { ...el, x: newX, y: newY } });
    setDragState(null); setDragOffset({ dx: 0, dy: 0 });
  }

  return (
    <div class="absolute pointer-events-none"
      style={{
        left: `${props.slideScreenX}px`, top: `${props.slideScreenY}px`,
        width: `${props.slide.width * props.zoom}px`,
        height: `${props.slide.height * props.zoom}px`,
      }}
      onMouseMove={onContainerMouseMove}
      onMouseUp={onContainerMouseUp}
      onMouseLeave={onContainerMouseUp}>
      {/* Drag capture overlay — sits above all elements while dragging */}
      <Show when={dragState() !== null}>
        <div class="absolute inset-0 pointer-events-auto z-40"
          style={{ cursor: "grabbing" }}
          onMouseMove={onContainerMouseMove}
          onMouseUp={onContainerMouseUp}
          onMouseLeave={onContainerMouseUp} />
      </Show>
      <For each={props.slide.elements}>
        {(el) => {
          const isSelected = () => selectedId() === el.id;
          const isEditing  = () => editingId()  === el.id;
          const isDragging = () => dragState()?.elId === el.id;
          const elLeft = () => (el.x + (isDragging() ? dragOffset().dx : 0)) * props.zoom;
          const elTop  = () => (el.y + (isDragging() ? dragOffset().dy : 0)) * props.zoom;

          return (
            <div class="absolute pointer-events-auto rounded-sm"
              style={{
                left: `${elLeft()}px`, top: `${elTop()}px`,
                width: `${el.width * props.zoom}px`, height: `${el.height * props.zoom}px`,
                background: isEditing() ? "rgba(99,102,241,0.12)" : elementBg(el),
                outline: isSelected() ? "2px solid #6366f1" : "none",
                "outline-offset": "1px",
                opacity: String(el.style.opacity ?? 1),
                cursor: el.locked ? "not-allowed" : isDragging() ? "grabbing" : isEditing() ? "text" : "pointer",
              }}
              onClick={(e) => { e.stopPropagation(); if (!el.locked) selectElement(el); }}
              onDblClick={(e) => {
                e.stopPropagation();
                if (el.locked || el.content.kind !== "text") return;
                selectElement(el); setEditingId(el.id);
              }}
              onMouseDown={(e) => {
                if (el.locked || isEditing()) return;
                e.stopPropagation();
                selectElement(el);
                setDragState({
                  elId: el.id,
                  startMouseX: e.clientX, startMouseY: e.clientY,
                  startElX: el.x, startElY: el.y,
                });
              }}>
              {/* Text editing contenteditable */}
              <Show when={isEditing() && el.content.kind === "text"}>
                <div contenteditable={true}
                  class="absolute inset-0 text-white p-1 outline-none overflow-auto whitespace-pre-wrap break-words bg-transparent"
                  style={{ "font-size": `${Math.max(10, 14 * props.zoom)}px` }}
                  ref={(div) => {
                    if (el.content.kind === "text") div.innerText = el.content.markdown;
                    requestAnimationFrame(() => {
                      div.focus();
                      const r = document.createRange();
                      r.selectNodeContents(div); r.collapse(false);
                      const s = window.getSelection();
                      s?.removeAllRanges(); s?.addRange(r);
                    });
                  }}
                  onBlur={(e) => handleTextCommit(el, e.currentTarget)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && !e.shiftKey) {
                      e.preventDefault(); handleTextCommit(el, e.currentTarget);
                    }
                    if (e.key === "Escape") setEditingId(null);
                    e.stopPropagation();
                  }} />
              </Show>
              {/* Lock badge */}
              <Show when={el.locked}>
                <span class="absolute top-0.5 right-0.5 text-yellow-400 text-[10px] leading-none pointer-events-none">
                  &#128274;
                </span>
              </Show>
              {/* Toolbar — shown when selected, not editing, not dragging */}
              <Show when={isSelected() && !isEditing() && !isDragging()}>
                <div class="absolute -top-7 left-0 flex items-center gap-1 px-1.5 py-1 bg-[#1a1a28] border border-[#3a3a4e] rounded-md shadow-lg z-50 pointer-events-auto"
                  style={{ "white-space": "nowrap" }}
                  onMouseDown={(e) => e.stopPropagation()}>
                  <span class="text-gray-400 text-[10px] font-mono mr-1">{el.content.kind}</span>
                  <Show when={el.content.kind === "text" && !el.locked}>
                    <button class="px-1.5 py-0.5 text-[10px] text-indigo-300 hover:bg-indigo-400/10 rounded"
                      onClick={(e) => { e.stopPropagation(); setEditingId(el.id); }}>
                      Edit
                    </button>
                  </Show>
                  <button class="px-1.5 py-0.5 text-[10px] text-yellow-400 hover:bg-yellow-400/10 rounded"
                    onClick={(e) => {
                      e.stopPropagation();
                      props.onPatch({ op: "upsert_element", slide_id: props.slide.id,
                        element: { ...el, locked: !el.locked } });
                    }}>
                    {el.locked ? "Unlock" : "Lock"}
                  </button>
                  <button class="px-1.5 py-0.5 text-[10px] text-red-400 hover:bg-red-400/10 rounded"
                    onClick={(e) => {
                      e.stopPropagation(); selectElement(null);
                      props.onPatch({ op: "delete_element", slide_id: props.slide.id,
                        element_id: el.id });
                    }}>
                    Delete
                  </button>
                </div>
              </Show>
            </div>
          );
        }}
      </For>
      {/* Click-away deselect */}
      <div class="absolute inset-0 pointer-events-auto -z-10"
        onClick={() => { selectElement(null); setEditingId(null); }} />
    </div>
  );
}
