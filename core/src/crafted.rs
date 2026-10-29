//! Boosters made up by the game for sets sold without any (Secret Lair, sold drop by drop):
//! no real booster gives their cards, so the game crafts one (`config/data.json`,
//! `craftedBoosters`). Their cards have no meaningful rarity (nearly all rare or mythic): each
//! gets the rarity of its price tier instead (the "rarity in the vault"), and the booster draws
//! a few cards from each tier, like a real booster from its rarity sheets. Its odds are ours,
//! not Wizards': the game says so wherever the booster is sold.

use crate::booster::{BoosterConfig, BoosterPack, BoosterSheet, ConfigError};
use crate::catalog::{PriceSet, SetFile};
use serde::Deserialize;
use std::collections::BTreeMap;

/// The rarities the tiers are named after, cheapest first.
pub const TIER_RARITIES: [&str; 4] = ["common", "uncommon", "rare", "mythic"];

/// A crafted booster's recipe.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CraftedBooster {
    /// Its booster kind ("secret-lair").
    pub kind: String,
    /// The price (in dollar cents, the cheapest finish of the card) from which a card is of a
    /// tier, by rarity ("uncommon": 800…); below every tier, a card is common, and so is a card
    /// without a price.
    pub tiers: BTreeMap<String, u32>,
    /// Its slots, drawn in this order.
    pub slots: Vec<CraftedSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CraftedSlot {
    /// Cards drawn (all of the same tier).
    pub count: u32,
    #[serde(default)]
    pub foil: bool,
    /// The tier of the slot, by weight ("uncommon": 65, "rare": 30, "mythic": 5).
    pub rarities: BTreeMap<String, u64>,
}

/// The tier of a card priced `cents` (see `CraftedBooster::tiers`).
fn tier(cents: Option<u32>, tiers: &BTreeMap<String, u32>) -> &'static str {
    let Some(cents) = cents else { return TIER_RARITIES[0] };
    TIER_RARITIES
        .iter()
        .rev()
        .find(|r| tiers.get(**r).is_some_and(|&from| cents >= from))
        .copied()
        .unwrap_or(TIER_RARITIES[0])
}

/// The price a card is tiered by: its cheapest finish, in dollars (else in euros).
fn tier_price(prices: Option<&PriceSet>) -> Option<u32> {
    let p = prices?;
    [p.usd, p.usd_foil].into_iter().flatten().min().or_else(|| [p.eur, p.eur_foil].into_iter().flatten().min())
}

/// The name of a tier's sheet ("craftedRare", "craftedRareFoil").
fn sheet_name(rarity: &str, foil: bool) -> String {
    let mut chars = rarity.chars();
    let title: String = chars.next().map(|c| c.to_ascii_uppercase()).into_iter().chain(chars).collect();
    format!("crafted{title}{}", if foil { "Foil" } else { "" })
}

/// Gives a set its crafted booster: each card (tokens aside) the rarity of its price tier, then
/// one sheet per tier and finish (a card in each finish it is printed in, etched counting as
/// foil), and a pack variant per choice of tier in each slot. A slot whose tier has no card in
/// its finish gives its weight to the others. Fails when no pack can be filled.
pub fn craft_booster(set: &mut SetFile, recipe: &CraftedBooster, prices: &BTreeMap<String, PriceSet>) -> Result<(), ConfigError> {
    for card in set.cards.iter_mut().filter(|c| !c.token) {
        card.rarity = tier(tier_price(prices.get(&card.uuid)), &recipe.tiers).to_string();
    }
    let mut sheets = BTreeMap::new();
    for rarity in TIER_RARITIES {
        for foil in [false, true] {
            let cards: BTreeMap<String, u64> = set
                .cards
                .iter()
                .filter(|c| !c.token && c.rarity == rarity)
                .filter(|c| c.finishes.iter().any(|f| if foil { f == "foil" || f == "etched" } else { f == "nonfoil" }))
                .map(|c| (c.uuid.clone(), 1))
                .collect();
            if !cards.is_empty() {
                let total_weight = cards.len() as u64;
                sheets.insert(
                    sheet_name(rarity, foil),
                    BoosterSheet { allow_duplicates: false, balance_colors: false, cards, fixed: false, foil, total_weight },
                );
            }
        }
    }
    // Every combination of the slots' tiers, weighted by the product of their weights.
    let mut packs = vec![BoosterPack { contents: BTreeMap::new(), weight: 1 }];
    for slot in &recipe.slots {
        let choices: Vec<(String, u64)> = slot
            .rarities
            .iter()
            .map(|(rarity, &weight)| (sheet_name(rarity, slot.foil), weight))
            .filter(|(name, weight)| *weight > 0 && sheets.get(name).is_some_and(|s| s.cards.len() >= slot.count as usize))
            .collect();
        if choices.is_empty() {
            return Err(ConfigError::Invalid(format!("crafted booster {}: a slot has no card to draw", recipe.kind)));
        }
        packs = packs
            .iter()
            .flat_map(|pack| {
                choices.iter().map(move |(name, weight)| {
                    let mut contents = pack.contents.clone();
                    *contents.entry(name.clone()).or_insert(0) += slot.count;
                    BoosterPack { contents, weight: pack.weight * weight }
                })
            })
            .collect();
    }
    // Two slots drawing from the same sheet must find enough cards for both.
    packs.retain(|pack| pack.contents.iter().all(|(name, &n)| sheets[name].cards.len() >= n as usize));
    sheets.retain(|name, _| packs.iter().any(|p| p.contents.contains_key(name)));
    let cfg = BoosterConfig {
        boosters_total_weight: packs.iter().map(|p| p.weight).sum(),
        boosters: packs,
        name: None,
        sheets,
        source_set_codes: Vec::new(),
    };
    if cfg.boosters.is_empty() {
        return Err(ConfigError::Invalid(format!("crafted booster {}: no pack can be filled", recipe.kind)));
    }
    cfg.validate()?;
    set.boosters.insert(recipe.kind.clone(), cfg);
    Ok(())
}
