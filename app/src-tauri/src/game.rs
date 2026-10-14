//! Game logic exposed to the UI. Independent from Tauri so it can be unit tested.

mod achievements;
mod details;
mod history;

pub use achievements::{AchievementTexts, AchievementView, AchievementsView};
use achievements::{OpeningEvents, PendingReveal};
pub use details::CardDetailsView;
pub use history::HistoryView;

use crate::store::{OpeningCard, OpeningOrder, OpeningRow, Owned, Sale, Store};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use unsealed_core::achievements::{Engine, Event};
use unsealed_core::catalog::{CardEntry, Manifest, PriceFile, SetFile, COLLECTOR_KIND, FOREIGN_LANGUAGES};
use unsealed_core::economy::{charity_available, EconomyConfig};
use unsealed_core::limited::LimitedConfig;
use unsealed_core::{open_booster_excluding, ColorIndex};

/// Largest number of boosters bought in one purchase.
const MAX_PURCHASE: u32 = 100;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetSummary {
    pub code: String,
    /// In the card language when the set's name was translated, else in English.
    pub name: String,
    /// Always in English, so a search finds the set by either name.
    pub english_name: String,
    pub release_date: String,
    pub boosters: Vec<String>,
    /// Art illustrating the set, with its credit.
    pub art: Option<SetArt>,
    /// The set's expansion symbol (black SVG, shown unaltered).
    pub icon_url: Option<String>,
}

/// The art crop of a set's featured card. Scryfall requires the artist to be credited
/// wherever an art crop is shown, and the image to be left unaltered.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetArt {
    pub image_url: String,
    pub card_name: String,
    pub artist: String,
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
    /// Back face, for double-faced cards.
    pub back_image_url: Option<String>,
    /// Market price in the game currency (cents), if known.
    pub price_cents: Option<u32>,
    /// The card has no market price of its own: `price_cents` is estimated.
    pub price_estimated: bool,
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
    /// Back face, for double-faced cards.
    pub back_image_url: Option<String>,
    pub quantity: u32,
    pub foil_quantity: u32,
    pub price_cents: Option<u32>,
    pub foil_price_cents: Option<u32>,
    /// Both prices are estimated (see `OpenedCard::price_estimated`).
    pub price_estimated: bool,
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
    /// Cards only collector boosters give; they do not count toward completion.
    pub extras: Vec<CollectionCard>,
    pub extras_owned: usize,
    /// The set's tokens (one in each booster); they do not count toward completion.
    pub tokens: Vec<CollectionCard>,
    pub tokens_owned: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// How the interface shows amounts: in Seals.
pub struct CurrencyView {
    pub seals_per_cent: f64,
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
    /// What the player paid for the whole stack (0 for free boosters).
    pub cost_cents: i64,
    /// Current price of one booster.
    pub price_cents: u32,
    /// What the Boosterium pays today for one booster of this stack.
    pub sell_value_cents: u32,
    /// Recent prices of one booster, oldest first.
    pub history: Vec<u32>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BoosterSaleResult {
    pub quantity: u32,
    pub total_cents: i64,
    /// Proceeds minus the cost basis of the sold boosters.
    pub profit_cents: i64,
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
    /// What the Boosterium would pay today for such a booster.
    pub sell_back_cents: u32,
    /// Recent prices, oldest first (one per price update seen by this install).
    pub history: Vec<u32>,
}

/// One past opening.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpeningView {
    pub id: i64,
    pub opened_at: i64,
    pub set_code: String,
    pub set_name: String,
    pub kind: String,
    /// Market value of its cards when it was opened.
    pub value_cents: i64,
    /// Its most valuable card, priced when it was pulled.
    pub best_card: Option<OpenedCard>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullView {
    /// Priced when it was pulled.
    pub card: OpenedCard,
    pub opened_at: i64,
}

/// Openings of one kind of booster.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BoosterStatsView {
    pub set_code: String,
    pub set_name: String,
    pub kind: String,
    pub boosters: u32,
    pub value_cents: i64,
    pub average_cents: i64,
    pub best_value_cents: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsView {
    pub boosters_opened: u32,
    pub cards_pulled: u32,
    pub foils_pulled: u32,
    /// Market value of every pulled card, when it was pulled.
    pub value_pulled_cents: i64,
    pub spent_on_boosters_cents: i64,
    pub earned_from_cards_cents: i64,
    pub earned_from_boosters_cents: i64,
    pub rewards_cents: i64,
    pub charity_cents: i64,
    pub balance_cents: i64,
    /// Distinct cards owned (any finish).
    pub distinct_cards: u32,
    /// Today's market value of the collection.
    pub collection_value_cents: i64,
    /// What the Boosterium would pay today for the sealed boosters.
    pub sealed_value_cents: i64,
    pub best_booster: Option<OpeningView>,
    pub worst_booster: Option<OpeningView>,
    pub recent_openings: Vec<OpeningView>,
    pub best_pulls: Vec<PullView>,
    pub boosters: Vec<BoosterStatsView>,
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
    /// Indices (in `file.cards`) of the set's own cards that its main boosters can give,
    /// in collector number order. Defines what "complete" means for this set.
    obtainable: Vec<usize>,
    /// The set's own cards that only collector boosters give (showcase, borderless,
    /// special foils...): collectible, but outside the completion target.
    extras: Vec<usize>,
    /// The set's tokens: outside the completion target too.
    tokens: Vec<usize>,
}

pub struct Game {
    data_dir: PathBuf,
    store: Store,
    economy: EconomyConfig,
    engine: Engine,
    /// Ids of the achievements unlocked and not yet shown to the player.
    notifications: Vec<String>,
    achievement_texts: AchievementTexts,
    /// See `set_card_language`.
    card_lang: String,
    /// The booster being revealed, whose achievement events wait (see `reveal_cards`).
    pending_reveal: Option<PendingReveal>,
    sets: HashMap<String, LoadedSet>,
    /// See `set_limited`.
    limited: LimitedConfig,
    /// Print run of each limited card of the loaded sets, by uuid.
    print_runs: HashMap<String, u32>,
    prices: Option<PriceFile>,
    /// Modification time of the prices file that `prices` was loaded from.
    prices_stamp: Option<SystemTime>,
    rng: ChaCha8Rng,
    clock: ClockFn,
}

/// How many past prices the Boosterium shows per booster.
const PRICE_HISTORY_POINTS: u32 = 30;
/// How many openings the statistics list.
const RECENT_OPENINGS: u32 = 20;
/// How many of the best cards ever pulled the statistics show.
const BEST_PULLS: u32 = 12;

fn err<E: ToString>(e: E) -> String {
    e.to_string()
}

/// A game closed in the middle of an opening still counts every card it gave.
impl Drop for Game {
    fn drop(&mut self) {
        self.flush_reveal();
    }
}

impl Game {
    pub fn new(data_dir: PathBuf, store: Store, economy: EconomyConfig, engine: Engine) -> Result<Self, String> {
        Self::with_parts(data_dir, store, economy, engine, ChaCha8Rng::from_entropy(), Box::new(system_clock))
    }

    pub fn with_parts(
        data_dir: PathBuf,
        mut store: Store,
        economy: EconomyConfig,
        engine: Engine,
        rng: ChaCha8Rng,
        clock: ClockFn,
    ) -> Result<Self, String> {
        store.ensure_starting_balance(economy.starting_balance_cents, clock().now).map_err(err)?;
        Ok(Self {
            data_dir,
            store,
            economy,
            engine,
            notifications: Vec::new(),
            achievement_texts: AchievementTexts::default(),
            card_lang: "en".to_string(),
            pending_reveal: None,
            sets: HashMap::new(),
            limited: LimitedConfig::default(),
            print_runs: HashMap::new(),
            prices: None,
            prices_stamp: None,
            rng,
            clock,
        })
    }

    /// Forgets everything loaded from the data directory; called after a background refresh.
    pub fn reload_data(&mut self) {
        self.sets.clear();
        self.print_runs.clear();
        self.prices = None;
        self.prices_stamp = None;
    }

    /// The cards of a limited print run: no more copies of them can be pulled in this game
    /// than were printed.
    pub fn set_limited(&mut self, limited: LimitedConfig) {
        self.limited = limited;
        self.sets.clear();
        self.print_runs.clear();
    }

    pub fn list_sets(&self) -> Result<Vec<SetSummary>, String> {
        read_sets(&self.data_dir, &self.card_lang)
    }

    /// Language of card names, card images and set names ("en", or one of
    /// `FOREIGN_LANGUAGES`); anything not translated stays in English.
    pub fn set_card_language(&mut self, lang: &str) {
        self.card_lang = if is_card_language(lang) { lang.to_string() } else { "en".to_string() };
    }

    // ---- Boosterium: wallet, shop, charity -------------------------------------

    pub fn shop(&mut self) -> Result<Vec<ShopItem>, String> {
        self.ensure_prices();
        let mut items = Vec::new();
        for set in self.list_sets()? {
            for kind in &set.boosters {
                let price_cents = self.booster_price(&set.code, kind);
                items.push(ShopItem {
                    price_cents,
                    sell_back_cents: self.economy.booster_sell_value_cents(price_cents),
                    set_code: set.code.clone(),
                    set_name: set.name.clone(),
                    history: self.price_history(&set.code, kind),
                    kind: kind.clone(),
                });
            }
        }
        Ok(items)
    }

