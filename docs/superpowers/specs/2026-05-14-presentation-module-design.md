# Presentation Module — Design Spec
**Date:** 2026-05-14  
**Status:** Draft — pending user approval  
**Scope:** New `minion-presentation` module integrated into the existing Tauri/Rust/SolidJS minion desktop app

---

## 1. Overview

A new module that accepts any combination of text, documents, images, URLs, and git repos and produces a cinematic, spatially-navigable presentation deck — with live AI agent streaming, a slide editor, full presentation playback, and one-click PPTX/PDF export.

The end-user never touches HTML, CSS, SVG, or animation code. The AI pipeline handles all production concerns. The user's only jobs are: provide content, pick a theme, review and optionally edit, then export.

---

## 2. Integration Into Minion

| Layer | Location |
|---|---|
| Rust crate | `crates/minion-presentation/` |
| Tauri IPC commands | `src-tauri/src/commands.rs` (6 new commands) |
| DB migrations | `crates/minion-presentation/migrations.rs` |
| SolidJS page | `ui/src/pages/Presentation.tsx` |
| TypeScript schema types | `ui/src/lib/deck-schema.ts` |
| Asset storage | `~/.local/share/minion/presentations/` (per-deck bundle dirs) |

Reuses: `minion-llm` (extended), `minion-db`, `minion-rag`, `minion-search`, existing credential vault for API keys.

---

## 3. DeckSchema — The Central Contract

Version: `schema_version: "1.0"`. All agents produce it; the renderer, editor, and export engine consume it. Validated after every agent step using the `jsonschema` crate.

### 3.1 Coordinate System

- **Canvas:** 50,000 × 50,000 logical units (lu). One lu = one CSS pixel at zoom level 1.0x.
- **Base slide:** 1,920 × 1,080 lu. Slides may be any size.
- **Origin:** (0, 0) top-left of canvas.
- **Zoom:** renderer enforces min 0.05x, max 20.0x. Beyond these bounds floating-point precision degrades.
- **Slide overlap:** allowed. Rendering order is determined by `z_layer: i32` on each slide (higher = in front). Default z_layer assigned sequentially by the planner so slides do not overlap unless the AI or user explicitly intends it.

### 3.2 Top-Level Deck Structure

```
Deck
├── meta: DeckMeta
├── theme: Theme
├── master: MasterSlide
├── assets: [Asset]
├── camera_path: [CameraStep]
├── sections: [Section]
│   └── slides: [Slide]
└── play_order: [SlideId]          ← explicit ordered list, separate from spatial position
```

### 3.3 DeckMeta

```rust
struct DeckMeta {
    title: String,
    author: String,
    deck_revision: u32,             // increments on every save; distinct from schema_version
    schema_version: String,         // "1.0" — the DeckSchema format version
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    aspect_ratio: AspectRatio,      // Ratio16x9 | Ratio4x3 | A4Portrait | A4Landscape | Custom(w,h)
    language: String,               // BCP-47 ("en-US", "fr-FR", "ar-SA")
    text_direction: TextDirection,  // Ltr | Rtl — drives layout mirroring
    target_duration_mins: Option<u32>,
    presentation_context: PresentationContext,
    // PresentationContext: LiveTalk | AsyncShare | Kiosk | RecordedVideo
}
```

### 3.4 Theme

```rust
struct Theme {
    name: String,
    color_roles: ColorRoles,
    typography: Typography,
    motion_preset: MotionPreset,    // Subtle | Balanced | Cinematic | Explosive
    font_fallback_stack: Vec<String>, // ["Inter", "Helvetica Neue", "Arial", "sans-serif"]
}

struct ColorRoles {
    // Semantic mapping — each agent and the renderer use these names, not hex values
    background: Color,
    surface: Color,
    primary: Color,       // headings, key CTAs
    secondary: Color,     // subheadings, section labels
    accent: Color,        // KPI numbers, highlights, emphasis
    body_text: Color,
    muted_text: Color,
    chart_series: [Color; 8],  // ordered palette for chart data series
    positive: Color,      // growth, success indicators
    negative: Color,      // risk, decline indicators
}

struct Typography {
    heading: FontSpec,
    subheading: FontSpec,
    body: FontSpec,
    mono: FontSpec,
    caption: FontSpec,
    // FontSpec: { family, weight, size_scale_base_px, line_height, letter_spacing }
}
```

### 3.5 MasterSlide

Elements defined here are rendered on every slide, beneath slide-specific content:

