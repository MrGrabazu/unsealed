//! Artifacts (see `unsealed_core::artifacts`): the ones owned, their effects put together, the
//! cabinet shown to the player, the Boosterium's curios, and finding or being given one.

use super::{err, Game};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use unsealed_core::achievements::Event;
use unsealed_core::artifacts::{ArtifactDef, ArtifactSource, ArtifactTranslations, ArtifactsConfig, Bonuses};

/// An artifact in the cabinet. One neither owned nor on sale keeps its secrets: only its
/// number, rarity, drawing (shown as a silhouette) and the kind of its source are told.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactView {
    pub id: String,
    /// Its place in the cabinet, from 1.
    pub number: usize,
    pub rarity: String,
    pub glyph: String,
    /// "owned", "shop" (on sale, not owned yet) or "hidden".
    pub status: &'static str,
    pub name: Option<String>,
    pub effect: Option<String>,
    pub flavor: Option<String>,
    /// "shop", "booster", "achievement" or "allOthers".
    pub source: &'static str,
    pub price_cents: Option<u32>,
    pub acquired_at: Option<i64>,
}

/// What the interface needs of the artifacts' effects to show the right amounts.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactPerks {
    /// Selling duplicates pays this much more, in percent.
    pub duplicates_percent: u32,
    /// The best share of the market price a card sells for, that bonus included.
    pub max_sell_rate_percent: u32,
    /// The balance wears a golden frame (the crown).
    pub crowned: bool,
}

/// The artifacts' state in a game: the config, who owns what, and the bonuses that follow.
#[derive(Default)]
pub(super) struct Artifacts {
    pub config: Option<ArtifactsConfig>,
    pub texts: HashMap<String, ArtifactTranslations>,
    pub owned: HashMap<String, (i64, String)>,
    pub bonuses: Bonuses,
    /// Artifacts given since the interface last asked (achievements, the crown).
    pub given: Vec<String>,
}

impl Game {
    /// The artifacts of this game: their definitions, and the ones already owned.
    pub fn set_artifacts(&mut self, config: ArtifactsConfig) {
        self.artifacts.config = Some(config);
        self.refresh_artifacts();
    }

    /// The artifacts' texts in other languages (English is in the config).
    pub fn set_artifact_texts(&mut self, texts: HashMap<String, ArtifactTranslations>) {
        self.artifacts.texts = texts;
    }

    /// Reads the artifacts owned again, and the bonuses they give.
    pub(super) fn refresh_artifacts(&mut self) {
        let owned: HashMap<String, (i64, String)> =
            self.store.artifacts().unwrap_or_default().into_iter().map(|(id, at, source)| (id, (at, source))).collect();
        let ids: HashSet<String> = owned.keys().cloned().collect();
        self.artifacts.bonuses = self.artifacts.config.as_ref().map(|c| c.bonuses(&ids)).unwrap_or_default();
        self.artifacts.owned = owned;
    }

    pub(super) fn bonuses(&self) -> &Bonuses {
        &self.artifacts.bonuses
    }

    fn owned_ids(&self) -> HashSet<String> {
        self.artifacts.owned.keys().cloned().collect()
    }

    pub(super) fn artifact_perks(&self) -> ArtifactPerks {
        ArtifactPerks {
            duplicates_percent: self.bonuses().duplicates_percent,
            max_sell_rate_percent: self.bonuses().max_sell_rate_percent,
            crowned: self.artifacts.owned.contains_key(CROWN),
        }
    }

    /// The cabinet: every artifact, in its order, in the interface language.
    pub fn artifacts(&self, lang: &str) -> Vec<ArtifactView> {
        let Some(config) = &self.artifacts.config else { return Vec::new() };
        config.artifacts.iter().enumerate().map(|(i, a)| self.artifact_view(a, i + 1, lang)).collect()
    }

    fn artifact_view(&self, a: &ArtifactDef, number: usize, lang: &str) -> ArtifactView {
        let owned = self.artifacts.owned.get(&a.id);
        let price = match a.source {
            ArtifactSource::Shop { price_cents } => Some(price_cents),
            _ => None,
        };
        let status = if owned.is_some() {
            "owned"
        } else if price.is_some() && self.artifacts.config.as_ref().is_some_and(|c| c.can_get(a, &self.owned_ids())) {
            "shop"
        } else {
            "hidden"
        };
        let told = status != "hidden";
        let text = self.artifacts.texts.get(lang).and_then(|t| t.get(&a.id));
        ArtifactView {
            id: a.id.clone(),
            number,
            rarity: a.rarity.clone(),
            glyph: a.glyph.clone(),
            status,
            name: told.then(|| text.map_or(&a.name, |t| &t.name).clone()),
            effect: told.then(|| text.map_or(&a.effect, |t| &t.effect).clone()),
            flavor: told.then(|| text.map_or(&a.flavor, |t| &t.flavor).clone()),
            source: match a.source {
                ArtifactSource::Shop { .. } => "shop",
                ArtifactSource::Booster { .. } => "booster",
                ArtifactSource::Achievement { .. } => "achievement",
                ArtifactSource::AllOthers => "allOthers",
            },
            price_cents: price.filter(|_| status == "shop"),
            acquired_at: owned.map(|(at, _)| *at),
        }
    }

