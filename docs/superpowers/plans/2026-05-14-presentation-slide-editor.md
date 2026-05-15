# Presentation — Plan C: Slide Editor

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add inline slide element editing — click to edit text, drag to reposition, lock/delete elements, with patch persistence.

**Architecture:** Pan/zoom lifted from SpatialCanvas to DeckWorkspace as controlled props. A new `SlideEditor` overlay renders at the selected slide's screen position using those same pan/zoom values. Text elements get `contenteditable` divs. All mutations emit `DeckPatch::UpsertElement` or `DeckPatch::DeleteElement` through the existing `handlePatch` in DeckWorkspace.

**Tech Stack:** SolidJS, TypeScript, TailwindCSS, `deck-schema.ts`, `deck-patch.ts`.

---

## File Map

| Action | File |
|--------|------|
| Modify | `ui/src/pages/presentation/SpatialCanvas.tsx` |
| Modify | `ui/src/pages/presentation/DeckWorkspace.tsx` |
| Create | `ui/src/pages/presentation/SlideEditor.tsx` |

---

### Task 1: Lift pan/zoom state into DeckWorkspace

**Files:**
- Modify: `ui/src/pages/presentation/SpatialCanvas.tsx`
- Modify: `ui/src/pages/presentation/DeckWorkspace.tsx`

SpatialCanvas currently owns `pan`/`zoom` as internal signals. Convert them to controlled props.

- [ ] **Step 1: Replace the Props interface and signal declarations in SpatialCanvas.tsx**

Change the existing `interface Props` at the top of `SpatialCanvas.tsx` from:

```tsx
interface Props { deck: Deck; selectedSlideId: string | null; onSelectSlide: (id: string) => void }
```

to:

```tsx
interface Props {
  deck: Deck;
  selectedSlideId: string | null;
  onSelectSlide: (id: string) => void;
  pan: { x: number; y: number };
  zoom: number;
  onPanChange: (p: { x: number; y: number }) => void;
  onZoomChange: (z: number) => void;
}
```

- [ ] **Step 2: Remove internal signals from SpatialCanvas, use props instead**

Delete these two lines inside `SpatialCanvas`:

```tsx
  const [pan, setPan] = createSignal({x:40,y:40});
  const [zoom, setZoom] = createSignal(0.3);
```

Replace every reference to the local `pan()` with `props.pan`, `zoom()` with `props.zoom`. Replace `setPan(...)` calls with `props.onPanChange(...)` and `setZoom(...)` with `props.onZoomChange(...)`. The three affected sites:

```tsx
  // onWheel — was: setZoom(z => Math.min(20, Math.max(0.05, z*(1-e.deltaY*0.001))));
  props.onZoomChange(Math.min(20, Math.max(0.05, props.zoom * (1 - e.deltaY * 0.001))));

  // onMove — was: setPan(p=>({x:p.x+dx,y:p.y+dy}));
  props.onPanChange({ x: props.pan.x + dx, y: props.pan.y + dy });

  // zoom badge — was: {Math.round(zoom()*100)}%
  {Math.round(props.zoom * 100)}%
```

All other references to `pan` and `zoom` in JSX style attributes become `props.pan` and `props.zoom` directly (no call parens since they are plain values, not signals).

- [ ] **Step 3: Add pan/zoom signals to DeckWorkspace and pass them to SpatialCanvas**

In `DeckWorkspace.tsx`, add two signals after the existing signal declarations:

```tsx
  const [pan, setPan] = createSignal<{ x: number; y: number }>({ x: 40, y: 40 });
  const [zoom, setZoom] = createSignal(0.3);
```

Change the `<SpatialCanvas ...>` call from:

```tsx
<SpatialCanvas deck={deck()} selectedSlideId={selected()} onSelectSlide={setSelected} />
```

to:

```tsx
<SpatialCanvas
  deck={deck()}
  selectedSlideId={selected()}
  onSelectSlide={setSelected}
  pan={pan()}
  zoom={zoom()}
  onPanChange={setPan}
  onZoomChange={setZoom}
/>
```

