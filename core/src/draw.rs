//! Weighted booster draw. The RNG is injected: same seed => same result.
//!
//! Known limitation: `balanceColors` (color balancing of commons) is ignored for now;
//! it will need card colors (Scryfall/MTGJSON data).

use crate::booster::{BoosterConfig, BoosterSheet};
use rand::distributions::{Distribution, WeightedIndex};
use rand::Rng;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawnCard {
    pub uuid: String,
    pub foil: bool,
    pub sheet: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DrawError {
    #[error("draw impossible: {0}")]
    Weights(#[from] rand::distributions::WeightedError),
}

/// Opens a booster: picks a pack variant by weight, then draws from each sheet.
pub fn open_booster<R: Rng + ?Sized>(
    cfg: &BoosterConfig,
    rng: &mut R,
) -> Result<Vec<DrawnCard>, DrawError> {
    let variants = WeightedIndex::new(cfg.boosters.iter().map(|b| b.weight))?;
    let pack = &cfg.boosters[variants.sample(rng)];
    let mut out = Vec::new();
    for (slot, &count) in &pack.contents {
        let sheet = &cfg.sheets[slot];
        draw_from_sheet(slot, sheet, count as usize, rng, &mut out)?;
    }
    Ok(out)
}

fn draw_from_sheet<R: Rng + ?Sized>(
    name: &str,
    sheet: &BoosterSheet,
    count: usize,
    rng: &mut R,
    out: &mut Vec<DrawnCard>,
) -> Result<(), DrawError> {
    let push = |uuid: &String, out: &mut Vec<DrawnCard>| {
        out.push(DrawnCard { uuid: uuid.clone(), foil: sheet.foil, sheet: name.to_string() })
    };
    // "fixed" sheet (Jumpstart-style): every card is guaranteed.
    if sheet.fixed {
        sheet.cards.keys().for_each(|u| push(u, out));
        return Ok(());
    }
    let uuids: Vec<&String> = sheet.cards.keys().collect();
    let mut weights: Vec<u64> = sheet.cards.values().copied().collect();
    for _ in 0..count {
        let i = WeightedIndex::new(&weights)?.sample(rng);
        push(uuids[i], out);
        if !sheet.allow_duplicates {
            weights[i] = 0; // draw without replacement
        }
    }
    Ok(())
}
