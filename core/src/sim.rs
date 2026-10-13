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
/// Exact for sheets that allow duplicates. Without duplicates, n cards are drawn by
/// successive sampling (each pick proportional to the weights of the cards left), and
/// each card's inclusion probability is computed (see `inclusion_probabilities`). It
/// matters for the oldest sets, whose sheets mirror print sheets where one card fills
/// several slots (weights 1 to 5): n × weight / total would overestimate frequent cards.
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
            let weights: Vec<u64> = sheet.cards.values().copied().collect();
            let per_card: Vec<f64> = if sheet.fixed {
                vec![1.0; weights.len()]
            } else if sheet.allow_duplicates {
                weights.iter().map(|&w| n as f64 * w as f64 / sheet.total_weight as f64).collect()
            } else {
                inclusion_probabilities(&weights, n as usize)
            };
            for (uuid, per_booster) in sheet.cards.keys().zip(per_card) {
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

/// Probability that each card is among `n` cards drawn without replacement, each pick
/// proportional to the weights of the cards left (successive sampling). Closed forms
/// for one card or equal weights; otherwise computed per distinct weight (see
/// `inclusion_of_weight`).
pub fn inclusion_probabilities(weights: &[u64], n: usize) -> Vec<f64> {
    let positive: Vec<f64> = weights.iter().filter(|&&w| w > 0).map(|&w| w as f64).collect();
    if n >= positive.len() {
        return weights.iter().map(|&w| if w > 0 { 1.0 } else { 0.0 }).collect();
    }
    let total: f64 = positive.iter().sum();
    if n == 1 {
        return weights.iter().map(|&w| w as f64 / total).collect();
    }
    if positive.iter().all(|&w| w == positive[0]) {
        let share = n as f64 / positive.len() as f64;
        return weights.iter().map(|&w| if w > 0 { share } else { 0.0 }).collect();
    }
    let mut distinct: Vec<u64> = weights.iter().copied().filter(|&w| w > 0).collect();
    distinct.sort_unstable();
    distinct.dedup();
    let by_weight: BTreeMap<u64, f64> =
        distinct.iter().map(|&w| (w, inclusion_of_weight(&positive, w as f64, n))).collect();
    weights.iter().map(|w| by_weight.get(w).copied().unwrap_or(0.0)).collect()
}

/// Successive sampling is the same as giving each card an exponential arrival time of
/// rate its weight and keeping the first `n` arrivals. So a card of weight `w` is kept
/// with probability ∫ w·e^(-wt) · P(fewer than n other cards arrived by t) dt, where the
/// count of the others follows a Poisson binomial law (computed by dynamic programming).
///
/// The integrand vanishes once the card has surely arrived (t ≫ 1/w) or the others surely
/// filled the `n` places (t ≫ n / their total weight), whichever comes first: the integral
/// stops there, and Simpson's rule takes steps small next to the arrivals of the whole
/// sheet. (Steps sized on a single card's weight are far too coarse when a sheet holds
/// hundreds of heavily weighted cards, like CLB's set booster wildcard.)
fn inclusion_of_weight(all: &[f64], w: f64, n: usize) -> f64 {
    let mut removed = false;
    let others: Vec<f64> = all
        .iter()
        .copied()
        .filter(|&x| {
            let is_self = !removed && x == w;
            removed |= is_self;
            !is_self
        })
        .collect();
    let mut fewer = vec![0.0; n];
    // P(fewer than n other cards arrived by t).
    let mut fewer_than_n = |t: f64| -> f64 {
        fewer.iter_mut().for_each(|p| *p = 0.0);
        fewer[0] = 1.0;
        for &x in &others {
            let arrived = 1.0 - (-x * t).exp();
            for k in (0..n).rev() {
                fewer[k] = fewer[k] * (1.0 - arrived) + if k > 0 { fewer[k - 1] * arrived } else { 0.0 };
            }
        }
        fewer.iter().sum()
    };

    // e^(-40) is negligible: past `horizon`, the card itself has surely arrived; the others
    // may have filled every place earlier.
    let total: f64 = others.iter().sum::<f64>() + w;
    let mut horizon = 40.0 / w;
    let mut t = n as f64 / total;
    while t < horizon {
        if fewer_than_n(t) < 1e-14 {
            horizon = t;
            break;
        }
        t *= 2.0;
    }
    let steps = ((horizon * total * 40.0) as usize).clamp(4_000, 400_000) / 2 * 2;
    let h = horizon / steps as f64;
    let mut integrand = |t: f64| w * (-w * t).exp() * fewer_than_n(t);
    let mut sum = integrand(0.0) + integrand(horizon);
    for i in 1..steps {
        sum += integrand(i as f64 * h) * if i % 2 == 1 { 4.0 } else { 2.0 };
    }
    (sum * h / 3.0).clamp(0.0, 1.0)
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
