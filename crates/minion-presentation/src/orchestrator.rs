use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
};

use anyhow::{bail, Result};
use tokio::sync::watch;

use crate::{
    agents::{
        design_critic::DesignCriticAgent, research::ResearchAgent,
        slide_planner::SlidePlannerAgent, storyteller::StorytellerAgent, visual::VisualAgent,
        AgentEvent, EventTx,
    },
    bundle,
    db::PresentationDb,
    input::{process_all, InputSource},
    router::{PresentationRouter, RoutingTask},
    schema::types::{DeckId, GenerationConfig},
};

pub struct Orchestrator {
    pub db: PresentationDb,
    pub router: PresentationRouter,
    pub data_dir: PathBuf,
}

impl Orchestrator {
    pub fn new(db: PresentationDb, router: PresentationRouter, data_dir: PathBuf) -> Self {
        Self { db, router, data_dir }
    }

    pub async fn generate(
        &self,
        _session_id: &str,
        inputs: Vec<InputSource>,
        config: GenerationConfig,
        event_tx: EventTx,
        cancel_rx: watch::Receiver<bool>,
    ) -> Result<DeckId> {
        let seq = AtomicU32::new(0);
        let provider: Arc<dyn minion_llm::LlmProvider> =
            Arc::from(self.router.provider_for(RoutingTask::ResearchExtraction));

        // Process inputs into a text corpus
        let corpus = process_all(inputs, provider.as_ref()).await?;
        if *cancel_rx.borrow() {
            bail!("interrupted after input processing");
        }

        // Research
        let research = ResearchAgent::new_with_provider(Arc::clone(&provider))
            .run(corpus, &config, &event_tx, &seq)
            .await?;
        if *cancel_rx.borrow() {
            bail!("interrupted after research");
        }

        // Storyteller
        let story = StorytellerAgent::new_with_provider(Arc::from(
            self.router.provider_for(RoutingTask::NarrativeGeneration),
        ))
        .run(&research, &event_tx, &seq)
        .await?;
        if *cancel_rx.borrow() {
            bail!("interrupted after storyteller");
        }

        // Slide planner
        let mut deck = SlidePlannerAgent::new_with_provider(Arc::from(
            self.router.provider_for(RoutingTask::SlideContentPlanning),
        ))
        .run(&story, &event_tx, &seq)
        .await?;
        if *cancel_rx.borrow() {
            bail!("interrupted after slide planner");
        }

        // Visual (diagram/chart placeholders — no LLM required)
        VisualAgent::new_without_provider().run(&mut deck, &event_tx, &seq).await?;

        // Design critic
        for patch in DesignCriticAgent::new().review(&deck) {
            bundle::apply_patch(&mut deck, patch);
        }
        bundle::validate_and_repair_play_order(&mut deck);

        // Persist
        let deck_id = DeckId::new();
        std::fs::create_dir_all(&self.data_dir)?;
        let bundle_path = self.data_dir.join(format!("{}.mnpz", deck_id.0));
        bundle::save_bundle(&deck, &bundle_path)?;
        let bundle_str = bundle_path.to_string_lossy().to_string();
        self.db.insert_presentation(&deck_id, &deck.meta.title, &bundle_str, None)?;
        self.db.update_slide_count(&deck_id, deck.slide_count())?;

        let _ = event_tx.send(AgentEvent::StreamComplete {
            seq: seq.load(Ordering::Relaxed),
            deck_id: deck_id.0.to_string(),
        });
        Ok(deck_id)
    }
}
