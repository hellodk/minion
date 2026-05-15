# Presentation Module — Sub-Plan 2d: Visual Generation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** SVG allowlist sanitizer, fallback templates, chart/diagram DSL generators, VisualAgent (parallel placeholder filling), DesignCriticAgent (deck quality review).

**Architecture:** All LLM-generated SVG passes roxmltree allowlist sanitizer before insertion. VisualAgent uses JoinSet+Semaphore(4) for parallel filling. DesignCriticAgent emits DeckPatch corrections.

**Tech Stack:** Rust, roxmltree, tokio (JoinSet, Semaphore), minion-llm, DeckSchema types.

---

## Prerequisites

Sub-plans 2a, 2b, 2c must be complete:

```bash
grep "pub mod agents" /home/dk/Documents/git/minion/crates/minion-presentation/src/lib.rs
ls /home/dk/Documents/git/minion/crates/minion-presentation/src/agents/
```

Expected: `mod.rs  research.rs  slide_planner.rs  storyteller.rs`

---

## Task 1: SVG Sanitizer (`src/visual/svg_sanitizer.rs`)

**Files:** create `src/visual/mod.rs`, `src/visual/svg_sanitizer.rs`; modify `src/lib.rs` (add `pub mod visual;`), `Cargo.toml` (add `regex = "1"`).

### Step 1.1 — Failing tests

- [ ] Create `crates/minion-presentation/tests/visual_tests.rs`:

```rust
use minion_presentation::visual::svg_sanitizer::sanitize_svg;

#[test]
fn valid_svg_passes_through() {
    let input = r#"<svg xmlns="http://www.w3.org/2000/svg"><rect width="10" height="10"/></svg>"#;
    assert!(sanitize_svg(input).unwrap().contains("<rect"));
}
#[test]
fn script_tag_stripped() {
    let input = r#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script><rect/></svg>"#;
    let out = sanitize_svg(input).unwrap();
    assert!(!out.contains("script") && out.contains("<rect"));
}
#[test]
fn invalid_use_href_rejected() {
    let input = r#"<svg xmlns="http://www.w3.org/2000/svg"><use href="javascript:alert(1)"/></svg>"#;
    assert!(sanitize_svg(input).is_err());
}
#[test]
fn on_event_attr_stripped() {
    let input = r#"<svg xmlns="http://www.w3.org/2000/svg"><rect onclick="evil()" width="10" height="10"/></svg>"#;
    assert!(!sanitize_svg(input).unwrap().contains("onclick"));
}
#[test]
fn fe_gaussian_blur_capped_at_20() {
    let input = r#"<svg xmlns="http://www.w3.org/2000/svg"><filter><feGaussianBlur stdDeviation="999"/></filter></svg>"#;
    assert!(sanitize_svg(input).unwrap().contains("stdDeviation=\"20\""));
}
```

- [ ] `cargo test -p minion-presentation sanitizer` — expect compile error.

### Step 1.2 — Implement

- [ ] `src/visual/mod.rs`:

```rust
pub mod chart_gen;
pub mod diagram_gen;
pub mod svg_sanitizer;
pub mod svg_templates;
```

- [ ] `src/visual/svg_sanitizer.rs`:

