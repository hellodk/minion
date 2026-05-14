use std::sync::{Arc, atomic::AtomicU32};

use anyhow::Context as _;
use minion_llm::{JsonExtractRequest, LlmProvider};
use serde::{Deserialize, Serialize};

use crate::{
    agents::{agent_name, next_seq, AgentEvent, EventTx},
    schema::types::GenerationConfig,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchFact {
    pub claim: String,
    pub source: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchOutput {
    pub audience: String,
    pub tone: String,
    pub language: String,
    pub key_themes: Vec<String>,
    pub facts: Vec<ResearchFact>,
    pub suggested_section_count: u32,
    pub target_duration_mins: Option<u32>,
}

const SYSTEM_PROMPT: &str = "You are a research analyst extracting structured information from content \
to be used in a presentation. Analyze the provided content and extract: target audience, tone, language, \
key themes, concrete facts with sources, suggested number of sections, and estimated presentation duration.\n\
Return ONLY valid JSON matching the provided schema.";

const EXAMPLE_JSON: &str = r#"{"audience":"engineering leadership","tone":"authoritative, concise","language":"en-US","key_themes":["scalability","migration"],"facts":[{"claim":"...","source":"doc page 1","confidence":0.9}],"suggested_section_count":5,"target_duration_mins":15}"#;

pub struct ResearchAgent {
    provider: Arc<dyn LlmProvider>,
}

impl ResearchAgent {
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }

    pub fn new_with_provider(provider: Arc<dyn LlmProvider>) -> Self {
        Self { provider }
    }

    pub async fn run(
        &self,
        corpus: String,
        _config: &GenerationConfig,
        event_tx: &EventTx,
        seq: &AtomicU32,
    ) -> anyhow::Result<ResearchOutput> {
        let _ = event_tx.send(AgentEvent::Started {
            seq: next_seq(seq),
            agent: agent_name::RESEARCH.to_string(),
        });

        let req = JsonExtractRequest {
            system_prompt: SYSTEM_PROMPT.to_string(),
            user_input: format!("Analyze this content for a presentation:\n\n{corpus}"),
            example_json: EXAMPLE_JSON.to_string(),
            model: None,
            temperature: Some(0.0),
        };

        let resp = self
            .provider
            .extract_json(req)
            .await
            .context("ResearchAgent: extract_json failed")?;
        let output: ResearchOutput = serde_json::from_value(resp.parsed)
            .context("ResearchAgent: failed to deserialize from LLM JSON")?;

        for fact in &output.facts {
            let _ = event_tx.send(AgentEvent::Progress {
                seq: next_seq(seq),
                agent: agent_name::RESEARCH.to_string(),
                data: format!("fact: {} (confidence {:.2})", fact.claim, fact.confidence),
            });
        }

        let _ = event_tx.send(AgentEvent::Completed {
            seq: next_seq(seq),
            agent: agent_name::RESEARCH.to_string(),
        });

        Ok(output)
    }
}
