use std::collections::{BTreeMap, HashSet};
use std::io::Cursor;
use unsealed_core::catalog::{AllSets, Manifest, ManifestSet, SetSelection};
use unsealed_core::import::{parse_cents, prices_from_scryfall_jsonl, set_from_mtgjson};
use unsealed_core::update::{refresh_reason, DataConfig, RefreshReason};

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
fn keeps_the_wanted_kinds_the_set_has() {
    use unsealed_core::import::source_set_codes;
    let json = raw_set("u1", r#""scryfallId": "s1""#);
    let set = set_from_mtgjson(&json, &["collector", "play"], &[]).unwrap();
    assert_eq!(set.boosters.keys().collect::<Vec<_>>(), ["play"]);
    assert!(source_set_codes(&json, &["collector", "play"]).unwrap().is_empty());
}

#[test]
fn the_featured_card_is_the_most_valuable_credited_card_of_the_main_boosters() {
    use unsealed_core::catalog::PriceSet;
    use unsealed_core::import::featured_card;
    let mut set = set_from_mtgjson(&raw_set("u1", r#""scryfallId": "s1""#), &["play"], &[]).unwrap();
    // Without an artist, a card cannot be credited, so it cannot illustrate the set.
    assert_eq!(featured_card(&set, &BTreeMap::new()), None);

    set.cards[0].artist = Some("Jane Doe".into());
    let mut second = set.cards[0].clone();
    (second.uuid, second.scryfall_id, second.name, second.number) = ("u2".into(), "s2".into(), "B".into(), "2".into());
    set.cards.push(second);
    set.boosters.get_mut("play").unwrap().sheets.get_mut("common").unwrap().cards.insert("u2".into(), 1);

    // No price: the lowest collector number. Otherwise: the most valuable card.
    assert_eq!(featured_card(&set, &BTreeMap::new()).unwrap().name, "A");
    let prices = [("u2".to_string(), PriceSet { usd: Some(500), ..Default::default() })].into();
    let featured = featured_card(&set, &prices).unwrap();
    assert_eq!((featured.name.as_str(), featured.scryfall_id.as_str(), featured.artist.as_str()), ("B", "s2", "Jane Doe"));
}

#[test]
fn rejects_a_set_without_any_wanted_kind() {
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

fn manifest(generated_at: &str, sets: &[&str]) -> Manifest {
    Manifest {
        generated_at: generated_at.into(),
        prices_updated_at: "day1".into(),
        sets: sets
            .iter()
            .map(|code| {
                (
                    code.to_string(),
                    ManifestSet {
                        name: "Set".into(),
                        release_date: "2024-08-02".into(),
                        source_version: "v1".into(),
                        boosters: vec!["play".into()],
                        featured: None,
                        icon_svg_uri: None,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>(),
        booster_kinds: ["play", "draft", "set", "default", "collector"].map(String::from).to_vec(),
        // What `data_config` asks for.
        requested_sets: Some(SetSelection::List(vec!["BLB".into(), "DSK".into()])),
        set_types: Vec::new(),
        skipped: BTreeMap::new(),
    }
}

fn data_config() -> DataConfig {
    DataConfig::from_json(r#"{"sets":["BLB","DSK"]}"#).unwrap()
}

#[test]
fn data_config_applies_defaults() {
    let cfg = data_config();
    assert_eq!(cfg.boosters, ["play", "draft", "set", "default", "collector"]);
    assert_eq!((cfg.max_age_hours, cfg.sim_boosters), (24, 20_000));
}

#[test]
fn refresh_when_nothing_is_installed_or_a_set_is_missing() {
    let cfg = data_config();
    assert_eq!(refresh_reason(None, &cfg, 1_000), Some(RefreshReason::NoData));
    assert_eq!(
        refresh_reason(Some(&manifest("1000", &["BLB"])), &cfg, 1_100),
        Some(RefreshReason::MissingSet("DSK".into()))
    );
}

#[test]
fn refresh_only_when_data_is_older_than_the_limit() {
    let cfg = data_config();
    let local = manifest("1000", &["BLB", "DSK"]);
    assert_eq!(refresh_reason(Some(&local), &cfg, 1_000 + 23 * 3600), None);
    assert_eq!(refresh_reason(Some(&local), &cfg, 1_000 + 24 * 3600), Some(RefreshReason::Stale));
    // Clock moved back, or garbage timestamp: rebuild.
    assert_eq!(refresh_reason(Some(&local), &cfg, 500), Some(RefreshReason::Stale));
    assert_eq!(refresh_reason(Some(&manifest("x", &["BLB", "DSK"])), &cfg, 1_000), Some(RefreshReason::Stale));
}

#[test]
fn refresh_when_the_wanted_booster_kinds_or_sets_change() {
    let cfg = data_config();
    let mut local = manifest("1000", &["BLB", "DSK"]);
    local.booster_kinds = vec!["play".into()];
    assert_eq!(refresh_reason(Some(&local), &cfg, 1_100), Some(RefreshReason::ConfigChanged));

    let all = DataConfig::from_json(r#"{"sets":"all"}"#).unwrap();
    assert_eq!(all.sets, SetSelection::All(AllSets::All));
    assert!(all.set_types.contains(&"expansion".to_string()));
    let local = manifest("1000", &["BLB", "DSK"]);
    assert_eq!(refresh_reason(Some(&local), &all, 1_100), Some(RefreshReason::ConfigChanged));
    let mut local = manifest("1000", &["BLB"]);
    (local.requested_sets, local.set_types) = (Some(all.sets.clone()), all.set_types.clone());
    assert_eq!(refresh_reason(Some(&local), &all, 1_100), None);
}

#[test]
fn a_requested_set_left_out_does_not_trigger_a_rebuild_loop() {
    let cfg = data_config();
    let mut local = manifest("1000", &["BLB"]);
    local.skipped.insert("DSK".into(), "failed the simulation".into());
    assert_eq!(refresh_reason(Some(&local), &cfg, 1_100), None);
}
