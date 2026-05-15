# Presentation Module — Sub-Plan 4: Export Pipeline

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement PPTX, PDF, and speaker-notes exports from the presentation module, wired into the Export button in DeckWorkspace.

**Architecture:** All export logic runs in the frontend WebView — no Tauri Rust code needed for PPTX or HTML. PDF uses window.print() with CSS @page media. PPTX uses pptxgenjs with direct schema mapping (no DOM capture for MVP). Exports trigger browser downloads or OS print dialogs.

**Tech Stack:** TypeScript, pptxgenjs, solid-js signals for modal state. `@tauri-apps/plugin-dialog` is already installed; `pptxgenjs` is not.

---

## Environment

- Working dir: `/home/dk/Documents/git/minion`
- Frontend: `ui/` — SolidJS, TypeScript, Tailwind CSS, Vite
- Key files:
  - `ui/src/pages/presentation/DeckWorkspace.tsx` — Export button stub at line 40
  - `ui/src/lib/deck-schema.ts` — `Deck`, `Slide`, `Element`, `Background`, `SpeakerNotes`, `Color`, `allSlides`, `colorToCss` (read-only)
  - `ui/src/lib/presentation-api.ts` — `ExportFormat` type (read-only)
  - New: `ui/src/lib/export-pptx.ts`
  - New: `ui/src/lib/export-pdf.ts`
  - New: `ui/src/pages/presentation/ExportDialog.tsx`

---

## Task 1 — Install dependencies + ExportDialog skeleton

### Steps

- [ ] From `ui/`: `pnpm add pptxgenjs` (pptxgenjs 3.x ships its own types; skip `html-to-image` for MVP)
- [ ] Create `ui/src/pages/presentation/ExportDialog.tsx` (code below)
- [ ] `cd ui && pnpm typecheck` — must pass

### `ui/src/pages/presentation/ExportDialog.tsx`

