//! SQLite persistence: wallet ledger, sealed boosters, collection and opening history.
//!
//! The schema is versioned with `PRAGMA user_version`; to change it, append a new
//! entry to `MIGRATIONS` (never edit an existing one). Every operation that touches
//! several tables runs in a single transaction.
//!
//! The wallet is a ledger: the balance is the sum of its rows, so it can never drift
//! from the history that explains it.

use crate::seal;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use unsealed_core::achievements::Progress;

const MIGRATIONS: &[&str] = &[
    // 1: collection and opening history
    "CREATE TABLE collection (
        card_uuid TEXT NOT NULL,
        foil INTEGER NOT NULL CHECK (foil IN (0, 1)),
        set_code TEXT NOT NULL,
        quantity INTEGER NOT NULL CHECK (quantity > 0),
        first_acquired_at INTEGER NOT NULL,
        PRIMARY KEY (card_uuid, foil)
    ) WITHOUT ROWID;
    CREATE INDEX collection_set ON collection (set_code);

    CREATE TABLE openings (
        id INTEGER PRIMARY KEY,
        opened_at INTEGER NOT NULL,
        set_code TEXT NOT NULL,
        booster_kind TEXT NOT NULL,
        value_cents INTEGER NOT NULL
    );
    CREATE TABLE opening_cards (
        opening_id INTEGER NOT NULL REFERENCES openings (id),
        position INTEGER NOT NULL,
        card_uuid TEXT NOT NULL,
        foil INTEGER NOT NULL,
        price_cents INTEGER,
        PRIMARY KEY (opening_id, position)
    ) WITHOUT ROWID;",
    // 2: wallet ledger and sealed boosters
    "CREATE TABLE ledger (
        id INTEGER PRIMARY KEY,
        at INTEGER NOT NULL,
        kind TEXT NOT NULL,
        amount_cents INTEGER NOT NULL,
        details TEXT
    );
    CREATE TABLE sealed (
        set_code TEXT NOT NULL,
        kind TEXT NOT NULL,
        quantity INTEGER NOT NULL CHECK (quantity >= 0),
        PRIMARY KEY (set_code, kind)
    ) WITHOUT ROWID;",
    // 3: cost basis of sealed boosters, to measure the profit or loss of speculation
    "ALTER TABLE sealed ADD COLUMN cost_cents INTEGER NOT NULL DEFAULT 0;",
    // 4: achievement progress
    "CREATE TABLE ach_metrics (name TEXT PRIMARY KEY, value INTEGER NOT NULL) WITHOUT ROWID;
    CREATE TABLE ach_seen (
        metric TEXT NOT NULL,
        value TEXT NOT NULL,
        PRIMARY KEY (metric, value)
    ) WITHOUT ROWID;
    CREATE TABLE ach_unlocked (id TEXT PRIMARY KEY, unlocked_at INTEGER NOT NULL) WITHOUT ROWID;",
    // 5: booster price history, one point per published price snapshot
    "CREATE TABLE price_history (
        set_code TEXT NOT NULL,
        kind TEXT NOT NULL,
        snapshot TEXT NOT NULL,
        at INTEGER NOT NULL,
        price_cents INTEGER NOT NULL,
        PRIMARY KEY (set_code, kind, snapshot)
    ) WITHOUT ROWID;",
    // 6: indexes for the statistics (best pulls, best and worst boosters)
    "CREATE INDEX opening_cards_price ON opening_cards (price_cents);
    CREATE INDEX openings_value ON openings (value_cents);",
    // 7: what describes the save itself (name, dates), see `saves`
    "CREATE TABLE save_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL) WITHOUT ROWID;",
    // 8: purchases and sales in detail (items as JSON, see `TxItem`), to show and undo them
    "CREATE TABLE transactions (
        id INTEGER PRIMARY KEY,
        ledger_id INTEGER NOT NULL UNIQUE REFERENCES ledger (id),
        at INTEGER NOT NULL,
        kind TEXT NOT NULL,
        amount_cents INTEGER NOT NULL,
        items TEXT NOT NULL,
        undone_at INTEGER,
        undo_cents INTEGER
    );",
    // 9: copies pulled of each card of a limited print run; never decreases (see `limited`)
    "CREATE TABLE limited_pulls (uuid TEXT PRIMARY KEY, pulled INTEGER NOT NULL) WITHOUT ROWID;",
    // 10: bets on boosters; a winner is paid once its booster's cards are all seen (`paid_at`)
    "CREATE TABLE bets (
        id INTEGER PRIMARY KEY,
        opening_id INTEGER NOT NULL REFERENCES openings (id),
        placed_at INTEGER NOT NULL,
        bet TEXT NOT NULL,
        stake_cents INTEGER NOT NULL,
        probability REAL NOT NULL,
        won INTEGER NOT NULL CHECK (won IN (0, 1)),
        payout_cents INTEGER NOT NULL,
        paid_at INTEGER
    );
    CREATE INDEX bets_unpaid ON bets (paid_at);",
    // 11: the debt of the Boosterium's advances, tried out for a while and given up for the
    // collection's income; no longer read
    "CREATE TABLE debt_movements (
        id INTEGER PRIMARY KEY,
        at INTEGER NOT NULL,
        kind TEXT NOT NULL,
        cents INTEGER NOT NULL,
        day TEXT
    );",
    // 12: the artifacts owned (each one once at most), with how they were got ("shop",
    // "booster", "achievement", "allOthers")
    "CREATE TABLE artifacts (
        id TEXT PRIMARY KEY,
        acquired_at INTEGER NOT NULL,
        source TEXT NOT NULL
    );",
    // 13: the booster boxes whose box topper was given, by set and booster kind: a box is a
    // number of boosters of a kind opened (the openings), its topper given once
    "CREATE TABLE box_toppers (
        set_code TEXT NOT NULL,
        kind TEXT NOT NULL,
        boxes INTEGER NOT NULL,
        PRIMARY KEY (set_code, kind)
    ) WITHOUT ROWID;",
];

/// Key of `save_meta`: the last day a lost bet's stake was given back (an artifact's effect).
const BET_REFUND_DAY: &str = "bet_refund_day";

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("not enough money: {needed} needed, {available} available (in cents)")]
    InsufficientFunds { needed: i64, available: i64 },
    #[error("no sealed booster of that kind")]
    NoSealedBooster,
    #[error("not enough copies of card {0}")]
    NotOwned(String),
    #[error("this transaction cannot be undone: {0}")]
    CannotUndo(&'static str),
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("invalid transaction record: {0}")]
    Record(#[from] serde_json::Error),
}

/// What a purchase or a sale was made of, kept to show it and to undo it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum TxItem {
    Card { uuid: String, set_code: String, foil: bool, quantity: u32, unit_cents: u32 },
    /// `cost_cents`: the cost basis of these boosters (what they cost the player).
    Boosters { set_code: String, kind: String, quantity: u32, unit_cents: u32, cost_cents: i64 },
}

/// A purchase or a sale, as recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxRecord {
    pub id: i64,
    pub at: i64,
    pub kind: String,
    /// Signed, as in the wallet: negative for a purchase.
    pub amount_cents: i64,
    pub items: Vec<TxItem>,
    pub undone_at: Option<i64>,
    /// What undoing it cost (sale) or refunded (purchase), signed as in the wallet.
    pub undo_cents: Option<i64>,
}

/// A movement of the wallet, with its transaction when it is a purchase or a sale that
/// was recorded in detail (older ones were not).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRow {
    pub at: i64,
    pub kind: String,
    pub amount_cents: i64,
    pub details: Option<String>,
    pub transaction: Option<TxRecord>,
}

pub type StoreResult<T> = Result<T, StoreError>;

pub struct Store {
    conn: Connection,
}

/// A card obtained from a booster, as recorded in the collection.
pub struct OpeningCard<'a> {
    pub uuid: &'a str,
    /// Set the card belongs to (may differ from the opened booster's set).
    pub set_code: &'a str,
    pub foil: bool,
    pub price_cents: Option<u32>,
    /// Of a limited print run: counted in `limited_pulls`.
    pub limited: bool,
}

/// A bet placed on a booster and settled by its draw (see `open_sealed_with_bets`).
pub struct PlacedBet {
    /// What it was on, as JSON (target and direction).
    pub bet: String,
    pub stake_cents: u32,
    pub probability: f64,
    pub won: bool,
    /// What it pays back, the stake included (0 when lost).
    pub payout_cents: i64,
}

