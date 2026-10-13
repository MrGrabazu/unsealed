//! Conversion of raw MTGJSON / Scryfall data into the compact catalog files.

use crate::booster::{BoosterConfig, ConfigError};
use crate::catalog::{CardEntry, FeaturedCard, ForeignCard, PriceSet, SetFile, FOREIGN_LANGUAGES};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::io::BufRead;

/// A single-set MTGJSON file (`<CODE>.json`).
#[derive(Deserialize)]
struct RawFile {
    meta: RawMeta,
    data: RawSet,
}

/// MTGJSON `AllPrintings.json`: every set, keyed by code. Only the fields the game
/// needs are kept in memory.
#[derive(Deserialize)]
pub struct AllPrintings {
    pub meta: RawMeta,
    pub data: BTreeMap<String, RawSet>,
}

#[derive(Deserialize)]
pub struct RawMeta {
    pub version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawSet {
    pub code: String,
    pub name: String,
    pub release_date: String,
    #[serde(rename = "type")]
    pub set_type: String,
    #[serde(default)]
    pub is_online_only: bool,
    /// The set's name by MTGJSON language name; often missing (null).
    #[serde(default)]
    translations: BTreeMap<String, Option<String>>,
    cards: Vec<RawCard>,
    #[serde(default)]
    pub booster: BTreeMap<String, BoosterConfig>,
}

impl RawSet {
    /// Whether the set has at least one of the wanted booster kinds.
    pub fn has_any_booster(&self, kinds: &[&str]) -> bool {
        kinds.iter().any(|k| self.booster.contains_key(*k))
    }

    /// Codes of the other sets whose cards the kept booster configs draw from
    /// (MTGJSON `sourceSetCodes`, e.g. a Special Guests sheet in a Play Booster).
    pub fn source_set_codes(&self, kinds: &[&str]) -> Result<Vec<String>, ConfigError> {
        let mut codes = Vec::new();
        for kind in kept_kinds(&self.booster, kinds)? {
            for c in &self.booster[kind].source_set_codes {
                if *c != self.code && !codes.contains(c) {
                    codes.push(c.clone());
                }
            }
        }
        Ok(codes)
    }
}

/// Reads `AllPrintings.json` (already decompressed) without holding the whole text in memory.
pub fn read_all_printings<R: std::io::Read>(reader: R) -> Result<AllPrintings, ConfigError> {
    Ok(serde_json::from_reader(std::io::BufReader::new(reader))?)
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RawCard {
    uuid: String,
    set_code: String,
    name: String,
    number: String,
    rarity: String,
    #[serde(default)]
    colors: Vec<String>,
    finishes: Vec<String>,
    layout: String,
    side: Option<String>,
    #[serde(default)]
    artist: Option<String>,
    identifiers: RawIdentifiers,
    #[serde(default)]
    foreign_data: Vec<RawForeign>,
}

#[derive(Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct RawIdentifiers {
    scryfall_id: Option<String>,
}

/// A translated printing of a card (MTGJSON `foreignData`).
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RawForeign {
    language: String,
    name: String,
    #[serde(default)]
    identifiers: RawIdentifiers,
}

/// The language code kept for an MTGJSON language name, if it is one of `FOREIGN_LANGUAGES`.
fn language_code(name: &str) -> Option<&'static str> {
    FOREIGN_LANGUAGES.iter().find(|(n, _)| *n == name).map(|(_, code)| *code)
}

fn to_entry(c: &RawCard) -> Result<CardEntry, ConfigError> {
    let scryfall_id = c.identifiers.scryfall_id.clone().ok_or_else(|| {
        ConfigError::Invalid(format!("card `{}` ({}) has no scryfallId", c.name, c.uuid))
    })?;
    let foreign = c
        .foreign_data
        .iter()
        .filter_map(|f| {
            let code = language_code(&f.language)?;
            Some((code.to_string(), ForeignCard { name: f.name.clone(), scryfall_id: f.identifiers.scryfall_id.clone() }))
        })
        .collect();
    let c = c.clone();
    Ok(CardEntry {
        foreign,
        uuid: c.uuid,
        set_code: c.set_code,
        scryfall_id,
        name: c.name,
        number: c.number,
        rarity: c.rarity,
        colors: c.colors,
        finishes: c.finishes,
        layout: c.layout,
        side: c.side,
        artist: c.artist,
    })
}

