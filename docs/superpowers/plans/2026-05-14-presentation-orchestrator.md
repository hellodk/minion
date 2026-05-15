# Presentation Module — Sub-Plan 2e: Orchestrator + Bundle + IPC

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the full pipeline together — bundle storage, orchestrator coordination, Tauri IPC commands — so that a text input goes in through the Tauri layer and a persisted deck comes out.

**Architecture:** Orchestrator runs agents sequentially, streams events via broadcast channel, checks a watch channel for cancellation between agents. Decks stored as ZIP bundles in the user data dir. Tauri commands spawn orchestrator tasks and return session IDs immediately.

**Tech Stack:** Rust, tokio (spawn, watch), zip, serde_json, minion-presentation agents, minion-db, Tauri AppState.

---

## Task 1 — Bundle (`crates/minion-presentation/src/bundle.rs`)

**Files:** `src/bundle.rs` (create), `src/lib.rs` (add `pub mod bundle;`), `tests/bundle_tests.rs` (create)

### Failing test

```rust
// tests/bundle_tests.rs
use minion_presentation::{bundle, schema::types::*};
use tempfile::tempdir;

fn minimal_deck(title: &str) -> Deck {
    Deck {
        meta: DeckMeta { title: title.to_string(), subtitle: None, author: "t".into(),
            date: None, tags: vec![], language: "en-US".into(),
            aspect_ratio: AspectRatio::Widescreen, target_duration_mins: None,
            presentation_context: PresentationContext::LiveTalk, slide_count_hint: None },
        theme: Theme::default(), master: MasterSlide::default(),
        assets: vec![], camera_path: vec![], sections: vec![], play_order: vec![],
    }
}

#[test]
fn bundle_roundtrip_preserves_title() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.mnpz");
    bundle::save_bundle(&minimal_deck("My Deck"), &path).unwrap();
    assert_eq!(bundle::load_bundle(&path).unwrap().meta.title, "My Deck");
}

#[test]
fn bundle_missing_file_errors() {
    assert!(bundle::load_bundle(std::path::Path::new("/no/such.mnpz")).is_err());
}

#[test]
fn apply_patch_set_meta_updates_title() {
    let mut deck = minimal_deck("Old");
    let mut m = deck.meta.clone(); m.title = "New".into();
    bundle::apply_patch(&mut deck, DeckPatch::SetMeta { meta: m });
    assert_eq!(deck.meta.title, "New");
}
```

Run: `cargo test -p minion-presentation bundle` — fails (module missing).

### Implement

```rust
// src/bundle.rs
use std::{io::{Read, Write}, path::Path};
use anyhow::{Context, Result};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};
use crate::schema::types::{Deck, DeckPatch};

const ENTRY: &str = "schema.json";

pub fn save_bundle(deck: &Deck, path: &Path) -> Result<()> {
    let file = std::fs::File::create(path)
        .with_context(|| format!("create {}", path.display()))?;
    let mut zip = ZipWriter::new(file);
    zip.start_file(ENTRY, SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated))?;
    zip.write_all(&serde_json::to_vec(deck)?)?;
    zip.finish()?;
    Ok(())
}

pub fn load_bundle(path: &Path) -> Result<Deck> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("open {}", path.display()))?;
    let mut zip = ZipArchive::new(file).context("parse ZIP")?;
    let mut entry = zip.by_name(ENTRY)
        .with_context(|| format!("{ENTRY} missing"))?;
    let mut buf = Vec::new();
    entry.read_to_end(&mut buf)?;
    serde_json::from_slice(&buf).context("deserialize deck")
}

pub fn apply_patch(deck: &mut Deck, patch: DeckPatch) {
    match patch {
        DeckPatch::SetMeta { meta } => deck.meta = meta,
        DeckPatch::SetTheme { theme } => deck.theme = theme,
        DeckPatch::SetPlayOrder { order } => deck.play_order = order,
        DeckPatch::SetCameraPath { path } => deck.camera_path = path,
        DeckPatch::UpsertAsset { asset } => {
            match deck.assets.iter_mut().find(|a| a.id == asset.id) {
                Some(a) => *a = asset,
                None => deck.assets.push(asset),
            }
        }
        DeckPatch::UpsertSlide { section_id, slide } => {
            if let Some(sec) = deck.sections.iter_mut().find(|s| s.id == section_id) {
                match sec.slides.iter_mut().find(|s| s.id == slide.id) {
                    Some(s) => *s = slide,
                    None => sec.slides.push(slide),
                }
            }
        }
        DeckPatch::DeleteSlide { slide_id } => {
            for sec in &mut deck.sections { sec.slides.retain(|s| s.id != slide_id); }
            deck.play_order.retain(|id| id != &slide_id);
        }
        DeckPatch::UpsertElement { slide_id, element } => {
            'outer: for sec in &mut deck.sections {
                for slide in &mut sec.slides {
                    if slide.id == slide_id {
                        match slide.elements.iter_mut().find(|e| e.id == element.id) {
                            Some(e) => *e = element,
                            None => slide.elements.push(element),
                        }
                        break 'outer;
                    }
                }
            }
        }
        DeckPatch::DeleteElement { slide_id, element_id } => {
            for sec in &mut deck.sections {
                for slide in &mut sec.slides {
                    if slide.id == slide_id {
                        slide.elements.retain(|e| e.id != element_id);
                        return;
                    }
                }
            }
        }
    }
}
```

