//! Artifacts: unique curios that help the player's economy (see `config/artifacts.json`).
//! Each one is found once at most: bought at the Boosterium, found in a booster, given by an
//! achievement, or given once every other one is owned. Some require others first (the keys).
//! Their effects never touch what a booster gives: only prices, sales, income, help and bets.
//! Adding an artifact takes no code, as long as its effects are of the kinds below.

use crate::booster::ConfigError;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Deserialize)]
pub struct ArtifactsConfig {
    pub artifacts: Vec<ArtifactDef>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactDef {
    pub id: String,
    /// "common", "uncommon", "rare" or "mythic": how hard it is to get.
    pub rarity: String,
    /// The drawing that shows it (`app/src/artifactGlyphs.ts`).
    pub glyph: String,
    /// English texts; the other languages live in `config/artifacts.<lang>.json`.
    pub name: String,
    pub effect: String,
    pub flavor: String,
    pub source: ArtifactSource,
    /// Artifacts to own before this one can be got (the keys, one after the other).
    #[serde(default)]
    pub requires: Vec<String>,
    pub effects: Vec<ArtifactEffect>,
}

/// How an artifact is got.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ArtifactSource {
    /// Bought at the Boosterium.
    Shop { price_cents: u32 },
    /// Found once in `one_in` boosters opened (of the kinds listed, any kind if none; of sets
    /// released before a date, any set if none).
    Booster {
        one_in: u32,
        #[serde(default)]
        kinds: Vec<String>,
        #[serde(default)]
        released_before: Option<String>,
    },
    /// Given when an achievement unlocks.
    Achievement { id: String },
    /// Given once every other artifact is owned.
    AllOthers,
}

/// What an artifact does. Percents are of the amount concerned.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ArtifactEffect {
    /// Cards matching are sold at `percent` of their market price (the best rate wins): of the
    /// rarities listed (any if none), worth at least `min_market_cents`, foil copies only if
    /// `foil`, of sets released before a date (any if none).
    SellRate {
        percent: u32,
        #[serde(default)]
        rarities: Vec<String>,
        #[serde(default)]
        min_market_cents: u32,
        #[serde(default)]
        foil: bool,
        #[serde(default)]
        released_before: Option<String>,
    },
    /// Selling duplicates pays `percent` more.
    DuplicatesBonus { percent: u32 },
    /// Boosters cost `percent` less: of the kinds listed (any if none), of sets released
    /// before a date (any if none). Discounts add up, to 90 % at most.
    BoosterDiscount {
        percent: u32,
        #[serde(default)]
        kinds: Vec<String>,
        #[serde(default)]
        released_before: Option<String>,
    },
    /// The collection earns `percent` more.
    IncomeBonus { percent: u32 },
    /// The collection earns `percent` more per set completed, up to `max_percent`.
    IncomePerCompletedSet { percent: u32, max_percent: u32 },
    /// The collection's income, every other bonus counted, is multiplied.
    IncomeMultiplier { factor: u32 },
    /// The collection earns while away for this long at most (the longest wins).
    OfflineCap { minutes: u32 },
    /// The daily help is multiplied.
    CharityMultiplier { factor: u32 },
    /// Undoing a sale costs `percent` of its usual penalty.
    UndoPenalty { percent: u32 },
    /// A bet won pays `percent` more (the odds shown stay fair: this comes on top).
    BetWinBonus { percent: u32 },
    /// Once a day, the stake of a bet lost is given back.
    DailyBetRefund,
}

/// A card being sold, as the artifacts' sell rates see it.
#[derive(Debug, Clone, Copy)]
pub struct SoldCard<'a> {
    pub rarity: &'a str,
    pub market_cents: u32,
    pub foil: bool,
    /// The release date of its set ("YYYY-MM-DD"), empty when unknown.
    pub released: &'a str,
}

/// Whether a set released on `released` is older than `before` (an unknown date never is).
fn released_before(released: &str, before: Option<&str>) -> bool {
    before.is_none_or(|b| !released.is_empty() && released < b)
}

#[derive(Debug, Clone, PartialEq)]
struct SellRule {
    percent: u32,
    rarities: Vec<String>,
    min_market_cents: u32,
    foil: bool,
    released_before: Option<String>,
}

/// The effects of the artifacts owned, put together (`default`: none, everything as usual).
#[derive(Debug, Clone, PartialEq)]
pub struct Bonuses {
    sell_rates: Vec<SellRule>,
    pub duplicates_percent: u32,
    discounts: Vec<(u32, Vec<String>, Option<String>)>,
    pub income_percent: u32,
    income_per_set: Option<(u32, u32)>,
    /// What the income is multiplied by, after the other bonuses.
    pub income_factor: u32,
    pub offline_cap_minutes: Option<u32>,
    pub charity_factor: u32,
    pub undo_penalty_percent: u32,
    pub bet_win_percent: u32,
    pub daily_bet_refund: bool,
}

