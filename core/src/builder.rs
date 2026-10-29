//! Builds the compact game data (sets, prices, manifest) from MTGJSON and Scryfall.
//! Shared by the `data-builder` CLI and by the app, which refreshes its data in the
//! background at launch. Every kept booster config must pass a rate simulation; a set
//! that cannot be built is left out (and listed in the manifest) instead of failing all.

use crate::booster::BoosterConfig;
use crate::catalog::{
    set_code_of_file, set_file_name, BoosterValue, Manifest, ManifestSet, PriceFile, PriceSet, SetFile, SetSelection,
    is_collector_kind, kind_wanted, ARTISTS_FILE, COLLECTOR_KIND, DATA_FORMAT_VERSION, GUESTS_FILE,
};
use crate::crafted::{craft_booster, CraftedBooster};
use crate::import::{add_art_cards_from, add_art_slots, artist_index, box_toppers, crafted_set, guest_index, topper_cards, add_token_slots, add_tokens_from, art_set_code, compact_set, merge_foil_printings, merge_token_faces, ArtCardOdds, featured_card, token_set_code, prices_by_uuid, prices_from_scryfall_jsonl, read_all_printings, AllPrintings, RawSet};
use crate::sim::expected_value_cents;
use crate::{simulate, ColorIndex};
use flate2::read::GzDecoder;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::fs::{self, File};
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

pub(crate) const USER_AGENT: &str = "Unsealed/0.1 (unofficial fan project)";
pub(crate) const ACCEPT: &str = "application/json;q=0.9,*/*;q=0.8";
/// Every MTGJSON set in one file (about 180 MB compressed).
const ALL_PRINTINGS_URL: &str = "https://mtgjson.com/api/v5/AllPrintings.json.gz";
const ALL_PRINTINGS_FILE: &str = "AllPrintings.json.gz";
/// The MTGJSON file is reused for this long. A set's content barely changes once released
/// (prices, which move daily, come from Scryfall).
const MTGJSON_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 3600);
/// Booster configs that already passed the simulation, by fingerprint (see `fingerprint`).
const VALIDATED_FILE: &str = "validated.json";
/// A config whose simulated rates deviate by more than this many standard deviations is rejected.
const MAX_Z: f64 = 5.0;

pub type BuildError = Box<dyn std::error::Error + Send + Sync>;

/// Receives a short human-readable status line and the overall progress, from 0 to 1.
/// The progress is an estimate (fixed shares per step), but it never goes back.
pub type Progress<'a> = &'a mut dyn FnMut(&str, f64);

/// Where each step ends in the overall progress.
const MTGJSON_DOWNLOAD_END: f64 = 0.25;
const MTGJSON_READ_END: f64 = 0.3;
const SETS_END: f64 = 0.6;
const BULK_END: f64 = 0.9;

#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub sets: SetSelection,
    /// With `SetSelection::All`: the MTGJSON set types kept.
    pub set_types: Vec<String>,
    pub boosters: Vec<String>,
    /// Where `manifest.json`, `prices.json` and `sets/` are written.
    pub out: PathBuf,
    /// Downloaded raw files (large: the Scryfall bulk file is several hundred MB).
    pub cache: PathBuf,
    pub sim_boosters: u64,
    /// See `DataConfig::token_foil_one_in`.
    pub token_foil_one_in: u64,
    /// See `DataConfig::art_cards`.
    pub art_cards: ArtCardOdds,
    /// See `DataConfig::crafted_boosters`.
    pub crafted_boosters: BTreeMap<String, CraftedBooster>,
}

/// A GET with the headers Scryfall and MTGJSON ask for (shared with `scryfall`).
pub(crate) fn http_get(url: &str) -> Result<ureq::Response, BuildError> {
    Ok(ureq::get(url).set("User-Agent", USER_AGENT).set("Accept", ACCEPT).call()?)
}

