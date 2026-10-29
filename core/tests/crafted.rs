use std::collections::BTreeMap;
use unsealed_core::catalog::{PriceSet, SetFile};
use unsealed_core::crafted::{craft_booster, CraftedBooster};
use unsealed_core::cycles::{drop_groups, GroupKind};
use unsealed_core::update::DataConfig;

const DATA_CONFIG: &str = include_str!("../../config/data.json");

/// A Secret Lair-like set: cards priced from 1 to 100 dollars, in both finishes but one foil
/// only, two drops, a foil "★" twin and a card of no drop.
fn set() -> SetFile {
    let card = |uuid: &str, number: &str, finishes: &str, drop: Option<&str>| {
        let drop = drop.map_or(String::new(), |d| format!(r#", "drop": "{d}""#));
        format!(
            r#"{{ "uuid": "{uuid}", "setCode": "SLD", "scryfallId": "s-{uuid}", "name": "Card {uuid}", "number": "{number}",
               "rarity": "rare", "finishes": [{finishes}], "layout": "normal"{drop} }}"#
        )
    };
    let both = r#""nonfoil", "foil""#;
    let cards = [
        card("a", "1", both, Some("Cats")),
        card("b", "2", both, Some("Cats")),
        card("c", "3", r#""foil""#, Some("Cats")),
        card("d", "10", both, Some("Dogs")),
        card("e", "10★", r#""foil""#, Some("Dogs")),
        card("f", "11", both, Some("Dogs")),
        card("g", "20", both, None),
    ];
    serde_json::from_str(&format!(
        r#"{{ "code": "SLD", "name": "Secret Lair Drop", "releaseDate": "2019-12-02", "setType": "box", "sourceVersion": "v1",
             "cards": [{}], "boosters": {{}} }}"#,
        cards.join(",")
    ))
    .unwrap()
}

fn prices() -> BTreeMap<String, PriceSet> {
    [("a", 100), ("b", 300), ("c", 900), ("d", 2_500), ("e", 9_000), ("f", 10_000), ("g", 500)]
        .into_iter()
        .map(|(uuid, cents)| (uuid.to_string(), PriceSet { usd: Some(cents), usd_foil: Some(cents * 2), ..Default::default() }))
        .collect()
}

fn recipe() -> CraftedBooster {
    serde_json::from_str(
        r#"{ "kind": "secret-lair", "tiers": { "uncommon": 800, "rare": 2000, "mythic": 5000 },
             "slots": [ { "count": 2, "rarities": { "common": 1 } },
                        { "count": 1, "foil": true, "rarities": { "common": 3, "rare": 1 } },
                        { "count": 1, "rarities": { "rare": 9, "mythic": 1 } } ] }"#,
    )
    .unwrap()
}

#[test]
fn each_card_takes_the_rarity_of_its_price_tier() {
    let mut set = set();
    craft_booster(&mut set, &recipe(), &prices()).unwrap();
    let rarity = |uuid: &str| set.cards.iter().find(|c| c.uuid == uuid).unwrap().rarity.clone();
    // Its cheapest finish: c is 9 dollars (uncommon), e (foil only) 90 dollars in foil (mythic).
    assert_eq!(
        ["a", "b", "c", "d", "e", "f", "g"].map(rarity),
        ["common", "common", "uncommon", "rare", "mythic", "mythic", "common"]
    );
}

#[test]
fn the_crafted_booster_draws_each_slot_from_its_tiers() {
    let mut set = set();
    craft_booster(&mut set, &recipe(), &prices()).unwrap();
    let cfg = &set.boosters["secret-lair"];
    cfg.validate().unwrap();
    // A foil-only card is only in the foil sheets, a card in both finishes in both; the sheets
    // no slot draws from (mythic foils here) are left out.
    assert!(!cfg.sheets["craftedMythic"].cards.contains_key("e"));
    assert!(cfg.sheets["craftedMythic"].cards.contains_key("f"));
    assert!(cfg.sheets["craftedRareFoil"].cards.contains_key("d"));
    assert!(cfg.sheets["craftedCommonFoil"].cards.contains_key("a"));
    assert!(!cfg.sheets.contains_key("craftedMythicFoil"));
    // 2 foil tiers × 2 top tiers, weighted by the products of their weights.
    assert_eq!(cfg.boosters.len(), 4);
    assert_eq!(cfg.boosters_total_weight, (3 + 1) * (9 + 1));
    for pack in &cfg.boosters {
        assert_eq!(pack.contents.values().sum::<u32>(), 4);
        assert_eq!(pack.contents["craftedCommon"], 2);
    }
}

#[test]
fn a_slot_with_nothing_to_draw_fails() {
    let mut set = set();
    let mut recipe = recipe();
    recipe.slots[0].count = 10;
    assert!(craft_booster(&mut set, &recipe, &prices()).is_err());
}

#[test]
fn drops_are_groups_with_a_twin_as_premium_printing() {
    let groups = drop_groups(&set());
    assert_eq!(groups.iter().map(|g| g.id.as_str()).collect::<Vec<_>>(), ["Cats", "Dogs"]);
    assert!(groups.iter().all(|g| g.kind == GroupKind::Drop));
    assert_eq!(groups[1].slots, [vec!["10".to_string(), "10★".to_string()], vec!["11".to_string()]]);
}

#[test]
fn the_shipped_secret_lair_recipe_is_read() {
    let cfg = DataConfig::from_json(DATA_CONFIG).unwrap();
    let recipe = &cfg.crafted_boosters["SLD"];
    assert_eq!(recipe.kind, "secret-lair");
    assert!(!recipe.slots.is_empty());
}
