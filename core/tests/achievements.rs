use unsealed_core::achievements::{Engine, Event, Progress, Rarity};

const SHIPPED: &str = include_str!("../../config/achievements.json");

/// A small config exercising every metric operation and condition type.
const CONFIG: &str = r#"{
  "metrics": {
    "opened": { "event": "BoosterOpened", "op": "sum" },
    "value": { "event": "BoosterOpened", "op": "sum", "field": "valueCents" },
    "best": { "event": "CardAcquired", "op": "max", "field": "priceCents" },
    "no_mythic": { "event": "BoosterOpened", "op": "streak",
                   "resetWhen": [{ "field": "mythics", "op": "gt", "value": 0 }] },
    "sets": { "event": "BoosterOpened", "op": "distinct", "field": "setCode" },
    "mythics_pulled": { "event": "CardAcquired", "op": "sum",
                        "where": [{ "field": "rarity", "op": "eq", "value": "mythic" }] },
    "unlocked": { "event": "AchievementUnlocked", "op": "sum" }
  },
  "achievements": [
    { "id": "open-3", "name": "Three", "description": "Open 3 boosters.", "rarity": "common",
      "emblem": "pack", "palette": "ember",
      "condition": { "type": "metric", "metric": "opened", "atLeast": 3 } },
    { "id": "value", "name": "Worth it", "description": "x", "rarity": "uncommon",
      "emblem": "coin", "palette": "gold",
      "condition": { "type": "metric", "metric": "value", "atLeast": 1000 } },
    { "id": "chase", "name": "Chase", "description": "x", "rarity": "rare", "secret": true,
      "emblem": "gem", "palette": "royal",
      "condition": { "type": "event", "event": "CardAcquired",
                     "where": [{ "field": "priceCents", "op": "gte", "value": 5000 }] } },
    { "id": "best-100", "name": "Best", "description": "x", "rarity": "common",
      "emblem": "star", "palette": "ice",
      "condition": { "type": "metric", "metric": "best", "atLeast": 100 } },
    { "id": "drought", "name": "Drought", "description": "x", "rarity": "rare", "secret": true,
      "emblem": "hourglass", "palette": "onyx",
      "condition": { "type": "metric", "metric": "no_mythic", "atLeast": 3 } },
    { "id": "two-sets", "name": "Two sets", "description": "x", "rarity": "common",
      "emblem": "book", "palette": "ocean",
      "condition": { "type": "metric", "metric": "sets", "atLeast": 2 } },
    { "id": "shiny", "name": "Shiny", "description": "x", "rarity": "rare",
      "emblem": "star", "palette": "rose",
      "condition": { "type": "event", "event": "CardAcquired", "where": [
        { "field": "rarity", "op": "eq", "value": "mythic" }, { "field": "foil", "op": "eq", "value": true } ] } },
    { "id": "two-unlocks", "name": "Meta", "description": "x", "rarity": "uncommon",
      "emblem": "trophy", "palette": "royal",
      "condition": { "type": "metric", "metric": "unlocked", "atLeast": 2 } },
    { "id": "all", "name": "All", "description": "x", "rarity": "mythic",
      "emblem": "crown", "palette": "gold", "condition": { "type": "allOthers" } }
  ]
}"#;

fn engine() -> Engine {
    Engine::from_json(CONFIG).expect("valid test config")
}

fn opened(set: &str, value: i64, mythics: i64) -> Event {
    Event::new("BoosterOpened").with("setCode", set).with("valueCents", value).with("mythics", mythics)
}

fn ids(unlocks: &[unsealed_core::achievements::Unlock]) -> Vec<&str> {
    unlocks.iter().map(|u| u.id.as_str()).collect()
}

#[test]
fn a_metric_threshold_unlocks_once() {
    let e = engine();
    let mut p = Progress::default();
    assert!(e.handle(&mut p, opened("A", 0, 1), 1).is_empty());
    assert!(e.handle(&mut p, opened("A", 0, 1), 2).is_empty());
    let unlocks = e.handle(&mut p, opened("A", 0, 1), 3);
    assert_eq!(ids(&unlocks), ["open-3"]);
    assert_eq!(unlocks[0].at, 3);
    // Already unlocked: nothing more, whatever happens.
    assert!(e.handle(&mut p, opened("A", 0, 1), 4).is_empty());
    assert_eq!(p.unlocked["open-3"], 3);
}

#[test]
fn sum_over_a_field_accumulates() {
    let e = engine();
    let mut p = Progress::default();
    e.handle(&mut p, opened("A", 600, 1), 1);
    assert_eq!(p.metrics["value"], 600);
    assert_eq!(ids(&e.handle(&mut p, opened("A", 500, 1), 2)), ["value"]);
}

