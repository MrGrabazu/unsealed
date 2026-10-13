use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use unsealed_core::{open_booster, simulate, BoosterConfig};

const FIXTURE: &str = include_str!("fixtures/mini_set.json");

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
    let a = open_booster(&c, &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
    let b = open_booster(&c, &mut ChaCha8Rng::seed_from_u64(42)).unwrap();
    assert_eq!(a, b);
}

#[test]
fn no_duplicates_on_sheets_that_forbid_them() {
    let c = cfg();
    let mut rng = ChaCha8Rng::seed_from_u64(1);
    for _ in 0..1000 {
        let pack = open_booster(&c, &mut rng).unwrap();
        let rares = pack.iter().filter(|d| d.sheet == "rareMythic").count();
        assert_eq!(rares, 1);
    }
}

#[test]
fn observed_rates_match_expected_over_100k_boosters() {
    let c = cfg();
    let report = simulate(&c, 100_000, &mut ChaCha8Rng::seed_from_u64(2024)).unwrap();
    let dev = report.max_abs_deviation();
    assert!(dev < 0.01, "max deviation {dev} too large: {report:#?}");
}
