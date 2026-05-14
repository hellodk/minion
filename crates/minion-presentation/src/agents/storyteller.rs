use std::sync::{Arc, atomic::AtomicU32};

use anyhow::Context as _;
use minion_llm::{JsonExtractRequest, LlmProvider};
use serde::{Deserialize, Serialize};

use crate::agents::{agent_name, next_seq, AgentEvent, EventTx};

use super::research::ResearchOutput;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorySection {
    pub title: String,
    pub slide_count: u32,
    pub purpose: String,
    pub pacing: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorytellerOutput {
    pub title: String,
    pub hook: String,
    pub sections: Vec<StorySection>,
    pub closing_cta: String,
    pub camera_narrative: String,
}

const SYSTEM_PROMPT: &str = "You are a master storyteller and presentation architect. \
Given research findings, create a compelling narrative structure with clear sections, \
emotional arc, and memorable hook. Return ONLY valid JSON.";

const EXAMPLE_JSON: &str = r#"{"title":"Scaling Without Limits","hook":"What if your biggest constraint disappeared?","sections":[{"title":"The Problem","slide_count":3,"purpose":"establish context","pacing":"slow, grounding"},{"title":"The Solution","slide_count":4,"purpose":"reveal answer","pacing":"building energy"}],"closing_cta":"Let's build this together","camera_narrative":"Open wide, zoom in on breakthrough, pull back for vision"}"#;

pub struct StorytellerAgent {
    provider: Arc<dyn LlmProvider>,
}

impl StorytellerAgent {
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }

    pub fn new_with_provider(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }

    pub async fn run(
        &self,
        research: &ResearchOutput,
        event_tx: &EventTx,
        seq: &AtomicU32,
    ) -> anyhow::Result<StorytellerOutput> {
        let _ = event_tx.send(AgentEvent::Started {
            seq: next_seq(seq),
            agent: agent_name::STORYTELLER.to_string(),
        });

        let user_input = format!(
            "Create a narrative for:\nAudience: {}\nTone: {}\nThemes: {}\nSections: {}\nDuration: {} mins",
            research.audience,
            research.tone,
            research.key_themes.join(", "),
            research.suggested_section_count,
            research
                .target_duration_mins
                .map_or("unspecified".to_string(), |d| d.to_string())
        );

        let req = JsonExtractRequest {
            system_prompt: SYSTEM_PROMPT.to_string(),
            user_input,
            example_json: EXAMPLE_JSON.to_string(),
            model: None,
            temperature: Some(0.3),
        };

        let resp = self
            .provider
            .extract_json(req)
            .await
            .context("StorytellerAgent: extract_json failed")?;
        let output: StorytellerOutput = serde_json::from_value(resp.parsed)
            .context("StorytellerAgent: deserialize failed")?;

        for section in &output.sections {
            let _ = event_tx.send(AgentEvent::Progress {
                seq: next_seq(seq),
                agent: agent_name::STORYTELLER.to_string(),
                data: format!("section: \"{}\" ({} slides)", section.title, section.slide_count),
            });
        }

        let _ = event_tx.send(AgentEvent::Completed {
            seq: next_seq(seq),
            agent: agent_name::STORYTELLER.to_string(),
        });

        Ok(output)
    }
}
