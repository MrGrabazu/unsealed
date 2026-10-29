//! Economy rules, driven by a hand-edited config file (see `config/economy.json`).
//! Amounts are integer cents of the market currency (USD or EUR). The player never sees
//! them: the interface shows Seals, a fictional currency derived from them (`Currency`).

use crate::booster::ConfigError;
use crate::catalog::PriceSet;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EconomyConfig {
    pub currency: Currency,
    pub starting_balance_cents: u32,
    /// Share of the market price paid when selling a card (0-100).
    pub sell_rate_percent: u32,
    /// Share of the current booster price paid when selling a sealed booster back (0-100).
    pub booster_sell_rate_percent: u32,
    pub booster_prices: BoosterPrices,
    pub charity: Charity,
    #[serde(default)]
    pub cancellation: Cancellation,
    /// Price of a card nothing prices (neither its own printing in any finish, nor its other
    /// printings: art cards, some tokens), by rarity ("common"…"mythic", "token", "art"; any
    /// other rarity takes "default"), so that it can still be sold. Shown as an estimate.
    #[serde(default)]
    pub fallback_prices: BTreeMap<String, FallbackPrice>,
    #[serde(default)]
    pub betting: Betting,
    #[serde(default)]
    pub income: Income,
    /// The cards of a set the game crafts a booster for (Secret Lair), found now and then in the
    /// boosters of the other sets, by that set's code. Not counted in the boosters' expected
    /// value: a rare bonus.
    #[serde(default)]
    pub crafted_guests: BTreeMap<String, CraftedGuest>,
}

/// How often a crafted set's card hides in another set's booster: one booster in this many (0:
/// never), a collector one more often. The card is one of a booster of its own, drawn apart.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CraftedGuest {
    pub one_in: u32,
    pub collector_one_in: u32,
}

/// What the collection earns by itself: each minute, a share of what it would sell for. No
/// strings attached: the more it is worth, the more it earns, which rewards keeping one's best
/// cards; but the share shrinks as it grows, the income never passing `max_per_minute_cents`
/// (else it would buy boosters that make it earn more, and so on without end).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Income {
    /// Earned each minute by a small collection, in percent of its resale value (real prices
    /// only).
    pub percent_per_minute: f64,
    /// The most the collection earns in a minute, however much it is worth (none: no limit).
    #[serde(default)]
    pub max_per_minute_cents: Option<f64>,
    /// The collection earns for this long at most between two visits (while the game is
    /// closed, or left on another screen).
    pub offline_cap_minutes: u32,
}

impl Default for Income {
    fn default() -> Self {
        Self { percent_per_minute: 0.1, max_per_minute_cents: None, offline_cap_minutes: 480 }
    }
}

impl Income {
    /// What a collection worth `value_cents` earns in a minute, in cents with their fraction:
    /// `percent_per_minute` of it while it is small, then less and less, nearing
    /// `max_per_minute_cents` without ever passing it (max × (1 − e^(−share × value / max))).
    pub fn per_minute_cents(&self, value_cents: i64) -> f64 {
        let linear = value_cents.max(0) as f64 * self.percent_per_minute / 100.0;
        match self.max_per_minute_cents {
            Some(max) if max > 0.0 => max * (1.0 - (-linear / max).exp()),
            Some(_) => 0.0,
            None => linear,
        }
    }

    /// What it earns over `minutes` (capped), in cents with their fraction (kept from one
    /// payment to the next).
    pub fn earned_cents(&self, value_cents: i64, minutes: f64) -> f64 {
        let minutes = minutes.clamp(0.0, f64::from(self.offline_cap_minutes));
        self.per_minute_cents(value_cents) * minutes
    }

    /// The same, with the artifacts' income bonuses: everything multiplied, the limit too.
    pub fn boosted(&self, multiplier: f64) -> Self {
        Self {
            percent_per_minute: self.percent_per_minute * multiplier,
            max_per_minute_cents: self.max_per_minute_cents.map(|m| m * multiplier),
            offline_cap_minutes: self.offline_cap_minutes,
        }
    }
}

/// Bets on a booster's contents (see `betting`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Betting {
    /// Share of the fair odds kept by the house (0: fair odds, a bet is worth its stake on average).
    pub house_margin_percent: u32,
    /// Boosters simulated to price the bets whose chance cannot be worked out exactly.
    pub simulated_boosters: u32,
    /// Such a bet can be placed only if it held at least this many times, and failed at least
    /// as many, in the simulation (rarer, its odds would be too rough).
    pub min_simulated_hits: u32,
    pub min_stake_cents: u32,
}

