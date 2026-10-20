//! The library: one binder per set the player owns cards of, with what its tooltip tells
//! (completion, value, best card…).

use super::{err, CollectionCard, Game};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};

/// A binder on the library's shelves: a set the player owns cards of.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryBinder {
    pub set_code: String,
    /// Distinct cards of the set owned (any finish), out of those its boosters give.
    pub owned_count: usize,
    pub total_count: usize,
    pub completion_percent: f64,
    /// The set's rares and mythics owned, out of those in its boosters.
    pub rares_owned: usize,
    pub rares_total: usize,
    pub mythics_owned: usize,
    pub mythics_total: usize,
    /// Distinct cards owned in the other sections (collector extras, tokens, art cards, guests
    /// from other sets).
    pub extras_owned: usize,
    pub extras_total: usize,
    pub tokens_owned: usize,
    pub tokens_total: usize,
    pub arts_owned: usize,
    pub arts_total: usize,
    pub guests_owned: usize,
    pub guests_total: usize,
    /// Every copy in the binder, and those in foil.
    pub copies: u32,
    pub foil_copies: u32,
    /// Market value of every copy, and what the Boosterium would pay for them.
    pub value_cents: u64,
    pub sell_value_cents: u64,
    /// The most valuable card owned, at the price of its finest finish owned.
    pub best_card: Option<BestCard>,
    pub boosters_opened: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BestCard {
    pub name: String,
    pub foil: bool,
    pub price_cents: u32,
}

impl Game {
    /// The binders of the library: every set offered that the player owns a card of, by
    /// release (oldest first, as on a shelf filled over the years).
    pub fn library(&mut self) -> Result<Vec<LibraryBinder>, String> {
        let offered = self.offered_sets();
        // A card is filed under its own set; tokens, art cards and Special Guests bear another
        // code than the set whose booster gave them ("TLTR", "ALTR": "LTR").
        let mut candidates = BTreeSet::new();
        for (_, card_set, booster_set) in self.store.owned_card_sources().map_err(err)? {
            if let Some(parent) = card_set.get(1..) {
                candidates.insert(parent.to_string());
            }
            candidates.extend(booster_set);
            candidates.insert(card_set);
        }
        let mut opened: HashMap<String, u32> = HashMap::new();
        for row in self.store.openings_by_booster().map_err(err)? {
            *opened.entry(row.set_code).or_default() += row.boosters;
        }

        let mut binders = Vec::new();
        for code in candidates.into_iter().filter(|c| offered.contains(c)) {
            let view = self.collection(&code)?;
            let sections = [&view.cards, &view.extras, &view.tokens, &view.arts, &view.guests];
            let owned = |c: &CollectionCard| c.quantity + c.foil_quantity > 0;
            if !sections.iter().any(|s| s.iter().any(owned)) {
                continue;
            }
            let of_rarity = |rarity: &str| {
                let cards: Vec<&CollectionCard> = view.cards.iter().filter(|c| c.rarity == rarity).collect();
                (cards.iter().filter(|c| owned(c)).count(), cards.len())
            };
            let (rares_owned, rares_total) = of_rarity("rare");
            let (mythics_owned, mythics_total) = of_rarity("mythic");
            let all = || sections.iter().flat_map(|s| s.iter());
            let best_card = all()
                .flat_map(|c| {
                    [(false, c.quantity, c.price_cents), (true, c.foil_quantity, c.foil_price_cents)]
                        .into_iter()
                        .filter(|(_, n, _)| *n > 0)
                        .filter_map(move |(foil, _, price)| Some((c, foil, price?)))
                })
                .max_by_key(|(_, _, price)| *price)
                .map(|(c, foil, price_cents)| BestCard { name: c.name.clone(), foil, price_cents });
            binders.push(LibraryBinder {
                owned_count: view.owned_count,
                total_count: view.total_count,
                completion_percent: view.completion_percent,
                rares_owned,
                rares_total,
                mythics_owned,
                mythics_total,
                extras_owned: view.extras_owned,
                extras_total: view.extras.len(),
                tokens_owned: view.tokens_owned,
                tokens_total: view.tokens.len(),
                arts_owned: view.arts_owned,
                arts_total: view.arts.len(),
                guests_owned: view.guests_owned,
                guests_total: view.guests.len(),
                copies: all().map(|c| c.quantity + c.foil_quantity).sum(),
                foil_copies: all().map(|c| c.foil_quantity).sum(),
                value_cents: all()
                    .map(|c| {
                        u64::from(c.quantity) * u64::from(c.price_cents.unwrap_or(0))
                            + u64::from(c.foil_quantity) * u64::from(c.foil_price_cents.unwrap_or(0))
                    })
                    .sum(),
                sell_value_cents: all()
                    .map(|c| {
                        u64::from(c.quantity) * u64::from(c.sell_value_cents)
                            + u64::from(c.foil_quantity) * u64::from(c.foil_sell_value_cents)
                    })
                    .sum(),
                best_card,
                boosters_opened: opened.get(&code).copied().unwrap_or(0),
                set_code: code,
            });
        }
        let release: HashMap<String, String> =
            self.list_sets()?.into_iter().map(|s| (s.code, s.release_date)).collect();
        binders.sort_by(|a, b| release.get(&a.set_code).cmp(&release.get(&b.set_code)).then(a.set_code.cmp(&b.set_code)));
        Ok(binders)
    }
}
