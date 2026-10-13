//! Game logic exposed to the UI. Independent from Tauri so it can be unit tested.

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
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
}

struct LoadedSet {
    file: SetFile,
    colors: ColorIndex,
    index: HashMap<String, usize>,
}

pub struct Game {
    data_dir: PathBuf,
    sets: HashMap<String, LoadedSet>,
    prices: Option<PriceFile>,
    rng: ChaCha8Rng,
}

impl Game {
    pub fn new(data_dir: PathBuf) -> Self {
        Self::with_rng(data_dir, ChaCha8Rng::from_entropy())
    }

    pub fn with_rng(data_dir: PathBuf, rng: ChaCha8Rng) -> Self {
        Self { data_dir, sets: HashMap::new(), prices: None, rng }
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

    /// Opens one booster. Cards come back in reveal order: the best card last.
    pub fn open_booster(&mut self, set_code: &str, kind: &str) -> Result<Vec<OpenedCard>, String> {
        self.ensure_loaded(set_code)?;
        if self.prices.is_none() {
            // Prices are optional: the game stays playable without them.
            self.prices = read_json(&self.data_dir.join("prices.json")).ok();
        }
        let Self { sets, prices, rng, .. } = self;
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
                }
            })
            .collect();
        cards.sort_by_key(|c| (rarity_rank(&c.rarity), c.foil));
        Ok(cards)
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
        self.sets.insert(code.to_string(), LoadedSet { file, colors, index });
        Ok(())
    }
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
        { "uuid": "c1", "setCode": "TST", "scryfallId": "abcdef", "name": "Common", "number": "1",
          "rarity": "common", "finishes": ["nonfoil"], "layout": "normal" },
        { "uuid": "m1", "setCode": "TST", "scryfallId": "123456", "name": "Mythic", "number": "2",
          "rarity": "mythic", "finishes": ["nonfoil"], "layout": "normal" }
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
        Game::with_rng(data_dir(name), ChaCha8Rng::seed_from_u64(1))
    }

    #[test]
    fn lists_sets_from_the_manifest() {
        let sets = game("list").list_sets().unwrap();
        assert_eq!(sets.len(), 1);
        assert_eq!((sets[0].code.as_str(), sets[0].boosters.as_slice()), ("TST", ["play".to_string()].as_slice()));
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
    }
}
