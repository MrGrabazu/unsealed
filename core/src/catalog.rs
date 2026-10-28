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
    /// Booster kind (of `boosters`) -> the box topper its booster box holds, if any.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub box_toppers: BTreeMap<String, BoxTopper>,
    /// The box toppers' boosters (MTGJSON "box-topper" configs, a pack of one or a few cards),
    /// by key ("box-topper", "box-topper-foil", "box-topper-zne" for another set's): opened
    /// like the others, never sold. Their cards are a section of the set's binder, outside
    /// its completion.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub topper_boosters: BTreeMap<String, BoosterConfig>,
}

/// Names Windows keeps for its devices, whatever the extension (before Windows 11): writing
/// "CON.json" (Conflux) writes to the console, and a game with none fails with "invalid
/// handle (os error 6)".
const RESERVED_NAMES: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3",
    "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Whether Windows reserves this file name (without its extension) for a device.
pub fn is_reserved(name: &str) -> bool {
    RESERVED_NAMES.iter().any(|r| r.eq_ignore_ascii_case(name))
}

/// The name of a set's file in `sets/`: its code, with "_" after a name Windows reserves
/// ("CON_.json" for Conflux).
pub fn set_file_name(code: &str) -> String {
    if is_reserved(code) { format!("{code}_.json") } else { format!("{code}.json") }
}

/// The set code of a file of `sets/` (see `set_file_name`); None for a file not named so (an
/// old "CON.json", left by a build made where Windows allowed it).
pub fn set_code_of_file(name: &str) -> Option<&str> {
    let stem = name.strip_suffix(".json")?;
    match stem.strip_suffix('_') {
        Some(code) if is_reserved(code) => Some(code),
        _ if is_reserved(stem) => None,
        _ => Some(stem),
    }
}

/// Bump when the compact files change in a way older data cannot satisfy: the app then
/// rebuilds its data at the next launch.
pub const DATA_FORMAT_VERSION: u32 = 13;

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

/// The kinds of a set's usual boosters: when a set has one, its other kinds (collector,
/// jumpstart, theme, faction…) do not define what completing it means.
pub const MAIN_KINDS: [&str; 4] = ["play", "draft", "set", "default"];

/// Whether a booster kind is a collector booster: "collector", or one of its editions
/// ("collector-special", LTR's Special Edition).
pub fn is_collector_kind(kind: &str) -> bool {
    kind == COLLECTOR_KIND || kind.strip_prefix(COLLECTOR_KIND).is_some_and(|rest| rest.starts_with('-'))
}

/// The kind a regional edition of a booster is made like: Japan's ("jp", the Japanese Draft
/// Booster; "set-jp", "collector-jp") are their English kind with a few cards of their own (War
/// of the Spark's alternate-art planeswalkers, Ikoria's Godzilla series, Strixhaven's Japanese
/// Mystical Archive). Any other kind is its own.
pub fn usual_kind(kind: &str) -> &str {
    match kind {
        "jp" => "draft",
        _ => kind.strip_suffix("-jp").unwrap_or(kind),
    }
}

/// The language a booster's cards are printed in, when it is not the player's: Japanese for
/// Japan's boosters (see `usual_kind`), whose cards are shown as they come out of it.
pub fn booster_language(kind: &str) -> Option<&'static str> {
    (kind == "jp" || kind.ends_with("-jp")).then_some("ja")
}

/// Whether a booster kind is one `data.json` asks for: listed as is, or matching an entry
/// ending with "*" ("theme-*": every theme booster, "theme-b", "theme-boros"…).
pub fn kind_wanted(kind: &str, wanted: &[&str]) -> bool {
    wanted.iter().any(|w| match w.strip_suffix('*') {
        Some(prefix) => kind.starts_with(prefix),
        None => kind == *w,
    })
}

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
    /// completing the set means): its usual ones (`MAIN_KINDS`) if it has any; else every
    /// kind but its collector boosters (Jumpstart); else its collector boosters (Doctor Who).
    pub fn is_main_booster(&self, kind: &str) -> bool {
        if self.boosters.keys().any(|k| MAIN_KINDS.contains(&k.as_str())) {
            MAIN_KINDS.contains(&kind)
        } else {
            !is_collector_kind(kind) || self.boosters.keys().all(|k| is_collector_kind(k))
        }
    }

    /// The config of one of the set's boosters, or of its box toppers' (`topper_boosters`).
    pub fn booster_config(&self, kind: &str) -> Option<&BoosterConfig> {
        self.boosters.get(kind).or_else(|| self.topper_boosters.get(kind))
    }

    /// Whether `kind` is a box topper's booster (never sold, earned with the boxes).
    pub fn is_topper(&self, kind: &str) -> bool {
        self.topper_boosters.contains_key(kind) && !self.boosters.contains_key(kind)
    }
}

/// What a booster box of one kind holds besides its boosters (MTGJSON `sealedProduct`): its
/// box topper packs. The game gives them every `boosters` boosters of that kind opened, as a
/// box would.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoxTopper {
    /// Boosters in the box (36 Play Boosters, 12 Collector Boosters…).
    pub boosters: u32,
    /// Its topper packs: their booster (a key of `SetFile::topper_boosters`) -> how many (two
    /// Expeditions in a Zendikar Rising Collector box, one elsewhere).
    pub packs: BTreeMap<String, u32>,
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
    /// The kind of its foil, when it is not the plain one: one of `FOIL_KINDS` (a galaxy
    /// foil, a surge foil…, MTGJSON `promoTypes`), or "etched" (an etched foil, its finish).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foil_kind: Option<String>,
}

/// The special foils told apart in the game, as MTGJSON and Scryfall name them in their
/// promo types: each has its badge, and most their own look (`foils.ts` of the app).
pub const FOIL_KINDS: [&str; 21] = [
    "galaxyfoil", "surgefoil", "doublerainbow", "textured", "fracturefoil", "halofoil", "oilslick", "gilded", "silverfoil",
    "manafoil", "neonink", "ripplefoil", "stepandcompleat", "raisedfoil", "firstplacefoil", "invisibleink", "facetfoil",
    "chocobotrackfoil", "cosmicfoil", "rainbowfoil", "singularityfoil",
];

/// The kind of foil of a printing (see `CardEntry::foil_kind`): its first promo type that is
/// a special foil, else "etched" for a printing etched but never plain foil.
pub fn foil_kind(promo_types: &[String], finishes: &[String]) -> Option<String> {
    let promo = promo_types.iter().find(|p| FOIL_KINDS.contains(&p.as_str()));
    let etched = finishes.iter().any(|f| f == "etched") && !finishes.iter().any(|f| f == "foil");
    promo.cloned().or_else(|| etched.then(|| "etched".to_string()))
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
    /// `SetFile::box_toppers`, here too: the game counts the boxes without reading every set.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub box_toppers: BTreeMap<String, BoxTopper>,
}

/// A card shown as the face of its set. Its art must be shown with its artist's name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeaturedCard {
    pub scryfall_id: String,
    pub name: String,
    pub artist: String,
}
