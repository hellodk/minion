# Presentation Module — Sub-Plan 1: Foundation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create the `minion-presentation` crate with DeckSchema types, DB schema, streaming LLM extension, Tauri IPC scaffolding, TypeScript schema types, and a SolidJS page shell — everything the AI Pipeline and Frontend sub-plans depend on.

**Architecture:** DeckSchema is the central JSON contract between all components. It lives as Rust structs (serde) on the backend and mirrored TypeScript types on the frontend. The Tauri IPC layer exposes 6 typed commands. This sub-plan produces no visible UI — only the plumbing all later sub-plans build on.

**Tech Stack:** Rust (serde, serde_json, jsonschema, uuid, chrono, r2d2/rusqlite), TypeScript (SolidJS, Tauri v2 IPC), tokio, reqwest (SSE streaming).

**Spec:** `docs/superpowers/specs/2026-05-14-presentation-module-design.md`

---

## File Map

### Created
| File | Responsibility |
|---|---|
| `crates/minion-presentation/Cargo.toml` | Crate manifest + deps |
| `crates/minion-presentation/src/lib.rs` | Public re-exports |
| `crates/minion-presentation/src/schema/mod.rs` | DeckSchema re-exports |
| `crates/minion-presentation/src/schema/types.rs` | All DeckSchema structs/enums |
| `crates/minion-presentation/src/schema/quaternion.rs` | Quaternion math + Euler helper |
| `crates/minion-presentation/src/schema/validate.rs` | Per-step schema validation |
| `crates/minion-presentation/src/db.rs` | Deck persistence against minion-db |
| `crates/minion-presentation/migrations.rs` | SQL migrations for 4 tables |
| `crates/minion-presentation/tests/schema_tests.rs` | Schema type + validation tests |
| `crates/minion-llm/src/streaming.rs` | SSE streaming + vision message types |
| `ui/src/lib/deck-schema.ts` | TypeScript mirror of DeckSchema |
| `ui/src/lib/deck-patch.ts` | DeckPatch type + apply helper |
| `ui/src/lib/presentation-api.ts` | Tauri invoke wrappers (typed) |
| `ui/src/pages/Presentation.tsx` | SolidJS page shell + sub-route switch |
| `ui/src/pages/presentation/PresentationLibrary.tsx` | Deck list skeleton (empty state) |

### Modified
| File | Change |
|---|---|
| `Cargo.toml` | Add `minion-presentation` to workspace members |
| `crates/minion-llm/src/lib.rs` | Re-export `streaming` module |
| `crates/minion-llm/src/types.rs` | Add `VisionMessage`, `StreamEvent` |
| `src-tauri/Cargo.toml` | Add `minion-presentation` dependency |
| `src-tauri/src/lib.rs` | Register 6 new IPC commands + add `presentation_commands` mod |
| `src-tauri/src/state.rs` | Add `PresentationState` to `AppState` |
| `ui/src/App.tsx` | Add `/presentation` route |
| `ui/src/components/Navigation.tsx` (or equivalent sidebar file) | Add Presentations nav item |

---

## Task 1: Workspace + Crate Skeleton

**Files:**
- Create: `crates/minion-presentation/Cargo.toml`
- Create: `crates/minion-presentation/src/lib.rs`
- Create: `crates/minion-presentation/migrations.rs`
- Modify: `Cargo.toml` (root workspace)

- [ ] **Step 1: Add crate to workspace**

In root `Cargo.toml`, add `"crates/minion-presentation"` to the `members` array:

```toml
members = [
    "crates/minion-core",
    "crates/minion-db",
    # ... existing members ...
    "crates/minion-presentation",   # ← add this
    "src-tauri",
]
```

- [ ] **Step 2: Create Cargo.toml**

```toml
# crates/minion-presentation/Cargo.toml
[package]
name = "minion-presentation"
version.workspace = true
authors.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
# Workspace deps
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
uuid = { workspace = true }
chrono = { workspace = true }
thiserror = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
reqwest = { workspace = true }
futures = { workspace = true }

# Local crates
minion-db = { path = "../minion-db" }
minion-llm = { path = "../minion-llm" }
minion-rag = { path = "../minion-rag" }

# Schema validation
jsonschema = "0.18"

# SVG parsing (allowlist sanitizer)
roxmltree = "0.20"

# ZIP for .minion-deck bundle
zip = "2.1"

# Quaternion math
glam = "0.28"   # provides Quat, Vec3, Mat4

# Temporary directories (git sandbox)
tempfile = "3.8"

# Git operations (git sandbox)
git2 = { version = "0.19", default-features = false }

# Document parsing
lopdf = "0.34"               # PDF text extraction
pulldown-cmark = "0.11"      # Markdown parsing
calamine = "0.24"            # XLSX/XLS reading

# OCR
leptess = { version = "0.14", optional = true }

[features]
default = []
ocr = ["leptess"]

[dev-dependencies]
tokio = { workspace = true, features = ["full"] }
```

- [ ] **Step 3: Create lib.rs skeleton**

```rust
// crates/minion-presentation/src/lib.rs
pub mod db;
pub mod migrations;
pub mod schema;

// These modules will be added in later sub-plans:
// pub mod agents;
// pub mod context;
// pub mod export;
// pub mod input;
// pub mod orchestrator;
// pub mod router;
// pub mod security;
// pub mod visual;

pub use schema::types::*;
```

- [ ] **Step 4: Create empty migrations.rs**

```rust
// crates/minion-presentation/migrations.rs
use minion_db::Result;

pub fn run(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(MIGRATIONS)?;
    Ok(())
}

const MIGRATIONS: &str = "
-- placeholder: filled in Task 5
";
```

- [ ] **Step 5: Verify crate compiles**

```bash
cargo build -p minion-presentation
```

Expected: compiles with no errors (empty lib, no logic yet).

- [ ] **Step 6: Commit**

```bash
git add crates/minion-presentation/ Cargo.toml Cargo.lock
git commit -m "feat(presentation): add minion-presentation crate skeleton"
```

---

## Task 2: DeckSchema Core Types

**Files:**
- Create: `crates/minion-presentation/src/schema/mod.rs`
- Create: `crates/minion-presentation/src/schema/types.rs`

- [ ] **Step 1: Write failing test for schema serialization round-trip**

Create `crates/minion-presentation/tests/schema_tests.rs`:

```rust
use minion_presentation::schema::types::*;
use uuid::Uuid;

#[test]
fn deck_serializes_and_deserializes() {
    let deck = Deck {
        meta: DeckMeta {
            title: "Test Deck".into(),
            author: "Test".into(),
            deck_revision: 1,
            schema_version: "1.0".into(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            aspect_ratio: AspectRatio::Ratio16x9,
            language: "en-US".into(),
            text_direction: TextDirection::Ltr,
            target_duration_mins: Some(10),
            presentation_context: PresentationContext::LiveTalk,
        },
        theme: Theme::default(),
        master: MasterSlide { elements: vec![], background: None },
        assets: vec![],
        camera_path: vec![],
        sections: vec![],
        play_order: vec![],
    };
    let json = serde_json::to_string(&deck).expect("serialize");
    let back: Deck = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.meta.title, "Test Deck");
    assert_eq!(back.meta.schema_version, "1.0");
}

#[test]
fn layout_kind_roundtrip() {
    let lk = LayoutKind::Kpi;
    let s = serde_json::to_string(&lk).unwrap();
    assert_eq!(s, r#""kpi""#);
    let back: LayoutKind = serde_json::from_str(&s).unwrap();
    assert_eq!(back, LayoutKind::Kpi);
}

#[test]
fn element_animation_trigger_by_id() {
    let id = ElementId(Uuid::new_v4());
    let trigger = AnimTrigger::AfterElement { element_id: id.clone() };
    let json = serde_json::to_string(&trigger).unwrap();
    let back: AnimTrigger = serde_json::from_str(&json).unwrap();
    match back {
        AnimTrigger::AfterElement { element_id: got } => assert_eq!(got, id),
        _ => panic!("wrong variant"),
    }
}
```

