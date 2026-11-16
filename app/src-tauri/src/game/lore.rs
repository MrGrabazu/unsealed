//! A set's lore as the player stands (see `unsealed_core::lore`): its plane and characters, and
//! its chapters, each open once every key card is owned (any printing of it in the set, in
//! either finish). A sealed chapter keeps its text: only its title and keys are told.

use super::{CollectionCard, Game, Owned};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use unsealed_core::catalog::CardEntry;
use unsealed_core::lore::{text, Lore};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoreView {
    pub set_code: String,
    pub plane: String,
    pub summary: String,
    /// The card whose art shows the plane.
    pub illustration: Option<LoreArt>,
    pub characters: Vec<CharacterView>,
    pub chapters: Vec<ChapterView>,
    /// Chapters open, of all.
    pub unlocked: u32,
    pub total: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterView {
    pub card: CollectionCard,
    pub owned: bool,
    pub name: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterView {
    pub title: String,
    pub unlocked: bool,
    /// Its paragraphs, once open (empty while sealed).
    pub paragraphs: Vec<String>,
    pub illustration: Option<LoreArt>,
    pub keys: Vec<LoreCard>,
}

/// A card whose art illustrates the lore, with its artist (credited beside it, Scryfall's terms).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoreArt {
    pub card: CollectionCard,
    pub artist: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoreCard {
    pub card: CollectionCard,
    pub owned: bool,
}

impl Game {
    /// The lore of every set (shared by the games of a session).
    pub fn set_lore(&mut self, lore: Arc<Lore>) {
        self.lore = Some(lore);
    }

    /// A few lines on a set's world in `lang`, for the Boosterium (no set loaded).
    pub fn lore_summary(&self, set_code: &str, lang: &str) -> Option<String> {
        let set = self.lore.as_ref()?.of_set(set_code)?;
        Some(text(&set.summary, lang).to_string())
    }

    /// A set's lore, as far as the player has read it, in `lang`; none when it is not written.
    pub fn lore(&mut self, set_code: &str, lang: &str) -> Result<Option<LoreView>, String> {
        let Some(lore) = self.lore.clone() else { return Ok(None) };
        let Some(set_lore) = lore.of_set(set_code) else { return Ok(None) };
        self.ensure_loaded(set_code)?;
        self.ensure_prices();
        let owned = self.owned_with_tokens(set_code)?;
        let set = self.sets.get(set_code).ok_or_else(|| format!("set {set_code} not loaded"))?;
        // The set's own cards, by number; a split or double-faced card's second face is never owned.
        let cards: Vec<&CardEntry> = set
            .file
            .cards
            .iter()
            .filter(|c| c.set_code == set_code && !c.token && !c.art && c.side.as_deref() != Some("b"))
            .collect();
        let by_number: HashMap<&str, &CardEntry> = cards.iter().map(|c| (c.number.as_str(), *c)).collect();
        // A card counts as owned through any of its printings in the set.
        let names_owned: HashSet<&str> = cards
            .iter()
            .filter(|c| owned.get(&c.uuid).is_some_and(|o| o.normal + o.foil > 0))
            .map(|c| c.name.as_str())
            .collect();
        let entry = |number: &str| by_number.get(number).copied();
        let has = |c: &CardEntry| names_owned.contains(c.name.as_str());
        let shown = |c: &CardEntry| self.collection_card(c, owned.get(&c.uuid).copied().unwrap_or(Owned::default()));
        let art = |c: &CardEntry| LoreArt { card: shown(c), artist: c.artist.clone() };

        let characters = set_lore
            .characters
            .iter()
            .filter_map(|ch| {
                let card = entry(&ch.card)?;
                Some(CharacterView { card: shown(card), owned: has(card), name: text(&ch.name, lang).to_string(), role: text(&ch.role, lang).to_string() })
            })
            .collect();
        let chapters: Vec<ChapterView> = set_lore
            .chapters
            .iter()
            .map(|ch| {
                let keys: Vec<&CardEntry> = ch.keys.iter().filter_map(|n| entry(n)).collect();
                let unlocked = keys.iter().all(|c| has(c));
                ChapterView {
                    title: text(&ch.title, lang).to_string(),
                    unlocked,
                    paragraphs: if unlocked { ch.paragraphs(lang).to_vec() } else { Vec::new() },
                    illustration: entry(&ch.illustration).map(art),
                    keys: keys.into_iter().map(|c| LoreCard { card: shown(c), owned: has(c) }).collect(),
                }
            })
            .collect();
        Ok(Some(LoreView {
            set_code: set_code.to_string(),
            plane: text(&set_lore.plane, lang).to_string(),
            summary: text(&set_lore.summary, lang).to_string(),
            illustration: entry(&set_lore.illustration).map(art),
            characters,
            unlocked: chapters.iter().filter(|c| c.unlocked).count() as u32,
            total: chapters.len() as u32,
            chapters,
        }))
    }
}
