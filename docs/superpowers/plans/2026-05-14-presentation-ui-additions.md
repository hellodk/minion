# Presentation — Plan B: Theme Picker + SlideTray

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a theme selection grid to CreationStudio and a slide thumbnail tray to DeckWorkspace.

**Architecture:** ThemePreset definitions in `ui/src/lib/themes.ts`. CreationStudio gains a horizontal theme swatch grid below the Language section. SlideTray is a standalone component wired at the bottom of DeckWorkspace below the flex body div.

**Tech Stack:** SolidJS, TypeScript, TailwindCSS, `deck-schema.ts`.

---

## Task 1: themes.ts + Theme picker in CreationStudio

**Files touched:**
- `ui/src/lib/themes.ts` — create
- `ui/src/pages/presentation/CreationStudio.tsx` — edit

### Step 1.1 — Create `ui/src/lib/themes.ts`

- [ ] Create `ui/src/lib/themes.ts` with this exact content:

```typescript
export interface ThemePreset {
  name: string;
  preview: { bg: string; accent: string; text: string };
}

export const THEMES: ThemePreset[] = [
  { name: "Dark Indigo",   preview: { bg: "#0f0f14", accent: "#6366f1", text: "#ffffff" } },
  { name: "Midnight Blue", preview: { bg: "#0a0a1a", accent: "#3b82f6", text: "#e0e0ff" } },
  { name: "Forest",        preview: { bg: "#0a1a0f", accent: "#22c55e", text: "#d0f0d0" } },
  { name: "Sunset",        preview: { bg: "#1a0a0a", accent: "#f97316", text: "#ffe0d0" } },
  { name: "Monochrome",    preview: { bg: "#111111", accent: "#e5e5e5", text: "#ffffff" } },
  { name: "Corporate",     preview: { bg: "#0f1a2a", accent: "#0ea5e9", text: "#e0f0ff" } },
];
```

### Step 1.2 — Edit `ui/src/pages/presentation/CreationStudio.tsx`

The file uses SolidJS signals. Make these targeted edits:

#### 1.2a — Add import at top (after existing imports, before the `type Tab` line):

```typescript
import { THEMES } from "../../lib/themes";
```

#### 1.2b — Add `theme` signal inside the component body (after the `[lang, setLang]` signal line):

```typescript
  const [theme, setTheme] = createSignal<string>("Dark Indigo");
```

#### 1.2c — In the `generate` function, add `theme_name` to the `GenerationConfig` object:

Replace:
```typescript
      const config: GenerationConfig = {
        audience: audience(), tone: tone(), language: lang(),
        presentation_context: "live_talk",
      };
```
With:
```typescript
      const config: GenerationConfig = {
        audience: audience(), tone: tone(), language: lang(),
        theme_name: theme(),
        presentation_context: "live_talk",
      };
```

#### 1.2d — Add the theme picker section below the Language `</div>` block and above `<Show when={err()}>`:

```tsx
      {/* Theme */}
      <div class="mb-6">
        <p class="text-xs text-gray-500 uppercase tracking-wider mb-3">Theme</p>
        <div class="grid grid-cols-3 gap-3">
          <For each={THEMES}>{(t) => (
            <button
              onClick={() => setTheme(t.name)}
              class={`flex flex-col items-center gap-1.5 rounded-lg p-1 border-2 transition-colors ${
                theme() === t.name
                  ? "border-indigo-500"
                  : "border-transparent hover:border-[#3a3a48]"
              }`}
            >
              {/* Swatch rectangle: 80×50px */}
              <div
                class="w-20 h-[50px] rounded-md flex-shrink-0 relative overflow-hidden"
                style={{ "background-color": t.preview.bg }}
              >
                {/* Accent bar at bottom */}
                <div
                  class="absolute bottom-0 left-0 right-0 h-[6px]"
                  style={{ "background-color": t.preview.accent }}
                />
                {/* Sample text dots */}
                <div class="absolute top-2 left-2 flex flex-col gap-1">
                  <div class="h-1.5 w-10 rounded-full opacity-70"
                    style={{ "background-color": t.preview.text }} />
                  <div class="h-1 w-7 rounded-full opacity-40"
                    style={{ "background-color": t.preview.text }} />
                </div>
              </div>
              <span class="text-[10px] text-gray-400 leading-none text-center">{t.name}</span>
            </button>
          )}</For>
        </div>
      </div>
```

### Step 1.3 — Typecheck