- [ ] **Step 2: Run test to confirm it fails**

```bash
cargo test -p minion-presentation 2>&1 | head -20
```

Expected: compile error — types not defined yet.

- [ ] **Step 3: Create schema/mod.rs**

```rust
// crates/minion-presentation/src/schema/mod.rs
pub mod quaternion;
pub mod types;
pub mod validate;
```

Also update `src/lib.rs`:
```rust
pub mod schema;
pub mod db;
pub mod migrations;
pub use schema::types::*;
```

- [ ] **Step 4: Create schema/types.rs — IDs and enums**

```rust
// crates/minion-presentation/src/schema/types.rs
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

impl DeckId    { pub fn new() -> Self { Self(Uuid::new_v4()) } }
impl SlideId   { pub fn new() -> Self { Self(Uuid::new_v4()) } }
impl SectionId { pub fn new() -> Self { Self(Uuid::new_v4()) } }
impl ElementId { pub fn new() -> Self { Self(Uuid::new_v4()) } }
impl AssetId   { pub fn new() -> Self { Self(Uuid::new_v4()) } }
impl CameraStepId { pub fn new() -> Self { Self(Uuid::new_v4()) } }

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
pub enum TextDirection { Ltr, Rtl }

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
    pub fn rgb(r: u8, g: u8, b: u8) -> Self { Self { r, g, b, a: 255 } }
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
pub enum MotionPreset { Subtle, Balanced, Cinematic, Explosive }

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
                background:  Color::rgb(15, 15, 20),
                surface:     Color::rgb(28, 28, 36),
                primary:     Color::rgb(255, 255, 255),
                secondary:   Color::rgb(160, 160, 180),
                accent:      Color::rgb(99, 102, 241),   // indigo-500
                body_text:   Color::rgb(220, 220, 230),
                muted_text:  Color::rgb(100, 100, 120),
                chart_series: [
                    Color::rgb(99,  102, 241),
                    Color::rgb(236, 72,  153),
                    Color::rgb(34,  197, 94),
                    Color::rgb(251, 191, 36),
                    Color::rgb(249, 115, 22),
                    Color::rgb(20,  184, 166),
                    Color::rgb(168, 85,  247),
                    Color::rgb(239, 68,  68),
                ],
                positive:    Color::rgb(34,  197, 94),
                negative:    Color::rgb(239, 68,  68),
            },
            typography: Typography {
                heading:    FontSpec { family: "Inter".into(), weight: 700, size_scale_base_px: 48.0, line_height: 1.1, letter_spacing: -0.02 },
                subheading: FontSpec { family: "Inter".into(), weight: 500, size_scale_base_px: 28.0, line_height: 1.3, letter_spacing: -0.01 },
                body:       FontSpec { family: "Inter".into(), weight: 400, size_scale_base_px: 18.0, line_height: 1.6, letter_spacing: 0.0  },
                mono:       FontSpec { family: "JetBrains Mono".into(), weight: 400, size_scale_base_px: 16.0, line_height: 1.5, letter_spacing: 0.0 },
                caption:    FontSpec { family: "Inter".into(), weight: 400, size_scale_base_px: 13.0, line_height: 1.4, letter_spacing: 0.01 },
                direction: TextDirection::Ltr,
            },
            motion_preset: MotionPreset::Cinematic,
            font_fallback_stack: vec!["Inter".into(), "Helvetica Neue".into(), "Arial".into(), "sans-serif".into()],
        }
    }
}

// ── Assets ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind { Image, Svg, Font, Video }

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
pub enum ImageFit { Cover, Contain, Fill }

// ── Slide transition ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransitionKind { Zoom, Fly, Morph, Fade, Push, Rotate3d, PortalZoom }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction { Left, Right, Up, Down }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideTransition {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathPoint { pub x: f64, pub y: f64, pub t: f64 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpringParams {
    pub stiffness: f64,   // 1.0 – 2000.0
    pub damping: f64,     // 0.1 – 100.0
    pub mass: f64,        // 0.1 – 10.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimPhase {
    pub effect: AnimEffect,
    pub delay_ms: u32,
    pub duration_ms: u32,
    pub spring: Option<SpringParams>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnimTrigger {
    // Unit variants work fine with internally-tagged serde enums.
    OnSlideEnter,
    OnClick,
    // Tuple variants do NOT work with `#[serde(tag)]` — use struct variants instead.
    AfterElement  { element_id: ElementId },
    WithElement   { element_id: ElementId },
    AutoAfterMs   { ms: u32 },
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
pub enum ElementKind { Text, Image, SvgGraphic, ChartSpec, DiagramDsl, Icon, Video }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ElementContent {
    Text    { markdown: String },
    Image   { asset_id: AssetId, alt: String },
    SvgGraphic { svg_xml: String },
    ChartSpec  { spec_json: serde_json::Value },   // D3 spec
    DiagramDsl { dsl: String, renderer: DiagramRenderer },
    Icon    { name: String, library: String },
    Video   { asset_id: AssetId },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagramRenderer { Mermaid, Graphviz }

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
pub struct SlideTransitionSpec {
    pub kind: TransitionKind,
    pub duration_ms: u32,
    pub easing: String,
    pub direction: Option<Direction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slide {
    pub id: SlideId,
    pub section_id: SectionId,
    pub canvas_x: f64,
    pub canvas_y: f64,
    pub width: f64,
    pub height: f64,
    pub z_layer: i32,
    /// Quaternion: [w, x, y, z]
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
            rotation: [1.0, 0.0, 0.0, 0.0],   // identity quaternion
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

/// Atomic unit of change applied to a Deck.
/// Used by the undo stack, agent streaming, and save_deck_patch IPC command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum DeckPatch {
    SetMeta        { meta: DeckMeta },
    SetTheme       { theme: Theme },
    UpsertSlide    { section_id: SectionId, slide: Slide },
    DeleteSlide    { slide_id: SlideId },
    UpsertElement  { slide_id: SlideId, element: Element },
    DeleteElement  { slide_id: SlideId, element_id: ElementId },
    SetPlayOrder   { order: Vec<SlideId> },
    SetCameraPath  { path: Vec<CameraStep> },
    UpsertAsset    { asset: Asset },
}

// ── Generation config (used by start_presentation_generation IPC) ─────────────

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

// ── Deck summary (for PresentationLibrary list) ───────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeckSummary {
    pub id: DeckId,
    pub title: String,
    pub slide_count: usize,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub thumbnail_data_url: Option<String>,  // base64 PNG
}
```

- [ ] **Step 5: Run failing test to confirm types now compile**

```bash
cargo test -p minion-presentation schema_tests::deck_serializes 2>&1 | tail -10
```

Expected: test passes.

- [ ] **Step 6: Run all schema tests**

```bash
cargo test -p minion-presentation
```

Expected: all 3 tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/minion-presentation/src/schema/ crates/minion-presentation/tests/
git commit -m "feat(presentation): add complete DeckSchema type system"
```

---

## Task 3: Quaternion Math

**Files:**
- Create: `crates/minion-presentation/src/schema/quaternion.rs`

- [ ] **Step 1: Write failing test**

Add to `tests/schema_tests.rs`:

```rust
use minion_presentation::schema::quaternion::*;

#[test]
fn identity_quaternion_gives_zero_euler() {
    let q = [1.0f64, 0.0, 0.0, 0.0];
    let (rx, ry, rz) = quaternion_to_euler_deg(&q);
    assert!((rx).abs() < 0.001, "rx={rx}");
    assert!((ry).abs() < 0.001, "ry={ry}");
    assert!((rz).abs() < 0.001, "rz={rz}");
}

#[test]
fn euler_roundtrip_simple_rotation() {
    let original_deg = (30.0f64, 0.0f64, 45.0f64);
    let q = euler_deg_to_quaternion(original_deg.0, original_deg.1, original_deg.2);
    let (rx, ry, rz) = quaternion_to_euler_deg(&q);
    // Allow 0.1° tolerance for floating-point
    assert!((rx - original_deg.0).abs() < 0.1, "rx expected {} got {}", original_deg.0, rx);
    assert!((ry - original_deg.1).abs() < 0.1, "ry expected {} got {}", original_deg.1, ry);
    assert!((rz - original_deg.2).abs() < 0.1, "rz expected {} got {}", original_deg.2, rz);
}

#[test]
fn quaternion_to_css_transform_identity() {
    let q = [1.0f64, 0.0, 0.0, 0.0];
    let css = quaternion_to_css_rotate3d(&q);
    // identity should produce rotate3d(0,0,1,0deg) or equivalent no-op
    assert!(css.contains("0deg") || css.contains("rotate3d(0,0,1,0"), "got: {css}");
}
```

- [ ] **Step 2: Run test to confirm failure**

```bash
cargo test -p minion-presentation quaternion 2>&1 | tail -5
```

Expected: compile error — module not found.

- [ ] **Step 3: Implement quaternion.rs**

```rust
// crates/minion-presentation/src/schema/quaternion.rs

/// Quaternion represented as [w, x, y, z].
pub type Quat = [f64; 4];

/// Convert Euler angles (degrees, XYZ order) to a unit quaternion [w, x, y, z].
pub fn euler_deg_to_quaternion(rx_deg: f64, ry_deg: f64, rz_deg: f64) -> Quat {
    let rx = rx_deg.to_radians() * 0.5;
    let ry = ry_deg.to_radians() * 0.5;
    let rz = rz_deg.to_radians() * 0.5;

    let (sx, cx) = rx.sin_cos();
    let (sy, cy) = ry.sin_cos();
    let (sz, cz) = rz.sin_cos();

    // XYZ intrinsic rotation order
    [
        cx * cy * cz + sx * sy * sz,  // w
        sx * cy * cz - cx * sy * sz,  // x
        cx * sy * cz + sx * cy * sz,  // y
        cx * cy * sz - sx * sy * cz,  // z
    ]
}

/// Extract Euler angles (degrees, XYZ order) from a quaternion.
/// Returns (rx_deg, ry_deg, rz_deg).
pub fn quaternion_to_euler_deg(q: &Quat) -> (f64, f64, f64) {
    let [w, x, y, z] = *q;

    // Normalise
    let n = (w * w + x * x + y * y + z * z).sqrt();
    let (w, x, y, z) = if n > 1e-10 {
        (w / n, x / n, y / n, z / n)
    } else {
        (1.0, 0.0, 0.0, 0.0)
    };

    // Detect gimbal lock (singularity at pitch ±90°)
    let sin_ry = 2.0 * (w * y - z * x);
    let sin_ry = sin_ry.clamp(-1.0, 1.0);

    let ry = sin_ry.asin();

    let (rx, rz) = if (1.0 - sin_ry * sin_ry).sqrt() > 1e-6 {
        let rx = (2.0 * (w * x + y * z)).atan2(1.0 - 2.0 * (x * x + y * y));
        let rz = (2.0 * (w * z + x * y)).atan2(1.0 - 2.0 * (y * y + z * z));
        (rx, rz)
    } else {
        // Gimbal lock — freeze roll, compute yaw
        let rx = 0.0;
        let rz = (2.0 * (x * z - w * y)).atan2(1.0 - 2.0 * (y * y + z * z));
        (rx, rz)
    };

    (rx.to_degrees(), ry.to_degrees(), rz.to_degrees())
}

/// Produce a CSS `rotate3d(x,y,z,angle)` string from a quaternion.
/// Uses the axis-angle decomposition. Safe for CSS `transform` property.
pub fn quaternion_to_css_rotate3d(q: &Quat) -> String {
    let [w, x, y, z] = *q;

    // Normalise
    let n = (w * w + x * x + y * y + z * z).sqrt();
    let (w, x, y, z) = if n > 1e-10 {
        (w / n, x / n, y / n, z / n)
    } else {
        (1.0, 0.0, 0.0, 0.0)
    };

    let angle_rad = 2.0 * w.clamp(-1.0, 1.0).acos();
    let sin_half = (1.0 - w * w).sqrt();

    if sin_half < 1e-6 {
        // Identity or near-identity — return a no-op
        return "rotate3d(0,0,1,0deg)".into();
    }

    let (ax, ay, az) = (x / sin_half, y / sin_half, z / sin_half);
    let angle_deg = angle_rad.to_degrees();

    format!("rotate3d({:.6},{:.6},{:.6},{:.4}deg)", ax, ay, az, angle_deg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity() {
        let q = euler_deg_to_quaternion(0.0, 0.0, 0.0);
        assert!((q[0] - 1.0).abs() < 1e-10);
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p minion-presentation quaternion
```

Expected: all 3 quaternion tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/minion-presentation/src/schema/quaternion.rs crates/minion-presentation/tests/schema_tests.rs
git commit -m "feat(presentation): add quaternion math for 3D slide rotation"
```

---

## Task 4: Schema Validation

**Files:**
- Create: `crates/minion-presentation/src/schema/validate.rs`

- [ ] **Step 1: Write failing test**

Add to `tests/schema_tests.rs`:

```rust
use minion_presentation::schema::validate::*;

#[test]
fn valid_camera_spring_passes() {
    let step = CameraStep {
        id: CameraStepId::new(),
        target: CameraTarget::Slide { slide_id: SlideId::new() },
        zoom: 1.0,
        duration_ms: 600,
        hold_ms: 200,
        easing: CameraEasing::Spring { stiffness: 300.0, damping: 20.0 },
    };
    assert!(validate_camera_step(&step).is_ok());
}

#[test]
fn invalid_spring_stiffness_fails() {
    let step = CameraStep {
        id: CameraStepId::new(),
        target: CameraTarget::Slide { slide_id: SlideId::new() },
        zoom: 1.0,
        duration_ms: 600,
        hold_ms: 0,
        easing: CameraEasing::Spring { stiffness: 0.0, damping: 20.0 },  // invalid: 0
    };
    let err = validate_camera_step(&step).unwrap_err();
    assert!(err.contains("stiffness"), "got: {err}");
}

#[test]
fn asset_size_limit_enforced() {
    let result = validate_asset_size(26 * 1024 * 1024);  // 26 MB — over 25 MB limit
    assert!(result.is_err());
}

#[test]
fn asset_size_within_limit_passes() {
    let result = validate_asset_size(24 * 1024 * 1024);  // 24 MB — under limit
    assert!(result.is_ok());
}
```

- [ ] **Step 2: Confirm failure**

```bash
cargo test -p minion-presentation validate 2>&1 | tail -5
```

Expected: compile error.

- [ ] **Step 3: Implement validate.rs**

```rust
// crates/minion-presentation/src/schema/validate.rs
use crate::schema::types::{Asset, CameraEasing, CameraStep, SpringParams};

const ASSET_MAX_BYTES: u64 = 25 * 1024 * 1024;       // 25 MB per asset
const DECK_MAX_BYTES: u64  = 200 * 1024 * 1024;      // 200 MB total

pub type ValidationResult = Result<(), String>;

pub fn validate_asset_size(size_bytes: u64) -> ValidationResult {
    if size_bytes > ASSET_MAX_BYTES {
        return Err(format!(
            "asset size {} MB exceeds maximum {} MB",
            size_bytes / 1024 / 1024,
            ASSET_MAX_BYTES / 1024 / 1024
        ));
    }
    Ok(())
}

pub fn validate_deck_total_size(total_bytes: u64) -> ValidationResult {
    if total_bytes > DECK_MAX_BYTES {
        return Err(format!(
            "deck bundle size {} MB exceeds maximum {} MB",
            total_bytes / 1024 / 1024,
            DECK_MAX_BYTES / 1024 / 1024
        ));
    }
    Ok(())
}

pub fn validate_spring(s: &SpringParams) -> ValidationResult {
    if s.stiffness < 1.0 || s.stiffness > 2000.0 {
        return Err(format!("stiffness {} out of range [1.0, 2000.0]", s.stiffness));
    }
    if s.damping < 0.1 || s.damping > 100.0 {
        return Err(format!("damping {} out of range [0.1, 100.0]", s.damping));
    }
    if s.mass < 0.1 || s.mass > 10.0 {
        return Err(format!("mass {} out of range [0.1, 10.0]", s.mass));
    }
    Ok(())
}

pub fn validate_camera_step(step: &CameraStep) -> ValidationResult {
    if step.zoom <= 0.0 {
        return Err(format!("zoom must be positive, got {}", step.zoom));
    }
    if let CameraEasing::Spring { stiffness, damping } = &step.easing {
        let p = SpringParams { stiffness: *stiffness, damping: *damping, mass: 1.0 };
        validate_spring(&p)?;
    }
    Ok(())
}

pub fn validate_asset(asset: &Asset) -> ValidationResult {
    validate_asset_size(asset.size_bytes)?;
    if asset.checksum_sha256.len() != 64 {
        return Err(format!(
            "checksum_sha256 must be 64 hex chars, got {}",
            asset.checksum_sha256.len()
        ));
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p minion-presentation validate
```

Expected: all 4 validation tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/minion-presentation/src/schema/validate.rs crates/minion-presentation/tests/schema_tests.rs
git commit -m "feat(presentation): add schema validation (asset limits, spring bounds)"
```

---

## Task 5: Database Schema + Persistence

**Files:**
- Modify: `crates/minion-presentation/migrations.rs`
- Create: `crates/minion-presentation/src/db.rs`

- [ ] **Step 1: Write failing test**

Add to `tests/schema_tests.rs`:

```rust
use minion_db::Database;
use minion_presentation::{db::PresentationDb, DeckSummary};
use std::path::PathBuf;
use tempfile::tempdir;

fn test_db() -> (Database, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let db = Database::new(&db_path, 2).unwrap();
    minion_presentation::migrations::run(&db.get().unwrap()).unwrap();
    (db, dir)
}

#[test]
fn insert_and_list_presentation() {
    let (db, _dir) = test_db();
    let pdb = PresentationDb::new(db);

    let id = DeckId::new();
    pdb.insert_presentation(
        &id,
        "My Deck",
        "/tmp/test.minion-deck",
        None,
    ).unwrap();

    let list = pdb.list_presentations().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].title, "My Deck");
}

#[test]
fn delete_presentation_removes_it() {
    let (db, _dir) = test_db();
    let pdb = PresentationDb::new(db);
    let id = DeckId::new();
    pdb.insert_presentation(&id, "Temp", "/tmp/x.minion-deck", None).unwrap();
    pdb.delete_presentation(&id).unwrap();
    assert!(pdb.list_presentations().unwrap().is_empty());
}
```

- [ ] **Step 2: Confirm failure**

```bash
cargo test -p minion-presentation db 2>&1 | tail -5
```

Expected: compile error.

- [ ] **Step 3: Fill migrations.rs**

```rust
// crates/minion-presentation/migrations.rs
use minion_db::Result;

pub fn run(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute_batch(MIGRATIONS).map_err(|e| minion_db::Error::Migration(e.to_string()))?;
    Ok(())
}

const MIGRATIONS: &str = "
CREATE TABLE IF NOT EXISTS presentations (
    id             TEXT PRIMARY KEY,
    title          TEXT NOT NULL,
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL,
    bundle_path    TEXT NOT NULL,
    thumbnail      BLOB,
    schema_version TEXT NOT NULL DEFAULT '1.0'
);

CREATE TABLE IF NOT EXISTS generation_sessions (
    id               TEXT PRIMARY KEY,
    presentation_id  TEXT REFERENCES presentations(id) ON DELETE CASCADE,
    status           TEXT NOT NULL CHECK(status IN ('running','completed','failed','interrupted')),
    started_at       INTEGER NOT NULL,
    completed_at     INTEGER,
    last_checkpoint  TEXT,
    error            TEXT
);

CREATE TABLE IF NOT EXISTS slide_results (
    session_id   TEXT REFERENCES generation_sessions(id) ON DELETE CASCADE,
    slide_index  INTEGER NOT NULL,
    slide_id     TEXT NOT NULL,
    status       TEXT NOT NULL CHECK(status IN ('pending','completed','failed')),
    deck_patch   TEXT,
    PRIMARY KEY (session_id, slide_index)
);

CREATE TABLE IF NOT EXISTS user_assets (
    id          TEXT PRIMARY KEY,
    filename    TEXT NOT NULL,
    kind        TEXT NOT NULL CHECK(kind IN ('image','svg','font','video')),
    checksum    TEXT NOT NULL,
    size_bytes  INTEGER NOT NULL,
    stored_at   TEXT NOT NULL,
    created_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_presentations_updated ON presentations(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_sessions_presentation ON generation_sessions(presentation_id);
";
```

- [ ] **Step 4: Create db.rs**

```rust
// crates/minion-presentation/src/db.rs
use crate::schema::types::{DeckId, DeckSummary};
use chrono::Utc;
use minion_db::{Database, Error, Result};

#[derive(Clone)]
pub struct PresentationDb {
    db: Database,
}

impl PresentationDb {
    pub fn new(db: Database) -> Self { Self { db } }

    pub fn insert_presentation(
        &self,
        id: &DeckId,
        title: &str,
        bundle_path: &str,
        thumbnail: Option<Vec<u8>>,
    ) -> Result<()> {
        let conn = self.db.get()?;
        let now = Utc::now().timestamp();
        conn.execute(
            "INSERT INTO presentations (id, title, created_at, updated_at, bundle_path, thumbnail)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                id.0.to_string(), title, now, now, bundle_path, thumbnail
            ],
        )?;
        Ok(())
    }

    pub fn update_presentation_title(&self, id: &DeckId, title: &str) -> Result<()> {
        let conn = self.db.get()?;
        let now = Utc::now().timestamp();
        conn.execute(
            "UPDATE presentations SET title = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![title, now, id.0.to_string()],
        )?;
        Ok(())
    }

    pub fn update_thumbnail(&self, id: &DeckId, thumbnail: Vec<u8>) -> Result<()> {
        let conn = self.db.get()?;
        let now = Utc::now().timestamp();
        conn.execute(
            "UPDATE presentations SET thumbnail = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![thumbnail, now, id.0.to_string()],
        )?;
        Ok(())
    }

    pub fn delete_presentation(&self, id: &DeckId) -> Result<()> {
        let conn = self.db.get()?;
        conn.execute(
            "DELETE FROM presentations WHERE id = ?1",
            rusqlite::params![id.0.to_string()],
        )?;
        Ok(())
    }

    pub fn list_presentations(&self) -> Result<Vec<DeckSummary>> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, created_at, updated_at, thumbnail
             FROM presentations ORDER BY updated_at DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            let id_str: String = row.get(0)?;
            let title: String = row.get(1)?;
            let created_ts: i64 = row.get(2)?;
            let updated_ts: i64 = row.get(3)?;
            let thumbnail: Option<Vec<u8>> = row.get(4)?;
            Ok((id_str, title, created_ts, updated_ts, thumbnail))
        })?;

        let mut summaries = Vec::new();
        for row in rows {
            let (id_str, title, created_ts, updated_ts, thumb) = row?;
            let id = uuid::Uuid::parse_str(&id_str)
                .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
            summaries.push(DeckSummary {
                id: DeckId(id),
                title,
                slide_count: 0,   // loaded lazily from the bundle file
                // DateTime<Utc> does not impl Default — use Utc::now() as fallback
                created_at: chrono::DateTime::from_timestamp(created_ts, 0)
                    .unwrap_or_else(chrono::Utc::now),
                updated_at: chrono::DateTime::from_timestamp(updated_ts, 0)
                    .unwrap_or_else(chrono::Utc::now),
                thumbnail_data_url: thumb.map(|b| {
                    use base64ct::{Base64, Encoding};
                    format!("data:image/png;base64,{}", Base64::encode_string(&b))
                }),
            });
        }
        Ok(summaries)
    }

    pub fn get_bundle_path(&self, id: &DeckId) -> Result<Option<String>> {
        let conn = self.db.get()?;
        let mut stmt = conn.prepare(
            "SELECT bundle_path FROM presentations WHERE id = ?1"
        )?;
        let mut rows = stmt.query_map(
            rusqlite::params![id.0.to_string()],
            |row| row.get::<_, String>(0),
        )?;
        Ok(rows.next().transpose()?)
    }
}

// base64 encoding uses the base64ct workspace crate (already in Cargo.toml).
// Add to Cargo.toml [dependencies]: base64ct = { workspace = true, features = ["alloc"] }
```

- [ ] **Step 5: Run DB tests**

```bash
cargo test -p minion-presentation db
```

Expected: both DB tests pass.

- [ ] **Step 6: Run all tests so far**

```bash
cargo test -p minion-presentation
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/minion-presentation/src/db.rs crates/minion-presentation/migrations.rs crates/minion-presentation/tests/schema_tests.rs
git commit -m "feat(presentation): add DB migrations and PresentationDb persistence layer"
```

---

## Task 6: LLM Streaming + Vision Types

**Files:**
- Create: `crates/minion-llm/src/streaming.rs`
- Modify: `crates/minion-llm/src/types.rs`
- Modify: `crates/minion-llm/src/lib.rs`

The existing `OpenAICompatibleProvider` sends `"stream": false`. For the presentation agent pipeline, agents need token-by-token streaming so the frontend can display live output. We also need to pass images to vision models (for OCR and SVG generation review).

- [ ] **Step 1: Write failing test**

Create `crates/minion-llm/tests/streaming_tests.rs`:

```rust
use minion_llm::streaming::{StreamEvent, parse_sse_line};

#[test]
fn parses_data_line() {
    let line = r#"data: {"id":"x","choices":[{"delta":{"content":"Hello"}}]}"#;
    let event = parse_sse_line(line);
    assert!(matches!(event, Some(StreamEvent::Token(t)) if t == "Hello"));
}

#[test]
fn parses_done_sentinel() {
    let line = "data: [DONE]";
    let event = parse_sse_line(line);
    assert!(matches!(event, Some(StreamEvent::Done)));
}

#[test]
fn ignores_comment_lines() {
    let line = ": keep-alive";
    let event = parse_sse_line(line);
    assert!(event.is_none());
}
```

- [ ] **Step 2: Confirm failure**

```bash
cargo test -p minion-llm streaming 2>&1 | tail -5
```

Expected: compile error.

- [ ] **Step 3: Add VisionMessage to types.rs**

In `crates/minion-llm/src/types.rs`, add after the existing `ChatMessage` definition:

```rust
/// Image content for vision-capable models.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageContent {
    /// base64-encoded image with MIME type, e.g. "data:image/png;base64,..."
    ImageUrl { image_url: ImageUrl },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,   // "data:image/png;base64,..." or https URL
}

/// A message that may contain text + images (for vision models).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionMessage {
    pub role: ChatRole,
    pub content: Vec<VisionContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VisionContent {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
}
```

- [ ] **Step 4: Create streaming.rs**

```rust
// crates/minion-llm/src/streaming.rs
//! SSE (Server-Sent Events) stream parsing for OpenAI-compatible streaming responses.

use futures::StreamExt;
use reqwest::Response;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub enum StreamEvent {
    Token(String),
    Done,
    Error(String),
}

#[derive(Deserialize)]
struct SseDelta {
    content: Option<String>,
}

#[derive(Deserialize)]
struct SseChoice {
    delta: SseDelta,
}

#[derive(Deserialize)]
struct SseChunk {
    choices: Vec<SseChoice>,
}

/// Parse a single SSE line into a `StreamEvent`.
/// Returns `None` for keep-alive comments and empty lines.
pub fn parse_sse_line(line: &str) -> Option<StreamEvent> {
    let line = line.trim();

    if line.is_empty() || line.starts_with(':') {
        return None;
    }

    let data = line.strip_prefix("data: ")?;

    if data == "[DONE]" {
        return Some(StreamEvent::Done);
    }

    match serde_json::from_str::<SseChunk>(data) {
        Ok(chunk) => {
            let token = chunk
                .choices
                .into_iter()
                .next()
                .and_then(|c| c.delta.content)
                .unwrap_or_default();
            if token.is_empty() {
                None
            } else {
                Some(StreamEvent::Token(token))
            }
        }
        Err(e) => Some(StreamEvent::Error(format!("SSE parse error: {e}"))),
    }
}

/// Collect a streaming OpenAI-compatible response into a full string.
/// Calls `on_token` for each incremental token (for live display).
pub async fn collect_stream<F>(
    response: Response,
    mut on_token: F,
) -> Result<String, String>
where
    F: FnMut(&str),
{
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut full_text = String::new();

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| e.to_string())?;
        let text = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
        buffer.push_str(text);

        // SSE lines are separated by '\n'. Process complete lines.
        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].to_string();
            buffer.drain(..=pos);

            if let Some(event) = parse_sse_line(&line) {
                match event {
                    StreamEvent::Token(t) => {
                        on_token(&t);
                        full_text.push_str(&t);
                    }
                    StreamEvent::Done => return Ok(full_text),
                    StreamEvent::Error(e) => return Err(e),
                }
            }
        }
    }
    Ok(full_text)
}
```

- [ ] **Step 5: Export from lib.rs**

In `crates/minion-llm/src/lib.rs`, add:

```rust
pub mod streaming;
pub use streaming::{StreamEvent, parse_sse_line, collect_stream};
```

- [ ] **Step 6: Run streaming tests**

```bash
cargo test -p minion-llm streaming
```

Expected: all 3 tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/minion-llm/src/streaming.rs crates/minion-llm/src/types.rs crates/minion-llm/src/lib.rs crates/minion-llm/tests/
git commit -m "feat(llm): add SSE streaming parser and vision message types"
```

---

## Task 7: Tauri IPC Commands Scaffold

**Files:**
- Create: `src-tauri/src/presentation_commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Add minion-presentation to src-tauri/Cargo.toml**

In `src-tauri/Cargo.toml`, add to `[dependencies]`:

```toml
minion-presentation = { path = "../crates/minion-presentation" }
```

- [ ] **Step 2: Add PresentationState to AppState**

In `src-tauri/src/state.rs`, add the import and field. Find the `AppState` struct and add:

```rust
// At the top of the file, add:
use minion_presentation::db::PresentationDb;

// Inside AppState struct, add the field:
pub presentation_db: PresentationDb,
```

In `AppState::new()` (or wherever the struct is initialized), add:

```rust
let presentation_db = PresentationDb::new(db.clone());
// Run migrations on startup:
{
    let conn = db.get().map_err(|e| e.to_string())?;
    minion_presentation::migrations::run(&conn).map_err(|e| e.to_string())?;
}
```

And include `presentation_db` in the struct initialization.

- [ ] **Step 3: Create presentation_commands.rs**

```rust
// src-tauri/src/presentation_commands.rs
//! Tauri IPC commands for the Presentation module.
//! AI orchestration is stubbed here — filled in by the AI Pipeline sub-plan.

use minion_presentation::schema::types::{
    DeckId, DeckPatch, DeckSummary, GenerationConfig,
};
use std::sync::Arc;
use tauri::{AppHandle, State};
use tokio::sync::RwLock;

use crate::state::AppState;

type AppStateHandle = State<'_, Arc<RwLock<AppState>>>;

/// Start AI generation for a new presentation.
/// Returns a session ID that the frontend uses to track progress via events.
/// The actual agent pipeline is wired up in the AI Pipeline sub-plan;
/// this stub creates a session record and returns its ID.
#[tauri::command]
pub async fn start_presentation_generation(
    _inputs: serde_json::Value,         // Vec<InputSource> — typed in AI Pipeline sub-plan
    _config: GenerationConfig,
    state: AppStateHandle,
    _app: AppHandle,
) -> Result<String, String> {
    let _guard = state.read().await;
    // Stub: return a placeholder session ID
    Ok(uuid::Uuid::new_v4().to_string())
}

/// Interrupt a running generation and redirect from a specific agent onwards.
#[tauri::command]
pub async fn interrupt_generation(
    _session_id: String,
    _after_agent: String,
    _instruction: String,
    state: AppStateHandle,
) -> Result<(), String> {
    let _guard = state.read().await;
    Ok(())   // stub
}

/// Load a full deck by ID (reads from .minion-deck bundle on disk).
#[tauri::command]
pub async fn get_deck(
    _id: String,
    state: AppStateHandle,
) -> Result<serde_json::Value, String> {
    let _guard = state.read().await;
    Err("not yet implemented — filled in AI Pipeline sub-plan".into())
}

/// Apply an array of DeckPatch operations to a persisted deck.
#[tauri::command]
pub async fn save_deck_patch(
    _id: String,
    _patches: Vec<DeckPatch>,
    state: AppStateHandle,
) -> Result<(), String> {
    let _guard = state.read().await;
    Ok(())   // stub
}

/// List all presentations for the library view.
#[tauri::command]
pub async fn list_presentations(
    state: AppStateHandle,
) -> Result<Vec<DeckSummary>, String> {
    let guard = state.read().await;
    guard
        .presentation_db
        .list_presentations()
        .map_err(|e| e.to_string())
}

/// Export a deck to the specified format and path.
#[tauri::command]
pub async fn export_presentation(
    _id: String,
    _format: String,    // "pptx" | "pdf" | "html" | "speaker_notes_pdf"
    _output_path: String,
    state: AppStateHandle,
) -> Result<serde_json::Value, String> {
    let _guard = state.read().await;
    Err("export not yet implemented — filled in Export sub-plan".into())
}
```

- [ ] **Step 4: Register commands in lib.rs**

In `src-tauri/src/lib.rs`, add the module and commands:

```rust
// Add module declaration near the other mod declarations:
mod presentation_commands;

// Inside the invoke_handler list, add:
presentation_commands::start_presentation_generation,
presentation_commands::interrupt_generation,
presentation_commands::get_deck,
presentation_commands::save_deck_patch,
presentation_commands::list_presentations,
presentation_commands::export_presentation,
```

- [ ] **Step 5: Build to confirm no compile errors**

```bash
cargo build -p minion-app 2>&1 | grep -E "^error" | head -20
# Also build the Tauri binary:
cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "^error" | head -20
```

Expected: compiles cleanly. Warnings about unused variables in stubs are acceptable.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/presentation_commands.rs src-tauri/src/lib.rs src-tauri/src/state.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(tauri): scaffold 6 presentation IPC commands"
```

---

## Task 8: TypeScript Schema Types + API Layer

**Files:**
- Create: `ui/src/lib/deck-schema.ts`
- Create: `ui/src/lib/deck-patch.ts`
- Create: `ui/src/lib/presentation-api.ts`

- [ ] **Step 1: Create deck-schema.ts**

```typescript
// ui/src/lib/deck-schema.ts
// TypeScript mirror of crates/minion-presentation/src/schema/types.rs
// Kept in sync manually. Types match serde_json output exactly.

// ── IDs ──────────────────────────────────────────────────────────────────────
export type DeckId    = string;  // uuid
export type SlideId   = string;
export type SectionId = string;
export type ElementId = string;
export type AssetId   = string;
export type CameraStepId = string;

// ── Enums ─────────────────────────────────────────────────────────────────────
export type AspectRatio =
  | "ratio16x9"
  | "ratio4x3"
  | "a4_portrait"
  | "a4_landscape"
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

// ── Color ─────────────────────────────────────────────────────────────────────
export interface Color { r: number; g: number; b: number; a: number }

export function colorToCss(c: Color): string {
  return `rgba(${c.r},${c.g},${c.b},${(c.a / 255).toFixed(3)})`;
}

// ── Theme ─────────────────────────────────────────────────────────────────────
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

// ── Assets ────────────────────────────────────────────────────────────────────
export type AssetKind = "image" | "svg" | "font" | "video";
export type AssetStorage =
  | { kind: "bundled_file"; relative_path: string }
  | { kind: "external_url"; url: string };

export interface Asset {
  id: AssetId; kind: AssetKind; filename: string;
  checksum_sha256: string; size_bytes: number; storage: AssetStorage;
}

// ── Camera path ───────────────────────────────────────────────────────────────
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

// ── Background ────────────────────────────────────────────────────────────────
export type Background =
  | { kind: "solid"; color: Color }
  | { kind: "gradient"; from: Color; to: Color; angle_deg: number }
  | { kind: "image"; asset_id: AssetId; fit: ImageFit }
  | { kind: "svg_pattern"; asset_id: AssetId };

// ── Animations ────────────────────────────────────────────────────────────────
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

// ── Elements ──────────────────────────────────────────────────────────────────
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

// ── Slide + Section ───────────────────────────────────────────────────────────
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
  z_layer: number; rotation: [number, number, number, number];  // [w,x,y,z]
  layout: LayoutKind; background: Background;
  transition: SlideTransitionSpec;
  elements: Element[]; speaker_notes: SpeakerNotes;
  auto_advance_ms?: number; user_locked: boolean;
}

export interface Section { id: SectionId; title: string; slides: Slide[] }

// ── MasterSlide ───────────────────────────────────────────────────────────────
export interface MasterElement { element: Element; exclude_slide_ids: SlideId[] }
export interface MasterSlide { elements: MasterElement[]; background?: Background }

// ── DeckMeta ──────────────────────────────────────────────────────────────────
export interface DeckMeta {
  title: string; author: string; deck_revision: number; schema_version: string;
  created_at: string; updated_at: string; aspect_ratio: AspectRatio;
  language: string; text_direction: TextDirection;
  target_duration_mins?: number; presentation_context: PresentationContext;
}

// ── Deck ──────────────────────────────────────────────────────────────────────
export interface Deck {
  meta: DeckMeta; theme: Theme; master: MasterSlide;
  assets: Asset[]; camera_path: CameraStep[];
  sections: Section[]; play_order: SlideId[];
}

// ── Helpers ───────────────────────────────────────────────────────────────────
export function allSlides(deck: Deck): Slide[] {
  return deck.sections.flatMap(s => s.slides);
}

export function slideById(deck: Deck, id: SlideId): Slide | undefined {
  return allSlides(deck).find(s => s.id === id);
}

export function slideCount(deck: Deck): number {
  return deck.sections.reduce((n, s) => n + s.slides.length, 0);
}

// ── GenerationConfig (sent to start_presentation_generation) ─────────────────
export interface GenerationConfig {
  theme_name?: string; audience: string; tone: string;
  language: string; target_duration_mins?: number;
  slide_count_hint?: number; presentation_context: PresentationContext;
}

// ── DeckSummary (from list_presentations) ────────────────────────────────────
export interface DeckSummary {
  id: DeckId; title: string; slide_count: number;
  created_at: string; updated_at: string; thumbnail_data_url?: string;
}
```

- [ ] **Step 2: Create deck-patch.ts**

```typescript
// ui/src/lib/deck-patch.ts
import type { Deck, DeckMeta, Theme, Section, Slide, Element, SlideId,
  SectionId, ElementId, CameraStep, Asset, DeckPatch } from "./deck-schema";

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

/** Apply a single patch to a Deck (immutably — returns a new Deck). */
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
          return { ...slide, elements: slide.elements.filter(e => e.id !== patch.element_id) };
        }),
      }));
      return { ...deck, sections };
    }
  }
}

