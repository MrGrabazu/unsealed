//! Conversion of raw MTGJSON / Scryfall data into the compact catalog files.

use crate::booster::{BoosterConfig, ConfigError};
use crate::catalog::{CardEntry, FeaturedCard, PriceSet, SetFile};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::io::BufRead;

#[derive(Deserialize)]
struct RawFile {
    meta: RawMeta,
    data: RawSet,
}

#[derive(Deserialize)]
struct RawMeta {
    version: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawSet {
    code: String,
    name: String,
    release_date: String,
    #[serde(rename = "type")]
    set_type: String,
    cards: Vec<RawCard>,
    #[serde(default)]
    booster: BTreeMap<String, BoosterConfig>,
}

#[derive(Deserialize)]
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
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawIdentifiers {
    scryfall_id: Option<String>,
}

fn to_entry(c: RawCard) -> Result<CardEntry, ConfigError> {
    let scryfall_id = c.identifiers.scryfall_id.ok_or_else(|| {
        ConfigError::Invalid(format!("card `{}` ({}) has no scryfallId", c.name, c.uuid))
    })?;
    Ok(CardEntry {
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
    let mut codes = Vec::new();
    for kind in kept_kinds(&raw.data.booster, kinds)? {
        let cfg = &raw.data.booster[kind];
        for c in &cfg.source_set_codes {
            if *c != raw.data.code && !codes.contains(c) {
                codes.push(c.clone());
            }
        }
    }
    Ok(codes)
}

/// Builds the compact set file from a raw MTGJSON set file, keeping the booster
/// kinds listed in `kinds` that the set has. Every kept config is validated, and every
/// card referenced by a sheet must exist either in the set or in `extra_sets`
/// (raw files of the sets returned by `source_set_codes`). Cards from other
/// sets are embedded only when a kept sheet references them; they keep their
/// own `set_code`, so completion tracking can ignore them.
pub fn set_from_mtgjson(
    json: &str,
    kinds: &[&str],
    extra_sets: &[&str],
) -> Result<SetFile, ConfigError> {
    let raw: RawFile = serde_json::from_str(json)?;
    let RawSet { code, name, release_date, set_type, cards, mut booster } = raw.data;

    let mut entries = cards.into_iter().map(to_entry).collect::<Result<Vec<_>, _>>()?;
    let mut foreign: BTreeMap<String, CardEntry> = BTreeMap::new();
    for extra in extra_sets {
        let extra: RawFile = serde_json::from_str(extra)?;
        for c in extra.data.cards {
            let entry = to_entry(c)?;
            foreign.insert(entry.uuid.clone(), entry);
        }
    }
    let own: HashSet<String> = entries.iter().map(|c| c.uuid.clone()).collect();

    let mut boosters = BTreeMap::new();
    let mut used_foreign: HashSet<String> = HashSet::new();
    for kind in kept_kinds(&booster, kinds)? {
        let cfg = booster.remove(kind).expect("kept kinds exist in the set");
        cfg.validate()?;
        for (sheet_name, sheet) in &cfg.sheets {
            for uuid in sheet.cards.keys() {
                if own.contains(uuid) {
                    continue;
                }
                if !foreign.contains_key(uuid) {
                    return Err(ConfigError::Invalid(format!(
                        "{code}/{kind}: sheet `{sheet_name}` references unknown card {uuid}"
                    )));
                }
                used_foreign.insert(uuid.clone());
            }
        }
        boosters.insert(kind.to_string(), cfg);
    }
    entries.extend(foreign.into_iter().filter(|(u, _)| used_foreign.contains(u)).map(|(_, c)| c));

    Ok(SetFile {
        code,
        name,
        release_date,
        set_type,
        source_version: raw.meta.version,
        cards: entries,
        boosters,
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
