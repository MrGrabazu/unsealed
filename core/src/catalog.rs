//! Compact data files produced by the data builder and consumed by the client.
//!
//! Layout of the published data:
//! - `manifest.json`   : small, always fetched first (see `update`)
//! - `sets/<CODE>.json`: one `SetFile` per set, downloaded on demand
//! - `prices.json`     : daily prices for every published card

use crate::booster::BoosterConfig;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetFile {
    pub code: String,
    pub name: String,
    pub release_date: String,
    pub set_type: String,
    /// Version of the MTGJSON build this file was generated from.
    pub source_version: String,
    pub cards: Vec<CardEntry>,
    /// Booster kind (e.g. "play") -> config.
    pub boosters: BTreeMap<String, BoosterConfig>,
}

/// Booster kind whose exclusive cards are extras, outside a set's completion target.
pub const COLLECTOR_KIND: &str = "collector";

impl SetFile {
    /// Whether a booster kind is one of the set's main boosters (those that define what
    /// completing the set means). A set offering nothing but collector boosters counts them.
    pub fn is_main_booster(&self, kind: &str) -> bool {
        kind != COLLECTOR_KIND || self.boosters.keys().all(|k| k == COLLECTOR_KIND)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CardEntry {
    pub uuid: String,
    /// Set the card belongs to. May differ from the file's set for cards drawn from
    /// other sets (e.g. Special Guests); completion tracking should ignore those.
    pub set_code: String,
    pub scryfall_id: String,
    pub name: String,
    pub number: String,
    pub rarity: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub colors: Vec<String>,
    pub finishes: Vec<String>,
    pub layout: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<String>,
    /// Illustrator, to credit the art wherever it is shown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
}

/// Prices in cents, as given by Scryfall.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceSet {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usd: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usd_foil: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eur: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eur_foil: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceFile {
    /// `updated_at` of the Scryfall bulk file the prices come from.
    pub updated_at: String,
    /// MTGJSON card uuid -> prices (the key used everywhere in the client).
    pub cards: BTreeMap<String, PriceSet>,
    /// "SET/kind" -> expected market value of one booster, computed from today's card
    /// prices. The client derives the booster price from it, so prices move with the market.
    #[serde(default)]
    pub boosters: BTreeMap<String, BoosterValue>,
}

/// Expected market value of a booster, in cents, per currency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoosterValue {
    pub usd: u32,
    pub eur: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub generated_at: String,
    pub prices_updated_at: String,
    pub sets: BTreeMap<String, ManifestSet>,
    /// Booster kinds the data was built for (the configured list, not what each set has):
    /// when the configuration asks for other kinds, the data must be rebuilt.
    #[serde(default)]
    pub booster_kinds: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSet {
    pub name: String,
    pub release_date: String,
    pub source_version: String,
    pub boosters: Vec<String>,
    /// The card whose art illustrates the set in the shop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub featured: Option<FeaturedCard>,
}

/// A card shown as the face of its set. Its art must be shown with its artist's name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeaturedCard {
    pub scryfall_id: String,
    pub name: String,
    pub artist: String,
}