impl Default for Bonuses {
    fn default() -> Self {
        Self {
            sell_rates: Vec::new(),
            duplicates_percent: 0,
            discounts: Vec::new(),
            income_percent: 0,
            income_per_set: None,
            income_factor: 1,
            offline_cap_minutes: None,
            charity_factor: 1,
            undo_penalty_percent: 100,
            bet_win_percent: 0,
            daily_bet_refund: false,
        }
    }
}

impl Bonuses {
    /// The sell rate of a card: the best of `base` and of the artifacts' rates that apply.
    pub fn sell_rate(&self, base: u32, card: SoldCard<'_>) -> u32 {
        self.sell_rates
            .iter()
            .filter(|r| {
                (r.rarities.is_empty() || r.rarities.iter().any(|x| x == card.rarity))
                    && card.market_cents >= r.min_market_cents
                    && (!r.foil || card.foil)
                    && released_before(card.released, r.released_before.as_deref())
            })
            .map(|r| r.percent)
            .fold(base, u32::max)
    }

    /// The discount on a booster of `kind` from a set released on `released` ("YYYY-MM-DD"),
    /// in percent (90 at most).
    pub fn booster_discount(&self, kind: &str, released: &str) -> u32 {
        self.discounts
            .iter()
            .filter(|(_, kinds, before)| {
                (kinds.is_empty() || kinds.iter().any(|k| k == kind)) && released_before(released, before.as_deref())
            })
            .map(|(percent, _, _)| *percent)
            .sum::<u32>()
            .min(90)
    }

    /// The bonus on the collection's income, in percent, with `completed_sets` sets complete.
    pub fn income_bonus(&self, completed_sets: usize) -> u32 {
        let per_set = self.income_per_set.map_or(0, |(percent, max)| (percent.saturating_mul(completed_sets as u32)).min(max));
        self.income_percent + per_set
    }

    /// Whether the income bonus depends on the sets completed (worth counting them).
    pub fn counts_completed_sets(&self) -> bool {
        self.income_per_set.is_some()
    }
}

impl ArtifactsConfig {
    pub fn from_json(text: &str) -> Result<Self, ConfigError> {
        let config: Self = serde_json::from_str(text).map_err(|e| ConfigError::Invalid(format!("artifacts: {e}")))?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        let bad = |msg: String| Err(ConfigError::Invalid(format!("artifacts: {msg}")));
        let mut ids = HashSet::new();
        for a in &self.artifacts {
            if a.id.is_empty() || !a.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
                return bad(format!("invalid id `{}`", a.id));
            }
            if !ids.insert(a.id.as_str()) {
                return bad(format!("duplicate id `{}`", a.id));
            }
            if !["common", "uncommon", "rare", "mythic"].contains(&a.rarity.as_str()) {
                return bad(format!("{}: unknown rarity `{}`", a.id, a.rarity));
            }
            match &a.source {
                ArtifactSource::Shop { price_cents: 0 } => return bad(format!("{}: free in the shop", a.id)),
                ArtifactSource::Booster { one_in: 0, .. } => return bad(format!("{}: found in 1 in 0 boosters", a.id)),
                ArtifactSource::Booster { released_before: Some(d), .. } if !is_date(d) => {
                    return bad(format!("{}: invalid date `{d}`", a.id))
                }
                _ => {}
            }
            for effect in &a.effects {
                let ok = match effect {
                    ArtifactEffect::SellRate { percent, released_before, .. } => {
                        *percent <= 100 && released_before.as_deref().is_none_or(is_date)
                    }
                    ArtifactEffect::BoosterDiscount { percent, released_before, .. } => {
                        *percent <= 90 && released_before.as_deref().is_none_or(is_date)
                    }
                    ArtifactEffect::UndoPenalty { percent } => *percent <= 100,
                    ArtifactEffect::CharityMultiplier { factor } | ArtifactEffect::IncomeMultiplier { factor } => *factor >= 1,
                    _ => true,
                };
                if !ok {
                    return bad(format!("{}: invalid effect {effect:?}", a.id));
                }
            }
        }
        for a in &self.artifacts {
            if let Some(missing) = a.requires.iter().find(|r| !ids.contains(r.as_str()) || *r == &a.id) {
                return bad(format!("{} requires unknown artifact `{missing}`", a.id));
            }
            if let ArtifactSource::AllOthers = a.source {
                if !a.requires.is_empty() {
                    return bad(format!("{}: given for all the others, it requires nothing else", a.id));
                }
            }
        }
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&ArtifactDef> {
        self.artifacts.iter().find(|a| a.id == id)
    }

