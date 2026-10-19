//! Bets on what a booster holds, placed just before it is opened, with odds from the real
//! chances. A bet is a set of conditions that must all hold: on the cards ("at least 1 basic
//! land, green, in foil", "no mythic", "exactly 2 rares") or on the booster's value. Its chance
//! is exact when it asks whether any card of a kind comes (computed from the sheets), else
//! counted over simulated boosters. Everything is in the game's virtual currency.

use crate::booster::BoosterConfig;
use crate::draw::{open_booster, ColorIndex, DrawError};
use crate::sim::inclusion_probabilities;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A bet: every one of its conditions must hold.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bet {
    pub clauses: Vec<Clause>,
}

/// One condition of a bet.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Clause {
    /// How many cards of the booster match `filter`.
    Cards { compare: Compare, count: u32, filter: CardFilter },
    /// What the booster's cards are worth together (`Exactly` is not offered).
    Value { compare: Compare, cents: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Compare {
    AtLeast,
    Exactly,
    AtMost,
}

impl Compare {
    fn holds(self, value: u64, target: u64) -> bool {
        match self {
            Compare::AtLeast => value >= target,
            Compare::Exactly => value == target,
            Compare::AtMost => value <= target,
        }
    }
}

/// Which cards a condition counts. Every criterion is optional; those given must all match.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CardFilter {
    /// A given card (its uuid): the other criteria still apply (its finish, say).
    pub card: Option<String>,
    pub kind: CardKind,
    /// Any of these ("common", "uncommon", "rare", "mythic", "special"…).
    pub rarities: Vec<String>,
    /// Any of these.
    pub types: Vec<CardType>,
    /// Any of these.
    pub colors: Vec<ColorChoice>,
    pub finish: Finish,
    pub treatment: Treatment,
    /// The card's price in its finish, within these bounds.
    pub min_cents: Option<u32>,
    pub max_cents: Option<u32>,
}

