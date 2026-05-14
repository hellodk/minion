use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Newtype IDs ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeckId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SlideId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SectionId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ElementId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AssetId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CameraStepId(pub Uuid);

impl DeckId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
impl SlideId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
impl SectionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
impl ElementId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
impl AssetId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}
impl CameraStepId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for DeckId {
    fn default() -> Self {
        Self::new()
    }
}
impl Default for SlideId {
    fn default() -> Self {
        Self::new()
    }
}
impl Default for SectionId {
    fn default() -> Self {
        Self::new()
    }
}
impl Default for ElementId {
    fn default() -> Self {
        Self::new()
    }
}
impl Default for AssetId {
    fn default() -> Self {
        Self::new()
    }
}
impl Default for CameraStepId {
    fn default() -> Self {
        Self::new()
    }
}

// ── Meta enums ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AspectRatio {
    Ratio16x9,
    Ratio4x3,
    A4Portrait,
    A4Landscape,
    Custom { width: f64, height: f64 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextDirection {
    Ltr,
    Rtl,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationContext {
    LiveTalk,
    AsyncShare,
    Kiosk,
    RecordedVideo,
}

// ── DeckMeta ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeckMeta {
    pub title: String,
    pub author: String,
    pub deck_revision: u32,
    pub schema_version: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub aspect_ratio: AspectRatio,
    pub language: String,
    pub text_direction: TextDirection,
    pub target_duration_mins: Option<u32>,
    pub presentation_context: PresentationContext,
}

// ── Theme ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub fn to_css(&self) -> String {
        format!("rgba({},{},{},{})", self.r, self.g, self.b, self.a as f32 / 255.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorRoles {
    pub background: Color,
    pub surface: Color,
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,
    pub body_text: Color,
    pub muted_text: Color,
    pub chart_series: [Color; 8],
    pub positive: Color,
    pub negative: Color,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontSpec {
    pub family: String,
    pub weight: u16,
    pub size_scale_base_px: f32,
    pub line_height: f32,
    pub letter_spacing: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Typography {
    pub heading: FontSpec,
    pub subheading: FontSpec,
    pub body: FontSpec,
    pub mono: FontSpec,
    pub caption: FontSpec,
    pub direction: TextDirection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MotionPreset {
    Subtle,
    Balanced,
    Cinematic,
    Explosive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub color_roles: ColorRoles,
    pub typography: Typography,
    pub motion_preset: MotionPreset,
    pub font_fallback_stack: Vec<String>,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            name: "Default Dark".into(),
            color_roles: ColorRoles {
                background: Color::rgb(15, 15, 20),
                surface: Color::rgb(28, 28, 36),
                primary: Color::rgb(255, 255, 255),
                secondary: Color::rgb(160, 160, 180),
                accent: Color::rgb(99, 102, 241),
                body_text: Color::rgb(220, 220, 230),
                muted_text: Color::rgb(100, 100, 120),
                chart_series: [
                    Color::rgb(99, 102, 241),
                    Color::rgb(236, 72, 153),
                    Color::rgb(34, 197, 94),
                    Color::rgb(251, 191, 36),
                    Color::rgb(249, 115, 22),
                    Color::rgb(20, 184, 166),
                    Color::rgb(168, 85, 247),
                    Color::rgb(239, 68, 68),
                ],
                positive: Color::rgb(34, 197, 94),
                negative: Color::rgb(239, 68, 68),
            },
            typography: Typography {
                heading: FontSpec {
                    family: "Inter".into(),
                    weight: 700,
                    size_scale_base_px: 48.0,
                    line_height: 1.1,
                    letter_spacing: -0.02,
                },
                subheading: FontSpec {
                    family: "Inter".into(),
                    weight: 500,
                    size_scale_base_px: 28.0,
                    line_height: 1.3,
                    letter_spacing: -0.01,
                },
                body: FontSpec {
                    family: "Inter".into(),
                    weight: 400,
                    size_scale_base_px: 18.0,
                    line_height: 1.6,
                    letter_spacing: 0.0,
                },
                mono: FontSpec {
                    family: "JetBrains Mono".into(),
                    weight: 400,
                    size_scale_base_px: 16.0,
                    line_height: 1.5,
                    letter_spacing: 0.0,
                },
                caption: FontSpec {
                    family: "Inter".into(),
                    weight: 400,
                    size_scale_base_px: 13.0,
                    line_height: 1.4,
                    letter_spacing: 0.01,
                },
                direction: TextDirection::Ltr,
            },
            motion_preset: MotionPreset::Cinematic,
            font_fallback_stack: vec![
                "Inter".into(),
                "Helvetica Neue".into(),
                "Arial".into(),
                "sans-serif".into(),
            ],
        }
    }
}

// ── Assets ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Image,
    Svg,
    Font,
    Video,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AssetStorage {
    BundledFile { relative_path: String },
    ExternalUrl { url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: AssetId,
    pub kind: AssetKind,
    pub filename: String,
    pub checksum_sha256: String,
    pub size_bytes: u64,
    pub storage: AssetStorage,
}

// ── Camera path ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CameraTarget {
    Slide { slide_id: SlideId },
    Canvas { x: f64, y: f64, width: f64, height: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CameraEasing {
    Linear,
    EaseInOut,
    Spring { stiffness: f64, damping: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraStep {
    pub id: CameraStepId,
    pub target: CameraTarget,
    pub zoom: f64,
    pub duration_ms: u32,
    pub hold_ms: u32,
    pub easing: CameraEasing,
}

// ── Layout ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutKind {
    Title,
    TitleWithMedia,
    Kpi,
    Comparison,
    Timeline,
    Quote,
    FullBleedMedia,
    Architecture,
    Process,
    Matrix,
    Storytelling,
    Blank,
}

// ── Background ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Background {
    Solid { color: Color },
    Gradient { from: Color, to: Color, angle_deg: f64 },
    Image { asset_id: AssetId, fit: ImageFit },
    SvgPattern { asset_id: AssetId },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFit {
    Cover,
    Contain,
    Fill,
}

// ── Slide transition ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionKind {
    Zoom,
    Fly,
    Morph,
    Fade,
    Push,
    Rotate3d,
    PortalZoom,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideTransitionSpec {
    pub kind: TransitionKind,
    pub duration_ms: u32,
    pub easing: String,
    pub direction: Option<Direction>,
}

// ── Element animations ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnimEffect {
    Fade,
    SlideIn { direction: Direction },
    ZoomIn,
    ZoomOut,
    Spring,
    ParticleBurst,
    TypewriterReveal,
    BlurReveal,
    ScaleUp,
    Glow,
    Shake,
    Pulse,
    MotionPath { points: Vec<PathPoint> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathPoint {
    pub x: f64,
    pub y: f64,
    pub t: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpringParams {
    /// 1.0 – 2000.0
    pub stiffness: f64,
    /// 0.1 – 100.0
    pub damping: f64,
    /// 0.1 – 10.0
    pub mass: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimPhase {
    pub effect: AnimEffect,
    pub delay_ms: u32,
    pub duration_ms: u32,
    pub spring: Option<SpringParams>,
}

// IMPORTANT: AnimTrigger uses struct variants (not tuple variants) because
// #[serde(tag)] does not support tuple variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnimTrigger {
    OnSlideEnter,
    OnClick,
    AfterElement { element_id: ElementId },
    WithElement { element_id: ElementId },
    AutoAfterMs { ms: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementAnimation {
    pub entrance: Option<AnimPhase>,
    pub exit: Option<AnimPhase>,
    pub emphasis: Option<AnimPhase>,
    pub trigger: AnimTrigger,
}

// ── Element ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ElementKind {
    Text,
    Image,
    SvgGraphic,
    ChartSpec,
    DiagramDsl,
    Icon,
    Video,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ElementContent {
    Text { markdown: String },
    Image { asset_id: AssetId, alt: String },
    SvgGraphic { svg_xml: String },
    ChartSpec { spec_json: serde_json::Value },
    DiagramDsl { dsl: String, renderer: DiagramRenderer },
    Icon { name: String, library: String },
    Video { asset_id: AssetId },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagramRenderer {
    Mermaid,
    Graphviz,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementStyle {
    pub opacity: f32,
    pub border_radius: f32,
    pub box_shadow: Option<String>,
    pub custom_css: Option<String>,
}

impl Default for ElementStyle {
    fn default() -> Self {
        Self { opacity: 1.0, border_radius: 0.0, box_shadow: None, custom_css: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Element {
    pub id: ElementId,
    pub kind: ElementKind,
    pub content: ElementContent,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub z_index: u32,
    pub style: ElementStyle,
    pub animation: ElementAnimation,
    pub user_asset_id: Option<AssetId>,
    pub locked: bool,
}

// ── Speaker notes ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PresenterCue {
    pub at_element_id: Option<ElementId>,
    pub cue: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpeakerNotes {
    pub talking_points: Vec<String>,
    pub presenter_cues: Vec<PresenterCue>,
    pub estimated_duration_secs: Option<u32>,
    pub anticipated_questions: Vec<String>,
}

// ── MasterSlide ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterElement {
    pub element: Element,
    pub exclude_slide_ids: Vec<SlideId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterSlide {
    pub elements: Vec<MasterElement>,
    pub background: Option<Background>,
}

// ── Slide + Section ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slide {
    pub id: SlideId,
    pub section_id: SectionId,
    pub canvas_x: f64,
    pub canvas_y: f64,
    pub width: f64,
    pub height: f64,
    pub z_layer: i32,
    /// Quaternion [w, x, y, z] — avoids gimbal lock
    pub rotation: [f64; 4],
    pub layout: LayoutKind,
    pub background: Background,
    pub transition: SlideTransitionSpec,
    pub elements: Vec<Element>,
    pub speaker_notes: SpeakerNotes,
    pub auto_advance_ms: Option<u32>,
    pub user_locked: bool,
}

impl Slide {
    pub fn new(section_id: SectionId, x: f64, y: f64, layout: LayoutKind) -> Self {
        Self {
            id: SlideId::new(),
            section_id,
            canvas_x: x,
            canvas_y: y,
            width: 1920.0,
            height: 1080.0,
            z_layer: 0,
            rotation: [1.0, 0.0, 0.0, 0.0],
            layout,
            background: Background::Solid { color: Color::rgb(15, 15, 20) },
            transition: SlideTransitionSpec {
                kind: TransitionKind::Fade,
                duration_ms: 600,
                easing: "ease-in-out".into(),
                direction: None,
            },
            elements: vec![],
            speaker_notes: SpeakerNotes::default(),
            auto_advance_ms: None,
            user_locked: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub id: SectionId,
    pub title: String,
    pub slides: Vec<Slide>,
}

// ── Top-level Deck ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deck {
    pub meta: DeckMeta,
    pub theme: Theme,
    pub master: MasterSlide,
    pub assets: Vec<Asset>,
    pub camera_path: Vec<CameraStep>,
    pub sections: Vec<Section>,
    pub play_order: Vec<SlideId>,
}

impl Deck {
    pub fn all_slides(&self) -> impl Iterator<Item = &Slide> {
        self.sections.iter().flat_map(|s| s.slides.iter())
    }

    pub fn slide_by_id(&self, id: &SlideId) -> Option<&Slide> {
        self.all_slides().find(|s| &s.id == id)
    }

    pub fn slide_count(&self) -> usize {
        self.sections.iter().map(|s| s.slides.len()).sum()
    }
}

// ── DeckPatch ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DeckPatch {
    SetMeta { meta: DeckMeta },
    SetTheme { theme: Theme },
    UpsertSlide { section_id: SectionId, slide: Slide },
    DeleteSlide { slide_id: SlideId },
    UpsertElement { slide_id: SlideId, element: Element },
    DeleteElement { slide_id: SlideId, element_id: ElementId },
    SetPlayOrder { order: Vec<SlideId> },
    SetCameraPath { path: Vec<CameraStep> },
    UpsertAsset { asset: Asset },
}

// ── Generation config ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    pub theme_name: Option<String>,
    pub audience: String,
    pub tone: String,
    pub language: String,
    pub target_duration_mins: Option<u32>,
    pub slide_count_hint: Option<u32>,
    pub presentation_context: PresentationContext,
}

// ── Deck summary ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeckSummary {
    pub id: DeckId,
    pub title: String,
    pub slide_count: usize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub thumbnail_data_url: Option<String>,
}
