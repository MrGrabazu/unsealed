//! Conversion of raw MTGJSON / Scryfall data into the compact catalog files.

use crate::booster::{BoosterConfig, BoosterPack, BoosterSheet, ConfigError};
use crate::catalog::{
    ArtistCard, ArtistIndex, CardEntry, FeaturedCard, ForeignCard, PriceSet, SetFile, COLLECTOR_KIND, FOREIGN_LANGUAGES,
};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};
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
    /// The set's tokens (MTGJSON lists them apart; no booster sheet holds them).
    #[serde(default)]
    tokens: Vec<RawCard>,
    /// The sub-set holding the set's tokens ("TLTR" for "LTR"), when there is one.
    #[serde(default)]
    pub token_set_code: Option<String>,
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
    /// Tokens have none.
    #[serde(default)]
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
    #[serde(default)]
    promo_types: Vec<String>,
    #[serde(default)]
    is_full_art: bool,
    #[serde(default)]
    border_color: Option<String>,
    #[serde(default)]
    frame_effects: Vec<String>,
    #[serde(default)]
    types: Vec<String>,
    #[serde(default)]
    supertypes: Vec<String>,
    #[serde(default)]
    color_identity: Vec<String>,
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
        serialized: c.promo_types.iter().any(|p| p == "serialized"),
        token: false,
        art: false,
        foil_scryfall_id: None,
        full_art: c.is_full_art,
        special_frame: c.border_color.as_deref() == Some("borderless")
            || c.frame_effects.iter().any(|f| f == "showcase" || f == "extendedart"),
        types: c.supertypes.iter().chain(&c.types).map(|t| t.to_lowercase()).collect(),
        color_identity: c.color_identity,
    })
}

/// A token's entry (rarity "token"). Tokens Scryfall does not know are left out.
fn to_token_entry(c: &RawCard) -> Option<CardEntry> {
    let entry = to_entry(c).ok()?;
    Some(CardEntry { rarity: TOKEN_RARITY.to_string(), token: true, ..entry })
}

/// The sub-set holding a set's tokens: MTGJSON's `tokenSetCode`, else "T" + the code.
pub fn token_set_code(raw: &RawSet) -> String {
    raw.token_set_code.clone().unwrap_or_else(|| format!("T{}", raw.code))
}

/// Adds the tokens of a set's token sub-set (its `tokens` and `cards`: everything in a
/// token set is a token, an emblem or a helper card) that the set does not have yet. The
/// parent's own `tokens` list is sometimes empty or partial, the sub-set is not.
pub fn add_tokens_from(set: &mut SetFile, token_set: &RawSet) {
    let known: HashSet<String> = set.cards.iter().map(|c| c.uuid.clone()).collect();
    let mut seen = HashSet::new();
    let added: Vec<CardEntry> = token_set
        .tokens
        .iter()
        .chain(&token_set.cards)
        .filter(|c| !known.contains(&c.uuid) && seen.insert(c.uuid.clone()))
        .filter_map(to_token_entry)
        .collect();
    set.cards.extend(added);
}

/// Rarity given to tokens.
pub const TOKEN_RARITY: &str = "token";

/// Sheets of the token slot added to every booster (MTGJSON describes none).
const TOKEN_SHEET: &str = "unsealedToken";
const TOKEN_FOIL_SHEET: &str = "unsealedTokenFoil";

/// Gives every booster of the set one of its tokens, like real boosters (where tokens share
/// a slot with ad and art cards, which the game leaves out). A collector booster's token is
/// always foil; elsewhere one in `foil_one_in` is. Each pack variant is split in two (plain
/// and foil token), so the odds stay exact and the simulation checks them like any other.
/// A set without tokens is left as it is.
pub fn add_token_slots(set: &mut SetFile, foil_one_in: u64) {
    let sheet = |foil: bool| {
        let finish = if foil { "foil" } else { "nonfoil" };
        let cards: BTreeMap<String, u64> = set
            .cards
            .iter()
            .filter(|c| c.token && c.finishes.iter().any(|f| f == finish))
            .map(|c| (c.uuid.clone(), 1))
            .collect();
        (!cards.is_empty()).then(|| BoosterSheet {
            allow_duplicates: false,
            balance_colors: false,
            total_weight: cards.len() as u64,
            cards,
            fixed: false,
            foil,
        })
    };
    let (plain, foil) = (sheet(false), sheet(true));
    let foil_one_in = foil_one_in.max(1);
    for (kind, cfg) in set.boosters.iter_mut() {
        // (sheet, share of the packs)
        let slots: Vec<(&str, u64)> = match (&plain, &foil) {
            // (MTGJSON describes neither tokens nor art cards in boosters.)
            (None, None) => return,
            (_, Some(_)) if kind == COLLECTOR_KIND => vec![(TOKEN_FOIL_SHEET, 1)],
            (Some(_), None) => vec![(TOKEN_SHEET, 1)],
            (None, Some(_)) => vec![(TOKEN_FOIL_SHEET, 1)],
            (Some(_), Some(_)) => vec![(TOKEN_SHEET, foil_one_in - 1), (TOKEN_FOIL_SHEET, 1)],
        };
        let slots: Vec<(Option<&str>, u64)> = slots.into_iter().map(|(name, share)| (Some(name), share)).collect();
        for &(name, _) in &slots {
            let name = name.expect("every token share has a sheet");
            let source = if name == TOKEN_SHEET { &plain } else { &foil };
            cfg.sheets.insert(name.to_string(), source.clone().expect("sheet checked above"));
        }
        split_variants(cfg, &slots);
    }
}

