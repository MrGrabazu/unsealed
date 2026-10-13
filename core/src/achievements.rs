//! Event-driven achievement engine. Everything about the achievements (metrics,
//! conditions, rewards, card look) lives in `config/achievements.json`: adding an
//! achievement needs no code change.
//!
//! The game emits events (`BoosterOpened`, `CardAcquired`, `CardSold`, ...), each a name
//! plus numeric/boolean/string fields. Two mechanisms listen to them:
//! - *metrics* are counters kept up to date from events (`sum`, `max`, `streak`, `distinct`);
//! - *conditions* unlock an achievement when a metric reaches a threshold, when one event
//!   matches a filter, or when every other achievement is unlocked.
//!
//! Unlocking emits an `AchievementUnlocked` event, so meta achievements can count them.

use crate::booster::ConfigError;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Emblems the card renderer knows. The UI must implement every one of them.
pub const EMBLEMS: &[&str] = &[
    "star", "crown", "flame", "coin", "key", "eye", "gem", "book", "hourglass", "trophy", "pack", "skull",
    "bolt", "scale", "cards", "chart", "question",
];

/// Color palettes the card renderer knows. The UI must implement every one of them.
pub const PALETTES: &[&str] = &["ember", "ocean", "forest", "royal", "onyx", "gold", "rose", "ice"];

