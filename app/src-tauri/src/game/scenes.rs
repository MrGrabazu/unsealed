//! A set's scenes as the player stands (see `unsealed_core::scenes`): each place filled by the
//! card when owned (in either finish), the artwork coming together card after card.

use super::{CollectionCard, Game};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use unsealed_core::achievements::Event;
use unsealed_core::catalog::CardEntry;
use unsealed_core::scenes::{Scene, Scenes};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenesView {
    pub set_code: String,
    /// Scenes complete, of all the set's.
    pub completed: u32,
    pub total: u32,
    /// Places filled, of all the scenes'.
    pub cards_owned: u32,
    pub cards_total: u32,
    pub scenes: Vec<SceneView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneView {
    pub id: String,
    pub name: String,
    pub columns: u32,
    pub rows: u32,
    pub owned: u32,
    pub total: u32,
    pub complete: bool,
    /// Every card owned in foil.
    pub all_foil: bool,
    /// Row by row.
    pub cells: Vec<SceneCellView>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneCellView {
    pub card: CollectionCard,
    pub owned: bool,
    /// A foil copy is owned (shown with its sheen).
    pub foil: bool,
    /// The scene is on the card's back face.
    pub back: bool,
}

impl Game {
    /// The scenes every set may have (shared by the games of a session).
    pub fn set_scenes(&mut self, scenes: Arc<Scenes>) {
        self.scenes = Some(scenes);
    }

    /// The cards of a scene in the set's file, place by place; none when one is missing (data
    /// built without it).
    fn scene_cards<'a>(&'a self, set_code: &str, scene: &Scene) -> Option<Vec<&'a CardEntry>> {
        let set = self.sets.get(set_code)?;
        // Tokens may bear the set's own code and the same numbers as its cards (LTR).
        let by_number: HashMap<&str, &CardEntry> = set
            .file
            .cards
            .iter()
            .filter(|c| c.set_code == set_code && !c.token && !c.art)
            .map(|c| (c.number.as_str(), c))
            .collect();
        scene.cells.iter().map(|cell| by_number.get(cell.number.as_str()).copied()).collect()
    }

    /// The scenes of a set, as far as the player is, their names in `lang`.
    pub fn scenes(&mut self, set_code: &str, lang: &str) -> Result<ScenesView, String> {
        self.ensure_loaded(set_code)?;
        self.ensure_prices();
        let owned = self.owned_with_tokens(set_code)?;
        let scenes = self.scenes.clone().unwrap_or_default();
        let mut view = ScenesView { set_code: set_code.to_string(), completed: 0, total: 0, cards_owned: 0, cards_total: 0, scenes: Vec::new() };
        for scene in scenes.of_set(set_code) {
            let Some(cards) = self.scene_cards(set_code, scene) else { continue };
            let cells: Vec<SceneCellView> = cards
                .iter()
                .zip(&scene.cells)
                .map(|(card, cell)| {
                    let have = owned.get(&card.uuid).copied().unwrap_or_default();
                    SceneCellView {
                        card: self.collection_card(card, have),
                        owned: have.normal + have.foil > 0,
                        foil: have.foil > 0,
                        back: cell.back,
                    }
                })
                .collect();
            let have = cells.iter().filter(|c| c.owned).count() as u32;
            let total = cells.len() as u32;
            view.total += 1;
            view.completed += u32::from(have == total);
            view.cards_owned += have;
            view.cards_total += total;
            view.scenes.push(SceneView {
                id: scene.id.clone(),
                name: scene.name(lang).to_string(),
                columns: scene.columns,
                rows: scene.rows(),
                owned: have,
                total,
                complete: have == total,
                all_foil: cells.iter().all(|c| c.foil),
                cells,
            });
        }
        Ok(view)
    }

    /// For the achievements: one `SceneCompleted` per complete scene of the set (counted once
    /// by its key).
    pub(super) fn scene_events(&self, set_code: &str) -> Vec<Event> {
        let Some(scenes) = self.scenes.clone() else { return Vec::new() };
        let list = scenes.of_set(set_code);
        if list.is_empty() {
            return Vec::new();
        }
        let Ok(owned) = self.owned_with_tokens(set_code) else { return Vec::new() };
        let mut events = Vec::new();
        for scene in list {
            let Some(cards) = self.scene_cards(set_code, scene) else { continue };
            let has = |c: &CardEntry| owned.get(&c.uuid).is_some_and(|o| o.normal + o.foil > 0);
            if !cards.iter().all(|c| has(c)) {
                continue;
            }
            events.push(
                Event::new("SceneCompleted")
                    .with("scene", format!("{set_code}/{}", scene.id))
                    .with("setCode", set_code)
                    .with("cards", cards.len() as i64)
                    .with("allFoil", cards.iter().all(|c| owned.get(&c.uuid).is_some_and(|o| o.foil > 0))),
            );
        }
        events
    }
}