/** Apply an ordered list of patches to a Deck. */
export function applyPatches(deck: Deck, patches: DeckPatch[]): Deck {
  return patches.reduce(applyPatch, deck);
}
```

- [ ] **Step 3: Create presentation-api.ts**

```typescript
// ui/src/lib/presentation-api.ts
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { DeckSummary, Deck, GenerationConfig } from "./deck-schema";
import type { DeckPatch } from "./deck-patch";

// ── Deck library ──────────────────────────────────────────────────────────────
export async function listPresentations(): Promise<DeckSummary[]> {
  return invoke<DeckSummary[]>("list_presentations");
}

export async function getDeck(id: string): Promise<Deck> {
  return invoke<Deck>("get_deck", { id });
}

export async function saveDeckPatch(id: string, patches: DeckPatch[]): Promise<void> {
  return invoke<void>("save_deck_patch", { id, patches });
}

// ── Generation ────────────────────────────────────────────────────────────────

export interface InputSource {
  kind: "text" | "file_path" | "url" | "git_url";
  content: string;   // text content, absolute file path, URL, or git remote URL
}

export async function startGeneration(
  inputs: InputSource[],
  config: GenerationConfig,
): Promise<string> {
  return invoke<string>("start_presentation_generation", { inputs, config });
}

export async function interruptGeneration(
  sessionId: string,
  afterAgent: string,
  instruction: string,
): Promise<void> {
  return invoke<void>("interrupt_generation", {
    sessionId,
    afterAgent,
    instruction,
  });
}

