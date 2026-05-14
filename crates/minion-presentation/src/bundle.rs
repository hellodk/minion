use std::{
    io::{Read, Write},
    path::Path,
};

use anyhow::{Context, Result};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

use crate::schema::types::{Deck, DeckPatch};
use crate::schema::validate::validate_play_order;

const ENTRY: &str = "schema.json";

pub fn save_bundle(deck: &Deck, path: &Path) -> Result<()> {
    let file = std::fs::File::create(path)
        .with_context(|| format!("create {}", path.display()))?;
    let mut zip = ZipWriter::new(file);
    zip.start_file(
        ENTRY,
        SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated),
    )?;
    zip.write_all(&serde_json::to_vec(deck)?)?;
    zip.finish()?;
    Ok(())
}

pub fn load_bundle(path: &Path) -> Result<Deck> {
    let file = std::fs::File::open(path)
        .with_context(|| format!("open {}", path.display()))?;
    let mut zip = ZipArchive::new(file).context("parse ZIP")?;
    let mut entry = zip
        .by_name(ENTRY)
        .with_context(|| format!("{ENTRY} missing from archive"))?;
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
            for sec in &mut deck.sections {
                sec.slides.retain(|s| s.id != slide_id);
            }
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

pub fn validate_and_repair_play_order(deck: &mut Deck) {
    if validate_play_order(deck).is_err() {
        deck.play_order = deck.all_slides().map(|s| s.id.clone()).collect();
    }
}