```rust
use roxmltree::Document;
use std::fmt::Write;

const ALLOWED: &[&str] = &[
    "svg","g","defs","symbol","path","rect","circle","ellipse","line","polyline",
    "polygon","text","tspan","title","desc","linearGradient","radialGradient","stop",
    "clipPath","mask","filter","feGaussianBlur","feColorMatrix","feComposite","feBlend",
    "animate","animateTransform","use","pattern",
];

static HREF_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
fn href_re() -> &'static regex::Regex {
    HREF_RE.get_or_init(|| regex::Regex::new(r"^#[a-zA-Z0-9_-]+$").unwrap())
}

pub fn sanitize_svg(input: &str) -> Result<String, String> {
    let doc = Document::parse(input).map_err(|e| e.to_string())?;
    let mut out = String::with_capacity(input.len());
    serialize_node(doc.root(), &mut out)?;
    Ok(out)
}

fn serialize_node(node: roxmltree::Node, out: &mut String) -> Result<(), String> {
    match node.node_type() {
        roxmltree::NodeType::Root => {
            for child in node.children() { serialize_node(child, out)?; }
        }
        roxmltree::NodeType::Element => {
            let tag = node.tag_name().name();
            if !ALLOWED.contains(&tag) { return Ok(()); }
            write!(out, "<{tag}").unwrap();
            for attr in node.attributes() {
                let (name, value) = (attr.name(), attr.value());
                if name.starts_with("on") { continue; }
                if (name == "href" || name == "xlink:href") && tag == "use" {
                    if !href_re().is_match(value) {
                        return Err(format!("invalid use href: {value}"));
                    }
                }
                if tag == "feGaussianBlur" && name == "stdDeviation" {
                    let v: f64 = value.parse().unwrap_or(0.0);
                    write!(out, " stdDeviation=\"{}\"", v.min(20.0)).unwrap();
                    continue;
                }
                write!(out, " {name}=\"{value}\"").unwrap();
            }
            if node.has_children() {
                write!(out, ">").unwrap();
                for child in node.children() { serialize_node(child, out)?; }
                write!(out, "</{tag}>").unwrap();
            } else {
                write!(out, "/>").unwrap();
            }
        }
        roxmltree::NodeType::Text => { out.push_str(node.text().unwrap_or("")); }
        _ => {}
    }
    Ok(())
}
```

### Step 1.3 — Run and commit

- [ ] `cargo test -p minion-presentation sanitizer` — all 5 pass.
- [ ] `cargo build -p minion-presentation`
- [ ] `git add crates/minion-presentation/src/visual/ crates/minion-presentation/src/lib.rs crates/minion-presentation/Cargo.toml crates/minion-presentation/tests/visual_tests.rs`
- [ ] `git commit -m "feat(presentation): add SVG allowlist sanitizer"`

---

## Task 2: SVG Templates (`src/visual/svg_templates.rs`)

**Files:** create `src/visual/svg_templates.rs`.

### Step 2.1 — Failing tests

- [ ] Append to `tests/visual_tests.rs`:

```rust
use minion_presentation::visual::svg_templates::template_for;

#[test] fn arrow_template() { assert!(template_for("arrow").contains("<svg")); }
#[test] fn process_template() { assert!(template_for("process").contains("<svg")); }
#[test] fn kpi_template() { assert!(template_for("kpi").contains("<svg")); }
#[test] fn comparison_template() { assert!(template_for("comparison").contains("<svg")); }
#[test] fn default_template() { assert!(template_for("unknown_xyz").contains("<svg")); }
```

- [ ] `cargo test -p minion-presentation template` — compile error expected.

### Step 2.2 — Implement

- [ ] `src/visual/svg_templates.rs`:

```rust
pub fn template_for(spec_hint: &str) -> String {
    match spec_hint {
        "arrow" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 60">
  <defs><marker id="ah" markerWidth="10" markerHeight="7" refX="10" refY="3.5" orient="auto">
    <polygon points="0 0,10 3.5,0 7" fill="#6366f1"/></marker></defs>
  <line x1="10" y1="30" x2="180" y2="30" stroke="#6366f1" stroke-width="3" marker-end="url(#ah)"/>
</svg>"#.into(),
        "process" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 80">
  <rect x="0" y="20" width="100" height="40" rx="6" fill="#6366f1"/>
  <rect x="150" y="20" width="100" height="40" rx="6" fill="#6366f1"/>
  <rect x="300" y="20" width="100" height="40" rx="6" fill="#6366f1"/>
  <line x1="100" y1="40" x2="150" y2="40" stroke="#fff" stroke-width="2"/>
  <line x1="250" y1="40" x2="300" y2="40" stroke="#fff" stroke-width="2"/>
</svg>"#.into(),
        "kpi" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100">
  <rect width="200" height="100" rx="12" fill="#1c1c24"/>
  <text x="100" y="55" text-anchor="middle" font-size="42" font-weight="700" fill="#6366f1">0</text>
  <text x="100" y="80" text-anchor="middle" font-size="14" fill="#a0a0b4">Label</text>
</svg>"#.into(),
        "comparison" => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 120">
  <rect x="0" y="0" width="140" height="120" rx="8" fill="#1c1c24"/>
  <rect x="160" y="0" width="140" height="120" rx="8" fill="#1c1c24"/>
  <text x="70" y="65" text-anchor="middle" font-size="16" fill="#fff">A</text>
  <text x="230" y="65" text-anchor="middle" font-size="16" fill="#fff">B</text>
</svg>"#.into(),
        _ => r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 100">
  <rect width="300" height="100" rx="8" fill="#1c1c24" stroke="#6366f1" stroke-width="1"/>
  <text x="150" y="55" text-anchor="middle" font-size="16" fill="#a0a0b4">Visual</text>
</svg>"#.into(),
    }
}
```

### Step 2.3 — Run and commit

- [ ] `cargo test -p minion-presentation template` — all 5 pass.
- [ ] `git add crates/minion-presentation/src/visual/svg_templates.rs`
- [ ] `git commit -m "feat(presentation): add 5 fallback SVG templates"`

---

## Task 3: Chart + Diagram Generators

**Files:** create `src/visual/chart_gen.rs`, `src/visual/diagram_gen.rs`.

### Step 3.1 — Failing tests

- [ ] Append to `tests/visual_tests.rs`:

```rust
use minion_presentation::visual::chart_gen::generate_chart_spec;
use minion_presentation::visual::diagram_gen::generate_mermaid_dsl;

#[test] fn chart_spec_has_type_key() {
    let s = generate_chart_spec("monthly revenue", "bar");
    assert_eq!(s["type"], "bar");
}
#[test] fn chart_spec_has_data_key() {
    assert!(generate_chart_spec("q1 sales", "pie").get("data").is_some());
}
#[test] fn mermaid_flowchart_keyword() {
    let d = generate_mermaid_dsl("login flow", "flowchart");
    assert!(d.trim_start().starts_with("flowchart") || d.trim_start().starts_with("graph"));
}
#[test] fn mermaid_sequence_keyword() {
    assert!(generate_mermaid_dsl("api lifecycle", "sequence").trim_start().starts_with("sequenceDiagram"));
}
#[test] fn mermaid_default_is_graph() {
    let d = generate_mermaid_dsl("process", "unknown");
    assert!(d.trim_start().starts_with("graph") || d.trim_start().starts_with("flowchart"));
}
```

- [ ] `cargo test -p minion-presentation chart_spec mermaid` — compile errors expected.

### Step 3.2 — Implement

- [ ] `src/visual/chart_gen.rs`:

```rust
pub fn generate_chart_spec(data_description: &str, chart_type: &str) -> serde_json::Value {
    serde_json::json!({
        "type": chart_type,
        "title": data_description,
        "data": { "labels": ["A","B","C"], "datasets": [{"label": data_description, "data": [0,0,0]}] },
        "options": { "responsive": true }
    })
}
```

- [ ] `src/visual/diagram_gen.rs`:

```rust
pub fn generate_mermaid_dsl(description: &str, diagram_type: &str) -> String {
    match diagram_type {
        "sequence" => format!("sequenceDiagram\n    A->>B: {description}\n    B-->>A: response"),
        "flowchart" | "flow" => format!("flowchart LR\n    A[Start] --> B[{description}] --> C[End]"),
        "class" => format!("classDiagram\n    class Entity\n    note \"{description}\""),
        "er" => format!("erDiagram\n    ENTITY {{string name}}\n    note \"{description}\""),
        _ => format!("graph LR\n    A[Start] --> B[{description}] --> C[End]"),
    }
}
```