```tsx
import { createSignal, Show } from "solid-js";
import type { Deck } from "../../lib/deck-schema";
import { allSlides, colorToCss } from "../../lib/deck-schema";
import { exportToPptx } from "../../lib/export-pptx";
import { exportToPdf } from "../../lib/export-pdf";

type FmtId = "pptx" | "pdf" | "speaker_notes" | "html";
const FORMATS: { id: FmtId; label: string; desc: string }[] = [
  { id: "pptx",          label: "PPTX",             desc: "PowerPoint download" },
  { id: "pdf",           label: "PDF",              desc: "Browser print → Save as PDF" },
  { id: "speaker_notes", label: "Speaker Notes PDF", desc: "Talking points — print to PDF" },
  { id: "html",          label: "Interactive HTML",  desc: "Self-contained file download" },
];

interface Props { deck: Deck; deckId: string; onClose: () => void }

export default function ExportDialog(props: Props) {
  const [busy, setBusy] = createSignal<FmtId | null>(null);
  const [status, setStatus] = createSignal<{ ok: boolean; msg: string } | null>(null);

  async function run(fmt: FmtId) {
    setBusy(fmt);
    setStatus(null);
    const safe = props.deck.meta.title.replace(/[^a-z0-9_\-\s]/gi, "_").trim() || "presentation";
    try {
      if (fmt === "pptx")          { await exportToPptx(props.deck, `${safe}.pptx`); setStatus({ ok: true, msg: "PPTX download started." }); }
      else if (fmt === "pdf")      { exportToPdf(props.deck, false); setStatus({ ok: true, msg: "Print dialog opened — choose 'Save as PDF'." }); }
      else if (fmt === "speaker_notes") { exportToPdf(props.deck, true); setStatus({ ok: true, msg: "Speaker notes print dialog opened." }); }
      else if (fmt === "html")     { exportToHtml(props.deck, safe); setStatus({ ok: true, msg: "HTML download started." }); }
    } catch (e) {
      setStatus({ ok: false, msg: String(e) });
    } finally { setBusy(null); }
  }

  function exportToHtml(deck: Deck, basename: string): void {
    const slides = allSlides(deck);
    const body = slides.map((slide, i) => {
      const bg = slide.background.kind === "solid"
        ? colorToCss(slide.background.color)
        : slide.background.kind === "gradient"
        ? `linear-gradient(${slide.background.angle_deg}deg,${colorToCss(slide.background.from)},${colorToCss(slide.background.to)})`
        : "#1a1a2e";
      const W = slide.width  || 1280;
      const H = slide.height || 720;
      const els = slide.elements
        .filter(el => el.content.kind === "text")
        .sort((a, b) => a.z_index - b.z_index)
        .map(el => {
          const md = (el.content as { kind: "text"; markdown: string }).markdown;
          return `<div style="position:absolute;left:${(el.x/W*100).toFixed(2)}%;top:${(el.y/H*100).toFixed(2)}%;width:${(el.width/W*100).toFixed(2)}%;height:${(el.height/H*100).toFixed(2)}%;color:#fff;overflow:hidden;white-space:pre-wrap;font-size:2.5vw">${esc(md)}</div>`;
        }).join("");
      return `<div style="background:${bg};position:relative;width:100%;aspect-ratio:16/9;overflow:hidden;page-break-after:always">${els}<span style="position:absolute;bottom:6px;right:10px;color:rgba(255,255,255,.3);font-size:1vw">${i+1}/${slides.length}</span></div>`;
    }).join("\n");
    const html = `<!DOCTYPE html><html><head><meta charset="utf-8"><title>${esc(deck.meta.title)}</title><style>*{margin:0;box-sizing:border-box}body{background:#000;font-family:sans-serif}@page{size:16in 9in;margin:0}</style></head><body>${body}</body></html>`;
    dl(`${basename}.html`, html, "text/html");
  }

  return (
    <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm"
      onClick={e => { if (e.target === e.currentTarget) props.onClose(); }}>
      <div class="bg-[#13131a] border border-[#2a2a36] rounded-xl w-[440px] shadow-2xl p-6 flex flex-col gap-4">
        <div class="flex items-center justify-between">
          <h2 class="text-white font-semibold">Export Presentation</h2>
          <button onClick={props.onClose} class="text-gray-500 hover:text-white text-xl leading-none">&times;</button>
        </div>
        <div class="flex flex-col gap-2">
          {FORMATS.map(f => (
            <button disabled={busy() !== null} onClick={() => run(f.id)}
              class="flex items-center gap-3 px-4 py-3 rounded-lg border border-[#2a2a36] hover:border-indigo-500 hover:bg-indigo-500/10 transition-colors text-left disabled:opacity-50">
              <Show when={busy() === f.id} fallback={
                <span class="w-5 h-5 rounded-sm bg-indigo-600/30 flex items-center justify-center text-indigo-400 text-xs font-bold flex-shrink-0">{f.label[0]}</span>
              }>
                <span class="w-5 h-5 rounded-full border-2 border-indigo-400 border-t-transparent animate-spin flex-shrink-0" />
              </Show>
              <div>
                <div class="text-white text-sm font-medium">{f.label}</div>
                <div class="text-gray-500 text-xs">{f.desc}</div>
              </div>
            </button>
          ))}
        </div>
        <Show when={status()}>
          {s => <div class={`text-xs rounded-lg px-3 py-2 ${s().ok ? "bg-green-900/40 text-green-400 border border-green-700/50" : "bg-red-900/40 text-red-400 border border-red-700/50"}`}>{s().msg}</div>}
        </Show>
      </div>
    </div>
  );
}

function esc(s: string): string {
  return s.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/"/g,"&quot;");
}
function dl(name: string, content: string, mime: string): void {
  const a = Object.assign(document.createElement("a"), { href: URL.createObjectURL(new Blob([content],{type:mime})), download: name });
  document.body.appendChild(a); a.click(); document.body.removeChild(a);
}
```

### Commit message
```
feat(presentation): add ExportDialog skeleton with 4 format buttons
```

---

## Task 2 — PPTX export (`ui/src/lib/export-pptx.ts`)

**Strategy:** pptxgenjs schema-driven mapping — no DOM capture. Each slide in `play_order` → PPTX slide with background color fill + text boxes + speaker notes. `pptx.writeFile()` triggers browser download.

### Steps

- [ ] Create `ui/src/lib/export-pptx.ts` (code below)
- [ ] `cd ui && pnpm typecheck` — must pass
- [ ] Commit

### `ui/src/lib/export-pptx.ts`

```typescript
import PptxGenJS from "pptxgenjs";
import { allSlides } from "./deck-schema";
import type { Deck, Slide, Background, Color } from "./deck-schema";

function hex(c: Color): string {
  const h = (n: number) => Math.round(n).toString(16).padStart(2,"0").toUpperCase();
  return `${h(c.r)}${h(c.g)}${h(c.b)}`;
}

function bgHex(bg: Background): string {
  if (bg.kind === "solid")    return hex(bg.color);
  if (bg.kind === "gradient") return hex(bg.from);
  return "1A1A2E";
}

