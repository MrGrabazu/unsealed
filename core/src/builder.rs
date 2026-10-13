//! Builds the compact game data (sets, prices, manifest) from MTGJSON and Scryfall.
//! Shared by the `data-builder` CLI and by the app, which refreshes its data in the
//! background at launch. Every kept booster config must pass a rate simulation.

use crate::catalog::{BoosterValue, Manifest, ManifestSet, PriceFile, PriceSet};
use crate::import::{prices_by_uuid, prices_from_scryfall_jsonl, set_from_mtgjson, source_set_codes};
use crate::sim::expected_value_cents;
use crate::{simulate, ColorIndex};
use flate2::read::GzDecoder;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const USER_AGENT: &str = "Unsealed/0.1 (unofficial fan project)";
const ACCEPT: &str = "application/json;q=0.9,*/*;q=0.8";
/// Raw MTGJSON set files are reused for this long.
const MTGJSON_MAX_AGE: Duration = Duration::from_secs(24 * 3600);
/// A config whose simulated rates deviate by more than this many standard deviations is rejected.
const MAX_Z: f64 = 5.0;

pub type BuildError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub sets: Vec<String>,
    pub boosters: Vec<String>,
    /// Where `manifest.json`, `prices.json` and `sets/` are written.
    pub out: PathBuf,
    /// Downloaded raw files (large: the Scryfall bulk file is several hundred MB).
    pub cache: PathBuf,
    pub sim_boosters: u64,
}

fn http_get(url: &str) -> Result<ureq::Response, BuildError> {
    Ok(ureq::get(url).set("User-Agent", USER_AGENT).set("Accept", ACCEPT).call()?)
}

fn download(url: &str, dest: &Path) -> Result<(), BuildError> {
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
fn read_mtgjson_set(cache: &Path, code: &str, progress: &mut dyn FnMut(&str)) -> Result<String, BuildError> {
    let path = cache.join(format!("{code}.json"));
    if !is_fresh(&path, MTGJSON_MAX_AGE) {
        progress(&format!("Downloading {code} from MTGJSON"));
        download(&format!("https://mtgjson.com/api/v5/{code}.json"), &path)?;
    }
    Ok(fs::read_to_string(&path)?)
}

/// Writes a file next to its destination, then renames it over it: a reader never sees half a file.
fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<u64, BuildError> {
    let tmp = path.with_extension("tmp");
    serde_json::to_writer(File::create(&tmp)?, value)?;
    let size = fs::metadata(&tmp)?.len();
    fs::rename(&tmp, path)?;
    Ok(size)
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

/// Runs the whole build. `progress` receives short human-readable status lines.
/// The manifest is written last, so an interrupted build never advertises missing files.
pub fn build(opts: &BuildOptions, progress: &mut dyn FnMut(&str)) -> Result<Manifest, BuildError> {
    let sets_dir = opts.out.join("sets");
    fs::create_dir_all(&opts.cache)?;
    fs::create_dir_all(&sets_dir)?;

    let kinds: Vec<&str> = opts.boosters.iter().map(String::as_str).collect();
    let mut manifest_sets = BTreeMap::new();
    let mut wanted = HashSet::new();
    let mut all_cards = Vec::new();
    let mut built_boosters = Vec::new();

    for code in &opts.sets {
        let json = read_mtgjson_set(&opts.cache, code, progress)?;
        let extra_jsons = source_set_codes(&json, &kinds)
            .map_err(|e| format!("{code}: {e}"))?
            .iter()
            .map(|c| read_mtgjson_set(&opts.cache, c, progress))
            .collect::<Result<Vec<_>, _>>()?;
        let extra_refs: Vec<&str> = extra_jsons.iter().map(String::as_str).collect();
        let set = set_from_mtgjson(&json, &kinds, &extra_refs).map_err(|e| format!("{code}: {e}"))?;

        progress(&format!("Validating {code} pull rates"));
        let colors = ColorIndex::from_cards(&set.cards);
        for (kind, cfg) in &set.boosters {
            built_boosters.push((format!("{code}/{kind}"), cfg.clone()));
            let report = simulate(cfg, &colors, opts.sim_boosters, &mut ChaCha8Rng::seed_from_u64(1))?;
            let z = report.max_abs_z();
            if z > MAX_Z {
                return Err(format!("{code}/{kind}: simulation failed, max |z| = {z:.2}").into());
            }
        }

        wanted.extend(set.cards.iter().map(|c| c.scryfall_id.clone()));
        all_cards.extend(set.cards.iter().cloned());
        write_json(&sets_dir.join(format!("{code}.json")), &set)?;
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
    let file_name = bulk.jsonl_download_uri.rsplit('/').next().unwrap_or("default-cards.jsonl.gz").to_string();
    let bulk_path = opts.cache.join(&file_name);
    if !bulk_path.exists() {
        progress("Downloading Scryfall prices (large file)");
        download(&bulk.jsonl_download_uri, &bulk_path)?;
    }
    progress("Reading prices");
    let reader = BufReader::new(GzDecoder::new(BufReader::new(File::open(&bulk_path)?)));
    let by_scryfall_id = prices_from_scryfall_jsonl(reader, &wanted)?;
    let cards = prices_by_uuid(&by_scryfall_id, &all_cards);

    // Expected market value of each booster: the game prices boosters from it.
    let mut boosters = BTreeMap::new();
    for (key, cfg) in &built_boosters {
        let value = |pick: fn(&PriceSet, bool) -> Option<u32>| {
            expected_value_cents(cfg, |uuid, foil| cards.get(uuid).and_then(|p| pick(p, foil))).round() as u32
        };
        boosters.insert(
            key.clone(),
            BoosterValue {
                usd: value(|p, foil| if foil { p.usd_foil } else { p.usd }),
                eur: value(|p, foil| if foil { p.eur_foil } else { p.eur }),
            },
        );
    }
    write_json(&opts.out.join("prices.json"), &PriceFile { updated_at: bulk.updated_at.clone(), cards, boosters })?;

    let generated_at = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_secs().to_string();
    let manifest = Manifest { generated_at, prices_updated_at: bulk.updated_at, sets: manifest_sets };
    write_json(&opts.out.join("manifest.json"), &manifest)?;

    // Older bulk files are useless now and weigh hundreds of MB each.
    for entry in fs::read_dir(&opts.cache)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("default-cards-") && name != file_name {
            let _ = fs::remove_file(entry.path());
        }
    }
    Ok(manifest)
}
