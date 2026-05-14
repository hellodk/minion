// crates/minion-presentation/src/schema/validate.rs
use crate::schema::types::{Asset, CameraEasing, CameraStep, SpringParams};

pub const ASSET_MAX_BYTES: u64 = 25 * 1024 * 1024;
pub const DECK_MAX_BYTES: u64  = 200 * 1024 * 1024;

pub type ValidationResult = Result<(), String>;

pub fn validate_asset_size(size_bytes: u64) -> ValidationResult {
    if size_bytes > ASSET_MAX_BYTES {
        return Err(format!(
            "asset size {} MB exceeds maximum {} MB",
            size_bytes / 1024 / 1024,
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
    Ok(())
}