#[derive(Debug, Clone, Deserialize)]
pub struct AchievementsConfig {
    #[serde(default)]
    pub metrics: BTreeMap<String, MetricDef>,
    pub achievements: Vec<AchievementDef>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricDef {
    pub event: String,
    /// The metric only reacts to events matching all of these filters.
    #[serde(default, rename = "where")]
    pub conditions: Vec<Cond>,
    pub op: MetricOp,
    /// Event field the operation reads (`sum` counts 1 per event when absent).
    #[serde(default)]
    pub field: Option<String>,
    /// `streak` only: the counter goes back to 0 when all of these match.
    #[serde(default)]
    pub reset_when: Vec<Cond>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MetricOp {
    /// Adds the field (or 1) on each matching event.
    Sum,
    /// Keeps the largest value of the field.
    Max,
    /// Counts consecutive matching events, resetting on `reset_when`.
    Streak,
    /// Counts the distinct values of the field.
    Distinct,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Cond {
    pub field: String,
    pub op: CmpOp,
    pub value: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CmpOp {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AchievementDef {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Secret achievements hide their name, goal and reward until unlocked.
    #[serde(default)]
    pub secret: bool,
    /// Proportional to the difficulty of the achievement.
    pub rarity: Rarity,
    pub emblem: String,
    pub palette: String,
    pub condition: Condition,
    #[serde(default)]
    pub reward: Reward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Mythic,
}

impl Rarity {
    pub fn as_str(self) -> &'static str {
        match self {
            Rarity::Common => "common",
            Rarity::Uncommon => "uncommon",
            Rarity::Rare => "rare",
            Rarity::Mythic => "mythic",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Condition {
    /// A metric reaches a threshold.
    Metric { metric: String, at_least: i64 },
    /// A single event matches (an instant achievement).
    Event {
        event: String,
        #[serde(default, rename = "where")]
        conditions: Vec<Cond>,
    },
    /// Every other achievement is unlocked (the ultimate one).
    AllOthers,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reward {
    #[serde(default)]
    pub cash_cents: u32,
    #[serde(default)]
    pub sealed: Option<SealedReward>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedReward {
    /// A set code, or "latest" for the most recent set available.
    pub set_code: String,
    pub kind: String,
    pub quantity: u32,
}

/// Something that happened in the game.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub name: String,
    pub fields: BTreeMap<String, Value>,
}

impl Event {
    pub fn new(name: &str) -> Self {
        Self { name: name.to_string(), fields: BTreeMap::new() }
    }

    pub fn with(mut self, field: &str, value: impl Into<Value>) -> Self {
        self.fields.insert(field.to_string(), value.into());
        self
    }

    fn int(&self, field: &str) -> Option<i64> {
        self.fields.get(field)?.as_i64()
    }
}

impl Cond {
    fn matches(&self, event: &Event) -> bool {
        event.fields.get(&self.field).is_some_and(|actual| compare(actual, &self.value, self.op))
    }
}

fn compare(actual: &Value, expected: &Value, op: CmpOp) -> bool {
    use std::cmp::Ordering;
    let ordering = match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => a.as_f64().zip(b.as_f64()).and_then(|(a, b)| a.partial_cmp(&b)),
        (Value::String(a), Value::String(b)) => Some(a.cmp(b)),
        (Value::Bool(a), Value::Bool(b)) => Some(a.cmp(b)),
        _ => None,
    };
    match (op, ordering) {
        (CmpOp::Eq, o) => o == Some(Ordering::Equal),
        (CmpOp::Ne, o) => o != Some(Ordering::Equal),
        (CmpOp::Gt, Some(o)) => o == Ordering::Greater,
        (CmpOp::Gte, Some(o)) => o != Ordering::Less,
        (CmpOp::Lt, Some(o)) => o == Ordering::Less,
        (CmpOp::Lte, Some(o)) => o != Ordering::Greater,
        _ => false,
    }
}

/// What the engine remembers about a player. Persisted by the app.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Progress {
    pub metrics: BTreeMap<String, i64>,
    /// Values already seen by `distinct` metrics.
    pub seen: BTreeMap<String, BTreeSet<String>>,
    /// Unlocked achievement id -> unlock time.
    pub unlocked: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unlock {
    pub id: String,
    pub at: i64,
}

pub struct Engine {
    config: AchievementsConfig,
}

impl Engine {
    pub fn from_json(json: &str) -> Result<Self, ConfigError> {
        Self::new(serde_json::from_str(json)?)
    }

    pub fn new(config: AchievementsConfig) -> Result<Self, ConfigError> {
        let invalid = |msg: String| Err(ConfigError::Invalid(msg));
        let mut ids = BTreeSet::new();
        for (name, m) in &config.metrics {
            let needs_field = matches!(m.op, MetricOp::Max | MetricOp::Distinct);
            if needs_field && m.field.is_none() {
                return invalid(format!("metric `{name}`: operation {:?} needs a `field`", m.op));
            }
        }
        for a in &config.achievements {
            if a.id.is_empty() || !ids.insert(a.id.as_str()) {
                return invalid(format!("achievement id `{}` is empty or duplicated", a.id));
            }
            if !EMBLEMS.contains(&a.emblem.as_str()) {
                return invalid(format!("achievement `{}`: unknown emblem `{}`", a.id, a.emblem));
            }
            if !PALETTES.contains(&a.palette.as_str()) {
                return invalid(format!("achievement `{}`: unknown palette `{}`", a.id, a.palette));
            }
            if let Condition::Metric { metric, at_least } = &a.condition {
                if !config.metrics.contains_key(metric) {
                    return invalid(format!("achievement `{}`: unknown metric `{metric}`", a.id));
                }
                if *at_least <= 0 {
                    return invalid(format!("achievement `{}`: atLeast must be positive", a.id));
                }
            }
        }
        Ok(Self { config })
    }

    pub fn definitions(&self) -> &[AchievementDef] {
        &self.config.achievements
    }

    /// Feeds one event to the engine and returns the achievements it unlocked, in order.
    /// Unlocking emits `AchievementUnlocked`, which can unlock further achievements.
    pub fn handle(&self, progress: &mut Progress, event: Event, now: i64) -> Vec<Unlock> {
        let mut unlocks = Vec::new();
        let mut queue = VecDeque::from([event]);
        while let Some(event) = queue.pop_front() {
            self.update_metrics(progress, &event);
            for def in &self.config.achievements {
                if progress.unlocked.contains_key(&def.id) || !self.is_met(def, progress, &event) {
                    continue;
                }
                progress.unlocked.insert(def.id.clone(), now);
                unlocks.push(Unlock { id: def.id.clone(), at: now });
                queue.push_back(
                    Event::new("AchievementUnlocked")
                        .with("id", def.id.as_str())
                        .with("rarity", def.rarity.as_str())
                        .with("secret", def.secret),
                );
            }
        }
        unlocks
    }

    /// Progress towards an achievement as (current, goal), when it can be measured.
    pub fn progress_of(&self, def: &AchievementDef, progress: &Progress) -> Option<(i64, i64)> {
        match &def.condition {
            Condition::Metric { metric, at_least } => {
                let current = progress.metrics.get(metric).copied().unwrap_or(0);
                Some((current.min(*at_least), *at_least))
            }
            Condition::Event { .. } => None,
            Condition::AllOthers => {
                let others = self.config.achievements.iter().filter(|a| a.id != def.id);
                let total = others.clone().count() as i64;
                let done = others.filter(|a| progress.unlocked.contains_key(&a.id)).count() as i64;
                Some((done, total))
            }
        }
    }

    fn update_metrics(&self, progress: &mut Progress, event: &Event) {
        for (name, def) in &self.config.metrics {
            if def.event != event.name || !def.conditions.iter().all(|c| c.matches(event)) {
                continue;
            }
            let field_int = || def.field.as_deref().and_then(|f| event.int(f));
            match def.op {
                MetricOp::Sum => {
                    let amount = if def.field.is_some() { field_int() } else { Some(1) };
                    if let Some(amount) = amount {
                        *progress.metrics.entry(name.clone()).or_insert(0) += amount;
                    }
                }
                MetricOp::Max => {
                    if let Some(value) = field_int() {
                        let entry = progress.metrics.entry(name.clone()).or_insert(value);
                        *entry = (*entry).max(value);
                    }
                }
                MetricOp::Streak => {
                    let reset = !def.reset_when.is_empty() && def.reset_when.iter().all(|c| c.matches(event));
                    let entry = progress.metrics.entry(name.clone()).or_insert(0);
                    *entry = if reset { 0 } else { *entry + 1 };
                }
                MetricOp::Distinct => {
                    let Some(value) = def.field.as_deref().and_then(|f| event.fields.get(f)) else { continue };
                    let seen = progress.seen.entry(name.clone()).or_default();
                    if seen.insert(value.to_string()) {
                        progress.metrics.insert(name.clone(), seen.len() as i64);
                    }
                }
            }
        }
    }

    fn is_met(&self, def: &AchievementDef, progress: &Progress, event: &Event) -> bool {
        match &def.condition {
            Condition::Metric { metric, at_least } => {
                progress.metrics.get(metric).copied().unwrap_or(0) >= *at_least
            }
            Condition::Event { event: name, conditions } => {
                event.name == *name && conditions.iter().all(|c| c.matches(event))
            }
            Condition::AllOthers => self
                .config
                .achievements
                .iter()
                .filter(|a| a.id != def.id)
                .all(|a| progress.unlocked.contains_key(&a.id)),
        }
    }
}