```rust
struct MasterSlide {
    elements: Vec<MasterElement>,
    // MasterElement: same as Element but with an additional
    // `exclude_slide_ids: Vec<SlideId>` field for per-slide opt-out
    background: Option<Background>,  // canvas-wide default background
}
```

Use cases: company logo bottom-left, slide number bottom-right, progress bar, watermark.

### 3.6 Asset Registry

All assets (images, fonts, embedded SVGs, user uploads) are registered here. Elements reference assets by ID, never inline.

```rust
struct Asset {
    id: AssetId,                    // uuid
    kind: AssetKind,                // Image | Svg | Font | Video
    filename: String,
    checksum_sha256: String,        // validated on load; mismatch = error, not silent ignore
    size_bytes: u64,
    storage: AssetStorage,
    // AssetStorage: BundledFile(relative_path) | ExternalUrl(url) [read-only, no export portability]
}
```

**Size limits:** single asset max 25 MB. Total deck bundle max 200 MB. Enforced at asset registration time.  
**Deduplication:** assets with identical `checksum_sha256` share one physical file regardless of how many elements reference them.  
**Bundle format:** `.minion-deck` is a zip archive containing `schema.json` + `assets/` directory. Fully portable — no file_path references.

### 3.7 CameraPath

```rust
struct CameraStep {
    id: CameraStepId,
    target: CameraTarget,
    // CameraTarget::Slide(SlideId) — centers on slide bounds
    // CameraTarget::Canvas { x, y, width, height } — arbitrary viewport rect
    zoom: f64,                      // explicit zoom factor at this step (e.g., 0.2 = overview, 1.0 = full slide)
    duration_ms: u32,
    hold_ms: u32,                   // pause at target before next step begins
    easing: CameraEasing,
    // CameraEasing: Linear | EaseInOut | Spring { stiffness: f64, damping: f64 }
    //   Spring bounds: stiffness 1.0–2000.0, damping 0.1–100.0 (validated on write)
}
```

### 3.8 Section and Slide

```rust
struct Section {
    id: SectionId,
    title: String,
    slides: Vec<Slide>,
}

struct Slide {
    id: SlideId,
    section_id: SectionId,

    // Spatial position on canvas
    canvas_x: f64,
    canvas_y: f64,
    width: f64,
    height: f64,
    z_layer: i32,

    // 3D rotation as quaternion (w, x, y, z) — avoids gimbal lock
    // Helper: slides may be created with Euler angles which are converted to quaternion on write
    rotation: Quaternion,

    layout: LayoutKind,
    background: Background,

    transition: SlideTransition,
    // SlideTransition: { kind, duration_ms, easing, direction: Option<Direction> }

    elements: Vec<Element>,
    speaker_notes: SpeakerNotes,

    // Auto-advance: if Some, slide advances automatically after this many ms (kiosk/video mode)
    auto_advance_ms: Option<u32>,

    user_locked: bool,              // true = DesignCritic skips this slide entirely
}
```

**LayoutKind** (fully defined enum — each specifies slot count and slot roles):

| Variant | Slots | Roles |
|---|---|---|
| `Title` | 2 | headline, subtitle |
| `TitleWithMedia` | 3 | headline, subtitle, media |
| `KPI` | 4 | headline, metric×3 |
| `Comparison` | 3 | headline, left-panel, right-panel |
| `Timeline` | 2 | headline, timeline-track |
| `Quote` | 2 | quote-text, attribution |
| `FullBleedMedia` | 2 | media-background, caption |
| `Architecture` | 2 | headline, diagram-area |
| `Process` | 2 | headline, step-track |
| `Matrix` | 2 | headline, matrix-grid |
| `Storytelling` | 3 | headline, body, visual |
| `Blank` | 0 | unconstrained — all elements free-positioned |

### 3.9 Element

```rust
struct Element {
    id: ElementId,                  // uuid — used by animation triggers
    kind: ElementKind,
    // ElementKind: Text | Image | SvgGraphic | ChartSpec | DiagramDsl | Icon | Video

    content: ElementContent,        // kind-tagged union

    // Position within slide (lu, relative to slide top-left)
    x: f64, y: f64,
    width: f64, height: f64,
    z_index: u32,

    style: ElementStyle,

    animation: ElementAnimation,

    // User asset override: if set, renderer uses this asset instead of AI-generated content
    user_asset_id: Option<AssetId>,

    // Lock this element from DesignCritic and AI re-generation
    locked: bool,
}

struct ElementAnimation {
    entrance: Option<AnimPhase>,
    exit: Option<AnimPhase>,
    emphasis: Option<AnimPhase>,

    // Trigger references element by ID, never by array position
    trigger: AnimTrigger,
    // AnimTrigger: OnSlideEnter | OnClick | AfterElement(ElementId) | WithElement(ElementId) | AutoAfterMs(u32)
}

struct AnimPhase {
    effect: AnimEffect,
    delay_ms: u32,
    duration_ms: u32,
    // Spring physics — only applicable when effect is Spring or ZoomIn/ZoomOut with spring easing
    spring: Option<SpringParams>,
    // SpringParams: { stiffness: f64 (1.0–2000.0), damping: f64 (0.1–100.0), mass: f64 (0.1–10.0) }
}
// Note: delay/duration are per-phase (entrance/exit/emphasis), not shared,
// so each phase has independent timing. No ambiguity about which duration applies.
```

