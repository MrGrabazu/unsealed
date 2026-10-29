//! The game's little secrets, found by the curious (the interface spots most of them, see
//! `eggs.ts`; the one of Magic's birthday is spotted here). Each one unlocks a secret
//! achievement (optional: the ultimate one never asks for them), and the oldest code of all
//! brings a card. Never described where a player reads: no wiki, changelog nor commit message.

use super::Game;
use serde::Serialize;
use unsealed_core::achievements::Event;

/// The secrets the interface may report.
const EGGS: [&str; 7] = ["konami", "seal", "speck", "lotus", "insomniac", "melody", "pages"];

/// Whether `egg` is one of them.
pub fn is_egg(egg: &str) -> bool {
    EGGS.contains(&egg)
}

/// The card the code brings, once per game, in foil: a cheat for a cheat code.
const KONAMI_CARD: (&str, &str) = ("UNH", "a4f6b074-48d6-57eb-92a1-becb2fb7ab8b");
/// Key of `save_meta`: the code's card was given.
const KONAMI_GIVEN: &str = "egg_konami_card";

/// What a secret found brings besides its achievement.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EggFound {
    /// A card given (in the card language), if any.
    pub card: Option<String>,
}

impl Game {
    /// A secret found: its achievement (the event `EasterEgg`), and the code's card the first
    /// time (none when the data lacks its set).
    pub fn found_egg(&mut self, egg: &str) -> Result<EggFound, String> {
        if !is_egg(egg) {
            return Err(format!("unknown secret `{egg}`"));
        }
        let mut found = EggFound::default();
        if egg == "konami" && self.store.meta(KONAMI_GIVEN).map_err(super::err)?.is_none() {
            found.card = self.give_konami_card()?;
        }
        self.emit(vec![Event::new("EasterEgg").with("egg", egg)]);
        Ok(found)
    }

    fn give_konami_card(&mut self) -> Result<Option<String>, String> {
        let (set_code, uuid) = KONAMI_CARD;
        if self.ensure_loaded(set_code).is_err() {
            return Ok(None);
        }
        let set = &self.sets[set_code];
        let Some(&i) = set.index.get(uuid) else { return Ok(None) };
        let card = &set.file.cards[i];
        let name = card.foreign.get(&self.card_lang).map_or(&card.name, |f| &f.name).clone();
        let card_set = card.set_code.clone();
        let now = (self.clock)().now;
        self.store.give_card(uuid, &card_set, true, now).map_err(super::err)?;
        self.store.set_meta(KONAMI_GIVEN, "1").map_err(super::err)?;
        Ok(Some(name))
    }

    /// Magic's birthday (Alpha came out on August 5, 1993): an Alpha booster opened that day.
    pub(super) fn birthday_egg(&self, set_code: &str) -> Option<Event> {
        let day = (self.clock)().day;
        (set_code == "LEA" && day.ends_with("-08-05")).then(|| Event::new("EasterEgg").with("egg", "birthday"))
    }
}
