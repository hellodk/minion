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
