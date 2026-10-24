//! Bets on boosters (see `unsealed_core::betting`): the chance of a bet on a booster before it
//! is opened, and the bets its draw settles. A bet's stake leaves the wallet when the booster
//! is torn open; what a winner pays waits until every card is seen (`Store::pay_bets`), so the
//! wallet never gives the draw away.

use super::{err, Game};
use crate::store::PlacedBet;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use unsealed_core::achievements::Event;
use unsealed_core::betting::{chance, payout_cents, Bet, BetCard, Chance, SimulatedPacks};

/// Key under which a game keeps the bets placed on every booster (`save_meta`).
const KEPT_BETS: &str = "kept_bets";

/// A bet as the player places it on a booster.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BetRequest {
    pub bet: Bet,
    pub stake_cents: u32,
}

/// A bet kept from one opening to the next (placed again on every booster). One naming a card
/// only applies to the boosters of its set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeptBet {
    pub bet: Bet,
    pub stake_cents: u32,
    #[serde(default)]
    pub set_code: Option<String>,
    /// The card it names, as the interface shows it.
    #[serde(default)]
    pub card_name: Option<String>,
    /// Paused: kept, but not placed until switched on again.
    #[serde(default = "on")]
    pub active: bool,
}

fn on() -> bool {
    true
}

/// A bet settled by the draw of its booster.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettledBet {
    pub bet: Bet,
    pub stake_cents: u32,
    /// The chance it had of winning.
    pub probability: f64,
    pub won: bool,
    /// What it pays back, the stake included (0 when lost).
    pub payout_cents: u64,
}

/// An opened booster: its cards, the bets it settled, and those left out (no stake taken).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedBooster {
    pub cards: Vec<super::OpenedCard>,
    pub bets: Vec<SettledBet>,
    pub skipped: Vec<SkippedBet>,
    /// The artifact found in it, if any (see `Game::find_artifact`).
    pub artifact: Option<super::ArtifactView>,
}

/// A bet left out of an opening, and why. The booster opens all the same.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedBet {
    pub bet: Bet,
    pub reason: SkipReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SkipReason {
    /// Nothing to price on this booster (certain either way, or too rare): a kept bet on a
    /// foil, say, in a booster that always has one.
    Unpriced,
    /// A stake under the minimum.
    Stake,
    /// The wallet could not pay its stake (the bets before it in the slip were placed).
    Funds,
}

/// What a bet slip needs to know of a booster.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BetContext {
    /// What the booster costs now (to bet on it being profitable or not).
    pub booster_price_cents: u32,
    pub house_margin_percent: u32,
    pub min_stake_cents: u32,
}

/// How likely a bet is; `probability` is `None` when it cannot be priced, and `unpriced` says
/// why.
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BetChance {
    pub probability: Option<f64>,
    /// Worked out from the sheets (else counted over simulated boosters).
    pub exact: bool,
    pub unpriced: Option<Unpriced>,
}

/// Why a bet has no odds.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Unpriced {
    /// Won by every booster of this kind (a Play Booster always holds a foil).
    Always,
    /// Won by none (a foil in a set printed before foils).
    Never,
    /// Too rare, or too sure, for the simulated boosters to tell its chance.
    Simulation,
}

/// Below this distance from 0 or 1, an exact chance is certain: the sums over the sheets leave
/// rounding errors (99.9999999 %), which were priced ×1.00.
const CERTAIN: f64 = 1e-9;

/// The cards of a booster's set as the bets see them, and boosters drawn ahead.
pub(super) struct BetTable {
    cards: Vec<BetCard>,
    index: HashMap<String, usize>,
    packs: Option<SimulatedPacks>,
}

/// The event an achievement can listen to.
pub(super) fn bet_event(bet: &SettledBet) -> Event {
    Event::new("BetSettled")
        .with("won", bet.won)
        .with("conditions", bet.bet.clauses.len() as i64)
        .with("stakeCents", i64::from(bet.stake_cents))
        .with("payoutCents", bet.payout_cents as i64)
        .with("probabilityPermille", (bet.probability * 1000.0).round() as i64)
}