/** Physical slide size in pptxgenjs inches (LAYOUT_WIDE). */
const W_IN = 13.33;
const H_IN = 7.5;

function addSlide(pptx: PptxGenJS, slide: Slide): void {
  const s = pptx.addSlide();
  s.background = { fill: bgHex(slide.background) };

  const CW = slide.width  || 1280;
  const CH = slide.height || 720;

  const textEls = slide.elements
    .filter(el => el.content.kind === "text")
    .sort((a, b) => a.z_index - b.z_index);

  for (const el of textEls) {
    const raw = (el.content as { kind: "text"; markdown: string }).markdown;
    const plain = raw
      .replace(/\*\*(.+?)\*\*/g, "$1")
      .replace(/\*(.+?)\*/g, "$1")
      .replace(/`(.+?)`/g, "$1")
      .replace(/^#+\s+/gm, "")
      .trim();
    if (!plain) continue;

    s.addText(plain, {
      x: (el.x      / CW) * W_IN,
      y: (el.y      / CH) * H_IN,
      w: (el.width  / CW) * W_IN,
      h: (el.height / CH) * H_IN,
      fontSize: 24,
      color: "FFFFFF",
      wrap: true,
      valign: "top",
      align: "left",
    });
  }

  const notes = slide.speaker_notes.talking_points;
  if (notes.length > 0) s.addNotes(notes.join("\n"));
}

/**
 * Export deck to a PPTX file — triggers browser download.
 */
export async function exportToPptx(deck: Deck, filename: string): Promise<void> {
  const pptx = new PptxGenJS();
  pptx.author  = deck.meta.author || "Minion";
  pptx.title   = deck.meta.title;
  pptx.layout  = "LAYOUT_WIDE";

  const ar = deck.meta.aspect_ratio;
  if (typeof ar === "object" && "custom" in ar) {
    pptx.defineLayout({ name: "CUSTOM", width: ar.custom.width / 96, height: ar.custom.height / 96 });
    pptx.layout = "CUSTOM";
  }

  const slideMap = new Map(allSlides(deck).map(s => [s.id, s]));
  const ids = deck.play_order.length > 0 ? deck.play_order : [...slideMap.keys()];
  for (const id of ids) {
    const slide = slideMap.get(id);
    if (slide) addSlide(pptx, slide);
  }

  await pptx.writeFile({ fileName: filename });
}
```

### Commit message
```
feat(presentation): add PPTX export via pptxgenjs (schema-driven)
```

---

## Task 3 — PDF export (`ui/src/lib/export-pdf.ts`)

**Strategy:** Open a new window, write HTML with `page-break-after:always` per slide, call `window.print()`. Speaker notes mode renders a talking-points table instead.

### Steps

- [ ] Create `ui/src/lib/export-pdf.ts` (code below)
- [ ] `cd ui && pnpm typecheck` — must pass
- [ ] Commit

### `ui/src/lib/export-pdf.ts`

```typescript
import { allSlides, colorToCss } from "./deck-schema";
import type { Deck, Slide, Background } from "./deck-schema";

function bgCss(bg: Background): string {
  if (bg.kind === "solid")    return colorToCss(bg.color);
  if (bg.kind === "gradient") return `linear-gradient(${bg.angle_deg}deg,${colorToCss(bg.from)},${colorToCss(bg.to)})`;
  return "#1a1a2e";
}

function esc(s: string): string {
  return s.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/"/g,"&quot;");
}

function renderSlide(slide: Slide, idx: number, total: number): string {
  const W = slide.width  || 1280;
  const H = slide.height || 720;
  const els = slide.elements
    .filter(el => el.content.kind === "text")
    .sort((a,b) => a.z_index - b.z_index)
    .map(el => {
      const md = (el.content as { kind:"text"; markdown:string }).markdown;
      return `<div style="position:absolute;left:${(el.x/W*100).toFixed(2)}%;top:${(el.y/H*100).toFixed(2)}%;width:${(el.width/W*100).toFixed(2)}%;height:${(el.height/H*100).toFixed(2)}%;color:#fff;overflow:hidden;white-space:pre-wrap;font-size:2.5vw">${esc(md)}</div>`;
    }).join("");
  return `<div style="background:${bgCss(slide.background)};position:relative;width:100%;aspect-ratio:16/9;overflow:hidden;page-break-after:always">${els}<span style="position:absolute;bottom:6px;right:10px;color:rgba(255,255,255,.3);font-size:1vw">${idx+1}/${total}</span></div>`;
}

function renderNotesHtml(deck: Deck): string {
  const slides = allSlides(deck);
  const rows = slides.map((s, i) => {
    const headline = s.elements.filter(el => el.content.kind === "text").sort((a,b)=>a.z_index-b.z_index)
      .map(el => (el.content as {kind:"text";markdown:string}).markdown.split("\n")[0]).find(Boolean) ?? "(untitled)";
    const pts = [...s.speaker_notes.talking_points, ...s.speaker_notes.presenter_cues.map(c=>`[CUE] ${c.cue}`)];
    const dur = s.speaker_notes.estimated_duration_secs;
    return `<tr><td>${i+1}</td><td>${esc(headline)}</td><td>${pts.length?`<ul>${pts.map(p=>`<li>${esc(p)}</li>`).join("")}</ul>`:"—"}</td><td>${dur!=null?`${dur}s`:"—"}</td></tr>`;
  }).join("");
  return `<!DOCTYPE html><html><head><meta charset="utf-8"><title>Speaker Notes — ${esc(deck.meta.title)}</title><style>body{font-family:Georgia,serif;margin:2cm;color:#111}table{border-collapse:collapse;width:100%;font-size:.85rem}th,td{border:1px solid #ccc;padding:6px 10px;vertical-align:top}th{background:#f0f0f0}ul{margin:0;padding-left:16px}@page{margin:1.5cm}</style></head><body><h1>${esc(deck.meta.title)}</h1><p>Speaker notes — ${slides.length} slides</p><table><thead><tr><th>#</th><th>Slide</th><th>Notes &amp; Cues</th><th>Time</th></tr></thead><tbody>${rows}</tbody></table></body></html>`;
}

function renderSlidesHtml(deck: Deck): string {
  const slides = allSlides(deck);
  const body = slides.map((s,i) => renderSlide(s,i,slides.length)).join("\n");
  return `<!DOCTYPE html><html><head><meta charset="utf-8"><title>${esc(deck.meta.title)}</title><style>*{margin:0;box-sizing:border-box}body{background:#000;font-family:sans-serif}@page{size:16in 9in;margin:0}</style></head><body>${body}</body></html>`;
}

function printWindow(html: string): void {
  const win = window.open("", "_blank");
  if (!win) throw new Error("Pop-up blocked — allow pop-ups for this app.");
  win.document.open();
  win.document.write(html);
  win.document.close();
  win.addEventListener("load", () => { win.focus(); win.print(); });
}

/**
 * Export deck to PDF via browser print dialog.
 * @param speakerNotesOnly  If true, prints a talking-points table instead of slides.
 */
export function exportToPdf(deck: Deck, speakerNotesOnly = false): void {
  printWindow(speakerNotesOnly ? renderNotesHtml(deck) : renderSlidesHtml(deck));
}
```

### Commit message
```
feat(presentation): add PDF + speaker-notes export via window.print()
```

---

## Task 4 — Wire ExportDialog into DeckWorkspace

### Steps

- [ ] Edit `ui/src/pages/presentation/DeckWorkspace.tsx` — three changes:
  1. Add `import ExportDialog from "./ExportDialog";` after the last import
  2. Add `const [exportOpen, setExportOpen] = createSignal(false);` after the `playerOpen` signal (line 14)
  3. Replace `onClick={() => console.log("export stub")}` (line 40) with `onClick={() => setExportOpen(true)}`
  4. Add the `<Show>` block for `ExportDialog` after the Player overlay block (after line 71)

```tsx
{/* Export dialog overlay */}
<Show when={exportOpen() && store.deck}>
  <ExportDialog deck={store.deck!} deckId={props.deckId} onClose={() => setExportOpen(false)} />
</Show>
```

- [ ] `cd ui && pnpm typecheck && pnpm lint` — must pass
- [ ] Commit

### Commit message
```
feat(presentation): wire ExportDialog into DeckWorkspace Export button
```

---

## Checklist summary

- [ ] Task 1: `pnpm add pptxgenjs` + `ExportDialog.tsx` + typecheck pass
- [ ] Task 2: `export-pptx.ts` + typecheck pass + commit
- [ ] Task 3: `export-pdf.ts` + typecheck pass + commit
- [ ] Task 4: `DeckWorkspace.tsx` patched + typecheck + lint + commit

## Out of scope (future)

- `html-to-image` slide capture for pixel-perfect PPTX thumbnails
- Tauri `export_presentation` Rust command (stub — leave for server-side large-deck export)
- Chart/diagram/SVG element rendering in PPTX (text-only for MVP)
