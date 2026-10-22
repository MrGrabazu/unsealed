use std::collections::{BTreeMap, HashSet};
use std::io::Cursor;
use unsealed_core::catalog::{AllSets, Manifest, ManifestSet, SetSelection, DATA_FORMAT_VERSION};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use unsealed_core::import::{
    add_art_cards_from, add_art_slots, add_token_slots, artist_index, add_tokens_from, merge_foil_printings, merge_token_faces, parse_cents,
    prices_by_uuid, prices_from_scryfall_jsonl, read_all_printings, set_from_mtgjson, ArtCardOdds,
};
use unsealed_core::catalog::PriceSet;
use unsealed_core::{open_booster, ColorIndex};
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

/// A set with a play and a collector booster, and two tokens: one printed in both
/// finishes, one non-foil only, and one Scryfall does not know (left out).
const SET_WITH_TOKENS: &str = r#"{
  "meta": { "version": "5.3.0+test" },
  "data": {
    "code": "TST", "name": "Test Set", "releaseDate": "2026-01-01", "type": "expansion",
    "cards": [
      { "uuid": "u1", "setCode": "TST", "name": "A", "number": "1", "rarity": "common",
        "finishes": ["nonfoil", "foil"], "layout": "normal", "identifiers": { "scryfallId": "s1" } }
    ],
    "tokens": [
      { "uuid": "t1", "setCode": "TTST", "name": "Soldier", "number": "1",
        "finishes": ["nonfoil", "foil"], "layout": "token", "identifiers": { "scryfallId": "st1" } },
      { "uuid": "t2", "setCode": "TTST", "name": "Spirit", "number": "2",
        "finishes": ["nonfoil"], "layout": "token", "identifiers": { "scryfallId": "st2" } },
      { "uuid": "t3", "setCode": "TTST", "name": "Unknown", "number": "3",
        "finishes": ["nonfoil"], "layout": "token", "identifiers": {} }
    ],
    "booster": {
      "play": {
        "boosters": [ { "contents": { "common": 1 }, "weight": 3 } ],
        "boostersTotalWeight": 3,
        "sheets": { "common": { "cards": { "u1": 1 }, "foil": false, "totalWeight": 1 } },
        "sourceSetCodes": ["TST"]
      },
      "collector": {
        "boosters": [ { "contents": { "common": 1 }, "weight": 1 } ],
        "boostersTotalWeight": 1,
        "sheets": { "common": { "cards": { "u1": 1 }, "foil": true, "totalWeight": 1 } },
        "sourceSetCodes": ["TST"]
      }
    }
  }
}"#;

#[test]
fn every_booster_gets_a_token_foil_in_collector_boosters() {
    let mut set = set_from_mtgjson(SET_WITH_TOKENS, &["play", "collector"], &[]).unwrap();
    let tokens: Vec<_> = set.cards.iter().filter(|c| c.token).collect();
    assert_eq!(tokens.iter().map(|t| t.uuid.as_str()).collect::<Vec<_>>(), ["t1", "t2"]);
    assert!(tokens.iter().all(|t| t.rarity == "token" && t.set_code == "TTST"));

    add_token_slots(&mut set, 15);
    let play = &set.boosters["play"];
    play.validate().unwrap();
    // One pack in 15 has a foil token (only t1 exists in foil), the others a plain one.
    let weights: Vec<(u64, bool)> = play
        .boosters
        .iter()
        .map(|p| (p.weight, p.contents.keys().any(|k| play.sheets[k].foil && k != "common")))
        .collect();
    assert_eq!(weights, [(42, false), (3, true)]);
    assert_eq!(play.boosters_total_weight, 45);
    let foil_sheet = play.sheets.values().find(|s| s.foil).unwrap();
    assert_eq!(foil_sheet.cards.keys().collect::<Vec<_>>(), ["t1"]);

    let collector = &set.boosters["collector"];
    collector.validate().unwrap();
    let mut rng = ChaCha8Rng::seed_from_u64(1);
    for _ in 0..200 {
        let pack = open_booster(collector, &ColorIndex::default(), &mut rng).unwrap();
        let token = pack.iter().find(|d| d.uuid.starts_with('t')).expect("a token in every pack");
        assert!(token.foil);
    }
}