### Run + commit

```bash
cargo test -p minion-presentation bundle
git add crates/minion-presentation/src/bundle.rs crates/minion-presentation/src/lib.rs \
        crates/minion-presentation/tests/bundle_tests.rs
git commit -m "feat(presentation): add bundle save/load and apply_patch"
```

---

## Task 2 — Orchestrator (`crates/minion-presentation/src/orchestrator.rs`)

**Files:** `src/orchestrator.rs` (create), `src/lib.rs` (add `pub mod orchestrator;`)

### Failing test

```rust
// tests/orchestrator_tests.rs — compile-time existence check
#[test]
fn orchestrator_type_exists() {
    let _ = std::mem::size_of::<minion_presentation::orchestrator::Orchestrator>();
}
```

Run: `cargo test -p minion-presentation orchestrator` — fails (module missing).

### Implement

```rust
// src/orchestrator.rs
use std::{path::PathBuf, sync::{Arc, atomic::AtomicU32}};
use anyhow::{bail, Result};
use tokio::sync::watch;

use crate::{
    agents::{AgentEvent, EventTx,
        design_critic::DesignCriticAgent, research::ResearchAgent,
        slide_planner::SlidePlannerAgent, storyteller::StorytellerAgent, visual::VisualAgent},
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
        session_id: &str,
        inputs: Vec<InputSource>,
        config: GenerationConfig,
        event_tx: EventTx,
        cancel_rx: watch::Receiver<bool>,
    ) -> Result<DeckId> {
        let seq = AtomicU32::new(0);

        let corpus = process_all(inputs,
            self.router.provider_for(RoutingTask::ResearchExtraction).as_ref()).await?;

        let research = ResearchAgent::new_with_provider(
            Arc::from(self.router.provider_for(RoutingTask::ResearchExtraction)))
            .run(corpus, &config, &event_tx, &seq).await?;

        if *cancel_rx.borrow() { bail!("interrupted after research"); }

        let story = StorytellerAgent::new_with_provider(
            Arc::from(self.router.provider_for(RoutingTask::NarrativeGeneration)))
            .run(&research, &event_tx, &seq).await?;

        if *cancel_rx.borrow() { bail!("interrupted after storyteller"); }

        let mut deck = SlidePlannerAgent::new_with_provider(
            Arc::from(self.router.provider_for(RoutingTask::SlideContentPlanning)))
            .run(&story, &event_tx, &seq).await?;

        VisualAgent::new_with_provider(
            Arc::from(self.router.provider_for(RoutingTask::SvgGeneration)))
            .run(&mut deck, &event_tx, &seq).await?;

        for patch in DesignCriticAgent::new().review(&deck) {
            bundle::apply_patch(&mut deck, patch);
        }

        let deck_id = DeckId::new();
        std::fs::create_dir_all(&self.data_dir)?;
        let bundle_path = self.data_dir.join(format!("{}.mnpz", deck_id.0));
        bundle::save_bundle(&deck, &bundle_path)?;

        let slide_count = deck.slide_count();
        let bundle_str = bundle_path.to_string_lossy().to_string();
        self.db.insert_presentation(&deck_id, &deck.meta.title, &bundle_str, None)?;
        self.db.update_slide_count(&deck_id, slide_count)?;

        let _ = event_tx.send(AgentEvent::StreamComplete {
            seq: seq.load(std::sync::atomic::Ordering::Relaxed),
            deck_id: deck_id.0.to_string(),
        });
        Ok(deck_id)
    }
}
```

