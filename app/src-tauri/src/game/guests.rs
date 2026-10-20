//! Guest series: the cards of other sets that boosters give (Special Guests, The List,
//! Masterpieces, a commander set's cards in collector boosters…). Each series has a binder
//! of its own, made of its cards from every set offered (the index `guests.json` is built
//! with the data). A series that is itself a set offered (Theros cards in Born of the Gods
//! boosters) shows them in its binder instead, apart ("elsewhere").

use super::{err, number_key, read_json, CollectionCard, CollectionView, Game, SetSummary};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap, HashSet};
use unsealed_core::catalog::{GuestCard, GuestIndex, GuestSeries, GUESTS_FILE};

/// In a set's completion panel: a guest series its boosters give, and how many of those
/// cards are owned. Its binder opens under `code`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestLink {
    pub code: String,
    pub name: String,
    pub owned: usize,
    pub total: usize,
}

impl Game {
    /// The guest series index, read once (and again after a data refresh); empty without data.
    fn guest_index(&mut self) -> &GuestIndex {
        if self.guests.is_none() {
            let index = read_json(&self.data_dir.join(GUESTS_FILE)).unwrap_or_default();
            self.guests = Some(index);
        }
        self.guests.as_ref().expect("just loaded")
    }

    /// A series' cards given by the boosters of sets offered today (each keeping only those).
    fn offered_series(&mut self, code: &str, offered: &HashSet<String>) -> Option<(GuestSeries, Vec<GuestCard>)> {
        let series = self.guest_index().series.get(code)?.clone();
        let cards: Vec<GuestCard> = series
            .cards
            .iter()
            .map(|c| GuestCard { uuid: c.uuid.clone(), boosters: c.boosters.iter().filter(|b| offered.contains(*b)).cloned().collect() })
            .filter(|c| !c.boosters.is_empty())
            .collect();
        (!cards.is_empty()).then_some((series, cards))
    }

    /// Whether `code` is a guest series with a binder of its own: not a set offered, and
    /// given by the boosters of one.
    pub(super) fn is_guest_series(&mut self, code: &str) -> bool {
        let offered = self.offered_sets();
        !offered.contains(code) && self.offered_series(code, &offered).is_some()
    }

    /// The guest series with a binder of their own, newest first (as `list_sets`, without
    /// boosters nor art).
    pub fn guest_series(&mut self) -> Vec<SetSummary> {
        let offered = self.offered_sets();
        let codes: Vec<String> = self.guest_index().series.keys().filter(|c| !offered.contains(*c)).cloned().collect();
        let mut list: Vec<SetSummary> = codes
            .iter()
            .filter_map(|code| {
                let (series, _) = self.offered_series(code, &offered)?;
                Some(SetSummary {
                    code: code.clone(),
                    name: series.names.get(&self.card_lang).unwrap_or(&series.name).clone(),
                    english_name: series.name,
                    release_date: series.release_date,
                    boosters: Vec::new(),
                    art: None,
                    icon_url: series.icon_svg_uri,
                })
            })
            .collect();
        list.sort_by(|a, b| b.release_date.cmp(&a.release_date));
        list
    }

    /// The cards of `cards`, as the collection shows them, read from the file of the first
    /// set whose boosters give each, with the sets that give it; in collector number order.
    /// Owned copies are those filed under `owner` (the cards' own set code).
    fn cards_from_boosters(&mut self, cards: &[GuestCard], owner: &str) -> Result<Vec<CollectionCard>, String> {
        for file in cards.iter().map(|c| c.boosters[0].clone()).collect::<BTreeSet<_>>() {
            self.ensure_loaded(&file)?;
        }
        let owned = self.store.owned_in_set(owner).map_err(err)?;
        let mut list: Vec<((u32, String), CollectionCard)> = cards
            .iter()
            .filter_map(|g| {
                let set = self.sets.get(&g.boosters[0])?;
                let entry = &set.file.cards[*set.index.get(&g.uuid)?];
                let mut card = self.collection_card(entry, owned.get(&g.uuid).copied().unwrap_or_default());
                card.boosters = g.boosters.clone();
                Some((number_key(&entry.number), card))
            })
            .collect();
        list.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(list.into_iter().map(|(_, card)| card).collect())
    }

