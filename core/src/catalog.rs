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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSet {
    pub name: String,
    pub release_date: String,
    pub source_version: String,
    pub boosters: Vec<String>,
}