**AnimEffect** variants: `Fade | SlideIn(Direction) | ZoomIn | ZoomOut | Spring | ParticleBurst | TypewriterReveal | BlurReveal | ScaleUp | Glow | Shake | Pulse | MotionPath(Vec<PathPoint>)`

### 3.10 SVG Safety — Allowlist Model

Only these SVG elements are permitted inside `SvgGraphic` content:

```
svg, g, defs, symbol
path, rect, circle, ellipse, line, polyline, polygon
text, tspan, title, desc
linearGradient, radialGradient, stop
clipPath, mask
filter (only children: feGaussianBlur, feColorMatrix, feComposite, feBlend)
animate (attribute animations only; repeatCount must be finite or "1")
animateTransform (transform animations only; repeatCount must be finite or "1")
use (href must match /^#[a-zA-Z0-9_-]+$/ — internal ID refs only)
pattern (tile size ≥ 1px enforced to prevent zero-tile infinite loops)
```

**Parameter limits:**
- `feGaussianBlur stdDeviation` max: 20 (above this is invisible and CPU-intensive)
- `animate dur` min: 100ms (prevents sub-frame thrashing)
- `pattern` max tile area: 250,000 lu² (prevents memory exhaustion)
- Circular `<use>` references: detected with a depth-first graph walk at parse time; cycles rejected

**Validation pipeline:** every AI-generated SVG is run through the sanitizer before insertion into DeckSchema. Invalid SVG triggers the retry pipeline (see §5.4).

### 3.11 SpeakerNotes (structured)

```rust
struct SpeakerNotes {
    talking_points: Vec<String>,    // bullet-form talking points
    presenter_cues: Vec<PresenterCue>,
    // PresenterCue: { at_element_id: Option<ElementId>, cue: String }
    // e.g., "click to reveal the diagram before explaining architecture"
    estimated_duration_secs: Option<u32>,
    anticipated_questions: Vec<String>,
}
```

---

## 4. Backend Architecture

### 4.1 Crate Structure

```
crates/minion-presentation/
├── src/
│   ├── lib.rs
│   ├── schema/
│   │   ├── mod.rs             — DeckSchema types, serde, jsonschema validation
│   │   ├── quaternion.rs      — quaternion math + Euler helper
│   │   └── validate.rs        — per-step validation logic
│   ├── orchestrator.rs        — agent pipeline coordinator, checkpoint manager
│   ├── agents/
│   │   ├── mod.rs             — Agent trait, AgentEvent types, streaming protocol
│   │   ├── research.rs
│   │   ├── storyteller.rs
│   │   ├── slide_planner.rs
│   │   ├── visual.rs
│   │   └── design_critic.rs
│   ├── router.rs              — LLM routing (Ollama / OpenAI)
│   ├── context.rs             — token budget tracking, RAG integration, inter-agent compression
│   ├── input/
│   │   ├── mod.rs             — InputSource enum, InputProcessor trait
│   │   ├── text.rs
│   │   ├── document.rs        — PDF (lopdf), DOCX (docx-rs), MD (pulldown-cmark), XLSX (calamine)
│   │   ├── image.rs           — OCR (leptess), image description via vision LLM
│   │   ├── url.rs             — HTTP fetch with SSRF guard, readability extraction
│   │   └── git.rs             — git2 shallow clone, sandboxed, size-limited
│   ├── visual/
│   │   ├── mod.rs
│   │   ├── svg_gen.rs         — LLM SVG generation + sanitizer + retry pipeline
│   │   ├── svg_templates.rs   — fallback template library (20+ pre-built SVG layouts)
│   │   ├── chart_gen.rs       — D3 spec JSON generation
│   │   └── diagram_gen.rs     — Mermaid DSL generation
│   ├── export/
│   │   ├── mod.rs
│   │   ├── pptx.rs            — DeckSchema → PPTX via pptxgenjs IPC bridge
│   │   └── pdf.rs             — DeckSchema → PDF via Tauri WebView programmatic print
│   ├── db.rs                  — deck persistence via minion-db
│   └── security/
│       ├── ssrf_guard.rs      — URL validation, private IP blocking
│       └── git_sandbox.rs     — sandboxed git operations
├── migrations.rs
└── Cargo.toml
```

