//! Decides at launch whether the local data must be rebuilt. The client builds its own
//! data from MTGJSON and Scryfall (see `builder`), so the only state is the local manifest.
//! Pure logic: no network, no clock access, so it is testable.

use crate::catalog::{Manifest, SetSelection, DATA_FORMAT_VERSION};
pub use crate::import::ArtCardOdds;
use serde::Deserialize;

/// What to download and how fresh it must be (shipped as `config/data.json`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataConfig {
    /// Set codes the game offers, or `"all"`.
    pub sets: SetSelection,
    /// With `"all"`: the MTGJSON set types kept (online-only sets are always left out).
    #[serde(default = "default_set_types")]
    pub set_types: Vec<String>,
    /// Booster kinds offered; each set keeps those it has.
    #[serde(default = "default_boosters")]
    pub boosters: Vec<String>,
    /// Data older than this is rebuilt at launch. Prices move daily, so a day is the natural unit.
    #[serde(default = "default_max_age_hours")]
    pub max_age_hours: i64,
    /// Boosters simulated per kept config to validate its rates.
    #[serde(default = "default_sim_boosters")]
    pub sim_boosters: u64,
    /// Every booster gets a token; outside collector boosters (always foil), one in this
    /// many is foil.
    #[serde(default = "default_token_foil_one_in")]
    pub token_foil_one_in: u64,
    /// How often boosters hold an art card.
    #[serde(default)]
    pub art_cards: ArtCardOdds,
    /// Where the ready-made data is published (`pack`): the address its `manifest.json` and
    /// its pack are found under. None: the game always builds its data itself.
    #[serde(default)]
    pub pack_url: Option<String>,
}

pub fn default_token_foil_one_in() -> u64 {
    15
}

fn default_boosters() -> Vec<String> {
    // "default" is the booster of the oldest sets (before draft boosters existed).
    ["play", "draft", "set", "default", "collector"].map(String::from).to_vec()
}

fn default_set_types() -> Vec<String> {
    ["core", "expansion", "masters", "draft_innovation", "funny", "starter"].map(String::from).to_vec()
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
    /// The configuration asks for other sets, set types or booster kinds than the data was built for.
    ConfigChanged,
    /// The data was built in an older format (see `DATA_FORMAT_VERSION`).
    FormatChanged,
    /// The data is older than the configured maximum age.
    Stale,
}

/// Whether a published data pack, described by its manifest, is worth installing: of this
/// version's format, built for the same sets, set types and booster kinds, and newer than the
/// local data (or there is none, or it is of another format).
pub fn pack_is_of_use(remote: &Manifest, local: Option<&Manifest>, cfg: &DataConfig, now: i64) -> bool {
    let compatible = matches!(refresh_reason(Some(remote), cfg, now), None | Some(RefreshReason::Stale));
    let newer = match local {
        Some(local) if local.format_version == DATA_FORMAT_VERSION => {
            let at = |m: &Manifest| m.generated_at.parse::<i64>().unwrap_or(0);
            at(remote) > at(local)
        }
        _ => true,
    };
    compatible && newer
}

/// Whether the data must be rebuilt now. `now` is a Unix time in seconds.
pub fn refresh_reason(local: Option<&Manifest>, cfg: &DataConfig, now: i64) -> Option<RefreshReason> {
    let Some(manifest) = local else { return Some(RefreshReason::NoData) };
    if manifest.format_version != DATA_FORMAT_VERSION {
        return Some(RefreshReason::FormatChanged);
    }
    if let SetSelection::List(codes) = &cfg.sets {
        let installed = |c: &String| manifest.sets.contains_key(c) || manifest.skipped.contains_key(c);
        if let Some(code) = codes.iter().find(|c| !installed(c)) {
            return Some(RefreshReason::MissingSet(code.clone()));
        }
    }
    let types_matter = matches!(cfg.sets, SetSelection::All(_));
    if manifest.booster_kinds != cfg.boosters
        || manifest.requested_sets.as_ref() != Some(&cfg.sets)
        || (types_matter && manifest.set_types != cfg.set_types)
    {
        return Some(RefreshReason::ConfigChanged);
    }
    // An unreadable timestamp or a clock set in the past counts as stale: rebuilding is harmless.
    let age = manifest.generated_at.parse::<i64>().ok().map(|t| now - t);
    match age {
        Some(secs) if (0..cfg.max_age_hours * 3600).contains(&secs) => None,
        _ => Some(RefreshReason::Stale),
    }
}