/// Cards of the set, its tokens, its art cards, or all of them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CardKind {
    #[default]
    Cards,
    Tokens,
    ArtCards,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CardType {
    Land,
    BasicLand,
    Creature,
    Instant,
    Sorcery,
    Artifact,
    Enchantment,
    Planeswalker,
    Battle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ColorChoice {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
    Multicolor,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Finish {
    #[default]
    Any,
    Normal,
    Foil,
}

impl Finish {
    fn matches(self, foil: bool) -> bool {
        match self {
            Finish::Any => true,
            Finish::Normal => !foil,
            Finish::Foil => foil,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Treatment {
    #[default]
    Any,
    FullArt,
    SpecialFrame,
}

/// What the bets know of a card of the set.
#[derive(Debug, Clone, Default)]
pub struct BetCard {
    pub uuid: String,
    pub rarity: String,
    /// Its types and supertypes, in lower case ("basic", "land", "creature"…).
    pub types: Vec<String>,
    /// Its colors ("W", "U", "B", "R", "G"); a land's are those of its color identity.
    pub colors: Vec<String>,
    pub token: bool,
    pub art: bool,
    pub full_art: bool,
    pub special_frame: bool,
    /// Its price, plain and foil (0 when it has none).
    pub price_cents: [u32; 2],
}

impl CardFilter {
    /// Whether a card, in a finish, is one this filter counts.
    pub fn matches(&self, card: &BetCard, foil: bool) -> bool {
        if let Some(uuid) = &self.card {
            if &card.uuid != uuid {
                return false;
            }
        } else {
            let kind_ok = match self.kind {
                CardKind::Cards => !card.token && !card.art,
                CardKind::Tokens => card.token,
                CardKind::ArtCards => card.art,
                CardKind::All => true,
            };
            if !kind_ok {
                return false;
            }
        }
        if !self.finish.matches(foil) {
            return false;
        }
        if !self.rarities.is_empty() && !self.rarities.iter().any(|r| *r == card.rarity) {
            return false;
        }
        let has = |t: &str| card.types.iter().any(|x| x == t);
        if !self.types.is_empty()
            && !self.types.iter().any(|t| match t {
                CardType::Land => has("land"),
                CardType::BasicLand => has("land") && has("basic"),
                CardType::Creature => has("creature"),
                CardType::Instant => has("instant"),
                CardType::Sorcery => has("sorcery"),
                CardType::Artifact => has("artifact"),
                CardType::Enchantment => has("enchantment"),
                CardType::Planeswalker => has("planeswalker"),
                CardType::Battle => has("battle"),
            })
        {
            return false;
        }
        let color = |c: &str| card.colors.iter().any(|x| x == c);
        if !self.colors.is_empty()
            && !self.colors.iter().any(|c| match c {
                ColorChoice::White => color("W"),
                ColorChoice::Blue => color("U"),
                ColorChoice::Black => color("B"),
                ColorChoice::Red => color("R"),
                ColorChoice::Green => color("G"),
                ColorChoice::Colorless => card.colors.is_empty(),
                ColorChoice::Multicolor => card.colors.len() >= 2,
            })
        {
            return false;
        }
        let treatment_ok = match self.treatment {
            Treatment::Any => true,
            Treatment::FullArt => card.full_art,
            Treatment::SpecialFrame => card.special_frame,
        };
        let price = card.price_cents[usize::from(foil)];
        treatment_ok && self.min_cents.map_or(true, |m| price >= m) && self.max_cents.map_or(true, |m| price <= m)
    }
}

impl Bet {
    /// Whether it holds for a booster's cards (indices into `cards`, with their finish).
    pub fn holds(&self, pack: &[(usize, bool)], cards: &[BetCard]) -> bool {
        self.clauses.iter().all(|clause| match clause {
            Clause::Cards { compare, count, filter } => {
                let n = pack.iter().filter(|&&(i, foil)| filter.matches(&cards[i], foil)).count();
                compare.holds(n as u64, u64::from(*count))
            }
            Clause::Value { compare, cents } => {
                let value: u64 = pack.iter().map(|&(i, foil)| u64::from(cards[i].price_cents[usize::from(foil)])).sum();
                compare.holds(value, u64::from(*cents))
            }
        })
    }

    /// Whether it can be placed: at least one condition, none on an exact value.
    pub fn validate(&self) -> Result<(), String> {
        if self.clauses.is_empty() {
            return Err("a bet needs at least one condition".into());
        }
        if self.clauses.iter().any(|c| matches!(c, Clause::Value { compare: Compare::Exactly, .. })) {
            return Err("a bet on the value is on at least or at most an amount".into());
        }
        Ok(())
    }

    /// Whether its chance is worked out exactly (else over simulated boosters).
    pub fn is_exact(&self) -> bool {
        self.any_or_none().is_some()
    }

    /// The card a bet names, if any (such a bet only makes sense on its set's boosters).
    pub fn named_card(&self) -> Option<&str> {
        self.clauses.iter().find_map(|c| match c {
            Clause::Cards { filter, .. } => filter.card.as_deref(),
            Clause::Value { .. } => None,
        })
    }

    /// The filter of a bet that only asks whether a card of a kind comes or not ("at least
    /// one", "none"): its chance is exact. `true` with it: the bet is on none coming.
    fn any_or_none(&self) -> Option<(&CardFilter, bool)> {
        match self.clauses.as_slice() {
            [Clause::Cards { compare: Compare::AtLeast, count: 1, filter }] => Some((filter, false)),
            [Clause::Cards { compare: Compare::AtMost | Compare::Exactly, count: 0, filter }] => Some((filter, true)),
            _ => None,
        }
    }
}

/// How likely a bet is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Chance {
    /// From the sheets.
    Exact(f64),
    /// Over simulated boosters: in how many it held, out of how many.
    Simulated { hits: usize, total: usize },
}

impl Chance {
    pub fn probability(self) -> f64 {
        match self {
            Chance::Exact(p) => p,
            Chance::Simulated { hits, total } => hits as f64 / total.max(1) as f64,
        }
    }
}

/// Boosters drawn ahead to count the chances of the bets: each one's cards (indices into the
/// set's cards, with their finish), one after the other.
#[derive(Debug, Clone, Default)]
pub struct SimulatedPacks {
    cards: Vec<(u32, bool)>,
    /// Where each booster starts in `cards`.
    starts: Vec<u32>,
}

impl SimulatedPacks {
    pub fn draw<R: Rng + ?Sized>(
        cfg: &BoosterConfig,
        colors: &ColorIndex,
        index: &HashMap<String, usize>,
        n: u32,
        rng: &mut R,
    ) -> Result<Self, DrawError> {
        let mut packs = SimulatedPacks::default();
        for _ in 0..n {
            packs.starts.push(packs.cards.len() as u32);
            for d in open_booster(cfg, colors, rng)? {
                if let Some(&i) = index.get(&d.uuid) {
                    packs.cards.push((i as u32, d.foil));
                }
            }
        }
        Ok(packs)
    }

    pub fn len(&self) -> usize {
        self.starts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.starts.is_empty()
    }

    fn pack(&self, k: usize) -> impl Iterator<Item = (usize, bool)> + '_ {
        let end = self.starts.get(k + 1).map_or(self.cards.len(), |&e| e as usize);
        self.cards[self.starts[k] as usize..end].iter().map(|&(i, foil)| (i as usize, foil))
    }
}

/// How likely a bet is, on a booster of config `cfg` whose cards are `cards` (`index`: by uuid).
pub fn chance(bet: &Bet, cfg: &BoosterConfig, cards: &[BetCard], index: &HashMap<String, usize>, packs: &SimulatedPacks) -> Chance {
    if let Some((filter, none)) = bet.any_or_none() {
        let any = any_probability(cfg, cards, index, filter);
        return Chance::Exact(if none { 1.0 - any } else { any });
    }
    let mut pack = Vec::new();
    let hits = (0..packs.len())
        .filter(|&k| {
            pack.clear();
            pack.extend(packs.pack(k));
            bet.holds(&pack, cards)
        })
        .count();
    Chance::Simulated { hits, total: packs.len() }
}

/// The exact chance that a booster holds at least one card `filter` counts. Over the pack
/// variants, one minus the chance that no sheet gives one. A sheet drawn without duplicates
/// gives none of a group of cards exactly as a single card weighing as much as the group
/// would not be drawn (successive sampling is a race of exponential clocks, and the first of
/// a group's clocks rings at the rate of the group's weight).
pub fn any_probability(cfg: &BoosterConfig, cards: &[BetCard], index: &HashMap<String, usize>, filter: &CardFilter) -> f64 {
    let mut chance = 0.0;
    for pack in &cfg.boosters {
        let share = pack.weight as f64 / cfg.boosters_total_weight as f64;
        let mut missed = 1.0;
        for (slot, &n) in &pack.contents {
            let sheet = &cfg.sheets[slot];
            let (mut group, mut others) = (0u64, Vec::new());
            for (uuid, &w) in &sheet.cards {
                if w == 0 {
                    continue;
                }
                let matches = index.get(uuid).is_some_and(|&i| filter.matches(&cards[i], sheet.foil));
                if matches {
                    group += w;
                } else {
                    others.push(w);
                }
            }
            if group == 0 || n == 0 {
                continue;
            }
            let found = if sheet.fixed {
                1.0
            } else if sheet.allow_duplicates {
                1.0 - (1.0 - group as f64 / sheet.total_weight as f64).powi(n as i32)
            } else {
                others.push(group);
                *inclusion_probabilities(&others, n as usize).last().expect("the group was just added")
            };
            missed *= 1.0 - found;
        }
        chance += share * (1.0 - missed);
    }
    chance.clamp(0.0, 1.0)
}

/// What a winning bet pays back for `stake_cents` (the stake included), at a chance
/// `probability` of winning, the house keeping `margin_percent`: fair odds at 0.
pub fn payout_cents(stake_cents: u32, probability: f64, margin_percent: u32) -> u64 {
    let odds = (1.0 - f64::from(margin_percent.min(100)) / 100.0) / probability;
    (f64::from(stake_cents) * odds).floor() as u64
}