    pub fn wallet(&mut self) -> Result<WalletView, String> {
        self.ensure_prices();
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
            .map(|row| {
                let price_cents = self.booster_price(&row.set_code, &row.kind);
                let history = self.price_history(&row.set_code, &row.kind);
                SealedStack {
                    set_name: names.get(&row.set_code).cloned().unwrap_or_else(|| row.set_code.clone()),
                    set_code: row.set_code,
                    kind: row.kind,
                    quantity: row.quantity,
                    cost_cents: row.cost_cents,
                    price_cents,
                    sell_value_cents: self.economy.booster_sell_value_cents(price_cents),
                    history,
                }
            })
            .collect();
        Ok(WalletView {
            balance_cents: balance,
            currency: CurrencyView { seals_per_cent: self.economy.currency.seals_per_cent },
            charity: CharityStatus { amount_cents: self.economy.charity.amount_cents, available: charity_open },
            sealed,
        })
    }

    pub fn buy_boosters(&mut self, set_code: &str, kind: &str, quantity: u32) -> Result<(), String> {
        self.flush_reveal();
        if !(1..=MAX_PURCHASE).contains(&quantity) {
            return Err(format!("quantity must be between 1 and {MAX_PURCHASE}"));
        }
        self.ensure_loaded(set_code)?;
        if !self.sets[set_code].file.boosters.contains_key(kind) {
            return Err(format!("set {set_code} has no `{kind}` booster"));
        }
        self.ensure_prices();
        let price = self.booster_price(set_code, kind);
        let now = (self.clock)().now;
        self.store.buy_boosters(set_code, kind, quantity, price, now).map_err(err)?;

        let mut events = vec![Event::new("BoosterBought")
            .with("setCode", set_code)
            .with("kind", kind)
            .with("quantity", quantity)
            .with("totalCents", i64::from(price) * i64::from(quantity))];
        events.extend(self.balance_event());
        self.emit(events);
        Ok(())
    }

    /// Sells sealed boosters back at today's price minus the Boosterium's margin. As the
    /// price follows the market, this is how a player can speculate on boosters.
    pub fn sell_boosters(&mut self, set_code: &str, kind: &str, quantity: u32) -> Result<BoosterSaleResult, String> {
        self.flush_reveal();
        if !(1..=MAX_PURCHASE).contains(&quantity) {
            return Err(format!("quantity must be between 1 and {MAX_PURCHASE}"));
        }
        self.ensure_prices();
        let unit = self.economy.booster_sell_value_cents(self.booster_price(set_code, kind));
        let now = (self.clock)().now;
        let sale = self.store.sell_boosters(set_code, kind, quantity, unit, now).map_err(err)?;
        let result = BoosterSaleResult {
            quantity,
            total_cents: sale.total_cents,
            profit_cents: sale.total_cents - sale.cost_cents,
        };

        let mut events = vec![Event::new("BoosterSold")
            .with("setCode", set_code)
            .with("kind", kind)
            .with("quantity", quantity)
            .with("totalCents", result.total_cents)
            .with("profitCents", result.profit_cents)];
        events.extend(self.balance_event());
        self.emit(events);
        Ok(result)
    }

    /// Current price of a sealed booster (follows the market unless overridden in the config).
    fn booster_price(&self, set_code: &str, kind: &str) -> u32 {
        let ev = self
            .prices
            .as_ref()
            .and_then(|p| p.boosters.get(&format!("{set_code}/{kind}")))
            .map(|v| if self.economy.currency.market == "EUR" { v.eur } else { v.usd });
        self.economy.booster_price_cents(set_code, kind, ev)
    }

    fn price_history(&self, set_code: &str, kind: &str) -> Vec<u32> {
        self.store.booster_price_history(set_code, kind, PRICE_HISTORY_POINTS).unwrap_or_default()
    }

