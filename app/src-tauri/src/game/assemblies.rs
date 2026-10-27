//! A set's assemblies (Unstable): an augment card laid over a host card, their metal seams
//! lined up, make one creature ("Half-Kitten, Half-" over "Ordinary Pony"). Every augment goes
//! with every host; a creature is assembled once both cards are owned (in either finish).
//! Nothing to configure: augments and hosts are told by their layout.

use super::{CollectionCard, Game};
use serde::Serialize;
use unsealed_core::achievements::Event;
use unsealed_core::catalog::CardEntry;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssembliesView {
    pub set_code: String,
    /// The augments, then the hosts, in collector number order.
    pub augments: Vec<AssemblyPart>,
    pub hosts: Vec<AssemblyPart>,
    /// Creatures assembled (augment and host both owned), of all `augments × hosts`.
    pub assembled: u32,
    pub total: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssemblyPart {
    pub card: CollectionCard,
    pub owned: bool,
    /// A foil copy is owned (shown with its sheen).
    pub foil: bool,
}

/// The set's own augments and hosts (no token, no art card), in collector number order.
fn parts<'a>(cards: &'a [CardEntry], set_code: &str, layout: &str) -> Vec<&'a CardEntry> {
    let mut list: Vec<&CardEntry> =
        cards.iter().filter(|c| c.set_code == set_code && c.layout == layout && !c.token && !c.art).collect();
    let number = |c: &CardEntry| c.number.trim_end_matches(|ch: char| !ch.is_ascii_digit()).parse::<u32>().unwrap_or(u32::MAX);
    list.sort_by(|a, b| number(a).cmp(&number(b)).then_with(|| a.number.cmp(&b.number)));
    list
}

impl Game {
    /// The set's assemblies, as far as the player is (none for a set without augments or hosts).
    pub fn assemblies(&mut self, set_code: &str) -> Result<AssembliesView, String> {
        self.ensure_loaded(set_code)?;
        self.ensure_prices();
        let owned = self.owned_with_tokens(set_code)?;
        let set = &self.sets[set_code];
        let view_of = |cards: Vec<&CardEntry>| -> Vec<AssemblyPart> {
            cards
                .into_iter()
                .map(|c| {
                    let have = owned.get(&c.uuid).copied().unwrap_or_default();
                    AssemblyPart { card: self.collection_card(c, have), owned: have.normal + have.foil > 0, foil: have.foil > 0 }
                })
                .collect()
        };
        let augments = view_of(parts(&set.file.cards, set_code, "augment"));
        let hosts = view_of(parts(&set.file.cards, set_code, "host"));
        let count = |list: &[AssemblyPart]| list.iter().filter(|p| p.owned).count() as u32;
        Ok(AssembliesView {
            set_code: set_code.to_string(),
            assembled: count(&augments) * count(&hosts),
            total: (augments.len() * hosts.len()) as u32,
            augments,
            hosts,
        })
    }

    /// For the achievements: the set's `AssembliesProgress` (creatures assembled, of all), when
    /// it has augments and hosts.
    pub(super) fn assembly_events(&self, set_code: &str) -> Vec<Event> {
        let Some(set) = self.sets.get(set_code) else { return Vec::new() };
        let augments = parts(&set.file.cards, set_code, "augment");
        let hosts = parts(&set.file.cards, set_code, "host");
        if augments.is_empty() || hosts.is_empty() {
            return Vec::new();
        }
        let Ok(owned) = self.owned_with_tokens(set_code) else { return Vec::new() };
        let has = |list: &[&CardEntry]| list.iter().filter(|c| owned.get(&c.uuid).is_some_and(|o| o.normal + o.foil > 0)).count() as i64;
        let assembled = has(&augments) * has(&hosts);
        let total = (augments.len() * hosts.len()) as i64;
        vec![Event::new("AssembliesProgress")
            .with("setCode", set_code)
            .with("assembled", assembled)
            .with("total", total)
            .with("allAssembled", assembled == total)]
    }
}
