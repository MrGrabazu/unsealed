use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;
use unsealed_core::{open_booster, simulate, BoosterConfig, ColorIndex};

const FIXTURE: &str = include_str!("fixtures/mini_set.json");

fn no_colors() -> ColorIndex {
    ColorIndex::default()
}

/// A balanced sheet: 2 mono cards per color (`w1`, `w2`, `u1`, ...) and 2 colorless cards.
fn balanced_config(count: u32, allow_duplicates: bool) -> (BoosterConfig, ColorIndex) {
    let mut cards = Vec::new();
    let mut colors = ColorIndex::default();
    for (symbol, prefix) in [("W", "w"), ("U", "u"), ("B", "b"), ("R", "r"), ("G", "g")] {
        for n in 1..=2 {
            let id = format!("{prefix}{n}");
            colors.insert(&id, &[symbol]);
            cards.push(format!(r#""{id}": 1"#));
        }
    }
    for id in ["a1", "a2"] {
        colors.insert(id, &[] as &[&str]);
        cards.push(format!(r#""{id}": 1"#));
    }
    let json = format!(
        r#"{{ "boosters": [ {{ "contents": {{ "common": {count} }}, "weight": 1 }} ],
             "boostersTotalWeight": 1,
             "sheets": {{ "common": {{ "allowDuplicates": {allow_duplicates}, "balanceColors": true,
               "cards": {{ {} }}, "foil": false, "totalWeight": 12 }} }},
             "sourceSetCodes": [] }}"#,
        cards.join(",")
    );
    (serde_json::from_str(&json).unwrap(), colors)
}

/// Number of cards of each mono color (W, U, B, R, G order) in a drawn pack.
fn mono_counts(pack: &[unsealed_core::DrawnCard]) -> [usize; 5] {
    let mut counts = [0; 5];
    for card in pack {
        if let Some(i) = "wubrg".find(&card.uuid[..1]) {
            counts[i] += 1;
        }
    }
    counts
}

fn cfg() -> BoosterConfig {
    BoosterConfig::from_set_json(FIXTURE, "play").expect("valid fixture")
}

#[test]
fn unknown_booster_kind_is_an_error() {
    assert!(BoosterConfig::from_set_json(FIXTURE, "collector").is_err());
}

#[test]
fn same_seed_gives_same_booster() {
    let c = cfg();
    let a = open_booster(&c, &no_colors(), &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
    let b = open_booster(&c, &no_colors(), &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
    assert_eq!(a, b);
}

#[test]
fn no_duplicates_on_sheets_that_forbid_them() {
    let c = cfg();
    let mut rng = ChaCha8Rng::seed_from_u64(1);
    for _ in 0..1000 {
        let pack = open_booster(&c, &no_colors(), &mut rng).unwrap();
        let rares = pack.iter().filter(|d| d.sheet == "rareMythic").count();
        assert_eq!(rares, 1);
    }
}

#[test]
fn balanced_sheet_always_has_every_color_without_duplicates() {
    let (cfg, colors) = balanced_config(6, false);
    let mut rng = ChaCha8Rng::seed_from_u64(7);
    for _ in 0..5_000 {
        let pack = open_booster(&cfg, &colors, &mut rng).unwrap();
        assert_eq!(pack.len(), 6);
        assert!(mono_counts(&pack).iter().all(|&n| n >= 1), "missing a color: {pack:?}");
        let mut ids: Vec<&str> = pack.iter().map(|c| c.uuid.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 6, "duplicate in {pack:?}");
    }
}

#[test]
fn balanced_sheet_with_duplicates_still_balances() {
    let (cfg, colors) = balanced_config(5, true);
    let mut rng = ChaCha8Rng::seed_from_u64(8);
    for _ in 0..2_000 {
        let pack = open_booster(&cfg, &colors, &mut rng).unwrap();
        assert_eq!(mono_counts(&pack), [1, 1, 1, 1, 1]);
    }
}

#[test]
fn balancing_is_skipped_when_there_are_fewer_cards_than_colors() {
    let (cfg, colors) = balanced_config(3, false);
    let pack = open_booster(&cfg, &colors, &mut ChaCha8Rng::seed_from_u64(9)).unwrap();
    assert_eq!(pack.len(), 3);
}

#[test]
fn balanced_sheet_is_reproducible_and_position_independent_of_color() {
    let (cfg, colors) = balanced_config(6, false);
    let draw = |seed| open_booster(&cfg, &colors, &mut ChaCha8Rng::seed_from_u64(seed)).unwrap();
    assert_eq!(draw(5), draw(5));
    // The first card must not always be the white one: the pack is shuffled.
    let mut first_colors: HashMap<char, u32> = HashMap::new();
    for seed in 0..500 {
        let first = draw(seed)[0].uuid.chars().next().unwrap();
        *first_colors.entry(first).or_default() += 1;
    }
    assert!(first_colors.len() >= 5, "{first_colors:?}");
}

#[test]
fn balanced_sheets_are_excluded_from_rate_comparison() {
    let (cfg, colors) = balanced_config(6, false);
    let report = simulate(&cfg, &colors, 2_000, &mut ChaCha8Rng::seed_from_u64(3)).unwrap();
    assert!(report.expected.is_empty());
}

#[test]
fn observed_rates_match_expected_over_100k_boosters() {
    let c = cfg();
    let report = simulate(&c, &no_colors(), 100_000, &mut ChaCha8Rng::seed_from_u64(2024)).unwrap();
    let dev = report.max_abs_deviation();
    assert!(dev < 0.01, "max deviation {dev} too large: {report:#?}");
}
