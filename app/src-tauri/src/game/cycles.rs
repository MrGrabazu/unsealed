//! A set's cycles as the player stands (see `unsealed_core::cycles`): each slot filled by
//! the best copy owned of its printings, the more premium the printing the better, a foil
//! better than its non-foil; and the events that achievements follow.

use super::{CollectionCard, Game, Owned};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use unsealed_core::achievements::Event;
use unsealed_core::catalog::CardEntry;
use unsealed_core::cycles::{Cycle, Cycles};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CyclesView {
    pub set_code: String,
    /// Cycles complete, of all the set's.
    pub completed: u32,
    pub total: u32,
    /// Slots filled, of all the cycles'.
    pub slots_owned: u32,
    pub slots_total: u32,
    pub cycles: Vec<CycleView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CycleView {
    pub id: String,
    pub name: String,
    /// One of ours, not the MTG Wiki's.
    pub custom: bool,
    pub owned: u32,
    pub total: u32,
    pub complete: bool,
    /// Every slot in its most premium printing (in foil, when it exists in foil).
    pub prestige: bool,
    pub slots: Vec<CycleSlot>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CycleSlot {
    /// The printing shown: the best one owned, else the first (the regular one).
    pub card: CollectionCard,
    pub owned: bool,
    /// The copy shown is a foil.
    pub foil: bool,
    /// Owned in its most premium printing and finish.
    pub prestige: bool,
    /// How many printings the slot has, the regular one included.
    pub printings: u32,
}

/// The state of one cycle: what fills each slot, and whether it is complete or prestige.
struct CycleState<'a> {
    /// For each slot: the card shown, the copy owned (and whether it is foil), and whether
    /// it is the slot's best possible.
    slots: Vec<(&'a CardEntry, Option<bool>, bool)>,
}

impl CycleState<'_> {
    fn owned(&self) -> u32 {
        self.slots.iter().filter(|s| s.1.is_some()).count() as u32
    }
    fn complete(&self) -> bool {
        self.slots.iter().all(|s| s.1.is_some())
    }
    fn prestige(&self) -> bool {
        self.slots.iter().all(|s| s.2)
    }
}

impl Game {
    /// The cycles every set may have (shared by the games of a session).
    pub fn set_cycles(&mut self, cycles: Arc<Cycles>) {
        self.cycles = Some(cycles);
    }

    /// The cycles of a set, as far as the player is, their names in `lang`.
    pub fn cycles(&mut self, set_code: &str, lang: &str) -> Result<CyclesView, String> {
        self.ensure_loaded(set_code)?;
        self.ensure_prices();
        let owned = self.owned_with_tokens(set_code)?;
        let cycles = self.cycles.clone().unwrap_or_default();
        let mut view = CyclesView { set_code: set_code.to_string(), completed: 0, total: 0, slots_owned: 0, slots_total: 0, cycles: Vec::new() };
        for cycle in cycles.of_set(set_code) {
            let Some(state) = self.cycle_state(set_code, cycle, &owned) else { continue };
            let slots: Vec<CycleSlot> = state
                .slots
                .iter()
                .zip(&cycle.slots)
                .map(|(&(card, have, best), printings)| CycleSlot {
                    card: self.collection_card(card, owned.get(&card.uuid).copied().unwrap_or_default()),
                    owned: have.is_some(),
                    foil: have == Some(true),
                    prestige: best,
                    printings: printings.len() as u32,
                })
                .collect();
            let (have, total) = (state.owned(), slots.len() as u32);
            view.total += 1;
            view.completed += u32::from(state.complete());
            view.slots_owned += have;
            view.slots_total += total;
            view.cycles.push(CycleView {
                id: cycle.id.clone(),
                name: cycle.name(lang).to_string(),
                custom: cycle.custom,
                owned: have,
                total,
                complete: state.complete(),
                prestige: state.prestige(),
                slots,
            });
        }
        Ok(view)
    }

    /// What fills a cycle's slots: of each slot's printings found in the set's file, the best
    /// copy owned (a later printing beats an earlier one, a foil its non-foil); none when no
    /// printing of a slot is in the file (data built before the cycle).
    fn cycle_state<'a>(&'a self, set_code: &str, cycle: &Cycle, owned: &HashMap<String, Owned>) -> Option<CycleState<'a>> {
        let set = self.sets.get(set_code)?;
        // Tokens may bear the set's own code and the same numbers as its cards (FRA, LTR):
        // a cycle's numbers are those of the cards.
        let by_number: HashMap<&str, &CardEntry> = set
            .file
            .cards
            .iter()
            .filter(|c| c.set_code == set_code && !c.token && !c.art)
            .map(|c| (c.number.as_str(), c))
            .collect();
        let mut slots = Vec::with_capacity(cycle.slots.len());
        for numbers in &cycle.slots {
            let printings: Vec<&CardEntry> = numbers.iter().filter_map(|n| by_number.get(n.as_str()).copied()).collect();
            let first = *printings.first()?;
            // Rank of a copy: its printing's place, then foil above non-foil.
            let mut best: Option<(usize, &CardEntry, bool)> = None;
            for (i, card) in printings.iter().enumerate() {
                let have = owned.get(&card.uuid).copied().unwrap_or_default();
                for (foil, count) in [(false, have.normal), (true, have.foil)] {
                    let rank = i * 2 + usize::from(foil);
                    if count > 0 && best.is_none_or(|(r, _, _)| rank > r) {
                        best = Some((rank, card, foil));
                    }
                }
            }
            // The best possible: the last printing, in foil if it is printed in foil.
            let last = *printings.last()?;
            let top = (printings.len() - 1) * 2 + usize::from(last.finishes.iter().any(|f| f == "foil"));
            slots.push(match best {
                Some((rank, card, foil)) => (card, Some(foil), rank >= top),
                None => (first, None, false),
            });
        }
        Some(CycleState { slots })
    }

    /// The events of a set's cycles, for the achievements: one `CycleCompleted` per complete
    /// cycle (achievements count them once by their key), then the set's `CyclesProgress`.
    pub(super) fn cycle_events(&self, set_code: &str) -> Vec<Event> {
        let Some(cycles) = self.cycles.clone() else { return Vec::new() };
        let list = cycles.of_set(set_code);
        if list.is_empty() || !self.sets.contains_key(set_code) {
            return Vec::new();
        }
        let Ok(owned) = self.owned_with_tokens(set_code) else { return Vec::new() };
        let released_year = self
            .release_dates
            .as_ref()
            .and_then(|d| d.get(set_code))
            .and_then(|d| d.get(0..4))
            .and_then(|y| y.parse::<i64>().ok())
            .unwrap_or(0);
        let mut events = Vec::new();
        let (mut complete, mut total) = (0i64, 0i64);
        for cycle in list {
            let Some(state) = self.cycle_state(set_code, cycle, &owned) else { continue };
            total += 1;
            if !state.complete() {
                continue;
            }
            complete += 1;
            let cards: Vec<&CardEntry> = state.slots.iter().map(|s| s.0).collect();
            events.push(
                Event::new("CycleCompleted")
                    .with("cycle", format!("{set_code}/{}", cycle.id))
                    .with("setCode", set_code)
                    .with("custom", cycle.custom)
                    .with("slots", cards.len() as i64)
                    .with("prestige", state.prestige())
                    .with("releasedYear", released_year)
                    .with("allMythic", cards.iter().all(|c| c.rarity == "mythic"))
                    .with("allLands", cards.iter().all(|c| c.types.iter().any(|t| t == "land"))),
            );
        }
        if total > 0 {
            events.push(
                Event::new("CyclesProgress")
                    .with("setCode", set_code)
                    .with("completed", complete)
                    .with("total", total)
                    .with("allComplete", complete == total),
            );
        }
        events
    }
}
