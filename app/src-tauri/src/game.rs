//! Game logic exposed to the UI. Independent from Tauri so it can be unit tested.

use crate::store::{OpeningCard, Sale, Store};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use unsealed_core::catalog::{Manifest, PriceFile, SetFile};
use unsealed_core::economy::{charity_available, EconomyConfig};
use unsealed_core::{open_booster, ColorIndex};

/// Largest number of boosters bought in one purchase.
const MAX_PURCHASE: u32 = 100;

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
    /// Market price in the game currency (cents), if known.
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
    pub foil_price_cents: Option<u32>,
    /// What the Boosterium pays for one copy (0 = cannot be sold).
    pub sell_value_cents: u32,
    pub foil_sell_value_cents: u32,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrencyView {
    pub code: String,
    pub symbol: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharityStatus {
    pub amount_cents: u32,
    pub available: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SealedStack {
    pub set_code: String,
    pub set_name: String,
    pub kind: String,
    pub quantity: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletView {
    pub balance_cents: i64,
    pub currency: CurrencyView,
    pub charity: CharityStatus,
    pub sealed: Vec<SealedStack>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ShopItem {
    pub set_code: String,
    pub set_name: String,
    pub kind: String,
    pub price_cents: u32,
}

#[derive(Debug, Deserialize)]
pub struct SaleRequest {
    pub uuid: String,
    pub foil: bool,
    pub quantity: u32,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SaleResult {
    pub sold_count: u32,
    pub total_cents: i64,
}

/// Current time and local calendar day ("YYYY-MM-DD", used for the daily charity).
pub struct Clock {
    pub now: i64,
    pub day: String,
}

pub type ClockFn = Box<dyn Fn() -> Clock + Send>;

pub fn system_clock() -> Clock {
    let now = chrono::Local::now();
    Clock { now: now.timestamp(), day: now.format("%Y-%m-%d").to_string() }
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
    economy: EconomyConfig,
    sets: HashMap<String, LoadedSet>,
    prices: Option<PriceFile>,
    rng: ChaCha8Rng,
    clock: ClockFn,
}

fn err<E: ToString>(e: E) -> String {
    e.to_string()
}

impl Game {
    pub fn new(data_dir: PathBuf, store: Store, economy: EconomyConfig) -> Result<Self, String> {
        Self::with_parts(data_dir, store, economy, ChaCha8Rng::from_entropy(), Box::new(system_clock))
    }

    pub fn with_parts(
        data_dir: PathBuf,
        mut store: Store,
        economy: EconomyConfig,
        rng: ChaCha8Rng,
        clock: ClockFn,
    ) -> Result<Self, String> {
        store.ensure_starting_balance(economy.starting_balance_cents, clock().now).map_err(err)?;
        Ok(Self { data_dir, store, economy, sets: HashMap::new(), prices: None, rng, clock })
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

    // ---- Boosterium: wallet, shop, charity -------------------------------------

    pub fn shop(&self) -> Result<Vec<ShopItem>, String> {
        let mut items = Vec::new();
        for set in self.list_sets()? {
            for kind in &set.boosters {
                items.push(ShopItem {
                    price_cents: self.economy.booster_price_cents(&set.code, kind),
                    set_code: set.code.clone(),
                    set_name: set.name.clone(),
                    kind: kind.clone(),
                });
            }
        }
        Ok(items)
    }

    pub fn wallet(&mut self) -> Result<WalletView, String> {
        let balance = self.store.balance().map_err(err)?;
        let charity_open = self.charity_open(balance)?;
        let names: HashMap<String, String> = self
            .list_sets()
            .map(|sets| sets.into_iter().map(|s| (s.code, s.name)).collect())
            .unwrap_or_default();
        let sealed = self
            .store
            .sealed_stacks()
            .map_err(err)?
            .into_iter()
            .map(|(set_code, kind, quantity)| {
                let set_name = names.get(&set_code).cloned().unwrap_or_else(|| set_code.clone());
                SealedStack { set_code, set_name, kind, quantity }
            })
            .collect();
        Ok(WalletView {
            balance_cents: balance,
            currency: CurrencyView {
                code: self.economy.currency.code.clone(),
                symbol: self.economy.currency.symbol.clone(),
            },
            charity: CharityStatus { amount_cents: self.economy.charity.amount_cents, available: charity_open },
            sealed,
        })
    }

    pub fn buy_boosters(&mut self, set_code: &str, kind: &str, quantity: u32) -> Result<(), String> {
        if !(1..=MAX_PURCHASE).contains(&quantity) {
            return Err(format!("quantity must be between 1 and {MAX_PURCHASE}"));
        }
        self.ensure_loaded(set_code)?;
        if !self.sets[set_code].file.boosters.contains_key(kind) {
            return Err(format!("set {set_code} has no `{kind}` booster"));
        }
        let price = self.economy.booster_price_cents(set_code, kind);
        let now = (self.clock)().now;
        self.store.buy_boosters(set_code, kind, quantity, price, now).map(|_| ()).map_err(err)
    }

    /// Daily help for a player who is truly stranded.
    pub fn claim_charity(&mut self) -> Result<(), String> {
        let balance = self.store.balance().map_err(err)?;
        if !self.charity_open(balance)? {
            return Err("charity is not available".to_string());
        }
        let clock = (self.clock)();
        self.store.claim_charity(self.economy.charity.amount_cents, &clock.day, clock.now).map_err(err)
    }

    fn charity_open(&mut self, balance: i64) -> Result<bool, String> {
        let cheapest = self
            .shop()
            .ok()
            .and_then(|items| items.iter().map(|i| i.price_cents).min())
            .unwrap_or(self.economy.booster_prices.default_cents);
        if balance >= i64::from(cheapest) {
            return Ok(false); // can still buy a booster: no need to look further
        }
        let has_sealed = !self.store.sealed_stacks().map_err(err)?.is_empty();
        let has_sellable = self.has_sellable_cards()?;
        let claims = self.store.charity_claims_on(&(self.clock)().day).map_err(err)?;
        Ok(charity_available(&self.economy.charity, balance, cheapest, has_sellable, has_sealed, claims))
    }

    // ---- opening ----------------------------------------------------------------

    /// Opens one sealed booster the player owns and adds its cards to the collection.
    /// Cards come back in reveal order: the best card last.
    pub fn open_booster(&mut self, set_code: &str, kind: &str) -> Result<Vec<OpenedCard>, String> {
        self.ensure_loaded(set_code)?;
        self.ensure_prices();
        let Self { sets, prices, rng, store, economy, clock, .. } = self;
        let set = &sets[set_code];
        let cfg = set
            .file
            .boosters
            .get(kind)
            .ok_or_else(|| format!("set {set_code} has no `{kind}` booster"))?;
        let drawn = open_booster(cfg, &set.colors, rng).map_err(err)?;

        let mut cards: Vec<OpenedCard> = drawn
            .into_iter()
            .map(|d| {
                let card = &set.file.cards[set.index[&d.uuid]];
                let price = prices
                    .as_ref()
                    .and_then(|p| p.cards.get(&d.uuid))
                    .and_then(|p| economy.market_price_cents(p, d.foil));
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
            store.open_sealed(set_code, kind, &records, clock().now).map_err(err)?
        };
        for (card, is_new) in cards.iter_mut().zip(flags) {
            card.is_new = is_new;
        }
        Ok(cards)
    }

    // ---- collection and selling ------------------------------------------------------

    /// The player's collection for one set, with completion progress.
    pub fn collection(&mut self, set_code: &str) -> Result<CollectionView, String> {
        self.ensure_loaded(set_code)?;
        self.ensure_prices();
        let owned = self.store.owned_in_set(set_code).map_err(err)?;
        let set = &self.sets[set_code];

        let cards: Vec<CollectionCard> = set
            .obtainable
            .iter()
            .map(|&i| {
                let c = &set.file.cards[i];
                let have = owned.get(&c.uuid).copied().unwrap_or_default();
                CollectionCard {
                    uuid: c.uuid.clone(),
                    name: c.name.clone(),
                    number: c.number.clone(),
                    rarity: c.rarity.clone(),
                    image_url: image_url(&c.scryfall_id),
                    quantity: have.normal,
                    foil_quantity: have.foil,
                    price_cents: self.market_price(&c.uuid, false),
                    foil_price_cents: self.market_price(&c.uuid, true),
                    sell_value_cents: self.sale_value(&c.uuid, false),
                    foil_sell_value_cents: self.sale_value(&c.uuid, true),
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

    /// Sells cards at the Boosterium's price. Nothing is sold if any request is invalid.
    pub fn sell_cards(&mut self, requests: &[SaleRequest]) -> Result<SaleResult, String> {
        self.ensure_prices();
        let mut sales = Vec::with_capacity(requests.len());
        for r in requests {
            if r.quantity == 0 {
                return Err("cannot sell zero cards".to_string());
            }
            let unit_cents = self.sale_value(&r.uuid, r.foil);
            if unit_cents == 0 {
                return Err(format!("card {} has no market value and cannot be sold", r.uuid));
            }
            sales.push(Sale { uuid: &r.uuid, foil: r.foil, quantity: r.quantity, unit_cents });
        }
        let sold_count = sales.iter().map(|s| s.quantity).sum();
        let total_cents = self.store.sell_cards(&sales, (self.clock)().now).map_err(err)?;
        Ok(SaleResult { sold_count, total_cents })
    }

    /// Sells every copy beyond the first of each card and finish in a set.
    pub fn sell_duplicates(&mut self, set_code: &str) -> Result<SaleResult, String> {
        self.ensure_loaded(set_code)?;
        let owned = self.store.owned_in_set(set_code).map_err(err)?;
        let mut requests = Vec::new();
        for (uuid, have) in owned {
            for (foil, quantity) in [(false, have.normal), (true, have.foil)] {
                if quantity > 1 && self.sale_value(&uuid, foil) > 0 {
                    requests.push(SaleRequest { uuid: uuid.clone(), foil, quantity: quantity - 1 });
                }
            }
        }
        if requests.is_empty() {
            return Ok(SaleResult { sold_count: 0, total_cents: 0 });
        }
        self.sell_cards(&requests)
    }

    fn has_sellable_cards(&mut self) -> Result<bool, String> {
        self.ensure_prices();
        let owned = self.store.owned_cards().map_err(err)?;
        Ok(owned.iter().any(|(uuid, foil, _)| self.sale_value(uuid, *foil) > 0))
    }

    fn market_price(&self, uuid: &str, foil: bool) -> Option<u32> {
        let prices = self.prices.as_ref()?.cards.get(uuid)?;
        self.economy.market_price_cents(prices, foil)
    }

    /// What the Boosterium pays for one copy; 0 when the card has no market price.
    fn sale_value(&self, uuid: &str, foil: bool) -> u32 {
        self.market_price(uuid, foil).map_or(0, |m| self.economy.sell_value_cents(m))
    }

    // ---- data loading ---------------------------------------------------------------

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
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    const ECONOMY: &str = include_str!("../../../config/economy.json");

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

    /// Common is worth $1.00, the mythic $10.00.
    const PRICES: &str = r#"{ "updatedAt": "d", "cards": { "c1": { "usd": 100 }, "m1": { "usd": 1000 } } }"#;
    const NO_PRICES: &str = r#"{ "updatedAt": "d", "cards": {} }"#;

    fn data_dir(name: &str, prices: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("unsealed-test-{}-{name}", std::process::id()));
        fs::create_dir_all(dir.join("sets")).unwrap();
        fs::write(dir.join("sets/TST.json"), SET).unwrap();
        fs::write(dir.join("manifest.json"), MANIFEST).unwrap();
        fs::write(dir.join("prices.json"), prices).unwrap();
        dir
    }

    /// A game plus a handle to change the current day (October, 2026).
    fn game_with(name: &str, prices: &str) -> (Game, Arc<AtomicU32>) {
        let day = Arc::new(AtomicU32::new(13));
        let handle = day.clone();
        let clock: ClockFn = Box::new(move || Clock {
            now: 0,
            day: format!("2026-10-{:02}", handle.load(Ordering::SeqCst)),
        });
        let game = Game::with_parts(
            data_dir(name, prices),
            Store::open_in_memory().unwrap(),
            EconomyConfig::from_json(ECONOMY).unwrap(),
            ChaCha8Rng::seed_from_u64(1),
            clock,
        )
        .unwrap();
        (game, day)
    }

    fn game(name: &str) -> Game {
        game_with(name, PRICES).0
    }

    /// A game where the player already bought and can open one booster.
    fn game_with_booster(name: &str) -> Game {
        let mut g = game(name);
        g.buy_boosters("TST", "play", 1).unwrap();
        g
    }

    fn balance(g: &mut Game) -> i64 {
        g.wallet().unwrap().balance_cents
    }

    #[test]
    fn lists_sets_and_shop_prices() {
        let g = game("list");
        let sets = g.list_sets().unwrap();
        assert_eq!((sets.len(), sets[0].code.as_str(), sets[0].boosters.as_slice()), (1, "TST", ["play".to_string()].as_slice()));
        assert_eq!(
            g.shop().unwrap(),
            [ShopItem { set_code: "TST".into(), set_name: "Test Set".into(), kind: "play".into(), price_cents: 400 }]
        );
    }

    #[test]
    fn a_new_player_has_the_starting_capital_and_no_help() {
        let mut g = game("wallet");
        let w = g.wallet().unwrap();
        assert_eq!((w.balance_cents, w.currency.symbol.as_str()), (10_000, "$"));
        assert!(w.sealed.is_empty());
        assert!(!w.charity.available);
    }

    #[test]
    fn buying_stocks_boosters_and_opening_consumes_them() {
        let mut g = game("buy");
        g.buy_boosters("TST", "play", 2).unwrap();
        let w = g.wallet().unwrap();
        assert_eq!(w.balance_cents, 9_200);
        assert_eq!(w.sealed, [SealedStack { set_code: "TST".into(), set_name: "Test Set".into(), kind: "play".into(), quantity: 2 }]);

        g.open_booster("TST", "play").unwrap();
        assert_eq!(g.wallet().unwrap().sealed[0].quantity, 1);
    }

    #[test]
    fn opening_requires_a_sealed_booster() {
        let mut g = game("no-sealed");
        assert!(g.open_booster("TST", "play").is_err());
        assert!(g.collection("TST").unwrap().cards.iter().all(|c| c.quantity == 0));
    }

    #[test]
    fn purchases_are_validated() {
        let mut g = game("validate");
        assert!(g.buy_boosters("TST", "play", 0).is_err());
        assert!(g.buy_boosters("TST", "play", 101).is_err());
        assert!(g.buy_boosters("TST", "collector", 1).is_err());
        assert!(g.buy_boosters("../x", "play", 1).is_err());
        assert!(g.buy_boosters("TST", "play", 26).is_err()); // 26 x $4 > $100
        assert_eq!(balance(&mut g), 10_000);
        g.buy_boosters("TST", "play", 25).unwrap(); // exactly $100
        assert_eq!(balance(&mut g), 0);
    }

    #[test]
    fn opens_a_booster_with_the_best_card_last_and_prices() {
        let cards = game_with_booster("open").open_booster("TST", "play").unwrap();
        let names: Vec<&str> = cards.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Common", "Common", "Mythic"]);
        assert_eq!(cards[0].price_cents, Some(100));
        assert_eq!(cards[2].price_cents, Some(1_000));
        assert_eq!(cards[0].image_url, "https://cards.scryfall.io/normal/front/a/b/abcdef.jpg");
    }

    #[test]
    fn flags_new_cards_only_on_first_discovery() {
        let mut g = game("new");
        g.buy_boosters("TST", "play", 2).unwrap();
        let first: Vec<bool> = g.open_booster("TST", "play").unwrap().iter().map(|c| c.is_new).collect();
        assert_eq!(first, [true, false, true]); // Common, Common (duplicate), Mythic
        let second: Vec<bool> = g.open_booster("TST", "play").unwrap().iter().map(|c| c.is_new).collect();
        assert_eq!(second, [false, false, false]);
    }

    #[test]
    fn collection_tracks_progress_on_obtainable_cards_only() {
        let mut g = game_with_booster("collection");
        let empty = g.collection("TST").unwrap();
        assert_eq!((empty.owned_count, empty.total_count), (0, 2)); // x1 is not in any sheet
        assert_eq!(empty.completion_percent, 0.0);

        g.open_booster("TST", "play").unwrap();
        let view = g.collection("TST").unwrap();
        assert_eq!((view.owned_count, view.total_count), (2, 2));
        assert_eq!(view.completion_percent, 100.0);
        // Sorted by collector number: 2 before 10; the sell value is 80% of the market price.
        let rows: Vec<(&str, u32, u32)> =
            view.cards.iter().map(|c| (c.name.as_str(), c.quantity, c.sell_value_cents)).collect();
        assert_eq!(rows, [("Mythic", 1, 800), ("Common", 2, 80)]);
    }

    #[test]
    fn selling_pays_80_percent_and_can_sacrifice_the_last_copy() {
        let mut g = game_with_booster("sell");
        g.open_booster("TST", "play").unwrap();
        let before = balance(&mut g);

        let sold = g.sell_cards(&[SaleRequest { uuid: "m1".into(), foil: false, quantity: 1 }]).unwrap();
        assert_eq!(sold, SaleResult { sold_count: 1, total_cents: 800 });
        assert_eq!(balance(&mut g), before + 800);
        // The mythic was the only copy: it left the collection.
        let view = g.collection("TST").unwrap();
        assert_eq!((view.owned_count, view.total_count), (1, 2));
    }

    #[test]
    fn invalid_sales_change_nothing() {
        let mut g = game_with_booster("bad-sale");
        g.open_booster("TST", "play").unwrap();
        let before = balance(&mut g);
        let sale = |uuid: &str, quantity| SaleRequest { uuid: uuid.into(), foil: false, quantity };
        assert!(g.sell_cards(&[sale("c1", 3)]).is_err()); // only 2 owned
        assert!(g.sell_cards(&[sale("c1", 0)]).is_err());
        assert!(g.sell_cards(&[sale("x1", 1)]).is_err()); // not owned, no price
        assert!(g.sell_cards(&[sale("c1", 1), sale("m1", 2)]).is_err()); // one bad line cancels all
        assert_eq!(balance(&mut g), before);
        assert_eq!(g.collection("TST").unwrap().cards[1].quantity, 2);
    }

    #[test]
    fn sell_duplicates_keeps_one_copy_of_each_card() {
        let mut g = game_with_booster("dupes");
        g.open_booster("TST", "play").unwrap();
        let result = g.sell_duplicates("TST").unwrap();
        assert_eq!(result, SaleResult { sold_count: 1, total_cents: 80 });
        assert_eq!(g.collection("TST").unwrap().owned_count, 2);
        assert_eq!(g.sell_duplicates("TST").unwrap(), SaleResult { sold_count: 0, total_cents: 0 });
    }

    #[test]
    fn help_is_only_for_players_who_are_truly_stranded() {
        let (mut g, day) = game_with("charity", NO_PRICES);
        assert!(g.claim_charity().is_err()); // $100: not stranded

        // Spend everything: $0 but 25 sealed boosters to open, so still not stranded.
        g.buy_boosters("TST", "play", 25).unwrap();
        assert!(!g.wallet().unwrap().charity.available);
        for _ in 0..25 {
            g.open_booster("TST", "play").unwrap();
        }
        // Nothing left to open and the cards have no market value: stranded.
        assert!(g.wallet().unwrap().charity.available);
        g.claim_charity().unwrap();
        assert_eq!(balance(&mut g), 2_000);
        assert!(g.claim_charity().is_err()); // not stranded any more

        // Spend the $20 too: stranded again, but only one claim per day.
        g.buy_boosters("TST", "play", 5).unwrap();
        for _ in 0..5 {
            g.open_booster("TST", "play").unwrap();
        }
        assert!(!g.wallet().unwrap().charity.available);
        assert!(g.claim_charity().is_err());

        // The next day the help is available again.
        day.store(14, Ordering::SeqCst);
        assert!(g.wallet().unwrap().charity.available);
        g.claim_charity().unwrap();
        assert_eq!(balance(&mut g), 2_000);
    }

    #[test]
    fn owning_a_card_worth_selling_means_no_help() {
        let (mut g, _) = game_with("sacrifice", PRICES);
        g.buy_boosters("TST", "play", 25).unwrap();
        for _ in 0..25 {
            g.open_booster("TST", "play").unwrap();
        }
        // $0 and nothing to open, but the cards are worth money: sell one instead.
        assert_eq!(balance(&mut g), 0);
        assert!(!g.wallet().unwrap().charity.available);
    }

    #[test]
    fn same_seed_gives_the_same_booster() {
        let a = game_with_booster("seed-a").open_booster("TST", "play").unwrap();
        let b = game_with_booster("seed-b").open_booster("TST", "play").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn rejects_bad_codes_and_unknown_kinds() {
        let mut g = game_with_booster("errors");
        assert!(g.open_booster("../secret", "play").is_err());
        assert!(g.open_booster("", "play").is_err());
        assert!(g.open_booster("NOPE", "play").is_err());
        assert!(g.open_booster("TST", "collector").is_err());
        assert!(g.collection("../secret").is_err());
        assert!(g.sell_duplicates("../secret").is_err());
    }

    /// Needs the real data: run `data-builder` first, then
    /// `cargo test -p unsealed-app -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_data_smoke_test() {
        let dist = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist");
        let mut g = Game::with_parts(
            dist,
            Store::open_in_memory().unwrap(),
            EconomyConfig::from_json(ECONOMY).unwrap(),
            ChaCha8Rng::seed_from_u64(7),
            Box::new(system_clock),
        )
        .unwrap();
        let sets = g.list_sets().unwrap();
        assert!(!sets.is_empty());
        for s in &sets {
            g.buy_boosters(&s.code, "play", 8).unwrap();
            let mut new_cards = 0;
            for _ in 0..8 {
                let cards = g.open_booster(&s.code, "play").unwrap();
                assert!(cards.len() >= 12);
                assert!(cards.iter().all(|c| c.image_url.starts_with("https://cards.scryfall.io/")));
                // Cards from other sets (e.g. Special Guests) do not count for this set.
                new_cards += cards.iter().filter(|c| c.is_new && c.set_code == s.code).count();
            }
            let view = g.collection(&s.code).unwrap();
            let sold = g.sell_duplicates(&s.code).unwrap();
            println!(
                "{}: {}/{} cards ({:.1}%), {new_cards} discoveries over 8 boosters, duplicates sold for {} cents",
                s.code, view.owned_count, view.total_count, view.completion_percent, sold.total_cents
            );
            assert_eq!(view.owned_count, new_cards);
            assert!(view.owned_count > 0 && view.owned_count <= view.total_count);
        }
        println!("balance after the session: {} cents", g.wallet().unwrap().balance_cents);
    }
}
