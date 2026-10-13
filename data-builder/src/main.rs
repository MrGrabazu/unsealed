//! Builds the data published for the client (see AGENTS.md, section 5).
//!
//! Usage:
//!   data-builder --sets BLB,FDN [--boosters play] [--out dist] [--cache .cache] [--sim-boosters 50000]
//!
//! Output: `<out>/manifest.json`, `<out>/prices.json`, `<out>/sets/<CODE>.json`.
//! Every kept booster config must pass a rate simulation before it is written.

use flate2::read::GzDecoder;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::error::Error;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use unsealed_core::catalog::{Manifest, ManifestSet, PriceFile};
use unsealed_core::import::{prices_from_scryfall_jsonl, set_from_mtgjson, source_set_codes};
use unsealed_core::simulate;

const USER_AGENT: &str = "Unsealed-data-builder/0.1";
const ACCEPT: &str = "application/json;q=0.9,*/*;q=0.8";
const MTGJSON_MAX_AGE: Duration = Duration::from_secs(24 * 3600);
/// A config whose simulated rates deviate by more than this many standard deviations is rejected.
const MAX_Z: f64 = 5.0;

struct Options {
    sets: Vec<String>,
    boosters: Vec<String>,
    out: PathBuf,
    cache: PathBuf,
    sim_boosters: u64,
}

fn parse_args() -> Result<Options, String> {
    let mut opts = Options {
        sets: Vec::new(),
        boosters: vec!["play".to_string()],
        out: PathBuf::from("dist"),
        cache: PathBuf::from(".cache"),
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

fn http_get(url: &str) -> Result<ureq::Response, Box<dyn Error>> {
    Ok(ureq::get(url).set("User-Agent", USER_AGENT).set("Accept", ACCEPT).call()?)
}

fn download(url: &str, dest: &Path) -> Result<(), Box<dyn Error>> {
    let mut reader = http_get(url)?.into_reader();
    let tmp = dest.with_extension("part");
    std::io::copy(&mut reader, &mut File::create(&tmp)?)?;
    fs::rename(&tmp, dest)?;
    Ok(())
}

fn is_fresh(path: &Path, max_age: Duration) -> bool {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < max_age)
}

/// Returns the raw MTGJSON file of a set, downloading it when the cache is older than a day.
fn read_mtgjson_set(cache: &Path, code: &str) -> Result<String, Box<dyn Error>> {
    let path = cache.join(format!("{code}.json"));
    if !is_fresh(&path, MTGJSON_MAX_AGE) {
        println!("{code}: downloading from MTGJSON");
        download(&format!("https://mtgjson.com/api/v5/{code}.json"), &path)?;
    }
    Ok(fs::read_to_string(&path)?)
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<u64, Box<dyn Error>> {
    serde_json::to_writer(File::create(path)?, value)?;
    Ok(fs::metadata(path)?.len())
}

#[derive(Deserialize)]
struct BulkList {
    data: Vec<BulkEntry>,
}

#[derive(Deserialize)]
struct BulkEntry {
    #[serde(rename = "type")]
    kind: String,
    updated_at: String,
    jsonl_download_uri: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let opts = parse_args()?;
    let sets_dir = opts.out.join("sets");
    fs::create_dir_all(&opts.cache)?;
    fs::create_dir_all(&sets_dir)?;

    let kinds: Vec<&str> = opts.boosters.iter().map(String::as_str).collect();
    let mut manifest_sets = BTreeMap::new();
    let mut wanted = HashSet::new();

    for code in &opts.sets {
        let json = read_mtgjson_set(&opts.cache, code)?;
        let extra_jsons = source_set_codes(&json, &kinds)
            .map_err(|e| format!("{code}: {e}"))?
            .iter()
            .map(|c| read_mtgjson_set(&opts.cache, c))
            .collect::<Result<Vec<_>, _>>()?;
        let extra_refs: Vec<&str> = extra_jsons.iter().map(String::as_str).collect();
        let set = set_from_mtgjson(&json, &kinds, &extra_refs).map_err(|e| format!("{code}: {e}"))?;

        for (kind, cfg) in &set.boosters {
            let report = simulate(cfg, opts.sim_boosters, &mut ChaCha8Rng::seed_from_u64(1))?;
            let z = report.max_abs_z();
            if z > MAX_Z {
                return Err(format!("{code}/{kind}: simulation failed, max |z| = {z:.2}").into());
            }
            println!("{code}/{kind}: simulation OK over {} boosters (max |z| = {z:.2})", opts.sim_boosters);
        }

        wanted.extend(set.cards.iter().map(|c| c.scryfall_id.clone()));
        let size = write_json(&sets_dir.join(format!("{code}.json")), &set)?;
        println!("{code}: {} cards, {} KB", set.cards.len(), size / 1024);
        manifest_sets.insert(
            code.clone(),
            ManifestSet {
                name: set.name,
                release_date: set.release_date,
                source_version: set.source_version,
                boosters: set.boosters.keys().cloned().collect(),
            },
        );
    }

    // Prices come from the Scryfall "default_cards" bulk file, cached by file name
    // (the name embeds its timestamp, so a new version is downloaded automatically).
    let list: BulkList = serde_json::from_reader(http_get("https://api.scryfall.com/bulk-data")?.into_reader())?;
    let bulk = list
        .data
        .into_iter()
        .find(|b| b.kind == "default_cards")
        .ok_or("Scryfall default_cards bulk file not found")?;
    let file_name = bulk.jsonl_download_uri.rsplit('/').next().unwrap_or("default-cards.jsonl.gz");
    let bulk_path = opts.cache.join(file_name);
    if !bulk_path.exists() {
        println!("downloading Scryfall bulk data ({file_name})");
        download(&bulk.jsonl_download_uri, &bulk_path)?;
    }
    let reader = BufReader::new(GzDecoder::new(BufReader::new(File::open(&bulk_path)?)));
    let cards = prices_from_scryfall_jsonl(reader, &wanted)?;
    let priced = cards.values().filter(|p| p.usd.is_some() || p.usd_foil.is_some()).count();
    println!("prices: {} cards matched out of {}, {priced} with a USD price", cards.len(), wanted.len());
    let size = write_json(&opts.out.join("prices.json"), &PriceFile { updated_at: bulk.updated_at.clone(), cards })?;
    println!("prices.json: {} KB", size / 1024);

    let generated_at = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_secs().to_string();
    write_json(
        &opts.out.join("manifest.json"),
        &Manifest { generated_at, prices_updated_at: bulk.updated_at, sets: manifest_sets },
    )?;
    println!("manifest.json written to {}", opts.out.display());
    Ok(())
}