- [ ] **Step 4: Typecheck**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck
```

Expected: no errors.

- [ ] **Step 5: Commit**

```bash
git add ui/src/pages/presentation/SpatialCanvas.tsx ui/src/pages/presentation/DeckWorkspace.tsx
git commit -m "refactor(presentation): lift pan/zoom state to DeckWorkspace"
```

---

### Task 2: SlideEditor — element display, selection, lock/delete

**Files:**
- Create: `ui/src/pages/presentation/SlideEditor.tsx`
- Modify: `ui/src/pages/presentation/DeckWorkspace.tsx`

- [ ] **Step 1: Create SlideEditor.tsx**

```tsx
// ui/src/pages/presentation/SlideEditor.tsx
import { createSignal, For, Show } from "solid-js";
import type { Element, Slide } from "../../lib/deck-schema";
import type { DeckPatch } from "../../lib/deck-patch";

interface Props {
  slide: Slide;
  zoom: number;
  slideScreenX: number;
  slideScreenY: number;
  onPatch: (p: DeckPatch) => void;
}

function elementBg(el: Element): string {
  if (el.content.kind === "text")  return "rgba(255,255,255,0.08)";
  if (el.content.kind === "image") return "rgba(99,102,241,0.25)";
  return "rgba(255,255,255,0.05)";
}