    /// Daily help for a player who is truly stranded.
    pub fn claim_charity(&mut self) -> Result<(), String> {
        self.flush_reveal();
        let balance = self.store.balance().map_err(err)?;
        if !self.charity_open(balance)? {
            return Err("charity is not available".to_string());
        }
        let clock = (self.clock)();
        let amount = self.economy.charity.amount_cents;
        self.store.claim_charity(amount, &clock.day, clock.now).map_err(err)?;

        let mut events = vec![Event::new("CharityClaimed").with("amountCents", amount)];
        events.extend(self.balance_event());
        self.emit(events);
        Ok(())
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
    /// Cards come back in reveal order: the best card last. Their achievement events wait
    /// for the cards to be revealed (`reveal_cards`).
    pub fn open_booster(&mut self, set_code: &str, kind: &str) -> Result<Vec<OpenedCard>, String> {
        self.open_booster_as(set_code, kind, true)
    }

    /// `open_booster`, saying whether the player watches the opening (`animated`) or skips it
    /// (quick opening): achievements count both ways apart.
    pub fn open_booster_as(&mut self, set_code: &str, kind: &str, animated: bool) -> Result<Vec<OpenedCard>, String> {
        self.flush_reveal();
        let (cards, events) = self.open_one(set_code, kind, animated)?;
        self.pending_reveal = Some(PendingReveal { per_card: events.per_card, booster: events.booster, revealed: 0 });
        Ok(cards)
    }

    /// Draws one booster and records it, returning its cards and their events (not emitted).
    fn open_one(&mut self, set_code: &str, kind: &str, animated: bool) -> Result<(Vec<OpenedCard>, OpeningEvents), String> {
        self.ensure_loaded(set_code)?;
        self.ensure_prices();
        // Cards of a limited print run whose every copy was pulled in this game.
        let exhausted: HashSet<String> = self
            .store
            .limited_pulls()
            .map_err(err)?
            .into_iter()
            .filter(|(uuid, pulled)| self.print_runs.get(uuid).is_some_and(|run| pulled >= run))
            .map(|(uuid, _)| uuid)
            .collect();
        let Self { sets, prices, rng, store, economy, clock, card_lang, print_runs, limited, .. } = self;
        let set = &sets[set_code];
        let cfg = set
            .file
            .boosters
            .get(kind)
            .ok_or_else(|| format!("set {set_code} has no `{kind}` booster"))?;
        let drawn = open_booster_excluding(cfg, &set.colors, rng, &exhausted).map_err(err)?;

        let mut cards: Vec<OpenedCard> = drawn
            .into_iter()
            .map(|d| {
                let card = &set.file.cards[set.index[&d.uuid]];
                let price = market_price(prices.as_ref(), economy, limited, print_runs, &d.uuid, d.foil);
                let price_estimated = prices.as_ref().and_then(|p| p.cards.get(&d.uuid)).is_some_and(|p| p.estimated);
                let shown = localized(card, card_lang);
                OpenedCard {
                    image_url: shown.image_url,
                    back_image_url: shown.back_image_url,
                    uuid: d.uuid,
                    name: shown.name,
                    number: card.number.clone(),
                    rarity: card.rarity.clone(),
                    set_code: card.set_code.clone(),
                    scryfall_id: card.scryfall_id.clone(),
                    foil: d.foil,
                    sheet: d.sheet,
                    price_cents: price,
                    price_estimated,
                    is_new: false,
                }
            })
            .collect();
        cards.sort_by_key(|c| reveal_rank(kind, c));

        let outcome = {
            let records: Vec<OpeningCard<'_>> = cards
                .iter()
                .map(|c| OpeningCard {
                    uuid: &c.uuid,
                    set_code: &c.set_code,
                    foil: c.foil,
                    price_cents: c.price_cents,
                    limited: print_runs.contains_key(&c.uuid),
                })
                .collect();
            store.open_sealed(set_code, kind, &records, clock().now).map_err(err)?
        };
        for (card, is_new) in cards.iter_mut().zip(&outcome.is_new) {
            card.is_new = *is_new;
        }

        let events = self.opening_events(set_code, kind, &cards, outcome.cost_cents, animated);
        Ok((cards, events))
    }

    /// Opens up to `quantity` sealed boosters in a row. Each opening is its own transaction, so
    /// running out of stock part-way keeps (and returns) what was already opened. Played fast
    /// (they count as opened without animation), so their events are emitted at once (the
    /// interface holds the notifications back).
    pub fn open_boosters(&mut self, set_code: &str, kind: &str, quantity: u32) -> Result<Vec<Vec<OpenedCard>>, String> {
        self.flush_reveal();
        let mut opened = Vec::new();
        for _ in 0..quantity {
            match self.open_one(set_code, kind, false) {
                Ok((cards, events)) => {
                    self.emit(events.per_card.into_iter().chain(events.booster).collect());
                    opened.push(cards);
                }
                Err(_) if !opened.is_empty() => break,
                Err(e) => return Err(e),
            }
        }
        Ok(opened)
    }

    // ---- collection and selling ------------------------------------------------------

    /// The player's collection for one set, with completion progress.
    pub fn collection(&mut self, set_code: &str) -> Result<CollectionView, String> {
        self.ensure_loaded(set_code)?;
        self.ensure_prices();
        let owned = self.owned_with_tokens(set_code)?;
        let set = &self.sets[set_code];

        let view = |indices: &[usize]| -> Vec<CollectionCard> {
            indices
                .iter()
                .map(|&i| {
                    let c = &set.file.cards[i];
                    let have = owned.get(&c.uuid).copied().unwrap_or_default();
                    let shown = localized(c, &self.card_lang);
                    CollectionCard {
                        uuid: c.uuid.clone(),
                        name: shown.name,
                        number: c.number.clone(),
                        rarity: c.rarity.clone(),
                        image_url: shown.image_url,
                        back_image_url: shown.back_image_url,
                        quantity: have.normal,
                        foil_quantity: have.foil,
                        price_cents: self.market_price(&c.uuid, false),
                        foil_price_cents: self.market_price(&c.uuid, true),
                        price_estimated: self.price_estimated(&c.uuid),
                        sell_value_cents: self.sale_value(&c.uuid, false),
                        foil_sell_value_cents: self.sale_value(&c.uuid, true),
                    }
                })
                .collect()
        };
        let owned_in = |cards: &[CollectionCard]| cards.iter().filter(|c| c.quantity + c.foil_quantity > 0).count();
        let cards = view(&set.obtainable);
        let extras = view(&set.extras);
        let tokens = view(&set.tokens);
        let owned_count = owned_in(&cards);
        let total_count = cards.len();
        let completion_percent =
            if total_count == 0 { 0.0 } else { owned_count as f64 * 100.0 / total_count as f64 };
        Ok(CollectionView {
            set_code: set.file.code.clone(),
            set_name: set.file.names.get(&self.card_lang).unwrap_or(&set.file.name).clone(),
            owned_count,
            total_count,
            completion_percent,
            extras_owned: owned_in(&extras),
            tokens_owned: owned_in(&tokens),
            cards,
            extras,
            tokens,
        })
    }

    /// Sells cards at the Boosterium's price. Nothing is sold if any request is invalid.
    pub fn sell_cards(&mut self, requests: &[SaleRequest]) -> Result<SaleResult, String> {
        self.flush_reveal();
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
        let context = self.sale_context();
        let now = (self.clock)().now;
        let total_cents = self.store.sell_cards(&sales, now).map_err(err)?;

        let events = self.sale_events(&context, requests, total_cents);
        self.emit(events);
        Ok(SaleResult { sold_count, total_cents })
    }

    /// Owned copies of a loaded set's cards, its tokens included: the collection files each
    /// card under its own set, and tokens have theirs ("TLTR" for "LTR").
    fn owned_with_tokens(&self, set_code: &str) -> Result<HashMap<String, Owned>, String> {
        let mut owned = self.store.owned_in_set(set_code).map_err(err)?;
        let set = &self.sets[set_code];
        let token_sets: HashSet<&str> =
            set.tokens.iter().map(|&i| set.file.cards[i].set_code.as_str()).filter(|c| *c != set_code).collect();
        for code in token_sets {
            owned.extend(self.store.owned_in_set(code).map_err(err)?);
        }
        Ok(owned)
    }

    /// Sells every copy beyond the first of each card and finish in a set (and its tokens).
    pub fn sell_duplicates(&mut self, set_code: &str) -> Result<SaleResult, String> {
        self.ensure_loaded(set_code)?;
        let owned = self.owned_with_tokens(set_code)?;
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

    // ---- statistics ------------------------------------------------------------------

    /// Personal statistics: what was opened, spent and earned, and the best pulls.
    pub fn stats(&mut self) -> Result<StatsView, String> {
        self.ensure_prices();
        let names: HashMap<String, String> = self
            .list_sets()
            .map(|sets| sets.into_iter().map(|s| (s.code, s.name)).collect())
            .unwrap_or_default();
        let set_name = |code: &str| names.get(code).cloned().unwrap_or_else(|| code.to_string());

        let totals = self.store.opening_totals().map_err(err)?;
        let ledger = self.store.ledger_totals().map_err(err)?;
        let ledger_sum = |kind: &str| ledger.get(kind).copied().unwrap_or(0);

        let owned = self.store.owned_cards().map_err(err)?;
        let distinct_cards = owned.iter().map(|(uuid, _, _)| uuid).collect::<HashSet<_>>().len() as u32;
        let collection_value_cents = owned
            .iter()
            .map(|(uuid, foil, quantity)| {
                i64::from(self.market_price(uuid, *foil).unwrap_or(0)) * i64::from(*quantity)
            })
            .sum();
        let sealed_value_cents = self
            .store
            .sealed_stacks()
            .map_err(err)?
            .iter()
            .map(|s| {
                let unit = self.economy.booster_sell_value_cents(self.booster_price(&s.set_code, &s.kind));
                i64::from(unit) * i64::from(s.quantity)
            })
            .sum();

        let best_booster = self.top_opening(OpeningOrder::MostValuable, &names)?;
        let worst_booster = self.top_opening(OpeningOrder::LeastValuable, &names)?;
        let recent_openings = self
            .store
            .openings(OpeningOrder::Latest, RECENT_OPENINGS)
            .map_err(err)?
            .into_iter()
            .map(|row| self.opening_view(row, &names))
            .collect();

        let best_pulls = self
            .store
            .best_pulls(BEST_PULLS)
            .map_err(err)?
            .into_iter()
            .filter_map(|p| {
                let card = self.pulled_card(&p.booster_set, &p.uuid, p.foil, Some(p.price_cents))?;
                Some(PullView { card, opened_at: p.opened_at })
            })
            .collect();

        let boosters = self
            .store
            .openings_by_booster()
            .map_err(err)?
            .into_iter()
            .map(|b| BoosterStatsView {
                set_name: set_name(&b.set_code),
                average_cents: b.value_cents / i64::from(b.boosters.max(1)),
                set_code: b.set_code,
                kind: b.kind,
                boosters: b.boosters,
                value_cents: b.value_cents,
                best_value_cents: b.best_value_cents,
            })
            .collect();

        Ok(StatsView {
            boosters_opened: totals.boosters,
            cards_pulled: totals.cards,
            foils_pulled: totals.foils,
            value_pulled_cents: totals.value_cents,
            spent_on_boosters_cents: -ledger_sum("booster_purchase"),
            earned_from_cards_cents: ledger_sum("card_sale"),
            earned_from_boosters_cents: ledger_sum("booster_sale"),
            rewards_cents: ledger_sum("achievement_reward"),
            charity_cents: ledger_sum("charity"),
            balance_cents: self.store.balance().map_err(err)?,
            distinct_cards,
            collection_value_cents,
            sealed_value_cents,
            best_booster,
            worst_booster,
            recent_openings,
            best_pulls,
            boosters,
        })
    }

    fn top_opening(&mut self, order: OpeningOrder, names: &HashMap<String, String>) -> Result<Option<OpeningView>, String> {
        let row = self.store.openings(order, 1).map_err(err)?.into_iter().next();
        Ok(row.map(|row| self.opening_view(row, names)))
    }

    fn opening_view(&mut self, row: OpeningRow, names: &HashMap<String, String>) -> OpeningView {
        let best_card = row
            .best_card
            .and_then(|b| self.pulled_card(&row.set_code, &b.uuid, b.foil, Some(b.price_cents)));
        OpeningView {
            set_name: names.get(&row.set_code).cloned().unwrap_or_else(|| row.set_code.clone()),
            id: row.id,
            opened_at: row.opened_at,
            set_code: row.set_code,
            kind: row.kind,
            value_cents: row.value_cents,
            best_card,
        }
    }

    /// A card pulled from a booster of `booster_set`, or `None` if its data is no longer available.
    fn pulled_card(&mut self, booster_set: &str, uuid: &str, foil: bool, price_cents: Option<u32>) -> Option<OpenedCard> {
        self.ensure_loaded(booster_set).ok()?;
        let set = &self.sets[booster_set];
        let card = &set.file.cards[*set.index.get(uuid)?];
        let shown = localized(card, &self.card_lang);
        Some(OpenedCard {
            uuid: card.uuid.clone(),
            name: shown.name,
            number: card.number.clone(),
            rarity: card.rarity.clone(),
            set_code: card.set_code.clone(),
            scryfall_id: card.scryfall_id.clone(),
            image_url: shown.image_url,
            back_image_url: shown.back_image_url,
            foil,
            sheet: String::new(),
            price_cents,
            // A pull keeps the price it had when pulled, estimated or not.
            price_estimated: false,
            is_new: false,
        })
    }

    fn has_sellable_cards(&mut self) -> Result<bool, String> {
        self.ensure_prices();
        let owned = self.store.owned_cards().map_err(err)?;
        Ok(owned.iter().any(|(uuid, foil, _)| self.sale_value(uuid, *foil) > 0))
    }

    fn market_price(&self, uuid: &str, foil: bool) -> Option<u32> {
        market_price(self.prices.as_ref(), &self.economy, &self.limited, &self.print_runs, uuid, foil)
    }

    /// Whether a card's price is an estimate (it has none of its own).
    fn price_estimated(&self, uuid: &str) -> bool {
        self.prices.as_ref().and_then(|p| p.cards.get(uuid)).is_some_and(|p| p.estimated)
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
        let (obtainable, extras) = obtainable_cards(&file);
        let tokens = token_cards(&file);
        for card in &file.cards {
            if let Some(run) = self.limited.print_run_of(card) {
                self.print_runs.insert(card.uuid.clone(), run);
            }
        }
        self.sets.insert(code.to_string(), LoadedSet { file, colors, index, obtainable, extras, tokens });
        Ok(())
    }

    /// Loads the prices, and reloads them whenever the file changes (a daily update, or
    /// a manual `data-builder` run), so market moves reach the game while it is open.
    /// Prices are optional: the game stays playable without them.
    fn ensure_prices(&mut self) {
        let path = self.data_dir.join("prices.json");
        let stamp = fs::metadata(&path).and_then(|m| m.modified()).ok();
        if self.prices.is_some() && stamp == self.prices_stamp {
            return;
        }
        self.prices = read_json(&path).ok();
        self.prices_stamp = stamp;
        self.record_price_snapshot();
    }

    /// Remembers today's booster prices so the Boosterium can show how the market moves.
    /// The history is a convenience: a failure to record it never disturbs the game.
    fn record_price_snapshot(&mut self) {
        let Some(snapshot) = self.prices.as_ref().map(|p| p.updated_at.clone()) else { return };
        let Ok(sets) = self.list_sets() else { return };
        let prices: Vec<(String, String, u32)> = sets
            .iter()
            .flat_map(|set| set.boosters.iter().map(move |kind| (set.code.clone(), kind.clone())))
            .map(|(set, kind)| {
                let price = self.booster_price(&set, &kind);
                (set, kind, price)
            })
            .collect();
        let _ = self.store.record_booster_prices(&snapshot, (self.clock)().now, &prices);
    }
}

/// A card's market price. An estimated one (the card has no price of its own) is
/// multiplied for a card of a limited print run: it is based on the ordinary printings.
fn market_price(
    prices: Option<&PriceFile>,
    economy: &EconomyConfig,
    limited: &LimitedConfig,
    print_runs: &HashMap<String, u32>,
    uuid: &str,
    foil: bool,
) -> Option<u32> {
    let card = prices?.cards.get(uuid)?;
    let price = economy.market_price_cents(card, foil)?;
    Some(match print_runs.get(uuid) {
        Some(&run) if card.estimated => price.saturating_mul(limited.estimate_multiplier(run)),
        _ => price,
    })
}

/// The set's own cards given by its main boosters, then those only collector boosters
/// give (see `SetFile::is_main_booster`), both in collector number order.
fn obtainable_cards(file: &SetFile) -> (Vec<usize>, Vec<usize>) {
    let in_sheets = |main: bool| -> HashSet<&str> {
        file.boosters
            .iter()
            .filter(|(kind, _)| file.is_main_booster(kind) == main)
            .flat_map(|(_, cfg)| cfg.sheets.values())
            .flat_map(|sheet| sheet.cards.keys().map(String::as_str))
            .collect()
    };
    let (main, collector) = (in_sheets(true), in_sheets(false));
    let pick = |keep: &dyn Fn(&str) -> bool| {
        let mut indices: Vec<usize> = file
            .cards
            .iter()
            .enumerate()
            // Tokens often bear the set's own code, and are in every booster: kept apart.
            .filter(|(_, c)| c.set_code == file.code && !c.token && keep(&c.uuid))
            .map(|(i, _)| i)
            .collect();
        indices.sort_by_key(|&i| number_key(&file.cards[i].number));
        indices
    };
    (pick(&|u| main.contains(u)), pick(&|u| collector.contains(u) && !main.contains(u)))
}

/// The set's tokens (each booster holds one), in collector number order.
fn token_cards(file: &SetFile) -> Vec<usize> {
    let mut indices: Vec<usize> = file.cards.iter().enumerate().filter(|(_, c)| c.token).map(|(i, _)| i).collect();
    indices.sort_by_key(|&i| number_key(&file.cards[i].number));
    indices
}

/// Sort key for collector numbers such as "12", "12★" or "A-12": leading digits first.
fn number_key(number: &str) -> (u32, String) {
    let digits: String = number.chars().take_while(char::is_ascii_digit).collect();
    (digits.parse().unwrap_or(u32::MAX), number.to_string())
}

/// The sets the downloaded data offers, newest first, named in `lang` when translated.
/// Needs no game: the home screen and the options read it too.
pub fn read_sets(data_dir: &Path, lang: &str) -> Result<Vec<SetSummary>, String> {
    Ok(read_manifest(data_dir)?.map(|m| sets_of(m, lang)).unwrap_or_default())
}

/// Languages cards and set names can be shown in (English, then `FOREIGN_LANGUAGES`).
pub fn is_card_language(lang: &str) -> bool {
    lang == "en" || FOREIGN_LANGUAGES.iter().any(|(_, code)| *code == lang)
}

/// What the downloaded data contains, for the options screen.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataInfo {
    pub set_count: usize,
    pub booster_count: usize,
    /// Date of the card prices (Scryfall), if any data was built.
    pub prices_updated_at: Option<String>,
    /// Unix time of the last data build.
    pub built_at: Option<i64>,
}

pub fn data_info(data_dir: &Path) -> Result<DataInfo, String> {
    let manifest = read_manifest(data_dir)?;
    Ok(DataInfo {
        set_count: manifest.as_ref().map_or(0, |m| m.sets.len()),
        booster_count: manifest.as_ref().map_or(0, |m| m.sets.values().map(|s| s.boosters.len()).sum()),
        prices_updated_at: manifest.as_ref().map(|m| m.prices_updated_at.clone()),
        built_at: manifest.and_then(|m| m.generated_at.parse().ok()),
    })
}

/// `None` on first launch: the background download has not produced any data yet.
fn read_manifest(data_dir: &Path) -> Result<Option<Manifest>, String> {
    let path = data_dir.join("manifest.json");
    if !path.exists() {
        return Ok(None);
    }
    read_json(&path).map(Some)
}

fn sets_of(manifest: Manifest, lang: &str) -> Vec<SetSummary> {
    let mut sets: Vec<SetSummary> = manifest
        .sets
        .into_iter()
        .map(|(code, s)| SetSummary {
            code,
            name: s.names.get(lang).unwrap_or(&s.name).clone(),
            english_name: s.name,
            release_date: s.release_date,
            boosters: s.boosters,
            art: s.featured.map(|f| SetArt {
                image_url: art_crop_url(&f.scryfall_id),
                card_name: f.name,
                artist: f.artist,
            }),
            icon_url: s.icon_svg_uri,
        })
        .collect();
    sets.sort_by(|a, b| b.release_date.cmp(&a.release_date));
    sets
}

fn read_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<T, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("invalid {}: {e}", path.display()))
}

