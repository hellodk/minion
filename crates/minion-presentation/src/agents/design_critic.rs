use crate::schema::types::*;

pub struct DesignCriticAgent;

impl DesignCriticAgent {
    pub fn new() -> Self {
        Self
    }

    pub fn review(&self, deck: &Deck) -> Vec<DeckPatch> {
        let mut patches = Vec::new();
        for section in &deck.sections {
            for slide in &section.slides {
                let enter_elems: Vec<&Element> = slide
                    .elements
                    .iter()
                    .filter(|e| {
                        matches!(e.animation.trigger, AnimTrigger::OnSlideEnter)
                            && e.animation.entrance.is_some()
                    })
                    .collect();
                if enter_elems.len() > 3 {
                    for (i, el) in enter_elems.iter().enumerate().skip(1) {
                        let mut patched = (*el).clone();
                        patched.animation.trigger = AnimTrigger::AutoAfterMs {
                            ms: i as u32 * 150,
                        };
                        patches.push(DeckPatch::UpsertElement {
                            slide_id: slide.id.clone(),
                            element: patched,
                        });
                    }
                }
            }
        }
        patches
    }
}

impl Default for DesignCriticAgent {
    fn default() -> Self {
        Self::new()
    }
}