/// Splits every pack variant of a booster by `slots` (sheet, share of the packs): each share
/// gets a copy of the variant with one card more from its sheet (none for `None`), weighted
/// by its share. The odds stay exact, and the simulation checks them like any other.
fn split_variants(cfg: &mut BoosterConfig, slots: &[(Option<&str>, u64)]) {
    let slots: Vec<(Option<&str>, u64)> = slots.iter().copied().filter(|&(_, share)| share > 0).collect();
    let total: u64 = slots.iter().map(|&(_, share)| share).sum();
    if total == 0 {
        return;
    }
    cfg.boosters = cfg
        .boosters
        .iter()
        .flat_map(|pack| {
            slots.iter().map(move |&(name, share)| {
                let mut contents = pack.contents.clone();
                if let Some(name) = name {
                    contents.insert(name.to_string(), 1);
                }
                BoosterPack { contents, weight: pack.weight * share }
            })
        })
        .collect();
    cfg.boosters_total_weight *= total;
}

/// The collections by artist: every card the sets' boosters can give (tokens and art cards
/// included), filed under each of its artists ("A & B" counts for both), once.
pub fn artist_index(sets: &[SetFile]) -> ArtistIndex {
    let mut index = ArtistIndex::default();
    let mut seen = HashSet::new();
    for set in sets {
        let in_boosters: HashSet<&str> = set
            .boosters
            .values()
            .flat_map(|cfg| cfg.sheets.values())
            .flat_map(|sheet| sheet.cards.keys().map(String::as_str))
            .collect();
        for card in set.cards.iter().filter(|c| in_boosters.contains(c.uuid.as_str())) {
            let Some(artist) = &card.artist else { continue };
            if !seen.insert(card.uuid.clone()) {
                continue;
            }
            for name in artist.split(" & ").map(str::trim).filter(|n| !n.is_empty()) {
                index
                    .artists
                    .entry(name.to_string())
                    .or_default()
                    .push(ArtistCard { uuid: card.uuid.clone(), set: set.code.clone() });
            }
        }
    }
    index
}

/// Rarity given to art cards.
pub const ART_RARITY: &str = "art";

/// Sheets of the art card slot (MTGJSON describes none): the regular and the gold signed one.
const ART_SHEET: &str = "unsealedArt";
const ART_SIGNED_SHEET: &str = "unsealedArtSigned";

/// The art series of a set: "A" + its code ("ALTR" for "LTR").
pub fn art_set_code(code: &str) -> String {
    format!("A{code}")
}

/// Adds a set's art cards, from its art series. MTGJSON lists them among the series' tokens,
/// each face apart (a and b, the same card): one entry per card is kept, named once, and its
/// gold signed version (finish "signed") is recorded as its "foil" finish.
pub fn add_art_cards_from(set: &mut SetFile, art_set: &RawSet) {
    let known: HashSet<String> = set.cards.iter().map(|c| c.uuid.clone()).collect();
    let added: Vec<CardEntry> = art_set
        .tokens
        .iter()
        .chain(&art_set.cards)
        .filter(|c| c.side.as_deref() != Some("b") && !known.contains(&c.uuid))
        .filter_map(|c| to_entry(c).ok())
        .map(|entry| CardEntry {
            name: entry.name.split(" // ").next().unwrap_or(&entry.name).to_string(),
            rarity: ART_RARITY.to_string(),
            finishes: entry.finishes.iter().map(|f| if f == "signed" { "foil".to_string() } else { f.clone() }).collect(),
            art: true,
            ..entry
        })
        .collect();
    set.cards.extend(added);
}

