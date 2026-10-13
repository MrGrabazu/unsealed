//! Builds the game data into a directory (the app does the same itself at launch).
//!
//! Usage:
//!   data-builder --sets BLB,FDN [--boosters play,draft,set,collector] [--out dist] [--cache .cache] [--sim-boosters 50000]
//!
//! Output: `<out>/manifest.json`, `<out>/prices.json`, `<out>/sets/<CODE>.json`.

use std::error::Error;
use unsealed_core::builder::{build, BuildOptions};

fn parse_args() -> Result<BuildOptions, String> {
    let mut opts = BuildOptions {
        sets: Vec::new(),
        boosters: ["play", "draft", "set", "collector"].map(String::from).to_vec(),
        out: "dist".into(),
        cache: ".cache".into(),
        sim_boosters: 50_000,
    };
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or_else(|| format!("missing value for {flag}"))?;
        let list = |v: &str| v.split(',').map(|s| s.trim().to_string()).collect::<Vec<_>>();
        match flag.as_str() {
            "--sets" => opts.sets = list(&value).iter().map(|s| s.to_uppercase()).collect(),
            "--boosters" => opts.boosters = list(&value),
            "--out" => opts.out = value.into(),
            "--cache" => opts.cache = value.into(),
            "--sim-boosters" => {
                opts.sim_boosters = value.parse().map_err(|_| "invalid --sim-boosters")?
            }
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    if opts.sets.is_empty() {
        return Err("--sets is required (e.g. --sets BLB,FDN)".to_string());
    }
    Ok(opts)
}

fn main() -> Result<(), Box<dyn Error>> {
    let opts = parse_args()?;
    let manifest = build(&opts, &mut |line| println!("{line}")).map_err(|e| e.to_string())?;
    println!("{} sets written to {}", manifest.sets.len(), opts.out.display());
    Ok(())
}
