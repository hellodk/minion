use minion_presentation::bundle::{apply_patch, load_bundle, save_bundle};
use minion_presentation::schema::types::*;

fn minimal_deck(title: &str) -> Deck {
    Deck {
        meta: DeckMeta {
            title: title.to_string(),
            author: "Test Author".into(),
            deck_revision: 1,
            schema_version: "1.0".into(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
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
        sections: vec![],
        play_order: vec![],
    }
}

#[test]
fn bundle_roundtrip_preserves_title() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("deck.mnpz");

    let deck = minimal_deck("Hello World");
    save_bundle(&deck, &path).expect("save_bundle should succeed");

    let loaded = load_bundle(&path).expect("load_bundle should succeed");
    assert_eq!(loaded.meta.title, "Hello World");
}

#[test]
fn bundle_missing_file_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("nonexistent.mnpz");

    let result = load_bundle(&path);
    assert!(result.is_err(), "loading a missing file should return an error");
}

#[test]
fn apply_patch_set_meta_updates_title() {
    let mut deck = minimal_deck("Original Title");

    let new_meta = DeckMeta {
        title: "Updated Title".into(),
        author: "New Author".into(),
        deck_revision: 2,
        schema_version: "1.0".into(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        aspect_ratio: AspectRatio::Ratio16x9,
        language: "en-US".into(),
        text_direction: TextDirection::Ltr,
        target_duration_mins: Some(20),
        presentation_context: PresentationContext::AsyncShare,
    };

    apply_patch(&mut deck, DeckPatch::SetMeta { meta: new_meta });
    assert_eq!(deck.meta.title, "Updated Title");
    assert_eq!(deck.meta.author, "New Author");
    assert_eq!(deck.meta.deck_revision, 2);
}
