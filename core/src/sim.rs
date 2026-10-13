//! Simulation mode: opens N boosters and compares observed rates to expected rates.

use crate::booster::BoosterConfig;
use crate::draw::{open_booster, ColorIndex, DrawError};
use rand::Rng;
use std::collections::BTreeMap;

#[derive(Debug)]
pub struct SimReport {
    pub boosters: u64,
    /// (uuid, foil) -> observed average number of copies per booster
    pub observed: BTreeMap<(String, bool), f64>,
    /// same, theoretical value
    pub expected: BTreeMap<(String, bool), f64>,
}

impl SimReport {
    /// Largest absolute gap between observed and expected rate (per card, per booster).
    pub fn max_abs_deviation(&self) -> f64 {
        self.expected
            .iter()
            .map(|(k, e)| (self.observed.get(k).copied().unwrap_or(0.0) - e).abs())
            .fold(0.0, f64::max)
    }
}

impl SimReport {
    /// Largest |z-score| of (observed - expected) over all cards, ignoring cards
    /// too rare to be statistically meaningful at this sample size.
    pub fn max_abs_z(&self) -> f64 {
        let n = self.boosters as f64;
        self.expected
            .iter()
            .filter(|&(_, &e)| e * n >= 30.0)
            .map(|(k, &e)| {
                let o = self.observed.get(k).copied().unwrap_or(0.0);
                ((o - e) / (e / n).sqrt()).abs()
            })
            .fold(0.0, f64::max)
    }
}

/// Theoretical expected number of copies of each card per booster.
/// Exact for sheets that allow duplicates; first-order approximation otherwise
/// (later draws from the same sheet are slightly conditioned).
///
/// Cards of color-balanced sheets are left out: balancing changes their per-card
/// frequencies by design (see `draw`), so they are checked by invariant instead.
pub fn expected_rates(cfg: &BoosterConfig) -> BTreeMap<(String, bool), f64> {
    rates(cfg, false)
}

/// Expected value of one booster: the sum over cards of (expected copies x price).
/// `price_of(uuid, foil)` gives a card's price in any unit; unpriced cards count for 0.
/// Unlike `expected_rates`, color-balanced sheets are included (first-order estimate).
pub fn expected_value_cents<F: Fn(&str, bool) -> Option<u32>>(cfg: &BoosterConfig, price_of: F) -> f64 {
    rates(cfg, true)
        .iter()
        .map(|((uuid, foil), rate)| rate * f64::from(price_of(uuid, *foil).unwrap_or(0)))
        .sum()
}

fn rates(cfg: &BoosterConfig, include_balanced: bool) -> BTreeMap<(String, bool), f64> {
    let mut out = BTreeMap::new();
    for pack in &cfg.boosters {
        let p = pack.weight as f64 / cfg.boosters_total_weight as f64;
        for (slot, &n) in &pack.contents {
            let sheet = &cfg.sheets[slot];
            for (uuid, &w) in &sheet.cards {
                let per_booster = if sheet.fixed {
                    1.0
                } else {
                    n as f64 * w as f64 / sheet.total_weight as f64
                };
                *out.entry((uuid.clone(), sheet.foil)).or_insert(0.0) += p * per_booster;
            }
        }
    }
    for sheet in cfg.sheets.values().filter(|s| s.balance_colors && !include_balanced) {
        for uuid in sheet.cards.keys() {
            out.remove(&(uuid.clone(), sheet.foil));
        }
    }
    out
}

pub fn simulate<R: Rng + ?Sized>(
    cfg: &BoosterConfig,
    colors: &ColorIndex,
    n: u64,
    rng: &mut R,
) -> Result<SimReport, DrawError> {
    let mut counts: BTreeMap<(String, bool), u64> = BTreeMap::new();
    for _ in 0..n {
        for c in open_booster(cfg, colors, rng)? {
            *counts.entry((c.uuid, c.foil)).or_insert(0) += 1;
        }
    }
    let observed = counts.into_iter().map(|(k, v)| (k, v as f64 / n as f64)).collect();
    Ok(SimReport { boosters: n, observed, expected: expected_rates(cfg) })
}
