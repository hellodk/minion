// ui/src/lib/deck-patch.ts
import type {
  Deck, DeckMeta, Theme, Slide, Element, SlideId, SectionId,
  ElementId, CameraStep, Asset,
} from "./deck-schema";

export type DeckPatch =
  | { op: "set_meta"; meta: DeckMeta }
  | { op: "set_theme"; theme: Theme }
  | { op: "upsert_slide"; section_id: SectionId; slide: Slide }
  | { op: "delete_slide"; slide_id: SlideId }
  | { op: "upsert_element"; slide_id: SlideId; element: Element }
  | { op: "delete_element"; slide_id: SlideId; element_id: ElementId }
  | { op: "set_play_order"; order: SlideId[] }
  | { op: "set_camera_path"; path: CameraStep[] }
  | { op: "upsert_asset"; asset: Asset };

export function applyPatch(deck: Deck, patch: DeckPatch): Deck {
  switch (patch.op) {
    case "set_meta":
      return { ...deck, meta: patch.meta };
    case "set_theme":
      return { ...deck, theme: patch.theme };
    case "set_play_order":
      return { ...deck, play_order: patch.order };
    case "set_camera_path":
      return { ...deck, camera_path: patch.path };
    case "upsert_asset": {
      const assets = deck.assets.filter(a => a.id !== patch.asset.id);
      return { ...deck, assets: [...assets, patch.asset] };
    }
    case "upsert_slide": {
      const sections = deck.sections.map(sec => {
        if (sec.id !== patch.section_id) return sec;
        const slides = sec.slides.filter(s => s.id !== patch.slide.id);
        return { ...sec, slides: [...slides, patch.slide] };
      });
      return { ...deck, sections };
    }
    case "delete_slide": {
      const sections = deck.sections.map(sec => ({
        ...sec,
        slides: sec.slides.filter(s => s.id !== patch.slide_id),
      }));
      const play_order = deck.play_order.filter(id => id !== patch.slide_id);
      return { ...deck, sections, play_order };
    }
    case "upsert_element": {
      const sections = deck.sections.map(sec => ({
        ...sec,
        slides: sec.slides.map(slide => {
          if (slide.id !== patch.slide_id) return slide;
          const elements = slide.elements.filter(e => e.id !== patch.element.id);
          return { ...slide, elements: [...elements, patch.element] };
        }),
      }));
      return { ...deck, sections };
    }
    case "delete_element": {
      const sections = deck.sections.map(sec => ({
        ...sec,
        slides: sec.slides.map(slide => {
          if (slide.id !== patch.slide_id) return slide;
          return {
            ...slide,
            elements: slide.elements.filter(e => e.id !== patch.element_id),
          };
        }),
      }));
      return { ...deck, sections };
    }
  }
}

export function applyPatches(deck: Deck, patches: DeckPatch[]): Deck {
  return patches.reduce(applyPatch, deck);
}
