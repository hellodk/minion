// ui/src/lib/deck-schema.ts
// TypeScript mirror of crates/minion-presentation/src/schema/types.rs
// Types match serde_json snake_case output exactly.

export type DeckId    = string;
export type SlideId   = string;
export type SectionId = string;
export type ElementId = string;
export type AssetId   = string;
export type CameraStepId = string;

export type AspectRatio =
  | "ratio16x9" | "ratio4x3" | "a4_portrait" | "a4_landscape"
  | { custom: { width: number; height: number } };

export type TextDirection = "ltr" | "rtl";
export type PresentationContext = "live_talk" | "async_share" | "kiosk" | "recorded_video";
export type MotionPreset = "subtle" | "balanced" | "cinematic" | "explosive";
export type LayoutKind =
  | "title" | "title_with_media" | "kpi" | "comparison"
  | "timeline" | "quote" | "full_bleed_media" | "architecture"
  | "process" | "matrix" | "storytelling" | "blank";
export type TransitionKind =
  | "zoom" | "fly" | "morph" | "fade" | "push" | "rotate3d" | "portal_zoom";
export type Direction = "left" | "right" | "up" | "down";
export type ImageFit = "cover" | "contain" | "fill";
export type DiagramRenderer = "mermaid" | "graphviz";

export interface Color { r: number; g: number; b: number; a: number }

export function colorToCss(c: Color): string {
  return `rgba(${c.r},${c.g},${c.b},${(c.a / 255).toFixed(3)})`;
}

export interface ColorRoles {
  background: Color; surface: Color; primary: Color; secondary: Color;
  accent: Color; body_text: Color; muted_text: Color;
  chart_series: [Color,Color,Color,Color,Color,Color,Color,Color];
  positive: Color; negative: Color;
}

export interface FontSpec {
  family: string; weight: number; size_scale_base_px: number;
  line_height: number; letter_spacing: number;
}

export interface Typography {
  heading: FontSpec; subheading: FontSpec; body: FontSpec;
  mono: FontSpec; caption: FontSpec; direction: TextDirection;
}

export interface Theme {
  name: string;
  color_roles: ColorRoles;
  typography: Typography;
  motion_preset: MotionPreset;
  font_fallback_stack: string[];
}

export type AssetKind = "image" | "svg" | "font" | "video";
export type AssetStorage =
  | { kind: "bundled_file"; relative_path: string }
  | { kind: "external_url"; url: string };

export interface Asset {
  id: AssetId; kind: AssetKind; filename: string;
  checksum_sha256: string; size_bytes: number; storage: AssetStorage;
}

export type CameraTarget =
  | { kind: "slide"; slide_id: SlideId }
  | { kind: "canvas"; x: number; y: number; width: number; height: number };

export type CameraEasing =
  | { kind: "linear" }
  | { kind: "ease_in_out" }
  | { kind: "spring"; stiffness: number; damping: number };

export interface CameraStep {
  id: CameraStepId; target: CameraTarget; zoom: number;
  duration_ms: number; hold_ms: number; easing: CameraEasing;
}

export type Background =
  | { kind: "solid"; color: Color }
  | { kind: "gradient"; from: Color; to: Color; angle_deg: number }
  | { kind: "image"; asset_id: AssetId; fit: ImageFit }
  | { kind: "svg_pattern"; asset_id: AssetId };

export type AnimEffect =
  | { kind: "fade" }
  | { kind: "slide_in"; direction: Direction }
  | { kind: "zoom_in" } | { kind: "zoom_out" } | { kind: "spring" }
  | { kind: "particle_burst" } | { kind: "typewriter_reveal" }
  | { kind: "blur_reveal" } | { kind: "scale_up" } | { kind: "glow" }
  | { kind: "shake" } | { kind: "pulse" }
  | { kind: "motion_path"; points: { x: number; y: number; t: number }[] };

export interface SpringParams { stiffness: number; damping: number; mass: number }

export interface AnimPhase {
  effect: AnimEffect; delay_ms: number; duration_ms: number;
  spring?: SpringParams;
}