/// The set's chase card: its most valuable own card from its main boosters (the
/// non-foil price, else the foil one), which illustrates the set. Cards without an
/// artist cannot be credited and are skipped; with no price at all, the first mythic
/// or rare by collector number is used.
pub fn featured_card(set: &SetFile, prices: &BTreeMap<String, PriceSet>) -> Option<FeaturedCard> {
    let in_main: HashSet<&str> = set
        .boosters
        .iter()
        .filter(|(kind, _)| set.is_main_booster(kind))
        .flat_map(|(_, cfg)| cfg.sheets.values())
        .flat_map(|sheet| sheet.cards.keys().map(String::as_str))
        .collect();
    let price = |c: &CardEntry| prices.get(&c.uuid).and_then(|p| p.usd.or(p.usd_foil)).unwrap_or(0);
    let rank = |c: &CardEntry| match c.rarity.as_str() {
        "mythic" => 2,
        "rare" => 1,
        _ => 0,
    };
    let candidates = set
        .cards
        .iter()
        .filter(|c| c.set_code == set.code && c.artist.is_some() && in_main.contains(c.uuid.as_str()));
    // Highest price, then highest rarity, then lowest collector number.
    let best = candidates.min_by(|a, b| {
        (price(b), rank(b))
            .cmp(&(price(a), rank(a)))
            .then_with(|| number_order(&a.number).cmp(&number_order(&b.number)))
    })?;
    Some(FeaturedCard {
        scryfall_id: best.scryfall_id.clone(),
        name: best.name.clone(),
        artist: best.artist.clone()?,
    })
}

/// Sort key for collector numbers such as "12", "12★" or "A-12": leading digits first.
fn number_order(number: &str) -> (u32, &str) {
    let digits: String = number.chars().take_while(char::is_ascii_digit).collect();
    (digits.parse().unwrap_or(u32::MAX), number)
}

/// The wanted booster kinds this set actually has, in the wanted order. Sets differ
/// (older sets have draft boosters, not play boosters; not every set has a collector
/// booster), so a kind the set lacks is skipped. Fails only when none is available.
fn kept_kinds<'a>(booster: &BTreeMap<String, BoosterConfig>, kinds: &[&'a str]) -> Result<Vec<&'a str>, ConfigError> {
    let kept: Vec<&str> = kinds.iter().copied().filter(|k| booster.contains_key(*k)).collect();
    if kept.is_empty() {
        return Err(ConfigError::UnknownBooster(kinds.join(", ")));
    }
    Ok(kept)
}

/// Codes of the other sets whose cards the kept booster configs draw from
/// (MTGJSON `sourceSetCodes`, e.g. a Special Guests sheet in a Play Booster).
/// Their raw files must be passed to `set_from_mtgjson`.
pub fn source_set_codes(json: &str, kinds: &[&str]) -> Result<Vec<String>, ConfigError> {
    let raw: RawFile = serde_json::from_str(json)?;
    raw.data.source_set_codes(kinds)
}

/// `compact_set` on single-set MTGJSON files (`<CODE>.json`).
pub fn set_from_mtgjson(json: &str, kinds: &[&str], extra_sets: &[&str]) -> Result<SetFile, ConfigError> {
    let raw: RawFile = serde_json::from_str(json)?;
    let extras = extra_sets.iter().map(|e| Ok(serde_json::from_str::<RawFile>(e)?.data)).collect::<Result<Vec<_>, ConfigError>>()?;
    compact_set(&raw.data, &raw.meta.version, kinds, &extras.iter().collect::<Vec<_>>())
}

