use minion_presentation::visual::svg_sanitizer::sanitize_svg;
use minion_presentation::visual::svg_templates::template_for;
use minion_presentation::visual::chart_gen::generate_chart_spec;
use minion_presentation::visual::diagram_gen::generate_mermaid_dsl;
use minion_presentation::agents::visual::VisualAgent;
use minion_presentation::agents::design_critic::DesignCriticAgent;
use minion_presentation::schema::types::*;
use std::sync::atomic::AtomicU32;
use chrono::Utc;

#[test]
fn valid_svg_passes_through() {
    let input = r#"<svg xmlns="http://www.w3.org/2000/svg"><rect width="10" height="10"/></svg>"#;
    assert!(sanitize_svg(input).unwrap().contains("<rect"));
}

#[test]
fn script_tag_stripped() {
    let input =
        r#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script><rect/></svg>"#;
    let out = sanitize_svg(input).unwrap();
    assert!(!out.contains("script") && out.contains("<rect"));
}

#[test]
fn invalid_use_href_rejected() {
    let input =
        r#"<svg xmlns="http://www.w3.org/2000/svg"><use href="javascript:alert(1)"/></svg>"#;
    assert!(sanitize_svg(input).is_err());
}

#[test]
fn on_event_attr_stripped() {
    let input = r#"<svg xmlns="http://www.w3.org/2000/svg"><rect onclick="evil()" width="10" height="10"/></svg>"#;
    assert!(!sanitize_svg(input).unwrap().contains("onclick"));
}

#[test]
fn fe_gaussian_blur_capped_at_20() {
    let input = r#"<svg xmlns="http://www.w3.org/2000/svg"><filter><feGaussianBlur stdDeviation="999"/></filter></svg>"#;
    assert!(sanitize_svg(input).unwrap().contains("stdDeviation=\"20\""));
}

// Template tests
#[test]
fn arrow_template() {
    assert!(template_for("arrow").contains("<svg"));
}

#[test]
fn process_template() {
    assert!(template_for("process").contains("<svg"));
}

#[test]
fn kpi_template() {
    assert!(template_for("kpi").contains("<svg"));
}

#[test]
fn comparison_template() {
    assert!(template_for("comparison").contains("<svg"));
}

#[test]
fn default_template() {
    assert!(template_for("unknown_xyz").contains("<svg"));
}

// Chart/diagram tests
#[test]
fn chart_spec_has_type_key() {
    assert_eq!(generate_chart_spec("monthly revenue", "bar")["type"], "bar");
}

#[test]
fn chart_spec_has_data_key() {
    assert!(generate_chart_spec("q1 sales", "pie").get("data").is_some());
}

#[test]
fn mermaid_flowchart_keyword() {
    let d = generate_mermaid_dsl("login flow", "flowchart");
    assert!(d.trim_start().starts_with("flowchart") || d.trim_start().starts_with("graph"));
}

#[test]
fn mermaid_sequence_keyword() {
    assert!(generate_mermaid_dsl("api lifecycle", "sequence")
        .trim_start()
        .starts_with("sequenceDiagram"));
}

#[test]
fn mermaid_default_is_graph() {
    let d = generate_mermaid_dsl("process", "unknown");
    assert!(d.trim_start().starts_with("graph") || d.trim_start().starts_with("flowchart"));
}

// VisualAgent test
fn placeholder_deck(n: usize) -> Deck {
    let sid = SectionId::new();
    let slides = (0..n)
        .map(|i| {
            let mut sl = Slide::new(sid.clone(), i as f64 * 1920.0, 0.0, LayoutKind::Blank);
            sl.elements.push(Element {
                id: ElementId::new(),
                content: ElementContent::Text {
                    markdown: "[[VISUAL_PLACEHOLDER: diagram | login flow]]".into(),
                },
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 400.0,
                z_index: 1,
                style: ElementStyle::default(),
                animation: ElementAnimation {
                    entrance: None,
                    exit: None,
                    emphasis: None,
                    trigger: AnimTrigger::OnSlideEnter,
                },
                user_asset_id: None,
                locked: false,
            });
            sl
        })
        .collect();
    Deck {
        meta: DeckMeta {
            title: "T".into(),
            author: "t".into(),
            deck_revision: 1,
            schema_version: "1.0".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
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
        sections: vec![Section { id: sid, title: "S".into(), slides }],
        play_order: vec![],
    }
}

#[tokio::test]
async fn visual_agent_fills_placeholder_slides() {
    let mut deck = placeholder_deck(2);
    let (tx, _rx) = tokio::sync::broadcast::channel(32);
    let seq = AtomicU32::new(0);
    VisualAgent::new_without_provider().run(&mut deck, &tx, &seq).await.unwrap();
    for slide in deck.sections[0].slides.iter() {
        for el in &slide.elements {
            if let ElementContent::Text { markdown } = &el.content {
                assert!(
                    !markdown.starts_with("[[VISUAL_PLACEHOLDER:"),
                    "placeholder not filled: {markdown}"
                );
            }
        }
    }
}

// DesignCriticAgent tests
fn wordy_slide(sid: SectionId, words: usize) -> Slide {
    let mut sl = Slide::new(sid, 0.0, 0.0, LayoutKind::Blank);
    sl.elements.push(Element {
        id: ElementId::new(),
        content: ElementContent::Text {
            markdown: "word ".repeat(words),
        },
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 400.0,
        z_index: 1,
        style: ElementStyle::default(),
        animation: ElementAnimation {
            entrance: None,
            exit: None,
            emphasis: None,
            trigger: AnimTrigger::OnSlideEnter,
        },
        user_asset_id: None,
        locked: false,
    });
    sl
}

fn anim_slide(sid: SectionId, n: usize) -> Slide {
    let mut sl = Slide::new(sid, 0.0, 0.0, LayoutKind::Blank);
    for _ in 0..n {
        sl.elements.push(Element {
            id: ElementId::new(),
            content: ElementContent::Text {
                markdown: "x".into(),
            },
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 40.0,
            z_index: 1,
            style: ElementStyle::default(),
            animation: ElementAnimation {
                entrance: Some(AnimPhase {
                    effect: AnimEffect::Fade,
                    delay_ms: 0,
                    duration_ms: 400,
                    spring: None,
                }),
                exit: None,
                emphasis: None,
                trigger: AnimTrigger::OnSlideEnter,
            },
            user_asset_id: None,
            locked: false,
        });
    }
    sl
}

#[test]
fn design_critic_detects_wordcount_over_80() {
    let mut deck = placeholder_deck(0);
    let section_id = deck.sections[0].id.clone();
    deck.sections[0].slides.push(wordy_slide(section_id, 90));
    let patches = DesignCriticAgent::new().review(&deck);
    assert!(patches
        .iter()
        .any(|p| matches!(p, DeckPatch::DeleteSlide { .. })));
}

#[test]
fn design_critic_staggers_excess_enter_animations() {
    let mut deck = placeholder_deck(0);
    let section_id = deck.sections[0].id.clone();
    deck.sections[0].slides.push(anim_slide(section_id, 4));
    let patches = DesignCriticAgent::new().review(&deck);
    assert!(patches
        .iter()
        .any(|p| matches!(p, DeckPatch::UpsertElement { .. })));
}
