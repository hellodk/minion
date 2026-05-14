use minion_presentation::schema::types::*;
use uuid::Uuid;

#[test]
fn deck_serializes_and_deserializes() {
    let deck = Deck {
        meta: DeckMeta {
            title: "Test Deck".into(),
            author: "Test".into(),
            deck_revision: 1,
            schema_version: "1.0".into(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            aspect_ratio: AspectRatio::Ratio16x9,
            language: "en-US".into(),
            text_direction: TextDirection::Ltr,
            target_duration_mins: Some(10),
            presentation_context: PresentationContext::LiveTalk,
        },
        theme: Theme::default(),
        master: MasterSlide { elements: vec![], background: None },
        assets: vec![],
        camera_path: vec![],
        sections: vec![],
        play_order: vec![],
    };
    let json = serde_json::to_string(&deck).expect("serialize");
    let back: Deck = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.meta.title, "Test Deck");
    assert_eq!(back.meta.schema_version, "1.0");
}

#[test]
fn layout_kind_roundtrip() {
    let lk = LayoutKind::Kpi;
    let s = serde_json::to_string(&lk).unwrap();
    assert_eq!(s, r#""kpi""#);
    let back: LayoutKind = serde_json::from_str(&s).unwrap();
    assert_eq!(back, LayoutKind::Kpi);
}

#[test]
fn element_animation_trigger_by_id() {
    let id = ElementId(Uuid::new_v4());
    let trigger = AnimTrigger::AfterElement { element_id: id.clone() };
    let json = serde_json::to_string(&trigger).unwrap();
    let back: AnimTrigger = serde_json::from_str(&json).unwrap();
    match back {
        AnimTrigger::AfterElement { element_id: got } => assert_eq!(got, id),
        _ => panic!("wrong variant"),
    }
}
