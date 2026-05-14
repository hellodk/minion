import { createSignal, Show } from "solid-js";
import type { Deck } from "../../lib/deck-schema";
import { allSlides, colorToCss } from "../../lib/deck-schema";
import { exportToPptx } from "../../lib/export-pptx";
import { exportToPdf } from "../../lib/export-pdf";

type FmtId = "pptx" | "pdf" | "speaker_notes" | "html";
const FORMATS: { id: FmtId; label: string; desc: string }[] = [
  { id: "pptx",          label: "PPTX",              desc: "PowerPoint download" },
  { id: "pdf",           label: "PDF",               desc: "Browser print → Save as PDF" },
  { id: "speaker_notes", label: "Speaker Notes PDF",  desc: "Talking points — print to PDF" },
  { id: "html",          label: "Interactive HTML",   desc: "Self-contained file download" },
];

interface Props { deck: Deck; deckId: string; onClose: () => void }

export default function ExportDialog(props: Props) {
  const [busy, setBusy] = createSignal<FmtId | null>(null);
  const [status, setStatus] = createSignal<{ ok: boolean; msg: string } | null>(null);

  async function run(fmt: FmtId) {
    setBusy(fmt); setStatus(null);
    const safe = props.deck.meta.title.replace(/[^a-z0-9_\-\s]/gi, "_").trim() || "presentation";
    try {
      if (fmt === "pptx")               { await exportToPptx(props.deck, `${safe}.pptx`); setStatus({ ok: true, msg: "PPTX download started." }); }
      else if (fmt === "pdf")           { exportToPdf(props.deck, false); setStatus({ ok: true, msg: "Print dialog opened — choose 'Save as PDF'." }); }
      else if (fmt === "speaker_notes") { exportToPdf(props.deck, true);  setStatus({ ok: true, msg: "Speaker notes print dialog opened." }); }
      else if (fmt === "html")          { exportToHtml(props.deck, safe); setStatus({ ok: true, msg: "HTML download started." }); }
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
      const W = slide.width || 1280;
      const H = slide.height || 720;
      const els = slide.elements
        .filter(el => el.content.kind === "text")
        .sort((a, b) => a.z_index - b.z_index)
        .map(el => {
          const md = (el.content as { kind: "text"; markdown: string }).markdown;
          const escaped = md.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;");
          return `<div style="position:absolute;left:${(el.x/W*100).toFixed(2)}%;top:${(el.y/H*100).toFixed(2)}%;width:${(el.width/W*100).toFixed(2)}%;height:${(el.height/H*100).toFixed(2)}%;color:#fff;overflow:hidden;white-space:pre-wrap;font-size:2.5vw">${escaped}</div>`;
        }).join("");
      return `<div style="background:${bg};position:relative;width:100%;aspect-ratio:16/9;overflow:hidden;page-break-after:always">${els}<span style="position:absolute;bottom:6px;right:10px;color:rgba(255,255,255,.3);font-size:1vw">${i+1}/${slides.length}</span></div>`;
    }).join("\n");
    const title = deck.meta.title.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;");
    const html = `<!DOCTYPE html><html><head><meta charset="utf-8"><title>${title}</title><style>*{margin:0;box-sizing:border-box}body{background:#000;font-family:sans-serif}@page{size:16in 9in;margin:0}</style></head><body>${body}</body></html>`;
    const a = Object.assign(document.createElement("a"), { href: URL.createObjectURL(new Blob([html],{type:"text/html"})), download: `${basename}.html` });
    document.body.appendChild(a); a.click(); document.body.removeChild(a);
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
