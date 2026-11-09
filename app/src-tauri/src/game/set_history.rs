//! A set's history: every booster of the set opened, the latest first, with its cards; the same
//! pulls card by card (when each card came first and last, how often, in foil); and what the set
//! has given so far. Read from the openings recorded, nothing kept apart.

use super::{err, Game, OpenedCard};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetHistoryView {
    pub set_code: String,
    /// The cards pulled, once each (drawn as in a foil, `foilKind` applying only to a foil pull).
    pub cards: HashMap<String, OpenedCard>,
    /// The boosters opened, the latest first.
    pub openings: Vec<SetOpeningView>,
    /// The cards pulled, in collector number order (the set's own first).
    pub ledger: Vec<CardPullsView>,
    pub summary: SetHistorySummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetOpeningView {
    pub id: i64,
    pub opened_at: i64,
    pub kind: String,
    /// Market value of its cards when it was opened.
    pub value_cents: i64,
    /// What it had cost (0 for a booster given); none for the openings before costs were kept.
    pub cost_cents: Option<i64>,
    pub pulls: Vec<PullView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PullView {
    pub uuid: String,
    pub foil: bool,
    /// Priced when it was pulled.
    pub price_cents: Option<u32>,
    pub is_new: bool,
    pub new_foil: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardPullsView {
    pub uuid: String,
    pub first_at: i64,
    pub last_at: i64,
    pub pulls: u32,
    pub foils: u32,
    /// Its best price when pulled.
    pub best_price_cents: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetHistorySummary {
    pub boosters: u32,
    pub cards_pulled: u32,
    pub distinct_cards: u32,
    pub first_opened_at: Option<i64>,
    /// Market value of every card pulled, when pulled.
    pub value_cents: i64,
    /// What the boosters whose cost is known had cost, and what their cards were worth.
    pub spent_cents: i64,
    pub value_of_paid_cents: i64,
    /// The most valuable card pulled (its pull), and the booster that gave it.
    pub best_pull: Option<PullView>,
    pub best_pull_opening: Option<i64>,
    /// The booster whose cards were worth the most.
    pub best_opening: Option<i64>,
}

/// Collector number order: the number's digits, then the whole number.
fn number_key(number: &str) -> (u32, String) {
    let digits: String = number.chars().take_while(char::is_ascii_digit).collect();
    (digits.parse().unwrap_or(u32::MAX), number.to_string())
}

impl Game {
    /// The history of a set's boosters, as `SetHistoryView` tells.
    pub fn set_history(&mut self, set_code: &str) -> Result<SetHistoryView, String> {
        let rows = self.store.set_openings(set_code).map_err(err)?;
        let mut cards: HashMap<String, OpenedCard> = HashMap::new();
        let mut ledger: HashMap<String, CardPullsView> = HashMap::new();
        let mut openings = Vec::with_capacity(rows.len());
        let mut summary = SetHistorySummary {
            boosters: rows.len() as u32,
            cards_pulled: 0,
            distinct_cards: 0,
            first_opened_at: rows.last().map(|r| r.opened_at),
            value_cents: 0,
            spent_cents: 0,
            value_of_paid_cents: 0,
            best_pull: None,
            best_pull_opening: None,
            best_opening: rows.iter().max_by_key(|r| (r.value_cents, r.id)).map(|r| r.id),
        };
        for row in rows {
            summary.value_cents += row.value_cents;
            if let Some(cost) = row.cost_cents {
                summary.spent_cents += cost;
                summary.value_of_paid_cents += row.value_cents;
            }
            let mut pulls = Vec::with_capacity(row.cards.len());
            for c in row.cards {
                if !cards.contains_key(&c.uuid) {
                    // A card whose data is gone (a set rebuilt without it) is left out.
                    let Some(card) = self.pulled_card(set_code, &c.uuid, true, None) else { continue };
                    cards.insert(c.uuid.clone(), card);
                }
                summary.cards_pulled += 1;
                // The rows come the latest first: the first pull seen is the last one.
                let entry = ledger.entry(c.uuid.clone()).or_insert(CardPullsView {
                    uuid: c.uuid.clone(),
                    first_at: row.opened_at,
                    last_at: row.opened_at,
                    pulls: 0,
                    foils: 0,
                    best_price_cents: None,
                });
                entry.first_at = row.opened_at;
                entry.pulls += 1;
                entry.foils += u32::from(c.foil);
                entry.best_price_cents = entry.best_price_cents.max(c.price_cents);
                let pull = PullView { uuid: c.uuid, foil: c.foil, price_cents: c.price_cents, is_new: c.is_new, new_foil: c.new_foil };
                let best = summary.best_pull.as_ref().and_then(|b| b.price_cents);
                if pull.price_cents.is_some() && pull.price_cents > best {
                    summary.best_pull = Some(pull.clone());
                    summary.best_pull_opening = Some(row.id);
                }
                pulls.push(pull);
            }
            openings.push(SetOpeningView {
                id: row.id,
                opened_at: row.opened_at,
                kind: row.kind,
                value_cents: row.value_cents,
                cost_cents: row.cost_cents,
                pulls,
            });
        }
        summary.distinct_cards = ledger.len() as u32;
        let mut ledger: Vec<CardPullsView> = ledger.into_values().collect();
        ledger.sort_by_cached_key(|e| {
            let card = &cards[&e.uuid];
            (card.set_code != set_code, number_key(&card.number), card.name.clone())
        });
        Ok(SetHistoryView { set_code: set_code.to_string(), cards, openings, ledger, summary })
    }
}