export default function SlideEditor(props: Props) {
  const [selectedId, setSelectedId] = createSignal<string | null>(null);

  return (
    <div class="absolute pointer-events-none"
      style={{
        left: `${props.slideScreenX}px`, top: `${props.slideScreenY}px`,
        width: `${props.slide.width * props.zoom}px`,
        height: `${props.slide.height * props.zoom}px`,
      }}>
      <For each={props.slide.elements}>
        {(el) => {
          const isSelected = () => selectedId() === el.id;
          return (
            <div class="absolute pointer-events-auto rounded-sm"
              style={{
                left: `${el.x * props.zoom}px`, top: `${el.y * props.zoom}px`,
                width: `${el.width * props.zoom}px`, height: `${el.height * props.zoom}px`,
                background: elementBg(el),
                outline: isSelected() ? "2px solid #6366f1" : "none",
                "outline-offset": "1px",
                opacity: String(el.style.opacity ?? 1),
                cursor: el.locked ? "not-allowed" : "pointer",
              }}
              onClick={(e) => { e.stopPropagation(); if (!el.locked) setSelectedId(el.id); }}>
              <Show when={el.locked}>
                <span class="absolute top-0.5 right-0.5 text-yellow-400 text-[10px] leading-none pointer-events-none">
                  &#128274;
                </span>
              </Show>
              <Show when={isSelected()}>
                <div class="absolute -top-7 left-0 flex items-center gap-1 px-1.5 py-1 bg-[#1a1a28] border border-[#3a3a4e] rounded-md shadow-lg z-50 pointer-events-auto"
                  style={{ "white-space": "nowrap" }}
                  onMouseDown={(e) => e.stopPropagation()}>
                  <span class="text-gray-400 text-[10px] font-mono mr-1">{el.content.kind}</span>
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
                      e.stopPropagation(); setSelectedId(null);
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
      <div class="absolute inset-0 pointer-events-auto -z-10"
        onClick={() => setSelectedId(null)} />
    </div>
  );
}
```

- [ ] **Step 2: Mount SlideEditor in DeckWorkspace**

Add imports at the top of `DeckWorkspace.tsx`:

```tsx
import { slideById } from "../../lib/deck-schema";
import SlideEditor from "./SlideEditor";
```

Add `relative` class to the canvas wrapper div (so absolute overlay is contained) and insert `<SlideEditor>` after `<SpatialCanvas>`. Replace the inner `<div class="flex-1 overflow-hidden">` block with:

```tsx
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
      />
    )}
  </Show>
</div>
```

- [ ] **Step 3: Typecheck**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add ui/src/pages/presentation/SlideEditor.tsx ui/src/pages/presentation/DeckWorkspace.tsx
git commit -m "feat(presentation): add SlideEditor overlay with element selection, lock/delete"
```

---

### Task 3: Text editing via contenteditable

**Files:**
- Modify: `ui/src/pages/presentation/SlideEditor.tsx`

- [ ] **Step 1: Add `editingId` signal and `handleTextCommit` to SlideEditor**

After the existing `selectedId` signal, add:

```tsx
  const [editingId, setEditingId] = createSignal<string | null>(null);

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
```

- [ ] **Step 2: Add `isEditing` inside the `For` callback and `onDblClick` on the element div**

After `const isSelected = () => selectedId() === el.id;` add:

```tsx
          const isEditing = () => editingId() === el.id;
```

On the element `div`, add the `onDblClick` handler and update `background`:

```tsx
              background: isEditing() ? "rgba(99,102,241,0.12)" : elementBg(el),
              // add alongside onClick:
              onDblClick={(e) => {
                e.stopPropagation();
                if (el.locked || el.content.kind !== "text") return;
                setSelectedId(el.id); setEditingId(el.id);
              }}
```

- [ ] **Step 3: Add contenteditable div as first child of the element div**

Insert before the lock badge `<Show>`:

```tsx
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
```

- [ ] **Step 4: Add "Edit" button to toolbar and hide toolbar while editing**

Change `<Show when={isSelected()}>` to `<Show when={isSelected() && !isEditing()}>`.

Before the Lock button, add:

```tsx
                  <Show when={el.content.kind === "text" && !el.locked}>
                    <button class="px-1.5 py-0.5 text-[10px] text-indigo-300 hover:bg-indigo-400/10 rounded"
                      onClick={(e) => { e.stopPropagation(); setEditingId(el.id); }}>
                      Edit
                    </button>
                  </Show>
```

Update the click-away div's `onClick`:

```tsx
        onClick={() => { setSelectedId(null); setEditingId(null); }}
```

- [ ] **Step 5: Typecheck**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck
```

Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add ui/src/pages/presentation/SlideEditor.tsx
git commit -m "feat(presentation): add contenteditable text editing to SlideEditor"
```

---

### Task 4: Drag to reposition elements

**Files:**
- Modify: `ui/src/pages/presentation/SlideEditor.tsx`

- [ ] **Step 1: Add drag types and signals**

After the `editingId` signal, add:

```tsx
  interface DragState {
    elId: string; startMouseX: number; startMouseY: number;
    startElX: number; startElY: number;
  }
  const [dragState, setDragState]   = createSignal<DragState | null>(null);
  const [dragOffset, setDragOffset] = createSignal<{ dx: number; dy: number }>({ dx: 0, dy: 0 });
```

- [ ] **Step 2: Add container mouse handlers**

After the drag signals:

```tsx
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
```

- [ ] **Step 3: Wire handlers to the container div and add drag-capture layer**

Add `onMouseMove`, `onMouseUp`, and `onMouseLeave` to the outermost `<div class="absolute pointer-events-none"`:

```tsx
      onMouseMove={onContainerMouseMove}
      onMouseUp={onContainerMouseUp}
      onMouseLeave={onContainerMouseUp}
```

Add a full-slide drag capture layer as the first child of that div (before `<For>`):

```tsx
      <Show when={dragState() !== null}>
        <div class="absolute inset-0 pointer-events-auto z-40"
          style={{ cursor: "grabbing" }}
          onMouseMove={onContainerMouseMove}
          onMouseUp={onContainerMouseUp}
          onMouseLeave={onContainerMouseUp} />
      </Show>
```

- [ ] **Step 4: Make element position reactive to drag offset**

Inside the `For` callback, after `isEditing`, add:

```tsx
          const isDragging = () => dragState()?.elId === el.id;
          const elLeft = () => (el.x + (isDragging() ? dragOffset().dx : 0)) * props.zoom;
          const elTop  = () => (el.y + (isDragging() ? dragOffset().dy : 0)) * props.zoom;
```

Replace the hardcoded `left`/`top` in the element div style:

```tsx
                left: `${elLeft()}px`, top: `${elTop()}px`,
                cursor: el.locked ? "not-allowed" : isDragging() ? "grabbing" : "pointer",
```

Add `onMouseDown` to the element div:

```tsx
              onMouseDown={(e) => {
                if (el.locked || isEditing() || e.button !== 0) return;
                e.stopPropagation();
                setSelectedId(el.id);
                setDragState({ elId: el.id, startMouseX: e.clientX, startMouseY: e.clientY,
                  startElX: el.x, startElY: el.y });
                setDragOffset({ dx: 0, dy: 0 });
              }}
```

Change the toolbar's `<Show>` to also hide during drag:

```tsx
              <Show when={isSelected() && !isEditing() && !isDragging()}>
```

- [ ] **Step 5: Typecheck and lint**

```bash
cd /home/dk/Documents/git/minion/ui && pnpm typecheck && pnpm lint
```

Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add ui/src/pages/presentation/SlideEditor.tsx
git commit -m "feat(presentation): add drag-to-reposition elements in SlideEditor"
```
