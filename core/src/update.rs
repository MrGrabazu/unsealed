//! Decides what the client must (re)download at launch, from the remote manifest
//! and the local state. Pure logic: no network, no clock access, so it is testable.

use crate::catalog::Manifest;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy)]
pub struct UpdateConfig {
    /// Minimum delay between two manifest checks. Prices are published daily, so
    /// this bounds staleness to roughly one publication cycle plus this delay.
    pub check_interval_secs: i64,
    /// Prices older than this are flagged as stale (e.g. warning in the UI).
    pub stale_prices_secs: i64,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self { check_interval_secs: 12 * 3600, stale_prices_secs: 48 * 3600 }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LocalState {
    /// Installed set code -> `source_version` of the local file.
    pub sets: BTreeMap<String, String>,
    pub prices_updated_at: Option<String>,
    /// Unix time of the last successful manifest check.
    pub last_check_at: Option<i64>,
    /// Unix time at which the local prices file was downloaded.
    pub prices_fetched_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UpdatePlan {
    pub sets_to_fetch: Vec<String>,
    pub fetch_prices: bool,
}

/// Whether the manifest should be fetched now.
pub fn should_check(local: &LocalState, now: i64, cfg: &UpdateConfig) -> bool {
    match local.last_check_at {
        Some(t) => now - t >= cfg.check_interval_secs || now < t,
        None => true,
    }
}

/// Compares the remote manifest to the local state. Only installed sets are
/// refreshed; other sets are downloaded on demand when the player opens them.
pub fn plan_update(local: &LocalState, remote: &Manifest) -> UpdatePlan {
    let sets_to_fetch = local
        .sets
        .iter()
        .filter(|(code, version)| {
            remote.sets.get(*code).is_some_and(|r| &r.source_version != *version)
        })
        .map(|(code, _)| code.clone())
        .collect();
    let fetch_prices = local.prices_updated_at.as_deref() != Some(remote.prices_updated_at.as_str());
    UpdatePlan { sets_to_fetch, fetch_prices }
}

/// Whether the local prices are old enough to warn the player.
pub fn prices_are_stale(local: &LocalState, now: i64, cfg: &UpdateConfig) -> bool {
    match local.prices_fetched_at {
        Some(t) => now - t > cfg.stale_prices_secs,
        None => true,
    }
}