/// Copies of one card sold at a fixed unit price.
pub struct Sale<'a> {
    pub uuid: &'a str,
    pub foil: bool,
    pub quantity: u32,
    pub unit_cents: u32,
}

/// A stack of sealed boosters. `cost_cents` is what the player paid for the whole stack
/// (0 for free boosters such as rewards).
#[derive(Debug, PartialEq, Eq)]
pub struct SealedRow {
    pub set_code: String,
    pub kind: String,
    pub quantity: u32,
    pub cost_cents: i64,
}

/// What an unlocked achievement pays, once resolved against the available data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantedReward {
    pub achievement_id: String,
    pub cash_cents: u32,
    /// (set code, booster kind, quantity) of free sealed boosters.
    pub sealed: Option<(String, String, u32)>,
}

/// Result of opening a sealed booster.
#[derive(Debug)]
pub struct OpenOutcome {
    /// For each card, whether it is a new discovery.
    pub is_new: Vec<bool>,
    /// For each card, whether it is a foil of a card only owned in non-foil until now.
    pub new_foil: Vec<bool>,
    /// Cost basis of the opened booster.
    pub cost_cents: i64,
}

/// Result of selling sealed boosters back.
#[derive(Debug)]
pub struct BoosterSale {
    pub total_cents: i64,
    /// Cost basis of the sold boosters.
    pub cost_cents: i64,
}

/// Totals over every booster ever opened.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct OpeningTotals {
    pub boosters: u32,
    pub cards: u32,
    pub foils: u32,
    /// Market value of the cards when they were pulled.
    pub value_cents: i64,
}

/// One past opening and its most valuable card.
#[derive(Debug, PartialEq, Eq)]
pub struct OpeningRow {
    pub id: i64,
    pub opened_at: i64,
    pub set_code: String,
    pub kind: String,
    pub value_cents: i64,
    /// The most valuable card, if any card had a price.
    pub best_card: Option<PullRef>,
}

/// A card of an opening, with its price when pulled.
#[derive(Debug, PartialEq, Eq)]
pub struct PullRef {
    pub uuid: String,
    pub foil: bool,
    pub price_cents: u32,
}

/// A card pulled from a booster, with its price at the time.
#[derive(Debug, PartialEq, Eq)]
pub struct PullRow {
    pub uuid: String,
    pub foil: bool,
    pub price_cents: u32,
    pub opened_at: i64,
    /// Set of the booster it came from (its card data lives in that set's file).
    pub booster_set: String,
}

/// Openings of one kind of booster.
#[derive(Debug, PartialEq, Eq)]
pub struct BoosterKindTotals {
    pub set_code: String,
    pub kind: String,
    pub boosters: u32,
    pub value_cents: i64,
    pub best_value_cents: i64,
}

/// Which openings to list.
#[derive(Debug, Clone, Copy)]
pub enum OpeningOrder {
    Latest,
    MostValuable,
    LeastValuable,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Owned {
    pub normal: u32,
    pub foil: u32,
}

impl Store {
    pub fn open(path: &Path) -> StoreResult<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::init(conn)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> StoreResult<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> StoreResult<Self> {
        conn.pragma_update(None, "foreign_keys", true)?;
        // Before anything is written: the seal's triggers call this function.
        seal::register(&conn)?;
        migrate(&mut conn)?;
        seal::install(&conn)?;
        Ok(Self { conn })
    }

    /// Whether the save is as the game left it, not edited by hand (see `seal`).
    pub fn seal_intact(&self) -> StoreResult<bool> {
        Ok(seal::intact(&self.conn)?)
    }

    // ---- save description -------------------------------------------------------

    pub fn meta(&self, key: &str) -> StoreResult<Option<String>> {
        Ok(self.conn.query_row("SELECT value FROM save_meta WHERE key = ?1", [key], |r| r.get(0)).optional()?)
    }

    pub fn set_meta(&mut self, key: &str, value: &str) -> StoreResult<()> {
        self.conn.execute(
            "INSERT INTO save_meta (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Adds one copy of a card to the collection, outside any booster (a gift).
    pub fn give_card(&mut self, uuid: &str, set_code: &str, foil: bool, now: i64) -> StoreResult<()> {
        self.conn.execute(
            "INSERT INTO collection (card_uuid, foil, set_code, quantity, first_acquired_at)
             VALUES (?1, ?2, ?3, 1, ?4)
             ON CONFLICT (card_uuid, foil) DO UPDATE SET quantity = quantity + 1",
            params![uuid, foil, set_code, now],
        )?;
        Ok(())
    }

    /// Writes a consistent copy of the whole save to `path` (which must not exist).
    pub fn copy_to(&self, path: &Path) -> StoreResult<()> {
        self.conn.execute("VACUUM INTO ?1", [path.to_string_lossy()])?;
        Ok(())
    }

    // ---- wallet -------------------------------------------------------------

    pub fn balance(&self) -> StoreResult<i64> {
        Ok(balance_of(&self.conn)?)
    }

    /// Credits what the collection earned, with the `save_meta` entries that say how far it was
    /// counted (`meta`), in one go. Earnings within the same hour share one wallet movement,
    /// so that the history is not flooded with one line a minute.
    pub fn credit_income(&mut self, cents: i64, meta: &[(&str, String)], now: i64) -> StoreResult<()> {
        let tx = self.conn.transaction()?;
        if cents > 0 {
            let last: Option<(i64, String, i64)> = tx
                .query_row("SELECT id, kind, at FROM ledger ORDER BY id DESC LIMIT 1", [], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })
                .optional()?;
            match last {
                Some((id, kind, at)) if kind == "income" && at.div_euclid(3600) == now.div_euclid(3600) => {
                    tx.execute("UPDATE ledger SET amount_cents = amount_cents + ?2 WHERE id = ?1", params![id, cents])?;
                }
                _ => {
                    add_ledger(&tx, now, "income", cents, None)?;
                }
            }
        }
        for (key, value) in meta {
            tx.execute(
                "INSERT INTO save_meta (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Grants the starting capital once, on the very first run.
    pub fn ensure_starting_balance(&mut self, amount_cents: u32, now: i64) -> StoreResult<()> {
        let tx = self.conn.transaction()?;
        let granted: Option<i64> = tx
            .query_row("SELECT 1 FROM ledger WHERE kind = 'starting_balance' LIMIT 1", [], |r| r.get(0))
            .optional()?;
        if granted.is_none() {
            add_ledger(&tx, now, "starting_balance", i64::from(amount_cents), None)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Number of charity claims made on a given local day ("YYYY-MM-DD").
    pub fn charity_claims_on(&self, day: &str) -> StoreResult<u32> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM ledger WHERE kind = 'charity' AND details = ?1",
            [day],
            |r| r.get(0),
        )?)
    }

    pub fn claim_charity(&mut self, amount_cents: u32, day: &str, now: i64) -> StoreResult<()> {
        add_ledger(&self.conn, now, "charity", i64::from(amount_cents), Some(day))?;
        Ok(())
    }

    // ---- achievements ---------------------------------------------------------

    pub fn load_progress(&self) -> StoreResult<Progress> {
        let mut progress = Progress::default();
        let mut stmt = self.conn.prepare("SELECT name, value FROM ach_metrics")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
            let (name, value) = row?;
            progress.metrics.insert(name, value);
        }
        let mut stmt = self.conn.prepare("SELECT metric, value FROM ach_seen")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (metric, value) = row?;
            progress.seen.entry(metric).or_default().insert(value);
        }
        let mut stmt = self.conn.prepare("SELECT id, unlocked_at FROM ach_unlocked")?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))? {
            let (id, at) = row?;
            progress.unlocked.insert(id, at);
        }
        Ok(progress)
    }