/// How often boosters hold an art card (see `ArtCardOdds`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtCardOdds {
    /// Set boosters always open on an art card; one in this many is the signed one.
    pub set_signed_one_in: u64,
    /// Play boosters of the sets released before `play_until`: percent of the packs with a
    /// regular art card, and with a signed one.
    pub play_percent: u64,
    pub play_signed_percent: u64,
    pub play_until: String,
    /// Collector boosters of the sets released from `collector_from` (art cards moved there).
    pub collector_percent: u64,
    pub collector_signed_percent: u64,
    pub collector_from: String,
}

impl Default for ArtCardOdds {
    /// The MTG wiki's odds; art cards moved from Play to Collector boosters with Aetherdrift.
    /// The Collector odds are not documented: an estimate.
    fn default() -> Self {
        Self {
            set_signed_one_in: 20,
            play_percent: 30,
            play_signed_percent: 5,
            play_until: "2025-02-14".into(),
            collector_percent: 45,
            collector_signed_percent: 5,
            collector_from: "2025-02-14".into(),
        }
    }
}

/// Gives the boosters that hold one an art card of the set (Set boosters always, Play and
/// Collector ones some of the time, depending on the set's date: see `ArtCardOdds`). In real
/// Play and Collector boosters the art card takes the token's place; here it comes along.
pub fn add_art_slots(set: &mut SetFile, odds: &ArtCardOdds) {
    let sheet = |signed: bool| {
        let finish = if signed { "foil" } else { "nonfoil" };
        let cards: BTreeMap<String, u64> =
            set.cards.iter().filter(|c| c.art && c.finishes.iter().any(|f| f == finish)).map(|c| (c.uuid.clone(), 1)).collect();
        (!cards.is_empty()).then(|| BoosterSheet {
            allow_duplicates: false,
            balance_colors: false,
            total_weight: cards.len() as u64,
            cards,
            fixed: false,
            foil: signed,
        })
    };
    let (plain, signed) = (sheet(false), sheet(true));
    if plain.is_none() && signed.is_none() {
        return;
    }
    let released = set.release_date.clone();
    for (kind, cfg) in set.boosters.iter_mut() {
        // Shares of the packs: (regular art card, signed one, none).
        let (regular, gold, none) = match kind.as_str() {
            "set" => (odds.set_signed_one_in.max(1) - 1, 1, 0),
            "play" if released < odds.play_until => {
                (odds.play_percent, odds.play_signed_percent, 100u64.saturating_sub(odds.play_percent + odds.play_signed_percent))
            }
            COLLECTOR_KIND if released >= odds.collector_from => (
                odds.collector_percent,
                odds.collector_signed_percent,
                100u64.saturating_sub(odds.collector_percent + odds.collector_signed_percent),
            ),
            _ => continue,
        };
        // A missing version gives its share to the other one.
        let (regular, gold) = match (&plain, &signed) {
            (Some(_), None) => (regular + gold, 0),
            (None, Some(_)) => (0, regular + gold),
            _ => (regular, gold),
        };
        if regular > 0 {
            cfg.sheets.insert(ART_SHEET.to_string(), plain.clone().expect("checked above"));
        }
        if gold > 0 {
            cfg.sheets.insert(ART_SIGNED_SHEET.to_string(), signed.clone().expect("checked above"));
        }
        split_variants(cfg, &[(Some(ART_SHEET), regular), (Some(ART_SIGNED_SHEET), gold), (None, none)]);
    }
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
        .filter(|c| c.set_code == set.code && !c.token && c.artist.is_some() && in_main.contains(c.uuid.as_str()));
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
    entries.extend(raw.tokens.iter().filter_map(to_token_entry));

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
        foil_aliases: BTreeMap::new(),
    })
}