    /// One artifact as shown, in the interface language.
    fn artifact_view_of(&self, id: &str, lang: &str) -> Option<ArtifactView> {
        let config = self.artifacts.config.as_ref()?;
        let (i, a) = config.artifacts.iter().enumerate().find(|(_, a)| a.id == id)?;
        Some(self.artifact_view(a, i + 1, lang))
    }

    /// Buys an artifact on sale at the Boosterium.
    pub fn buy_artifact(&mut self, id: &str, lang: &str) -> Result<ArtifactView, String> {
        self.flush_reveal();
        let config = self.artifacts.config.as_ref().ok_or("no artifacts")?;
        let a = config.get(id).ok_or_else(|| format!("unknown artifact {id}"))?;
        let ArtifactSource::Shop { price_cents } = a.source else { return Err(format!("{id} is not on sale")) };
        if !config.can_get(a, &self.owned_ids()) {
            return Err(format!("{id} cannot be bought"));
        }
        let now = (self.clock)().now;
        self.store.buy_artifact(id, price_cents, now).map_err(err)?;
        self.acquired(id);
        self.artifact_view_of(id, lang).ok_or_else(|| "artifact gone".to_string())
    }

    /// Rolls the artifacts hidden in a booster of `kind` of `set_code` just opened: at most one
    /// is found, recorded at once, and shown at the end of the opening. Its events wait with
    /// the booster's (returned).
    pub(super) fn find_artifact(&mut self, set_code: &str, kind: &str) -> (Option<ArtifactView>, Vec<Event>) {
        let owned = self.owned_ids();
        let Some(config) = &self.artifacts.config else { return (None, Vec::new()) };
        let released = self.release_dates.as_ref().and_then(|d| d.get(set_code)).map_or("", String::as_str);
        let Some(found) = config.roll_booster(kind, released, &owned, &mut self.rng).map(|a| a.id.clone()) else {
            return (None, Vec::new());
        };
        let now = (self.clock)().now;
        if !self.store.add_artifact(&found, "booster", now).unwrap_or(false) {
            return (None, Vec::new());
        }
        let events = self.acquired(&found);
        (self.artifact_view_of(&found, &self.card_lang.clone()), events)
    }

    /// Gives the artifacts an achievement just unlocked brings (and the crown, if they were
    /// the last). Returns their events.
    pub(super) fn give_achievement_artifacts(&mut self, unlocked: &[String]) -> Vec<Event> {
        let Some(config) = &self.artifacts.config else { return Vec::new() };
        let due: Vec<String> =
            unlocked.iter().flat_map(|id| config.for_achievement(id).map(|a| a.id.clone()).collect::<Vec<_>>()).collect();
        let now = (self.clock)().now;
        let mut events = Vec::new();
        for id in due {
            if self.store.add_artifact(&id, "achievement", now).unwrap_or(false) {
                self.artifacts.given.push(id.clone());
                events.extend(self.acquired(&id));
            }
        }
        events
    }

    /// An artifact just got: the bonuses follow, and the crown comes once every other one is
    /// owned. Returns the events to emit (`ArtifactAcquired`).
    fn acquired(&mut self, id: &str) -> Vec<Event> {
        self.refresh_artifacts();
        let mut events = vec![self.artifact_event(id)];
        let crown = self.artifacts.config.as_ref().and_then(|c| c.all_others_due(&self.owned_ids()).map(|a| a.id.clone()));
        if let Some(crown) = crown {
            let now = (self.clock)().now;
            if self.store.add_artifact(&crown, "allOthers", now).unwrap_or(false) {
                self.artifacts.given.push(crown.clone());
                self.refresh_artifacts();
                events.push(self.artifact_event(&crown));
            }
        }
        events
    }

    fn artifact_event(&self, id: &str) -> Event {
        let rarity = self.artifacts.config.as_ref().and_then(|c| c.get(id)).map_or("", |a| a.rarity.as_str());
        Event::new("ArtifactAcquired")
            .with("id", id)
            .with("rarity", rarity)
            .with("owned", self.artifacts.owned.len() as i64)
    }

    /// Artifacts given since the last call (by achievements, or the crown): to celebrate.
    pub fn take_given_artifacts(&mut self, lang: &str) -> Vec<ArtifactView> {
        let given = std::mem::take(&mut self.artifacts.given);
        given.iter().filter_map(|id| self.artifact_view_of(id, lang)).collect()
    }

    /// The number of sets complete, for an income bonus that depends on it (counted at most
    /// once a minute: it reads every set owned).
    pub(super) fn completed_sets(&mut self) -> usize {
        let now = (self.clock)().now;
        if let Some((at, count)) = self.completed_sets_cache {
            if now - at < 60 {
                return count;
            }
        }
        let count = self.library().map(|binders| binders.iter().filter(|b| b.total_count > 0 && b.owned_count == b.total_count).count()).unwrap_or(0);
        self.completed_sets_cache = Some((now, count));
        count
    }
}

/// The artifact that marks the balance with a golden frame.
const CROWN: &str = "counter-crown";
