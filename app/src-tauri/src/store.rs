//! SQLite persistence: collection and opening history.
//!
//! The schema is versioned with `PRAGMA user_version`; to change it, append a new
//! entry to `MIGRATIONS` (never edit an existing one).

use rusqlite::{params, Connection, OptionalExtension};
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
];

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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Owned {
    pub normal: u32,
    pub foil: u32,
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::init(conn)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> rusqlite::Result<Self> {
        conn.pragma_update(None, "foreign_keys", true)?;
        migrate(&mut conn)?;
        Ok(Self { conn })
    }

    /// Records an opening and adds its cards to the collection, atomically.
    /// Returns, for each card, whether it is a new discovery: the player did not own
    /// that card (in any finish) before, including earlier cards of the same booster.
    pub fn record_opening(
        &mut self,
        set_code: &str,
        booster_kind: &str,
        cards: &[OpeningCard<'_>],
        now: i64,
    ) -> rusqlite::Result<Vec<bool>> {
        let tx = self.conn.transaction()?;
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
        Ok(is_new)
    }

    /// Owned quantities of the cards belonging to a set, by card uuid.
    pub fn owned_in_set(&self, set_code: &str) -> rusqlite::Result<HashMap<String, Owned>> {
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
}

fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
        let tx = conn.transaction()?;
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

    #[test]
    fn flags_new_discoveries_including_within_the_same_booster() {
        let mut store = Store::open_in_memory().unwrap();
        let flags = store
            .record_opening("TST", "play", &[card("a", false, None), card("a", false, None), card("b", false, None)], 1)
            .unwrap();
        assert_eq!(flags, [true, false, true]);

        // A foil copy of an owned card is not a new discovery.
        let flags = store.record_opening("TST", "play", &[card("a", true, None), card("c", true, None)], 2).unwrap();
        assert_eq!(flags, [false, true]);
    }

    #[test]
    fn counts_normal_and_foil_copies_separately() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .record_opening("TST", "play", &[card("a", false, None), card("a", false, None), card("a", true, None)], 1)
            .unwrap();
        let owned = store.owned_in_set("TST").unwrap();
        assert_eq!(owned["a"], Owned { normal: 2, foil: 1 });
        assert!(store.owned_in_set("OTHER").unwrap().is_empty());
    }

    #[test]
    fn records_the_opening_history() {
        let mut store = Store::open_in_memory().unwrap();
        store.record_opening("TST", "play", &[card("a", false, Some(25)), card("b", true, Some(100)), card("c", false, None)], 42).unwrap();
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
    fn persists_across_reopen_and_migrates_only_once() {
        let dir = std::env::temp_dir().join(format!("unsealed-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.sqlite");
        let _ = std::fs::remove_file(&path);

        Store::open(&path).unwrap().record_opening("TST", "play", &[card("a", false, None)], 1).unwrap();
        let store = Store::open(&path).unwrap();
        assert_eq!(store.owned_in_set("TST").unwrap()["a"].normal, 1);
        let version: i64 = store.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
    }
}
