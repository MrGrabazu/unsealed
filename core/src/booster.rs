//! MTGJSON `BoosterConfig` schema (v5.2.2+), checked against
//! https://mtgjson.com/data-models/booster/booster-config/ (+ booster-pack, booster-sheet).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("booster `{0}` not found in the set file")]
    UnknownBooster(String),
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoosterConfig {
    pub boosters: Vec<BoosterPack>,
    pub boosters_total_weight: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub sheets: BTreeMap<String, BoosterSheet>,
    #[serde(default)]
    pub source_set_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoosterPack {
    /// slot (sheet name) -> number of cards drawn from that sheet
    pub contents: BTreeMap<String, u32>,
    pub weight: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoosterSheet {
    #[serde(default)]
    pub allow_duplicates: bool,
    #[serde(default)]
    pub balance_colors: bool,
    /// uuid -> weight. BTreeMap: stable ordering, required for reproducible draws.
    pub cards: BTreeMap<String, u64>,
    #[serde(default)]
    pub fixed: bool,
    pub foil: bool,
    pub total_weight: u64,
}

impl BoosterConfig {
    /// Extracts `data.booster.<kind>` from an MTGJSON set file (e.g. "play", "draft").
    pub fn from_set_json(json: &str, kind: &str) -> Result<Self, ConfigError> {
        #[derive(Deserialize)]
        struct SetFile {
            data: SetData,
        }
        #[derive(Deserialize)]
        struct SetData {
            #[serde(default)]
            booster: BTreeMap<String, BoosterConfig>,
        }
        let file: SetFile = serde_json::from_str(json)?;
        let cfg = file
            .data
            .booster
            .into_iter()
            .find(|(k, _)| k == kind)
            .map(|(_, v)| v)
            .ok_or_else(|| ConfigError::UnknownBooster(kind.to_string()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Checks internal consistency (totals, referenced sheets).
    pub fn validate(&self) -> Result<(), ConfigError> {
        let sum: u64 = self.boosters.iter().map(|b| b.weight).sum();
        if sum != self.boosters_total_weight {
            return Err(ConfigError::Invalid(format!(
                "boostersTotalWeight={} but weights sum to {}",
                self.boosters_total_weight, sum
            )));
        }
        for (name, s) in &self.sheets {
            let sum: u64 = s.cards.values().sum();
            if sum != s.total_weight {
                return Err(ConfigError::Invalid(format!(
                    "sheet `{name}`: totalWeight={} but weights sum to {sum}",
                    s.total_weight
                )));
            }
        }
        for pack in &self.boosters {
            for (slot, &n) in &pack.contents {
                let sheet = self.sheets.get(slot).ok_or_else(|| {
                    ConfigError::Invalid(format!("slot `{slot}` has no matching sheet"))
                })?;
                if !sheet.fixed && !sheet.allow_duplicates && n as usize > sheet.cards.len() {
                    return Err(ConfigError::Invalid(format!(
                        "slot `{slot}`: {n} cards requested but only {} available",
                        sheet.cards.len()
                    )));
                }
            }
        }
        Ok(())
    }
}