### 4.2 Database Schema

```sql
-- Deck registry
CREATE TABLE presentations (
    id          TEXT PRIMARY KEY,   -- uuid
    title       TEXT NOT NULL,
    created_at  INTEGER NOT NULL,   -- unix timestamp
    updated_at  INTEGER NOT NULL,
    bundle_path TEXT NOT NULL,      -- path to .minion-deck bundle on disk
    thumbnail   BLOB,               -- PNG thumbnail for deck library
    schema_version TEXT NOT NULL
);

-- Generation sessions (for resume / partial deck recovery)
CREATE TABLE generation_sessions (
    id              TEXT PRIMARY KEY,
    presentation_id TEXT REFERENCES presentations(id),
    status          TEXT NOT NULL,  -- running | completed | failed | interrupted
    started_at      INTEGER NOT NULL,
    completed_at    INTEGER,
    last_checkpoint TEXT,           -- agent name of last completed checkpoint
    error           TEXT
);

-- Per-slide generation results (partial deck delivery)
CREATE TABLE slide_results (
    session_id  TEXT REFERENCES generation_sessions(id),
    slide_index INTEGER NOT NULL,
    slide_id    TEXT NOT NULL,
    status      TEXT NOT NULL,      -- pending | completed | failed
    deck_patch  TEXT,               -- JSON patch for this slide
    PRIMARY KEY (session_id, slide_index)
);

-- User asset library
CREATE TABLE user_assets (
    id          TEXT PRIMARY KEY,
    filename    TEXT NOT NULL,
    kind        TEXT NOT NULL,      -- image | svg | font | video
    checksum    TEXT NOT NULL,
    size_bytes  INTEGER NOT NULL,
    stored_at   TEXT NOT NULL,      -- relative path within asset store
    created_at  INTEGER NOT NULL
);
```

### 4.3 Streaming Protocol

All agent events are emitted via Tauri's event system as `presentation://agent-event` with this typed payload:

```typescript
type AgentEvent =
  | { seq: number; agent: AgentName; kind: "started" }
  | { seq: number; agent: AgentName; kind: "progress"; data: unknown }
  | { seq: number; agent: AgentName; kind: "slide_ready"; slide_index: number; patch: DeckPatch }
  | { seq: number; agent: AgentName; kind: "completed" }
  | { seq: number; agent: AgentName; kind: "error"; message: string; recoverable: boolean }
  | { seq: number; kind: "stream_complete"; deck_id: string }
  | { seq: number; kind: "stream_error"; message: string }
```

- `seq` is a monotonically increasing integer. The frontend uses this to detect gaps and request a replay from the last seen sequence number.
- The frontend can send a `presentation://interrupt` event with `{ after_agent: AgentName, instruction: string }` to redirect the pipeline.
- On reconnect, the frontend sends its last seen `seq`; the backend replays from that point using the `slide_results` table.

### 4.4 Interrupt / Redirect — Transaction Model

Each agent is a **checkpoint**. Before an agent starts, its inputs are written to `generation_sessions.last_checkpoint`. On interrupt:

1. In-flight Tokio tasks for the current and all downstream agents are cancelled via `CancellationToken`.
2. The `slide_results` rows for all slides at or after the interrupt point are set to `pending`.
3. The redirect instruction is injected into the relevant agent's system prompt context.
4. The pipeline re-runs from that agent.

No partial DB writes are possible mid-agent because each agent writes its full output atomically in a single transaction only on successful completion. A cancelled agent leaves no DB trace.

**Redirect cascade rules:**
- Instruction affects tone/voice → re-run from StorytellerAgent
- Instruction affects structure/sections → re-run from SlidePlannerAgent
- Instruction affects a specific slide → re-run VisualAgent for that slide only
- Instruction affects design quality → re-run DesignCriticAgent only

### 4.5 Context Window Management

A `ContextManager` (`context.rs`) tracks token budgets across the pipeline:

- Maintains a running `UsedTokens` counter per model.
- Before each agent call, estimates token cost (input + expected output) using model-specific tokenizer.
- If estimated cost would exceed 80% of context window: runs RAG retrieval to compress the research corpus to the most relevant chunks for the current agent's task.
- Inter-agent compression: each agent's output is summarized to ≤2,000 tokens before being passed as context to the next agent. Full output is stored in the `generation_sessions` row.
- `minion-rag` is used for: chunking uploaded documents, embedding chunks, retrieving relevant chunks per agent task.

