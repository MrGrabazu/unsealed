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

    // Expected value of a booster, if the prices file sits next to the sets folder.
    if let Some(prices) = std::path::Path::new(&args[1])
        .parent()
        .and_then(|sets| sets.parent())
        .and_then(|dist| std::fs::read_to_string(dist.join("prices.json")).ok())
        .and_then(|text| serde_json::from_str::<unsealed_core::catalog::PriceFile>(&text).ok())
    {
        // Re-simulate counting every drawn card (balanced sheets included).
        let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(1));
        let mut values: Vec<u64> = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let value: u64 = open_booster(cfg, &colors, &mut rng)
                .expect("draw")
                .iter()
                .map(|c| {
                    let p = prices.cards.get(&c.uuid);
                    u64::from(p.and_then(|p| if c.foil { p.usd_foil } else { p.usd }).unwrap_or(0))
                })
                .sum();
            values.push(value);
        }
        values.sort_unstable();
        let ev = values.iter().sum::<u64>() as f64 / n as f64 / 100.0;
        let median = values[values.len() / 2] as f64 / 100.0;
        println!("expected market value of a booster: ${ev:.2} (${:.2} if every card is sold at 80%)", ev * 0.8);
        println!("median booster: ${median:.2} market (${:.2} sold at 80%)", median * 0.8);
        // Share of boosters whose cards, all sold at 80%, pay back a $4.00 booster.
        let paying = values.iter().filter(|&&v| v * 80 / 100 >= 400).count();
        println!("boosters worth at least $4.00 once sold at 80%: {:.1}%", paying as f64 * 100.0 / n as f64);
    }
    println!("{} boosters, {} (card, foil) entries checked", n, report.expected.len());
    println!("max absolute deviation per booster: {:.5}", report.max_abs_deviation());
    println!("max |z|: {:.2} (above ~5 would be suspicious)", report.max_abs_z());
}
