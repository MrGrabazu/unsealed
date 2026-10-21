//! The wallet's history, and undoing a sale (for a growing penalty) or a purchase (right
//! away only). The rules live in `economy.json` (`cancellation`, see `economy::Cancellation`).

use super::{err, localized, Game};
use crate::store::{HistoryRow, TxItem, TxRecord};
use serde::Serialize;
use unsealed_core::achievements::Event;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryView {
    pub entries: Vec<HistoryEntryView>,
    /// More entries exist past these.
    pub has_more: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntryView {
    pub at: i64,
    /// "booster_purchase", "card_sale", "booster_sale", "sale_undone", "purchase_undone",
    /// "charity", "achievement_reward", "starting_balance"...
    pub kind: String,
    /// Signed, as in the wallet.
    pub amount_cents: i64,
    /// For purchases and sales recorded in detail.
    pub transaction_id: Option<i64>,
    pub items: Vec<HistoryItemView>,
    pub undone_at: Option<i64>,
    /// What undoing it cost or refunded, signed as in the wallet.
    pub undo_cents: Option<i64>,
    /// Whether, and at what price, it can be undone now.
    pub undo: Option<UndoOffer>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItemView {
    /// The card's name, or the set's name for boosters.
    pub name: String,
    pub set_code: String,
    /// Booster kind, for boosters.
    pub booster_kind: Option<String>,
    pub foil: bool,
    pub quantity: u32,
    pub unit_cents: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoOffer {
    /// What undoing a sale costs, or undoing a purchase refunds (positive).
    pub amount_cents: i64,
    pub penalty_percent: u32,
    /// For a sale: whether the player has enough money.
    pub affordable: bool,
    /// For a purchase: until when it can be undone (Unix time, seconds).
    pub until: Option<i64>,
}

impl Game {
    pub fn history(&mut self, limit: u32, offset: u32) -> Result<HistoryView, String> {
        let mut rows = self.store.history(limit + 1, offset).map_err(err)?;
        let has_more = rows.len() > limit as usize;
        rows.truncate(limit as usize);
        let balance = self.store.balance().map_err(err)?;
        let entries = rows.into_iter().map(|row| self.entry_view(row, balance)).collect();
        Ok(HistoryView { entries, has_more })
    }

    /// Undoes a sale or a purchase at today's terms; returns the money moved (signed).
    pub fn undo_transaction(&mut self, id: i64) -> Result<i64, String> {
        self.flush_reveal();
        let record = self.store.transaction(id).map_err(err)?.ok_or("unknown transaction")?;
        let balance = self.store.balance().map_err(err)?;
        let offer = self.undo_offer(&record, balance).ok_or("this transaction can no longer be undone")?;
        let now = (self.clock)().now;
        let moved = if record.kind == "booster_purchase" {
            self.store.undo_purchase(id, offer.amount_cents, now).map_err(err)?;
            offer.amount_cents
        } else {
            self.store.undo_sale(id, offer.amount_cents, now).map_err(err)?;
            -offer.amount_cents
        };
        let events: Vec<Event> = self.balance_event().into_iter().collect();
        self.emit(events);
        Ok(moved)
    }

    fn undo_offer(&self, record: &TxRecord, balance: i64) -> Option<UndoOffer> {
        if record.undone_at.is_some() {
            return None;
        }
        let rules = &self.economy.cancellation;
        let age = (self.clock)().now - record.at;
        if record.kind == "booster_purchase" {
            let refund = rules.purchase_refund_cents(-record.amount_cents, age)?;
            // Only while every booster bought is still sealed.
            let stacks = self.store.sealed_stacks().ok()?;
            let still_sealed = record.items.iter().all(|item| match item {
                TxItem::Boosters { set_code, kind, quantity, .. } => stacks
                    .iter()
                    .any(|s| &s.set_code == set_code && &s.kind == kind && s.quantity >= *quantity),
                TxItem::Card { .. } => false,
            });
            if !still_sealed {
                return None;
            }
            return Some(UndoOffer {
                amount_cents: refund,
                penalty_percent: rules.purchase_penalty_percent,
                affordable: true,
                until: Some(record.at + i64::from(rules.purchase_within_minutes) * 60),
            });
        }
        // An artifact may lighten the penalty (a share of the usual one).
        let lighter = self.bonuses().undo_penalty_percent;
        let usual = rules.sale_cancel_cost_cents(record.amount_cents, age);
        let cost = record.amount_cents + (usual - record.amount_cents) * i64::from(lighter) / 100;
        Some(UndoOffer {
            amount_cents: cost,
            penalty_percent: rules.sale_penalty_percent(age) * lighter / 100,
            affordable: balance >= cost,
            until: None,
        })
    }

    fn entry_view(&mut self, row: HistoryRow, balance: i64) -> HistoryEntryView {
        let undo = row.transaction.as_ref().and_then(|t| self.undo_offer(t, balance));
        let items = row
            .transaction
            .as_ref()
            .map(|t| t.items.iter().map(|item| self.item_view(item)).collect())
            .unwrap_or_default();
        HistoryEntryView {
            at: row.at,
            kind: row.kind,
            amount_cents: row.amount_cents,
            transaction_id: row.transaction.as_ref().map(|t| t.id),
            items,
            undone_at: row.transaction.as_ref().and_then(|t| t.undone_at),
            undo_cents: row.transaction.as_ref().and_then(|t| t.undo_cents),
            undo,
        }
    }

    fn item_view(&mut self, item: &TxItem) -> HistoryItemView {
        match item {
            TxItem::Card { uuid, set_code, foil, quantity, unit_cents } => HistoryItemView {
                name: self.card_name(set_code, uuid).unwrap_or_else(|| uuid.clone()),
                set_code: set_code.clone(),
                booster_kind: None,
                foil: *foil,
                quantity: *quantity,
                unit_cents: *unit_cents,
            },
            TxItem::Boosters { set_code, kind, quantity, unit_cents, .. } => HistoryItemView {
                name: self.set_name(set_code),
                set_code: set_code.clone(),
                booster_kind: Some(kind.clone()),
                foil: false,
                quantity: *quantity,
                unit_cents: *unit_cents,
            },
        }
    }

    /// A card's name in the card language, from its set's data.
    fn card_name(&mut self, set_code: &str, uuid: &str) -> Option<String> {
        self.ensure_loaded(set_code).ok()?;
        let set = &self.sets[set_code];
        let card = &set.file.cards[*set.index.get(uuid)?];
        Some(localized(card, &self.card_lang, &self.image_quality).name)
    }

    fn set_name(&self, set_code: &str) -> String {
        self.list_sets()
            .ok()
            .and_then(|sets| sets.into_iter().find(|s| s.code == set_code))
            .map_or_else(|| set_code.to_string(), |s| s.name)
    }
}