### Step 3.3 — Run and commit

- [ ] `cargo test -p minion-presentation chart_spec mermaid` — all 5 pass.
- [ ] `git add crates/minion-presentation/src/visual/chart_gen.rs crates/minion-presentation/src/visual/diagram_gen.rs`
- [ ] `git commit -m "feat(presentation): add chart spec and mermaid DSL generators"`

---

## Task 4: VisualAgent (`src/agents/visual.rs`)

**Files:** create `src/agents/visual.rs`; modify `src/agents/mod.rs` (add `pub mod visual;`).

### Step 4.1 — Failing test

- [ ] Append to `tests/visual_tests.rs`:

```rust
use minion_presentation::agents::visual::VisualAgent;
use minion_presentation::schema::types::*;
use std::sync::atomic::AtomicU32;
use chrono::Utc;

fn placeholder_deck(n: usize) -> Deck {
    let sid = SectionId::new();
    let slides = (0..n).map(|i| {
        let mut sl = Slide::new(sid.clone(), i as f64 * 1920.0, 0.0, LayoutKind::Blank);
        sl.elements.push(Element {
            id: ElementId::new(),
            content: ElementContent::Text { markdown: "[[VISUAL_PLACEHOLDER: diagram | login flow]]".into() },
            x: 0.0, y: 0.0, width: 800.0, height: 400.0, z_index: 1,
            style: ElementStyle::default(),
            animation: ElementAnimation { entrance: None, exit: None, emphasis: None, trigger: AnimTrigger::OnSlideEnter },
            user_asset_id: None, locked: false,
        });
        sl
    }).collect();
    Deck {
        meta: DeckMeta { title: "T".into(), author: "t".into(), deck_revision: 1,
            schema_version: "1.0".into(), created_at: Utc::now(), updated_at: Utc::now(),
            aspect_ratio: AspectRatio::Ratio16x9, language: "en".into(),
            text_direction: TextDirection::Ltr, target_duration_mins: None,
            presentation_context: PresentationContext::LiveTalk },
        theme: Theme::default(),
        master: MasterSlide { elements: vec![], background: None },
        assets: vec![], camera_path: vec![],
        sections: vec![Section { id: sid, title: "S".into(), slides }],
        play_order: vec![],
    }
}

#[tokio::test]
async fn visual_agent_fills_placeholder_slides() {
    let mut deck = placeholder_deck(2);
    let (tx, _rx) = tokio::sync::broadcast::channel(32);
    let seq = AtomicU32::new(0);
    VisualAgent::new_without_provider().run(&mut deck, &tx, &seq).await.unwrap();
    for slide in deck.sections[0].slides.iter() {
        for el in &slide.elements {
            if let ElementContent::Text { markdown } = &el.content {
                assert!(!markdown.starts_with("[[VISUAL_PLACEHOLDER:"));
            }
        }
    }
}
```

- [ ] `cargo test -p minion-presentation visual_agent` — compile error expected.

### Step 4.2 — Implement

- [ ] `src/agents/visual.rs`:

```rust
use crate::agents::{agent_name, next_seq, AgentEvent, EventTx};
use crate::schema::types::*;
use crate::visual::{diagram_gen::generate_mermaid_dsl, svg_templates::template_for};
use std::sync::{atomic::AtomicU32, Arc};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

pub struct VisualAgent { provider: Option<Arc<dyn minion_llm::LlmProvider>> }

impl VisualAgent {
    pub fn new_with_provider(p: Arc<dyn minion_llm::LlmProvider>) -> Self { Self { provider: Some(p) } }
    pub fn new_without_provider() -> Self { Self { provider: None } }

    pub async fn run(&self, deck: &mut Deck, event_tx: &EventTx, seq: &AtomicU32) -> anyhow::Result<()> {
        let sem = Arc::new(Semaphore::new(4));
        let mut set: JoinSet<(usize, usize, ElementContent)> = JoinSet::new();
        for (si, section) in deck.sections.iter().enumerate() {
            for (li, slide) in section.slides.iter().enumerate() {
                for el in &slide.elements {
                    if let ElementContent::Text { markdown } = &el.content {
                        if let Some(spec) = parse_placeholder(markdown) {
                            let sem = sem.clone();
                            set.spawn(async move {
                                let _p = sem.acquire_owned().await.unwrap();
                                (si, li, fill_placeholder(&spec).await)
                            });
                            let _ = event_tx.send(AgentEvent::Progress {
                                seq: next_seq(seq), agent: agent_name::VISUAL.into(),
                                data: format!("queued slide {li}"),
                            });
                        }
                    }
                }
            }
        }
        while let Some(Ok((si, li, content))) = set.join_next().await {
            let slide = &mut deck.sections[si].slides[li];
            for el in &mut slide.elements {
                if matches!(&el.content, ElementContent::Text { markdown }
                    if markdown.starts_with("[[VISUAL_PLACEHOLDER:"))
                {
                    el.content = content.clone();
                    break;
                }
            }
            let _ = event_tx.send(AgentEvent::SlideReady {
                seq: next_seq(seq), agent: agent_name::VISUAL.into(),
                slide_index: li as u32,
                patch: DeckPatch::UpsertSlide { section_id: deck.sections[si].id.clone(), slide: slide.clone() },
            });
        }
        Ok(())
    }
}

fn parse_placeholder(md: &str) -> Option<String> {
    Some(md.strip_prefix("[[VISUAL_PLACEHOLDER:")?.strip_suffix("]]")?.trim().to_string())
}

async fn fill_placeholder(spec: &str) -> ElementContent {
    let parts: Vec<&str> = spec.splitn(2, '|').collect();
    let hint = parts[0].trim();
    let desc = parts.get(1).map(|s| s.trim()).unwrap_or(hint);
    match hint {
        "diagram" | "mermaid" => ElementContent::DiagramDsl {
            dsl: generate_mermaid_dsl(desc, "flowchart"),
            renderer: DiagramRenderer::Mermaid,
        },
        "chart" => ElementContent::ChartSpec {
            spec_json: crate::visual::chart_gen::generate_chart_spec(desc, "bar"),
        },
        _ => ElementContent::SvgGraphic { svg_xml: template_for(hint) },
    }
}
```

### Step 4.3 — Run and commit

- [ ] `cargo test -p minion-presentation visual_agent` — passes.
- [ ] `git add crates/minion-presentation/src/agents/visual.rs crates/minion-presentation/src/agents/mod.rs`
- [ ] `git commit -m "feat(presentation): add VisualAgent with Semaphore(4) parallel placeholder filling"`

---

## Task 5: DesignCriticAgent (`src/agents/design_critic.rs`)

**Files:** create `src/agents/design_critic.rs`; modify `src/agents/mod.rs` (add `pub mod design_critic;`).

### Step 5.1 — Failing tests

- [ ] Append to `tests/visual_tests.rs`:

```rust
use minion_presentation::agents::design_critic::DesignCriticAgent;

fn wordy_slide(sid: SectionId, words: usize) -> Slide {
    let mut sl = Slide::new(sid, 0.0, 0.0, LayoutKind::Blank);
    sl.elements.push(Element {
        id: ElementId::new(),
        content: ElementContent::Text { markdown: "word ".repeat(words) },
        x: 0.0, y: 0.0, width: 800.0, height: 400.0, z_index: 1,
        style: ElementStyle::default(),
        animation: ElementAnimation { entrance: None, exit: None, emphasis: None, trigger: AnimTrigger::OnSlideEnter },
        user_asset_id: None, locked: false,
    });
    sl
}

fn anim_slide(sid: SectionId, n: usize) -> Slide {
    let mut sl = Slide::new(sid, 0.0, 0.0, LayoutKind::Blank);
    for _ in 0..n {
        sl.elements.push(Element {
            id: ElementId::new(),
            content: ElementContent::Text { markdown: "x".into() },
            x: 0.0, y: 0.0, width: 100.0, height: 40.0, z_index: 1,
            style: ElementStyle::default(),
            animation: ElementAnimation {
                entrance: Some(AnimPhase { effect: AnimEffect::Fade, delay_ms: 0, duration_ms: 400, spring: None }),
                exit: None, emphasis: None, trigger: AnimTrigger::OnSlideEnter,
            },
            user_asset_id: None, locked: false,
        });
    }
    sl
}

#[test]
fn design_critic_detects_wordcount_over_80() {
    let mut deck = placeholder_deck(0);
    deck.sections[0].slides.push(wordy_slide(deck.sections[0].id.clone(), 90));
    let patches = DesignCriticAgent::new().review(&deck);
    assert!(patches.iter().any(|p| matches!(p, DeckPatch::DeleteSlide { .. })));
}

#[test]
fn design_critic_staggers_excess_enter_animations() {
    let mut deck = placeholder_deck(0);
    deck.sections[0].slides.push(anim_slide(deck.sections[0].id.clone(), 4));
    let patches = DesignCriticAgent::new().review(&deck);
    assert!(patches.iter().any(|p| matches!(p, DeckPatch::UpsertElement { .. })));
}
```

- [ ] `cargo test -p minion-presentation design_critic` — compile error expected.

### Step 5.2 — Implement

- [ ] `src/agents/design_critic.rs`:

```rust
use crate::schema::types::*;

pub struct DesignCriticAgent;

impl DesignCriticAgent {
    pub fn new() -> Self { Self }

    pub fn review(&self, deck: &Deck) -> Vec<DeckPatch> {
        let mut patches = Vec::new();
        for section in &deck.sections {
            for slide in &section.slides {
                if word_count(slide) > 80 {
                    patches.push(DeckPatch::DeleteSlide { slide_id: slide.id.clone() });
                }
                let enter_elems: Vec<&Element> = slide.elements.iter()
                    .filter(|e| matches!(e.animation.trigger, AnimTrigger::OnSlideEnter)
                        && e.animation.entrance.is_some())
                    .collect();
                if enter_elems.len() > 3 {
                    for (i, el) in enter_elems.iter().enumerate().skip(1) {
                        let mut patched = (*el).clone();
                        patched.animation.trigger = AnimTrigger::AutoAfterMs { ms: i as u32 * 150 };
                        patches.push(DeckPatch::UpsertElement { slide_id: slide.id.clone(), element: patched });
                    }
                }
            }
        }
        patches
    }
}

impl Default for DesignCriticAgent { fn default() -> Self { Self::new() } }

fn word_count(slide: &Slide) -> usize {
    slide.elements.iter().map(|e| match &e.content {
        ElementContent::Text { markdown } => markdown.split_whitespace().count(),
        _ => 0,
    }).sum()
}
```

### Step 5.3 — Run and commit

- [ ] `cargo test -p minion-presentation design_critic` — both tests pass.
- [ ] `cargo test -p minion-presentation` — full suite green.
- [ ] `cargo build -p minion-presentation`
- [ ] `git add crates/minion-presentation/src/agents/design_critic.rs crates/minion-presentation/src/agents/mod.rs`
- [ ] `git commit -m "feat(presentation): add DesignCriticAgent for word count and anim stagger patches"`

---

## Final Verification

```bash
cargo test -p minion-presentation
cargo clippy -p minion-presentation -- -D warnings
```

All tests must pass. Clippy must emit zero warnings.
