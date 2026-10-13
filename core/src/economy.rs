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

    /// What the Boosterium pays for a card worth `market_cents` (rounded down).
    pub fn sell_value_cents(&self, market_cents: u32) -> u32 {
        (u64::from(market_cents) * u64::from(self.sell_rate_percent) / 100) as u32
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
