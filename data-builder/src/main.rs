//! Builds the game data into a directory (the app does the same itself at launch).
//!
//! Usage:
//!   data-builder --sets all|BLB,FDN [--set-types core,expansion] [--boosters play,draft,set,collector]
//!                [--out dist] [--cache .cache] [--sim-boosters 50000]
//!
//! Without `--set-types`, `--sets all` keeps the types listed in `config/data.json`.
//!
//! Output: `<out>/manifest.json`, `<out>/prices.json`, `<out>/sets/<CODE>.json`.

use std::error::Error;
use unsealed_core::builder::{build, BuildOptions};
use unsealed_core::catalog::{AllSets, SetSelection};
use unsealed_core::update::DataConfig;

/// Defaults shared with the app.
const DATA_CONFIG: &str = include_str!("../../config/data.json");

fn parse_args() -> Result<BuildOptions, String> {
    let defaults = DataConfig::from_json(DATA_CONFIG).map_err(|e| format!("config/data.json: {e}"))?;
    let mut opts = BuildOptions {
        sets: SetSelection::List(Vec::new()),
        set_types: defaults.set_types,
        boosters: defaults.boosters,
        out: "dist".into(),
        cache: ".cache".into(),
        sim_boosters: 50_000,
        token_foil_one_in: defaults.token_foil_one_in,
        art_cards: defaults.art_cards,
    };
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("missing value for {flag}"))?;
        let list = |v: &str| v.split(',').map(|s| s.trim().to_string()).collect::<Vec<_>>();
        match flag.as_str() {
            "--sets" if value == "all" => opts.sets = SetSelection::All(AllSets::All),
            "--sets" => opts.sets = SetSelection::List(list(&value).iter().map(|s| s.to_uppercase()).collect()),
            "--set-types" => opts.set_types = list(&value),
            "--boosters" => opts.boosters = list(&value),
            "--out" => opts.out = value.into(),
            "--cache" => opts.cache = value.into(),
            "--sim-boosters" => {
                opts.sim_boosters = value.parse().map_err(|_| "invalid --sim-boosters")?
            }
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    if opts.sets == SetSelection::List(Vec::new()) {
        return Err("--sets is required (e.g. --sets all, or --sets BLB,FDN)".to_string());
    }
    Ok(opts)
}

fn main() -> Result<(), Box<dyn Error>> {
    let opts = parse_args()?;
    // Print each step once, not every progress tick of a download.
    let mut last = String::new();
    let manifest = build(&opts, &mut |line, _| {
        if line != last {
            println!("{line}");
            last = line.to_string();
        }
    })
    .map_err(|e| e.to_string())?;
    println!("{} sets written to {}", manifest.sets.len(), opts.out.display());
    if !manifest.skipped.is_empty() {
        println!("{} sets left out:", manifest.skipped.len());
        for (code, reason) in &manifest.skipped {
            println!("  {code}: {reason}");
        }
    }
    Ok(())
}