/// Unhinged-like: "1★" is the foil of "1" (merged), "2★" has another artist (kept apart).
const SET_WITH_STAR_FOILS: &str = r#"{
  "meta": { "version": "5.3.0+test" },
  "data": {
    "code": "TST", "name": "Test Set", "releaseDate": "2005-01-01", "type": "funny",
    "cards": [
      { "uuid": "u1", "setCode": "TST", "name": "A", "number": "1", "rarity": "common", "artist": "Ann",
        "finishes": ["nonfoil"], "layout": "normal", "identifiers": { "scryfallId": "s1" } },
      { "uuid": "u1s", "setCode": "TST", "name": "A", "number": "1★", "rarity": "common", "artist": "Ann",
        "finishes": ["foil"], "layout": "normal", "identifiers": { "scryfallId": "s1s" } },
      { "uuid": "u2", "setCode": "TST", "name": "B", "number": "2", "rarity": "rare", "artist": "Bob",
        "finishes": ["nonfoil"], "layout": "normal", "identifiers": { "scryfallId": "s2" } },
      { "uuid": "u2s", "setCode": "TST", "name": "B", "number": "2★", "rarity": "rare", "artist": "Kaz",
        "finishes": ["foil"], "layout": "normal", "identifiers": { "scryfallId": "s2s" } }
    ],
    "booster": {
      "draft": {
        "boosters": [ { "contents": { "common": 1, "foil": 1 }, "weight": 1 } ],
        "boostersTotalWeight": 1,
        "sheets": {
          "common": { "cards": { "u1": 1, "u2": 1 }, "foil": false, "totalWeight": 2 },
          "foil": { "cards": { "u1s": 3, "u2s": 1 }, "foil": true, "totalWeight": 4 }
        },
        "sourceSetCodes": ["TST"]
      }
    }
  }
}"#;

#[test]
fn star_foil_printings_become_the_foil_copies_of_their_card() {
    let mut set = set_from_mtgjson(SET_WITH_STAR_FOILS, &["draft"], &[]).unwrap();
    merge_foil_printings(&mut set);
    let numbers: Vec<&str> = set.cards.iter().map(|c| c.number.as_str()).collect();
    assert_eq!(numbers, ["1", "2", "2★"]);
    let a = &set.cards[0];
    assert_eq!(a.finishes, ["nonfoil", "foil"]);
    assert_eq!(a.foil_scryfall_id.as_deref(), Some("s1s"));
    let foil = &set.boosters["draft"].sheets["foil"];
    assert_eq!(foil.cards, BTreeMap::from([("u1".to_string(), 3), ("u2s".to_string(), 1)]));
    set.boosters["draft"].validate().unwrap();
    assert_eq!(set.foil_aliases, BTreeMap::from([("u1s".to_string(), "u1".to_string())]));

    // The card's foil prices come from the merged printing.
    let by_id = BTreeMap::from([
        ("s1".to_string(), PriceSet { usd: Some(33), ..Default::default() }),
        ("s1s".to_string(), PriceSet { usd_foil: Some(880), ..Default::default() }),
    ]);
    let prices = prices_by_uuid(&by_id, &set.cards);
    assert_eq!((prices["u1"].usd, prices["u1"].usd_foil), (Some(33), Some(880)));
}

#[test]
fn tokens_kept_in_the_token_sub_set_join_the_set() {
    // TTST holds the set's tokens: one also in the parent's list (t1), one only there (t9).
    let all = read_all_printings(
        r#"{ "meta": { "version": "v" }, "data": { "TTST": {
          "code": "TTST", "name": "Test Set Tokens", "releaseDate": "2026-01-01", "type": "token",
          "cards": [],
          "tokens": [
            { "uuid": "t1", "setCode": "TTST", "name": "Soldier", "number": "1",
              "finishes": ["nonfoil", "foil"], "layout": "token", "identifiers": { "scryfallId": "st1" } },
            { "uuid": "t9", "setCode": "TTST", "name": "Emblem", "number": "9",
              "finishes": ["nonfoil"], "layout": "emblem", "identifiers": { "scryfallId": "st9" } } ] } } }"#
            .as_bytes(),
    )
    .unwrap();
    let mut set = set_from_mtgjson(SET_WITH_TOKENS, &["play"], &[]).unwrap();
    add_tokens_from(&mut set, &all.data["TTST"]);
    let tokens: Vec<&str> = set.cards.iter().filter(|c| c.token).map(|c| c.uuid.as_str()).collect();
    assert_eq!(tokens, ["t1", "t2", "t9"]);
}