// ── Export ────────────────────────────────────────────────────────────────────
export type ExportFormat = "pptx" | "pdf" | "html" | "speaker_notes_pdf";

export async function exportPresentation(
  id: string,
  format: ExportFormat,
  outputPath: string,
): Promise<{ file_size_bytes: number; path: string }> {
  return invoke("export_presentation", { id, format, outputPath });
}

// ── Agent event streaming ─────────────────────────────────────────────────────
export type AgentName =
  | "research" | "storyteller" | "slide_planner" | "visual" | "design_critic";

export type AgentEvent =
  | { seq: number; agent: AgentName; kind: "started" }
  | { seq: number; agent: AgentName; kind: "progress"; data: string }
  | { seq: number; agent: AgentName; kind: "slide_ready"; slide_index: number; patch: DeckPatch }
  | { seq: number; agent: AgentName; kind: "completed" }
  | { seq: number; agent: AgentName; kind: "error"; message: string; recoverable: boolean }
  | { seq: number; kind: "stream_complete"; deck_id: string }
  | { seq: number; kind: "stream_error"; message: string };

export function listenToAgentEvents(
  sessionId: string,
  onEvent: (e: AgentEvent) => void,
): Promise<UnlistenFn> {
  return listen<AgentEvent>(`presentation://agent-event/${sessionId}`, e => {
    onEvent(e.payload);
  });
}
```

- [ ] **Step 4: Run TypeScript type check**

```bash
cd ui && pnpm typecheck 2>&1 | grep -E "error TS" | head -20
```

Expected: no errors in the three new files. (Other pre-existing errors are acceptable — only check new files.)

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/deck-schema.ts ui/src/lib/deck-patch.ts ui/src/lib/presentation-api.ts
git commit -m "feat(ui): add TypeScript DeckSchema types, patch system, and API wrappers"
```

