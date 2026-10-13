//! Decides at launch whether the local data must be rebuilt. The client builds its own
//! data from MTGJSON and Scryfall (see `builder`), so the only state is the local manifest.
//! Pure logic: no network, no clock access, so it is testable.

use crate::catalog::Manifest;
use serde::Deserialize;

/// What to download and how fresh it must be (shipped as `config/data.json`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataConfig {
    /// Set codes the game offers.
    pub sets: Vec<String>,
    /// Booster kinds offered; each set keeps those it has.
    #[serde(default = "default_boosters")]
    pub boosters: Vec<String>,
    /// Data older than this is rebuilt at launch. Prices move daily, so a day is the natural unit.
    #[serde(default = "default_max_age_hours")]
    pub max_age_hours: i64,
    /// Boosters simulated per kept config to validate its rates.
    #[serde(default = "default_sim_boosters")]
    pub sim_boosters: u64,
}

fn default_boosters() -> Vec<String> {
    ["play", "draft", "set", "collector"].map(String::from).to_vec()
}

fn default_max_age_hours() -> i64 {
    24
}

fn default_sim_boosters() -> u64 {
    20_000
}

impl DataConfig {
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshReason {
    /// Nothing usable on disk yet (first launch).
    NoData,
    /// A configured set is not installed.
    MissingSet(String),
    /// The configuration asks for other booster kinds than the data was built for.
    BoostersChanged,
    /// The data is older than the configured maximum age.
    Stale,
}

/// Whether the data must be rebuilt now. `now` is a Unix time in seconds.
pub fn refresh_reason(local: Option<&Manifest>, cfg: &DataConfig, now: i64) -> Option<RefreshReason> {
    let Some(manifest) = local else { return Some(RefreshReason::NoData) };
    if let Some(code) = cfg.sets.iter().find(|c| !manifest.sets.contains_key(*c)) {
        return Some(RefreshReason::MissingSet(code.clone()));
    }
    if manifest.booster_kinds != cfg.boosters {
        return Some(RefreshReason::BoostersChanged);
    }
    // An unreadable timestamp or a clock set in the past counts as stale: rebuilding is harmless.
    let age = manifest.generated_at.parse::<i64>().ok().map(|t| now - t);
    match age {
        Some(secs) if (0..cfg.max_age_hours * 3600).contains(&secs) => None,
        _ => Some(RefreshReason::Stale),
    }
}
