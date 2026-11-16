//! How the art cards are turned to be seen: neither MTGJSON nor Scryfall says it, and their
//! images all stand upright, a landscape illustration lying on its side, some even upside down.
//! `config/art-orientation.json` is kept by hand (`.cache/art-orient-tool.js`): for each art card
//! ("ATLA/50": its own set code and number), "up", "right" (a quarter turn clockwise to be
//! seen), "left" (anticlockwise) or "flip" (half a turn). A card not listed stands upright.

use serde::Deserialize;
use std::collections::BTreeMap;

/// The turns a card can need, as the card details name them (`reading`).
pub const TURNS: [&str; 4] = ["up", "right", "left", "flip"];

#[derive(Debug, Default, Deserialize)]
pub struct ArtTurns {
    #[serde(default)]
    cards: BTreeMap<String, String>,
}

impl ArtTurns {
    pub fn from_json(json: &str) -> Result<Self, String> {
        let turns: ArtTurns = serde_json::from_str(json).map_err(|e| format!("art-orientation.json: {e}"))?;
        for (key, turn) in &turns.cards {
            if !TURNS.contains(&turn.as_str()) {
                return Err(format!("art-orientation.json: {key}: unknown turn \"{turn}\""));
            }
            if key.split_once('/').is_none_or(|(set, number)| set.is_empty() || number.is_empty()) {
                return Err(format!("art-orientation.json: \"{key}\" is not SET/number"));
            }
        }
        Ok(turns)
    }

    /// How the card is turned to be seen, `None` when it stands upright (or is not listed).
    pub fn turn(&self, set_code: &str, number: &str) -> Option<&'static str> {
        // A card's own version with a gold signature ("12s", Scryfall has no image of it) is
        // turned as its card.
        let turn = self.cards.get(&format!("{set_code}/{number}")).or_else(|| {
            let unsigned = number.strip_suffix('s')?;
            self.cards.get(&format!("{set_code}/{unsigned}"))
        })?;
        TURNS.iter().copied().find(|t| t == turn).filter(|t| *t != "up")
    }

    /// The cards listed, with their turn ("up" included).
    pub fn cards(&self) -> impl Iterator<Item = (&str, &str)> {
        self.cards.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}