---

## Task 9: SolidJS Page Shell + Navigation

**Files:**
- Create: `ui/src/pages/Presentation.tsx`
- Create: `ui/src/pages/presentation/PresentationLibrary.tsx`
- Modify: `ui/src/App.tsx` (add route)
- Modify: navigation sidebar file (add nav item)

- [ ] **Step 1: Find the navigation and routing files**

```bash
grep -rn "Dashboard\|Finance\|/finance\|route" ui/src/App.tsx ui/src/components/ 2>/dev/null | head -20
```

Use the output to identify the exact component file that contains route definitions and the sidebar nav component.

- [ ] **Step 2: Create PresentationLibrary.tsx (empty state)**

```tsx
// ui/src/pages/presentation/PresentationLibrary.tsx
import { createSignal, onMount, For, Show } from "solid-js";
import { listPresentations } from "../../lib/presentation-api";
import type { DeckSummary } from "../../lib/deck-schema";

interface Props {
  onOpenDeck: (id: string) => void;
  onNewDeck: () => void;
}

export default function PresentationLibrary(props: Props) {
  const [decks, setDecks] = createSignal<DeckSummary[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [error, setError] = createSignal<string | null>(null);

  onMount(async () => {
    try {
      const list = await listPresentations();
      setDecks(list);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  });

  return (
    <div class="flex flex-col h-full bg-[#0f0f14] text-white p-8">
      {/* Header */}
      <div class="flex items-center justify-between mb-8">
        <div>
          <h1 class="text-3xl font-bold tracking-tight">Presentations</h1>
          <p class="text-gray-400 mt-1 text-sm">AI-generated cinematic decks</p>
        </div>
        <button
          onClick={props.onNewDeck}
          class="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 rounded-lg text-sm font-medium transition-colors"
        >
          + New Presentation
        </button>
      </div>

      {/* States */}
      <Show when={loading()}>
        <div class="flex items-center justify-center flex-1 text-gray-500">
          Loading...
        </div>
      </Show>

      <Show when={error()}>
        <div class="text-red-400 text-sm">{error()}</div>
      </Show>

      <Show when={!loading() && decks().length === 0}>
        <div class="flex flex-col items-center justify-center flex-1 gap-4 text-center">
          <div class="text-6xl">🎞</div>
          <h2 class="text-xl font-medium text-gray-300">No presentations yet</h2>
          <p class="text-gray-500 text-sm max-w-sm">
            Paste your notes, upload a document, or drop in a URL and let AI build your deck.
          </p>
          <button
            onClick={props.onNewDeck}
            class="px-6 py-3 bg-indigo-600 hover:bg-indigo-500 rounded-xl text-sm font-medium transition-colors mt-2"
          >
            Create your first presentation
          </button>
        </div>
      </Show>

      <Show when={!loading() && decks().length > 0}>
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
          <For each={decks()}>
            {(deck) => (
              <button
                onClick={() => props.onOpenDeck(deck.id)}
                class="group text-left bg-[#1c1c24] hover:bg-[#252530] border border-[#2a2a36] hover:border-indigo-500/40 rounded-xl overflow-hidden transition-all"
              >
                {/* Thumbnail */}
                <div class="aspect-video bg-[#0f0f14] flex items-center justify-center border-b border-[#2a2a36]">
                  <Show
                    when={deck.thumbnail_data_url}
                    fallback={<span class="text-4xl opacity-20">▶</span>}
                  >
                    <img
                      src={deck.thumbnail_data_url!}
                      alt={deck.title}
                      class="w-full h-full object-cover"
                    />
                  </Show>
                </div>
                {/* Info */}
                <div class="p-3">
                  <p class="font-medium text-sm truncate">{deck.title}</p>
                  <p class="text-xs text-gray-500 mt-0.5">
                    {deck.slide_count} slides ·{" "}
                    {new Date(deck.updated_at).toLocaleDateString()}
                  </p>
                </div>
              </button>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
```

