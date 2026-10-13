//! Game logic exposed to the UI. Independent from Tauri so it can be unit tested.

use crate::store::{OpeningCard, Store};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use unsealed_core::catalog::{Manifest, PriceFile, SetFile};
use unsealed_core::{open_booster, ColorIndex};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSummary {
    pub code: String,
    pub name: String,
    pub release_date: String,
    pub boosters: Vec<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OpenedCard {
    pub uuid: String,
    pub name: String,
    pub number: String,
    pub rarity: String,
    pub set_code: String,
    pub scryfall_id: String,
    pub foil: bool,
    pub sheet: String,
    pub image_url: String,
    /// Market price in USD cents, if known.
    pub price_cents: Option<u32>,
    /// First time the player owns this card (in any finish).
    pub is_new: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionCard {
    pub uuid: String,
    pub name: String,
    pub number: String,
    pub rarity: String,
    pub image_url: String,
    pub quantity: u32,
    pub foil_quantity: u32,
    pub price_cents: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionView {
    pub set_code: String,
    pub set_name: String,
    /// Distinct obtainable cards owned (any finish).
    pub owned_count: usize,
    /// Distinct cards of the set that boosters can give.
    pub total_count: usize,
    pub completion_percent: f64,
    pub cards: Vec<CollectionCard>,
}

struct LoadedSet {
    file: SetFile,
    colors: ColorIndex,
    index: HashMap<String, usize>,
    /// Indices (in `file.cards`) of the set's own cards that a configured booster can
    /// give, in collector number order. Defines what "complete" means for this set.
    obtainable: Vec<usize>,
}

pub struct Game {
    data_dir: PathBuf,
    store: Store,
    sets: HashMap<String, LoadedSet>,
    prices: Option<PriceFile>,
    rng: ChaCha8Rng,
}

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

fn db_err(e: rusqlite::Error) -> String {
    format!("database error: {e}")
}

impl Game {
    pub fn new(data_dir: PathBuf, store: Store) -> Self {
        Self::with_rng(data_dir, store, ChaCha8Rng::from_entropy())
    }

    pub fn with_rng(data_dir: PathBuf, store: Store, rng: ChaCha8Rng) -> Self {
        Self { data_dir, store, sets: HashMap::new(), prices: None, rng }
    }

    pub fn list_sets(&self) -> Result<Vec<SetSummary>, String> {
        let path = self.data_dir.join("manifest.json");
        let manifest: Manifest = read_json(&path)?;
        let mut sets: Vec<SetSummary> = manifest
            .sets
            .into_iter()
            .map(|(code, s)| SetSummary {
                code,
                name: s.name,
                release_date: s.release_date,
                boosters: s.boosters,
            })
            .collect();
        sets.sort_by(|a, b| b.release_date.cmp(&a.release_date));
        Ok(sets)
    }

    /// Opens one booster and adds its cards to the collection.
    /// Cards come back in reveal order: the best card last.
    pub fn open_booster(&mut self, set_code: &str, kind: &str) -> Result<Vec<OpenedCard>, String> {
        self.ensure_loaded(set_code)?;
        self.ensure_prices();
        let Self { sets, prices, rng, store, .. } = self;
        let set = &sets[set_code];
        let cfg = set
            .file
            .boosters
            .get(kind)
            .ok_or_else(|| format!("set {set_code} has no `{kind}` booster"))?;
        let drawn = open_booster(cfg, &set.colors, rng).map_err(|e| e.to_string())?;

        let mut cards: Vec<OpenedCard> = drawn
            .into_iter()
            .map(|d| {
                let card = &set.file.cards[set.index[&d.uuid]];
                let price = prices.as_ref().and_then(|p| p.cards.get(&card.scryfall_id)).and_then(
                    |p| if d.foil { p.usd_foil } else { p.usd },
                );
                OpenedCard {
                    image_url: image_url(&card.scryfall_id),
                    uuid: d.uuid,
                    name: card.name.clone(),
                    number: card.number.clone(),
                    rarity: card.rarity.clone(),
                    set_code: card.set_code.clone(),
                    scryfall_id: card.scryfall_id.clone(),
                    foil: d.foil,
                    sheet: d.sheet,
                    price_cents: price,
                    is_new: false,
                }
            })
            .collect();
        cards.sort_by_key(|c| (rarity_rank(&c.rarity), c.foil));

        let flags = {
            let records: Vec<OpeningCard<'_>> = cards
                .iter()
                .map(|c| OpeningCard {
                    uuid: &c.uuid,
                    set_code: &c.set_code,
                    foil: c.foil,
                    price_cents: c.price_cents,
                })
                .collect();
            store.record_opening(set_code, kind, &records, now_secs()).map_err(db_err)?
        };
        for (card, is_new) in cards.iter_mut().zip(flags) {
            card.is_new = is_new;
        }
        Ok(cards)
    }

    /// The player's collection for one set, with completion progress.
    pub fn collection(&mut self, set_code: &str) -> Result<CollectionView, String> {
        self.ensure_loaded(set_code)?;
        self.ensure_prices();
        let owned = self.store.owned_in_set(set_code).map_err(db_err)?;
        let set = &self.sets[set_code];

        let cards: Vec<CollectionCard> = set
            .obtainable
            .iter()
            .map(|&i| {
                let c = &set.file.cards[i];
                let have = owned.get(&c.uuid).copied().unwrap_or_default();
                let price = self.prices.as_ref().and_then(|p| p.cards.get(&c.scryfall_id)).and_then(|p| p.usd);
                CollectionCard {
                    uuid: c.uuid.clone(),
                    name: c.name.clone(),
                    number: c.number.clone(),
                    rarity: c.rarity.clone(),
                    image_url: image_url(&c.scryfall_id),
                    quantity: have.normal,
                    foil_quantity: have.foil,
                    price_cents: price,
                }
            })
            .collect();
        let owned_count = cards.iter().filter(|c| c.quantity + c.foil_quantity > 0).count();
        let total_count = cards.len();
        let completion_percent =
            if total_count == 0 { 0.0 } else { owned_count as f64 * 100.0 / total_count as f64 };
        Ok(CollectionView {
            set_code: set.file.code.clone(),
            set_name: set.file.name.clone(),
            owned_count,
            total_count,
            completion_percent,
            cards,
        })
    }

    fn ensure_loaded(&mut self, code: &str) -> Result<(), String> {
        // The code ends up in a file path: only accept plain alphanumeric codes.
        if code.is_empty() || !code.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(format!("invalid set code `{code}`"));
        }
        if self.sets.contains_key(code) {
            return Ok(());
        }
        let file: SetFile = read_json(&self.data_dir.join("sets").join(format!("{code}.json")))?;
        let colors = ColorIndex::from_cards(&file.cards);
        let index = file.cards.iter().enumerate().map(|(i, c)| (c.uuid.clone(), i)).collect();
        let obtainable = obtainable_cards(&file);
        self.sets.insert(code.to_string(), LoadedSet { file, colors, index, obtainable });
        Ok(())
    }

    /// Prices are optional: the game stays playable without them.
    fn ensure_prices(&mut self) {
        if self.prices.is_none() {
            self.prices = read_json(&self.data_dir.join("prices.json")).ok();
        }
    }
}

fn obtainable_cards(file: &SetFile) -> Vec<usize> {
    let in_sheets: HashSet<&str> = file
        .boosters
        .values()
        .flat_map(|cfg| cfg.sheets.values())
        .flat_map(|sheet| sheet.cards.keys().map(String::as_str))
        .collect();
    let mut indices: Vec<usize> = file
        .cards
        .iter()
        .enumerate()
        .filter(|(_, c)| c.set_code == file.code && in_sheets.contains(c.uuid.as_str()))
        .map(|(i, _)| i)
        .collect();
    indices.sort_by_key(|&i| number_key(&file.cards[i].number));
    indices
}

/// Sort key for collector numbers such as "12", "12★" or "A-12": leading digits first.
fn number_key(number: &str) -> (u32, String) {
    let digits: String = number.chars().take_while(char::is_ascii_digit).collect();
    (digits.parse().unwrap_or(u32::MAX), number.to_string())
}

fn read_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<T, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("invalid {}: {e}", path.display()))
}

