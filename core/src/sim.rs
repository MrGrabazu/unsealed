//! Simulation mode: opens N boosters and compares observed rates to expected rates.

use crate::booster::BoosterConfig;
use crate::draw::{open_booster, DrawError};
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
pub fn expected_rates(cfg: &BoosterConfig) -> BTreeMap<(String, bool), f64> {
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
    out
}

pub fn simulate<R: Rng + ?Sized>(
    cfg: &BoosterConfig,
    n: u64,
    rng: &mut R,
) -> Result<SimReport, DrawError> {
    let mut counts: BTreeMap<(String, bool), u64> = BTreeMap::new();
    for _ in 0..n {
        for c in open_booster(cfg, rng)? {
            *counts.entry((c.uuid, c.foil)).or_insert(0) += 1;
        }
    }
    let observed = counts.into_iter().map(|(k, v)| (k, v as f64 / n as f64)).collect();
    Ok(SimReport { boosters: n, observed, expected: expected_rates(cfg) })
}
