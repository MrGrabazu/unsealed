//! Achievements as seen by the game: turns what happens into engine events, pays the
//! rewards, and builds the views shown to the player. The rules themselves live in
//! `unsealed_core::achievements` and in `config/achievements.json`.

use super::{err, Game, OpenedCard, SaleRequest, SetSummary};
use crate::store::GrantedReward;
use serde::Serialize;
use std::collections::HashMap;
use unsealed_core::achievements::{AchievementDef, Event, Progress, Reward, SealedReward};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProgressView {
    pub current: i64,
    pub goal: i64,
}

/// An achievement card as the player may see it. A locked secret hides everything but
/// its rarity: name, goal, reward and artwork stay unknown until it is unlocked.
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
    pub reward: Option<String>,
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
    pub fn achievements(&mut self) -> Result<AchievementsView, String> {
        self.ensure_prices();
        let progress = self.store.load_progress().map_err(err)?;
        let sets = self.list_sets().unwrap_or_default();
        let items: Vec<AchievementView> = self
            .engine
            .definitions()
            .iter()
            .enumerate()
            .map(|(i, def)| self.view_of(i + 1, def, &progress, &sets))
            .collect();
        let unlocked_count = items.iter().filter(|a| a.unlocked).count();
        let secret_remaining = items.iter().filter(|a| a.secret && !a.unlocked).count();
        Ok(AchievementsView { total: items.len(), unlocked_count, secret_remaining, items })
    }

    /// Achievements unlocked since the last call, so the UI can celebrate them.
    pub fn take_unlocks(&mut self) -> Vec<AchievementView> {
        std::mem::take(&mut self.notifications)
    }

    // ---- events -------------------------------------------------------------------

    /// Feeds events to the engine. The action that caused them is already saved, so a
    /// failure here must not fail it: it is reported and the events are lost.
    pub(super) fn emit(&mut self, events: Vec<Event>) {
        if events.is_empty() {
            return;
        }
        if let Err(e) = self.try_emit(events) {
            eprintln!("achievement engine: {e}");
        }
    }

    fn try_emit(&mut self, events: Vec<Event>) -> Result<(), String> {
        let now = (self.clock)().now;
        let mut progress = self.store.load_progress().map_err(err)?;
        let before = progress.clone();
        let mut unlocked: Vec<String> = Vec::new();
        for event in events {
            unlocked.extend(self.engine.handle(&mut progress, event, now).into_iter().map(|u| u.id));
        }
        if progress == before {
            return Ok(());
        }
        self.ensure_prices();
        let sets = self.list_sets().unwrap_or_default();
        let rewards: Vec<GrantedReward> = unlocked.iter().filter_map(|id| self.resolve_reward(id, &sets)).collect();
        self.store.save_progress(&progress, &rewards, now).map_err(err)?;

        for id in &unlocked {
            let found = self.engine.definitions().iter().enumerate().find(|(_, d)| d.id == *id);
            if let Some((i, def)) = found {
                let view = self.view_of(i + 1, def, &progress, &sets);
                self.notifications.push(view);
            }
        }
        Ok(())
    }

    pub(super) fn balance_event(&self) -> Option<Event> {
        let balance = self.store.balance().ok()?;
        Some(Event::new("BalanceChanged").with("balanceCents", balance))
    }

    /// Events of an opened booster: the booster itself, each card, and the set progress.
    pub(super) fn opening_events(
        &self,
        set_code: &str,
        kind: &str,
        cards: &[OpenedCard],
        paid_cents: i64,
    ) -> Vec<Event> {
        let value: i64 = cards.iter().map(|c| i64::from(c.price_cents.unwrap_or(0))).sum();
        let net_sale: i64 = cards.iter().map(|c| i64::from(self.sale_value(&c.uuid, c.foil))).sum();
        let count = |keep: &dyn Fn(&OpenedCard) -> bool| cards.iter().filter(|c| keep(c)).count() as i64;

        let mut events = vec![Event::new("BoosterOpened")
            .with("setCode", set_code)
            .with("kind", kind)
            .with("valueCents", value)
            .with("paidCents", paid_cents)
            .with("profitCents", net_sale - paid_cents)
            .with("cardCount", cards.len() as i64)
            .with("newCardCount", count(&|c| c.is_new))
            .with("rareCount", count(&|c| c.rarity == "rare"))
            .with("mythicCount", count(&|c| c.rarity == "mythic"))
            .with("foilCount", count(&|c| c.foil))
            .with("foilMythicCount", count(&|c| c.foil && c.rarity == "mythic"))
            .with("topCardValueCents", cards.iter().map(|c| i64::from(c.price_cents.unwrap_or(0))).max().unwrap_or(0))];
        for c in cards {
            events.push(
                Event::new("CardAcquired")
                    .with("uuid", c.uuid.as_str())
                    .with("setCode", c.set_code.as_str())
                    .with("rarity", c.rarity.as_str())
                    .with("foil", c.foil)
                    .with("priceCents", i64::from(c.price_cents.unwrap_or(0)))
                    .with("isNew", c.is_new),
            );
        }
        events.extend(self.set_progress_event(set_code));
        events.extend(self.balance_event());
        events
    }

    /// How far the player is in a set: overall, and for rares and mythics.
    fn set_progress_event(&self, set_code: &str) -> Option<Event> {
        let owned = self.store.owned_in_set(set_code).ok()?;
        let set = self.sets.get(set_code)?;
        // (cards, owned) overall, for rares and for mythics.
        let (mut all, mut rares, mut mythics) = ((0i64, 0i64), (0i64, 0i64), (0i64, 0i64));
        for &i in &set.obtainable {
            let card = &set.file.cards[i];
            let have = owned.get(&card.uuid).is_some_and(|o| o.normal + o.foil > 0);
            let bump = |t: &mut (i64, i64)| {
                t.0 += 1;
                t.1 += i64::from(have);
            };
            bump(&mut all);
            match card.rarity.as_str() {
                "rare" => bump(&mut rares),
                "mythic" => bump(&mut mythics),
                _ => {}
            }
        }
        let permille = if all.0 == 0 { 0 } else { all.1 * 1000 / all.0 };
        Some(
            Event::new("SetProgress")
                .with("setCode", set_code)
                .with("completionPermille", permille)
                .with("allRaresOwned", rares.0 > 0 && rares.0 == rares.1)
                .with("allMythicsOwned", mythics.0 > 0 && mythics.0 == mythics.1),
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

    fn view_of(&self, number: usize, def: &AchievementDef, progress: &Progress, sets: &[SetSummary]) -> AchievementView {
        let unlocked_at = progress.unlocked.get(&def.id).copied();
        let unlocked = unlocked_at.is_some();
        if def.secret && !unlocked {
            return AchievementView {
                id: def.id.clone(),
                number,
                name: "Secret achievement".to_string(),
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
        AchievementView {
            id: def.id.clone(),
            number,
            name: def.name.clone(),
            description: Some(def.description.clone()),
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
            reward: self.reward_text(&def.reward, sets),
        }
    }

    fn reward_text(&self, reward: &Reward, sets: &[SetSummary]) -> Option<String> {
        let mut parts = Vec::new();
        if reward.cash_cents > 0 {
            parts.push(format!("+{}", self.money_text(i64::from(reward.cash_cents))));
        }
        if let Some(sealed) = &reward.sealed {
            let name = resolve_set(sealed, sets).map_or("newest set", |s| s.name.as_str());
            let plural = if sealed.quantity > 1 { "s" } else { "" };
            parts.push(format!("{} × {name} {} booster{plural}", sealed.quantity, sealed.kind));
        }
        (!parts.is_empty()).then(|| parts.join(" + "))
    }

    fn money_text(&self, cents: i64) -> String {
        format!("{}{}.{:02}", self.economy.currency.symbol, cents / 100, cents % 100)
    }
}

/// The set a sealed reward refers to: "latest" is the newest available set.
fn resolve_set<'a>(reward: &SealedReward, sets: &'a [SetSummary]) -> Option<&'a SetSummary> {
    if reward.set_code == "latest" {
        sets.first() // `list_sets` sorts the newest set first
    } else {
        sets.iter().find(|s| s.code == reward.set_code)
    }
}