impl Default for Betting {
    fn default() -> Self {
        Self { house_margin_percent: 0, simulated_boosters: 100_000, min_simulated_hits: 20, min_stake_cents: 10 }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FallbackPrice {
    pub normal_cents: u32,
    /// The foil copy (for an art card: its gold signed version).
    pub foil_cents: u32,
}

/// Undoing a transaction. A sale can be undone at any time: the player pays back what they
/// received plus a penalty that grows with the time elapsed. A purchase can only be undone
/// right away (a misclick), while the boosters are sealed: otherwise refunding the price paid
/// would insure every purchase against a falling market.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cancellation {
    /// Penalty by age of the sale, in increasing `within_minutes` order.
    pub sale_steps: Vec<PenaltyStep>,
    /// Penalty once older than the last step.
    pub sale_penalty_after_percent: u32,
    /// How long a purchase can be undone.
    pub purchase_within_minutes: u32,
    pub purchase_penalty_percent: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PenaltyStep {
    pub within_minutes: u32,
    pub penalty_percent: u32,
}

impl Default for Cancellation {
    fn default() -> Self {
        let step = |within_minutes, penalty_percent| PenaltyStep { within_minutes, penalty_percent };
        Self {
            sale_steps: vec![step(5, 5), step(60, 10), step(1_440, 25), step(4_320, 50), step(20_160, 75)],
            sale_penalty_after_percent: 90,
            purchase_within_minutes: 5,
            purchase_penalty_percent: 5,
        }
    }
}

impl Cancellation {
    /// Penalty for undoing a sale made `age_secs` ago, in percent of the amount received.
    pub fn sale_penalty_percent(&self, age_secs: i64) -> u32 {
        let minutes = age_secs.max(0) as f64 / 60.0;
        self.sale_steps
            .iter()
            .find(|s| minutes <= f64::from(s.within_minutes))
            .map_or(self.sale_penalty_after_percent, |s| s.penalty_percent)
    }

    /// What undoing a sale of `received_cents` costs: the amount plus the penalty.
    pub fn sale_cancel_cost_cents(&self, received_cents: i64, age_secs: i64) -> i64 {
        received_cents + received_cents * i64::from(self.sale_penalty_percent(age_secs)) / 100
    }

    /// What undoing a purchase of `paid_cents` refunds, or `None` once it is too late.
    pub fn purchase_refund_cents(&self, paid_cents: i64, age_secs: i64) -> Option<i64> {
        (age_secs.max(0) <= i64::from(self.purchase_within_minutes) * 60)
            .then(|| paid_cents - paid_cents * i64::from(self.purchase_penalty_percent) / 100)
    }
}

/// The market prices behind the economy, and how the game's own currency, the Seals, is
/// derived from them. Seals keep the game visibly apart from real money while following
/// the real market.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Currency {
    /// "USD" or "EUR": which Scryfall prices drive the economy.
    #[serde(alias = "code")]
    pub market: String,
    /// Seals shown for one cent of the market currency.
    #[serde(default = "one")]
    pub seals_per_cent: f64,
}

fn one() -> f64 {
    1.0
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoosterPrices {
    /// Price when the market value of the booster is unknown.
    pub default_cents: u32,
    /// Price as a percentage of the booster's expected market value (100 = fair price).
    #[serde(default = "hundred")]
    pub ev_markup_percent: u32,
    /// The price never goes below this.
    #[serde(default)]
    pub min_cents: u32,
    /// Fixed prices that ignore the market: set code -> booster kind -> price.
    #[serde(default)]
    pub overrides: BTreeMap<String, BTreeMap<String, u32>>,
}

fn hundred() -> u32 {
    100
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Charity {
    pub amount_cents: u32,
    pub max_claims_per_day: u32,
}

impl EconomyConfig {
    pub fn from_json(json: &str) -> Result<Self, ConfigError> {
        let cfg: Self = serde_json::from_str(json)?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if !matches!(self.currency.market.as_str(), "USD" | "EUR") {
            return Err(ConfigError::Invalid(format!(
                "unsupported market currency `{}` (expected USD or EUR)",
                self.currency.market
            )));
        }
        if !(self.currency.seals_per_cent > 0.0) {
            return Err(ConfigError::Invalid("currency.sealsPerCent must be positive".into()));
        }
        if self.sell_rate_percent > 100 {
            return Err(ConfigError::Invalid("sellRatePercent must be between 0 and 100".into()));
        }
        if self.booster_sell_rate_percent > 100 {
            return Err(ConfigError::Invalid("boosterSellRatePercent must be between 0 and 100".into()));
        }
        let c = &self.cancellation;
        let increasing = c.sale_steps.windows(2).all(|w| w[0].within_minutes < w[1].within_minutes);
        let penalties = c.sale_steps.iter().map(|s| s.penalty_percent).chain([c.sale_penalty_after_percent, c.purchase_penalty_percent]);
        if !increasing || penalties.into_iter().any(|p| p > 100) {
            return Err(ConfigError::Invalid(
                "cancellation: steps must have increasing withinMinutes and penalties of at most 100%".into(),
            ));
        }
        if !(0.0..=100.0).contains(&self.income.percent_per_minute) {
            return Err(ConfigError::Invalid("income.percentPerMinute must be between 0 and 100".into()));
        }
        if self.income.max_per_minute_cents.is_some_and(|m| !(m > 0.0)) {
            return Err(ConfigError::Invalid("income.maxPerMinuteCents must be positive".into()));
        }
        if self.booster_prices.default_cents == 0 || self.booster_prices.ev_markup_percent == 0 {
            return Err(ConfigError::Invalid(
                "boosterPrices.defaultCents and evMarkupPercent must be positive".into(),
            ));
        }
        Ok(())
    }

    /// Price of a sealed booster. A configured override wins; otherwise the price follows
    /// the booster's expected market value (`ev_cents`) when known, else the default.
    pub fn booster_price_cents(&self, set_code: &str, kind: &str, ev_cents: Option<u32>) -> u32 {
        let prices = &self.booster_prices;
        if let Some(fixed) = prices.overrides.get(set_code).and_then(|kinds| kinds.get(kind)) {
            return *fixed;
        }
        match ev_cents {
            Some(ev) => ((u64::from(ev) * u64::from(prices.ev_markup_percent) / 100) as u32).max(prices.min_cents),
            None => prices.default_cents,
        }
    }

    /// What the Boosterium pays for a sealed booster currently priced `price_cents`.
    pub fn booster_sell_value_cents(&self, price_cents: u32) -> u32 {
        (u64::from(price_cents) * u64::from(self.booster_sell_rate_percent) / 100) as u32
    }

    /// Market price of a card in the configured currency. A foil card without a foil
    /// price has no price (it is not valued as a regular copy).
    pub fn market_price_cents(&self, prices: &PriceSet, foil: bool) -> Option<u32> {
        match (self.currency.market.as_str(), foil) {
            ("EUR", true) => prices.eur_foil,
            ("EUR", false) => prices.eur,
            (_, true) => prices.usd_foil,
            (_, false) => prices.usd,
        }
    }

    /// Price of a card without any market price, from its rarity, else the `default` entry
    /// (none if neither is configured).
    pub fn fallback_price_cents(&self, rarity: &str, foil: bool) -> Option<u32> {
        let price = self.fallback_prices.get(rarity).or_else(|| self.fallback_prices.get("default"))?;
        Some(if foil { price.foil_cents } else { price.normal_cents }).filter(|&c| c > 0)
    }

    /// What the Boosterium pays for a card worth `market_cents` (rounded down).
    pub fn sell_value_cents(&self, market_cents: u32) -> u32 {
        Self::sell_value_at(market_cents, self.sell_rate_percent)
    }

    /// What a card worth `market_cents` sells for at `rate_percent` (an artifact's better rate).
    pub fn sell_value_at(market_cents: u32, rate_percent: u32) -> u32 {
        (u64::from(market_cents) * u64::from(rate_percent) / 100) as u32
    }
}

/// Whether the player may claim the daily charity. It is only for players who are truly
/// stranded: they cannot afford the cheapest booster, own no sealed booster to open and
/// no card worth selling (otherwise they must sacrifice a card, which is part of the
/// game), and they have not claimed too much today.
pub fn charity_available(
    charity: &Charity,
    balance_cents: i64,
    cheapest_booster_cents: u32,
    has_sellable_cards: bool,
    has_sealed_boosters: bool,
    claims_today: u32,
) -> bool {
    claims_today < charity.max_claims_per_day
        && balance_cents < i64::from(cheapest_booster_cents)
        && !has_sellable_cards
        && !has_sealed_boosters
}