#[test]
fn an_event_filter_unlocks_instantly_and_max_tracks_the_best() {
    let e = engine();
    let mut p = Progress::default();
    let card = |price: i64| Event::new("CardAcquired").with("priceCents", price).with("rarity", "common").with("foil", false);
    assert!(e.handle(&mut p, card(40), 1).is_empty());
    assert_eq!(ids(&e.handle(&mut p, card(120), 2)), ["best-100"]);
    assert_eq!(p.metrics["best"], 120);
    // "chase" is the second unlock, so the "two unlocks" meta achievement follows at once.
    assert_eq!(ids(&e.handle(&mut p, card(9_000), 3)), ["chase", "two-unlocks"]);
    // A cheaper card never lowers the maximum.
    e.handle(&mut p, card(5), 4);
    assert_eq!(p.metrics["best"], 9_000);
}

#[test]
fn conditions_must_all_match_and_missing_fields_never_match() {
    let e = engine();
    let mut p = Progress::default();
    let card = |rarity: &str, foil: bool| Event::new("CardAcquired").with("rarity", rarity).with("foil", foil).with("priceCents", 1);
    assert!(e.handle(&mut p, card("mythic", false), 1).is_empty());
    assert!(e.handle(&mut p, card("rare", true), 2).is_empty());
    assert_eq!(ids(&e.handle(&mut p, card("mythic", true), 3)), ["shiny"]);
    // No fields at all: the filters cannot match.
    let mut q = Progress::default();
    assert!(e.handle(&mut q, Event::new("CardAcquired"), 1).is_empty());
}

#[test]
fn a_streak_counts_consecutive_events_and_resets() {
    let e = engine();
    let mut p = Progress::default();
    e.handle(&mut p, opened("A", 0, 0), 1);
    e.handle(&mut p, opened("A", 0, 0), 2);
    assert_eq!(p.metrics["no_mythic"], 2);
    e.handle(&mut p, opened("A", 0, 1), 3); // a mythic breaks the streak
    assert_eq!(p.metrics["no_mythic"], 0);
    e.handle(&mut p, opened("A", 0, 0), 4);
    e.handle(&mut p, opened("A", 0, 0), 5);
    assert!(!p.unlocked.contains_key("drought"));
    let unlocks = e.handle(&mut p, opened("A", 0, 0), 6);
    assert!(ids(&unlocks).contains(&"drought"));
}

#[test]
fn distinct_counts_each_value_once() {
    let e = engine();
    let mut p = Progress::default();
    e.handle(&mut p, opened("A", 0, 1), 1);
    e.handle(&mut p, opened("A", 0, 1), 2);
    assert_eq!(p.metrics["sets"], 1);
    let unlocks = e.handle(&mut p, opened("B", 0, 1), 3);
    assert!(ids(&unlocks).contains(&"two-sets"));
    assert_eq!(p.metrics["sets"], 2);
}

#[test]
fn a_filtered_metric_only_counts_matching_events() {
    let e = engine();
    let mut p = Progress::default();
    let card = |rarity: &str| Event::new("CardAcquired").with("rarity", rarity).with("foil", false).with("priceCents", 1);
    e.handle(&mut p, card("common"), 1);
    e.handle(&mut p, card("mythic"), 2);
    e.handle(&mut p, card("mythic"), 3);
    assert_eq!(p.metrics["mythics_pulled"], 2);
}

#[test]
fn unlocking_feeds_meta_achievements_and_allothers_comes_last() {
    let e = engine();
    let mut p = Progress::default();
    // Three boosters worth $6 each unlock "open-3" and "value".
    for i in 0..3 {
        e.handle(&mut p, opened("A", 600, 0), i);
    }
    assert!(p.unlocked.contains_key("open-3") && p.unlocked.contains_key("value"));
    // Two unlocks happened: the meta achievement fired in the same cascade.
    assert!(p.unlocked.contains_key("two-unlocks"), "{:?}", p.unlocked);
    assert!(!p.unlocked.contains_key("all"));

    // Unlock everything else, then the ultimate achievement follows by itself.
    let card = |price: i64, rarity: &str, foil: bool| {
        Event::new("CardAcquired").with("priceCents", price).with("rarity", rarity).with("foil", foil)
    };
    e.handle(&mut p, card(9_000, "mythic", true), 10);
    e.handle(&mut p, opened("B", 0, 1), 11);
    assert!(p.unlocked.contains_key("all"), "{:?}", p.unlocked);
}