#[test]
fn a_token_on_two_faces_is_one_card_its_back_an_alias() {
    // Amonkhet's Punchcard: one token, listed once per face under the same number.
    let all = read_all_printings(
        r#"{ "meta": { "version": "v" }, "data": { "TTST": {
          "code": "TTST", "name": "Test Set Tokens", "releaseDate": "2026-01-01", "type": "token",
          "cards": [],
          "tokens": [
            { "uuid": "p1", "setCode": "TTST", "name": "Punchcard // Punchcard", "number": "26", "side": "a",
              "finishes": ["nonfoil"], "layout": "double_faced_token", "identifiers": { "scryfallId": "sp1" } },
            { "uuid": "p1b", "setCode": "TTST", "name": "Punchcard // Punchcard", "number": "26", "side": "b",
              "finishes": ["nonfoil"], "layout": "double_faced_token", "identifiers": { "scryfallId": "sp1" } } ] } } }"#
            .as_bytes(),
    )
    .unwrap();
    let mut set = set_from_mtgjson(SET_WITH_TOKENS, &["play"], &[]).unwrap();
    add_tokens_from(&mut set, &all.data["TTST"]);
    merge_token_faces(&mut set);
    let punchcards: Vec<&str> = set.cards.iter().filter(|c| c.name.starts_with("Punchcard")).map(|c| c.uuid.as_str()).collect();
    assert_eq!(punchcards, ["p1"]);
    assert_eq!(set.foil_aliases.get("p1b").map(String::as_str), Some("p1"));
}

#[test]
fn art_cards_join_the_set_and_the_boosters_of_their_time() {
    // The art series lists each card's two faces; the first comes with its signed version.
    let all = read_all_printings(
        r#"{ "meta": { "version": "v" }, "data": { "ATST": {
          "code": "ATST", "name": "Test Set Art Series", "releaseDate": "2026-01-01", "type": "memorabilia",
          "cards": [],
          "tokens": [
            { "uuid": "a1", "setCode": "ATST", "name": "Dragon // Dragon", "number": "1", "side": "a",
              "finishes": ["nonfoil", "signed"], "layout": "art_series", "identifiers": { "scryfallId": "sa1" } },
            { "uuid": "a1b", "setCode": "ATST", "name": "Dragon // Dragon", "number": "1", "side": "b",
              "finishes": ["nonfoil", "signed"], "layout": "art_series", "identifiers": { "scryfallId": "sa1" } },
            { "uuid": "a2", "setCode": "ATST", "name": "Forest // Forest", "number": "2", "side": "a",
              "finishes": ["nonfoil"], "layout": "art_series", "identifiers": { "scryfallId": "sa2" } } ] } } }"#
            .as_bytes(),
    )
    .unwrap();
    let mut set = set_from_mtgjson(SET_WITH_TOKENS, &["play", "collector"], &[]).unwrap();
    add_art_cards_from(&mut set, &all.data["ATST"]);
    let arts: Vec<(&str, &str, &Vec<String>)> =
        set.cards.iter().filter(|c| c.art).map(|c| (c.uuid.as_str(), c.name.as_str(), &c.finishes)).collect();
    assert_eq!(arts.len(), 2);
    assert_eq!((arts[0].0, arts[0].1), ("a1", "Dragon"));
    assert_eq!(arts[0].2, &vec!["nonfoil".to_string(), "foil".to_string()]);
    assert!(set.cards.iter().filter(|c| c.art).all(|c| c.rarity == "art"));

    // Released in 2026: art cards in collector boosters (45% + 5% signed), not in play ones.
    let play_before = set.boosters["play"].boosters.clone();
    add_art_slots(&mut set, &ArtCardOdds::default());
    assert_eq!(set.boosters["play"].boosters.len(), play_before.len());
    let collector = &set.boosters["collector"];
    collector.validate().unwrap();
    let share = |sheet: &str| -> u64 {
        collector.boosters.iter().filter(|p| p.contents.contains_key(sheet)).map(|p| p.weight).sum::<u64>() * 100
            / collector.boosters_total_weight
    };
    assert_eq!((share("unsealedArt"), share("unsealedArtSigned")), (45, 5));
    assert_eq!(collector.sheets["unsealedArtSigned"].cards.keys().collect::<Vec<_>>(), ["a1"]);
}

