//! Scenes: one artwork spread over several borderless cards of a set, which form it laid edge
//! to edge (Bilbo's Birthday Party over six cards of the Lord of the Rings). Neither MTGJSON
//! nor Scryfall tells them apart: they live in `config/scenes.json`, written by hand (names and
//! grids from the MTG Wiki, the cards' places checked on their images).

use crate::booster::ConfigError;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};

/// The languages every scene is named in.
pub const SCENE_LANGUAGES: [&str; 5] = ["en", "fr", "de", "es", "it"];

#[derive(Debug, Clone, Deserialize)]
struct ScenesFile {
    sets: BTreeMap<String, Vec<RawScene>>,
}

#[derive(Debug, Clone, Deserialize)]
struct RawScene {
    id: String,
    name: BTreeMap<String, String>,
    columns: u32,
    /// Collector numbers row by row, "328:back" for the back face of a double-faced card.
    cells: Vec<String>,
}

/// A card of a scene, at its place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneCell {
    pub number: String,
    /// The scene is on the card's back face.
    pub back: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub id: String,
    /// By language, English always there.
    pub names: BTreeMap<String, String>,
    pub columns: u32,
    /// Row by row.
    pub cells: Vec<SceneCell>,
}

impl Scene {
    /// Its name in `lang`, else in English.
    pub fn name(&self, lang: &str) -> &str {
        self.names.get(lang).or_else(|| self.names.get("en")).map_or(self.id.as_str(), String::as_str)
    }

    pub fn rows(&self) -> u32 {
        self.cells.len() as u32 / self.columns
    }
}

/// Every scene, by set code.
#[derive(Debug, Clone, Default)]
pub struct Scenes {
    by_set: HashMap<String, Vec<Scene>>,
}

impl Scenes {
    pub fn from_json(json: &str) -> Result<Self, ConfigError> {
        let bad = |msg: String| Err(ConfigError::Invalid(format!("scenes: {msg}")));
        let file: ScenesFile = serde_json::from_str(json).map_err(|e| ConfigError::Invalid(format!("scenes: {e}")))?;
        let mut by_set = HashMap::new();
        for (code, raw) in file.sets {
            let mut ids = HashSet::new();
            let mut scenes = Vec::with_capacity(raw.len());
            for s in raw {
                if s.id.is_empty() || !ids.insert(s.id.clone()) {
                    return bad(format!("{code}: id `{}` empty or twice", s.id));
                }
                if !s.name.contains_key("en") {
                    return bad(format!("{code}/{} has no English name", s.id));
                }
                if s.columns == 0 || s.cells.len() < 2 || s.cells.len() % s.columns as usize != 0 {
                    return bad(format!("{code}/{}: {} cards do not fill rows of {}", s.id, s.cells.len(), s.columns));
                }
                let mut cells = Vec::with_capacity(s.cells.len());
                for cell in &s.cells {
                    let (number, back) = match cell.split_once(':') {
                        None => (cell.as_str(), false),
                        Some((number, "back")) => (number, true),
                        Some(_) => return bad(format!("{code}/{}: `{cell}` (a face is \":back\")", s.id)),
                    };
                    if number.is_empty() || cells.iter().any(|c: &SceneCell| c.number == number && c.back == back) {
                        return bad(format!("{code}/{}: `{cell}` empty or twice", s.id));
                    }
                    cells.push(SceneCell { number: number.to_string(), back });
                }
                scenes.push(Scene { id: s.id, names: s.name, columns: s.columns, cells });
            }
            if !scenes.is_empty() {
                by_set.insert(code, scenes);
            }
        }
        Ok(Self { by_set })
    }

    /// The scenes of a set, in their order.
    pub fn of_set(&self, code: &str) -> &[Scene] {
        self.by_set.get(code).map_or(&[], Vec::as_slice)
    }

    /// The codes of the sets that have scenes.
    pub fn set_codes(&self) -> impl Iterator<Item = &str> {
        self.by_set.keys().map(String::as_str)
    }
}
