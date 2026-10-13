//! Conversion of raw MTGJSON / Scryfall data into the compact catalog files.

use crate::booster::{BoosterConfig, ConfigError};
use crate::catalog::{CardEntry, PriceSet, SetFile};
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
    })
}

/// Codes of the other sets whose cards the kept booster configs draw from
/// (MTGJSON `sourceSetCodes`, e.g. a Special Guests sheet in a Play Booster).
/// Their raw files must be passed to `set_from_mtgjson`.
pub fn source_set_codes(json: &str, kinds: &[&str]) -> Result<Vec<String>, ConfigError> {
    let raw: RawFile = serde_json::from_str(json)?;
    let mut codes = Vec::new();
    for &kind in kinds {
        let cfg = raw
            .data
            .booster
            .get(kind)
            .ok_or_else(|| ConfigError::UnknownBooster(kind.to_string()))?;
        for c in &cfg.source_set_codes {
            if *c != raw.data.code && !codes.contains(c) {
                codes.push(c.clone());
            }
        }
    }
    Ok(codes)
}

/// Builds the compact set file from a raw MTGJSON set file, keeping only the
/// booster kinds listed in `kinds`. Every kept config is validated, and every
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
    for &kind in kinds {
        let cfg = booster
            .remove(kind)
            .ok_or_else(|| ConfigError::UnknownBooster(kind.to_string()))?;
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
