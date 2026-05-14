use minion_presentation::schema::types::*;
use minion_presentation::schema::quaternion::*;
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

#[test]
fn identity_quaternion_gives_zero_euler() {
    let q = [1.0f64, 0.0, 0.0, 0.0];
    let (rx, ry, rz) = quaternion_to_euler_deg(&q);
    assert!((rx).abs() < 0.001, "rx={rx}");
    assert!((ry).abs() < 0.001, "ry={ry}");
    assert!((rz).abs() < 0.001, "rz={rz}");
}

#[test]
fn euler_roundtrip_simple_rotation() {
    let original_deg = (30.0f64, 0.0f64, 45.0f64);
    let q = euler_deg_to_quaternion(original_deg.0, original_deg.1, original_deg.2);
    let (rx, ry, rz) = quaternion_to_euler_deg(&q);
    assert!((rx - original_deg.0).abs() < 0.1, "rx expected {} got {}", original_deg.0, rx);
    assert!((ry - original_deg.1).abs() < 0.1, "ry expected {} got {}", original_deg.1, ry);
    assert!((rz - original_deg.2).abs() < 0.1, "rz expected {} got {}", original_deg.2, rz);
}

#[test]
fn quaternion_to_css_transform_identity() {
    let q = [1.0f64, 0.0, 0.0, 0.0];
    let css = quaternion_to_css_rotate3d(&q);
    assert!(css.contains("0deg") || css.contains("rotate3d(0,0,1,0"), "got: {css}");
}
