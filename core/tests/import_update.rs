use std::collections::{BTreeMap, HashSet};
use std::io::Cursor;
use unsealed_core::catalog::{Manifest, ManifestSet};
use unsealed_core::import::{parse_cents, prices_from_scryfall_jsonl, set_from_mtgjson};
use unsealed_core::update::{
    plan_update, prices_are_stale, should_check, LocalState, UpdateConfig, UpdatePlan,
};

fn raw_set(sheet_card: &str, scryfall_id: &str) -> String {
    format!(
        r#"{{
  "meta": {{ "version": "5.3.0+test" }},
  "data": {{
    "code": "TST", "name": "Test Set", "releaseDate": "2026-01-01", "type": "expansion",
    "cards": [
      {{ "uuid": "u1", "setCode": "TST", "name": "A", "number": "1", "rarity": "common", "colors": ["W"],
         "finishes": ["nonfoil", "foil"], "layout": "normal",
         "identifiers": {{ {scryfall_id} }} }}
    ],
    "booster": {{
      "play": {{
        "boosters": [ {{ "contents": {{ "common": 1 }}, "weight": 1 }} ],
        "boostersTotalWeight": 1,
        "sheets": {{ "common": {{ "cards": {{ "{sheet_card}": 1 }}, "foil": false, "totalWeight": 1 }} }},
        "sourceSetCodes": ["TST"]
      }}
    }}
  }}
}}"#
    )
}

#[test]
fn converts_a_valid_set() {
    let set = set_from_mtgjson(&raw_set("u1", r#""scryfallId": "s1""#), &["play"], &[]).unwrap();
    assert_eq!(set.code, "TST");
    assert_eq!(set.source_version, "5.3.0+test");
    assert_eq!(set.cards[0].scryfall_id, "s1");
    assert_eq!(set.cards[0].set_code, "TST");
    assert!(set.boosters.contains_key("play"));
}

#[test]
fn rejects_sheet_referencing_unknown_card() {
    let err =
        set_from_mtgjson(&raw_set("ghost", r#""scryfallId": "s1""#), &["play"], &[]).unwrap_err();
    assert!(err.to_string().contains("unknown card"), "{err}");
}

#[test]
fn embeds_only_referenced_cards_from_other_sets() {
    let extra = r#"{
      "meta": { "version": "x" },
      "data": { "code": "SPG", "name": "Special Guests", "releaseDate": "2026-01-01", "type": "memorabilia",
        "cards": [
          { "uuid": "g1", "setCode": "SPG", "name": "G1", "number": "1", "rarity": "mythic",
            "finishes": ["foil"], "layout": "normal", "identifiers": { "scryfallId": "sg1" } },
          { "uuid": "g2", "setCode": "SPG", "name": "G2", "number": "2", "rarity": "mythic",
            "finishes": ["foil"], "layout": "normal", "identifiers": { "scryfallId": "sg2" } }
        ] } }"#;
    let set = set_from_mtgjson(&raw_set("g1", r#""scryfallId": "s1""#), &["play"], &[extra]).unwrap();
    let uuids: Vec<&str> = set.cards.iter().map(|c| c.uuid.as_str()).collect();
    assert_eq!(uuids, ["u1", "g1"]);
    assert_eq!(set.cards[1].set_code, "SPG");
}

#[test]
fn lists_source_sets_other_than_itself() {
    use unsealed_core::import::source_set_codes;
    let json = raw_set("u1", r#""scryfallId": "s1""#).replace(r#"["TST"]"#, r#"["TST", "SPG"]"#);
    assert_eq!(source_set_codes(&json, &["play"]).unwrap(), ["SPG"]);
}

#[test]
fn rejects_card_without_scryfall_id() {
    assert!(set_from_mtgjson(&raw_set("u1", r#""mtgoId": "1""#), &["play"], &[]).is_err());
}

#[test]
fn rejects_missing_booster_kind() {
    assert!(set_from_mtgjson(&raw_set("u1", r#""scryfallId": "s1""#), &["collector"], &[]).is_err());
}

#[test]
fn parses_cents_without_floats() {
    assert_eq!(parse_cents("0.05"), Some(5));
    assert_eq!(parse_cents("12"), Some(1200));
    assert_eq!(parse_cents("1.5"), Some(150));
    assert_eq!(parse_cents("1.234"), None);
    assert_eq!(parse_cents("abc"), None);
}

#[test]
fn extracts_only_wanted_prices() {
    let jsonl = concat!(
        r#"{"id":"s1","name":"A","prices":{"usd":"0.25","usd_foil":null,"eur":"0.20","eur_foil":"1.10"}}"#, "\n",
        "\n",
        r#"{"id":"other","prices":{"usd":"9.99"}}"#, "\n",
    );
    let wanted: HashSet<String> = ["s1".to_string()].into();
    let prices = prices_from_scryfall_jsonl(Cursor::new(jsonl), &wanted).unwrap();
    assert_eq!(prices.len(), 1);
    let p = prices["s1"];
    assert_eq!((p.usd, p.usd_foil, p.eur, p.eur_foil), (Some(25), None, Some(20), Some(110)));
}

fn manifest(version: &str, prices: &str) -> Manifest {
    Manifest {
        generated_at: "0".into(),
        prices_updated_at: prices.into(),
        sets: BTreeMap::from([(
            "BLB".to_string(),
            ManifestSet {
                name: "Bloomburrow".into(),
                release_date: "2024-08-02".into(),
                source_version: version.into(),
                boosters: vec!["play".into()],
            },
        )]),
    }
}

#[test]
fn plan_refreshes_only_what_changed() {
    let local = LocalState {
        sets: BTreeMap::from([("BLB".to_string(), "v1".to_string())]),
        prices_updated_at: Some("day1".into()),
        ..Default::default()
    };
    assert_eq!(plan_update(&local, &manifest("v1", "day1")), UpdatePlan::default());
    assert_eq!(
        plan_update(&local, &manifest("v2", "day2")),
        UpdatePlan { sets_to_fetch: vec!["BLB".into()], fetch_prices: true }
    );
}

#[test]
fn plan_ignores_sets_that_are_not_installed() {
    let local = LocalState { prices_updated_at: Some("day1".into()), ..Default::default() };
    assert!(plan_update(&local, &manifest("v9", "day1")).sets_to_fetch.is_empty());
}

#[test]
fn check_interval_and_stale_prices() {
    let cfg = UpdateConfig::default();
    let mut local = LocalState::default();
    assert!(should_check(&local, 1_000, &cfg));
    assert!(prices_are_stale(&local, 1_000, &cfg));

    local.last_check_at = Some(1_000);
    local.prices_fetched_at = Some(1_000);
    assert!(!should_check(&local, 1_000 + 3600, &cfg));
    assert!(should_check(&local, 1_000 + cfg.check_interval_secs, &cfg));
    assert!(!prices_are_stale(&local, 1_000 + 3600, &cfg));
    assert!(prices_are_stale(&local, 1_000 + cfg.stale_prices_secs + 1, &cfg));
}