- [ ] **Step 3: Create Presentation.tsx page shell**

```tsx
// ui/src/pages/Presentation.tsx
import { createSignal, Switch, Match } from "solid-js";
import PresentationLibrary from "./presentation/PresentationLibrary";

type View = "library" | "studio" | "workspace";

export default function PresentationPage() {
  const [view, setView] = createSignal<View>("library");
  const [activeDeckId, setActiveDeckId] = createSignal<string | null>(null);

  return (
    <div class="h-full w-full">
      <Switch>
        <Match when={view() === "library"}>
          <PresentationLibrary
            onOpenDeck={(id) => {
              setActiveDeckId(id);
              setView("workspace");
            }}
            onNewDeck={() => setView("studio")}
          />
        </Match>
        <Match when={view() === "studio"}>
          {/* CreationStudio — added in Frontend sub-plan */}
          <div class="flex items-center justify-center h-full text-gray-500 bg-[#0f0f14]">
            <div class="text-center">
              <p class="text-lg font-medium text-white mb-2">Creation Studio</p>
              <p class="text-sm">Coming in Frontend sub-plan</p>
              <button
                onClick={() => setView("library")}
                class="mt-4 px-4 py-2 bg-[#1c1c24] rounded-lg text-sm"
              >
                ← Back
              </button>
            </div>
          </div>
        </Match>
        <Match when={view() === "workspace"}>
          {/* DeckWorkspace — added in Frontend sub-plan */}
          <div class="flex items-center justify-center h-full text-gray-500 bg-[#0f0f14]">
            <div class="text-center">
              <p class="text-lg font-medium text-white mb-2">
                Deck: {activeDeckId()}
              </p>
              <p class="text-sm">Workspace coming in Frontend sub-plan</p>
              <button
                onClick={() => setView("library")}
                class="mt-4 px-4 py-2 bg-[#1c1c24] rounded-lg text-sm"
              >
                ← Back to Library
              </button>
            </div>
          </div>
        </Match>
      </Switch>
    </div>
  );
}
```

