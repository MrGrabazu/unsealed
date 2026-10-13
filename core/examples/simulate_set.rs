//! Validates a built set file by simulation and prints a sample booster.
//!
//! Usage: cargo run --release -p unsealed-core --example simulate_set -- <dist/sets/CODE.json> <kind> [boosters] [seed]

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::collections::BTreeMap;
use unsealed_core::catalog::SetFile;
use unsealed_core::{open_booster, simulate, ColorIndex};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: simulate_set <set.json> <kind> [boosters] [seed]");
        std::process::exit(2);
    }
    let n: u64 = args.get(3).map_or(100_000, |s| s.parse().expect("boosters"));
    let seed: u64 = args.get(4).map_or(1, |s| s.parse().expect("seed"));

    let json = std::fs::read_to_string(&args[1]).expect("read set file");
    let set: SetFile = serde_json::from_str(&json).expect("valid set file");
    let cfg = set.boosters.get(&args[2]).expect("booster kind present in the set file");
    let colors = ColorIndex::from_cards(&set.cards);
    println!("{} / {:?}: {} pack variants", set.code, cfg.name, cfg.boosters.len());
    for pack in &cfg.boosters {
        println!("  weight {:>4}: {:?}", pack.weight, pack.contents);
    }

    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let names: BTreeMap<&str, &str> =
        set.cards.iter().map(|c| (c.uuid.as_str(), c.name.as_str())).collect();
    let sample = open_booster(cfg, &colors, &mut rng).expect("draw");
    println!("sample booster ({} cards):", sample.len());
    for c in &sample {
        println!("  {:<24} {} foil={}", c.sheet, names[c.uuid.as_str()], c.foil);
    }

    let report = simulate(cfg, &colors, n, &mut rng).expect("simulation");
    println!("{} boosters, {} (card, foil) entries checked", n, report.expected.len());
    println!("max absolute deviation per booster: {:.5}", report.max_abs_deviation());
    println!("max |z|: {:.2} (above ~5 would be suspicious)", report.max_abs_z());
}
