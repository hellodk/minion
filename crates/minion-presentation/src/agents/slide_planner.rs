use std::sync::{Arc, atomic::AtomicU32};

use anyhow::Context as _;
use chrono::Utc;
use minion_llm::{JsonExtractRequest, LlmProvider};
use serde::Deserialize;
use tokio::task::JoinSet;

use crate::{
    agents::{agent_name, next_seq, AgentEvent, EventTx},
    schema::types::{
        AnimEffect, AnimPhase, AnimTrigger, AspectRatio, Deck, DeckMeta, DeckPatch, Element,
        ElementAnimation, ElementContent, ElementId, ElementStyle, LayoutKind, MasterSlide,
        PresentationContext, Section, SectionId, Slide, TextDirection, Theme,
    },
};

use super::storyteller::{StorySection, StorytellerOutput};

#[derive(Debug, Deserialize)]
struct LlmSlide {
    layout: String,
    headline: String,
    body: String,
    visual_spec: Option<String>,
    talking_points: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct LlmSectionResponse {
    slides: Vec<LlmSlide>,
}

const SLIDE_STRIDE_X: f64 = 2100.0;
const SECTION_STRIDE_Y: f64 = 1280.0;

const SYSTEM_PROMPT: &str = "You are a presentation slide designer. Generate slides for a \
narrative section. Use layouts: title, kpi, comparison, process, architecture, quote, timeline, \
storytelling. Return ONLY valid JSON.";

const EXAMPLE_JSON: &str = r#"{"slides":[{"layout":"title","headline":"The Big Idea","body":"One-sentence summary","visual_spec":null,"talking_points":["open with hook"]},{"layout":"kpi","headline":"10x Faster","body":"Benchmark results","visual_spec":"bar chart of latency","talking_points":["cite benchmarks"]}]}"#;

fn parse_layout(s: &str) -> LayoutKind {
    match s {
        "title" => LayoutKind::Title,
        "kpi" => LayoutKind::Kpi,
        "comparison" => LayoutKind::Comparison,
        "process" => LayoutKind::Process,
        "architecture" => LayoutKind::Architecture,
        "quote" => LayoutKind::Quote,
        "timeline" => LayoutKind::Timeline,
        _ => LayoutKind::Storytelling,
    }
}

fn placeholder_element(visual_spec: &str) -> Element {
    Element {
        id: ElementId::new(),
        content: ElementContent::Text {
            markdown: format!("[[VISUAL_PLACEHOLDER: {visual_spec}]]"),
        },
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
        z_index: 0,
        style: ElementStyle::default(),
        animation: ElementAnimation {
            entrance: None,
            exit: None,
            emphasis: None,
            trigger: AnimTrigger::OnSlideEnter,
        },
        user_asset_id: None,
        locked: false,
    }
}

fn text_element(markdown: String, x: f64, y: f64, width: f64, height: f64, z: u32) -> Element {
    Element {
        id: ElementId::new(),
        content: ElementContent::Text { markdown },
        x,
        y,
        width,
        height,
        z_index: z,
        style: ElementStyle::default(),
        animation: ElementAnimation {
            entrance: Some(AnimPhase {
                effect: AnimEffect::Fade,
                delay_ms: 0,
                duration_ms: 400,
                spring: None,
            }),
            exit: None,
            emphasis: None,
            trigger: AnimTrigger::OnSlideEnter,
        },
        user_asset_id: None,
        locked: false,
    }
}

pub struct SlidePlannerAgent {
    provider: Arc<dyn LlmProvider>,
}

impl SlidePlannerAgent {
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }

    pub fn new_with_provider(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }

    pub async fn run(
        &self,
        story: &StorytellerOutput,
        event_tx: &EventTx,
        seq: &AtomicU32,
    ) -> anyhow::Result<Deck> {
        let _ = event_tx.send(AgentEvent::Started {
            seq: next_seq(seq),
            agent: agent_name::SLIDE_PLANNER.to_string(),
        });

        let sections_data = story.sections.clone();
        let section_count = sections_data.len();
        let mut section_results: Vec<(usize, LlmSectionResponse)> =
            Vec::with_capacity(section_count);

        let mut chunk_start = 0;
        while chunk_start < section_count {
            let chunk_end = (chunk_start + 3).min(section_count);
            let mut join_set: JoinSet<anyhow::Result<(usize, LlmSectionResponse)>> =
                JoinSet::new();

            for idx in chunk_start..chunk_end {
                let section = sections_data[idx].clone();
                let provider = Arc::clone(&self.provider);
                let section_y = idx as f64 * SECTION_STRIDE_Y;
                join_set.spawn(async move {
                    let result = plan_section(&*provider, &section, idx, section_y).await?;
                    Ok((idx, result))
                });
            }

            while let Some(r) = join_set.join_next().await {
                section_results.push(r.context("join error")??);
            }
            chunk_start = chunk_end;
        }

        section_results.sort_by_key(|(i, _)| *i);

        let mut global_slide_index: u32 = 0;
        let mut sections: Vec<Section> = Vec::with_capacity(section_count);

        for (section_idx, llm_section) in section_results {
            let section_id = SectionId::new();
            let section_title = story.sections[section_idx].title.clone();
            let section_y = section_idx as f64 * SECTION_STRIDE_Y;
            let mut slides: Vec<Slide> = Vec::with_capacity(llm_section.slides.len());

            for (slide_idx, llm_slide) in llm_section.slides.iter().enumerate() {
                let canvas_x = slide_idx as f64 * SLIDE_STRIDE_X;
                let layout = parse_layout(&llm_slide.layout);
                let mut slide = Slide::new(section_id.clone(), canvas_x, section_y, layout);

                slide.elements.push(text_element(
                    format!("## {}", llm_slide.headline),
                    48.0,
                    80.0,
                    1824.0,
                    160.0,
                    1,
                ));
                slide.elements.push(text_element(
                    llm_slide.body.clone(),
                    48.0,
                    260.0,
                    1824.0,
                    600.0,
                    2,
                ));
                if let Some(spec) = &llm_slide.visual_spec {
                    slide.elements.push(placeholder_element(spec));
                }
                slide.speaker_notes.talking_points = llm_slide.talking_points.clone();

                let patch = DeckPatch::UpsertSlide {
                    section_id: section_id.clone(),
                    slide: slide.clone(),
                };
                let _ = event_tx.send(AgentEvent::SlideReady {
                    seq: next_seq(seq),
                    agent: agent_name::SLIDE_PLANNER.to_string(),
                    slide_index: global_slide_index,
                    patch,
                });
                global_slide_index += 1;
                slides.push(slide);
            }
            sections.push(Section { id: section_id, title: section_title, slides });
        }

        let play_order = sections
            .iter()
            .flat_map(|s| s.slides.iter().map(|sl| sl.id.clone()))
            .collect();

        let deck = Deck {
            meta: DeckMeta {
                title: story.title.clone(),
                author: String::new(),
                deck_revision: 1,
                schema_version: "1.0".into(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                aspect_ratio: AspectRatio::Ratio16x9,
                language: "en-US".into(),
                text_direction: TextDirection::Ltr,
                target_duration_mins: None,
                presentation_context: PresentationContext::LiveTalk,
            },
            theme: Theme::default(),
            master: MasterSlide { elements: vec![], background: None },
            assets: vec![],
            camera_path: vec![],
            sections,
            play_order,
        };

        let _ = event_tx.send(AgentEvent::Completed {
            seq: next_seq(seq),
            agent: agent_name::SLIDE_PLANNER.to_string(),
        });

        Ok(deck)
    }
}

async fn plan_section(
    provider: &dyn LlmProvider,
    section: &StorySection,
    section_idx: usize,
    section_y: f64,
) -> anyhow::Result<LlmSectionResponse> {
    let req = JsonExtractRequest {
        system_prompt: SYSTEM_PROMPT.to_string(),
        user_input: format!(
            "Section: {}\nPurpose: {}\nPacing: {}\nSlides: {}\ncanvas_y: {}",
            section.title, section.purpose, section.pacing, section.slide_count, section_y
        ),
        example_json: EXAMPLE_JSON.to_string(),
        model: None,
        temperature: Some(0.4),
    };
    let resp = provider
        .extract_json(req)
        .await
        .context("plan_section: extract_json failed")?;
    let result: LlmSectionResponse =
        serde_json::from_value(resp.parsed).context("plan_section: deserialize failed")?;
    let _ = section_idx; // used for spawning, not needed here
    Ok(result)
}