### 4.6 LLM Router

Task routing table for Phase 1 (Ollama / OpenAI):

| Task | OpenAI | Ollama |
|---|---|---|
| Research extraction | gpt-4o | llama3.2:latest |
| Narrative generation | gpt-4o | llama3.2:latest |
| Slide content planning | gpt-4o-mini | llama3.2:latest |
| SVG generation | gpt-4o (vision) | llava:latest |
| Chart/diagram DSL | gpt-4o-mini | llama3.2:latest |
| OCR/image description | gpt-4o (vision) | llava:latest |
| Design critique | gpt-4o | llama3.2:latest |

Router selection: configured provider → context window fit → task complexity. Falls back to Ollama if OpenAI key is unavailable. API keys stored in existing minion credential vault.

**OpenAI provider extension (`minion-llm/src/providers/openai.rs`) must implement:**
- Streaming responses (SSE parsing)
- Function/tool calling
- Vision API (base64 image in messages)
- Token counting (tiktoken-rs)
- Rate limit handling (429 → exponential backoff, up to 5 retries)
- Context exceeded handling (graceful truncation with warning)
- API key retrieval from credential vault

### 4.7 SSRF Protection (`security/ssrf_guard.rs`)

Before any HTTP fetch:
1. Resolve hostname to IP via DNS.
2. Reject if IP falls in: `127.0.0.0/8`, `::1`, `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `169.254.0.0/16` (AWS metadata), `fc00::/7` (IPv6 ULA), `fe80::/10` (link-local).
3. Reject non-HTTP/HTTPS schemes.
4. Enforce max response size: 10 MB.
5. Enforce timeout: 30 seconds.
6. Follow max 3 redirects; re-validate each redirect target IP.

### 4.8 XLSX Handling in ResearchAgent

Spreadsheets ingested via `calamine` are converted to a structured text representation: sheet name + column headers + first 200 rows per sheet (truncated with a note if larger). Numeric columns are summarized with min/mean/max. This text representation is passed to ResearchAgent as part of the input corpus. Charts embedded in XLSX are not extracted (Phase 1 limitation).

### 4.9 Git Ingestion Sandbox (`security/git_sandbox.rs`)

1. Clone into a temp directory (`tempfile::TempDir` — auto-deleted on drop).
2. Shallow clone only (`--depth=1`), no credential helper propagation.
3. Abort if uncompressed repo size exceeds 100 MB.
4. Only read: `README.md`, `*.md` in root, directory tree (no file contents beyond docs), `Cargo.toml`/`package.json`/`pyproject.toml` for structure.
5. No `.git/config` or credential file access.

### 4.10 Default Camera Path Fallback

If SlidePlannerAgent does not produce a `camera_path` (e.g., the LLM output is malformed), the orchestrator generates a default linear path: each slide in `play_order` is visited in sequence at zoom 1.0x, with a 600ms EaseInOut transition between slides. The DesignCriticAgent may enrich this with zoom-in/out steps based on slide layout and content type.

### 4.11 Visual Generation — Retry and Fallback

SVG generation pipeline (per element):
1. Prompt LLM with SVG generation prompt.
2. Parse result as SVG using `roxmltree` (a safe, allocation-efficient Rust XML parser).
3. Run allowlist sanitizer.
4. If parse or sanitization fails: retry with a more constrained prompt (up to 3 attempts, varying prompt temperature).
5. If all 3 attempts fail: fall back to the closest matching template from `svg_templates.rs` (20+ pre-built layouts covering: arrows, process flows, architecture diagrams, timeline tracks, icon clusters, data callouts, etc.).
6. Log fallback occurrence as a `critique_issue` so the user can see which visuals are template-generated vs AI-generated.

---

## 5. AI Agent Pipeline (Detail)

### 5.1 ResearchAgent

**Input:** All processed input sources → unified text corpus  
**Token budget:** max 12,000 tokens input (RAG-compressed if larger)  
**Output:**
```json
{
  "audience": "engineering leadership",
  "tone": "authoritative, concise",
  "language": "en-US",
  "key_themes": ["scalability", "migration", "cost"],
  "facts": [{ "claim": "...", "source": "doc page 3", "confidence": 0.9 }],
  "suggested_section_count": 5,
  "target_duration_mins": 15,
  "raw_assets": ["screenshot-1.png"]
}
```
**Streams:** `progress` event per fact group extracted.

### 5.2 StorytellerAgent

**Input:** ResearchAgent output  
**Output:**
```json
{
  "title": "...",
  "hook": "...",
  "sections": [
    {
      "title": "The Problem",
      "slide_count": 2,
      "purpose": "establish tension",
      "pacing": "slow",
      "dominant_emotion": "concern"
    }
  ],
  "closing_cta": "...",
  "camera_narrative": "open at overview → zoom to problem → zoom to solution → pull back to roadmap"
}
```
**Streams:** `progress` event per section decided.

### 5.3 SlidePlannerAgent

**Input:** StorytellerAgent output + facts  
**Output:** Full DeckSchema with all slides populated — except SVG/chart/diagram elements, which are left as typed placeholders with generation specs.  
**Streams:** `slide_ready` event per slide (frontend renders skeleton immediately).

### 5.4 VisualAgent

**Input:** Each slide's visual placeholder specs  
**Concurrency:** per-slide `tokio::JoinSet` with a semaphore-based concurrency limit of 4 simultaneous LLM calls (prevents rate limit exhaustion on OpenAI and prevents saturating Ollama). Results collected in-order: `JoinSet` stores `(slide_index, result)` tuples; the orchestrator assembles slides in ascending `slide_index` order after all tasks complete.  
**Output:** Filled element payloads replacing placeholders.  
**Streams:** `slide_ready` events with visual patches as each slide completes.

### 5.5 DesignCriticAgent

**Input:** Complete draft DeckSchema  
**Checks:**
- Text density per slide (word count > 80 → split suggestion)
- Color contrast (WCAG AA, 4.5:1 minimum for body text)
- Visual hierarchy (heading weight vs body weight ratio)
- Animation rhythm (> 3 simultaneous entrances on one slide → stagger them)
- Camera path coherence (zoom level changes > 10x in one step → insert intermediate step)
- Consistency (font/spacing variance across slides)

**Output:** Array of `DeckPatch` operations applied atomically. Skips slides with `user_locked: true` and elements with `locked: true`.  
**Streams:** `progress` event per issue found and patched.

---

## 6. Frontend

### 6.1 Page Structure

```
ui/src/pages/Presentation.tsx
├── PresentationLibrary          — deck list, search, create new
├── CreationStudio               — multi-source input + theme picker
├── DeckWorkspace
│   ├── AgentSidebar             — live agent activity (collapsible, hidden by default)
│   ├── SpatialCanvas            — SVG-first infinite canvas renderer
│   │   ├── SlideNode(×N)        — each slide rendered as SVG group
│   │   └── CameraController     — zoom/pan with spring physics
│   ├── SlideEditor              — click any element to edit inline
│   │   ├── TextEditor           — contenteditable with typography controls
│   │   ├── ElementControls      — resize, reposition, lock, replace with user asset
│   │   └── AnimationPanel       — per-element animation controls
│   ├── SlideTray                — thumbnail strip, reorder, add/delete slides
│   └── Toolbar                  — theme switcher, undo/redo, export
├── PresentationPlayer           — full-screen cinematic playback
│   ├── CameraAnimator           — executes camera_path with spring/easing
│   ├── ElementAnimator          — executes ElementAnimation per trigger
│   └── PresenterOverlay         — speaker notes, timer, next-slide preview
└── ExportDialog                 — format selection, progress, file location
```

### 6.2 Spatial Canvas Renderer

- **SVG-first:** the canvas is a `<div>` with an inner `<svg>` coordinate layer. Slides are `<div>` elements absolutely positioned inside a transform layer. SVG graphics and diagrams are rendered as inline `<svg>` within those divs.
- **Camera:** the outer transform div receives `transform: translate(Xpx, Ypx) scale(Z)`. Spring-physics camera moves via Motion One's `animate()`.
- **3D rotations:** CSS `transform: perspective(1200px) rotateX(Xdeg) rotateY(Ydeg) rotateZ(Zdeg)` applied to slide divs. Values derived from DeckSchema quaternion via `quaternionToCssAngles()` (Euler extraction for CSS, with gimbal-lock detection — if lock is detected, the rotation is decomposed via the swing-twist decomposition into a safe CSS form). Pure SVG `<g>` transforms do not support perspective; using CSS transforms is correct here.
- **Performance:** slides whose bounding box falls entirely outside the current visible viewport rect (computed from camera translate + scale) are rendered with `visibility: hidden` and `content-visibility: auto`. Slide content (SVG, charts) lazy-initialises when the camera comes within 2× viewport width of the slide. The viewport threshold of 2× provides smooth pre-loading without wasting resources on distant slides.
- **Zoom navigation:** scroll wheel = zoom, drag = pan, click slide = camera flies to that slide (triggers `CameraStep`).

### 6.3 Presentation Player

Activated by "Present" button → full-screen mode.

- Executes `camera_path` sequentially.
- Each `CameraStep` uses `CameraEasing` (spring or ease-in-out) via Motion One's `animate()`.
- Element animations fire based on `AnimTrigger`:
  - `OnSlideEnter` → fires when camera arrives at slide
  - `OnClick` / spacebar → user advance
  - `AfterElement(id)` / `WithElement(id)` → chained timing
  - `AutoAfterMs(n)` → used in kiosk/video mode
- Presenter overlay (second window / split view): shows `SpeakerNotes.talking_points`, timer, next slide thumbnail.

### 6.4 Slide Editor

Click any element on the canvas → inline edit mode:
- **Text elements:** `contenteditable` div overlaid at exact element position. Typography controls in a floating toolbar.
- **Visual elements (SVG/chart/diagram):** replace button opens asset browser or triggers AI re-generation for this element only.
- **Drag to reposition:** pointer events on element → update `x, y` in local SolidJS store (immediate, reactive) → debounced Tauri `save_deck_patch` call after 800ms of inactivity.
- **Resize:** resize handles on element corners.
- **Lock:** lock icon in element toolbar sets `locked: true` — AI will not touch this element.
- **User asset:** "Replace with my asset" → opens asset browser → sets `user_asset_id`.

### 6.5 Animation Panel

Per-element, per-slide controls:
- Entrance / exit / emphasis effect picker (dropdown of AnimEffect variants)
- Delay and duration sliders
- Trigger picker
- Spring parameter sliders (stiffness, damping) with live preview
- "None" option to disable animation on a specific element

Global:
- `motion_preset` selector (Subtle / Balanced / Cinematic / Explosive) — sets defaults, overridable per element.

### 6.6 Creation Studio

Multi-source input area:
- **Text tab:** large textarea for paste/type
- **Files tab:** drag-and-drop or file picker (PDF, DOCX, TXT, MD, XLSX, PNG, JPG)
- **URL tab:** URL input field (SSRF-guarded)
- **Git tab:** git repo URL field
- Multiple sources combinable in one session
- **Audience/tone panel:** `audience` free-text or preset chips (Engineering / Executive / Investor / General), `tone` chips (Authoritative / Conversational / Inspirational / Technical), `target_duration_mins` number input, `language` selector
- **Theme picker:** grid of 8 built-in themes with live preview thumbnail
- **Slide count:** optional override (AI will target this count)

### 6.7 Agent Sidebar

- Hidden by default. Activated via "AI Activity" button in toolbar.
- Shows each agent as a card: name, status (waiting/running/done), live streaming text output.
- Interrupt button per agent: opens redirect input field.
- Non-technical users never need to open this.

### 6.8 Deck Library (PresentationLibrary)

- Grid of deck thumbnails with title, date, slide count.
- Search by title (minion-search integration).
- Click → open in DeckWorkspace.
- Right-click → duplicate, rename, delete, export.
- "New Presentation" button → opens Creation Studio.

### 6.9 Undo / Redo

- Client-side undo stack using SolidJS `createStore` + a history array signal. Max 50 states.
- Each user edit (text change, element move, animation change) pushes a `DeckPatch` onto the stack.
- Undo replays patches in reverse. Redo replays forward.
- AI generation is not undoable to a pre-generation state (that would require a full deck snapshot — stored as `deck_revision` in DB instead; user can "revert to original generated version").

### 6.10 Export Dialog

- Format tabs: PPTX / PDF / Interactive HTML / Speaker Notes (PDF)
- File size estimate shown before export starts
- Progress bar during export
- On completion: "Open file" / "Reveal in Finder" / "Copy path" buttons
- PPTX: note shown — "Animations and 3D transitions are approximated in PPTX format"

---

## 7. Export Pipeline

### 7.1 PPTX Export

**Strategy:** high-fidelity slide content + best-effort animations.

1. Tauri command triggers `export/pptx.rs`.
2. For each slide: the frontend renders each slide in a hidden off-screen div at 1920×1080, then uses the browser's `html-to-image` library (which serializes DOM to canvas) to produce a PNG data URL. This runs inside the existing WebView — no additional WebView needed.
3. `pptxgenjs` (running in the WebView) receives: PNG slide images + text elements with original content + speaker notes.
4. PPTX is built with: PNG as slide background (pixel-perfect) + transparent text boxes overlaid (preserves copy-paste of text) + speaker notes.
5. PPTX animations: mapped from DeckSchema where pptxgenjs supports it (fade, push, zoom); unsupported effects (particle, spring, motion path) are dropped gracefully.
6. Output: `.pptx` file saved to user-specified path.

**Known limitations documented to user:** "3D transitions, particle effects, spring animations, and motion paths are not supported in PPTX format and will appear as simple cuts."

### 7.2 PDF Export

**Strategy:** programmatic WebView print, not print dialog.

1. The frontend switches the active presentation into "PDF render mode": animations disabled, all elements fully visible at once, layout locked.
2. CSS `@page` rules set exact dimensions per `meta.aspect_ratio`.
3. Each slide is one page. Text remains as DOM text (searchable, copy-pasteable).
4. Tauri calls `webview.print_to_pdf(options)` (available in `tauri-plugin-webview`).
5. Speaker notes variant: separate PDF with one page per slide, thumbnail + structured notes.

### 7.3 Interactive HTML Export

**Phase 1 risk note:** this requires building and maintaining a separate tree-shaken production build of the SolidJS player with no editor dependencies. This is non-trivial and is the most complex export format. If it delays Phase 1 delivery it should be deferred to Phase 2.

1. Bundle: compiled SolidJS presentation player (tree-shaken, editor excluded via build flag) + DeckSchema JSON + assets.
2. Output: `index.html` + `assets/` directory zipped as `<deck-title>-interactive.zip`.
3. Fully offline. No server required.
4. Includes full cinematic camera path and animations.

---

## 8. Tauri IPC Commands (6 new commands)

```rust
// src-tauri/src/commands.rs additions:

#[tauri::command]
async fn start_presentation_generation(
    inputs: Vec<InputSource>,
    config: GenerationConfig,   // theme, audience, tone, target_duration, language, slide_count_hint
    state: State<AppState>,
) -> Result<GenerationSessionId, String>

#[tauri::command]
async fn interrupt_generation(
    session_id: GenerationSessionId,
    after_agent: String,
    instruction: String,
    state: State<AppState>,
) -> Result<(), String>

#[tauri::command]
async fn get_deck(id: DeckId, state: State<AppState>) -> Result<DeckSchema, String>

#[tauri::command]
async fn save_deck_patch(
    id: DeckId,
    patch: Vec<DeckPatch>,
    state: State<AppState>,
) -> Result<(), String>

#[tauri::command]
async fn list_presentations(state: State<AppState>) -> Result<Vec<DeckSummary>, String>

#[tauri::command]
async fn export_presentation(
    id: DeckId,
    format: ExportFormat,       // Pptx | Pdf | Html | SpeakerNotesPdf
    output_path: String,
    state: State<AppState>,
) -> Result<ExportResult, String>
```

---

## 9. Security Summary

| Threat | Mitigation |
|---|---|
| SSRF via URL ingestion | `ssrf_guard.rs`: private IP blocklist, redirect revalidation, 10MB limit |
| XSS via LLM SVG | SVG element allowlist, parameter bounds, circular ref detection |
| Credential exfil via git | Shallow clone, temp sandbox, no credential helper, no `.git/config` read |
| Oversized asset DoS | 25MB per asset, 200MB per deck, validated at registration |
| API key exposure | Keys stored in existing minion credential vault (AES-256-GCM) |
| Malicious PPTX/PDF | Export-only; no import of PPTX/PDF into the rendering engine |
| Prompt injection via inputs | Input is passed as user content, never interpolated into system prompt |

---

## 10. Out of Scope (Phase 1)

- Real-time multiplayer collaboration
- Notion / Jira / Confluence integrations
- YouTube transcript ingestion
- Voice narration / audio generation
- Gemini, Groq, DeepSeek, Anthropic providers (Phase 2)
- Mobile / web deployment
- Kubernetes / Docker (desktop app)
- Multi-tenant RBAC (single-user desktop)
- AI-generated presenter voice

---

## 11. Open Questions (to resolve during implementation)

1. Does `tauri-plugin-webview` expose `print_to_pdf()` on the current Tauri 2 version used in minion? If not, fallback is `wkhtmltopdf` subprocess.
2. Does Motion One's spring animation API match the spring parameter model in DeckSchema, or does it need a wrapper?
3. What is the exact tokenizer used by `tiktoken-rs` for Ollama models? (OpenAI models are well-defined; Ollama varies by model.)
4. Is `leptess` (Tesseract bindings) available as a static link on Linux, or does it require a system Tesseract install? (affects packaging)

---

*End of spec. Ready for implementation plan.*