- [ ] Run `cd /home/dk/Documents/git/minion/ui && pnpm typecheck` and confirm zero errors.

---

## Task 2: SlideTray component

**Files touched:**
- `ui/src/pages/presentation/SlideTray.tsx` — create

### Step 2.1 — Create `ui/src/pages/presentation/SlideTray.tsx`

- [ ] Create with this exact content:

```tsx
import { For } from "solid-js";
import { allSlides, slideById } from "../../lib/deck-schema";
import type { Deck, SlideId, Background } from "../../lib/deck-schema";

interface Props {
  deck: Deck;
  selectedSlideId: SlideId | null;
  onSelectSlide: (id: SlideId) => void;
  onAddSlide: () => void;
}

/** Resolve a Background to a CSS background-color string for thumbnail preview. */
function bgColor(bg: Background): string {
  switch (bg.kind) {
    case "solid":    return bg.color;
    case "gradient": return bg.from;
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
            {/* Thumbnail: 160×90px */}
            <div
              class="w-40 h-[90px] rounded-md flex-shrink-0 flex items-end justify-start overflow-hidden relative"
              style={{ "background-color": bgColor(slide.background) }}
            >
              {/* Slide number badge */}
              <span class="absolute top-1 left-1.5 text-[9px] font-mono text-white/40 leading-none">
                {idx() + 1}
              </span>
              {/* Layout label chip */}
              <span class="absolute bottom-1 left-1.5 text-[9px] text-white/50 bg-black/30 rounded px-1 leading-none py-0.5">
                {slide.layout}
              </span>
            </div>
          </button>
        )}
      </For>

      {/* Add Slide button */}
      <button
        onClick={() => props.onAddSlide()}
        class="flex-shrink-0 w-40 h-[90px] rounded-md border-2 border-dashed border-[#2a2a36]
               hover:border-indigo-500/60 text-gray-600 hover:text-indigo-400 text-xl transition-colors
               flex items-center justify-center"
        title="Add slide"
      >
        +
      </button>
    </div>
  );
}
```

### Step 2.2 — Typecheck

- [ ] Run `cd /home/dk/Documents/git/minion/ui && pnpm typecheck` and confirm zero errors.

---

## Task 3: Wire SlideTray into DeckWorkspace

**Files touched:**
- `ui/src/pages/presentation/DeckWorkspace.tsx` — edit

### Step 3.1 — Add import

In `DeckWorkspace.tsx`, add the SlideTray import after the `ExportDialog` import line:

```typescript
import SlideTray from "./SlideTray";
```

### Step 3.2 — Wire component and add `onAddSlide` handler

Add a no-op `handleAddSlide` function in the component body, after `handlePatch`:

```typescript
  const handleAddSlide = () => {
    console.log("[DeckWorkspace] onAddSlide — not yet implemented");
  };
```

### Step 3.3 — Add SlideTray to JSX

The current JSX structure inside the outer `<div class="flex flex-col h-full ...">` ends with:

```tsx
      {/* Body */}
      <div class="flex flex-1 overflow-hidden relative">
        ...
      </div>

      {/* Player overlay */}
```

Add `<SlideTray>` between the Body div closing tag and the Player overlay comment. The exact replacement is:

Replace:
```tsx
      {/* Player overlay */}
      <Show when={playerOpen() && store.deck}>
```

With:
```tsx
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
```

### Step 3.4 — Typecheck and commit

- [ ] Run `cd /home/dk/Documents/git/minion/ui && pnpm typecheck` — must be zero errors.
- [ ] Commit with:

```bash
git add ui/src/lib/themes.ts \
        ui/src/pages/presentation/CreationStudio.tsx \
        ui/src/pages/presentation/SlideTray.tsx \
        ui/src/pages/presentation/DeckWorkspace.tsx
git commit -m "feat(presentation): add theme picker to CreationStudio and SlideTray to DeckWorkspace"
```

---

## Completion checklist

- [ ] `ui/src/lib/themes.ts` exists with 6 THEMES entries
- [ ] CreationStudio renders a 3-column theme swatch grid below Language; selected swatch has indigo border; `theme_name` flows into `GenerationConfig`
- [ ] `SlideTray.tsx` renders 160×90 thumbnails in `play_order`, highlights selected with indigo ring, shows layout label chip, Add Slide button at end, scrollable horizontally, fixed h-28 with border-t
- [ ] `DeckWorkspace.tsx` imports and renders SlideTray below the body flex div
- [ ] `pnpm typecheck` passes with zero errors
