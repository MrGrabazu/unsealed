//! Box toppers: the packs a booster box holds besides its boosters (`BoxTopper`). The game
//! sells no boxes (no price to be had, and far too dear for a player), but counts the
//! boosters opened: every box's worth of one set and kind (36 Play Boosters, 12 Collector
//! Boosters…) gives that box's topper, as a sealed booster of its own, never sold. Boosters
//! opened before box toppers existed count too (the openings are all recorded). Opened, not
//! bought: counting purchases would let a player buy a box, get its topper and undo the purchase.

use super::*;
use std::collections::BTreeMap;
use unsealed_core::catalog::BoxTopper;

/// How far the box of a set and kind is: boosters opened toward its next box topper.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BoxProgress {
    /// Boosters opened toward the next box (0 to `size` - 1).
    pub opened: u32,
    /// Boosters in the box.
    pub size: u32,
    /// Box topper packs the box gives.
    pub packs: u32,
}

/// Box toppers just given, to show: one line per set and topper booster.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToppersGiven {
    pub set_code: String,
    pub set_name: String,
    /// The topper's booster (a key of `SetFile::topper_boosters`).
    pub kind: String,
    pub quantity: u32,
}

#[derive(Default)]
pub(super) struct BoxToppers {
    /// The boxes of each set (from the manifest): booster kind -> its box topper, and the
    /// set's name.
    pub(super) boxes: Option<HashMap<String, (String, BTreeMap<String, BoxTopper>)>>,
    /// Given and not shown yet (`take_box_toppers`).
    given: Vec<ToppersGiven>,
    /// The boxes opened before were counted, once per session (see `take_box_toppers`).
    caught_up: bool,
}

impl Game {
    fn ensure_boxes(&mut self) {
        if self.box_toppers.boxes.is_none() {
            let boxes = read_manifest(&self.data_dir)
                .ok()
                .flatten()
                .map(|m| {
                    m.sets
                        .into_iter()
                        .filter(|(_, s)| !s.box_toppers.is_empty())
                        .map(|(code, s)| (code, (s.name, s.box_toppers)))
                        .collect()
                })
                .unwrap_or_default();
            self.box_toppers.boxes = Some(boxes);
        }
    }

    fn box_of(&mut self, set_code: &str, kind: &str) -> Option<BoxTopper> {
        self.ensure_boxes();
        self.box_toppers.boxes.as_ref()?.get(set_code)?.1.get(kind).cloned()
    }

    /// Whether a sealed booster of the set is a box topper's pack.
    pub(super) fn is_topper_kind(&mut self, set_code: &str, kind: &str) -> bool {
        self.ensure_boxes();
        self.box_toppers
            .boxes
            .as_ref()
            .and_then(|b| b.get(set_code))
            .is_some_and(|(_, kinds)| kinds.values().any(|t| t.packs.contains_key(kind)))
    }

    /// The box of a set and booster kind, if it holds a box topper: how far it is.
    pub(super) fn box_progress(&mut self, set_code: &str, kind: &str, opened: &HashMap<(String, String), u32>) -> Option<BoxProgress> {
        let topper = self.box_of(set_code, kind)?;
        let count = opened.get(&(set_code.to_string(), kind.to_string())).copied().unwrap_or(0);
        Some(BoxProgress { opened: count % topper.boosters.max(1), size: topper.boosters, packs: topper.packs.values().sum() })
    }

    /// How many boosters of each set and kind were opened (for `box_progress`).
    pub(super) fn opened_counts(&self) -> HashMap<(String, String), u32> {
        self.store.opened_by_kind().unwrap_or_default().into_iter().map(|(set, kind, n)| ((set, kind), n)).collect()
    }

    /// Gives the box toppers of the boxes now complete for a set and kind, after an opening.
    pub(super) fn give_box_toppers(&mut self, set_code: &str, kind: &str) -> Result<(), String> {
        let Some(topper) = self.box_of(set_code, kind) else { return Ok(()) };
        let opened = self.store.opened_of(set_code, kind).map_err(err)?;
        self.give_boxes(set_code, kind, &topper, opened)
    }

    fn give_boxes(&mut self, set_code: &str, kind: &str, topper: &BoxTopper, opened: u32) -> Result<(), String> {
        let boxes = opened / topper.boosters.max(1);
        let packs: Vec<(&str, u32)> = topper.packs.iter().map(|(k, n)| (k.as_str(), *n)).collect();
        let new = self.store.give_box_toppers(set_code, kind, boxes, &packs).map_err(err)?;
        if new == 0 {
            return Ok(());
        }
        let name = self.box_toppers.boxes.as_ref().and_then(|b| b.get(set_code)).map_or_else(|| set_code.to_string(), |(n, _)| n.clone());
        for (pack, count) in &topper.packs {
            let quantity = count * new;
            match self.box_toppers.given.iter_mut().find(|g| g.set_code == set_code && g.kind == *pack) {
                Some(g) => g.quantity += quantity,
                None => self.box_toppers.given.push(ToppersGiven {
                    set_code: set_code.to_string(),
                    set_name: name.clone(),
                    kind: pack.clone(),
                    quantity,
                }),
            }
        }
        self.emit(vec![Event::new("BoxTopperEarned").with("setCode", set_code).with("kind", kind).with("boxes", new)]);
        Ok(())
    }

    /// The box toppers given since last asked, to show the player. The first call of a session
    /// also gives those of the boxes opened before (boosters opened before box toppers existed,
    /// or with older data).
    pub fn take_box_toppers(&mut self) -> Result<Vec<ToppersGiven>, String> {
        if !self.box_toppers.caught_up {
            self.box_toppers.caught_up = true;
            self.ensure_boxes();
            let due: Vec<(String, String, BoxTopper, u32)> = self
                .store
                .opened_by_kind()
                .map_err(err)?
                .into_iter()
                .filter_map(|(set, kind, opened)| {
                    let topper = self.box_toppers.boxes.as_ref()?.get(&set)?.1.get(&kind)?.clone();
                    Some((set, kind, topper, opened))
                })
                .collect();
            for (set, kind, topper, opened) in due {
                self.give_boxes(&set, &kind, &topper, opened)?;
            }
        }
        Ok(std::mem::take(&mut self.box_toppers.given))
    }
}
