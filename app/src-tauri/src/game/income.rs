//! What the collection earns by itself (see `EconomyConfig::income`): each minute, a small
//! share of what it would sell for, paid whenever the interface asks (every minute while the
//! game is shown, and on coming back, for the time away up to a cap).

use super::{err, Game};
use serde::Serialize;
use unsealed_core::achievements::Event;

/// Keys of `save_meta`: until when the income was paid (Unix time, seconds), the fraction of a
/// cent left over, and the collection's value it was last paid on.
const PAID_UNTIL: &str = "income_paid_until";
const CARRY: &str = "income_carry";
const VALUE: &str = "income_value";

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IncomeView {
    /// What the collection would sell for (real market prices only).
    pub collection_value_cents: i64,
    /// What it earns each minute, in cents (with their fraction).
    pub per_minute_cents: f64,
    /// Paid by this call.
    pub earned_cents: i64,
    /// The time this call paid for, in minutes (capped).
    pub minutes: f64,
    pub percent_per_minute: f64,
    pub offline_cap_minutes: u32,
}

impl Game {
    /// Pays what the collection earned since the last payment. The very first call only
    /// starts the count. While a booster is being revealed, the income goes on with the value
    /// the collection had before it, so as to give nothing away.
    pub fn collect_income(&mut self) -> Result<IncomeView, String> {
        self.ensure_prices();
        let now = (self.clock)().now;
        let meta = |key| self.store.meta(key).ok().flatten();
        let saved_value = meta(VALUE).and_then(|v| v.parse::<i64>().ok());
        let value = match saved_value {
            Some(v) if self.pending_reveal.is_some() => v,
            _ => self.collection_resale_value()?,
        };
        let paid_until = meta(PAID_UNTIL).and_then(|v| v.parse::<i64>().ok());
        let carry = meta(CARRY).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
        let rules = &self.economy.income;
        let minutes = paid_until.map_or(0.0, |at| (now - at).max(0) as f64 / 60.0);
        let total = rules.earned_cents(value, minutes) + carry;
        let earned = total.floor();
        let view = IncomeView {
            collection_value_cents: value,
            per_minute_cents: rules.earned_cents(value, 1.0),
            earned_cents: earned as i64,
            minutes: minutes.min(f64::from(rules.offline_cap_minutes)),
            percent_per_minute: rules.percent_per_minute,
            offline_cap_minutes: rules.offline_cap_minutes,
        };
        let saved = [(PAID_UNTIL, now.to_string()), (CARRY, (total - earned).to_string()), (VALUE, value.to_string())];
        self.store.credit_income(view.earned_cents, &saved, now).map_err(err)?;
        if view.earned_cents > 0 {
            let mut events = vec![Event::new("IncomeEarned").with("amountCents", view.earned_cents)];
            events.extend(self.balance_event());
            self.emit(events);
        }
        Ok(view)
    }

    /// What the collection would sell for at the Boosterium, on real market prices only:
    /// estimated prices (a limited card's multiplied ones among them) and rarity fallbacks
    /// earn nothing.
    fn collection_resale_value(&self) -> Result<i64, String> {
        let Some(prices) = self.prices.as_ref() else { return Ok(0) };
        let mut value = 0i64;
        for (uuid, foil, quantity) in self.store.owned_cards().map_err(err)? {
            let Some(entry) = prices.cards.get(&uuid).filter(|p| !p.estimated) else { continue };
            if let Some(market) = self.economy.market_price_cents(entry, foil) {
                value += i64::from(self.economy.sell_value_cents(market)) * i64::from(quantity);
            }
        }
        Ok(value)
    }
}
