//! Games and their saves. Each game is its own SQLite file in the saves directory, so
//! several players can each have their collection on the same machine, and a player can
//! start over without losing anything.
//!
//! Saving is automatic: every action is committed as it happens. A *save point* is a
//! manual copy of a game under a new name, to come back to later.
//!
//! A save describes itself (name, creation and last played dates) in its `save_meta`
//! table, so the list is rebuilt by reading the files: no index can drift from them.

use crate::store::Store;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const EXTENSION: &str = "sqlite";
const NAME: &str = "name";
const CREATED_AT: &str = "created_at";
const LAST_PLAYED_AT: &str = "last_played_at";
/// Longest game name kept.
const MAX_NAME_CHARS: usize = 60;
/// Extension of an exported game: a copy of its SQLite file, to take to another computer.
const EXPORT_EXTENSION: &str = "unsealed";
/// How every SQLite file starts.
const SQLITE_HEADER: &[u8] = b"SQLite format 3\0";
/// The error of a file to import that is not a game (the interface translates it).
const NOT_A_SAVE: &str = "not an Unsealed save";
/// The error of a save edited outside the game (see `seal`; the interface translates it).
const TAMPERED: &str = "this save was edited outside the game";
/// Left in the saves directory when such a save was found, for the next game to get its
/// secret achievement (Smart Boy).
const CAUGHT_FILE: &str = "caught";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SaveInfo {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub last_played_at: i64,
}

pub struct Saves {
    dir: PathBuf,
}

