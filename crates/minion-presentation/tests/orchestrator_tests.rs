#[test]
fn orchestrator_type_exists() {
    let _ = std::mem::size_of::<minion_presentation::orchestrator::Orchestrator>();
}

#[test]
fn bundle_db_smoke_test() {
    use chrono::Utc;
    use minion_db::in_memory;
    use minion_presentation::{
        bundle,
        db::PresentationDb,
        migrations,
        schema::types::{
            AspectRatio, Deck, DeckId, DeckMeta, MasterSlide, PresentationContext, TextDirection,
            Theme,
        },
    };

    // ── 1. In-memory DB + migrations ────────────────────────────────────────
    let db = in_memory().expect("in-memory DB");
    {
        let conn = db.get().expect("connection");
        migrations::run(&conn).expect("migrations");
    }

    // ── 2. Build a minimal Deck ─────────────────────────────────────────────
    let now = Utc::now();
    let deck = Deck {
        meta: DeckMeta {
            title: "Smoke Test Deck".into(),
            author: "tester".into(),
            deck_revision: 1,
            schema_version: "1.0".into(),
            created_at: now,
            updated_at: now,
            aspect_ratio: AspectRatio::Ratio16x9,
            language: "en".into(),
            text_direction: TextDirection::Ltr,
            target_duration_mins: None,
            presentation_context: PresentationContext::LiveTalk,
        },
        theme: Theme::default(),
        master: MasterSlide { elements: vec![], background: None },
        assets: vec![],
        camera_path: vec![],
        sections: vec![],
        play_order: vec![],
    };

    // ── 3. Save bundle to a temp dir ────────────────────────────────────────
    let tmp = tempfile::tempdir().expect("tempdir");
    let deck_id = DeckId::new();
    let bundle_path = tmp.path().join(format!("{}.mnpz", deck_id.0));
    bundle::save_bundle(&deck, &bundle_path).expect("save_bundle");
    assert!(bundle_path.exists(), "bundle file should exist on disk");

    // ── 4. Insert presentation in DB ────────────────────────────────────────
    let pdb = PresentationDb::new(db);
    let bundle_str = bundle_path.to_string_lossy().to_string();
    pdb.insert_presentation(&deck_id, &deck.meta.title, &bundle_str, None)
        .expect("insert_presentation");

    // ── 5. list_presentations returns 1 entry with the correct title ────────
    let summaries = pdb.list_presentations().expect("list_presentations");
    assert_eq!(summaries.len(), 1, "expected exactly one presentation");
    assert_eq!(summaries[0].title, "Smoke Test Deck");

    // ── 6. load_bundle round-trips the deck correctly ───────────────────────
    let loaded = bundle::load_bundle(&bundle_path).expect("load_bundle");
    assert_eq!(loaded.meta.title, deck.meta.title);
    assert_eq!(loaded.meta.author, deck.meta.author);
    assert_eq!(loaded.sections.len(), 0);
}