/// Merges the foil versions MTGJSON lists as printings of their own ("★" numbers: the foils of
/// 7th to 10th Edition, Unhinged…) into their card: a foil copy of the card, not another card
/// to collect. A "★" printing is merged only when it is foil only, has the same name, artist
/// and face as the card numbered without the "★", and that card has no foil finish (WAR's
/// "★" planeswalkers, with another art, stay apart). Its sheets draw the card instead (they
/// are foil sheets), its foil price comes from it (`foil_scryfall_id`), and `foil_aliases`
/// keeps its uuid for games saved before.
pub fn merge_foil_printings(set: &mut SetFile) {
    let mut aliases = BTreeMap::new();
    for star in set.cards.iter().filter(|c| c.number.ends_with('★') && c.finishes == ["foil"]) {
        let base_number = star.number.trim_end_matches('★');
        let base = set.cards.iter().find(|c| {
            c.number == base_number
                && c.name == star.name
                && c.artist == star.artist
                && c.side == star.side
                && c.set_code == star.set_code
                && !c.finishes.iter().any(|f| f == "foil")
        });
        if let Some(base) = base {
            aliases.insert(star.uuid.clone(), (base.uuid.clone(), star.scryfall_id.clone()));
        }
    }
    if aliases.is_empty() {
        return;
    }
    for card in &mut set.cards {
        if let Some((_, scryfall_id)) = aliases.values().find(|(base, _)| *base == card.uuid) {
            card.finishes.push("foil".to_string());
            card.foil_scryfall_id = Some(scryfall_id.clone());
        }
    }
    set.cards.retain(|c| !aliases.contains_key(&c.uuid));
    for sheet in set.boosters.values_mut().flat_map(|b| b.sheets.values_mut()) {
        for (star, (base, _)) in &aliases {
            if let Some(weight) = sheet.cards.remove(star) {
                *sheet.cards.entry(base.clone()).or_insert(0) += weight;
            }
        }
    }
    set.foil_aliases = aliases.into_iter().map(|(star, (base, _))| (star, base)).collect();
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
        .filter_map(|c| {
            let mut prices = by_scryfall_id.get(&c.scryfall_id).copied();
            // A foil version merged into the card (`merge_foil_printings`) gives its foil prices.
            if let Some(foil) = c.foil_scryfall_id.as_ref().and_then(|id| by_scryfall_id.get(id)) {
                let p = prices.get_or_insert(PriceSet { estimated: foil.estimated, ..Default::default() });
                p.usd_foil = foil.usd_foil.or(p.usd_foil);
                p.eur_foil = foil.eur_foil.or(p.eur_foil);
            }
            Some((c.uuid.clone(), prices?))
        })
        .collect()
}

#[derive(Deserialize)]
struct ScryfallCard {
    id: String,
    /// The card across all its printings (on the faces only, for reversible cards).
    #[serde(default)]
    oracle_id: Option<String>,
    #[serde(default)]
    card_faces: Vec<ScryfallFace>,
    #[serde(default)]
    prices: ScryfallPrices,
    #[serde(default)]
    promo_types: Vec<String>,
}

#[derive(Deserialize)]
struct ScryfallFace {
    #[serde(default)]
    oracle_id: Option<String>,
}

impl ScryfallCard {
    fn oracle_id(&self) -> Option<&str> {
        self.oracle_id.as_deref().or_else(|| self.card_faces.iter().find_map(|f| f.oracle_id.as_deref()))
    }
}

#[derive(Deserialize, Default)]
struct ScryfallPrices {
    usd: Option<String>,
    usd_foil: Option<String>,
    eur: Option<String>,
    eur_foil: Option<String>,
}

/// Reads a Scryfall bulk file (one JSON card per line) and returns the prices of the wanted
/// card ids only. A wanted printing without any price (a serialized card, a rare promo...)
/// gets an estimate instead: the best prices of the other printings of the same card,
/// marked `estimated`.
pub fn prices_from_scryfall_jsonl<R: BufRead>(
    reader: R,
    wanted: &HashSet<String>,
) -> Result<BTreeMap<String, PriceSet>, ConfigError> {
    let mut out = BTreeMap::new();
    // Best prices of each card over all its printings, and the card of each wanted printing.
    let mut best: HashMap<String, PriceSet> = HashMap::new();
    let mut oracle_of: HashMap<String, String> = HashMap::new();
    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let card: ScryfallCard = serde_json::from_str(line)?;
        let p = &card.prices;
        let cents = |v: &Option<String>| v.as_deref().and_then(parse_cents);
        let prices = PriceSet {
            usd: cents(&p.usd),
            usd_foil: cents(&p.usd_foil),
            eur: cents(&p.eur),
            eur_foil: cents(&p.eur_foil),
            estimated: false,
        };
        if let Some(oracle) = card.oracle_id() {
            // Serialized printings already carry their scarcity in their price: the base of
            // an estimate is the ordinary printings (the game multiplies it for scarce cards).
            if !prices.is_empty() && !card.promo_types.iter().any(|p| p == "serialized") {
                best.entry(oracle.to_string()).or_default().merge_best(&prices);
            }
            if wanted.contains(&card.id) {
                oracle_of.insert(card.id.clone(), oracle.to_string());
            }
        }
        if wanted.contains(&card.id) {
            out.insert(card.id, prices);
        }
    }
    for (id, prices) in out.iter_mut() {
        if let Some(best) = prices.is_empty().then(|| oracle_of.get(id).and_then(|o| best.get(o))).flatten() {
            *prices = PriceSet::estimate_from(*best);
        }
    }
    out.retain(|_, p| !p.is_empty());
    Ok(out)
}