impl Game {
    pub fn bet_context(&mut self, set_code: &str, kind: &str) -> Result<BetContext, String> {
        self.ensure_loaded(set_code)?;
        self.ensure_prices();
        Ok(BetContext {
            booster_price_cents: self.booster_price(set_code, kind),
            house_margin_percent: self.economy.betting.house_margin_percent,
            min_stake_cents: self.economy.betting.min_stake_cents,
        })
    }

    /// How likely a bet is on a booster of this kind.
    pub fn bet_chance(&mut self, set_code: &str, kind: &str, bet: &Bet) -> Result<BetChance, String> {
        bet.validate()?;
        let min = self.economy.betting.min_simulated_hits as usize;
        let exact = bet.is_exact();
        let key = self.prepare_bet_table(set_code, kind, !exact)?;
        let table = &self.bet_tables[&key];
        let cfg = &self.sets[set_code].file.boosters[kind];
        let empty = SimulatedPacks::default();
        let found = chance(bet, cfg, &table.cards, &table.index, table.packs.as_ref().unwrap_or(&empty));
        let (probability, unpriced) = match found {
            Chance::Exact(p) if p <= CERTAIN => (None, Some(Unpriced::Never)),
            Chance::Exact(p) if p >= 1.0 - CERTAIN => (None, Some(Unpriced::Always)),
            Chance::Exact(p) => (Some(p), None),
            Chance::Simulated { hits: 0, .. } => (None, Some(Unpriced::Never)),
            Chance::Simulated { hits, total } if hits == total => (None, Some(Unpriced::Always)),
            Chance::Simulated { hits, total } if hits >= min && total - hits >= min => (Some(found.probability()), None),
            Chance::Simulated { .. } => (None, Some(Unpriced::Simulation)),
        };
        Ok(BetChance { probability, exact, unpriced })
    }

    pub fn kept_bets(&self) -> Result<Vec<KeptBet>, String> {
        let saved = self.store.meta(KEPT_BETS).map_err(err)?;
        Ok(saved.and_then(|json| serde_json::from_str(&json).ok()).unwrap_or_default())
    }

    pub fn set_kept_bets(&mut self, bets: &[KeptBet]) -> Result<(), String> {
        let json = serde_json::to_string(bets).map_err(err)?;
        self.store.set_meta(KEPT_BETS, &json).map_err(err)
    }

    /// Splits the bets into those placed on this booster, with their chance of winning, and
    /// those left out: a stake under the minimum, nothing to price (certain either way, too
    /// rare, or not a valid bet), or no money left for the stake (taken in the slip's order).
    /// A bet kept from another booster may well be left out: the booster still opens.
    pub(super) fn price_bets(
        &mut self,
        set_code: &str,
        kind: &str,
        bets: &[BetRequest],
    ) -> Result<(Vec<(BetRequest, f64)>, Vec<SkippedBet>), String> {
        // No bets on a box topper (a pack of one or two cards, given, not bought): the bets kept
        // for the other boosters are not even mentioned.
        if self.sets.get(set_code).is_some_and(|s| s.file.is_topper(kind)) {
            return Ok((Vec::new(), Vec::new()));
        }
        let min_stake = self.economy.betting.min_stake_cents;
        let mut available = if bets.is_empty() { 0 } else { self.store.balance().map_err(err)? };
        let (mut placed, mut skipped) = (Vec::new(), Vec::new());
        for b in bets {
            let skip = |reason| SkippedBet { bet: b.bet.clone(), reason };
            if b.stake_cents < min_stake {
                skipped.push(skip(SkipReason::Stake));
                continue;
            }
            let chance = match self.bet_chance(set_code, kind, &b.bet) {
                Ok(c) => c.probability,
                Err(_) if b.bet.validate().is_err() => None,
                Err(e) => return Err(e),
            };
            match chance {
                None => skipped.push(skip(SkipReason::Unpriced)),
                Some(_) if i64::from(b.stake_cents) > available => skipped.push(skip(SkipReason::Funds)),
                Some(p) => {
                    available -= i64::from(b.stake_cents);
                    placed.push((b.clone(), p));
                }
            }
        }
        Ok((placed, skipped))
    }

