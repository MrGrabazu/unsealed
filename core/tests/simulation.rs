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

/// Like Alpha's commons: 84 cards on a 121-slot print sheet (one card fills 1 to 5
/// slots), 11 drawn without duplicates.
fn print_sheet_config() -> BoosterConfig {
    let weights: Vec<u64> = (0..84).map(|i| [1, 1, 1, 2, 3, 5][i % 6]).collect();
    let total: u64 = weights.iter().sum();
    let cards: Vec<String> = weights.iter().enumerate().map(|(i, w)| format!(r#""c{i}": {w}"#)).collect();
    let json = format!(
        r#"{{ "boosters": [ {{ "contents": {{ "common": 11 }}, "weight": 1 }} ],
             "boostersTotalWeight": 1,
             "sheets": {{ "common": {{ "cards": {{ {} }}, "foil": false, "totalWeight": {total} }} }},
             "sourceSetCodes": [] }}"#,
        cards.join(",")
    );
    serde_json::from_str(&json).unwrap()
}

#[test]
fn uneven_sheets_without_duplicates_match_their_expected_rates() {
    let report = simulate(&print_sheet_config(), &no_colors(), 50_000, &mut ChaCha8Rng::seed_from_u64(3)).unwrap();
    // A first-order estimate (11 × weight / total) is off by more than 10 standard deviations here.
    assert!(report.max_abs_z() < 5.0, "max |z| = {}", report.max_abs_z());
}

/// Exact inclusion probabilities of successive sampling, by enumerating every draw order.
fn brute_force_inclusion(weights: &[f64], n: usize) -> Vec<f64> {
    fn walk(weights: &[f64], left: &mut Vec<bool>, n: usize, p: f64, out: &mut [f64]) {
        if n == 0 {
            return;
        }
        let total: f64 = weights.iter().zip(left.iter()).filter(|(_, &l)| l).map(|(w, _)| w).sum();
        for i in 0..weights.len() {
            if left[i] {
                let q = p * weights[i] / total;
                out[i] += q;
                left[i] = false;
                walk(weights, left, n - 1, q, out);
                left[i] = true;
            }
        }
    }
    let mut out = vec![0.0; weights.len()];
    walk(weights, &mut vec![true; weights.len()], n, 1.0, &mut out);
    out
}

#[test]
fn inclusion_probabilities_match_an_exhaustive_enumeration() {
    use unsealed_core::sim::inclusion_probabilities;
    for (weights, n) in [(vec![1, 1, 3, 5, 2, 1], 2), (vec![1, 2, 3, 5, 1, 4, 2], 3), (vec![1, 1, 10, 1], 2)] {
        let exact = brute_force_inclusion(&weights.iter().map(|&w| w as f64).collect::<Vec<_>>(), n);
        let computed = inclusion_probabilities(&weights, n);
        for (c, e) in computed.iter().zip(&exact) {
            assert!((c - e).abs() < 1e-4, "{weights:?} n={n}: {computed:?} vs {exact:?}");
        }
    }
    // Closed forms: equal weights, a single card, everything drawn, zero weight.
    assert!(inclusion_probabilities(&[2, 2, 2, 2], 3).iter().all(|p| (p - 0.75).abs() < 1e-12));
    assert_eq!(inclusion_probabilities(&[1, 3], 1), [0.25, 0.75]);
    assert_eq!(inclusion_probabilities(&[1, 4, 0], 2), [1.0, 1.0, 0.0]);
}

#[test]
fn a_light_card_among_many_heavy_ones_keeps_its_exact_rate() {
    // Like CLB's set booster wildcard: hundreds of heavily weighted cards, 2 drawn. A coarse
    // integration overestimated the light cards about twentyfold.
    use unsealed_core::sim::inclusion_probabilities;
    let mut weights = vec![50_000u64; 200];
    weights.push(1_000);
    let computed = inclusion_probabilities(&weights, 2)[200];
    // Exact for two draws: picked first, or picked second after a heavy card.
    let total = 200.0 * 50_000.0 + 1_000.0;
    let exact = 1_000.0 / total + (1.0 - 1_000.0 / total) * 1_000.0 / (total - 50_000.0);
    assert!((computed - exact).abs() / exact < 1e-3, "{computed} vs {exact}");
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

/// Price of the fixture cards, in cents.
fn fixture_price(uuid: &str, _foil: bool) -> Option<u32> {
    match uuid {
        "c1" => Some(100),
        "c2" => Some(200),
        "c3" => Some(300),
        "r1" => Some(500),
        "m1" => Some(5_000),
        _ => None,
    }
}

#[test]
fn expected_value_matches_the_simulated_average() {
    use unsealed_core::sim::expected_value_cents;
    let c = cfg();
    let expected = expected_value_cents(&c, fixture_price);

    let mut rng = ChaCha8Rng::seed_from_u64(11);
    let n = 100_000u64;
    let total: u64 = (0..n)
        .map(|_| {
            open_booster(&c, &no_colors(), &mut rng)
                .unwrap()
                .iter()
                .map(|d| u64::from(fixture_price(&d.uuid, d.foil).unwrap()))
                .sum::<u64>()
        })
        .sum();
    let simulated = total as f64 / n as f64;
    assert!((simulated - expected).abs() / expected < 0.02, "simulated {simulated} vs expected {expected}");
}

#[test]
fn observed_rates_match_expected_over_100k_boosters() {
    let c = cfg();
    let report = simulate(&c, &no_colors(), 100_000, &mut ChaCha8Rng::seed_from_u64(2024)).unwrap();
    let dev = report.max_abs_deviation();
    assert!(dev < 0.01, "max deviation {dev} too large: {report:#?}");
}
