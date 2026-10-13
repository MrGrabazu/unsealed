//! Economy rules, driven by a hand-edited config file (see `config/economy.json`).
//! Amounts are integer cents of the configured currency.

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
}

#[derive(Debug, Clone, Deserialize)]
pub struct Currency {
    /// "USD" or "EUR": also selects which Scryfall price is used as the market price.
    pub code: String,
    pub symbol: String,
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
        if !matches!(self.currency.code.as_str(), "USD" | "EUR") {
            return Err(ConfigError::Invalid(format!(
                "unsupported currency `{}` (expected USD or EUR)",
                self.currency.code
            )));
        }
        if self.sell_rate_percent > 100 {
            return Err(ConfigError::Invalid("sellRatePercent must be between 0 and 100".into()));
        }
        if self.booster_sell_rate_percent > 100 {
            return Err(ConfigError::Invalid("boosterSellRatePercent must be between 0 and 100".into()));
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
        match (self.currency.code.as_str(), foil) {
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
