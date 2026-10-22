use std::collections::BTreeSet;
use unsealed_core::cycles::{Cycles, WikiCycles};

const WIKI: &str = include_str!("../../config/cycles.json");
const CUSTOM: &str = include_str!("../../config/cycles.custom.json");
const TRANSLATIONS: [(&str, &str); 4] = [
    ("fr", include_str!("../../config/cycles.fr.json")),
    ("de", include_str!("../../config/cycles.de.json")),
    ("es", include_str!("../../config/cycles.es.json")),
    ("it", include_str!("../../config/cycles.it.json")),
];

#[test]
fn the_shipped_cycles_are_valid_and_every_name_is_translated() {
    let cycles = Cycles::from_json(WIKI, CUSTOM, &TRANSLATIONS).unwrap();
    assert!(cycles.set_codes().count() > 100);
    let wiki: WikiCycles = serde_json::from_str(WIKI).unwrap();
    let names: BTreeSet<&str> = wiki.sets.values().flatten().map(|c| c.name.as_str()).collect();
    for (lang, text) in TRANSLATIONS {
        let texts: std::collections::BTreeMap<String, String> = serde_json::from_str(text).unwrap();
        let translated: BTreeSet<&str> = texts.keys().map(String::as_str).collect();
        let missing: Vec<&&str> = names.difference(&translated).collect();
        assert!(missing.is_empty(), "{lang} misses {missing:?}");
        assert!(texts.values().all(|t| !t.trim().is_empty()), "{lang} has an empty name");
    }
    // Ours: the Nine of the Lord of the Rings, each art with its borderless version.
    let nine = cycles.of_set("LTR").iter().find(|c| c.id == "the-nine").unwrap();
    assert!(nine.custom && nine.slots.len() == 9 && nine.slots.iter().all(|s| s.len() == 2));
    assert_eq!((nine.name("fr"), nine.name("ja")), ("Les Neuf", "The Nine"));
    // A wiki cycle, translated.
    let lands = cycles.of_set("LTR").iter().find(|c| c.id == "legendary-lands").unwrap();
    assert!(!lands.custom && lands.name("fr") != lands.name("en"));
}

#[test]
fn ours_replace_or_hide_the_wikis_and_bad_cycles_are_refused() {
    let wiki = r#"{ "sets": { "AAA": [
      { "id": "a", "name": "A", "slots": [["1"], ["2"]] },
      { "id": "b", "name": "B", "slots": [["3"], ["4"]] } ] } }"#;
    let custom = r#"{ "sets": { "AAA": [ { "id": "a", "name": { "en": "Ours" }, "slots": [["1", "9"], ["2"]] } ] },
                      "hidden": ["AAA/b"] }"#;
    let cycles = Cycles::from_json(wiki, custom, &[]).unwrap();
    let list = cycles.of_set("AAA");
    assert_eq!(list.len(), 1);
    assert_eq!((list[0].name("en"), list[0].custom, list[0].slots[0].len()), ("Ours", true, 2));
    assert!(cycles.of_set("ZZZ").is_empty());

    let one_slot = r#"{ "sets": { "AAA": [ { "id": "a", "name": { "en": "X" }, "slots": [["1"]] } ] } }"#;
    assert!(Cycles::from_json(r#"{ "sets": {} }"#, one_slot, &[]).is_err(), "a cycle of one card");
    let no_english = r#"{ "sets": { "AAA": [ { "id": "z", "name": { "fr": "X" }, "slots": [["1"], ["2"]] } ] } }"#;
    assert!(Cycles::from_json(r#"{ "sets": {} }"#, no_english, &[]).is_err(), "no English name");
}