#[test]
fn artists_list_the_cards_boosters_give_under_each_of_their_artists() {
    let raw = r#"{ "meta": { "version": "v" }, "data": {
      "code": "TST", "name": "Test Set", "releaseDate": "2026-01-01", "type": "expansion",
      "cards": [
        { "uuid": "u1", "setCode": "TST", "name": "A", "number": "1", "rarity": "common", "artist": "Alice",
          "finishes": ["nonfoil"], "layout": "normal", "identifiers": { "scryfallId": "s1" } },
        { "uuid": "u2", "setCode": "TST", "name": "B", "number": "2", "rarity": "common", "artist": "Bob & Alice",
          "finishes": ["nonfoil"], "layout": "normal", "identifiers": { "scryfallId": "s2" } },
        { "uuid": "u3", "setCode": "TST", "name": "C", "number": "3", "rarity": "rare", "artist": "Carol",
          "finishes": ["nonfoil"], "layout": "normal", "identifiers": { "scryfallId": "s3" } } ],
      "booster": { "play": {
        "boosters": [ { "contents": { "common": 1 }, "weight": 1 } ], "boostersTotalWeight": 1,
        "sheets": { "common": { "cards": { "u1": 1, "u2": 1 }, "foil": false, "totalWeight": 2 } },
        "sourceSetCodes": ["TST"] } } } }"#;
    let set = set_from_mtgjson(raw, &["play"], &[]).unwrap();
    // The same set twice: each card is listed once.
    let index = artist_index(&[set.clone(), set]);
    let names: Vec<&str> = index.artists.keys().map(String::as_str).collect();
    assert_eq!(names, ["Alice", "Bob"]); // Carol's card is in no booster
    let alice: Vec<&str> = index.artists["Alice"].iter().map(|c| c.uuid.as_str()).collect();
    assert_eq!(alice, ["u1", "u2"]);
    assert_eq!(index.artists["Bob"][0].set, "TST");
}

/// A compact set file whose boosters give `own` cards of its own and `guests` of other sets
/// (as "uuid:SET").
fn set_with_guests(code: &str, own: &[&str], guests: &[&str]) -> unsealed_core::catalog::SetFile {
    let card = |uuid: &str, set: &str| {
        format!(
            r#"{{ "uuid": "{uuid}", "setCode": "{set}", "scryfallId": "s-{uuid}", "name": "{uuid}", "number": "1",
              "rarity": "common", "finishes": ["nonfoil"], "layout": "normal" }}"#
        )
    };
    let all: Vec<(&str, &str)> =
        own.iter().map(|u| (*u, code)).chain(guests.iter().map(|g| g.split_once(':').unwrap())).collect();
    let cards: Vec<String> = all.iter().map(|(u, s)| card(u, s)).collect();
    let sheet: Vec<String> = all.iter().map(|(u, _)| format!(r#""{u}": 1"#)).collect();
    serde_json::from_str(&format!(
        r#"{{ "code": "{code}", "name": "{code}", "releaseDate": "2026-01-01", "setType": "expansion", "sourceVersion": "v",
          "cards": [{}],
          "boosters": {{ "play": {{ "boosters": [ {{ "contents": {{ "common": 1 }}, "weight": 1 }} ], "boostersTotalWeight": 1,
            "sheets": {{ "common": {{ "cards": {{ {} }}, "foil": false, "totalWeight": {} }} }} }} }} }}"#,
        cards.join(","),
        sheet.join(","),
        all.len()
    ))
    .unwrap()
}

#[test]
fn guests_are_indexed_by_their_series_with_every_set_that_gives_them() {
    use unsealed_core::import::guest_index;
    let a = set_with_guests("AAA", &["a1", "a2"], &["g1:SPG"]);
    let b = set_with_guests("BBB", &["b1", "b2"], &["g1:SPG", "g2:SPG", "l1:PLST"]);
    // Nearly all The List, as a Mystery Booster: it keeps its guests.
    let mystery = set_with_guests("MYS", &["m1"], &["l1:PLST", "l2:PLST", "l3:PLST"]);
    assert!(!a.keeps_its_guests() && !b.keeps_its_guests() && mystery.keeps_its_guests());

    let index = guest_index(&[a, b, mystery]);
    assert_eq!(index.series.keys().collect::<Vec<_>>(), ["PLST", "SPG"]);
    let spg: Vec<(&str, &Vec<String>)> = index.series["SPG"].cards.iter().map(|c| (c.uuid.as_str(), &c.boosters)).collect();
    assert_eq!(spg, [("g1", &vec!["AAA".to_string(), "BBB".to_string()]), ("g2", &vec!["BBB".to_string()])]);
    // The List's cards of the Mystery Booster stay in its binder.
    assert_eq!(index.series["PLST"].cards.iter().map(|c| c.uuid.as_str()).collect::<Vec<_>>(), ["l1"]);
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

    // A card the set reprints from another set only stands in when the set has none of its own
    // (the Mystery Boosters hold nothing but reprints).
    set.cards[1].set_code = "PLST".into();
    let prices = [("u2".to_string(), PriceSet { usd: Some(500), ..Default::default() })].into();
    assert_eq!(featured_card(&set, &prices).unwrap().name, "A");
    set.cards[0].set_code = "PLST".into();
    assert_eq!(featured_card(&set, &prices).unwrap().name, "B");
}

