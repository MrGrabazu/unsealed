//! Cards of a limited print run (`config/limited.json`): serialized printings exist in a few
//! hundred copies, LTR's The One Ring #0 in a single one. In a game, no more copies of such a
//! card can ever be pulled than were printed, whatever happens to them afterwards (sold,
//! sale cancelled...). MTGJSON marks serialized printings but gives no print run, so runs are
//! configured: per card, else a default for every serialized printing.

use crate::booster::ConfigError;
use crate::catalog::CardEntry;
use serde::Deserialize;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitedConfig {
    /// Print run of a serialized printing not listed in `cards`; `None` leaves them unlimited.
    #[serde(default)]
    pub serialized_print_run: Option<u32>,
    #[serde(default)]
    pub cards: Vec<LimitedCard>,
    /// Applied to the estimated price of a limited card (estimated from its ordinary
    /// printings: see `PriceSet::estimated`): the first step whose `printRunUpTo` covers its
    /// print run. A card with a market price of its own keeps it.
    #[serde(default)]
    pub estimate_multipliers: Vec<MultiplierStep>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiplierStep {
    pub print_run_up_to: u32,
    pub multiplier: u32,
}

/// A card of a limited print run, by set and collector number.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitedCard {
    pub set: String,
    pub number: String,
    /// For whoever edits the file; not used.
    #[serde(default)]
    pub name: Option<String>,
    pub print_run: u32,
}

impl LimitedConfig {
    pub fn from_json(json: &str) -> Result<Self, ConfigError> {
        let cfg: Self = serde_json::from_str(json)?;
        if cfg.serialized_print_run == Some(0) || cfg.cards.iter().any(|c| c.print_run == 0) {
            return Err(ConfigError::Invalid("limited: print runs must be at least 1".into()));
        }
        let steps = &cfg.estimate_multipliers;
        if steps.windows(2).any(|w| w[0].print_run_up_to >= w[1].print_run_up_to) || steps.iter().any(|s| s.multiplier == 0) {
            return Err(ConfigError::Invalid(
                "limited: estimateMultipliers need increasing printRunUpTo and positive multipliers".into(),
            ));
        }
        Ok(cfg)
    }

    /// What the estimated price of a card printed `print_run` times is multiplied by.
    pub fn estimate_multiplier(&self, print_run: u32) -> u32 {
        self.estimate_multipliers.iter().find(|s| print_run <= s.print_run_up_to).map_or(1, |s| s.multiplier)
    }

    /// How many copies of a card exist: its own entry, else the default of serialized
    /// printings; `None` when unlimited.
    pub fn print_run_of(&self, card: &CardEntry) -> Option<u32> {
        let listed = self
            .cards
            .iter()
            .find(|c| c.set.eq_ignore_ascii_case(&card.set_code) && c.number == card.number);
        match listed {
            Some(c) => Some(c.print_run),
            None if card.serialized => self.serialized_print_run,
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(set: &str, number: &str, serialized: bool) -> CardEntry {
        CardEntry {
            uuid: format!("{set}-{number}"),
            set_code: set.into(),
            scryfall_id: String::new(),
            name: String::new(),
            number: number.into(),
            rarity: "mythic".into(),
            colors: vec![],
            finishes: vec!["nonfoil".into()],
            layout: "normal".into(),
            side: None,
            artist: None,
            foreign: Default::default(),
            serialized,
            token: false,
            art: false,
            foil_scryfall_id: None,
            full_art: false,
            special_frame: false,
            types: vec![],
            color_identity: vec![],
            foil_kind: None,
            drop: None,
        }
    }

    #[test]
    fn a_listed_card_wins_over_the_serialized_default() {
        let cfg = LimitedConfig::from_json(
            r#"{ "serializedPrintRun": 500, "cards": [ { "set": "LTR", "number": "0", "printRun": 1 } ] }"#,
        )
        .unwrap();
        assert_eq!(cfg.print_run_of(&card("LTR", "0", true)), Some(1));
        assert_eq!(cfg.print_run_of(&card("ltr", "748z", true)), Some(500));
        assert_eq!(cfg.print_run_of(&card("LTR", "246", false)), None);
        assert_eq!(LimitedConfig::default().print_run_of(&card("LTR", "748z", true)), None);
    }

    #[test]
    fn the_multiplier_follows_the_print_run() {
        let cfg = LimitedConfig::from_json(
            r#"{ "estimateMultipliers": [ { "printRunUpTo": 1, "multiplier": 10000 },
                                          { "printRunUpTo": 500, "multiplier": 20 } ] }"#,
        )
        .unwrap();
        assert_eq!(cfg.estimate_multiplier(1), 10_000);
        assert_eq!(cfg.estimate_multiplier(2), 20);
        assert_eq!(cfg.estimate_multiplier(500), 20);
        assert_eq!(cfg.estimate_multiplier(501), 1);
        let unordered = r#"{ "estimateMultipliers": [ { "printRunUpTo": 500, "multiplier": 20 },
                                                      { "printRunUpTo": 1, "multiplier": 10000 } ] }"#;
        assert!(LimitedConfig::from_json(unordered).is_err());
    }

    #[test]
    fn a_print_run_of_zero_is_refused() {
        assert!(LimitedConfig::from_json(r#"{ "cards": [ { "set": "X", "number": "1", "printRun": 0 } ] }"#).is_err());
        assert!(LimitedConfig::from_json(r#"{ "serializedPrintRun": 0 }"#).is_err());
    }
}