### Run + commit

```bash
cargo build -p minion-presentation
cargo test -p minion-presentation orchestrator
git add crates/minion-presentation/src/orchestrator.rs crates/minion-presentation/src/lib.rs
git commit -m "feat(presentation): add Orchestrator — sequential agent pipeline with cancel support"
```

---

## Task 3 — AppState + IPC commands

**Files:** `src-tauri/src/state.rs`, `src-tauri/src/presentation_commands.rs`

### AppState additions (`state.rs`)

Add two fields to `pub struct AppState` (after `presentation_db`):

```rust
pub cancel_senders: tokio::sync::Mutex<
    std::collections::HashMap<String, tokio::sync::watch::Sender<bool>>>,
pub orchestrator: std::sync::Arc<minion_presentation::orchestrator::Orchestrator>,
```

In `AppState::new()`, after `let presentation_db = PresentationDb::new(db.clone());`:

```rust
let presentations_dir = data_dir.join("presentations");
let orchestrator = {
    use minion_presentation::{orchestrator::Orchestrator, router::{PresentationRouter, RouterConfig}};
    std::sync::Arc::new(Orchestrator::new(
        presentation_db.clone(),
        PresentationRouter::new(RouterConfig::default()),
        presentations_dir,
    ))
};
```

In the `Ok(Self { ... })` literal add:

```rust
cancel_senders: tokio::sync::Mutex::new(std::collections::HashMap::new()),
orchestrator,
```

### IPC command implementations (`presentation_commands.rs`)

Replace the file completely:

```rust
// src-tauri/src/presentation_commands.rs
use minion_presentation::{bundle, input::InputSource,
    schema::types::{DeckId, DeckPatch, DeckSummary, GenerationConfig}};
use std::sync::Arc;
use tauri::{AppHandle, State};
use tokio::sync::RwLock;
use crate::state::AppState;

type AppStateHandle<'a> = State<'a, Arc<RwLock<AppState>>>;

#[tauri::command]
pub async fn start_presentation_generation(
    inputs: serde_json::Value,
    config: GenerationConfig,
    state: AppStateHandle<'_>,
    _app: AppHandle,
) -> Result<String, String> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
    let (event_tx, _) = tokio::sync::broadcast::channel(256);
    let sources: Vec<InputSource> = serde_json::from_value(inputs).map_err(|e| e.to_string())?;

    let guard = state.read().await;
    guard.cancel_senders.lock().await.insert(session_id.clone(), cancel_tx);
    let orchestrator = Arc::clone(&guard.orchestrator);
    let sid = session_id.clone();

    tokio::spawn(async move {
        if let Err(e) = orchestrator.generate(&sid, sources, config, event_tx, cancel_rx).await {
            tracing::error!("generation failed session={sid}: {e:#}");
        }
    });
    Ok(session_id)
}

#[tauri::command]
pub async fn interrupt_generation(
    session_id: String,
    _after_agent: String,
    _instruction: String,
    state: AppStateHandle<'_>,
) -> Result<(), String> {
    let guard = state.read().await;
    if let Some(tx) = guard.cancel_senders.lock().await.remove(&session_id) {
        let _ = tx.send(true);
    }
    Ok(())
}

#[tauri::command]
pub async fn get_deck(id: String, state: AppStateHandle<'_>) -> Result<serde_json::Value, String> {
    let guard = state.read().await;
    let deck_id = DeckId(uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?);
    let path_str = guard.presentation_db.get_bundle_path(&deck_id)
        .map_err(|e| e.to_string())?.ok_or_else(|| format!("deck {id} not found"))?;
    let deck = bundle::load_bundle(std::path::Path::new(&path_str)).map_err(|e| e.to_string())?;
    serde_json::to_value(deck).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_deck_patch(
    id: String, patches: Vec<DeckPatch>, state: AppStateHandle<'_>,
) -> Result<(), String> {
    let guard = state.read().await;
    let deck_id = DeckId(uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?);
    let path_str = guard.presentation_db.get_bundle_path(&deck_id)
        .map_err(|e| e.to_string())?.ok_or_else(|| format!("deck {id} not found"))?;
    let path = std::path::Path::new(&path_str);
    let mut deck = bundle::load_bundle(path).map_err(|e| e.to_string())?;
    for patch in patches { bundle::apply_patch(&mut deck, patch); }
    bundle::save_bundle(&deck, path).map_err(|e| e.to_string())?;
    guard.presentation_db.update_slide_count(&deck_id, deck.slide_count())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_presentations(state: AppStateHandle<'_>) -> Result<Vec<DeckSummary>, String> {
    state.read().await.presentation_db.list_presentations().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_presentation(
    _id: String, _format: String, _output_path: String, state: AppStateHandle<'_>,
) -> Result<serde_json::Value, String> {
    let _guard = state.read().await;
    Err("export not yet implemented — filled in Export sub-plan".into())
}
```

