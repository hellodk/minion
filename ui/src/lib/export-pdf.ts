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
  // document.write finishes synchronously; the load event may have already fired.
  if (win.document.readyState === "complete") {
    win.focus(); win.print();
  } else {
    win.addEventListener("load", () => { win.focus(); win.print(); }, { once: true });
  }
}

export function exportToPdf(deck: Deck, speakerNotesOnly = false): void {
  printWindow(speakerNotesOnly ? renderNotesHtml(deck) : renderSlidesHtml(deck));
}
