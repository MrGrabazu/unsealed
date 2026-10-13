//! Diagnostic of CLB's set booster, which fails the rate simulation by a hair. Needs the
//! raw MTGJSON files in `.cache/probe/` (CLB.json, PLST.json), so it only runs on demand:
//! `cargo test --release -p unsealed-core --test clb_probe -- --ignored --nocapture`.

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use unsealed_core::import::set_from_mtgjson;
use unsealed_core::{simulate, ColorIndex};

#[test]
#[ignore = "needs .cache/probe/CLB.json and PLST.json"]
fn which_cards_of_clb_set_boosters_deviate() {
    let read = |name: &str| std::fs::read_to_string(format!("../.cache/probe/{name}.json")).unwrap();
    let set = set_from_mtgjson(&read("CLB"), &["set"], &[&read("PLST")]).unwrap();
    let cfg = &set.boosters["set"];
    let colors = ColorIndex::from_cards(&set.cards);
    for seed in [1u64, 2, 3] {
        let report = simulate(cfg, &colors, 20_000, &mut ChaCha8Rng::seed_from_u64(seed)).unwrap();
        let n = report.boosters as f64;
        let mut worst: Vec<(f64, String, f64, f64)> = report
            .expected
            .iter()
            .filter(|&(_, &e)| e * n >= 30.0)
            .map(|((uuid, foil), &e)| {
                let o = report.observed.get(&(uuid.clone(), *foil)).copied().unwrap_or(0.0);
                let sheets: Vec<&str> = cfg
                    .sheets
                    .iter()
                    .filter(|(_, s)| s.foil == *foil && s.cards.contains_key(uuid))
                    .map(|(name, _)| name.as_str())
                    .collect();
                (((o - e) / (e / n).sqrt()), format!("{uuid} foil={foil} {sheets:?}"), o, e)
            })
            .collect();
        worst.sort_by(|a, b| b.0.abs().total_cmp(&a.0.abs()));
        println!("seed {seed}: max |z| = {:.2}", report.max_abs_z());
        for (z, what, o, e) in worst.iter().take(4) {
            println!("  z={z:+.2} observed={o:.4} expected={e:.4} {what}");
        }
    }
}