- [ ] **Step 4: Add route and nav item**

Using the file paths discovered in Step 1, add:

In `App.tsx` (route config):
```tsx
// Add alongside existing routes:
<Route path="/presentation" component={PresentationPage} />
// Import at top:
import PresentationPage from "./pages/Presentation";
```

In the navigation sidebar component, add a Presentations item. Follow exactly the same pattern used by an existing nav item (e.g., Finance or Reader). The `href` should be `"/presentation"`. The icon can be a `▶` or a presentation icon — use whatever icon system the existing nav items use.

- [ ] **Step 5: Run dev server and verify page loads**

```bash
cd ui && pnpm dev
```

Open `http://localhost:5173`, navigate to Presentations in the sidebar. Expected: the library page renders with "No presentations yet" empty state and a "Create your first presentation" button.

- [ ] **Step 6: Run typecheck**

```bash
cd ui && pnpm typecheck
```

Expected: no errors in the new files.

- [ ] **Step 7: Commit**

```bash
git add ui/src/pages/Presentation.tsx ui/src/pages/presentation/ ui/src/App.tsx ui/src/components/
git commit -m "feat(ui): add Presentation page shell and empty PresentationLibrary"
```

---

## Task 10: End-to-End Foundation Smoke Test

This task verifies the entire foundation layer works together: crate builds, IPC connects, UI page loads and calls a real IPC command.