#[test]
fn progress_is_reported_for_measurable_goals_only() {
    let e = engine();
    let mut p = Progress::default();
    e.handle(&mut p, opened("A", 0, 1), 1);
    let by_id = |id: &str| e.definitions().iter().find(|d| d.id == id).unwrap();
    assert_eq!(e.progress_of(by_id("open-3"), &p), Some((1, 3)));
    assert_eq!(e.progress_of(by_id("chase"), &p), None);
    assert_eq!(e.progress_of(by_id("all"), &p), Some((0, 8)));
    for _ in 0..5 {
        e.handle(&mut p, opened("A", 0, 1), 2);
    }
    // The displayed progress never exceeds the goal.
    assert_eq!(e.progress_of(by_id("open-3"), &p), Some((3, 3)));
}

#[test]
fn invalid_configs_are_rejected() {
    let bad = |from: &str, to: &str| Engine::from_json(&CONFIG.replacen(from, to, 1)).is_err();
    assert!(bad(r#""id": "value""#, r#""id": "open-3""#), "duplicate id");
    assert!(bad(r#""metric": "opened""#, r#""metric": "nope""#), "unknown metric");
    assert!(bad(r#""emblem": "pack""#, r#""emblem": "dragon""#), "unknown emblem");
    assert!(bad(r#""palette": "ember""#, r#""palette": "neon""#), "unknown palette");
    assert!(bad(r#""atLeast": 3"#, r#""atLeast": 0"#), "non-positive threshold");
    assert!(bad(r#""op": "max", "field": "priceCents""#, r#""op": "max""#), "max without field");
}

/// The `{cents:N}` amount tokens of a text, in order.
fn amount_tokens(text: &str) -> Vec<&str> {
    text.match_indices("{cents:").map(|(i, _)| &text[i..i + text[i..].find('}').map_or(0, |end| end + 1)]).collect()
}

#[test]
fn the_shipped_definitions_write_amounts_as_tokens_not_dollars() {
    let engine = Engine::from_json(SHIPPED).unwrap();
    for def in engine.definitions() {
        assert!(!def.name.contains('$') && !def.description.contains('$'), "{}: names a real currency", def.id);
    }
    let chase = engine.definitions().iter().find(|d| d.id == "chase-card").unwrap();
    assert_eq!(amount_tokens(&chase.description), ["{cents:5000}"]);
}

#[test]
fn every_shipped_translation_covers_exactly_the_shipped_achievements() {
    let translations = [
        ("fr", include_str!("../../config/achievements.fr.json")),
        ("de", include_str!("../../config/achievements.de.json")),
        ("es", include_str!("../../config/achievements.es.json")),
        ("it", include_str!("../../config/achievements.it.json")),
    ];
    let engine = Engine::from_json(SHIPPED).unwrap();
    let ids: Vec<&str> = engine.definitions().iter().map(|d| d.id.as_str()).collect();
    for (lang, json) in translations {
        let texts: serde_json::Map<String, serde_json::Value> = serde_json::from_str(json).expect(lang);
        let translated: Vec<&str> = texts.keys().map(String::as_str).collect();
        let mut expected = ids.clone();
        expected.sort_unstable();
        let mut got = translated.clone();
        got.sort_unstable();
        assert_eq!(got, expected, "{lang}: translated ids differ from the definitions");
        for (id, text) in &texts {
            for field in ["name", "description"] {
                let value = text[field].as_str().unwrap_or_default();
                assert!(!value.trim().is_empty(), "{lang}/{id}: empty {field}");
                assert!(!value.contains('$'), "{lang}/{id}: {field} names a real currency");
            }
            // Amounts are `{cents:N}` tokens, shown in Seals: the same ones as in English.
            let english = &engine.definitions().iter().find(|d| d.id == *id).unwrap().description;
            assert_eq!(
                amount_tokens(text["description"].as_str().unwrap_or_default()),
                amount_tokens(english),
                "{lang}/{id}: amounts differ from English"
            );
        }
    }
}

#[test]
fn the_shipped_config_is_valid_and_well_balanced() {
    let e = Engine::from_json(SHIPPED).expect("shipped achievements config is valid");
    let defs = e.definitions();
    assert!(defs.len() >= 40, "{} achievements", defs.len());

    let secret = defs.iter().filter(|d| d.secret).count();
    assert!((5..=defs.len() / 3).contains(&secret), "{secret} secret achievements");

    // Every rarity is represented, and the rarest ones are the minority.
    let count = |r: Rarity| defs.iter().filter(|d| d.rarity == r).count();
    assert!(count(Rarity::Common) > 0 && count(Rarity::Uncommon) > 0 && count(Rarity::Rare) > 0 && count(Rarity::Mythic) > 0);
    assert!(count(Rarity::Mythic) < count(Rarity::Common));

    // Exactly one ultimate achievement.
    let ultimate = defs.iter().filter(|d| matches!(d.condition, unsealed_core::achievements::Condition::AllOthers)).count();
    assert_eq!(ultimate, 1);
}