    /// The effects of the artifacts owned, put together.
    pub fn bonuses(&self, owned: &HashSet<String>) -> Bonuses {
        let mut b = Bonuses::default();
        for effect in self.artifacts.iter().filter(|a| owned.contains(&a.id)).flat_map(|a| &a.effects) {
            match effect {
                ArtifactEffect::SellRate { percent, rarities, min_market_cents, foil, released_before } => {
                    b.sell_rates.push(SellRule {
                        percent: *percent,
                        rarities: rarities.clone(),
                        min_market_cents: *min_market_cents,
                        foil: *foil,
                        released_before: released_before.clone(),
                    })
                }
                ArtifactEffect::DuplicatesBonus { percent } => b.duplicates_percent += percent,
                ArtifactEffect::BoosterDiscount { percent, kinds, released_before } => {
                    b.discounts.push((*percent, kinds.clone(), released_before.clone()))
                }
                ArtifactEffect::IncomeBonus { percent } => b.income_percent += percent,
                ArtifactEffect::IncomePerCompletedSet { percent, max_percent } => {
                    let (p, m) = b.income_per_set.unwrap_or((0, 0));
                    b.income_per_set = Some((p + percent, m + max_percent));
                }
                ArtifactEffect::IncomeMultiplier { factor } => b.income_factor *= factor,
                ArtifactEffect::OfflineCap { minutes } => {
                    b.offline_cap_minutes = Some(b.offline_cap_minutes.unwrap_or(0).max(*minutes))
                }
                ArtifactEffect::CharityMultiplier { factor } => b.charity_factor *= factor,
                ArtifactEffect::UndoPenalty { percent } => b.undo_penalty_percent = b.undo_penalty_percent * percent / 100,
                ArtifactEffect::BetWinBonus { percent } => b.bet_win_percent += percent,
                ArtifactEffect::DailyBetRefund => b.daily_bet_refund = true,
            }
        }
        b
    }

    /// Whether an artifact not owned can be got now: everything it requires is owned.
    pub fn can_get(&self, artifact: &ArtifactDef, owned: &HashSet<String>) -> bool {
        !owned.contains(&artifact.id) && artifact.requires.iter().all(|r| owned.contains(r))
    }

    /// Rolls the artifacts a booster of `kind`, from a set released on `released`, may hide,
    /// rarest first; at most one is found. Uses its own draws of `rng`, after the booster's
    /// cards: the cards are never affected.
    pub fn roll_booster<R: Rng + ?Sized>(
        &self,
        kind: &str,
        released: &str,
        owned: &HashSet<String>,
        rng: &mut R,
    ) -> Option<&ArtifactDef> {
        let mut candidates: Vec<(&ArtifactDef, u32)> = self
            .artifacts
            .iter()
            .filter(|a| self.can_get(a, owned))
            .filter_map(|a| match &a.source {
                ArtifactSource::Booster { one_in, kinds, released_before: before }
                    if (kinds.is_empty() || kinds.iter().any(|k| k == kind)) && released_before(released, before.as_deref()) =>
                {
                    Some((a, *one_in))
                }
                _ => None,
            })
            .collect();
        candidates.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.id.cmp(&b.0.id)));
        candidates.into_iter().find(|(_, one_in)| rng.gen_range(0..*one_in) == 0).map(|(a, _)| a)
    }

    /// The artifacts an achievement gives, when it unlocks.
    pub fn for_achievement<'a>(&'a self, achievement: &'a str) -> impl Iterator<Item = &'a ArtifactDef> + 'a {
        self.artifacts.iter().filter(move |a| matches!(&a.source, ArtifactSource::Achievement { id } if id == achievement))
    }

    /// The artifact given for owning all the others, if it is due now.
    pub fn all_others_due(&self, owned: &HashSet<String>) -> Option<&ArtifactDef> {
        let last = self.artifacts.iter().find(|a| a.source == ArtifactSource::AllOthers)?;
        let others = self.artifacts.iter().filter(|a| a.id != last.id).all(|a| owned.contains(&a.id));
        (others && !owned.contains(&last.id)).then_some(last)
    }
}

/// Whether `text` looks like a "YYYY-MM-DD" date.
fn is_date(text: &str) -> bool {
    text.len() == 10 && text.chars().enumerate().all(|(i, c)| if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() })
}

/// Names, effects and flavour texts in another language (`config/artifacts.<lang>.json`),
/// by artifact id.
pub type ArtifactTranslations = BTreeMap<String, ArtifactText>;

#[derive(Debug, Clone, Deserialize)]
pub struct ArtifactText {
    pub name: String,
    pub effect: String,
    pub flavor: String,
}
