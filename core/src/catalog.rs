//! Compact data files produced by the data builder and consumed by the client.
//!
//! Layout of the published data:
//! - `manifest.json`   : small, always fetched first (see `update`)
//! - `sets/<CODE>.json`: one `SetFile` per set, downloaded on demand
//! - `prices.json`     : daily prices for every published card
//! - `artists.json`, `guests.json`: indexes across the sets (by artist, guest series)

use crate::booster::BoosterConfig;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

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
    /// The set's name in other languages (code -> name), when it was translated.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub names: BTreeMap<String, String>,
    /// Entries merged into another card: foil printings into their card (see
    /// `CardEntry::foil_scryfall_id`), a two-faced token's face b into its face a
    /// (`merge_token_faces`). Their uuid -> the card's: games saved before own them under the
    /// old uuid.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub foil_aliases: BTreeMap<String, String>,
}

/// Bump when the compact files change in a way older data cannot satisfy: the app then
/// rebuilds its data at the next launch.
pub const DATA_FORMAT_VERSION: u32 = 11;

/// Languages kept for card and set names, as (MTGJSON language name, code). Cards printed
/// in other languages, or never translated, stay in English.
pub const FOREIGN_LANGUAGES: &[(&str, &str)] =
    &[("French", "fr"), ("German", "de"), ("Spanish", "es"), ("Italian", "it"), ("Japanese", "ja")];

/// A card's printing in another language.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignCard {
    pub name: String,
    /// The translated printing on Scryfall, for its image; `None` shows the English one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scryfall_id: Option<String>,
}

/// Name of the artist index in the data directory.
pub const ARTISTS_FILE: &str = "artists.json";

/// The cards of each artist across the sets offered (`artists.json`, built with the sets):
/// the collections by artist. Each card is listed once, with the set file holding it (a card
/// can come from another set's boosters, e.g. a Special Guest).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtistIndex {
    pub artists: BTreeMap<String, Vec<ArtistCard>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtistCard {
    pub uuid: String,
    /// The set file (`sets/<set>.json`) the card was found in.
    pub set: String,
}

/// Name of the guest series index in the data directory.
pub const GUESTS_FILE: &str = "guests.json";