### Run + commit

```bash
cargo build --workspace
git add src-tauri/src/state.rs src-tauri/src/presentation_commands.rs
git commit -m "feat(presentation): wire orchestrator into AppState and fill IPC command stubs"
```

---

## Task 4 — Integration smoke test

**File:** `crates/minion-presentation/tests/orchestrator_tests.rs`

```rust
// Integration smoke: real in-memory DB + temp dir, no LLM — validates bundle+DB contract.
use minion_db::Database;
use minion_presentation::{bundle, db::PresentationDb, migrations,
    schema::types::{AspectRatio, Deck, DeckId, DeckMeta, MasterSlide,
                    PresentationContext, Theme}};
use tempfile::tempdir;

fn make_pdb() -> PresentationDb {
    let db = Database::new(":memory:", 1).unwrap();
    { let conn = db.get().unwrap(); migrations::run(&conn).unwrap(); }
    PresentationDb::new(db)
}

fn minimal_deck(title: &str) -> Deck {
    Deck {
        meta: DeckMeta { title: title.to_string(), subtitle: None, author: "t".into(),
            date: None, tags: vec![], language: "en-US".into(),
            aspect_ratio: AspectRatio::Widescreen, target_duration_mins: None,
            presentation_context: PresentationContext::LiveTalk, slide_count_hint: None },
        theme: Theme::default(), master: MasterSlide::default(),
        assets: vec![], camera_path: vec![], sections: vec![], play_order: vec![],
    }
}

#[test]
fn smoke_bundle_and_db_round_trip() {
    let dir = tempdir().unwrap();
    let pdb = make_pdb();
    let deck = minimal_deck("Smoke Test Deck");
    let deck_id = DeckId::new();
    let bundle_path = dir.path().join(format!("{}.mnpz", deck_id.0));

    bundle::save_bundle(&deck, &bundle_path).unwrap();
    pdb.insert_presentation(&deck_id, &deck.meta.title,
        &bundle_path.to_string_lossy(), None).unwrap();
    pdb.update_slide_count(&deck_id, deck.slide_count()).unwrap();

    // list_presentations returns 1 entry
    let list = pdb.list_presentations().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].title, "Smoke Test Deck");

    // bundle round-trips correctly
    let loaded = bundle::load_bundle(&bundle_path).unwrap();
    assert_eq!(loaded.meta.title, "Smoke Test Deck");

    // get_bundle_path returns the stored path
    let stored = pdb.get_bundle_path(&deck_id).unwrap().unwrap();
    assert!(std::path::Path::new(&stored).exists());
}
```

### Run + commit

```bash
cargo test -p minion-presentation orchestrator
cargo test --workspace
git add crates/minion-presentation/tests/orchestrator_tests.rs
git commit -m "test(presentation): add orchestrator integration smoke test"
```

---

## Completion checklist

- [ ] Task 1: `bundle.rs` — `save_bundle`, `load_bundle`, `apply_patch` — 3 unit tests pass
- [ ] Task 2: `orchestrator.rs` — `Orchestrator::new` + `generate()` — compiles clean, existence test passes
- [ ] Task 3: `state.rs` + `presentation_commands.rs` — `cancel_senders`, `orchestrator` in AppState; 4 IPC stubs filled
- [ ] Task 4: smoke test — DB entry persists, bundle round-trips, `list_presentations` returns 1 entry
- [ ] `cargo clippy --workspace -- -D warnings` passes
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo test --workspace` green (623+ tests)