/// A POST of a JSON body, with the same headers.
pub(crate) fn http_post_json(url: &str, body: &str) -> Result<ureq::Response, BuildError> {
    Ok(ureq::post(url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", ACCEPT)
        .set("Content-Type", "application/json")
        .send_string(body)?)
}

/// How many times a download cut off by the network is taken up again.
const DOWNLOAD_ATTEMPTS: u32 = 5;

/// Downloads a file, reporting the fraction received (at most once per percent) when the
/// server announces its size. A connection lost on the way (common on a large file: Wi-Fi,
/// sleep, an antivirus, Windows' "invalid handle" from its network stack) is taken up again
/// where it stopped when the server allows it (a Range request), else from the start.
pub(crate) fn download(url: &str, dest: &Path, on_fraction: &mut dyn FnMut(f64)) -> Result<(), BuildError> {
    let tmp = dest.with_extension("part");
    let mut file = File::create(&tmp)?;
    let (mut received, mut total, mut last_percent) = (0u64, None::<u64>, None);
    let mut buffer = vec![0u8; 64 * 1024];
    let mut attempt = 1;
    loop {
        let request = ureq::get(url).set("User-Agent", USER_AGENT).set("Accept", ACCEPT);
        let request = if received > 0 { request.set("Range", &format!("bytes={received}-")) } else { request };
        let response = match request.call() {
            Ok(response) => response,
            // Only a network failure is worth another try, not an answer such as a 404.
            Err(ureq::Error::Transport(_)) if attempt < DOWNLOAD_ATTEMPTS => {
                attempt += 1;
                std::thread::sleep(Duration::from_secs(2 * attempt as u64));
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        let length: Option<u64> = response.header("Content-Length").and_then(|v| v.parse().ok());
        if response.status() == 206 {
            total = total.or(length.map(|l| l + received));
        } else {
            // The whole file (first request, or a server ignoring Range): from the start.
            if received > 0 {
                file = File::create(&tmp)?;
                received = 0;
            }
            total = length.filter(|&t| t > 0);
        }
        let mut reader = response.into_reader();
        let cut = loop {
            let n = match reader.read(&mut buffer) {
                Ok(0) => break None,
                Ok(n) => n,
                Err(e) => break Some(e),
            };
            file.write_all(&buffer[..n])?;
            received += n as u64;
            if let Some(total) = total {
                let percent = received * 100 / total;
                if last_percent != Some(percent) {
                    last_percent = Some(percent);
                    on_fraction((received as f64 / total as f64).min(1.0));
                }
            }
        };
        // Cut short without an error is a cut too.
        let cut = cut.or_else(|| {
            total.filter(|&t| received < t).map(|t| io::Error::new(io::ErrorKind::UnexpectedEof, format!("{received} of {t} bytes received")))
        });
        match cut {
            None => break,
            Some(_) if attempt < DOWNLOAD_ATTEMPTS => {
                attempt += 1;
                std::thread::sleep(Duration::from_secs(2 * attempt as u64));
            }
            Some(e) => return Err(e.into()),
        }
    }
    file.flush()?;
    drop(file);
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

/// Every MTGJSON set, downloaded when the cached copy is too old.
fn read_mtgjson(cache: &Path, progress: Progress<'_>) -> Result<AllPrintings, BuildError> {
    let path = cache.join(ALL_PRINTINGS_FILE);
    if !is_fresh(&path, MTGJSON_MAX_AGE) {
        let message = "Downloading every set from MTGJSON (large file)";
        progress(message, 0.0);
        download(ALL_PRINTINGS_URL, &path, &mut |f| progress(message, MTGJSON_DOWNLOAD_END * f))?;
    }
    progress("Reading the sets", MTGJSON_DOWNLOAD_END);
    Ok(read_all_printings(GzDecoder::new(BufReader::new(File::open(&path)?)))?)
}

/// The requested sets that have a wanted booster, and the others with the reason.
fn select_sets(all: &AllPrintings, opts: &BuildOptions, kinds: &[&str]) -> (Vec<String>, BTreeMap<String, String>) {
    let mut skipped = BTreeMap::new();
    let mut selected = Vec::new();
    match &opts.sets {
        SetSelection::List(codes) => {
            for code in codes {
                match all.data.get(code) {
                    None => {
                        skipped.insert(code.clone(), "unknown set".to_string());
                    }
                    Some(set) if !set.has_any_booster(kinds) => {
                        skipped.insert(code.clone(), "no wanted booster".to_string());
                    }
                    Some(_) => selected.push(code.clone()),
                }
            }
        }
        SetSelection::All(_) => {
            for s in all.data.values().filter(|s| !s.is_online_only && opts.set_types.contains(&s.set_type)) {
                let offered = || s.booster.keys().cloned().collect::<Vec<_>>().join(", ");
                if !s.has_any_booster(kinds) {
                    // A set with boosters of none of the kinds kept: maybe a new kind of main
                    // booster (Wizards renames them now and then) that `data.json` should list.
                    if !s.booster.is_empty() {
                        skipped.insert(s.code.clone(), format!("no booster of a kind kept (MTGJSON has: {})", offered()));
                    }
                    continue;
                }
                // Only its collector boosters kept: its main booster is most likely of a kind
                // not listed yet. The set is offered, but says so. Not for a set sold in collector
                // boosters only (Doctor Who, Fallout).
                if s.booster.keys().filter(|k| kind_wanted(k, kinds)).all(|k| is_collector_kind(k))
                    && s.booster.keys().any(|k| !k.starts_with(COLLECTOR_KIND))
                {
                    skipped.insert(
                        format!("{}/main", s.code),
                        format!("only collector boosters kept: a new kind of main booster? (MTGJSON has: {})", offered()),
                    );
                }
                selected.push(s.code.clone());
            }
        }
    }
    (selected, skipped)
}

/// Builds one set's compact file and checks its booster configs by simulation, unless
/// an identical config already passed (`validated` maps config keys to fingerprints).
/// A booster kind that fails is dropped (and reported in `dropped` as "CODE/kind"); the
/// set fails only when none is left.
fn build_set(
    all: &AllPrintings,
    code: &str,
    kinds: &[&str],
    sim_boosters: u64,
    extra: &BuildOptions,
    validated: &mut BTreeMap<String, String>,
    dropped: &mut BTreeMap<String, String>,
) -> Result<SetFile, BuildError> {
    let raw = &all.data[code];
    let extras = raw
        .source_set_codes(kinds)?
        .iter()
        .map(|c| all.data.get(c).ok_or_else(|| format!("source set {c} not found")))
        .collect::<Result<Vec<_>, _>>()?;
    let mut set = compact_set(raw, &all.meta.version, kinds, &extras)?;
    merge_foil_printings(&mut set);
    if let Some(token_set) = all.data.get(&token_set_code(raw)) {
        add_tokens_from(&mut set, token_set);
    }
    merge_token_faces(&mut set);
    add_token_slots(&mut set, extra.token_foil_one_in);
    if let Some(art_set) = all.data.get(&art_set_code(code)) {
        add_art_cards_from(&mut set, art_set);
        add_art_slots(&mut set, &extra.art_cards);
    }
    // The box toppers of its booster boxes (kept kinds only), and the cards they draw.
    let kept: Vec<&str> = set.boosters.keys().map(String::as_str).collect();
    let (toppers, configs) = box_toppers(raw, &all.data, &kept);
    (set.box_toppers, set.topper_boosters) = (toppers, configs.into_iter().filter(|(_, cfg)| cfg.validate().is_ok()).collect());
    let added = topper_cards(&set, &all.data);
    set.cards.extend(added);

    validate_boosters(&mut set, sim_boosters, validated, dropped)?;
    // A box topper goes with its kind's boosters, and needs its packs' configs.
    set.box_toppers.retain(|kind, _| set.boosters.contains_key(kind));
    for topper in set.box_toppers.values_mut() {
        topper.packs.retain(|key, _| set.topper_boosters.contains_key(key));
    }
    set.box_toppers.retain(|_, topper| !topper.packs.is_empty());
    if set.boosters.is_empty() {
        return Err("every booster failed the simulation".into());
    }
    Ok(set)
}

/// Checks a set's booster configs by simulation, unless an identical config already passed
/// (`validated` maps config keys to fingerprints). A config that fails is dropped (and reported
/// in `dropped` as "CODE/kind").
fn validate_boosters(
    set: &mut SetFile,
    sim_boosters: u64,
    validated: &mut BTreeMap<String, String>,
    dropped: &mut BTreeMap<String, String>,
) -> Result<(), BuildError> {
    let colors = ColorIndex::from_cards(&set.cards);
    let mut failed = Vec::new();
    for (kind, cfg) in set.boosters.iter().chain(&set.topper_boosters) {
        let key = format!("{}/{kind}", set.code);
        let print = fingerprint(cfg, sim_boosters)?;
        if validated.get(&key) == Some(&print) {
            continue;
        }
        let report = simulate(cfg, &colors, sim_boosters, &mut ChaCha8Rng::seed_from_u64(1))?;
        let z = report.max_abs_z();
        if z > MAX_Z {
            failed.push(kind.clone());
            dropped.insert(key, format!("failed the simulation (max |z| = {z:.2})"));
        } else {
            validated.insert(key, print);
        }
    }
    for kind in failed {
        set.boosters.remove(&kind);
        set.topper_boosters.remove(&kind);
    }
    Ok(())
}

/// The cards of a set sold without boosters, whose booster the game crafts once prices are
/// known (`finish_crafted_set`).
fn crafted_cards(raw: &RawSet, version: &str, all: &AllPrintings) -> SetFile {
    let mut set = crafted_set(raw, version);
    merge_foil_printings(&mut set);
    if let Some(token_set) = all.data.get(&token_set_code(raw)) {
        add_tokens_from(&mut set, token_set);
    }
    merge_token_faces(&mut set);
    set
}

/// Crafts the booster of such a set from its cards' prices (see `crafted`), gives it a token,
/// and checks it by simulation like any other.
fn finish_crafted_set(
    set: &mut SetFile,
    recipe: &CraftedBooster,
    prices: &BTreeMap<String, PriceSet>,
    opts: &BuildOptions,
    validated: &mut BTreeMap<String, String>,
    dropped: &mut BTreeMap<String, String>,
) -> Result<(), BuildError> {
    craft_booster(set, recipe, prices)?;
    add_token_slots(set, opts.token_foil_one_in);
    validate_boosters(set, opts.sim_boosters, validated, dropped)?;
    if set.boosters.is_empty() {
        return Err("its crafted booster failed the simulation".into());
    }
    Ok(())
}

/// Bump whenever the draw or the expected rates change: every config is validated again.
const VALIDATION_VERSION: u32 = 3;

/// Identifies a booster config and the simulation it passed (FNV-1a over its JSON form).
fn fingerprint(cfg: &BoosterConfig, sim_boosters: u64) -> Result<String, BuildError> {
    let text = format!("{}|{sim_boosters}|{MAX_Z}|v{VALIDATION_VERSION}", serde_json::to_string(cfg)?);
    let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3));
    Ok(format!("{hash:016x}"))
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
struct ScryfallSetList {
    data: Vec<ScryfallSet>,
}

#[derive(Deserialize)]
struct ScryfallSet {
    code: String,
    icon_svg_uri: Option<String>,
}

/// Expansion symbol URL of every Scryfall set, by uppercase set code. The symbols are
/// cosmetic: if Scryfall cannot be reached, the sets simply show none.
fn set_icons() -> BTreeMap<String, String> {
    let list: Result<ScryfallSetList, BuildError> =
        http_get("https://api.scryfall.com/sets").and_then(|r| Ok(serde_json::from_reader(r.into_reader())?));
    match list {
        Ok(list) => list
            .data
            .into_iter()
            .filter_map(|s| Some((s.code.to_uppercase(), s.icon_svg_uri?)))
            .collect(),
        Err(e) => {
            eprintln!("set symbols unavailable: {e}");
            BTreeMap::new()
        }
    }
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

/// Runs the whole build, reporting its progress. The manifest is written last, so an
/// interrupted build never advertises missing files.
pub fn build(opts: &BuildOptions, progress: Progress<'_>) -> Result<Manifest, BuildError> {
    // An error says at which step it happened: "invalid handle (os error 6)" alone tells nothing.
    let mut step = String::new();
    let result = build_steps(opts, &mut |line, fraction| {
        if step != line {
            step = line.to_string();
        }
        progress(line, fraction)
    });
    result.map_err(|e| if step.is_empty() { e } else { format!("{step}: {e}").into() })
}

fn build_steps(opts: &BuildOptions, progress: Progress<'_>) -> Result<Manifest, BuildError> {
    let sets_dir = opts.out.join("sets");
    fs::create_dir_all(&opts.cache)?;
    fs::create_dir_all(&sets_dir)?;

    let kinds: Vec<&str> = opts.boosters.iter().map(String::as_str).collect();
    let mut manifest_sets = BTreeMap::new();
    let mut wanted = HashSet::new();
    let mut all_cards = Vec::new();
    let mut built_boosters = Vec::new();
    let mut built_sets = Vec::new();

    let all = read_mtgjson(&opts.cache, progress)?;
    let (codes, mut skipped) = select_sets(&all, opts, &kinds);
    let validated_path = opts.cache.join(VALIDATED_FILE);
    let mut validated: BTreeMap<String, String> =
        fs::read_to_string(&validated_path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();

    for (i, code) in codes.iter().enumerate() {
        let at = MTGJSON_READ_END + (SETS_END - MTGJSON_READ_END) * i as f64 / codes.len().max(1) as f64;
        progress(&format!("Preparing {} ({}/{})", all.data[code].name, i + 1, codes.len()), at);
        let set = match build_set(&all, code, &kinds, opts.sim_boosters, opts, &mut validated, &mut skipped) {
            Ok(set) => set,
            Err(e) => {
                eprintln!("{code} left out: {e}");
                skipped.insert(code.clone(), e.to_string());
                continue;
            }
        };
        for (kind, cfg) in &set.boosters {
            built_boosters.push((format!("{code}/{kind}"), cfg.clone()));
        }
        wanted.extend(set.cards.iter().flat_map(|c| [Some(c.scryfall_id.clone()), c.foil_scryfall_id.clone()]).flatten());
        all_cards.extend(set.cards.iter().cloned());
        write_json(&sets_dir.join(set_file_name(code)), &set)?;
        manifest_sets.insert(
            code.clone(),
            ManifestSet {
                name: set.name.clone(),
                names: set.names.clone(),
                release_date: set.release_date.clone(),
                source_version: set.source_version.clone(),
                boosters: set.boosters.keys().cloned().collect(),
                featured: None, // chosen once prices are known
                icon_svg_uri: None,
                box_toppers: set.box_toppers.clone(),
            },
        );
        built_sets.push(set);
    }
    // The sets sold without boosters (Secret Lair), for which the game crafts one: their cards
    // now, their booster once the prices are known. A set that has real boosters keeps them.
    let mut crafted = Vec::new();
    for (code, recipe) in &opts.crafted_boosters {
        if manifest_sets.contains_key(code) {
            continue;
        }
        let Some(raw) = all.data.get(code) else {
            skipped.insert(code.clone(), "unknown set".to_string());
            continue;
        };
        // Not left out for having no booster of a kind kept, then.
        skipped.remove(code);
        let set = crafted_cards(raw, &all.meta.version, &all);
        wanted.extend(set.cards.iter().flat_map(|c| [Some(c.scryfall_id.clone()), c.foil_scryfall_id.clone()]).flatten());
        all_cards.extend(set.cards.iter().cloned());
        crafted.push((set, recipe));
    }
    // The guest series, named while every set of MTGJSON is at hand.
    let mut guests = guest_index(&built_sets);
    for (code, series) in &mut guests.series {
        match all.data.get(code) {
            Some(raw) => {
                series.name = raw.name.clone();
                series.names = raw.names();
                series.release_date = raw.release_date.clone();
            }
            None => series.name = code.clone(),
        }
    }
    drop(all); // hundreds of MB, no longer needed
    if built_sets.is_empty() {
        return Err("no set could be built".into());
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
        let message = "Downloading Scryfall prices (large file)";
        progress(message, SETS_END);
        download(&bulk.jsonl_download_uri, &bulk_path, &mut |f| {
            progress(message, SETS_END + (BULK_END - SETS_END) * f)
        })?;
    }
    progress("Reading prices", BULK_END);
    let reader = BufReader::new(GzDecoder::new(BufReader::new(File::open(&bulk_path)?)));
    let by_scryfall_id = prices_from_scryfall_jsonl(reader, &wanted)?;
    let cards = prices_by_uuid(&by_scryfall_id, &all_cards);
    for (mut set, recipe) in crafted {
        let code = set.code.clone();
        if let Err(e) = finish_crafted_set(&mut set, recipe, &cards, opts, &mut validated, &mut skipped) {
            eprintln!("{code} left out: {e}");
            skipped.insert(code, e.to_string());
            continue;
        }
        for (kind, cfg) in &set.boosters {
            built_boosters.push((format!("{code}/{kind}"), cfg.clone()));
        }
        write_json(&sets_dir.join(set_file_name(&code)), &set)?;
        manifest_sets.insert(
            code,
            ManifestSet {
                name: set.name.clone(),
                names: set.names.clone(),
                release_date: set.release_date.clone(),
                source_version: set.source_version.clone(),
                boosters: set.boosters.keys().cloned().collect(),
                featured: None,
                icon_svg_uri: None,
                box_toppers: BTreeMap::new(),
            },
        );
        built_sets.push(set);
    }
    write_json(&validated_path, &validated)?;
    let icons = set_icons();
    for set in &built_sets {
        if let Some(entry) = manifest_sets.get_mut(&set.code) {
            entry.featured = featured_card(set, &cards);
            entry.icon_svg_uri = icons.get(&set.code).cloned();
        }
    }
    for (code, series) in &mut guests.series {
        series.icon_svg_uri = icons.get(code).cloned();
    }

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
    write_json(&opts.out.join(ARTISTS_FILE), &artist_index(&built_sets))?;
    write_json(&opts.out.join(GUESTS_FILE), &guests)?;

    progress("Saving the game data", 0.98);
    let generated_at = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_secs().to_string();
    let manifest = Manifest {
        generated_at,
        prices_updated_at: bulk.updated_at,
        sets: manifest_sets,
        booster_kinds: opts.boosters.clone(),
        requested_sets: Some(opts.sets.clone()),
        set_types: opts.set_types.clone(),
        skipped,
        format_version: DATA_FORMAT_VERSION,
    };
    write_json(&opts.out.join("manifest.json"), &manifest)?;

    // Older bulk files weigh hundreds of MB each, and per-set MTGJSON files are no longer
    // used (AllPrintings replaced them).
    for entry in fs::read_dir(&opts.cache)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let old_bulk = name.starts_with("default-cards-") && name != file_name;
        let per_set = name
            .strip_suffix(".json")
            .is_some_and(|stem| !stem.is_empty() && stem.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));
        if old_bulk || per_set {
            let _ = fs::remove_file(entry.path());
        }
    }
    // Sets no longer offered.
    for entry in fs::read_dir(&sets_dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".json") && set_code_of_file(&name).is_none_or(|code| !manifest.sets.contains_key(code)) {
            let _ = fs::remove_file(entry.path());
        }
    }
    Ok(manifest)
}