/// Scryfall CDN image of a card (normal size). Double-faced back sides are not handled yet.
fn image_url(scryfall_id: &str) -> String {
    match scryfall_id.get(0..2) {
        Some(p) => format!("https://cards.scryfall.io/normal/front/{}/{}/{scryfall_id}.jpg", &p[0..1], &p[1..2]),
        None => String::new(),
    }
}

fn rarity_rank(rarity: &str) -> u8 {
    match rarity {
        "common" => 0,
        "uncommon" => 1,
        "rare" => 2,
        "mythic" => 3,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SET: &str = r#"{
      "code": "TST", "name": "Test Set", "releaseDate": "2026-01-01", "setType": "expansion",
      "sourceVersion": "v1",
      "cards": [
        { "uuid": "c1", "setCode": "TST", "scryfallId": "abcdef", "name": "Common", "number": "10",
          "rarity": "common", "finishes": ["nonfoil"], "layout": "normal" },
        { "uuid": "m1", "setCode": "TST", "scryfallId": "123456", "name": "Mythic", "number": "2",
          "rarity": "mythic", "finishes": ["nonfoil"], "layout": "normal" },
        { "uuid": "x1", "setCode": "TST", "scryfallId": "999999", "name": "Not in boosters", "number": "3",
          "rarity": "rare", "finishes": ["nonfoil"], "layout": "normal" }
      ],
      "boosters": { "play": {
        "boosters": [ { "contents": { "mythic": 1, "common": 2 }, "weight": 1 } ],
        "boostersTotalWeight": 1,
        "sheets": {
          "common": { "allowDuplicates": true, "cards": { "c1": 1 }, "foil": false, "totalWeight": 1 },
          "mythic": { "cards": { "m1": 1 }, "foil": false, "totalWeight": 1 }
        },
        "sourceSetCodes": ["TST"] } } }"#;

    const MANIFEST: &str = r#"{ "generatedAt": "0", "pricesUpdatedAt": "d", "sets": {
      "TST": { "name": "Test Set", "releaseDate": "2026-01-01", "sourceVersion": "v1", "boosters": ["play"] } } }"#;

    const PRICES: &str = r#"{ "updatedAt": "d", "cards": { "abcdef": { "usd": 25 } } }"#;

    fn data_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("unsealed-test-{}-{name}", std::process::id()));
        fs::create_dir_all(dir.join("sets")).unwrap();
        fs::write(dir.join("sets/TST.json"), SET).unwrap();
        fs::write(dir.join("manifest.json"), MANIFEST).unwrap();
        fs::write(dir.join("prices.json"), PRICES).unwrap();
        dir
    }

    fn game(name: &str) -> Game {
        Game::with_rng(data_dir(name), Store::open_in_memory().unwrap(), ChaCha8Rng::seed_from_u64(1))
    }

    #[test]
    fn lists_sets_from_the_manifest() {
        let sets = game("list").list_sets().unwrap();
        assert_eq!(sets.len(), 1);
        assert_eq!(sets[0].code, "TST");
        assert_eq!(sets[0].boosters, ["play"]);
    }

    #[test]
    fn opens_a_booster_with_the_best_card_last_and_prices() {
        let cards = game("open").open_booster("TST", "play").unwrap();
        let names: Vec<&str> = cards.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Common", "Common", "Mythic"]);
        assert_eq!(cards[0].price_cents, Some(25));
        assert_eq!(cards[2].price_cents, None);
        assert_eq!(cards[0].image_url, "https://cards.scryfall.io/normal/front/a/b/abcdef.jpg");
    }

    #[test]
    fn flags_new_cards_only_on_first_discovery() {
        let mut g = game("new");
        let first: Vec<bool> = g.open_booster("TST", "play").unwrap().iter().map(|c| c.is_new).collect();
        assert_eq!(first, [true, false, true]); // Common, Common (duplicate), Mythic
        let second: Vec<bool> = g.open_booster("TST", "play").unwrap().iter().map(|c| c.is_new).collect();
        assert_eq!(second, [false, false, false]);
    }

    #[test]
    fn collection_tracks_progress_on_obtainable_cards_only() {
        let mut g = game("collection");
        let empty = g.collection("TST").unwrap();
        assert_eq!((empty.owned_count, empty.total_count), (0, 2)); // x1 is not in any sheet
        assert_eq!(empty.completion_percent, 0.0);

        g.open_booster("TST", "play").unwrap();
        let view = g.collection("TST").unwrap();
        assert_eq!((view.owned_count, view.total_count), (2, 2));
        assert_eq!(view.completion_percent, 100.0);
        // Sorted by collector number: 2 before 10.
        let owned: Vec<(&str, u32)> = view.cards.iter().map(|c| (c.name.as_str(), c.quantity)).collect();
        assert_eq!(owned, [("Mythic", 1), ("Common", 2)]);
    }

    /// Needs the real data: run `data-builder` first, then
    /// `cargo test -p unsealed-app -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_data_smoke_test() {
        let dist = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist");
        let mut g = Game::with_rng(dist, Store::open_in_memory().unwrap(), ChaCha8Rng::seed_from_u64(7));
        let sets = g.list_sets().unwrap();
        assert!(!sets.is_empty());
        for s in &sets {
            let mut new_cards = 0;
            for _ in 0..30 {
                let cards = g.open_booster(&s.code, "play").unwrap();
                assert!(cards.len() >= 12);
                assert!(cards.iter().all(|c| c.image_url.starts_with("https://cards.scryfall.io/")));
                // Cards from other sets (e.g. Special Guests) do not count for this set.
                new_cards += cards.iter().filter(|c| c.is_new && c.set_code == s.code).count();
            }
            let view = g.collection(&s.code).unwrap();
            println!(
                "{}: {}/{} cards ({:.1}%), {new_cards} discoveries over 30 boosters",
                s.code, view.owned_count, view.total_count, view.completion_percent
            );
            assert_eq!(view.owned_count, new_cards);
            assert!(view.owned_count > 0 && view.owned_count <= view.total_count);
        }
    }

    #[test]
    fn same_seed_gives_the_same_booster() {
        let a = game("seed-a").open_booster("TST", "play").unwrap();
        let b = game("seed-b").open_booster("TST", "play").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn rejects_bad_codes_and_unknown_kinds() {
        let mut g = game("errors");
        assert!(g.open_booster("../secret", "play").is_err());
        assert!(g.open_booster("", "play").is_err());
        assert!(g.open_booster("NOPE", "play").is_err());
        assert!(g.open_booster("TST", "collector").is_err());
        assert!(g.collection("../secret").is_err());
    }
}