#[test]
fn keeps_translated_names_and_printings_of_the_wanted_languages() {
    use unsealed_core::catalog::ForeignCard;
    let json = raw_set("u1", r#""scryfallId": "s1""#)
        .replace(
            r#""layout": "normal","#,
            r#""layout": "normal", "foreignData": [
              { "language": "French", "name": "A fr", "identifiers": { "scryfallId": "s1-fr" } },
              { "language": "Russian", "name": "A ru" },
              { "language": "German", "name": "A de" } ],"#,
        )
        .replace(
            r#""type": "expansion","#,
            r#""type": "expansion", "translations": { "French": "Ensemble", "German": null, "Russian": "Nabor" },"#,
        );
    let set = set_from_mtgjson(&json, &["play"], &[]).unwrap();
    let card = &set.cards[0];
    // Russian is not kept; German has no translated printing on Scryfall.
    assert_eq!(card.foreign.keys().collect::<Vec<_>>(), ["de", "fr"]);
    assert_eq!(card.foreign["fr"], ForeignCard { name: "A fr".into(), scryfall_id: Some("s1-fr".into()) });
    assert_eq!(card.foreign["de"].scryfall_id, None);
    assert_eq!(set.names, BTreeMap::from([("fr".to_string(), "Ensemble".to_string())]));
}

#[test]
fn data_built_in_an_older_format_is_rebuilt() {
    let mut local = manifest("1000", &["BLB", "DSK"]);
    local.format_version = DATA_FORMAT_VERSION - 1;
    assert_eq!(refresh_reason(Some(&local), &data_config(), 1_100), Some(RefreshReason::FormatChanged));
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

#[test]
fn a_printing_without_price_is_estimated_from_the_other_printings() {
    let jsonl = concat!(
        // The serialized printing: no price at all.
        r#"{"id":"ring0","oracle_id":"ring","prices":{"usd":null,"usd_foil":null}}"#, "\n",
        r#"{"id":"ring1","oracle_id":"ring","prices":{"usd":"40.00","usd_foil":"55.00","eur":"35.00"}}"#, "\n",
        r#"{"id":"ring2","oracle_id":"ring","prices":{"usd":"60.00","eur":null,"eur_foil":"50.00"}}"#, "\n",
        // Another serialized printing: its scarcity price is not a base.
        r#"{"id":"ring3","oracle_id":"ring","promo_types":["serialized"],"prices":{"usd":"9000.00"}}"#, "\n",
        // Reversible cards carry the oracle id on their faces.
        r#"{"id":"rev","card_faces":[{"oracle_id":"rv"},{"oracle_id":"rv"}],"prices":{}}"#, "\n",
        r#"{"id":"rev2","oracle_id":"rv","prices":{"usd":"1.00"}}"#, "\n",
        r#"{"id":"alone","oracle_id":"x","prices":{}}"#, "\n",
    );
    let wanted: HashSet<String> = ["ring0", "ring1", "rev", "alone"].map(String::from).into();
    let prices = prices_from_scryfall_jsonl(Cursor::new(jsonl), &wanted).unwrap();

    // Best of each price over the other printings; a missing finish takes the other one.
    let ring = prices["ring0"];
    assert!(ring.estimated);
    assert_eq!((ring.usd, ring.usd_foil, ring.eur, ring.eur_foil), (Some(6000), Some(5500), Some(3500), Some(5000)));
    // A printing with a price of its own keeps it.
    assert!(!prices["ring1"].estimated);
    assert_eq!(prices["ring1"].usd, Some(4000));
    assert_eq!((prices["rev"].usd, prices["rev"].estimated), (Some(100), true));
    // Nothing to estimate from: no price.
    assert!(!prices.contains_key("alone"));
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
                        names: BTreeMap::new(),
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
        format_version: DATA_FORMAT_VERSION,
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