/// Layouts whose Scryfall printing has a back face image.
const DOUBLE_FACED: &[&str] = &["transform", "modal_dfc", "reversible_card", "double_faced_token", "art_series", "battle"];

/// How a card looks in a language: its name and images.
struct Shown {
    name: String,
    image_url: String,
    /// For double-faced cards.
    back_image_url: Option<String>,
    /// The printing shown (the translated one when it exists).
    scryfall_id: String,
    /// The language actually shown: the requested one, or "en" when not printed in it.
    lang: String,
}

/// A card's name and images in `lang`, or in English when it was not printed in that language.
/// A translated name without a translated printing on Scryfall keeps the English images.
fn localized(card: &CardEntry, lang: &str) -> Shown {
    let foreign = card.foreign.get(lang);
    let scryfall_id = foreign.and_then(|f| f.scryfall_id.clone()).unwrap_or_else(|| card.scryfall_id.clone());
    Shown {
        name: foreign.map_or(&card.name, |f| &f.name).clone(),
        image_url: image_url(&scryfall_id),
        back_image_url: DOUBLE_FACED
            .contains(&card.layout.as_str())
            .then(|| scryfall_image_face(&scryfall_id, "normal", "back")),
        scryfall_id,
        lang: if foreign.is_some() { lang.to_string() } else { "en".to_string() },
    }
}

/// Scryfall CDN image of a card (normal size). Double-faced back sides are not handled yet.
fn image_url(scryfall_id: &str) -> String {
    scryfall_image(scryfall_id, "normal")
}

/// Scryfall CDN crop of a card's art only. Must be shown with the artist's name.
fn art_crop_url(scryfall_id: &str) -> String {
    scryfall_image(scryfall_id, "art_crop")
}

fn scryfall_image(scryfall_id: &str, version: &str) -> String {
    scryfall_image_face(scryfall_id, version, "front")
}

fn scryfall_image_face(scryfall_id: &str, version: &str, face: &str) -> String {
    match scryfall_id.get(0..2) {
        Some(p) => format!("https://cards.scryfall.io/{version}/{face}/{}/{}/{scryfall_id}.jpg", &p[0..1], &p[1..2]),
        None => String::new(),
    }
}