/// Builds the compact set file of a raw MTGJSON set, keeping the booster kinds listed
/// in `kinds` that the set has. Every kept config is validated, and every card
/// referenced by a sheet must exist either in the set or in `extra_sets` (the sets
/// returned by `RawSet::source_set_codes`). Cards from other sets are embedded only
/// when a kept sheet references them; they keep their own `set_code`, so completion
/// tracking can ignore them.
pub fn compact_set(raw: &RawSet, version: &str, kinds: &[&str], extra_sets: &[&RawSet]) -> Result<SetFile, ConfigError> {
    let code = &raw.code;
    let mut entries = raw.cards.iter().map(to_entry).collect::<Result<Vec<_>, _>>()?;
    let own: HashSet<&str> = raw.cards.iter().map(|c| c.uuid.as_str()).collect();

    let mut boosters = BTreeMap::new();
    let mut foreign_needed: BTreeMap<String, String> = BTreeMap::new(); // uuid -> where it is referenced
    for kind in kept_kinds(&raw.booster, kinds)? {
        let cfg = raw.booster[kind].clone();
        cfg.validate()?;
        for (sheet_name, sheet) in &cfg.sheets {
            for uuid in sheet.cards.keys().filter(|u| !own.contains(u.as_str())) {
                foreign_needed.entry(uuid.clone()).or_insert_with(|| format!("{code}/{kind}: sheet `{sheet_name}`"));
            }
        }
        boosters.insert(kind.to_string(), cfg);
    }
    for card in extra_sets.iter().flat_map(|s| &s.cards) {
        if foreign_needed.remove(&card.uuid).is_some() {
            entries.push(to_entry(card)?);
        }
    }
    if let Some((uuid, place)) = foreign_needed.into_iter().next() {
        return Err(ConfigError::Invalid(format!("{place} references unknown card {uuid}")));
    }

    Ok(SetFile {
        code: code.clone(),
        name: raw.name.clone(),
        release_date: raw.release_date.clone(),
        set_type: raw.set_type.clone(),
        source_version: version.to_string(),
        cards: entries,
        boosters,
        names: raw
            .translations
            .iter()
            .filter_map(|(language, name)| Some((language_code(language)?.to_string(), name.clone()?)))
            .collect(),
    })
}

/// Parses a decimal price string ("0.05", "12", "1.5") into cents without floats.
pub fn parse_cents(s: &str) -> Option<u32> {
    let (whole, frac) = s.split_once('.').unwrap_or((s, ""));
    let whole: u32 = whole.parse().ok()?;
    let frac_cents: u32 = match frac.len() {
        0 => 0,
        1 => frac.parse::<u32>().ok()? * 10,
        2 => frac.parse().ok()?,
        _ => return None,
    };
    whole.checked_mul(100)?.checked_add(frac_cents)
}

/// Re-keys prices by MTGJSON uuid, the key the client uses everywhere. A Scryfall id can
/// match several uuids (the faces of a double-faced card): each one gets the same prices.
pub fn prices_by_uuid(
    by_scryfall_id: &BTreeMap<String, PriceSet>,
    cards: &[CardEntry],
) -> BTreeMap<String, PriceSet> {
    cards
        .iter()
        .filter_map(|c| by_scryfall_id.get(&c.scryfall_id).map(|p| (c.uuid.clone(), *p)))
        .collect()
}

#[derive(Deserialize)]
struct ScryfallCard {
    id: String,
    #[serde(default)]
    prices: ScryfallPrices,
}

#[derive(Deserialize, Default)]
struct ScryfallPrices {
    usd: Option<String>,
    usd_foil: Option<String>,
    eur: Option<String>,
    eur_foil: Option<String>,
}

/// Reads a Scryfall bulk file (one JSON card per line) and returns the prices
/// of the wanted card ids only.
pub fn prices_from_scryfall_jsonl<R: BufRead>(
    reader: R,
    wanted: &HashSet<String>,
) -> Result<BTreeMap<String, PriceSet>, ConfigError> {
    let mut out = BTreeMap::new();
    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let card: ScryfallCard = serde_json::from_str(line)?;
        if !wanted.contains(&card.id) {
            continue;
        }
        let p = card.prices;
        let cents = |v: &Option<String>| v.as_deref().and_then(parse_cents);
        out.insert(
            card.id,
            PriceSet {
                usd: cents(&p.usd),
                usd_foil: cents(&p.usd_foil),
                eur: cents(&p.eur),
                eur_foil: cents(&p.eur_foil),
            },
        );
    }
    Ok(out)
}
