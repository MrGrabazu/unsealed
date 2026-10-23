//! The game data in one file, ready made: built once a day by a GitHub Action and published
//! with the project's releases, so that a game downloads a few MB instead of building its data
//! itself from MTGJSON and Scryfall (hundreds of MB, minutes of work: too much for a phone).
//!
//! A pack is a gzip stream of records, one per file of the data directory: the file's path
//! (relative, with "/"), then its bytes, each preceded by its length. The gzip checksum tells a
//! damaged download. `manifest.json` is published beside it, for a game to tell whether the
//! pack is of use before downloading it, and is written last when a pack is installed.

use crate::builder::{download, http_get, BuildError, Progress};
use crate::catalog::{is_reserved, Manifest};
use crate::update::{pack_is_of_use, DataConfig};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

/// The data directory's file the others depend on: installed last.
pub const MANIFEST: &str = "manifest.json";
/// The name the pack is published under.
pub const PACK_FILE: &str = "unsealed-data.pack.gz";

/// The files of a data directory worth packing: its JSON files and its sets'.
fn data_files(dir: &Path) -> io::Result<Vec<String>> {
    let mut files = Vec::new();
    for (sub, prefix) in [(dir.to_path_buf(), ""), (dir.join("sets"), "sets/")] {
        for entry in fs::read_dir(&sub)?.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.file_type()?.is_file() && name.ends_with(".json") {
                files.push(format!("{prefix}{name}"));
            }
        }
    }
    files.sort();
    // The manifest last, as when it is installed.
    if let Some(i) = files.iter().position(|f| f == MANIFEST) {
        let manifest = files.remove(i);
        files.push(manifest);
    }
    Ok(files)
}

/// Writes the pack of a data directory; returns how many files it holds.
pub fn write_pack(dir: &Path, dest: &Path) -> io::Result<usize> {
    let files = data_files(dir)?;
    let mut out = GzEncoder::new(BufWriter::new(File::create(dest)?), Compression::best());
    for name in &files {
        let bytes = fs::read(dir.join(name))?;
        out.write_all(&(name.len() as u32).to_le_bytes())?;
        out.write_all(name.as_bytes())?;
        out.write_all(&(bytes.len() as u64).to_le_bytes())?;
        out.write_all(&bytes)?;
    }
    out.finish()?.flush()?;
    Ok(files.len())
}

/// Reads a pack's files, as (path, bytes), checking each path stays inside the directory.
pub fn read_pack(reader: impl Read) -> io::Result<Vec<(String, Vec<u8>)>> {
    let bad = |what: &str| io::Error::new(io::ErrorKind::InvalidData, format!("damaged data pack: {what}"));
    let mut input = GzDecoder::new(BufReader::new(reader));
    let mut files = Vec::new();
    loop {
        let mut len = [0u8; 4];
        match input.read_exact(&mut len) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        }
        let mut name = vec![0u8; u32::from_le_bytes(len) as usize];
        input.read_exact(&mut name)?;
        let name = String::from_utf8(name).map_err(|_| bad("a file name"))?;
        // Nor a name Windows reserves for a device ("sets/CON.json"): see `set_file_name`.
        let safe = !name.is_empty()
            && !name.starts_with('/')
            && name.split('/').all(|p| !p.is_empty() && p != "." && p != ".." && !is_reserved(p.split('.').next().unwrap_or(p)));
        if !safe || name.contains('\\') || name.contains(':') {
            return Err(bad(&format!("unsafe path {name}")));
        }
        let mut size = [0u8; 8];
        input.read_exact(&mut size)?;
        let size = u64::from_le_bytes(size);
        let mut bytes = Vec::with_capacity(size.min(256 << 20) as usize);
        (&mut input).take(size).read_to_end(&mut bytes)?;
        if bytes.len() as u64 != size {
            return Err(bad("a file cut short"));
        }
        files.push((name, bytes));
    }
    if !files.iter().any(|(name, _)| name == MANIFEST) {
        return Err(bad("no manifest"));
    }
    Ok(files)
}

/// Installs the published pack in `dir` when it is of use (`pack_is_of_use`): its manifest is
/// read first (a few KB), then the pack downloaded into `cache` and installed. Returns the
/// manifest installed, or None when the pack was of no use (the game builds its data itself).
pub fn fetch_pack(
    cfg: &DataConfig,
    local: Option<&Manifest>,
    now: i64,
    dir: &Path,
    cache: &Path,
    progress: Progress<'_>,
) -> Result<Option<Manifest>, BuildError> {
    let Some(base) = cfg.pack_url.as_deref().filter(|u| !u.is_empty()) else { return Ok(None) };
    let base = base.trim_end_matches('/');
    progress("Looking for the ready-made game data", 0.0);
    let remote: Manifest = serde_json::from_reader(http_get(&format!("{base}/{MANIFEST}"))?.into_reader())?;
    if !pack_is_of_use(&remote, local, cfg, now) {
        return Ok(None);
    }
    fs::create_dir_all(cache)?;
    let path = cache.join(PACK_FILE);
    let message = "Downloading the game data";
    download(&format!("{base}/{PACK_FILE}"), &path, &mut |f| progress(message, 0.9 * f))?;
    progress("Installing the game data", 0.95);
    let files = read_pack(File::open(&path)?)?;
    let manifest: Manifest = serde_json::from_slice(&files.iter().find(|(name, _)| name == MANIFEST).expect("checked by read_pack").1)?;
    install_pack(&files, dir)?;
    let _ = fs::remove_file(&path);
    Ok(Some(manifest))
}

/// Installs a pack's files in a data directory: each written beside its place then renamed
/// over it, the manifest last, so a game never sees a manifest announcing missing files; the
/// sets no longer in the pack are removed.
pub fn install_pack(files: &[(String, Vec<u8>)], dir: &Path) -> io::Result<()> {
    let target = |name: &str| -> PathBuf { name.split('/').fold(dir.to_path_buf(), |p, part| p.join(part)) };
    let (manifest, others): (Vec<_>, Vec<_>) = files.iter().partition(|(name, _)| name == MANIFEST);
    for (name, bytes) in others.iter().chain(manifest.iter()) {
        let path = target(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("incoming");
        fs::write(&tmp, bytes)?;
        fs::rename(&tmp, &path)?;
    }
    let sets: Vec<&str> = files.iter().filter_map(|(name, _)| name.strip_prefix("sets/")).collect();
    if let Ok(entries) = fs::read_dir(dir.join("sets")) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".json") && !sets.contains(&name.as_str()) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    Ok(())
}
