// crates/minion-presentation/src/schema/validate.rs
use crate::schema::types::{Asset, CameraEasing, CameraStep, Deck, SlideId, SpringParams};
use std::collections::HashSet;

pub const ASSET_MAX_BYTES: u64 = 25 * 1024 * 1024;
pub const DECK_MAX_BYTES: u64  = 200 * 1024 * 1024;

pub type ValidationResult = Result<(), String>;

pub fn validate_asset_size(size_bytes: u64) -> ValidationResult {
    if size_bytes > ASSET_MAX_BYTES {
        return Err(format!(
            "asset size {:.1} MB exceeds maximum {} MB",
            size_bytes as f64 / 1_048_576.0,
            ASSET_MAX_BYTES / 1024 / 1024
        ));
    }
    Ok(())
}

pub fn validate_deck_total_size(total_bytes: u64) -> ValidationResult {
    if total_bytes > DECK_MAX_BYTES {
        return Err(format!(
            "deck bundle size {} MB exceeds maximum {} MB",
            total_bytes / 1024 / 1024,
            DECK_MAX_BYTES / 1024 / 1024
        ));
    }
    Ok(())
}

pub fn validate_spring(s: &SpringParams) -> ValidationResult {
    if s.stiffness < 1.0 || s.stiffness > 2000.0 {
        return Err(format!("stiffness {} out of range [1.0, 2000.0]", s.stiffness));
    }
    if s.damping < 0.1 || s.damping > 100.0 {
        return Err(format!("damping {} out of range [0.1, 100.0]", s.damping));
    }
    if s.mass < 0.1 || s.mass > 10.0 {
        return Err(format!("mass {} out of range [0.1, 10.0]", s.mass));
    }
    Ok(())
}

pub fn validate_camera_step(step: &CameraStep) -> ValidationResult {
    if step.zoom <= 0.0 {
        return Err(format!("zoom must be positive, got {}", step.zoom));
    }
    if let CameraEasing::Spring { stiffness, damping } = &step.easing {
        let p = SpringParams { stiffness: *stiffness, damping: *damping, mass: 1.0 };
        validate_spring(&p)?;
    }
    Ok(())
}

pub fn validate_asset(asset: &Asset) -> ValidationResult {
    validate_asset_size(asset.size_bytes)?;
    if asset.checksum_sha256.len() != 64 {
        return Err(format!(
            "checksum_sha256 must be 64 hex chars, got {}",
            asset.checksum_sha256.len()
        ));
    }
    if !asset.checksum_sha256.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("checksum_sha256 contains non-hex characters".into());
    }
    Ok(())
}

/// Validate that play_order references only slides that exist in the deck,
/// and that every slide in the deck appears in play_order.
pub fn validate_play_order(deck: &Deck) -> ValidationResult {
    let actual: HashSet<&SlideId> = deck.all_slides().map(|s| &s.id).collect();
    let ordered: HashSet<&SlideId> = deck.play_order.iter().collect();

    for id in &ordered {
        if !actual.contains(id) {
            return Err(format!(
                "play_order references slide {} which does not exist in any section",
                id.0
            ));
        }
    }
    for id in &actual {
        if !ordered.contains(id) {
            return Err(format!(
                "slide {} exists in sections but is missing from play_order",
                id.0
            ));
        }
    }
    if deck.play_order.len() != ordered.len() {
        return Err("play_order contains duplicate slide IDs".into());
    }
    Ok(())
}
