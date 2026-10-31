//! Achievements as seen by the game: turns what happens into engine events, pays the
//! rewards, and builds the views shown to the player. The rules themselves live in
//! `unsealed_core::achievements` and in `config/achievements.json`.

use super::{err, Game, OpenedCard, SaleRequest, SetSummary};
use crate::store::GrantedReward;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use unsealed_core::achievements::{AchievementDef, Event, Progress, Reward, SealedReward};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProgressView {
    pub current: i64,
    pub goal: i64,
}

/// An achievement card as the player may see it. A locked secret hides everything but
/// its rarity: name (empty), goal, reward and artwork stay unknown until it is unlocked.
/// Name and description are in the requested language when translated, else in English.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AchievementView {
    pub id: String,
    /// Position in the achievement collection (1-based).
    pub number: usize,
    pub name: String,
    pub description: Option<String>,
    pub secret: bool,
    pub rarity: String,
    pub emblem: String,
    pub palette: String,
    pub unlocked: bool,
    pub unlocked_at: Option<i64>,
    pub progress: Option<ProgressView>,
    /// Described as data, so the interface words it in its own language.
    pub reward: Option<RewardView>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RewardView {
    pub cash_cents: u32,
    pub sealed: Option<SealedRewardView>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SealedRewardView {
    /// `None` when the reward is "the newest set" and no data is available yet.
    pub set_name: Option<String>,
    pub kind: String,
    pub quantity: u32,
}

/// Translated name and description of one achievement.
#[derive(Debug, Clone, Deserialize)]
pub struct AchievementText {
    pub name: String,
    pub description: String,
}

/// Translations of the achievements: language code -> achievement id -> texts. Like the
/// definitions, they live in configuration files (`config/achievements.<lang>.json`).
#[derive(Debug, Clone, Default)]
pub struct AchievementTexts(HashMap<String, HashMap<String, AchievementText>>);

impl AchievementTexts {
    /// Adds a language from the JSON of its file (achievement id -> name and description).
    pub fn add_language(&mut self, lang: &str, json: &str) -> Result<(), serde_json::Error> {
        self.0.insert(lang.to_string(), serde_json::from_str(json)?);
        Ok(())
    }

    fn get(&self, lang: &str, id: &str) -> Option<&AchievementText> {
        self.0.get(lang)?.get(id)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AchievementsView {
    pub total: usize,
    pub unlocked_count: usize,
    pub secret_remaining: usize,
    pub items: Vec<AchievementView>,
}

/// What the player owned before a sale, to describe the sale (last copy, best card...).
pub(super) struct SaleContext {
    copies: HashMap<String, u32>,
    max_market_cents: u32,
}

impl Game {
    pub fn set_achievement_texts(&mut self, texts: AchievementTexts) {
        self.achievement_texts = texts;
    }

    /// Every achievement, its texts in `lang` (English when not translated).
    pub fn achievements(&mut self, lang: &str) -> Result<AchievementsView, String> {
        self.ensure_prices();
        let progress = self.store.load_progress().map_err(err)?;
        let sets = self.list_sets().unwrap_or_default();
        let items: Vec<AchievementView> = self
            .engine
            .definitions()
            .iter()
            .enumerate()
            .map(|(i, def)| self.view_of(i + 1, def, &progress, &sets, lang))
            .collect();
        let unlocked_count = items.iter().filter(|a| a.unlocked).count();
        let secret_remaining = items.iter().filter(|a| a.secret && !a.unlocked).count();
        Ok(AchievementsView { total: items.len(), unlocked_count, secret_remaining, items })
    }

    /// Achievements unlocked since the last call, so the UI can celebrate them.
    pub fn take_unlocks(&mut self, lang: &str) -> Vec<AchievementView> {
        let ids = std::mem::take(&mut self.notifications);
        if ids.is_empty() {
            return Vec::new();
        }
        let progress = self.store.load_progress().unwrap_or_default();
        let sets = self.list_sets().unwrap_or_default();
        ids.iter()
            .filter_map(|id| {
                let (i, def) = self.engine.definitions().iter().enumerate().find(|(_, d)| d.id == *id)?;
                Some(self.view_of(i + 1, def, &progress, &sets, lang))
            })
            .collect()
    }

    // ---- events -------------------------------------------------------------------

    /// Feeds events to the engine. The action that caused them is already saved, so a
    /// failure here must not fail it: it is reported and the events are lost.
    pub(super) fn emit(&mut self, events: Vec<Event>) {
        if events.is_empty() {
            return;
        }
        match self.try_emit(events) {
            // The artifacts these achievements give, whose own events may unlock more.
            Ok(unlocked) => {
                let given = self.give_achievement_artifacts(&unlocked);
                self.emit(given);
            }
            Err(e) => eprintln!("achievement engine: {e}"),
        }
    }

    /// Returns the achievements just unlocked.
    fn try_emit(&mut self, events: Vec<Event>) -> Result<Vec<String>, String> {
        let now = (self.clock)().now;
        let mut progress = self.store.load_progress().map_err(err)?;
        let before = progress.clone();
        let mut unlocked: Vec<String> = Vec::new();
        for event in events {
            unlocked.extend(self.engine.handle(&mut progress, event, now).into_iter().map(|u| u.id));
        }
        if progress == before {
            return Ok(Vec::new());
        }
        self.ensure_prices();
        let sets = self.list_sets().unwrap_or_default();
        let rewards: Vec<GrantedReward> = unlocked.iter().filter_map(|id| self.resolve_reward(id, &sets)).collect();
        self.store.save_progress(&progress, &rewards, now).map_err(err)?;
        self.notifications.extend(unlocked.iter().cloned());
        Ok(unlocked)
    }

    /// A save edited outside the game was refused (see `seal`): its secret achievement, for
    /// fun, rewarding nothing.
    pub fn caught_editing_a_save(&mut self) {
        self.emit(vec![Event::new("SaveTampered")]);
    }

    pub(super) fn balance_event(&self) -> Option<Event> {
        let balance = self.store.balance().ok()?;
        Some(Event::new("BalanceChanged").with("balanceCents", balance))
    }

    /// Events of an opened booster: one `CardAcquired` per card (in reveal order), and those
    /// about the whole booster (the booster itself, set and year progress, balance), which
    /// are only known once every card is seen.
    pub(super) fn opening_events(
        &mut self,
        set_code: &str,
        kind: &str,
        cards: &[OpenedCard],
        paid_cents: i64,
        animated: bool,
    ) -> OpeningEvents {
        let value: i64 = cards.iter().map(|c| i64::from(c.price_cents.unwrap_or(0))).sum();
        let net_sale: i64 = cards.iter().map(|c| i64::from(self.sale_value(&c.uuid, c.foil))).sum();
        let count = |keep: &dyn Fn(&OpenedCard) -> bool| cards.iter().filter(|c| keep(c)).count() as i64;

        let per_card = cards
            .iter()
            .map(|c| {
                Event::new("CardAcquired")
                    .with("uuid", c.uuid.as_str())
                    .with("setCode", c.set_code.as_str())
                    .with("rarity", c.rarity.as_str())
                    .with("foil", c.foil)
                    .with("priceCents", i64::from(c.price_cents.unwrap_or(0)))
                    .with("isNew", c.is_new)
                    .with("newFoil", c.new_foil)
                    // A Secret Lair card found in another set's booster.
                    .with("craftedGuest", c.sheet == super::CRAFTED_GUEST_SHEET)
                    // A Japanese booster's own card found in an English one.
                    .with("japaneseGuest", c.sheet == super::JAPANESE_GUEST_SHEET)
            })
            .collect();
        let mut booster = vec![Event::new("BoosterOpened")
            .with("setCode", set_code)
            .with("kind", kind)
            // Watched (the pack torn, the cards revealed one by one) or skipped (quick, batch).
            .with("animated", animated)
            .with("valueCents", value)
            .with("paidCents", paid_cents)
            .with("profitCents", net_sale - paid_cents)
            .with("cardCount", cards.len() as i64)
            .with("newCardCount", count(&|c| c.is_new))
            .with("newFoilCount", count(&|c| c.new_foil))
            .with("rareCount", count(&|c| c.rarity == "rare"))
            .with("mythicCount", count(&|c| c.rarity == "mythic"))
            .with("foilCount", count(&|c| c.foil))
            .with("foilMythicCount", count(&|c| c.foil && c.rarity == "mythic"))
            .with("topCardValueCents", cards.iter().map(|c| i64::from(c.price_cents.unwrap_or(0))).max().unwrap_or(0))];
        booster.extend(self.set_progress_event(set_code));
        booster.extend(self.cycle_events(set_code));
        booster.extend(self.scene_events(set_code));
        booster.extend(self.assembly_events(set_code));
        booster.extend(self.year_progress_event(set_code));
        // A card of another set found in it (Secret Lair): that set's progress and drops too.
        let mut others: Vec<&str> =
            cards.iter().filter(|c| c.sheet == super::CRAFTED_GUEST_SHEET).map(|c| c.set_code.as_str()).collect();
        others.dedup();
        for other in others {
            booster.extend(self.set_progress_event(other));
            booster.extend(self.cycle_events(other));
        }
        booster.extend(self.balance_event());
        OpeningEvents { per_card, booster }
    }

    /// Emits the events of the cards revealed so far, up to `count` cards (all of them with
    /// `usize::MAX`), then those of the whole booster once every card is revealed. Before
    /// that, achievements would give away what the pile holds.
    pub fn reveal_cards(&mut self, count: usize) {
        let Some(pending) = self.pending_reveal.as_mut() else { return };
        let upto = count.min(pending.per_card.len()).max(pending.revealed);
        let mut events: Vec<Event> = pending.per_card[pending.revealed..upto].to_vec();
        pending.revealed = upto;
        let complete = upto == pending.per_card.len();
        if complete {
            events.extend(std::mem::take(&mut pending.booster));
            self.pending_reveal = None;
        }
        self.emit(events);
        // Every card is seen: the bets placed on the booster pay now.
        if complete {
            self.pay_bets();
        }
    }

    /// Emits whatever an opening still holds back: the player moved on.
    pub(super) fn flush_reveal(&mut self) {
        self.reveal_cards(usize::MAX);
    }
}

/// The achievement events of an opened booster, see `Game::opening_events`.
pub(super) struct OpeningEvents {
    pub per_card: Vec<Event>,
    pub booster: Vec<Event>,
}

/// An opened booster whose cards are being revealed one by one.
pub(super) struct PendingReveal {
    pub per_card: Vec<Event>,
    pub booster: Vec<Event>,
    pub revealed: usize,
}

impl Game {

    /// How far the player is in a set: overall, for rares and mythics, in foil (cards
    /// printed in foil only), for the collector extras, the tokens and the art cards (and
    /// their gold signed versions).
    fn set_progress_event(&self, set_code: &str) -> Option<Event> {
        let set = self.sets.get(set_code)?;
        // Tokens and art cards may be filed under their own set codes.
        let owned = self.owned_with_tokens(set_code).ok()?;
        // (cards, owned) overall, for rares, for mythics, in foil and for the extras.
        let (mut all, mut rares, mut mythics, mut foils, mut extras) = Default::default();
        let (mut tokens, mut arts, mut signed): ((i64, i64), (i64, i64), (i64, i64)) = Default::default();
        let bump = |t: &mut (i64, i64), have: bool| {
            t.0 += 1;
            t.1 += i64::from(have);
        };
        for &i in &set.obtainable {
            let card = &set.file.cards[i];
            let have = owned.get(&card.uuid).copied().unwrap_or_default();
            bump(&mut all, have.normal + have.foil > 0);
            match card.rarity.as_str() {
                "rare" => bump(&mut rares, have.normal + have.foil > 0),
                "mythic" => bump(&mut mythics, have.normal + have.foil > 0),
                _ => {}
            }
            if card.finishes.iter().any(|f| f == "foil") {
                bump(&mut foils, have.foil > 0);
            }
        }
        for &i in &set.extras {
            bump(&mut extras, owned.get(&set.file.cards[i].uuid).is_some_and(|o| o.normal + o.foil > 0));
        }
        for &i in &set.tokens {
            bump(&mut tokens, owned.get(&set.file.cards[i].uuid).is_some_and(|o| o.normal + o.foil > 0));
        }
        for &i in &set.arts {
            let card = &set.file.cards[i];
            let have = owned.get(&card.uuid).copied().unwrap_or_default();
            bump(&mut arts, have.normal + have.foil > 0);
            if card.finishes.iter().any(|f| f == "foil") {
                bump(&mut signed, have.foil > 0);
            }
        }
        let all_owned = |t: (i64, i64)| t.0 > 0 && t.0 == t.1;
        Some(
            Event::new("SetProgress")
                .with("setCode", set_code)
                .with("completionPermille", permille(all))
                .with("allRaresOwned", all_owned(rares))
                .with("allMythicsOwned", all_owned(mythics))
                .with("foilCompletionPermille", permille(foils))
                .with("extrasCompletionPermille", permille(extras))
                .with("tokensCompletionPermille", permille(tokens))
                .with("artsCompletionPermille", permille(arts))
                .with("signedArtsCompletionPermille", permille(signed)),
        )
    }

    /// How many of the sets released the same year as `set_code` are complete.
    pub(super) fn year_progress_event(&mut self, set_code: &str) -> Option<Event> {
        let sets = self.list_sets().ok()?;
        let year = sets.iter().find(|s| s.code == set_code)?.release_date.get(0..4)?.to_string();
        let same_year: Vec<String> =
            sets.into_iter().filter(|s| s.release_date.starts_with(&year)).map(|s| s.code).collect();
        let mut completed = 0i64;
        for code in &same_year {
            self.ensure_loaded(code).ok()?;
            let owned = self.store.owned_in_set(code).ok()?;
            let set = &self.sets[code];
            let complete = !set.obtainable.is_empty()
                && set
                    .obtainable
                    .iter()
                    .all(|&i| owned.get(&set.file.cards[i].uuid).is_some_and(|o| o.normal + o.foil > 0));
            completed += i64::from(complete);
        }
        let set_count = same_year.len() as i64;
        Some(
            Event::new("YearProgress")
                .with("year", year)
                .with("setCount", set_count)
                .with("completedSets", completed)
                .with("complete", completed == set_count),
        )
    }

    pub(super) fn sale_context(&self) -> SaleContext {
        let mut copies: HashMap<String, u32> = HashMap::new();
        let mut max_market_cents = 0;
        for (uuid, foil, quantity) in self.store.owned_cards().unwrap_or_default() {
            if let Some(market) = self.market_price(&uuid, foil) {
                max_market_cents = max_market_cents.max(market);
            }
            *copies.entry(uuid).or_insert(0) += quantity;
        }
        SaleContext { copies, max_market_cents }
    }

    pub(super) fn sale_events(&self, context: &SaleContext, requests: &[SaleRequest], total_cents: i64) -> Vec<Event> {
        let mut events = Vec::new();
        let mut card_count = 0i64;
        for r in requests {
            let unit = self.sale_value(&r.uuid, r.foil);
            let market = self.market_price(&r.uuid, r.foil).unwrap_or(0);
            events.push(
                Event::new("CardSold")
                    .with("uuid", r.uuid.as_str())
                    .with("foil", r.foil)
                    .with("quantity", r.quantity)
                    .with("unitCents", unit)
                    .with("totalCents", i64::from(unit) * i64::from(r.quantity))
                    .with("wasLastCopy", context.copies.get(&r.uuid).is_some_and(|&owned| owned <= r.quantity))
                    .with("wasMostValuableCard", market > 0 && market >= context.max_market_cents),
            );
            card_count += i64::from(r.quantity);
        }
        events.push(Event::new("SaleCompleted").with("cardCount", card_count).with("totalCents", total_cents));
        events.extend(self.balance_event());
        events
    }

    // ---- rewards and views -----------------------------------------------------------

    /// Resolves a reward against the data available ("latest" set, existing booster kind).
    fn resolve_reward(&self, id: &str, sets: &[SetSummary]) -> Option<GrantedReward> {
        let reward = &self.engine.definitions().iter().find(|d| d.id == id)?.reward;
        let sealed = reward.sealed.as_ref().and_then(|s| {
            let set = resolve_set(s, sets)?;
            set.boosters.contains(&s.kind).then(|| (set.code.clone(), s.kind.clone(), s.quantity))
        });
        (reward.cash_cents > 0 || sealed.is_some()).then(|| GrantedReward {
            achievement_id: id.to_string(),
            cash_cents: reward.cash_cents,
            sealed,
        })
    }

    fn view_of(
        &self,
        number: usize,
        def: &AchievementDef,
        progress: &Progress,
        sets: &[SetSummary],
        lang: &str,
    ) -> AchievementView {
        let unlocked_at = progress.unlocked.get(&def.id).copied();
        let unlocked = unlocked_at.is_some();
        if def.secret && !unlocked {
            return AchievementView {
                id: def.id.clone(),
                number,
                name: String::new(),
                description: None,
                secret: true,
                rarity: def.rarity.as_str().to_string(),
                emblem: "question".to_string(),
                palette: "onyx".to_string(),
                unlocked: false,
                unlocked_at: None,
                progress: None,
                reward: None,
            };
        }
        let text = self.achievement_texts.get(lang, &def.id);
        AchievementView {
            id: def.id.clone(),
            number,
            name: text.map_or(&def.name, |t| &t.name).clone(),
            description: Some(text.map_or(&def.description, |t| &t.description).clone()),
            secret: def.secret,
            rarity: def.rarity.as_str().to_string(),
            emblem: def.emblem.clone(),
            palette: def.palette.clone(),
            unlocked,
            unlocked_at,
            progress: self
                .engine
                .progress_of(def, progress)
                .map(|(current, goal)| ProgressView { current, goal }),
            reward: reward_view(&def.reward, sets),
        }
    }
}

fn reward_view(reward: &Reward, sets: &[SetSummary]) -> Option<RewardView> {
    let sealed = reward.sealed.as_ref().map(|s| SealedRewardView {
        set_name: resolve_set(s, sets).map(|set| set.name.clone()),
        kind: s.kind.clone(),
        quantity: s.quantity,
    });
    (reward.cash_cents > 0 || sealed.is_some()).then(|| RewardView { cash_cents: reward.cash_cents, sealed })
}

/// Share of `(total, owned)` that is owned, in thousandths.
fn permille((total, owned): (i64, i64)) -> i64 {
    if total == 0 {
        0
    } else {
        owned * 1000 / total
    }
}

/// The set a sealed reward refers to: "latest" is the newest set offering that booster kind
/// (the newest of all may have other kinds only, e.g. a draft-only special set).
fn resolve_set<'a>(reward: &SealedReward, sets: &'a [SetSummary]) -> Option<&'a SetSummary> {
    if reward.set_code == "latest" {
        // `list_sets` sorts the newest set first.
        sets.iter().find(|s| s.boosters.contains(&reward.kind))
    } else {
        sets.iter().find(|s| s.code == reward.set_code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(code: &str, boosters: &[&str]) -> SetSummary {
        SetSummary {
            code: code.into(),
            name: code.into(),
            english_name: code.into(),
            release_date: String::new(),
            boosters: boosters.iter().map(|b| b.to_string()).collect(),
            art: None,
            icon_url: None,
        }
    }

    #[test]
    fn latest_is_the_newest_set_offering_the_kind() {
        // Newest first, as `list_sets` returns them: the newest set has draft boosters only.
        let sets = [set("MBC", &["draft"]), set("FRA", &["collector", "play"]), set("OLD", &["play"])];
        let latest = |kind: &str| {
            let reward = SealedReward { set_code: "latest".into(), kind: kind.into(), quantity: 1 };
            resolve_set(&reward, &sets).map(|s| s.code.as_str())
        };
        assert_eq!(latest("play"), Some("FRA"));
        assert_eq!(latest("draft"), Some("MBC"));
        assert_eq!(latest("set"), None);
    }
}
