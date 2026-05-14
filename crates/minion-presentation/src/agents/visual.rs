use crate::agents::{agent_name, next_seq, AgentEvent, EventTx};
use crate::schema::types::*;
use crate::visual::{diagram_gen::generate_mermaid_dsl, svg_templates::template_for};
use std::sync::{atomic::AtomicU32, Arc};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

pub struct VisualAgent {
    #[allow(dead_code)]
    provider: Option<Arc<dyn minion_llm::LlmProvider>>,
}

impl VisualAgent {
    pub fn new_with_provider(p: Arc<dyn minion_llm::LlmProvider>) -> Self {
        Self { provider: Some(p) }
    }

    pub fn new_without_provider() -> Self {
        Self { provider: None }
    }

    pub async fn run(
        &self,
        deck: &mut Deck,
        event_tx: &EventTx,
        seq: &AtomicU32,
    ) -> anyhow::Result<()> {
        let _ = event_tx
            .send(AgentEvent::Started { seq: next_seq(seq), agent: agent_name::VISUAL.into() });
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
                        }
                    }
                }
            }
        }
        while let Some(Ok((si, li, content))) = set.join_next().await {
            let section_id = deck.sections[si].id.clone();
            let slide = &mut deck.sections[si].slides[li];
            for el in &mut slide.elements {
                if matches!(&el.content, ElementContent::Text { markdown } if markdown.starts_with("[[VISUAL_PLACEHOLDER:"))
                {
                    el.content = content.clone();
                    break;
                }
            }
            let slide_clone = slide.clone();
            let _ = event_tx.send(AgentEvent::SlideReady {
                seq: next_seq(seq),
                agent: agent_name::VISUAL.into(),
                slide_index: li as u32,
                patch: Box::new(DeckPatch::UpsertSlide {
                    section_id,
                    slide: slide_clone,
                }),
            });
        }
        let _ = event_tx
            .send(AgentEvent::Completed { seq: next_seq(seq), agent: agent_name::VISUAL.into() });
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