    /// Settles the bets placed on a booster by its cards (with their chances, from `price_bets`).
    pub(super) fn settle_bets(
        &mut self,
        set_code: &str,
        kind: &str,
        bets: &[(BetRequest, f64)],
        drawn: &[(String, bool)],
    ) -> Result<(Vec<SettledBet>, Vec<PlacedBet>), String> {
        if bets.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }
        let margin = self.economy.betting.house_margin_percent;
        let key = self.prepare_bet_table(set_code, kind, false)?;
        let table = &self.bet_tables[&key];
        let pack: Vec<(usize, bool)> = drawn.iter().filter_map(|(uuid, foil)| Some((*table.index.get(uuid)?, *foil))).collect();
        let settled: Vec<SettledBet> = bets
            .iter()
            .map(|(b, probability)| {
                let probability = *probability;
                let won = b.bet.holds(&pack, &table.cards);
                SettledBet {
                    bet: b.bet.clone(),
                    stake_cents: b.stake_cents,
                    probability,
                    won,
                    payout_cents: if won { payout_cents(b.stake_cents, probability, margin) } else { 0 },
                }
            })
            .collect();
        let placed = settled
            .iter()
            .map(|s| PlacedBet {
                bet: serde_json::to_string(&s.bet).unwrap_or_default(),
                stake_cents: s.stake_cents,
                probability: s.probability,
                won: s.won,
                payout_cents: s.payout_cents as i64,
            })
            .collect();
        Ok((settled, placed))
    }

    /// The bets' view of a booster's set, kept until the data or the prices change (the bets
    /// on prices and values depend on them); `with_packs`: with boosters drawn ahead (always the
    /// same ones, from a fixed seed: the odds do not move from one look to the next).
    /// Returns the table's key in `bet_tables`.
    fn prepare_bet_table(&mut self, set_code: &str, kind: &str, with_packs: bool) -> Result<String, String> {
        self.ensure_loaded(set_code)?;
        self.ensure_prices();
        if !self.sets[set_code].file.boosters.contains_key(kind) {
            return Err(format!("set {set_code} has no `{kind}` booster"));
        }
        let key = format!("{set_code}/{kind}");
        if !self.bet_tables.contains_key(&key) {
            let set = &self.sets[set_code];
            let cards: Vec<BetCard> = set
                .file
                .cards
                .iter()
                .map(|c| {
                    let land = c.types.iter().any(|t| t == "land");
                    BetCard {
                        uuid: c.uuid.clone(),
                        rarity: c.rarity.clone(),
                        types: c.types.clone(),
                        // A land is colourless: its colours are those of the mana it makes.
                        colors: if land { c.color_identity.clone() } else { c.colors.clone() },
                        token: c.token,
                        art: c.art,
                        full_art: c.full_art,
                        special_frame: c.special_frame,
                        price_cents: [false, true].map(|foil| self.market_price(&c.uuid, foil).unwrap_or(0)),
                    }
                })
                .collect();
            let index = cards.iter().enumerate().map(|(i, c)| (c.uuid.clone(), i)).collect();
            self.bet_tables.insert(key.clone(), BetTable { cards, index, packs: None });
        }
        if with_packs && self.bet_tables[&key].packs.is_none() {
            let set = &self.sets[set_code];
            let mut rng = ChaCha8Rng::seed_from_u64(0xB37);
            let index = &self.bet_tables[&key].index;
            let n = self.economy.betting.simulated_boosters;
            let packs = SimulatedPacks::draw(&set.file.boosters[kind], &set.colors, index, n, &mut rng).map_err(err)?;
            self.bet_tables.get_mut(&key).expect("just made").packs = Some(packs);
        }
        Ok(key)
    }
}
