//! A set's lore: its plane, its characters and its story in chapters, each chapter opening once
//! the player owns its key cards. Written by us in `config/lore.json`, in every language, from
//! the facts of the MTG Wiki and the Magic Story episodes (never their sentences). Cards are
//! named by their collector number in the set; any printing of the same card counts.

use crate::booster::ConfigError;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

/// The languages every text of the lore is written in.
pub const LORE_LANGUAGES: [&str; 5] = ["en", "fr", "de", "es", "it"];

/// A text by language.
pub type Texts = BTreeMap<String, String>;

#[derive(Debug, Clone, Deserialize)]
struct LoreFile {
    sets: BTreeMap<String, SetLore>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SetLore {
    pub plane: Texts,
    /// The card whose art shows the plane.
    pub illustration: String,
    /// A few lines on the world, for the Boosterium.
    pub summary: Texts,
    pub characters: Vec<Character>,
    pub chapters: Vec<Chapter>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Character {
    /// The card that shows them.
    pub card: String,
    pub name: Texts,
    pub role: Texts,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Chapter {
    pub title: Texts,
    /// The card whose art illustrates the chapter.
    pub illustration: String,
    /// The cards to own for the chapter to open (none: open from the start).
    pub keys: Vec<String>,
    /// Its paragraphs, by language.
    pub text: BTreeMap<String, Vec<String>>,
}

/// The text in `lang`, else in English.
pub fn text<'a>(texts: &'a Texts, lang: &str) -> &'a str {
    texts.get(lang).or_else(|| texts.get("en")).map_or("", String::as_str)
}

impl Chapter {
    /// Its paragraphs in `lang`, else in English.
    pub fn paragraphs(&self, lang: &str) -> &[String] {
        self.text.get(lang).or_else(|| self.text.get("en")).map_or(&[], Vec::as_slice)
    }
}

/// Every set's lore, by set code.
#[derive(Debug, Clone, Default)]
pub struct Lore {
    by_set: HashMap<String, SetLore>,
}

impl Lore {
    /// Reads `lore.json`, refusing a text missing in a language or a card left unnamed.
    pub fn from_json(json: &str) -> Result<Self, ConfigError> {
        let bad = |msg: String| Err(ConfigError::Invalid(format!("lore: {msg}")));
        let file: LoreFile = serde_json::from_str(json).map_err(|e| ConfigError::Invalid(format!("lore: {e}")))?;
        let full = |texts: &Texts| LORE_LANGUAGES.iter().all(|l| texts.get(*l).is_some_and(|t| !t.trim().is_empty()));
        for (code, set) in &file.sets {
            if !full(&set.plane) || !full(&set.summary) {
                return bad(format!("{code}: its plane or summary is missing a language"));
            }
            if set.illustration.is_empty() || set.chapters.is_empty() {
                return bad(format!("{code}: no illustration or no chapter"));
            }
            for (i, c) in set.characters.iter().enumerate() {
                if c.card.is_empty() || !full(&c.name) || !full(&c.role) {
                    return bad(format!("{code}: character {} has no card, or a name or role missing a language", i + 1));
                }
            }
            for (i, c) in set.chapters.iter().enumerate() {
                let written = LORE_LANGUAGES.iter().all(|l| c.text.get(*l).is_some_and(|p| !p.is_empty() && p.iter().all(|s| !s.trim().is_empty())));
                if !full(&c.title) || !written || c.illustration.is_empty() || c.keys.iter().any(String::is_empty) {
                    return bad(format!("{code}: chapter {} is missing a language, its illustration or a key", i + 1));
                }
            }
        }
        Ok(Self { by_set: file.sets.into_iter().collect() })
    }

    /// The lore of a set, if written.
    pub fn of_set(&self, code: &str) -> Option<&SetLore> {
        self.by_set.get(code)
    }

    /// The codes of the sets whose lore is written.
    pub fn set_codes(&self) -> impl Iterator<Item = &str> {
        self.by_set.keys().map(String::as_str)
    }
}
