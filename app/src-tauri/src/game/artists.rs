//! Collections by artist: every card an artist illustrated, across the sets offered (the
//! index `artists.json` is built with the data).

use super::{err, number_key, read_json, CollectionCard, Game};
use crate::store::Owned;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use unsealed_core::catalog::{ArtistIndex, ARTISTS_FILE};

/// An artist in the list: how many of their cards the game offers, how many are owned.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistSummary {
    pub name: String,
    pub card_count: usize,
    pub owned_count: usize,
}

/// One of an artist's cards, with the set it comes from (to show its details).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistCardView {
    #[serde(flatten)]
    pub card: CollectionCard,
    /// The set file holding the card.
    pub set_code: String,
    pub set_name: String,
    pub release_date: String,
}

/// An artist's collection, oldest card first.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtistView {
    pub name: String,
    pub owned_count: usize,
    pub total_count: usize,
    pub completion_percent: f64,
    pub cards: Vec<ArtistCardView>,
}

impl Game {
    /// The artist index, read once (and again after a data refresh); empty without data.
    fn artist_index(&mut self) -> &ArtistIndex {
        if self.artists.is_none() {
            let index = read_json(&self.data_dir.join(ARTISTS_FILE)).unwrap_or_default();
            self.artists = Some(index);
        }
        self.artists.as_ref().expect("just loaded")
    }

    /// Copies owned of every card, by uuid.
    fn owned_by_uuid(&self) -> Result<HashMap<String, Owned>, String> {
        let mut owned: HashMap<String, Owned> = HashMap::new();
        for (uuid, foil, quantity) in self.store.owned_cards().map_err(err)? {
            let have = owned.entry(uuid).or_default();
            if foil {
                have.foil += quantity;
            } else {
                have.normal += quantity;
            }
        }
        Ok(owned)
    }

    /// Codes of the sets offered today (released ones).
    fn offered_sets(&self) -> HashSet<String> {
        self.list_sets().unwrap_or_default().into_iter().map(|s| s.code).collect()
    }

    /// Every artist with cards in the sets offered, by name.
    pub fn artists(&mut self) -> Result<Vec<ArtistSummary>, String> {
        let offered = self.offered_sets();
        let owned = self.owned_by_uuid()?;
        let index = self.artist_index();
        Ok(index
            .artists
            .iter()
            .filter_map(|(name, cards)| {
                let cards: Vec<_> = cards.iter().filter(|c| offered.contains(&c.set)).collect();
                (!cards.is_empty()).then(|| ArtistSummary {
                    name: name.clone(),
                    card_count: cards.len(),
                    owned_count: cards.iter().filter(|c| owned.get(&c.uuid).is_some_and(|o| o.normal + o.foil > 0)).count(),
                })
            })
            .collect())
    }

    /// Every card owned, across the sets, with the set it comes from (to sort and sell them
    /// in one list). A card is found in the file of the set whose booster gave it, else in its
    /// own set's, else in its parent set's ("TLTR", "ALTR": "LTR").
    pub fn owned_everywhere(&mut self) -> Result<Vec<ArtistCardView>, String> {
        self.ensure_prices();
        let sources = self.store.owned_card_sources().map_err(err)?;
        let owned = self.owned_by_uuid()?;
        let mut cards = Vec::new();
        for (uuid, card_set, booster_set) in sources {
            let parent = card_set.get(1..).map(str::to_string);
            let file = [booster_set, Some(card_set), parent]
                .into_iter()
                .flatten()
                .find(|code| self.ensure_loaded(code).is_ok() && self.sets[code].index.contains_key(&uuid));
            let Some(file) = file else { continue };
            let set = &self.sets[&file];
            let card = &set.file.cards[set.index[&uuid]];
            cards.push(ArtistCardView {
                card: self.collection_card(card, owned.get(&uuid).copied().unwrap_or_default()),
                set_code: file.clone(),
                set_name: set.file.names.get(&self.card_lang).unwrap_or(&set.file.name).clone(),
                release_date: set.file.release_date.clone(),
            });
        }
        Ok(cards)
    }

    /// The collection of one artist: their cards in the sets offered, oldest first.
    pub fn artist_collection(&mut self, name: &str) -> Result<ArtistView, String> {
        self.ensure_prices();
        let offered = self.offered_sets();
        let entries: Vec<_> = self
            .artist_index()
            .artists
            .get(name)
            .ok_or_else(|| format!("unknown artist {name}"))?
            .iter()
            .filter(|c| offered.contains(&c.set))
            .cloned()
            .collect();
        for set in entries.iter().map(|c| c.set.clone()).collect::<HashSet<_>>() {
            self.ensure_loaded(&set)?;
        }
        let owned = self.owned_by_uuid()?;
        let mut cards: Vec<ArtistCardView> = entries
            .iter()
            .filter_map(|entry| {
                let set = self.sets.get(&entry.set)?;
                let card = &set.file.cards[*set.index.get(&entry.uuid)?];
                Some(ArtistCardView {
                    card: self.collection_card(card, owned.get(&card.uuid).copied().unwrap_or_default()),
                    set_code: entry.set.clone(),
                    set_name: set.file.names.get(&self.card_lang).unwrap_or(&set.file.name).clone(),
                    release_date: set.file.release_date.clone(),
                })
            })
            .collect();
        cards.sort_by(|a, b| {
            (a.release_date.as_str(), &a.set_code, number_key(&a.card.number))
                .cmp(&(b.release_date.as_str(), &b.set_code, number_key(&b.card.number)))
        });
        let total_count = cards.len();
        let owned_count = cards.iter().filter(|c| c.card.quantity + c.card.foil_quantity > 0).count();
        Ok(ArtistView {
            name: name.to_string(),
            owned_count,
            total_count,
            completion_percent: if total_count == 0 { 0.0 } else { owned_count as f64 * 100.0 / total_count as f64 },
            cards,
        })
    }
}