// Matches Rust AnimTrigger struct variants (not tuple variants)
export type AnimTrigger =
  | { kind: "on_slide_enter" }
  | { kind: "on_click" }
  | { kind: "after_element";  element_id: ElementId }
  | { kind: "with_element";   element_id: ElementId }
  | { kind: "auto_after_ms";  ms: number };

export interface ElementAnimation {
  entrance?: AnimPhase; exit?: AnimPhase; emphasis?: AnimPhase;
  trigger: AnimTrigger;
}

export type ElementContent =
  | { kind: "text"; markdown: string }
  | { kind: "image"; asset_id: AssetId; alt: string }
  | { kind: "svg_graphic"; svg_xml: string }
  | { kind: "chart_spec"; spec_json: unknown }
  | { kind: "diagram_dsl"; dsl: string; renderer: DiagramRenderer }
  | { kind: "icon"; name: string; library: string }
  | { kind: "video"; asset_id: AssetId };

export interface ElementStyle {
  opacity: number; border_radius: number;
  box_shadow?: string; custom_css?: string;
}

export interface Element {
  id: ElementId; kind: string; content: ElementContent;
  x: number; y: number; width: number; height: number; z_index: number;
  style: ElementStyle; animation: ElementAnimation;
  user_asset_id?: AssetId; locked: boolean;
}

export interface SpeakerNotes {
  talking_points: string[];
  presenter_cues: { at_element_id?: ElementId; cue: string }[];
  estimated_duration_secs?: number;
  anticipated_questions: string[];
}

export interface SlideTransitionSpec {
  kind: TransitionKind; duration_ms: number;
  easing: string; direction?: Direction;
}

export interface Slide {
  id: SlideId; section_id: SectionId;
  canvas_x: number; canvas_y: number; width: number; height: number;
  z_layer: number; rotation: [number, number, number, number];
  layout: LayoutKind; background: Background;
  transition: SlideTransitionSpec;
  elements: Element[]; speaker_notes: SpeakerNotes;
  auto_advance_ms?: number; user_locked: boolean;
}

export interface Section { id: SectionId; title: string; slides: Slide[] }

export interface MasterElement { element: Element; exclude_slide_ids: SlideId[] }
export interface MasterSlide { elements: MasterElement[]; background?: Background }

export interface DeckMeta {
  title: string; author: string; deck_revision: number; schema_version: string;
  created_at: string; updated_at: string; aspect_ratio: AspectRatio;
  language: string; text_direction: TextDirection;
  target_duration_mins?: number; presentation_context: PresentationContext;
}

export interface Deck {
  meta: DeckMeta; theme: Theme; master: MasterSlide;
  assets: Asset[]; camera_path: CameraStep[];
  sections: Section[]; play_order: SlideId[];
}

export function allSlides(deck: Deck): Slide[] {
  return deck.sections.flatMap(s => s.slides);
}

export function slideById(deck: Deck, id: SlideId): Slide | undefined {
  return allSlides(deck).find(s => s.id === id);
}

export function slideCount(deck: Deck): number {
  return deck.sections.reduce((n, s) => n + s.slides.length, 0);
}

export function createBlankSlide(sectionId: SectionId, canvasX: number, canvasY: number): Slide {
  return {
    id: crypto.randomUUID(),
    section_id: sectionId,
    canvas_x: canvasX,
    canvas_y: canvasY,
    width: 1920,
    height: 1080,
    z_layer: 0,
    rotation: [0, 0, 0, 1],
    layout: "blank",
    background: { kind: "solid", color: { r: 17, g: 17, b: 26, a: 255 } },
    transition: { kind: "fade", duration_ms: 400, easing: "ease" },
    elements: [],
    speaker_notes: { talking_points: [], presenter_cues: [], anticipated_questions: [] },
    user_locked: false,
  };
}

export interface GenerationConfig {
  theme_name?: string; audience: string; tone: string;
  language: string; target_duration_mins?: number;
  slide_count_hint?: number; presentation_context: PresentationContext;
}

export interface DeckSummary {
  id: DeckId; title: string; slide_count: number;
  created_at: string; updated_at: string; thumbnail_data_url?: string;
}