/// The series whose cards come in other sets' boosters (`guests.json`, built with the sets):
/// Special Guests, The List, Masterpieces, a commander set's cards in collector boosters…
/// Each has its own binder, unless it is a set offered (its binder exists already). Sets
/// that keep their guests (`SetFile::keeps_its_guests`) are left out.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuestIndex {
    pub series: BTreeMap<String, GuestSeries>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestSeries {
    pub name: String,
    /// The name in other languages (code -> name), when it was translated.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub names: BTreeMap<String, String>,
    pub release_date: String,
    /// Scryfall's SVG of the series' symbol (black, to be shown unaltered).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_svg_uri: Option<String>,
    pub cards: Vec<GuestCard>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuestCard {
    pub uuid: String,
    /// The sets whose boosters give it; the first one's file (`sets/<set>.json`) holds it.
    pub boosters: Vec<String>,
}

/// Booster kind whose exclusive cards are extras, outside a set's completion target.
pub const COLLECTOR_KIND: &str = "collector";

/// See `SetFile::keeps_its_guests`.
const KEEPS_GUESTS_RATIO: usize = 3;

impl SetFile {
    /// The cards its boosters can give (their uuids).
    pub fn in_boosters(&self) -> HashSet<&str> {
        self.boosters
            .values()
            .flat_map(|cfg| cfg.sheets.values())
            .flat_map(|sheet| sheet.cards.keys().map(String::as_str))
            .collect()
    }

    /// The cards of other sets its boosters give (Special Guests, The List…). Tokens and art
    /// cards, filed under sets of their own too, are not guests.
    pub fn guest_cards(&self) -> Vec<&CardEntry> {
        let in_boosters = self.in_boosters();
        self.cards
            .iter()
            .filter(|c| c.set_code != self.code && !c.token && !c.art && in_boosters.contains(c.uuid.as_str()))
            .collect()
    }

    /// Whether the set keeps its guests in its own binder: its boosters give at least
    /// `KEEPS_GUESTS_RATIO` times more cards of other sets than of its own (the Mystery
    /// Boosters, nearly all The List; sets with many List cards, DMU or MOM, stay under
    /// twice). Elsewhere, the guests go to the binders of their series.
    pub fn keeps_its_guests(&self) -> bool {
        let in_boosters = self.in_boosters();
        let own = self
            .cards
            .iter()
            .filter(|c| c.set_code == self.code && !c.token && !c.art && in_boosters.contains(c.uuid.as_str()))
            .count();
        self.guest_cards().len() >= KEEPS_GUESTS_RATIO * own.max(1)
    }

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
    /// Translated printings, by language code (see `FOREIGN_LANGUAGES`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub foreign: BTreeMap<String, ForeignCard>,
    /// A serialized printing (MTGJSON promo type "serialized"): numbered copies, a limited
    /// print run (see `LimitedConfig`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub serialized: bool,
    /// One of the set's tokens (its own set code, e.g. "TLTR"; rarity "token").
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub token: bool,
    /// One of the set's art cards (its art series, e.g. "ALTR"; rarity "art"). Its gold
    /// signed version is drawn as its "foil" finish.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub art: bool,
    /// The foil version, when MTGJSON lists it as a printing of its own ("★" numbers: 7th to
    /// 10th Edition, Unhinged…), merged into this card (`merge_foil_printings`): the Scryfall
    /// printing its foil price comes from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foil_scryfall_id: Option<String>,
    /// A full-art printing (MTGJSON `isFullArt`), for the bets.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub full_art: bool,
    /// A special frame: borderless, showcase or extended art (MTGJSON `borderColor`,
    /// `frameEffects`), for the bets.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub special_frame: bool,
    /// Types and supertypes in lower case ("basic", "land", "creature"…), for the bets.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub types: Vec<String>,
    /// MTGJSON `colorIdentity` (a land's colors, for the bets).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub color_identity: Vec<String>,
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
    /// The printing has no price of its own: these are the best prices of the other
    /// printings of the same card (see `prices_from_scryfall_jsonl`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub estimated: bool,
}

impl PriceSet {
    pub fn is_empty(&self) -> bool {
        self.usd.is_none() && self.usd_foil.is_none() && self.eur.is_none() && self.eur_foil.is_none()
    }

    /// The highest of both, price by price.
    fn max(self, other: Self) -> Self {
        let max = |a: Option<u32>, b: Option<u32>| a.max(b);
        Self {
            usd: max(self.usd, other.usd),
            usd_foil: max(self.usd_foil, other.usd_foil),
            eur: max(self.eur, other.eur),
            eur_foil: max(self.eur_foil, other.eur_foil),
            estimated: false,
        }
    }

    /// An estimate from the best prices of the other printings of a card. A finish they
    /// have no price for takes the other finish's (better than no price at all).
    pub fn estimate_from(best: Self) -> Self {
        Self {
            usd: best.usd.or(best.usd_foil),
            usd_foil: best.usd_foil.or(best.usd),
            eur: best.eur.or(best.eur_foil),
            eur_foil: best.eur_foil.or(best.eur),
            estimated: true,
        }
    }

    pub fn merge_best(&mut self, other: &Self) {
        *self = self.max(*other);
    }
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
    /// The set selection the data was built for, likewise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_sets: Option<SetSelection>,
    /// Set types kept when every set is requested, likewise.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub set_types: Vec<String>,
    /// Requested sets ("CODE") or booster kinds ("CODE/kind") left out, with the reason
    /// (no wanted booster, invalid data, failed simulation...). They are not retried
    /// until the data is rebuilt for another reason.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub skipped: BTreeMap<String, String>,
    /// `DATA_FORMAT_VERSION` of the build (0 for data older than the versioning).
    #[serde(default)]
    pub format_version: u32,
}

/// Which sets the game offers: an explicit list of codes, or `"all"` (every set of the
/// configured types that has a wanted booster).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SetSelection {
    List(Vec<String>),
    All(AllSets),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AllSets {
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSet {
    pub name: String,
    /// The name in other languages (code -> name), when it was translated.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub names: BTreeMap<String, String>,
    pub release_date: String,
    pub source_version: String,
    pub boosters: Vec<String>,
    /// The card whose art illustrates the set in the shop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub featured: Option<FeaturedCard>,
    /// Scryfall's SVG of the set's expansion symbol (black, to be shown unaltered).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_svg_uri: Option<String>,
}

/// A card shown as the face of its set. Its art must be shown with its artist's name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeaturedCard {
    pub scryfall_id: String,
    pub name: String,
    pub artist: String,
}