    /// The binder of a guest series: its cards from every set offered.
    pub(super) fn series_collection(&mut self, code: &str) -> Result<CollectionView, String> {
        self.ensure_prices();
        let offered = self.offered_sets();
        let (series, cards) = self.offered_series(code, &offered).ok_or_else(|| format!("unknown series {code}"))?;
        let cards = self.cards_from_boosters(&cards, code)?;
        let owned_count = cards.iter().filter(|c| c.quantity + c.foil_quantity > 0).count();
        let total_count = cards.len();
        Ok(CollectionView {
            set_code: code.to_string(),
            set_name: series.names.get(&self.card_lang).unwrap_or(&series.name).clone(),
            owned_count,
            total_count,
            completion_percent: if total_count == 0 { 0.0 } else { owned_count as f64 * 100.0 / total_count as f64 },
            cards,
            extras: Vec::new(),
            extras_owned: 0,
            tokens: Vec::new(),
            tokens_owned: 0,
            arts: Vec::new(),
            arts_owned: 0,
            guests: Vec::new(),
            guests_owned: 0,
            elsewhere: Vec::new(),
            elsewhere_owned: 0,
            guest_links: Vec::new(),
        })
    }

    /// A set offered's own cards that only other sets' boosters give (absent from its file).
    pub(super) fn cards_elsewhere(&mut self, set_code: &str) -> Result<Vec<CollectionCard>, String> {
        let offered = self.offered_sets();
        let Some((_, cards)) = self.offered_series(set_code, &offered) else { return Ok(Vec::new()) };
        let own = &self.sets[set_code].index;
        let cards: Vec<GuestCard> = cards.into_iter().filter(|c| !own.contains_key(&c.uuid)).collect();
        self.cards_from_boosters(&cards, set_code)
    }

    /// The guest series a loaded set's boosters give, with the cards of each owned (none for
    /// a set that keeps its guests in its own binder).
    pub(super) fn guest_links(&mut self, set_code: &str) -> Result<Vec<GuestLink>, String> {
        let set = &self.sets[set_code];
        if set.keeps_guests {
            return Ok(Vec::new());
        }
        let mut by_series: HashMap<String, Vec<String>> = HashMap::new();
        for &i in &set.guests {
            let card = &set.file.cards[i];
            by_series.entry(card.set_code.clone()).or_default().push(card.uuid.clone());
        }
        let names: HashMap<String, String> = self.list_sets()?.into_iter().map(|s| (s.code, s.name)).collect();
        let lang = self.card_lang.clone();
        let mut links = Vec::new();
        for (code, uuids) in by_series {
            let owned = self.store.owned_in_set(&code).map_err(err)?;
            let name = match names.get(&code) {
                Some(name) => name.clone(),
                None => self
                    .guest_index()
                    .series
                    .get(&code)
                    .map(|s| s.names.get(&lang).unwrap_or(&s.name).clone())
                    .unwrap_or_else(|| code.clone()),
            };
            links.push(GuestLink {
                owned: uuids.iter().filter(|u| owned.get(*u).is_some_and(|o| o.normal + o.foil > 0)).count(),
                total: uuids.len(),
                name,
                code,
            });
        }
        links.sort_by(|a, b| b.total.cmp(&a.total).then(a.code.cmp(&b.code)));
        Ok(links)
    }

    /// The name and release date of a guest series with a binder of its own.
    pub(super) fn series_label(&mut self, code: &str) -> Option<(String, String)> {
        if !self.is_guest_series(code) {
            return None;
        }
        let lang = self.card_lang.clone();
        let series = self.guest_index().series.get(code)?;
        Some((series.names.get(&lang).unwrap_or(&series.name).clone(), series.release_date.clone()))
    }
}