/// Reveal order, as cards come in a real booster of each kind (MTGJSON gives none: each
/// card's place is told by its rarity and the sheet it was drawn from). The token always
/// comes last; cards of a same place keep their drawing order.
/// - Play boosters (slots #1-15 of the MTG wiki): commons, then the common slot's Special
///   Guest / The List card, uncommons, the non-foil wildcard, the rare or mythic, the land,
///   the foil wildcard.
/// - Set boosters (the wiki's "chapters"): the land first, commons and uncommons, the
///   head-turning showcase common/uncommon, the wildcards, the rare or mythic, the foil, and
///   in the epilogue The List card before the token.
/// - Collector boosters (nearly all foil): by rarity only, commons first, rares last.
/// - Draft boosters and the oldest ones: commons, uncommons, the rare or mythic, the foil,
///   the basic land.
fn reveal_rank(kind: &str, card: &OpenedCard) -> u8 {
    if card.rarity == "token" {
        return 99;
    }
    let sheet = card.sheet.to_lowercase();
    let land = (sheet.contains("land") || sheet.contains("basic")) && !sheet.contains("orbasic");
    let by_rarity = |common: u8, uncommon: u8, rare: u8| match card.rarity.as_str() {
        "common" => common,
        "uncommon" => uncommon,
        _ => rare,
    };
    match kind {
        COLLECTOR_KIND => by_rarity(0, 1, 2),
        "set" => {
            let rare = !matches!(card.rarity.as_str(), "common" | "uncommon");
            if land {
                0
            } else if sheet.contains("thelist") {
                6
            } else if card.foil {
                5
            } else if sheet.contains("wildcard") {
                3
            } else if rare {
                4
            } else if sheet.contains("showcase") {
                2
            } else {
                1
            }
        }
        "play" => {
            if land {
                5
            } else if card.foil {
                6
            } else if sheet.contains("wildcard") {
                3
            } else if ["specialguest", "thelist", "sourcematerial"].iter().any(|s| sheet.contains(s)) {
                1
            } else {
                by_rarity(0, 2, 4)
            }
        }
        _ => {
            if card.foil {
                3
            } else if land {
                4
            } else {
                by_rarity(0, 1, 2)
            }
        }
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
          "rarity": "rare", "finishes": ["nonfoil"], "layout": "normal" },
        { "uuid": "t1", "setCode": "TTST", "scryfallId": "777777", "name": "Soldier", "number": "1",
          "rarity": "token", "finishes": ["nonfoil"], "layout": "token", "token": true },
        { "uuid": "t2", "setCode": "TST", "scryfallId": "888888", "name": "Spirit", "number": "2",
          "rarity": "token", "finishes": ["nonfoil"], "layout": "token", "token": true }
      ],
      "boosters": { "play": {
        "boosters": [ { "contents": { "mythic": 1, "common": 2 }, "weight": 1 } ],
        "boostersTotalWeight": 1,
        "sheets": {
          "common": { "allowDuplicates": true, "cards": { "c1": 1 }, "foil": false, "totalWeight": 1 },
          "mythic": { "cards": { "m1": 1 }, "foil": false, "totalWeight": 1 },
          "unsealedToken": { "cards": { "t2": 1 }, "foil": false, "totalWeight": 1 }
        },
        "sourceSetCodes": ["TST"] } } }"#;

    const MANIFEST: &str = r#"{ "generatedAt": "0", "pricesUpdatedAt": "d", "sets": {
      "TST": { "name": "Test Set", "releaseDate": "2026-01-01", "sourceVersion": "v1", "boosters": ["play"] } } }"#;

    /// Common is worth $1.00, the mythic $10.00.
    const PRICES: &str = r#"{ "updatedAt": "d", "cards": { "c1": { "usd": 100 }, "m1": { "usd": 1000 } } }"#;
    const NO_PRICES: &str = r#"{ "updatedAt": "d", "cards": {} }"#;

    /// The shipped economy, with a $4.00 fallback booster price so that the tests that do
    /// not care about market pricing keep simple numbers.
    fn test_economy() -> EconomyConfig {
        let mut economy = EconomyConfig::from_json(ECONOMY).unwrap();
        economy.booster_prices.default_cents = 400;
        economy
    }

    fn data_dir(name: &str, prices: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("unsealed-test-{}-{name}", std::process::id()));
        fs::create_dir_all(dir.join("sets")).unwrap();
        fs::write(dir.join("sets/TST.json"), SET).unwrap();
        fs::write(dir.join("manifest.json"), MANIFEST).unwrap();
        fs::write(dir.join("prices.json"), prices).unwrap();
        dir
    }

    /// An engine without achievements: most tests must not be disturbed by rewards.
    fn no_achievements() -> Engine {
        Engine::from_json(r#"{ "achievements": [] }"#).unwrap()
    }

    /// An engine with the given achievements (a JSON list) and an `opened` metric.
    fn achievements(list: &str) -> Engine {
        Engine::from_json(&format!(
            r#"{{ "metrics": {{ "opened": {{ "event": "BoosterOpened", "op": "sum" }} }}, "achievements": [{list}] }}"#
        ))
        .unwrap()
    }

    fn game_with(name: &str, prices: &str) -> (Game, Arc<AtomicU32>) {
        game_with_engine(name, prices, no_achievements())
    }

    /// A game plus a handle to change the current day (October, 2026).
    fn game_with_engine(name: &str, prices: &str, engine: Engine) -> (Game, Arc<AtomicU32>) {
        let day = Arc::new(AtomicU32::new(13));
        let handle = day.clone();
        let clock: ClockFn = Box::new(move || Clock {
            now: 0,
            day: format!("2026-10-{:02}", handle.load(Ordering::SeqCst)),
        });
        let game = Game::with_parts(
            data_dir(name, prices),
            Store::open_in_memory().unwrap(),
            test_economy(),
            engine,
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

    #[test]
    fn cards_come_in_the_order_of_a_real_booster_of_their_kind() {
        let card = |uuid: &str, rarity: &str, foil: bool, sheet: &str| OpenedCard {
            uuid: uuid.into(),
            name: String::new(),
            number: String::new(),
            rarity: rarity.into(),
            set_code: String::new(),
            scryfall_id: String::new(),
            foil,
            sheet: sheet.into(),
            image_url: String::new(),
            back_image_url: None,
            price_cents: None,
            price_estimated: false,
            is_new: false,
        };
        let order = |kind: &str, mut cards: Vec<OpenedCard>| {
            cards.sort_by_key(|c| reveal_rank(kind, c));
            cards.into_iter().map(|c| c.uuid).collect::<Vec<_>>()
        };

        // Draft: commons, uncommons, rare, foil, basic land, token.
        let draft = vec![
            card("token", "token", true, "unsealedTokenFoil"),
            card("land", "common", false, "basic"),
            card("foil", "common", true, "foil"),
            card("mythic", "mythic", false, "rareMythic"),
            card("u1", "uncommon", false, "uncommon"),
            card("c1", "common", false, "common"),
            card("c2", "common", false, "common"),
        ];
        assert_eq!(order("draft", draft), ["c1", "c2", "u1", "mythic", "foil", "land", "token"]);

        // Play: commons, Special Guest, uncommons, wildcard, rare, land, foil wildcard, token.
        let play = vec![
            card("token", "token", false, "unsealedToken"),
            card("foil", "uncommon", true, "foil"),
            card("land", "common", true, "foilLand"),
            card("rare", "rare", false, "rareMythic"),
            card("wild", "rare", false, "wildcard"),
            card("u1", "uncommon", false, "uncommon"),
            card("guest", "mythic", false, "specialGuest"),
            card("c1", "common", false, "common"),
        ];
        assert_eq!(order("play", play), ["c1", "guest", "u1", "wild", "rare", "land", "foil", "token"]);

        // Set: land, commons/uncommons, showcase, wildcard, rare, foil, The List, token.
        let set = vec![
            card("token", "token", false, "unsealedToken"),
            card("list", "rare", false, "theList"),
            card("foil", "common", true, "foilWithShowcase"),
            card("rare", "mythic", false, "rareMythic"),
            card("wild", "uncommon", false, "wildcard"),
            card("showcase", "uncommon", false, "commonUncommonShowcase"),
            card("u1", "uncommon", false, "uncommon"),
            card("c1", "common", false, "common"),
            card("land", "common", true, "foilBasic"),
        ];
        assert_eq!(order("set", set), ["land", "u1", "c1", "showcase", "wild", "rare", "foil", "list", "token"]);

        // Collector: by rarity, foil or not.
        let collector = vec![
            card("token", "token", true, "unsealedTokenFoil"),
            card("mythic", "mythic", true, "foilRareMythic"),
            card("showcase", "uncommon", false, "uncommonShowcase"),
            card("fc", "common", true, "foilCommon"),
            card("rare", "rare", false, "extendedRareMythic"),
            card("fu", "uncommon", true, "foilUncommon"),
        ];
        assert_eq!(order("collector", collector), ["fc", "showcase", "fu", "mythic", "rare", "token"]);
    }

    #[test]
    fn openings_watched_and_skipped_are_counted_apart() {
        let rushed = r#"{ "id": "rushed", "name": "Rushed", "description": "Skip an opening.",
            "rarity": "common", "emblem": "bolt", "palette": "ember",
            "condition": { "type": "event", "event": "BoosterOpened",
              "where": [{ "field": "animated", "op": "eq", "value": false }] } }"#;
        let (mut g, _) = game_with_engine("rushed", PRICES, achievements(rushed));
        g.buy_boosters("TST", "play", 3).unwrap();
        // Watched: nothing.
        g.open_booster("TST", "play").unwrap();
        g.reveal_cards(usize::MAX);
        assert!(g.take_unlocks("en").is_empty());
        // Skipped (quick opening): unlocked.
        g.open_booster_as("TST", "play", false).unwrap();
        g.reveal_cards(usize::MAX);
        assert_eq!(g.take_unlocks("en").len(), 1);
    }

    #[test]
    fn a_set_shows_its_tokens_filed_under_their_own_set() {
        let mut g = game_with_booster("tokens");
        // The booster has no token slot here: the token is acquired as if pulled.
        g.store
            .open_sealed(
                "TST",
                "play",
                &[OpeningCard { uuid: "t1", set_code: "TTST", foil: false, price_cents: None, limited: false }],
                0,
            )
            .unwrap();
        let view = g.collection("TST").unwrap();
        let tokens: Vec<(&str, u32)> = view.tokens.iter().map(|c| (c.uuid.as_str(), c.quantity)).collect();
        assert_eq!(tokens, [("t1", 1), ("t2", 0)]);
        assert_eq!(view.tokens_owned, 1);
        // Tokens do not count toward completion, even one bearing the set's own code and
        // found in its boosters' sheets.
        assert!(view.cards.iter().all(|c| !c.uuid.starts_with('t')));
        assert_eq!(view.total_count, 2);
    }

    #[test]
    fn a_card_of_a_limited_print_run_is_never_pulled_beyond_it() {
        let (mut g, _) = game_with("limited", PRICES);
        // The mythic (TST #2) exists in 2 copies; every TST booster holds it.
        g.set_limited(LimitedConfig::from_json(r#"{ "cards": [ { "set": "TST", "number": "2", "printRun": 2 } ] }"#).unwrap());
        g.buy_boosters("TST", "play", 3).unwrap();
        for _ in 0..2 {
            assert!(g.open_booster("TST", "play").unwrap().iter().any(|c| c.uuid == "m1"));
        }
        // Selling them gives nothing back: both copies were pulled.
        g.sell_cards(&[SaleRequest { uuid: "m1".into(), foil: false, quantity: 2 }]).unwrap();
        // No booster can hold it any more (here, none at all): the third stays sealed.
        assert!(g.open_booster("TST", "play").is_err());
        assert_eq!(g.wallet().unwrap().sealed[0].quantity, 1);
    }

    #[test]
    fn an_estimated_price_is_multiplied_for_a_limited_card_only() {
        // The mythic has no price of its own: $10.00 estimated from its other printings.
        let prices = r#"{ "updatedAt": "d", "cards": { "c1": { "usd": 100, "estimated": true },
                                                      "m1": { "usd": 1000, "estimated": true } } }"#;
        let (mut g, _) = game_with("estimated", prices);
        g.set_limited(
            LimitedConfig::from_json(
                r#"{ "cards": [ { "set": "TST", "number": "2", "printRun": 1 } ],
                     "estimateMultipliers": [ { "printRunUpTo": 1, "multiplier": 1000 } ] }"#,
            )
            .unwrap(),
        );
        g.buy_boosters("TST", "play", 1).unwrap();
        let cards = g.open_booster("TST", "play").unwrap();
        let price = |uuid: &str| cards.iter().find(|c| c.uuid == uuid).map(|c| (c.price_cents, c.price_estimated));
        assert_eq!(price("m1"), Some((Some(1_000_000), true)));
        assert_eq!(price("c1"), Some((Some(100), true)));
    }

    fn balance(g: &mut Game) -> i64 {
        g.wallet().unwrap().balance_cents
    }

    #[test]
    fn lists_sets_and_shop_prices() {
        let mut g = game("list");
        let sets = g.list_sets().unwrap();
        assert_eq!((sets.len(), sets[0].code.as_str(), sets[0].boosters.as_slice()), (1, "TST", ["play".to_string()].as_slice()));
        assert_eq!(
            g.shop().unwrap(),
            [ShopItem {
                set_code: "TST".into(),
                set_name: "Test Set".into(),
                kind: "play".into(),
                price_cents: 400,
                sell_back_cents: 320,
                history: vec![400],
            }]
        );
    }

    #[test]
    fn a_new_player_has_the_starting_capital_and_no_help() {
        let mut g = game("wallet");
        let w = g.wallet().unwrap();
        assert_eq!((w.balance_cents, w.currency.seals_per_cent), (10_000, 1.0));
        assert!(w.sealed.is_empty());
        assert!(!w.charity.available);
    }

    #[test]
    fn buying_stocks_boosters_and_opening_consumes_them() {
        let mut g = game("buy");
        g.buy_boosters("TST", "play", 2).unwrap();
        let w = g.wallet().unwrap();
        assert_eq!(w.balance_cents, 9_200);
        assert_eq!(
            w.sealed,
            [SealedStack {
                set_code: "TST".into(),
                set_name: "Test Set".into(),
                kind: "play".into(),
                quantity: 2,
                cost_cents: 800,
                price_cents: 400,
                sell_value_cents: 320,
                history: vec![400],
            }]
        );

        g.open_booster("TST", "play").unwrap();
        let stack = &g.wallet().unwrap().sealed[0];
        assert_eq!((stack.quantity, stack.cost_cents), (1, 400));
    }

    /// Prices where a TST booster is worth $10.00 on the market.
    fn market_prices(usd: u32) -> String {
        format!(
            r#"{{ "updatedAt": "d{usd}", "cards": {{}}, "boosters": {{ "TST/play": {{ "usd": {usd}, "eur": 0 }} }} }}"#
        )
    }

    #[test]
    fn undo_offers_follow_the_penalty_scale_over_time() {
        use std::sync::atomic::AtomicI64;
        let now = Arc::new(AtomicI64::new(1_000));
        let clock_now = now.clone();
        let mut g = Game::with_parts(
            data_dir("undo", PRICES),
            Store::open_in_memory().unwrap(),
            test_economy(),
            no_achievements(),
            ChaCha8Rng::seed_from_u64(1),
            Box::new(move || Clock { now: clock_now.load(Ordering::SeqCst), day: "2026-10-13".into() }),
        )
        .unwrap();
        g.buy_boosters("TST", "play", 2).unwrap();
        g.open_booster("TST", "play").unwrap();
        // The $10.00 mythic sells for 800.
        g.sell_cards(&[SaleRequest { uuid: "m1".into(), foil: false, quantity: 1 }]).unwrap();

        let offer = |g: &mut Game, i: usize| g.history(10, 0).unwrap().entries[i].undo.as_ref().map(|o| (o.amount_cents, o.penalty_percent));
        assert_eq!(offer(&mut g, 0), Some((840, 5)));
        // The first purchase cannot be undone: one of its boosters was opened.
        assert_eq!(offer(&mut g, 1), None);
        g.buy_boosters("TST", "play", 1).unwrap();
        assert_eq!(offer(&mut g, 0), Some((380, 5)), "a fresh purchase can be undone");

        now.fetch_add(2 * 24 * 3600, Ordering::SeqCst);
        assert_eq!(offer(&mut g, 1), Some((1_200, 50)), "the sale, two days later");
        assert_eq!(offer(&mut g, 0), None, "too late to undo the purchase");

        let sale = g.history(10, 0).unwrap().entries[1].transaction_id.unwrap();
        let balance = g.wallet().unwrap().balance_cents;
        assert_eq!(g.undo_transaction(sale).unwrap(), -1_200);
        assert_eq!(g.wallet().unwrap().balance_cents, balance - 1_200);
        assert_eq!(g.collection("TST").unwrap().cards[0].quantity, 1, "the mythic is back");
        assert!(g.undo_transaction(sale).is_err());
    }

    /// Diagnostic on the real data of `dist/` (built by `data-builder`).
    #[test]
    #[ignore = "needs the data built in dist/"]
    fn real_data_shows_german_cards() {
        let dist = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist");
        let mut g = Game::with_parts(
            dist,
            Store::open_in_memory().unwrap(),
            test_economy(),
            no_achievements(),
            ChaCha8Rng::seed_from_u64(1),
            Box::new(|| Clock { now: 0, day: "2026-10-13".into() }),
        )
        .unwrap();
        g.set_card_language("de");
        let view = g.collection("BLB").unwrap();
        let german = view.cards.iter().filter(|c| c.image_url.contains("/front/")).take(3);
        for c in german {
            println!("{} -> {}", c.name, c.image_url);
        }
        assert!(view.cards.iter().any(|c| c.name == "Bannendes Licht"), "set name: {}", view.set_name);
    }

    #[test]
    fn card_details_combine_local_data_and_every_printing() {
        let mut g = game_with_booster("details");
        g.open_booster("TST", "play").unwrap();

        let (mut view, query) = g.card_details_local("TST", "m1").unwrap();
        assert_eq!((view.name.as_str(), view.owned, view.price_cents), ("Mythic", 1, Some(1_000)));
        assert!(!view.online && view.back_image_url.is_none());
        assert_eq!((query.english_id.as_str(), query.localized_id), ("123456", None));

        let printing = |id: &str, set: &str, usd: &str| -> unsealed_core::scryfall::Card {
            serde_json::from_str(&format!(
                r#"{{ "id": "{id}", "oracle_id": "o1", "name": "Mythic", "set": "{set}", "set_name": "Set {set}",
                     "collector_number": "2", "rarity": "mythic", "mana_cost": "{{2}}{{G}}", "type_line": "Creature",
                     "oracle_text": "Trample", "power": "3", "toughness": "4", "prices": {{ "usd": "{usd}" }} }}"#
            ))
            .unwrap()
        };
        let card = printing("123456", "tst", "10.00");
        let printings = [printing("old1", "old", "2.50"), card.clone()];
        g.card_details_complete(&mut view, &card, None, &printings);

        assert!(view.online);
        assert_eq!(view.faces[0].stats.as_deref(), Some("3/4"));
        assert_eq!(view.faces[0].mana_cost.as_deref(), Some("{2}{G}"));
        let summary: Vec<_> = view
            .printings
            .iter()
            .map(|p| (p.set_code.as_str(), p.in_game, p.owned, p.current, p.price_cents))
            .collect();
        assert_eq!(summary, [("OLD", false, 0, false, Some(250)), ("TST", true, 1, true, Some(1_000))]);
    }

    #[test]
    fn cards_and_sets_follow_the_card_language_with_english_as_fallback() {
        let mut g = game("card-lang");
        // The common was printed in French (with its own image), the mythic was not.
        let set = SET
            .replace(
                r#""name": "Common", "number": "10","#,
                r#""name": "Common", "number": "10", "foreign": { "fr": { "name": "Commune", "scryfallId": "fr0001" } },"#,
            )
            .replace(r#""name": "Test Set","#, r#""name": "Test Set", "names": { "fr": "Jeu de test" },"#);
        fs::write(g.data_dir.join("sets/TST.json"), set).unwrap();
        fs::write(
            g.data_dir.join("manifest.json"),
            MANIFEST.replace(r#""name": "Test Set","#, r#""name": "Test Set", "names": { "fr": "Jeu de test" },"#),
        )
        .unwrap();

        g.set_card_language("fr");
        let view = g.collection("TST").unwrap();
        assert_eq!(view.set_name, "Jeu de test");
        let names: Vec<&str> = view.cards.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Mythic", "Commune"]);
        assert!(view.cards[1].image_url.contains("fr0001"));
        let sets = g.list_sets().unwrap();
        assert_eq!((sets[0].name.as_str(), sets[0].english_name.as_str()), ("Jeu de test", "Test Set"));

        // An unknown language means English.
        g.set_card_language("xx");
        assert_eq!(g.collection("TST").unwrap().cards[1].name, "Common");
    }

    #[test]
    fn collector_exclusive_cards_are_extras_outside_completion() {
        let mut g = game("collector");
        // The test set plus a collector booster giving the card no play booster gives.
        let set = SET.replace(
            r#""sourceSetCodes": ["TST"] } } }"#,
            r#""sourceSetCodes": ["TST"] },
              "collector": {
                "boosters": [ { "contents": { "extra": 1 }, "weight": 1 } ],
                "boostersTotalWeight": 1,
                "sheets": { "extra": { "cards": { "x1": 1 }, "foil": true, "totalWeight": 1 } },
                "sourceSetCodes": ["TST"] } } }"#,
        );
        fs::write(g.data_dir.join("sets/TST.json"), set).unwrap();

        let view = g.collection("TST").unwrap();
        assert_eq!(view.cards.iter().map(|c| c.uuid.as_str()).collect::<Vec<_>>(), ["m1", "c1"]);
        assert_eq!(view.extras.iter().map(|c| c.uuid.as_str()).collect::<Vec<_>>(), ["x1"]);

        g.buy_boosters("TST", "collector", 1).unwrap();
        let cards = g.open_booster("TST", "collector").unwrap();
        assert_eq!((cards[0].uuid.as_str(), cards[0].foil), ("x1", true));
        let view = g.collection("TST").unwrap();
        assert_eq!((view.owned_count, view.total_count, view.extras_owned), (0, 2, 1));
        assert_eq!(view.extras[0].foil_quantity, 1);
    }

    #[test]
    fn stats_sum_up_openings_money_and_best_pulls() {
        let mut g = game("stats");
        let empty = g.stats().unwrap();
        assert_eq!((empty.boosters_opened, empty.best_pulls.len()), (0, 0));
        assert!(empty.best_booster.is_none());

        g.buy_boosters("TST", "play", 2).unwrap();
        g.open_booster("TST", "play").unwrap();
        g.sell_duplicates("TST").unwrap();

        let s = g.stats().unwrap();
        // One booster: the $10.00 mythic and two $1.00 commons.
        assert_eq!((s.boosters_opened, s.cards_pulled, s.foils_pulled, s.value_pulled_cents), (1, 3, 0, 1_200));
        assert_eq!((s.spent_on_boosters_cents, s.earned_from_cards_cents), (800, 80));
        assert_eq!((s.distinct_cards, s.collection_value_cents, s.sealed_value_cents), (2, 1_100, 320));
        assert_eq!(s.balance_cents, 10_000 - 800 + 80);

        let best = s.best_booster.unwrap();
        assert_eq!((best.set_name.as_str(), best.value_cents), ("Test Set", 1_200));
        let best_card = best.best_card.unwrap();
        assert_eq!((best_card.name.as_str(), best_card.price_cents), ("Mythic", Some(1_000)));
        assert_eq!(s.recent_openings.len(), 1);
        assert_eq!(
            s.best_pulls.iter().map(|p| p.card.name.as_str()).collect::<Vec<_>>(),
            ["Mythic", "Common", "Common"]
        );
        assert_eq!(
            s.boosters,
            [BoosterStatsView {
                set_code: "TST".into(),
                set_name: "Test Set".into(),
                kind: "play".into(),
                boosters: 1,
                value_cents: 1_200,
                average_cents: 1_200,
                best_value_cents: 1_200,
            }]
        );
    }

    #[test]
    fn shop_and_stacks_expose_the_price_history() {
        let (mut g, _) = game_with("history", &market_prices(1_000));
        assert_eq!(g.shop().unwrap()[0].history, [1_000]);

        std::thread::sleep(std::time::Duration::from_millis(30));
        fs::write(g.data_dir.join("prices.json"), market_prices(1_500)).unwrap();
        assert_eq!(g.shop().unwrap()[0].history, [1_000, 1_500]);
        g.buy_boosters("TST", "play", 1).unwrap();
        assert_eq!(g.wallet().unwrap().sealed[0].history, [1_000, 1_500]);
    }

    #[test]
    fn booster_price_follows_the_market_and_can_be_speculated_on() {
        let (mut g, _) = game_with("market", &market_prices(1_000));
        let item = g.shop().unwrap().remove(0);
        assert_eq!((item.price_cents, item.sell_back_cents), (1_000, 800));

        // Buy 2 at $10.00 and sell them back right away: the margin makes a loss.
        g.buy_boosters("TST", "play", 2).unwrap();
        assert_eq!(balance(&mut g), 8_000);
        let stack = g.wallet().unwrap().sealed.remove(0);
        assert_eq!((stack.cost_cents, stack.sell_value_cents), (2_000, 800));

        // The market goes up 50%: the same boosters now sell above their cost.
        std::thread::sleep(std::time::Duration::from_millis(30));
        fs::write(g.data_dir.join("prices.json"), market_prices(1_500)).unwrap();
        let stack = g.wallet().unwrap().sealed.remove(0);
        assert_eq!((stack.price_cents, stack.sell_value_cents), (1_500, 1_200));

        let sale = g.sell_boosters("TST", "play", 2).unwrap();
        assert_eq!(sale, BoosterSaleResult { quantity: 2, total_cents: 2_400, profit_cents: 400 });
        assert_eq!(balance(&mut g), 8_000 + 2_400);
        assert!(g.wallet().unwrap().sealed.is_empty());
    }

    #[test]
    fn cannot_sell_boosters_that_are_not_owned() {
        let mut g = game_with_booster("sell-none");
        assert!(g.sell_boosters("TST", "play", 2).is_err());
        assert!(g.sell_boosters("TST", "play", 0).is_err());
        assert!(g.sell_boosters("TST", "collector", 1).is_err());
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
    fn opens_many_boosters_and_stops_when_the_stock_runs_out() {
        let mut g = game("batch");
        g.buy_boosters("TST", "play", 3).unwrap();
        let opened = g.open_boosters("TST", "play", 10).unwrap();
        assert_eq!(opened.len(), 3);
        // Discoveries are only flagged in the first booster.
        assert!(opened[0].iter().any(|c| c.is_new));
        assert!(opened[1..].iter().all(|b| b.iter().all(|c| !c.is_new)));
        assert!(g.open_boosters("TST", "play", 5).is_err());
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

    /// The achievements unlocked so far, once every card opened has been revealed.
    fn unlocked_ids(g: &mut Game) -> Vec<String> {
        g.reveal_cards(usize::MAX);
        g.take_unlocks("en").into_iter().map(|a| a.id).collect()
    }

    const OPEN_ONCE: &str = r#"{ "id": "open-1", "name": "First Crack", "description": "Open a booster.",
        "rarity": "common", "emblem": "pack", "palette": "ember",
        "condition": { "type": "metric", "metric": "opened", "atLeast": 1 } }"#;

    #[test]
    fn completing_every_set_of_a_year_unlocks_and_foil_needs_foil_cards() {
        let defs = r#"
            { "id": "year", "name": "Year", "description": "d", "rarity": "mythic", "emblem": "hourglass", "palette": "gold",
              "condition": { "type": "event", "event": "YearProgress", "where": [
                { "field": "complete", "op": "eq", "value": true }, { "field": "year", "op": "eq", "value": "2026" } ] } },
            { "id": "foil", "name": "Foil", "description": "d", "rarity": "mythic", "emblem": "gem", "palette": "ice",
              "condition": { "type": "event", "event": "SetProgress", "where": [
                { "field": "foilCompletionPermille", "op": "gte", "value": 1000 } ] } }"#;
        let (mut g, _) = game_with_engine("ach-year", PRICES, achievements(defs));
        g.buy_boosters("TST", "play", 1).unwrap();
        // The only set of 2026 is completed by its first booster (the mythic and the common).
        g.open_booster("TST", "play").unwrap();
        // No card of the test set exists in foil: the foil completion cannot be reached.
        assert_eq!(unlocked_ids(&mut g), ["year"]);
    }

    #[test]
    fn opening_unlocks_an_achievement_and_notifies_once() {
        let (mut g, _) = game_with_engine("ach-open", PRICES, achievements(OPEN_ONCE));
        g.buy_boosters("TST", "play", 1).unwrap();
        assert!(unlocked_ids(&mut g).is_empty());

        g.open_booster("TST", "play").unwrap();
        assert_eq!(unlocked_ids(&mut g), ["open-1"]);
        assert!(g.take_unlocks("en").is_empty()); // notified once

        let view = g.achievements("en").unwrap();
        assert_eq!((view.total, view.unlocked_count, view.secret_remaining), (1, 1, 0));
        assert!(view.items[0].unlocked && view.items[0].unlocked_at.is_some());
    }

    #[test]
    fn achievements_unlock_as_the_cards_are_revealed() {
        // One per card (the mythic, revealed last), one for the whole booster.
        let defs = r#"
            { "id": "mythic", "name": "M", "description": "d", "rarity": "rare", "emblem": "crown", "palette": "ember",
              "condition": { "type": "event", "event": "CardAcquired", "where": [{ "field": "rarity", "op": "eq", "value": "mythic" }] } },
            { "id": "open-1", "name": "O", "description": "d", "rarity": "common", "emblem": "pack", "palette": "ember",
              "condition": { "type": "metric", "metric": "opened", "atLeast": 1 } }"#;
        let (mut g, _) = game_with_engine("ach-reveal", PRICES, achievements(defs));
        g.buy_boosters("TST", "play", 2).unwrap();
        let cards = g.open_booster("TST", "play").unwrap();
        assert_eq!(cards.iter().map(|c| c.rarity.as_str()).collect::<Vec<_>>(), ["common", "common", "mythic"]);
        assert!(g.take_unlocks("en").is_empty(), "nothing before a card is seen");

        g.reveal_cards(2);
        assert!(g.take_unlocks("en").is_empty(), "two commons: nothing yet");
        g.reveal_cards(1);
        assert!(g.take_unlocks("en").is_empty(), "revealing fewer than already seen does nothing");
        g.reveal_cards(3);
        let ids: Vec<String> = g.take_unlocks("en").into_iter().map(|a| a.id).collect();
        assert_eq!(ids, ["mythic", "open-1"], "the mythic, then the whole booster");

        // Moving on emits what an opening still holds back.
        g.open_booster("TST", "play").unwrap();
        g.buy_boosters("TST", "play", 1).unwrap();
        assert_eq!(g.achievements("en").unwrap().unlocked_count, 2);
        assert!(g.pending_reveal.is_none());
    }

    #[test]
    fn achievements_are_shown_in_the_requested_language_with_english_as_fallback() {
        let (mut g, _) = game_with_engine("ach-lang", PRICES, achievements(OPEN_ONCE));
        let mut texts = AchievementTexts::default();
        texts
            .add_language("fr", r#"{ "open-1": { "name": "Première ouverture", "description": "Ouvrir un booster." } }"#)
            .unwrap();
        g.set_achievement_texts(texts);
        g.buy_boosters("TST", "play", 1).unwrap();
        g.open_booster("TST", "play").unwrap();
        g.reveal_cards(usize::MAX);

        let unlocked = g.take_unlocks("fr");
        assert_eq!((unlocked[0].name.as_str(), unlocked[0].description.as_deref()), ("Première ouverture", Some("Ouvrir un booster.")));
        assert_eq!(g.achievements("de").unwrap().items[0].name, "First Crack");
    }

    #[test]
    fn rewards_pay_cash_and_free_boosters_once() {
        let reward = OPEN_ONCE.replace(
            r#""condition""#,
            r#""reward": { "cashCents": 500, "sealed": { "setCode": "latest", "kind": "play", "quantity": 2 } },
        "condition""#,
        );
        let (mut g, _) = game_with_engine("ach-reward", PRICES, achievements(&reward));
        g.buy_boosters("TST", "play", 2).unwrap();
        g.open_booster("TST", "play").unwrap();
        g.reveal_cards(usize::MAX);

        let w = g.wallet().unwrap();
        assert_eq!(w.balance_cents, 10_000 - 800 + 500);
        // The unopened bought booster (cost 400) plus the 2 free ones: two stacks merged.
        assert_eq!(w.sealed.len(), 1);
        assert_eq!((w.sealed[0].quantity, w.sealed[0].cost_cents), (3, 400));
        let unlocked = g.take_unlocks("en");
        assert_eq!(
            unlocked[0].reward,
            Some(achievements::RewardView {
                cash_cents: 500,
                sealed: Some(achievements::SealedRewardView {
                    set_name: Some("Test Set".into()),
                    kind: "play".into(),
                    quantity: 2
                }),
            })
        );

        // Opening again never pays the reward a second time.
        g.open_booster("TST", "play").unwrap();
        assert_eq!(g.wallet().unwrap().balance_cents, 10_000 - 800 + 500);
    }

    #[test]
    fn a_secret_achievement_stays_hidden_until_it_is_unlocked() {
        let wrong_card = r#"{ "id": "wrong-card", "name": "Wrong Card", "secret": true,
            "description": "Sell the most valuable card you own.",
            "rarity": "uncommon", "emblem": "eye", "palette": "rose",
            "condition": { "type": "event", "event": "CardSold", "where": [
              { "field": "wasMostValuableCard", "op": "eq", "value": true },
              { "field": "unitCents", "op": "gte", "value": 400 } ] } }"#;
        let (mut g, _) = game_with_engine("ach-secret", PRICES, achievements(wrong_card));
        g.buy_boosters("TST", "play", 1).unwrap();
        g.open_booster("TST", "play").unwrap();

        let hidden = g.achievements("en").unwrap();
        assert_eq!(hidden.secret_remaining, 1);
        let item = &hidden.items[0];
        assert_eq!((item.name.as_str(), item.emblem.as_str()), ("", "question"));
        assert!(item.description.is_none() && item.progress.is_none() && item.reward.is_none());
        assert_eq!(item.rarity, "uncommon"); // the rarity is the only hint

        // Selling a cheap card does not trigger it: it is not the most valuable one.
        g.sell_cards(&[SaleRequest { uuid: "c1".into(), foil: false, quantity: 1 }]).unwrap();
        assert!(unlocked_ids(&mut g).is_empty());
        // Selling the $10 mythic does.
        g.sell_cards(&[SaleRequest { uuid: "m1".into(), foil: false, quantity: 1 }]).unwrap();
        assert_eq!(unlocked_ids(&mut g), ["wrong-card"]);

        let shown = g.achievements("en").unwrap();
        assert_eq!((shown.secret_remaining, shown.items[0].name.as_str()), (0, "Wrong Card"));
        assert!(shown.items[0].description.is_some());
    }

    #[test]
    fn set_progress_and_balance_events_reach_the_engine() {
        let list = r#"
          { "id": "set-complete", "name": "Complete Set", "description": "x", "rarity": "rare",
            "emblem": "trophy", "palette": "gold", "condition": { "type": "event", "event": "SetProgress",
            "where": [{ "field": "completionPermille", "op": "gte", "value": 1000 }] } },
          { "id": "mythics", "name": "Mythics", "description": "x", "rarity": "rare",
            "emblem": "crown", "palette": "ember", "condition": { "type": "event", "event": "SetProgress",
            "where": [{ "field": "allMythicsOwned", "op": "eq", "value": true }] } },
          { "id": "rock-bottom", "name": "Rock Bottom", "description": "x", "rarity": "common", "secret": true,
            "emblem": "skull", "palette": "onyx", "condition": { "type": "event", "event": "BalanceChanged",
            "where": [{ "field": "balanceCents", "op": "lte", "value": 0 }] } }"#;
        let (mut g, _) = game_with_engine("ach-events", PRICES, achievements(list));
        g.buy_boosters("TST", "play", 25).unwrap(); // spends the whole $100
        assert_eq!(unlocked_ids(&mut g), ["rock-bottom"]);

        // The fixture set has one mythic and one common: one opening completes the set.
        g.open_booster("TST", "play").unwrap();
        let mut ids = unlocked_ids(&mut g);
        ids.sort();
        assert_eq!(ids, ["mythics", "set-complete"]);
    }

    #[test]
    fn selling_boosters_for_a_profit_unlocks_speculation_achievements() {
        let speculator = r#"{ "id": "speculator", "name": "Speculator", "description": "x", "rarity": "uncommon",
            "emblem": "chart", "palette": "forest", "condition": { "type": "event", "event": "BoosterSold",
            "where": [{ "field": "profitCents", "op": "gte", "value": 100 }] } }"#;
        let (mut g, _) = game_with_engine("ach-spec", &market_prices(1_000), achievements(speculator));
        g.buy_boosters("TST", "play", 1).unwrap(); // bought at $10.00

        // Sold right away at 80%: a loss, no achievement.
        std::thread::sleep(std::time::Duration::from_millis(30));
        g.buy_boosters("TST", "play", 1).unwrap();
        g.sell_boosters("TST", "play", 1).unwrap();
        assert!(unlocked_ids(&mut g).is_empty());

        // The market rises 50%: the remaining booster now sells above its cost.
        fs::write(g.data_dir.join("prices.json"), market_prices(1_500)).unwrap();
        g.sell_boosters("TST", "play", 1).unwrap();
        assert_eq!(unlocked_ids(&mut g), ["speculator"]);
    }

    #[test]
    fn the_shipped_achievements_work_end_to_end_with_the_shipped_economy() {
        let engine = Engine::from_json(include_str!("../../../config/achievements.json")).unwrap();
        let (mut g, _) = game_with_engine("ach-shipped", PRICES, engine);
        g.buy_boosters("TST", "play", 3).unwrap();
        g.open_booster("TST", "play").unwrap();
        // First booster: "First Crack"; the fixture mythic is a $10 card.
        assert!(unlocked_ids(&mut g).contains(&"open-1".to_string()));
        let view = g.achievements("en").unwrap();
        assert!(view.total >= 40 && view.unlocked_count >= 1);
        assert!(view.secret_remaining >= 5);
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
            Engine::from_json(include_str!("../../../config/achievements.json")).unwrap(),
            ChaCha8Rng::seed_from_u64(7),
            Box::new(system_clock),
        )
        .unwrap();
        let sets = g.list_sets().unwrap();
        assert!(!sets.is_empty());
        for s in &sets {
            g.buy_boosters(&s.code, "play", 3).unwrap();
            let mut new_cards = 0;
            for _ in 0..3 {
                let cards = g.open_booster(&s.code, "play").unwrap();
                assert!(cards.len() >= 12);
                assert!(cards.iter().all(|c| c.image_url.starts_with("https://cards.scryfall.io/")));
                // Cards from other sets (e.g. Special Guests) do not count for this set.
                new_cards += cards.iter().filter(|c| c.is_new && c.set_code == s.code).count();
            }
            let view = g.collection(&s.code).unwrap();
            let sold = g.sell_duplicates(&s.code).unwrap();
            println!(
                "{}: {}/{} cards ({:.1}%), {new_cards} discoveries over 3 boosters, duplicates sold for {} cents",
                s.code, view.owned_count, view.total_count, view.completion_percent, sold.total_cents
            );
            assert_eq!(view.owned_count, new_cards);
            assert!(view.owned_count > 0 && view.owned_count <= view.total_count);
        }
        let view = g.achievements("en").unwrap();
        println!("achievements: {}/{} unlocked", view.unlocked_count, view.total);
        for a in g.take_unlocks("en") {
            println!("  unlocked: {} ({}) reward: {:?}", a.name, a.rarity, a.reward);
        }
        println!("balance after the session: {} cents", g.wallet().unwrap().balance_cents);
    }
}
