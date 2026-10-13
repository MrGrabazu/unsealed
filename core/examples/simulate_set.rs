//! Validates a real MTGJSON set file by simulation.
//!
//! Usage: cargo run --release --example simulate_set -- <set.json> <kind> [boosters] [seed]

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::collections::BTreeMap;
use unsealed_core::{open_booster, simulate, BoosterConfig};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: simulate_set <set.json> <kind> [boosters] [seed]");
        std::process::exit(2);
    }
    let n: u64 = args.get(3).map_or(100_000, |s| s.parse().expect("boosters"));
    let seed: u64 = args.get(4).map_or(1, |s| s.parse().expect("seed"));

    let json = std::fs::read_to_string(&args[1]).expect("read set file");
    let cfg = BoosterConfig::from_set_json(&json, &args[2]).expect("valid booster config");
    println!("config OK: {:?}, {} pack variants", cfg.name, cfg.boosters.len());
    for pack in &cfg.boosters {
        println!("  weight {:>4}: {:?}", pack.weight, pack.contents);
    }

    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let sample = open_booster(&cfg, &mut rng).expect("draw");
    println!("sample booster ({} cards):", sample.len());
    for c in &sample {
        println!("  {:<24} {} foil={}", c.sheet, c.uuid, c.foil);
    }

    let report = simulate(&cfg, n, &mut rng).expect("simulation");

    // z-score per card: (observed - expected) / standard deviation of the observed mean.
    let mut z: Vec<(f64, &(String, bool), f64, f64)> = report
        .expected
        .iter()
        .map(|(k, &e)| {
            let o = report.observed.get(k).copied().unwrap_or(0.0);
            (((o - e) / (e / n as f64).sqrt()).abs(), k, o, e)
        })
        .collect();
    z.sort_by(|a, b| b.0.total_cmp(&a.0));

    let mut per_sheet: BTreeMap<&str, f64> = BTreeMap::new();
    for pack in &cfg.boosters {
        let p = pack.weight as f64 / cfg.boosters_total_weight as f64;
        for (slot, &cnt) in &pack.contents {
            *per_sheet.entry(slot).or_default() += p * cnt as f64;
        }
    }
    println!("expected cards per booster by sheet: {per_sheet:.3?}");
    println!("{} boosters, {} distinct (card, foil) entries", n, report.expected.len());
    println!("max absolute deviation per booster: {:.5}", report.max_abs_deviation());
    println!("worst z-scores (|z| above ~5 would be suspicious):");
    for (zv, (uuid, foil), o, e) in z.iter().take(5) {
        println!("  z={zv:.2} {uuid} foil={foil} observed={o:.5} expected={e:.5}");
    }
}
