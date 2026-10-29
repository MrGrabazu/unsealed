//! The seal of a save: a fingerprint of what is worth something in it (the wallet, the cards,
//! the sealed boosters, the artifacts, the bets, the undoable transactions, the achievements and
//! box toppers already given), kept in the save itself (`seal` table) and checked when a game
//! is loaded or imported: a save edited by hand (more Seals, more cards) is refused.
//!
//! A save is written at every action, so the fingerprint is kept up to date by SQLite itself:
//! triggers on each table fold every row inserted, deleted or changed into its table's digest
//! (an XOR of keyed hashes of the rows: the order of the changes does not matter, and the digest
//! of the table as it stands can be computed again). The triggers call `unsealed_seal`, a
//! function only the game defines: in another SQLite tool, writing to these tables fails; with
//! the triggers removed, the digest no longer matches the rows.
//!
//! A deterrent, not a lock: the game's code is open, its key with it. It stops editing a save
//! with an SQLite browser, not someone who reads the code.
//!
//! Never `INSERT OR REPLACE` into a sealed table: the row it deletes fires no trigger, the
//! digest would go wrong and an honest player be refused. `ON CONFLICT DO UPDATE` and
//! `INSERT OR IGNORE` are fine. A new table worth something goes into `TABLES`.

use rusqlite::functions::FunctionFlags;
use rusqlite::types::ValueRef;
use rusqlite::{params, Connection, OptionalExtension};

/// Mixed into every hash, so that a digest cannot be worked out without the game.
const KEY: &[u8] = b"unsealed/seal/v1:crack&collect:3f9c1e77a2d4";

/// The tables sealed and the columns that matter in each.
const TABLES: &[(&str, &[&str])] = &[
    ("collection", &["card_uuid", "foil", "set_code", "quantity"]),
    ("ledger", &["id", "at", "kind", "amount_cents"]),
    ("sealed", &["set_code", "kind", "quantity", "cost_cents"]),
    ("artifacts", &["id", "acquired_at", "source"]),
    ("transactions", &["id", "ledger_id", "amount_cents", "items", "undone_at", "undo_cents"]),
    ("bets", &["id", "opening_id", "stake_cents", "won", "payout_cents", "paid_at"]),
    ("ach_unlocked", &["id", "unlocked_at"]),
    ("box_toppers", &["set_code", "kind", "boxes"]),
];

/// The SQL function the triggers call: `unsealed_seal(digest, table, values…)`.
const FUNCTION: &str = "unsealed_seal";

/// Key of `save_meta`: the save has been sealed once (a seal missing after that was removed).
const SEALED: &str = "sealed";

/// A row's keyed hash (FNV-1a, 64 bits: stable from one version of Rust to the next).
fn row_hash<'a>(table: &str, values: impl Iterator<Item = ValueRef<'a>>) -> i64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for &b in bytes {
            hash = (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    };
    feed(KEY);
    feed(table.as_bytes());
    for value in values {
        match value {
            ValueRef::Null => feed(b"\x00"),
            ValueRef::Integer(i) => {
                feed(b"\x01");
                feed(&i.to_le_bytes());
            }
            ValueRef::Real(f) => {
                feed(b"\x02");
                feed(&f.to_bits().to_le_bytes());
            }
            ValueRef::Text(t) => {
                feed(b"\x03");
                feed(t);
            }
            ValueRef::Blob(b) => {
                feed(b"\x04");
                feed(b);
            }
        }
        feed(b"\xff");
    }
    hash as i64
}

/// Defines `unsealed_seal` on a connection; done before anything is written.
pub fn register(conn: &Connection) -> rusqlite::Result<()> {
    conn.create_scalar_function(FUNCTION, -1, FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC, |ctx| {
        let digest: i64 = ctx.get(0)?;
        let table: String = ctx.get(1)?;
        Ok(digest ^ row_hash(&table, (2..ctx.len()).map(|i| ctx.get_raw(i))))
    })
}

/// The digest of a table as it stands.
fn digest_of(conn: &Connection, table: &str, columns: &[&str]) -> rusqlite::Result<i64> {
    let mut stmt = conn.prepare(&format!("SELECT {} FROM {table}", columns.join(", ")))?;
    let mut rows = stmt.query([])?;
    let mut digest = 0i64;
    while let Some(row) = rows.next()? {
        digest ^= row_hash(table, (0..columns.len()).map(|i| row.get_ref(i).unwrap_or(ValueRef::Null)));
    }
    Ok(digest)
}

/// Puts the seal in place: its table and triggers (again at every opening: removed, they come
/// back, and the digest then no longer matches), and the digests of a save never sealed yet (a
/// game of an older version: what it holds is taken as it is).
pub fn install(conn: &Connection) -> rusqlite::Result<()> {
    let mut sql = String::from("CREATE TABLE IF NOT EXISTS seal (name TEXT PRIMARY KEY, digest INTEGER NOT NULL) WITHOUT ROWID;");
    for (table, columns) in TABLES {
        let row = |of: &str| columns.iter().map(|c| format!("{of}.{c}")).collect::<Vec<_>>().join(", ");
        let fold = |digest: &str, of: &str| format!("{FUNCTION}({digest}, '{table}', {})", row(of));
        sql.push_str(&format!(
            "CREATE TRIGGER IF NOT EXISTS seal_{table}_insert AFTER INSERT ON {table} BEGIN
               UPDATE seal SET digest = {} WHERE name = '{table}'; END;
             CREATE TRIGGER IF NOT EXISTS seal_{table}_delete AFTER DELETE ON {table} BEGIN
               UPDATE seal SET digest = {} WHERE name = '{table}'; END;
             CREATE TRIGGER IF NOT EXISTS seal_{table}_update AFTER UPDATE ON {table} BEGIN
               UPDATE seal SET digest = {} WHERE name = '{table}'; END;",
            fold("digest", "NEW"),
            fold("digest", "OLD"),
            fold(&fold("digest", "OLD"), "NEW"),
        ));
    }
    conn.execute_batch(&sql)?;
    let sealed: Option<String> = conn.query_row("SELECT value FROM save_meta WHERE key = ?1", [SEALED], |r| r.get(0)).optional()?;
    if sealed.is_none() {
        for (table, columns) in TABLES {
            let digest = digest_of(conn, table, columns)?;
            conn.execute("INSERT OR REPLACE INTO seal (name, digest) VALUES (?1, ?2)", params![table, digest])?;
        }
        conn.execute("INSERT INTO save_meta (key, value) VALUES (?1, '1')", [SEALED])?;
    }
    Ok(())
}

/// Whether the save is as the game left it: every table's digest matches its rows.
pub fn intact(conn: &Connection) -> rusqlite::Result<bool> {
    for (table, columns) in TABLES {
        let stored: Option<i64> = conn.query_row("SELECT digest FROM seal WHERE name = ?1", [table], |r| r.get(0)).optional()?;
        if stored != Some(digest_of(conn, table, columns)?) {
            return Ok(false);
        }
    }
    Ok(true)
}
