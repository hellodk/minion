pub mod research;
pub mod slide_planner;
pub mod storyteller;

use crate::schema::types::DeckPatch;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentEvent {
    Started        { seq: u32, agent: String },
    Progress       { seq: u32, agent: String, data: String },
    SlideReady     { seq: u32, agent: String, slide_index: u32, patch: DeckPatch },
    Completed      { seq: u32, agent: String },
    Error          { seq: u32, agent: String, message: String, recoverable: bool },
    StreamComplete { seq: u32, deck_id: String },
    StreamError    { seq: u32, message: String },
}

pub type EventTx = tokio::sync::broadcast::Sender<AgentEvent>;

pub fn next_seq(counter: &AtomicU32) -> u32 {
    counter.fetch_add(1, Ordering::Relaxed)
}

pub mod agent_name {
    pub const RESEARCH: &str = "research";
    pub const STORYTELLER: &str = "storyteller";
    pub const SLIDE_PLANNER: &str = "slide_planner";
    pub const VISUAL: &str = "visual";
    pub const DESIGN_CRITIC: &str = "design_critic";
}
