//! SQLite persistence: wallet ledger, sealed boosters, collection and opening history.
//!
//! The schema is versioned with `PRAGMA user_version`; to change it, append a new
//! entry to `MIGRATIONS` (never edit an existing one). Every operation that touches
//! several tables runs in a single transaction.
//!
//! The wallet is a ledger: the balance is the sum of its rows, so it can never drift
//! from the history that explains it.

use rusqlite::{params, Connection, OptionalExtension, Transaction};
use std::collections::HashMap;
use std::path::Path;

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
];

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("not enough money: {needed} needed, {available} available (in cents)")]
    InsufficientFunds { needed: i64, available: i64 },
    #[error("no sealed booster of that kind")]
    NoSealedBooster,
    #[error("not enough copies of card {0}")]
    NotOwned(String),
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
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

/// Result of opening a sealed booster.
#[derive(Debug)]
pub struct OpenOutcome {
    /// For each card, whether it is a new discovery.
    pub is_new: Vec<bool>,
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
        migrate(&mut conn)?;
        Ok(Self { conn })
    }

    // ---- wallet -------------------------------------------------------------

    pub fn balance(&self) -> StoreResult<i64> {
        Ok(balance_of(&self.conn)?)
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
        add_ledger(&tx, now, "booster_purchase", -total, Some(&format!("{set_code}/{kind} x{quantity}")))?;
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
        add_ledger(&tx, now, "booster_sale", total_cents, Some(&format!("{set_code}/{kind} x{quantity}")))?;
        tx.commit()?;
        Ok(BoosterSale { total_cents, cost_cents })
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
    pub fn open_sealed(
        &mut self,
        set_code: &str,
        booster_kind: &str,
        cards: &[OpeningCard<'_>],
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
        for (position, c) in cards.iter().enumerate() {
            let known: Option<i64> = tx
                .query_row("SELECT 1 FROM collection WHERE card_uuid = ?1 LIMIT 1", [c.uuid], |r| r.get(0))
                .optional()?;
            is_new.push(known.is_none());
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
        }
        tx.commit()?;
        Ok(OpenOutcome { is_new, cost_cents })
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
    pub fn owned_cards(&self) -> StoreResult<Vec<(String, bool, u32)>> {
        let mut stmt = self.conn.prepare("SELECT card_uuid, foil, quantity FROM collection")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Sells cards from the collection, atomically: all sales succeed or none does.
    /// Returns the total paid. Selling the last copy removes the card from the collection.
    pub fn sell_cards(&mut self, sales: &[Sale<'_>], now: i64) -> StoreResult<i64> {
        let tx = self.conn.transaction()?;
        let mut total = 0i64;
        let mut count = 0u32;
        for s in sales {
            let owned: Option<u32> = tx
                .query_row(
                    "SELECT quantity FROM collection WHERE card_uuid = ?1 AND foil = ?2",
                    params![s.uuid, s.foil],
                    |r| r.get(0),
                )
                .optional()?;
            match owned {
                Some(q) if q > s.quantity => {
                    tx.execute(
                        "UPDATE collection SET quantity = quantity - ?3 WHERE card_uuid = ?1 AND foil = ?2",
                        params![s.uuid, s.foil, s.quantity],
                    )?;
                }
                Some(q) if q == s.quantity => {
                    tx.execute(
                        "DELETE FROM collection WHERE card_uuid = ?1 AND foil = ?2",
                        params![s.uuid, s.foil],
                    )?;
                }
                _ => return Err(StoreError::NotOwned(s.uuid.to_string())),
            }
            total += i64::from(s.unit_cents) * i64::from(s.quantity);
            count += s.quantity;
        }
        add_ledger(&tx, now, "card_sale", total, Some(&format!("{count} cards")))?;
        tx.commit()?;
        Ok(total)
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

fn add_ledger(conn: &Connection, at: i64, kind: &str, amount_cents: i64, details: Option<&str>) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO ledger (at, kind, amount_cents, details) VALUES (?1, ?2, ?3, ?4)",
        params![at, kind, amount_cents, details],
    )?;
    Ok(())
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

    fn card<'a>(uuid: &'a str, foil: bool, price: Option<u32>) -> OpeningCard<'a> {
        OpeningCard { uuid, set_code: "TST", foil, price_cents: price }
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
    fn flags_new_discoveries_including_within_the_same_booster() {
        let mut store = store_with(2);
        let flags = store
            .open_sealed("TST", "play", &[card("a", false, None), card("a", false, None), card("b", false, None)], 1)
            .unwrap()
            .is_new;
        assert_eq!(flags, [true, false, true]);

        // A foil copy of an owned card is not a new discovery.
        let flags =
            store.open_sealed("TST", "play", &[card("a", true, None), card("c", true, None)], 2).unwrap().is_new;
        assert_eq!(flags, [false, true]);
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