- [ ] **Step 1: Run full workspace build**

```bash
cargo build --workspace 2>&1 | grep -E "^error" | head -20
```

Expected: zero errors.

- [ ] **Step 2: Run all Rust tests**

```bash
cargo test --workspace 2>&1 | tail -20
```

Expected: all pre-existing tests still pass, new `minion-presentation` tests pass.

- [ ] **Step 3: Run full Tauri dev build**

```bash
cargo tauri dev 2>&1 | head -30
```

Expected: app launches without panic. Navigate to Presentations — page renders, "list_presentations" IPC call succeeds (returns empty array), empty state shows.

- [ ] **Step 4: Run UI type check and lint**

```bash
cd ui && pnpm typecheck && pnpm lint
```

Expected: no type errors or lint errors in the new files.

- [ ] **Step 5: Final commit**

```bash
git add -p  # review any remaining changes
git commit -m "test(presentation): foundation smoke test — all layers verified"
```

---

## Foundation Sub-Plan Complete

At this point the system has:
- `minion-presentation` crate with full DeckSchema types, validation, quaternion math, and DB persistence
- 4 SQLite tables for presentations, sessions, slide results, and user assets
- `minion-llm` extended with SSE streaming and vision message types
- 6 Tauri IPC commands registered (5 stubbed, 1 fully working: `list_presentations`)
- TypeScript `DeckSchema` types, `DeckPatch` system, and typed API wrappers
- SolidJS `Presentation` page with working library that queries the real IPC

**Next sub-plan:** `2026-05-14-presentation-ai-pipeline.md` — ResearchAgent, StorytellerAgent, SlidePlannerAgent, VisualAgent, DesignCriticAgent, LLM router, input processing, SVG sanitizer, and full orchestrator with streaming.