impl Saves {
    pub fn new(dir: PathBuf) -> std::io::Result<Self> {
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    /// Every readable save, the last played first.
    pub fn list(&self) -> Vec<SaveInfo> {
        let mut saves: Vec<SaveInfo> = fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                let id = path.file_stem()?.to_str()?.to_string();
                (path.extension()? == EXTENSION && valid_id(&id)).then_some(id)
            })
            .filter_map(|id| self.info(&id).ok())
            .collect();
        saves.sort_by(|a, b| b.last_played_at.cmp(&a.last_played_at).then_with(|| a.name.cmp(&b.name)));
        saves
    }

    pub fn info(&self, id: &str) -> Result<SaveInfo, String> {
        let store = Store::open(&self.path(id)?).map_err(|e| e.to_string())?;
        describe(id, &store)
    }

    /// Creates an empty game; the caller opens it to start playing.
    pub fn create(&self, name: &str, now: i64) -> Result<SaveInfo, String> {
        let (id, path) = self.free_slot(now)?;
        let mut store = Store::open(&path).map_err(|e| e.to_string())?;
        stamp(&mut store, &clean_name(name), now, now)?;
        describe(&id, &store)
    }

    /// Opens a game to play it, which makes it the last played one.
    /// A game edited outside it (see `seal`) is refused, and the player caught.
    pub fn open(&self, id: &str, now: i64) -> Result<(SaveInfo, Store), String> {
        let path = self.path(id)?;
        if !path.exists() {
            return Err(format!("save {id} not found"));
        }
        let mut store = Store::open(&path).map_err(|e| e.to_string())?;
        if !store.seal_intact().map_err(|e| e.to_string())? {
            return Err(self.caught());
        }
        store.set_meta(LAST_PLAYED_AT, &now.to_string()).map_err(|e| e.to_string())?;
        Ok((describe(id, &store)?, store))
    }

    /// Copies a game into a new save point. The copy counts as played just before the
    /// original, so "continue" keeps going with the original game.
    pub fn save_point(&self, id: &str, name: &str, now: i64) -> Result<SaveInfo, String> {
        let source = Store::open(&self.path(id)?).map_err(|e| e.to_string())?;
        let (copy_id, copy_path) = self.free_slot(now)?;
        source.copy_to(&copy_path).map_err(|e| e.to_string())?;
        let mut copy = Store::open(&copy_path).map_err(|e| e.to_string())?;
        stamp(&mut copy, &clean_name(name), now, now - 1)?;
        describe(&copy_id, &copy)
    }

    pub fn rename(&self, id: &str, name: &str) -> Result<SaveInfo, String> {
        let mut store = Store::open(&self.path(id)?).map_err(|e| e.to_string())?;
        store.set_meta(NAME, &clean_name(name)).map_err(|e| e.to_string())?;
        describe(id, &store)
    }

    /// Deletes a save for good (with SQLite's side files).
    pub fn delete(&self, id: &str) -> Result<(), String> {
        let path = self.path(id)?;
        fs::remove_file(&path).map_err(|e| format!("cannot delete save {id}: {e}"))?;
        for suffix in ["-wal", "-shm"] {
            let _ = fs::remove_file(side_file(&path, suffix));
        }
        Ok(())
    }

    /// Copies a game into `dir`, as a single file named after it (`<name>.unsealed`, an earlier
    /// export never replaced), to carry it to another computer. Returns where.
    pub fn export(&self, id: &str, dir: &Path) -> Result<PathBuf, String> {
        let store = Store::open(&self.path(id)?).map_err(|e| e.to_string())?;
        let info = describe(id, &store)?;
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let stem = file_stem(&info.name);
        let mut path = dir.join(format!("{stem}.{EXPORT_EXTENSION}"));
        let mut n = 2;
        while path.exists() {
            path = dir.join(format!("{stem} ({n}).{EXPORT_EXTENSION}"));
            n += 1;
        }
        store.copy_to(&path).map_err(|e| e.to_string())?;
        Ok(path)
    }

    /// Adds a game from an exported file (its bytes): a new save, with its own name, which
    /// counts as played now (first in the list, and "Continue" takes it). Anything but an
    /// Unsealed game is refused, and nothing is left of it.
    pub fn import(&self, bytes: &[u8], now: i64) -> Result<SaveInfo, String> {
        if !bytes.starts_with(SQLITE_HEADER) {
            return Err(NOT_A_SAVE.to_string());
        }
        let (id, path) = self.free_slot(now)?;
        let incoming = path.with_extension("incoming");
        fs::write(&incoming, bytes).map_err(|e| e.to_string())?;
        let checked = (|| {
            let mut store = Store::open(&incoming).map_err(|_| NOT_A_SAVE.to_string())?;
            store.meta(NAME).ok().flatten().ok_or_else(|| NOT_A_SAVE.to_string())?;
            // Edited by hand before being brought here: refused, and the player caught.
            if !store.seal_intact().map_err(|e| e.to_string())? {
                return Err(self.caught());
            }
            store.set_meta(LAST_PLAYED_AT, &now.to_string()).map_err(|e| e.to_string())
        })();
        if let Err(e) = checked {
            for suffix in ["", "-wal", "-shm"] {
                let _ = fs::remove_file(side_file(&incoming, suffix));
            }
            return Err(e);
        }
        fs::rename(&incoming, &path).map_err(|e| e.to_string())?;
        self.info(&id)
    }

    /// A save edited by hand was found: remembered (`take_caught`) for the next game played,
    /// which gets its secret achievement. Returns the error to show.
    fn caught(&self) -> String {
        let _ = fs::write(self.dir.join(CAUGHT_FILE), b"");
        TAMPERED.to_string()
    }

    /// Whether a save edited by hand was found since the last call.
    pub fn take_caught(&self) -> bool {
        fs::remove_file(self.dir.join(CAUGHT_FILE)).is_ok()
    }

    /// Turns the save of the versions without games (a single database) into the first
    /// game, once. The old file is kept aside, renamed, in case anything goes wrong.
    pub fn adopt_legacy(&self, legacy: &Path, now: i64) -> Result<Option<SaveInfo>, String> {
        if !legacy.exists() || !self.list().is_empty() {
            return Ok(None);
        }
        let old = Store::open(legacy).map_err(|e| e.to_string())?;
        let (id, path) = self.free_slot(now)?;
        old.copy_to(&path).map_err(|e| e.to_string())?;
        drop(old);
        let mut store = Store::open(&path).map_err(|e| e.to_string())?;
        stamp(&mut store, "My first game", now, now)?;
        let info = describe(&id, &store)?;
        for suffix in ["", "-wal", "-shm"] {
            let file = side_file(legacy, suffix);
            if file.exists() {
                let _ = fs::rename(&file, side_file(legacy, &format!("{suffix}.migrated")));
            }
        }
        Ok(Some(info))
    }

    fn path(&self, id: &str) -> Result<PathBuf, String> {
        // The id ends up in a file path: only accept what `free_slot` produces.
        if !valid_id(id) {
            return Err(format!("invalid save id `{id}`"));
        }
        Ok(self.dir.join(format!("{id}.{EXTENSION}")))
    }

    fn free_slot(&self, now: i64) -> Result<(String, PathBuf), String> {
        (0..1000)
            .map(|n| format!("game-{now}-{n}"))
            .map(|id| self.path(&id).map(|p| (id, p)))
            .find(|slot| slot.as_ref().map_or(true, |(_, p)| !p.exists()))
            .unwrap_or_else(|| Err("no free save slot".to_string()))
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn side_file(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

fn clean_name(name: &str) -> String {
    let name: String = name.trim().chars().filter(|c| !c.is_control()).take(MAX_NAME_CHARS).collect();
    if name.is_empty() {
        "Unnamed game".to_string()
    } else {
        name
    }
}

/// A game's name as a file name, after "Unsealed - ": its letters, digits and dashes, any other
/// character a space.
fn file_stem(name: &str) -> String {
    let stem: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if stem.is_empty() { "Unsealed".to_string() } else { format!("Unsealed - {stem}") }
}

fn stamp(store: &mut Store, name: &str, created_at: i64, last_played_at: i64) -> Result<(), String> {
    let set = |store: &mut Store, key: &str, value: &str| store.set_meta(key, value).map_err(|e| e.to_string());
    set(store, NAME, name)?;
    set(store, CREATED_AT, &created_at.to_string())?;
    set(store, LAST_PLAYED_AT, &last_played_at.to_string())
}

fn describe(id: &str, store: &Store) -> Result<SaveInfo, String> {
    let get = |key: &str| store.meta(key).map_err(|e| e.to_string());
    let number = |key: &str| -> Result<i64, String> { Ok(get(key)?.and_then(|v| v.parse().ok()).unwrap_or(0)) };
    Ok(SaveInfo {
        id: id.to_string(),
        name: get(NAME)?.unwrap_or_else(|| "Unnamed game".to_string()),
        created_at: number(CREATED_AT)?,
        last_played_at: number(LAST_PLAYED_AT)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saves(name: &str) -> Saves {
        let dir = std::env::temp_dir().join(format!("unsealed-saves-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        Saves::new(dir).unwrap()
    }

    #[test]
    fn games_are_listed_last_played_first() {
        let s = saves("list");
        let a = s.create("  Alice  ", 100).unwrap();
        let b = s.create("Bob", 200).unwrap();
        assert_eq!(a.name, "Alice");
        assert_eq!(s.list().iter().map(|g| g.name.as_str()).collect::<Vec<_>>(), ["Bob", "Alice"]);

        s.open(&a.id, 300).unwrap();
        assert_eq!(s.list()[0].id, a.id);
        assert_ne!(a.id, b.id);
    }

    #[test]
    fn a_save_point_copies_the_game_and_keeps_the_original_as_the_latest() {
        let s = saves("point");
        let game = s.create("Main", 100).unwrap();
        {
            let (_, mut store) = s.open(&game.id, 150).unwrap();
            store.ensure_starting_balance(1_234, 150).unwrap();
        }
        let point = s.save_point(&game.id, "Before the big spend", 200).unwrap();
        assert_eq!(point.name, "Before the big spend");
        let (_, copy) = s.open(&point.id, 201).unwrap();
        assert_eq!(copy.balance().unwrap(), 1_234);
        drop(copy);
        s.open(&game.id, 202).unwrap();
        assert_eq!(s.list()[0].id, game.id);
    }

    #[test]
    fn rename_delete_and_invalid_ids() {
        let s = saves("edit");
        let game = s.create("Old", 100).unwrap();
        assert_eq!(s.rename(&game.id, "New").unwrap().name, "New");
        assert_eq!(s.rename(&game.id, "   ").unwrap().name, "Unnamed game");
        s.delete(&game.id).unwrap();
        assert!(s.list().is_empty());
        assert!(s.open("../escape", 1).is_err());
        assert!(s.open("game-1-0", 1).is_err()); // does not exist
    }

    #[test]
    fn a_game_exported_here_is_imported_there_as_a_new_save() {
        let here = saves("export");
        let game = here.create("Ma partie : la bonne", 100).unwrap();
        {
            let (_, mut store) = here.open(&game.id, 150).unwrap();
            store.ensure_starting_balance(4_321, 150).unwrap();
        }
        let out = here.dir.join("exports");
        let file = here.export(&game.id, &out).unwrap();
        assert_eq!(file.file_name().unwrap(), "Unsealed - Ma partie la bonne.unsealed");
        // A second export never replaces the first.
        assert_eq!(here.export(&game.id, &out).unwrap().file_name().unwrap(), "Unsealed - Ma partie la bonne (2).unsealed");

        let there = saves("import");
        there.create("Already there", 500).unwrap();
        let imported = there.import(&fs::read(&file).unwrap(), 1_000).unwrap();
        assert_eq!((imported.name.as_str(), imported.last_played_at), ("Ma partie : la bonne", 1_000));
        assert_eq!(there.list()[0].id, imported.id);
        let (_, store) = there.open(&imported.id, 1_001).unwrap();
        assert_eq!(store.balance().unwrap(), 4_321);
    }

    #[test]
    fn importing_anything_but_a_game_is_refused_and_leaves_nothing() {
        let s = saves("import-bad");
        assert!(s.import(b"hello", 1).is_err());
        // A SQLite file, but not a game.
        let other = s.dir.join("other.db");
        rusqlite::Connection::open(&other).unwrap().execute_batch("CREATE TABLE t (x INTEGER);").unwrap();
        assert!(s.import(&fs::read(&other).unwrap(), 2).is_err());
        let left: Vec<_> = fs::read_dir(&s.dir).unwrap().flatten().map(|e| e.file_name()).filter(|n| n != "other.db").collect();
        assert!(left.is_empty(), "left behind: {left:?}");
    }

    #[test]
    fn a_save_edited_by_hand_is_refused_and_the_player_caught() {
        let s = saves("tampered");
        let game = s.create("Honest", 100).unwrap();
        {
            let (_, mut store) = s.open(&game.id, 150).unwrap();
            store.ensure_starting_balance(1_000, 150).unwrap();
        }
        assert!(s.open(&game.id, 160).is_ok(), "a game played normally loads");

        // In an SQLite browser, the triggers call a function only the game has: no edit...
        let path = s.path(&game.id).unwrap();
        let raw = rusqlite::Connection::open(&path).unwrap();
        assert!(raw.execute("UPDATE ledger SET amount_cents = 999999", []).is_err());
        // ...and with the triggers removed, the seal no longer matches.
        for t in ["insert", "delete", "update"] {
            raw.execute(&format!("DROP TRIGGER seal_ledger_{t}"), []).unwrap();
        }
        raw.execute("UPDATE ledger SET amount_cents = 999999", []).unwrap();
        drop(raw);
        assert!(!s.take_caught());
        assert_eq!(s.open(&game.id, 170).err().as_deref(), Some(TAMPERED));
        assert!(s.take_caught(), "the next game played gets its achievement");
        assert!(!s.take_caught(), "once");

        // Carried to another computer, it is refused there too.
        let file = s.export(&game.id, &s.dir.join("out")).unwrap();
        let there = saves("tampered-there");
        assert_eq!(there.import(&fs::read(file).unwrap(), 200).err().as_deref(), Some(TAMPERED));
        assert!(there.list().is_empty());
        assert!(there.take_caught());
    }

    #[test]
    fn the_legacy_save_becomes_the_first_game_once() {
        let s = saves("legacy");
        let legacy = s.dir.with_extension("legacy.sqlite");
        let _ = fs::remove_file(&legacy);
        {
            let mut old = Store::open(&legacy).unwrap();
            old.ensure_starting_balance(777, 1).unwrap();
        }
        let adopted = s.adopt_legacy(&legacy, 50).unwrap().unwrap();
        assert_eq!(adopted.name, "My first game");
        assert!(!legacy.exists());
        let (_, store) = s.open(&adopted.id, 60).unwrap();
        assert_eq!(store.balance().unwrap(), 777);
        // Once there are games, nothing is adopted any more.
        assert!(s.adopt_legacy(&legacy, 70).unwrap().is_none());
    }
}
