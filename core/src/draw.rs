//! Weighted booster draw. The RNG is injected: same seed => same result.
//!
//! Color balancing (`balanceColors`): MTGJSON only says the sheet "needs its colors
//! balanced". We follow the rule recommended by the data's author (magic-sealed-data):
//! first pick one card of each color, then pick the rest at random. Interpretation
//! choices, since the rule is not fully specified:
//! - a card counts for a color only if it is mono-colored in that color; colorless and
//!   multicolor cards only come from the random part;
//! - a color with no mono-colored card in the sheet is skipped;
//! - if the sheet has more colors than cards to draw, no balancing is applied;
//! - the drawn cards of a balanced sheet are shuffled, so position reveals nothing.
//!
//! Known bias: the author notes that weights would need adjusting for per-card
//! frequencies to match exactly (colorless cards come out less often than their weight
//! suggests). Balanced sheets are therefore validated by invariant, not by card rates.

use crate::booster::{BoosterConfig, BoosterSheet};
use crate::catalog::CardEntry;
use rand::distributions::{Distribution, WeightedIndex};
use rand::seq::SliceRandom;
use rand::Rng;
use std::collections::HashMap;

/// Color symbols in WUBRG order; the position is the bit index in the mask.
pub const COLORS: [&str; 5] = ["W", "U", "B", "R", "G"];

/// Colors of cards by uuid, needed to balance colors.
#[derive(Debug, Clone, Default)]
pub struct ColorIndex(HashMap<String, u8>);

impl ColorIndex {
    pub fn from_cards(cards: &[CardEntry]) -> Self {
        let mut index = Self::default();
        for c in cards {
            index.insert(&c.uuid, &c.colors);
        }
        index
    }

    pub fn insert<S: AsRef<str>>(&mut self, uuid: &str, colors: &[S]) {
        let mask = colors
            .iter()
            .filter_map(|c| COLORS.iter().position(|k| *k == c.as_ref()))
            .fold(0u8, |m, bit| m | (1 << bit));
        self.0.insert(uuid.to_string(), mask);
    }

    /// Index in `COLORS` if the card has exactly one color.
    fn mono_color(&self, uuid: &str) -> Option<usize> {
        let mask = *self.0.get(uuid)?;
        (mask.count_ones() == 1).then(|| mask.trailing_zeros() as usize)
    }
}

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
    colors: &ColorIndex,
    rng: &mut R,
) -> Result<Vec<DrawnCard>, DrawError> {
    let variants = WeightedIndex::new(cfg.boosters.iter().map(|b| b.weight))?;
    let pack = &cfg.boosters[variants.sample(rng)];
    let mut out = Vec::new();
    for (slot, &count) in &pack.contents {
        let sheet = &cfg.sheets[slot];
        draw_from_sheet(slot, sheet, count as usize, colors, rng, &mut out)?;
    }
    Ok(out)
}

fn draw_from_sheet<R: Rng + ?Sized>(
    name: &str,
    sheet: &BoosterSheet,
    count: usize,
    colors: &ColorIndex,
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
    let mut picked: Vec<usize> = Vec::with_capacity(count);

    if sheet.balance_colors {
        let present: Vec<usize> = (0..COLORS.len())
            .filter(|&c| {
                uuids.iter().zip(&weights).any(|(u, &w)| w > 0 && colors.mono_color(u) == Some(c))
            })
            .collect();
        if present.len() <= count {
            for c in present {
                let candidates: Vec<u64> = uuids
                    .iter()
                    .zip(&weights)
                    .map(|(u, &w)| if colors.mono_color(u) == Some(c) { w } else { 0 })
                    .collect();
                let i = WeightedIndex::new(&candidates)?.sample(rng);
                picked.push(i);
                if !sheet.allow_duplicates {
                    weights[i] = 0;
                }
            }
        }
    }
    while picked.len() < count {
        let i = WeightedIndex::new(&weights)?.sample(rng);
        picked.push(i);
        if !sheet.allow_duplicates {
            weights[i] = 0; // draw without replacement
        }
    }
    if sheet.balance_colors {
        picked.shuffle(rng);
    }
    picked.into_iter().for_each(|i| push(uuids[i], out));
    Ok(())
}