    /// Saves the achievement progress and pays the rewards of the achievements that are
    /// unlocked for the first time, atomically. A reward is paid once, whatever happens.
    pub fn save_progress(&mut self, progress: &Progress, rewards: &[GrantedReward], now: i64) -> StoreResult<()> {
        let tx = self.conn.transaction()?;
        for (name, value) in &progress.metrics {
            tx.execute(
                "INSERT INTO ach_metrics (name, value) VALUES (?1, ?2)
                 ON CONFLICT (name) DO UPDATE SET value = excluded.value",
                params![name, value],
            )?;
        }
        for (metric, values) in &progress.seen {
            for value in values {
                tx.execute("INSERT OR IGNORE INTO ach_seen (metric, value) VALUES (?1, ?2)", params![metric, value])?;
            }
        }
        for (id, at) in &progress.unlocked {
            let inserted =
                tx.execute("INSERT OR IGNORE INTO ach_unlocked (id, unlocked_at) VALUES (?1, ?2)", params![id, at])?;
            let Some(reward) = rewards.iter().find(|r| r.achievement_id == *id).filter(|_| inserted == 1) else {
                continue;
            };
            if reward.cash_cents > 0 {
                add_ledger(&tx, now, "achievement_reward", i64::from(reward.cash_cents), Some(id))?;
            }
            if let Some((set_code, kind, quantity)) = &reward.sealed {
                tx.execute(
                    "INSERT INTO sealed (set_code, kind, quantity, cost_cents) VALUES (?1, ?2, ?3, 0)
                     ON CONFLICT (set_code, kind) DO UPDATE SET quantity = quantity + excluded.quantity",
                    params![set_code, kind, quantity],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    // ---- box toppers ----------------------------------------------------------

    /// How many boosters of each set and kind were opened, box toppers' own included.
    pub fn opened_by_kind(&self) -> StoreResult<Vec<(String, String, u32)>> {
        let mut stmt = self.conn.prepare("SELECT set_code, booster_kind, COUNT(*) FROM openings GROUP BY set_code, booster_kind")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// How many boosters of a set and kind were opened.
    pub fn opened_of(&self, set_code: &str, kind: &str) -> StoreResult<u32> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM openings WHERE set_code = ?1 AND booster_kind = ?2",
            params![set_code, kind],
            |r| r.get(0),
        )?)
    }

    /// Gives the box toppers of the boxes completed (`boxes`, of a set and booster kind) not
    /// given yet: `packs` (topper booster, how many) for each, as sealed boosters that cost
    /// nothing. Returns how many boxes were given now (0 if none).
    pub fn give_box_toppers(&mut self, set_code: &str, kind: &str, boxes: u32, packs: &[(&str, u32)]) -> StoreResult<u32> {
        let tx = self.conn.transaction()?;
        let given: u32 = tx
            .query_row("SELECT boxes FROM box_toppers WHERE set_code = ?1 AND kind = ?2", params![set_code, kind], |r| r.get(0))
            .optional()?
            .unwrap_or(0);
        if boxes <= given {
            return Ok(0);
        }
        let new = boxes - given;
        for (topper, count) in packs {
            tx.execute(
                "INSERT INTO sealed (set_code, kind, quantity, cost_cents) VALUES (?1, ?2, ?3, 0)
                 ON CONFLICT (set_code, kind) DO UPDATE SET quantity = quantity + excluded.quantity",
                params![set_code, topper, count * new],
            )?;
        }
        tx.execute(
            "INSERT INTO box_toppers (set_code, kind, boxes) VALUES (?1, ?2, ?3)
             ON CONFLICT (set_code, kind) DO UPDATE SET boxes = excluded.boxes",
            params![set_code, kind, boxes],
        )?;
        tx.commit()?;
        Ok(new)
    }

    // ---- sealed boosters ----------------------------------------------------

    /// Buys sealed boosters. Returns the new balance.
    pub fn buy_boosters(
        &mut self,
        set_code: &str,
        kind: &str,
        quantity: u32,
        unit_price_cents: u32,
        now: i64,
    ) -> StoreResult<i64> {
        let tx = self.conn.transaction()?;
        let total = i64::from(unit_price_cents) * i64::from(quantity);
        let balance = balance_of(&tx)?;
        if balance < total {
            return Err(StoreError::InsufficientFunds { needed: total, available: balance });
        }
        let ledger_id =
            add_ledger(&tx, now, "booster_purchase", -total, Some(&format!("{set_code}/{kind} x{quantity}")))?;
        let item = TxItem::Boosters {
            set_code: set_code.to_string(),
            kind: kind.to_string(),
            quantity,
            unit_cents: unit_price_cents,
            cost_cents: total,
        };
        add_transaction(&tx, ledger_id, now, "booster_purchase", -total, &[item])?;
        tx.execute(
            "INSERT INTO sealed (set_code, kind, quantity, cost_cents) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (set_code, kind) DO UPDATE SET
                quantity = quantity + excluded.quantity,
                cost_cents = cost_cents + excluded.cost_cents",
            params![set_code, kind, quantity, total],
        )?;
        tx.commit()?;
        Ok(balance - total)
    }

    /// Sells sealed boosters back at `unit_cents` each.
    pub fn sell_boosters(
        &mut self,
        set_code: &str,
        kind: &str,
        quantity: u32,
        unit_cents: u32,
        now: i64,
    ) -> StoreResult<BoosterSale> {
        let tx = self.conn.transaction()?;
        let cost_cents = take_sealed(&tx, set_code, kind, quantity)?;
        let total_cents = i64::from(unit_cents) * i64::from(quantity);
        let ledger_id =
            add_ledger(&tx, now, "booster_sale", total_cents, Some(&format!("{set_code}/{kind} x{quantity}")))?;
        let item = TxItem::Boosters { set_code: set_code.to_string(), kind: kind.to_string(), quantity, unit_cents, cost_cents };
        add_transaction(&tx, ledger_id, now, "booster_sale", total_cents, &[item])?;
        tx.commit()?;
        Ok(BoosterSale { total_cents, cost_cents })
    }

    // ---- price history --------------------------------------------------------

    /// Records booster prices for one price snapshot. A snapshot already recorded is left untouched.
    pub fn record_booster_prices(&mut self, snapshot: &str, at: i64, prices: &[(String, String, u32)]) -> StoreResult<()> {
        let tx = self.conn.transaction()?;
        for (set_code, kind, price_cents) in prices {
            tx.execute(
                "INSERT OR IGNORE INTO price_history (set_code, kind, snapshot, at, price_cents)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![set_code, kind, snapshot, at, price_cents],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// The latest `limit` recorded prices of a booster, oldest first.
    pub fn booster_price_history(&self, set_code: &str, kind: &str, limit: u32) -> StoreResult<Vec<u32>> {
        let mut stmt = self.conn.prepare(
            "SELECT price_cents FROM (
                 SELECT price_cents, at, snapshot FROM price_history WHERE set_code = ?1 AND kind = ?2
                 ORDER BY at DESC, snapshot DESC LIMIT ?3
             ) ORDER BY at ASC, snapshot ASC",
        )?;
        let rows = stmt.query_map(params![set_code, kind, limit], |r| r.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    // ---- statistics -----------------------------------------------------------

    pub fn opening_totals(&self) -> StoreResult<OpeningTotals> {
        let (boosters, value_cents) = self.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(value_cents), 0) FROM openings",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let (cards, foils) = self.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(foil), 0) FROM opening_cards",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok(OpeningTotals { boosters, cards, foils, value_cents })
    }

    /// Sum of the ledger by kind of movement ("booster_purchase", "card_sale"...).
    pub fn ledger_totals(&self) -> StoreResult<HashMap<String, i64>> {
        let mut stmt = self.conn.prepare("SELECT kind, SUM(amount_cents) FROM ledger GROUP BY kind")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn openings(&self, order: OpeningOrder, limit: u32) -> StoreResult<Vec<OpeningRow>> {
        let order_by = match order {
            OpeningOrder::Latest => "o.id DESC",
            OpeningOrder::MostValuable => "o.value_cents DESC, o.id DESC",
            OpeningOrder::LeastValuable => "o.value_cents ASC, o.id DESC",
        };
        let mut stmt = self.conn.prepare(&format!(
            "SELECT o.id, o.opened_at, o.set_code, o.booster_kind, o.value_cents, c.card_uuid, c.foil, c.price_cents
             FROM openings o
             LEFT JOIN opening_cards c ON c.opening_id = o.id AND c.position = (
                 SELECT b.position FROM opening_cards b WHERE b.opening_id = o.id AND b.price_cents IS NOT NULL
                 ORDER BY b.price_cents DESC, b.position DESC LIMIT 1)
             ORDER BY {order_by} LIMIT ?1"
        ))?;
        let rows = stmt.query_map([limit], |r| {
            let uuid: Option<String> = r.get(5)?;
            let best_card = match uuid {
                Some(uuid) => Some(PullRef { uuid, foil: r.get(6)?, price_cents: r.get(7)? }),
                None => None,
            };
            Ok(OpeningRow {
                id: r.get(0)?,
                opened_at: r.get(1)?,
                set_code: r.get(2)?,
                kind: r.get(3)?,
                value_cents: r.get(4)?,
                best_card,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The most valuable cards ever pulled, at their price when pulled.
    pub fn best_pulls(&self, limit: u32) -> StoreResult<Vec<PullRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.card_uuid, c.foil, c.price_cents, o.opened_at, o.set_code
             FROM opening_cards c JOIN openings o ON o.id = c.opening_id
             WHERE c.price_cents IS NOT NULL
             ORDER BY c.price_cents DESC, o.id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |r| {
            Ok(PullRow {
                uuid: r.get(0)?,
                foil: r.get(1)?,
                price_cents: r.get(2)?,
                opened_at: r.get(3)?,
                booster_set: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Openings grouped by set and kind of booster, most opened first.
    pub fn openings_by_booster(&self) -> StoreResult<Vec<BoosterKindTotals>> {
        let mut stmt = self.conn.prepare(
            "SELECT set_code, booster_kind, COUNT(*), SUM(value_cents), MAX(value_cents)
             FROM openings GROUP BY set_code, booster_kind ORDER BY COUNT(*) DESC, set_code, booster_kind",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(BoosterKindTotals {
                set_code: r.get(0)?,
                kind: r.get(1)?,
                boosters: r.get(2)?,
                value_cents: r.get(3)?,
                best_value_cents: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Sealed boosters owned.
    pub fn sealed_stacks(&self) -> StoreResult<Vec<SealedRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT set_code, kind, quantity, cost_cents FROM sealed WHERE quantity > 0 ORDER BY set_code, kind",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(SealedRow { set_code: r.get(0)?, kind: r.get(1)?, quantity: r.get(2)?, cost_cents: r.get(3)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    // ---- openings and collection ---------------------------------------------

    /// Opens one sealed booster: consumes it, records the opening and adds its cards to
    /// the collection, atomically. Returns, for each card, whether it is a new
    /// discovery: the player did not own that card (in any finish) before, including
    /// earlier cards of the same booster.
    #[cfg(test)]
    pub fn open_sealed(
        &mut self,
        set_code: &str,
        booster_kind: &str,
        cards: &[OpeningCard<'_>],
        now: i64,
    ) -> StoreResult<OpenOutcome> {
        self.open_sealed_with_bets(set_code, booster_kind, cards, &[], now)
    }

    /// `open_sealed`, with bets placed on the booster: their stakes leave the wallet with the
    /// opening (refused if they cannot be afforded); what the winners pay waits for `pay_bets`,
    /// once every card is seen.
    pub fn open_sealed_with_bets(
        &mut self,
        set_code: &str,
        booster_kind: &str,
        cards: &[OpeningCard<'_>],
        bets: &[PlacedBet],
        now: i64,
    ) -> StoreResult<OpenOutcome> {
        let tx = self.conn.transaction()?;
        let cost_cents = take_sealed(&tx, set_code, booster_kind, 1)?;
        let value: i64 = cards.iter().map(|c| i64::from(c.price_cents.unwrap_or(0))).sum();
        tx.execute(
            "INSERT INTO openings (opened_at, set_code, booster_kind, value_cents) VALUES (?1, ?2, ?3, ?4)",
            params![now, set_code, booster_kind, value],
        )?;
        let opening_id = tx.last_insert_rowid();

        let mut is_new = Vec::with_capacity(cards.len());
        let mut new_foil = Vec::with_capacity(cards.len());
        for (position, c) in cards.iter().enumerate() {
            // The best finish ever owned (a row stays once its copies are sold): none, or
            // whether one was foil.
            let known: Option<bool> = tx
                .query_row("SELECT MAX(foil) FROM collection WHERE card_uuid = ?1", [c.uuid], |r| r.get(0))?;
            is_new.push(known.is_none());
            new_foil.push(c.foil && known == Some(false));
            tx.execute(
                "INSERT INTO collection (card_uuid, foil, set_code, quantity, first_acquired_at)
                 VALUES (?1, ?2, ?3, 1, ?4)
                 ON CONFLICT (card_uuid, foil) DO UPDATE SET quantity = quantity + 1",
                params![c.uuid, c.foil, c.set_code, now],
            )?;
            tx.execute(
                "INSERT INTO opening_cards (opening_id, position, card_uuid, foil, price_cents)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![opening_id, position as i64, c.uuid, c.foil, c.price_cents],
            )?;
            if c.limited {
                tx.execute(
                    "INSERT INTO limited_pulls (uuid, pulled) VALUES (?1, 1)
                     ON CONFLICT (uuid) DO UPDATE SET pulled = pulled + 1",
                    [c.uuid],
                )?;
            }
        }
        if !bets.is_empty() {
            let stakes: i64 = bets.iter().map(|b| i64::from(b.stake_cents)).sum();
            let available = balance_of(&tx)?;
            if available < stakes {
                return Err(StoreError::InsufficientFunds { needed: stakes, available });
            }
            let details = format!("{set_code}/{booster_kind}");
            add_ledger(&tx, now, "bet_stake", -stakes, Some(&details))?;
            for b in bets {
                tx.execute(
                    "INSERT INTO bets (opening_id, placed_at, bet, stake_cents, probability, won, payout_cents)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![opening_id, now, b.bet, b.stake_cents, b.probability, b.won, b.payout_cents],
                )?;
            }
        }
        tx.commit()?;
        Ok(OpenOutcome { is_new, new_foil, cost_cents })
    }

    /// Pays the bets won and not paid yet (those of the booster whose cards were all seen, or
    /// left from a game closed before), returning what they brought in. Paid once.
    /// Pays the bets won and settled, `win_bonus_percent` more (an artifact's). With
    /// `refund_day` ("YYYY-MM-DD", an artifact's), the stake of one bet lost among them is
    /// given back, once that day.
    pub fn pay_bets(&mut self, now: i64, win_bonus_percent: u32, refund_day: Option<&str>) -> StoreResult<i64> {
        let tx = self.conn.transaction()?;
        let won: Vec<(i64, i64)> = {
            let mut stmt = tx.prepare("SELECT opening_id, SUM(payout_cents) FROM bets WHERE paid_at IS NULL AND won = 1 GROUP BY opening_id")?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            rows.collect::<Result<_, _>>()?
        };
        let mut paid = 0;
        for &(opening_id, payout) in &won {
            let payout = payout + payout * i64::from(win_bonus_percent) / 100;
            add_ledger(&tx, now, "bet_win", payout, Some(&opening_id.to_string()))?;
            paid += payout;
        }
        if let Some(day) = refund_day {
            let used: Option<String> =
                tx.query_row("SELECT value FROM save_meta WHERE key = ?1", [BET_REFUND_DAY], |r| r.get(0)).optional()?;
            let lost: Option<(i64, i64)> = tx
                .query_row(
                    "SELECT opening_id, stake_cents FROM bets WHERE paid_at IS NULL AND won = 0 ORDER BY stake_cents DESC LIMIT 1",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            if let (Some((opening_id, stake)), true) = (lost, used.as_deref() != Some(day)) {
                add_ledger(&tx, now, "bet_refund", stake, Some(&opening_id.to_string()))?;
                tx.execute(
                    "INSERT INTO save_meta (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                    params![BET_REFUND_DAY, day],
                )?;
                paid += stake;
            }
        }
        tx.execute("UPDATE bets SET paid_at = ?1 WHERE paid_at IS NULL", [now])?;
        tx.commit()?;
        Ok(paid)
    }

    // ---- artifacts ----------------------------------------------------------------------

    /// The artifacts owned: (id, when, how it was got), oldest first.
    pub fn artifacts(&self) -> StoreResult<Vec<(String, i64, String)>> {
        let mut stmt = self.conn.prepare("SELECT id, acquired_at, source FROM artifacts ORDER BY acquired_at, id")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Records an artifact got for free (found, or given): false if it was already owned.
    pub fn add_artifact(&mut self, id: &str, source: &str, now: i64) -> StoreResult<bool> {
        let added = self.conn.execute("INSERT OR IGNORE INTO artifacts (id, acquired_at, source) VALUES (?1, ?2, ?3)", params![id, now, source])?;
        Ok(added == 1)
    }

    /// Buys an artifact: pays its price and records it, atomically.
    pub fn buy_artifact(&mut self, id: &str, price_cents: u32, now: i64) -> StoreResult<()> {
        let tx = self.conn.transaction()?;
        let price = i64::from(price_cents);
        let available = balance_of(&tx)?;
        if available < price {
            return Err(StoreError::InsufficientFunds { needed: price, available });
        }
        add_ledger(&tx, now, "artifact_purchase", -price, Some(id))?;
        tx.execute("INSERT INTO artifacts (id, acquired_at, source) VALUES (?1, ?2, 'shop')", params![id, now])?;
        tx.commit()?;
        Ok(())
    }

    /// Copies ever pulled of each card of a limited print run, by uuid.
    pub fn limited_pulls(&self) -> StoreResult<HashMap<String, u32>> {
        let mut stmt = self.conn.prepare("SELECT uuid, pulled FROM limited_pulls")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Files the copies of a card under another uuid (a foil printing merged into its card in
    /// the data), with its openings: the copies add up, the history follows. Nothing happens
    /// when no copy of `from` was ever owned.
    pub fn merge_card(&mut self, from: &str, into: &str) -> StoreResult<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO collection (card_uuid, foil, set_code, quantity, first_acquired_at)
             SELECT ?2, foil, set_code, quantity, first_acquired_at FROM collection WHERE card_uuid = ?1
             ON CONFLICT (card_uuid, foil) DO UPDATE SET quantity = quantity + excluded.quantity,
               first_acquired_at = MIN(first_acquired_at, excluded.first_acquired_at)",
            params![from, into],
        )?;
        tx.execute("DELETE FROM collection WHERE card_uuid = ?1", [from])?;
        tx.execute("UPDATE opening_cards SET card_uuid = ?2 WHERE card_uuid = ?1", params![from, into])?;
        tx.commit()?;
        Ok(())
    }

    /// Owned quantities of the cards belonging to a set, by card uuid.
    pub fn owned_in_set(&self, set_code: &str) -> StoreResult<HashMap<String, Owned>> {
        let mut stmt =
            self.conn.prepare("SELECT card_uuid, foil, quantity FROM collection WHERE set_code = ?1")?;
        let rows = stmt.query_map([set_code], |r| Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?, r.get::<_, u32>(2)?)))?;
        let mut owned: HashMap<String, Owned> = HashMap::new();
        for row in rows {
            let (uuid, foil, quantity) = row?;
            let entry = owned.entry(uuid).or_default();
            if foil {
                entry.foil = quantity;
            } else {
                entry.normal = quantity;
            }
        }
        Ok(owned)
    }

    /// Every owned card, as (uuid, foil, quantity).
    /// Where each owned card came from: (uuid, its own set code, the set of a booster that
    /// gave it, if any). Tokens, art cards and Special Guests bear another code than the
    /// booster's set, whose file holds them.
    /// A summary of everything that changes what the collection holds or is worth: the
    /// copies owned, the latest opening and wallet movement (every purchase, sale, undo…),
    /// the artifacts (their sell rates). Equal stamps, same collection.
    pub fn collection_stamp(&self) -> StoreResult<String> {
        Ok(self.conn.query_row(
            "SELECT (SELECT COUNT(*) || ':' || TOTAL(quantity) || ':' || TOTAL(foil * quantity) || ':' || IFNULL(MAX(first_acquired_at), 0) FROM collection)
                 || '/' || (SELECT IFNULL(MAX(id), 0) FROM openings)
                 || '/' || (SELECT IFNULL(MAX(id), 0) FROM ledger)
                 || '/' || (SELECT COUNT(*) FROM artifacts)",
            [],
            |r| r.get(0),
        )?)
    }

    pub fn owned_card_sources(&self) -> StoreResult<Vec<(String, String, Option<String>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.card_uuid, MIN(c.set_code), MIN(o.set_code)
             FROM collection c
             LEFT JOIN opening_cards oc ON oc.card_uuid = c.card_uuid
             LEFT JOIN openings o ON o.id = oc.opening_id
             WHERE c.quantity > 0
             GROUP BY c.card_uuid",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn owned_cards(&self) -> StoreResult<Vec<(String, bool, u32)>> {
        let mut stmt = self.conn.prepare("SELECT card_uuid, foil, quantity FROM collection")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Sells cards from the collection, atomically: all sales succeed or none does.
    /// Returns the total paid. Selling the last copy removes the card from the collection.
    pub fn sell_cards(&mut self, sales: &[Sale<'_>], now: i64) -> StoreResult<i64> {
        let tx = self.conn.transaction()?;
        let (total, count, items) = take_cards(&tx, sales)?;
        let ledger_id = add_ledger(&tx, now, "card_sale", total, Some(&format!("{count} cards")))?;
        add_transaction(&tx, ledger_id, now, "card_sale", total, &items)?;
        tx.commit()?;
        Ok(total)
    }

    // ---- history and undoing --------------------------------------------------

    /// The wallet's movements, newest first, with the detail of the purchases and sales.
    pub fn history(&self, limit: u32, offset: u32) -> StoreResult<Vec<HistoryRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT l.at, l.kind, l.amount_cents, l.details, t.id
             FROM ledger l LEFT JOIN transactions t ON t.ledger_id = l.id
             ORDER BY l.id DESC LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt
            .query_map(params![limit, offset], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, Option<i64>>(4)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(at, kind, amount_cents, details, id)| {
                let transaction = id.map(|id| transaction_of(&self.conn, id)).transpose()?.flatten();
                Ok(HistoryRow { at, kind, amount_cents, details, transaction })
            })
            .collect()
    }

    pub fn transaction(&self, id: i64) -> StoreResult<Option<TxRecord>> {
        transaction_of(&self.conn, id)
    }

    /// Undoes a sale: the player pays `cost_cents` (what they received plus the penalty)
    /// and gets the cards or boosters back, with the boosters' cost basis.
    pub fn undo_sale(&mut self, id: i64, cost_cents: i64, now: i64) -> StoreResult<()> {
        let tx = self.conn.transaction()?;
        let record = transaction_of(&tx, id)?.ok_or(StoreError::CannotUndo("unknown transaction"))?;
        if !record.kind.ends_with("_sale") {
            return Err(StoreError::CannotUndo("not a sale"));
        }
        if record.undone_at.is_some() {
            return Err(StoreError::CannotUndo("already undone"));
        }
        let balance = balance_of(&tx)?;
        if balance < cost_cents {
            return Err(StoreError::InsufficientFunds { needed: cost_cents, available: balance });
        }
        for item in &record.items {
            match item {
                TxItem::Card { uuid, set_code, foil, quantity, .. } => {
                    tx.execute(
                        "INSERT INTO collection (card_uuid, foil, set_code, quantity, first_acquired_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)
                         ON CONFLICT (card_uuid, foil) DO UPDATE SET quantity = quantity + excluded.quantity",
                        params![uuid, foil, set_code, quantity, now],
                    )?;
                }
                TxItem::Boosters { set_code, kind, quantity, cost_cents, .. } => {
                    tx.execute(
                        "INSERT INTO sealed (set_code, kind, quantity, cost_cents) VALUES (?1, ?2, ?3, ?4)
                         ON CONFLICT (set_code, kind) DO UPDATE SET
                            quantity = quantity + excluded.quantity,
                            cost_cents = cost_cents + excluded.cost_cents",
                        params![set_code, kind, quantity, cost_cents],
                    )?;
                }
            }
        }
        add_ledger(&tx, now, "sale_undone", -cost_cents, Some(&id.to_string()))?;
        tx.execute("UPDATE transactions SET undone_at = ?2, undo_cents = ?3 WHERE id = ?1", params![id, now, -cost_cents])?;
        tx.commit()?;
        Ok(())
    }

    /// Undoes a purchase while its boosters are still sealed: they go back to the Boosterium
    /// and the player is refunded `refund_cents`.
    pub fn undo_purchase(&mut self, id: i64, refund_cents: i64, now: i64) -> StoreResult<()> {
        let tx = self.conn.transaction()?;
        let record = transaction_of(&tx, id)?.ok_or(StoreError::CannotUndo("unknown transaction"))?;
        if record.kind != "booster_purchase" {
            return Err(StoreError::CannotUndo("not a purchase"));
        }
        if record.undone_at.is_some() {
            return Err(StoreError::CannotUndo("already undone"));
        }
        for item in &record.items {
            if let TxItem::Boosters { set_code, kind, quantity, .. } = item {
                take_sealed(&tx, set_code, kind, *quantity).map_err(|e| match e {
                    StoreError::NoSealedBooster => StoreError::CannotUndo("the boosters were opened or sold"),
                    other => other,
                })?;
            }
        }
        add_ledger(&tx, now, "purchase_undone", refund_cents, Some(&id.to_string()))?;
        tx.execute("UPDATE transactions SET undone_at = ?2, undo_cents = ?3 WHERE id = ?1", params![id, now, refund_cents])?;
        tx.commit()?;
        Ok(())
    }
}

/// Removes `count` boosters from a stack and returns the cost basis they carried
/// (proportional share of the stack's cost; the last booster takes the remainder).
fn take_sealed(tx: &Transaction<'_>, set_code: &str, kind: &str, count: u32) -> StoreResult<i64> {
    let row: Option<(u32, i64)> = tx
        .query_row(
            "SELECT quantity, cost_cents FROM sealed WHERE set_code = ?1 AND kind = ?2",
            params![set_code, kind],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (quantity, cost) = match row {
        Some((quantity, cost)) if count > 0 && quantity >= count => (quantity, cost),
        _ => return Err(StoreError::NoSealedBooster),
    };
    let taken = if count == quantity { cost } else { cost * i64::from(count) / i64::from(quantity) };
    tx.execute(
        "UPDATE sealed SET quantity = quantity - ?3, cost_cents = cost_cents - ?4 WHERE set_code = ?1 AND kind = ?2",
        params![set_code, kind, count, taken],
    )?;
    Ok(taken)
}

fn balance_of(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("SELECT COALESCE(SUM(amount_cents), 0) FROM ledger", [], |r| r.get(0))
}

/// Takes copies out of the collection, failing if any is not owned. Returns what they are
/// worth at the prices given, how many there were, and their detail.
fn take_cards(conn: &Connection, sales: &[Sale<'_>]) -> StoreResult<(i64, u32, Vec<TxItem>)> {
    let mut total = 0i64;
    let mut count = 0u32;
    let mut items = Vec::with_capacity(sales.len());
    for s in sales {
        let owned: Option<(u32, String)> = conn
            .query_row(
                "SELECT quantity, set_code FROM collection WHERE card_uuid = ?1 AND foil = ?2",
                params![s.uuid, s.foil],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((_, set_code)) = &owned {
            items.push(TxItem::Card {
                uuid: s.uuid.to_string(),
                set_code: set_code.clone(),
                foil: s.foil,
                quantity: s.quantity,
                unit_cents: s.unit_cents,
            });
        }
        match owned.map(|(q, _)| q) {
            Some(q) if q > s.quantity => {
                conn.execute(
                    "UPDATE collection SET quantity = quantity - ?3 WHERE card_uuid = ?1 AND foil = ?2",
                    params![s.uuid, s.foil, s.quantity],
                )?;
            }
            Some(q) if q == s.quantity => {
                conn.execute("DELETE FROM collection WHERE card_uuid = ?1 AND foil = ?2", params![s.uuid, s.foil])?;
            }
            _ => return Err(StoreError::NotOwned(s.uuid.to_string())),
        }
        total += i64::from(s.unit_cents) * i64::from(s.quantity);
        count += s.quantity;
    }
    Ok((total, count, items))
}

fn add_ledger(conn: &Connection, at: i64, kind: &str, amount_cents: i64, details: Option<&str>) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO ledger (at, kind, amount_cents, details) VALUES (?1, ?2, ?3, ?4)",
        params![at, kind, amount_cents, details],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Records a purchase or a sale in detail, next to its wallet movement.
fn add_transaction(conn: &Connection, ledger_id: i64, at: i64, kind: &str, amount_cents: i64, items: &[TxItem]) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO transactions (ledger_id, at, kind, amount_cents, items) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![ledger_id, at, kind, amount_cents, serde_json::to_string(items)?],
    )?;
    Ok(())
}

fn transaction_of(conn: &Connection, id: i64) -> StoreResult<Option<TxRecord>> {
    let row = conn
        .query_row(
            "SELECT id, at, kind, amount_cents, items, undone_at, undo_cents FROM transactions WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, String>(4)?, r.get(5)?, r.get(6)?)),
        )
        .optional()?;
    row.map(|(id, at, kind, amount_cents, items, undone_at, undo_cents)| {
        Ok(TxRecord { id, at, kind, amount_cents, items: serde_json::from_str(&items)?, undone_at, undo_cents })
    })
    .transpose()
}

fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
        let tx: Transaction<'_> = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn price_history_keeps_one_point_per_snapshot_oldest_first() {
        let mut store = Store::open_in_memory().unwrap();
        let point = |cents: u32| vec![("TST".to_string(), "play".to_string(), cents)];
        store.record_booster_prices("d1", 100, &point(700)).unwrap();
        store.record_booster_prices("d2", 200, &point(900)).unwrap();
        // Recording a snapshot twice does not duplicate or overwrite it.
        store.record_booster_prices("d2", 300, &point(1)).unwrap();
        store.record_booster_prices("d3", 400, &point(800)).unwrap();
        assert_eq!(store.booster_price_history("TST", "play", 30).unwrap(), [700, 900, 800]);
        assert_eq!(store.booster_price_history("TST", "play", 2).unwrap(), [900, 800]);
        assert!(store.booster_price_history("TST", "draft", 30).unwrap().is_empty());
    }

    fn card<'a>(uuid: &'a str, foil: bool, price: Option<u32>) -> OpeningCard<'a> {
        OpeningCard { uuid, set_code: "TST", foil, price_cents: price, limited: false }
    }

    #[test]
    fn limited_pulls_only_ever_grow() {
        let mut store = store_with(2);
        let ring = OpeningCard { limited: true, ..card("ring", false, None) };
        store.open_sealed("TST", "play", &[card("a", false, Some(10)), ring], 1).unwrap();
        assert_eq!(store.limited_pulls().unwrap(), HashMap::from([("ring".to_string(), 1)]));
        // Selling it changes nothing: it was pulled.
        store.sell_cards(&[Sale { uuid: "ring", foil: false, quantity: 1, unit_cents: 0 }], 2).unwrap();
        let ring = OpeningCard { limited: true, ..card("ring", false, None) };
        store.open_sealed("TST", "play", &[ring], 3).unwrap();
        assert_eq!(store.limited_pulls().unwrap()["ring"], 2);
    }

    /// A store with $100.00 and `boosters` free sealed boosters of TST/play.
    fn store_with(boosters: u32) -> Store {
        let mut store = Store::open_in_memory().unwrap();
        store.ensure_starting_balance(10_000, 0).unwrap();
        if boosters > 0 {
            store.buy_boosters("TST", "play", boosters, 0, 0).unwrap();
        }
        store
    }

    #[test]
    fn undoing_a_card_sale_gives_the_card_back_once_at_a_cost() {
        let mut store = store_with(1);
        store.open_sealed("TST", "play", &[card("a", true, Some(500))], 1).unwrap();
        store.sell_cards(&[Sale { uuid: "a", foil: true, quantity: 1, unit_cents: 400 }], 2).unwrap();
        assert!(store.owned_in_set("TST").unwrap().is_empty());

        let history = store.history(10, 0).unwrap();
        let sale = history[0].transaction.clone().unwrap();
        assert_eq!((sale.kind.as_str(), sale.amount_cents), ("card_sale", 400));
        assert_eq!(
            sale.items,
            [TxItem::Card { uuid: "a".into(), set_code: "TST".into(), foil: true, quantity: 1, unit_cents: 400 }]
        );

        let before = store.balance().unwrap();
        store.undo_sale(sale.id, 420, 3).unwrap();
        assert_eq!(store.balance().unwrap(), before - 420);
        assert_eq!(store.owned_in_set("TST").unwrap()["a"], Owned { normal: 0, foil: 1 });
        assert!(matches!(store.undo_sale(sale.id, 420, 4), Err(StoreError::CannotUndo(_))));
        let undone = store.transaction(sale.id).unwrap().unwrap();
        assert_eq!((undone.undone_at, undone.undo_cents), (Some(3), Some(-420)));
        assert_eq!(store.history(10, 0).unwrap()[0].kind, "sale_undone");
    }

    #[test]
    fn undoing_needs_the_money_and_sealed_boosters() {
        let mut store = store_with(0);
        store.buy_boosters("TST", "play", 2, 300, 1).unwrap();
        let purchase = store.history(1, 0).unwrap()[0].transaction.clone().unwrap();
        store.open_sealed("TST", "play", &[card("a", false, None)], 2).unwrap();
        // One of the two boosters was opened: the purchase cannot be undone any more.
        assert!(matches!(store.undo_purchase(purchase.id, 570, 3), Err(StoreError::CannotUndo(_))));

        store.sell_boosters("TST", "play", 1, 240, 4).unwrap();
        let sale = store.history(1, 0).unwrap()[0].transaction.clone().unwrap();
        // The sold booster carried half of the 600 paid.
        assert!(matches!(&sale.items[0], TxItem::Boosters { cost_cents: 300, .. }));
        assert!(matches!(store.undo_sale(sale.id, 1_000_000, 5), Err(StoreError::InsufficientFunds { .. })));
        store.undo_sale(sale.id, 252, 5).unwrap();
        assert_eq!(store.sealed_stacks().unwrap()[0], SealedRow { set_code: "TST".into(), kind: "play".into(), quantity: 1, cost_cents: 300 });
    }

    #[test]
    fn a_purchase_undone_returns_its_boosters_and_refunds() {
        let mut store = store_with(0);
        store.buy_boosters("TST", "play", 2, 300, 1).unwrap();
        let purchase = store.history(1, 0).unwrap()[0].transaction.clone().unwrap();
        let before = store.balance().unwrap();
        store.undo_purchase(purchase.id, 570, 2).unwrap();
        assert_eq!(store.balance().unwrap(), before + 570);
        assert!(store.sealed_stacks().unwrap().is_empty());
    }

    #[test]
    fn statistics_summarize_the_opening_history() {
        let mut store = store_with(3);
        store.open_sealed("TST", "play", &[card("a", false, Some(50)), card("b", true, Some(900))], 10).unwrap();
        store.open_sealed("TST", "play", &[card("c", false, None)], 20).unwrap();
        store.open_sealed("TST", "play", &[card("a", false, Some(50)), card("d", false, Some(300))], 30).unwrap();

        assert_eq!(store.opening_totals().unwrap(), OpeningTotals { boosters: 3, cards: 5, foils: 1, value_cents: 1_300 });

        let latest = store.openings(OpeningOrder::Latest, 2).unwrap();
        assert_eq!(latest.iter().map(|o| o.opened_at).collect::<Vec<_>>(), [30, 20]);
        assert_eq!(latest[0].best_card, Some(PullRef { uuid: "d".into(), foil: false, price_cents: 300 }));
        assert_eq!(latest[1].best_card, None);
        let best = &store.openings(OpeningOrder::MostValuable, 1).unwrap()[0];
        assert_eq!(best.value_cents, 950);
        assert_eq!(best.best_card, Some(PullRef { uuid: "b".into(), foil: true, price_cents: 900 }));
        assert_eq!(store.openings(OpeningOrder::LeastValuable, 1).unwrap()[0].value_cents, 0);

        let pulls = store.best_pulls(2).unwrap();
        assert_eq!(pulls.iter().map(|p| (p.uuid.as_str(), p.price_cents)).collect::<Vec<_>>(), [("b", 900), ("d", 300)]);

        assert_eq!(
            store.openings_by_booster().unwrap(),
            [BoosterKindTotals {
                set_code: "TST".into(),
                kind: "play".into(),
                boosters: 3,
                value_cents: 1_300,
                best_value_cents: 950
            }]
        );
        let ledger = store.ledger_totals().unwrap();
        assert_eq!((ledger["starting_balance"], ledger["booster_purchase"]), (10_000, 0));
    }

    #[test]
    fn statistics_of_a_new_player_are_empty() {
        let store = store_with(0);
        assert_eq!(store.opening_totals().unwrap(), OpeningTotals::default());
        assert!(store.openings(OpeningOrder::Latest, 10).unwrap().is_empty());
        assert!(store.best_pulls(10).unwrap().is_empty());
        assert!(store.openings_by_booster().unwrap().is_empty());
    }

    #[test]
    fn flags_new_discoveries_including_within_the_same_booster() {
        let mut store = store_with(3);
        let flags = store
            .open_sealed("TST", "play", &[card("a", false, None), card("a", false, None), card("b", false, None)], 1)
            .unwrap()
            .is_new;
        assert_eq!(flags, [true, false, true]);

        // A foil copy of an owned card is not a new discovery, but a new foil (once).
        let outcome = store
            .open_sealed("TST", "play", &[card("a", true, None), card("c", true, None), card("a", true, None)], 2)
            .unwrap();
        assert_eq!(outcome.is_new, [false, true, false]);
        assert_eq!(outcome.new_foil, [true, false, false]);

        // Owned in foil already: no new foil; a non-foil copy never is one.
        let outcome = store.open_sealed("TST", "play", &[card("a", true, None), card("b", false, None)], 3).unwrap();
        assert_eq!(outcome.new_foil, [false, false]);
    }

    #[test]
    fn counts_normal_and_foil_copies_separately() {
        let mut store = store_with(1);
        store
            .open_sealed("TST", "play", &[card("a", false, None), card("a", false, None), card("a", true, None)], 1)
            .unwrap();
        let owned = store.owned_in_set("TST").unwrap();
        assert_eq!(owned["a"], Owned { normal: 2, foil: 1 });
        assert!(store.owned_in_set("OTHER").unwrap().is_empty());
    }

    #[test]
    fn artifacts_add_to_bets_won_and_give_back_one_lost_stake_a_day() {
        let mut store = store_with(3);
        let bet = |won: bool, stake: u32| PlacedBet { bet: "{}".into(), stake_cents: stake, probability: 0.5, won, payout_cents: if won { 400 } else { 0 } };
        let start = store.balance().unwrap();
        // One booster: a bet won (400) and two lost (100 and 300).
        store.open_sealed_with_bets("TST", "play", &[card("a", false, Some(25))], &[bet(true, 200), bet(false, 100), bet(false, 300)], 10).unwrap();
        assert_eq!(store.balance().unwrap(), start - 600);
        // +10 % on the win, and the larger lost stake back: 440 + 300.
        assert_eq!(store.pay_bets(20, 10, Some("2026-10-21")).unwrap(), 740);
        // Another lost bet the same day: nothing back.
        store.open_sealed_with_bets("TST", "play", &[card("a", false, Some(25))], &[bet(false, 100)], 30).unwrap();
        assert_eq!(store.pay_bets(40, 10, Some("2026-10-21")).unwrap(), 0);
        // The next day, again.
        store.open_sealed_with_bets("TST", "play", &[card("a", false, Some(25))], &[bet(false, 100)], 50).unwrap();
        assert_eq!(store.pay_bets(60, 10, Some("2026-10-22")).unwrap(), 100);
    }

    #[test]
    fn an_artifact_is_owned_once_and_bought_with_the_balance() {
        let mut store = store_with(0);
        assert!(store.add_artifact("bronze", "booster", 5).unwrap());
        assert!(!store.add_artifact("bronze", "booster", 6).unwrap());
        assert!(store.buy_artifact("loupe", 20_000, 7).is_err(), "too expensive");
        store.buy_artifact("loupe", 4_000, 8).unwrap();
        assert!(store.buy_artifact("loupe", 4_000, 9).is_err(), "already owned");
        assert_eq!(store.balance().unwrap(), 6_000);
        let owned: Vec<(String, String)> = store.artifacts().unwrap().into_iter().map(|(id, _, source)| (id, source)).collect();
        assert_eq!(owned, [("bronze".to_string(), "booster".to_string()), ("loupe".to_string(), "shop".to_string())]);
    }

    #[test]
    fn records_the_opening_history() {
        let mut store = store_with(1);
        store.open_sealed("TST", "play", &[card("a", false, Some(25)), card("b", true, Some(100)), card("c", false, None)], 42).unwrap();
        let (kind, value, cards): (String, i64, i64) = store
            .conn
            .query_row(
                "SELECT booster_kind, value_cents, (SELECT COUNT(*) FROM opening_cards) FROM openings",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!((kind.as_str(), value, cards), ("play", 125, 3));
    }

    #[test]
    fn starting_balance_is_granted_once() {
        let mut store = Store::open_in_memory().unwrap();
        store.ensure_starting_balance(10_000, 0).unwrap();
        store.ensure_starting_balance(10_000, 1).unwrap();
        assert_eq!(store.balance().unwrap(), 10_000);
    }

    #[test]
    fn buying_boosters_spends_money_and_stocks_them() {
        let mut store = store_with(0);
        assert_eq!(store.buy_boosters("TST", "play", 3, 400, 1).unwrap(), 8_800);
        store.buy_boosters("TST", "play", 2, 400, 2).unwrap();
        assert_eq!(store.balance().unwrap(), 8_000);
        assert_eq!(
            store.sealed_stacks().unwrap(),
            [SealedRow { set_code: "TST".into(), kind: "play".into(), quantity: 5, cost_cents: 2_000 }]
        );
    }

    #[test]
    fn sealed_boosters_keep_a_cost_basis_that_follows_opening_and_selling() {
        let mut store = store_with(0);
        store.buy_boosters("TST", "play", 4, 500, 1).unwrap(); // cost 2000
        store.buy_boosters("TST", "play", 1, 1_000, 2).unwrap(); // cost 3000 for 5: 600 each
        let opened = store.open_sealed("TST", "play", &[card("a", false, None)], 3).unwrap();
        assert_eq!(opened.cost_cents, 600);

        // 4 left for 2400: selling 2 takes half of the cost.
        let sale = store.sell_boosters("TST", "play", 2, 900, 4).unwrap();
        assert_eq!((sale.total_cents, sale.cost_cents), (1_800, 1_200));
        let rows = store.sealed_stacks().unwrap();
        assert_eq!((rows[0].quantity, rows[0].cost_cents), (2, 1_200));
        assert_eq!(store.balance().unwrap(), 10_000 - 3_000 + 1_800);

        // Selling the last ones takes the whole remaining cost.
        store.sell_boosters("TST", "play", 2, 0, 5).unwrap();
        assert!(store.sealed_stacks().unwrap().is_empty());
    }

    #[test]
    fn cannot_sell_boosters_you_do_not_have() {
        let mut store = store_with(1);
        assert!(matches!(store.sell_boosters("TST", "play", 2, 100, 1), Err(StoreError::NoSealedBooster)));
        assert!(matches!(store.sell_boosters("TST", "play", 0, 100, 1), Err(StoreError::NoSealedBooster)));
        assert!(matches!(store.sell_boosters("TST", "collector", 1, 100, 1), Err(StoreError::NoSealedBooster)));
        assert_eq!(store.sealed_stacks().unwrap().len(), 1);
    }

    #[test]
    fn cannot_buy_more_than_the_balance_allows() {
        let mut store = store_with(0);
        let err = store.buy_boosters("TST", "play", 26, 400, 1).unwrap_err();
        assert!(matches!(err, StoreError::InsufficientFunds { needed: 10_400, available: 10_000 }));
        assert_eq!(store.balance().unwrap(), 10_000);
        assert!(store.sealed_stacks().unwrap().is_empty());
    }

    #[test]
    fn opening_consumes_a_sealed_booster_and_fails_without_one() {
        let mut store = store_with(1);
        store.open_sealed("TST", "play", &[card("a", false, None)], 1).unwrap();
        assert!(store.sealed_stacks().unwrap().is_empty());
        let err = store.open_sealed("TST", "play", &[card("b", false, None)], 2).unwrap_err();
        assert!(matches!(err, StoreError::NoSealedBooster));
        // The failed opening left nothing behind.
        assert!(!store.owned_in_set("TST").unwrap().contains_key("b"));
    }

    #[test]
    fn selling_pays_the_player_and_removes_the_last_copy() {
        let mut store = store_with(1);
        store.open_sealed("TST", "play", &[card("a", false, None), card("a", false, None), card("b", true, None)], 1).unwrap();
        let paid = store
            .sell_cards(&[Sale { uuid: "a", foil: false, quantity: 1, unit_cents: 200 }], 2)
            .unwrap();
        assert_eq!((paid, store.balance().unwrap()), (200, 10_200));
        assert_eq!(store.owned_in_set("TST").unwrap()["a"].normal, 1);

        store.sell_cards(&[Sale { uuid: "a", foil: false, quantity: 1, unit_cents: 200 }], 3).unwrap();
        assert!(!store.owned_in_set("TST").unwrap().contains_key("a"));
        assert_eq!(store.owned_cards().unwrap(), [("b".to_string(), true, 1)]);
    }

    #[test]
    fn a_failed_sale_changes_nothing() {
        let mut store = store_with(1);
        store.open_sealed("TST", "play", &[card("a", false, None)], 1).unwrap();
        let err = store
            .sell_cards(
                &[
                    Sale { uuid: "a", foil: false, quantity: 1, unit_cents: 100 },
                    Sale { uuid: "ghost", foil: false, quantity: 1, unit_cents: 100 },
                ],
                2,
            )
            .unwrap_err();
        assert!(matches!(err, StoreError::NotOwned(_)));
        assert_eq!(store.balance().unwrap(), 10_000);
        assert_eq!(store.owned_in_set("TST").unwrap()["a"].normal, 1);
    }

    #[test]
    fn charity_claims_are_counted_per_day() {
        let mut store = store_with(0);
        assert_eq!(store.charity_claims_on("2026-10-13").unwrap(), 0);
        store.claim_charity(2_000, "2026-10-13", 1).unwrap();
        assert_eq!(store.charity_claims_on("2026-10-13").unwrap(), 1);
        assert_eq!(store.charity_claims_on("2026-10-14").unwrap(), 0);
        assert_eq!(store.balance().unwrap(), 12_000);
    }

    #[test]
    fn progress_round_trips_and_rewards_are_paid_exactly_once() {
        let mut store = store_with(0);
        let mut progress = Progress::default();
        progress.metrics.insert("boosters_opened".into(), 10);
        progress.seen.entry("sets_opened".into()).or_default().insert("\"BLB\"".into());
        progress.unlocked.insert("open-10".into(), 5);
        let reward = GrantedReward {
            achievement_id: "open-10".into(),
            cash_cents: 500,
            sealed: Some(("TST".into(), "play".into(), 2)),
        };

        store.save_progress(&progress, std::slice::from_ref(&reward), 5).unwrap();
        assert_eq!(store.load_progress().unwrap(), progress);
        assert_eq!(store.balance().unwrap(), 10_500);
        let stacks = store.sealed_stacks().unwrap();
        assert_eq!((stacks[0].quantity, stacks[0].cost_cents), (2, 0));

        // Saving again (even with the reward listed again) pays nothing more.
        progress.metrics.insert("boosters_opened".into(), 11);
        store.save_progress(&progress, &[reward], 6).unwrap();
        assert_eq!(store.balance().unwrap(), 10_500);
        assert_eq!(store.sealed_stacks().unwrap()[0].quantity, 2);
        assert_eq!(store.load_progress().unwrap().metrics["boosters_opened"], 11);
    }

    #[test]
    fn persists_across_reopen_and_migrates_only_once() {
        let dir = std::env::temp_dir().join(format!("unsealed-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.sqlite");
        let _ = std::fs::remove_file(&path);

        let mut first = Store::open(&path).unwrap();
        first.ensure_starting_balance(10_000, 0).unwrap();
        first.buy_boosters("TST", "play", 1, 400, 1).unwrap();
        first.open_sealed("TST", "play", &[card("a", false, None)], 2).unwrap();
        drop(first);

        let store = Store::open(&path).unwrap();
        assert_eq!(store.owned_in_set("TST").unwrap()["a"].normal, 1);
        assert_eq!(store.balance().unwrap(), 9_600);
        let version: i64 = store.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
    }

    #[test]
    fn upgrades_a_version_1_database_keeping_the_collection() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATIONS[0]).unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        conn.execute("INSERT INTO collection VALUES ('a', 0, 'TST', 3, 0)", []).unwrap();
        migrate(&mut conn).unwrap();
        let store = Store { conn };
        assert_eq!(store.owned_in_set("TST").unwrap()["a"].normal, 3);
        assert_eq!(store.balance().unwrap(), 0);
    }
}
