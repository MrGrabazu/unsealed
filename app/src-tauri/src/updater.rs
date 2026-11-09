//! Background data refresh. At launch a thread checks the age of the local data and, when it is
//! too old (or incomplete), installs the ready-made data published every day (`pack`), or else
//! rebuilds it from MTGJSON and Scryfall. The game stays playable on the previous data
//! meanwhile; the new files are swapped in atomically, the manifest last.

use crate::Session;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use unsealed_core::builder::{build, BuildOptions};
use unsealed_core::catalog::Manifest;
use unsealed_core::pack::{fetch_pack, PackFetch};
use unsealed_core::update::{refresh_reason, DataConfig, RefreshReason};

/// Event name the frontend listens to.
pub const STATUS_EVENT: &str = "data-status";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum DataStatus {
    /// The check has not finished yet.
    Checking,
    UpToDate,
    /// `percent` is an estimate of the overall progress (see `builder::Progress`).
    Updating { message: String, percent: u8 },
    /// The refresh failed (offline, source down...). The game keeps using the data it has.
    Failed { message: String },
}

pub type SharedStatus = Arc<Mutex<DataStatus>>;

/// What a refresh needs, kept to start one again after a failure (`retry`).
pub struct Refresh {
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub cfg: DataConfig,
}

fn set_status(app: &AppHandle, shared: &SharedStatus, status: DataStatus) {
    if let Ok(mut current) = shared.lock() {
        *current = status.clone();
    }
    let _ = app.emit(STATUS_EVENT, status);
}

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn read_manifest(data_dir: &Path) -> Option<Manifest> {
    serde_json::from_str(&fs::read_to_string(data_dir.join("manifest.json")).ok()?).ok()
}

/// Drops what the game cached from the old files (taking its lock only now, not during the
/// refresh).
fn reload_session(app: &AppHandle) {
    if let Some(session) = app.try_state::<Mutex<Session>>() {
        if let Ok(mut session) = session.lock() {
            session.reload_data();
        }
    }
}

/// Starts the refresh again after a failure (offline at the first launch…). Does nothing
/// while one is running or when the data is up to date.
pub fn retry(app: AppHandle, status: SharedStatus, refresh: Arc<Refresh>) {
    let failed = status.lock().is_ok_and(|s| matches!(*s, DataStatus::Failed { .. }));
    if failed {
        set_status(&app, &status, DataStatus::Checking);
        spawn(app, status, refresh);
    }
}

/// Lowers the priority of the calling thread: a local build keeps a core busy for minutes
/// while the player plays, and the game's own threads should come first. (Windows only: on
/// macOS, `nice` would lower the whole app.)
#[cfg(windows)]
fn lower_priority() {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentThread() -> *mut std::ffi::c_void;
        fn SetThreadPriority(thread: *mut std::ffi::c_void, priority: i32) -> i32;
    }
    const THREAD_PRIORITY_BELOW_NORMAL: i32 = -1;
    // SAFETY: the pseudo-handle of the current thread is always valid.
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
    }
}

#[cfg(not(windows))]
fn lower_priority() {}

/// Starts the refresh check on its own thread. Never blocks the UI and never panics the app.
pub fn spawn(app: AppHandle, status: SharedStatus, refresh: Arc<Refresh>) {
    std::thread::spawn(move || {
        lower_priority();
        // The published pack is the data installed: looked for again later, the game playing
        // on its data meanwhile.
        while refresh_once(&app, &status, &refresh) {
            std::thread::sleep(PACK_RETRY);
        }
    });
}

/// How long the game waits before looking for a newer pack, when the one published is the
/// data installed (built twice a day: it comes within hours).
const PACK_RETRY: std::time::Duration = std::time::Duration::from_secs(3600);

/// Brings the data up to date: from the published pack, else built here. True when the data is
/// stale but the pack published is already installed: to look again later.
fn refresh_once(app: &AppHandle, status: &SharedStatus, refresh: &Refresh) -> bool {
    {
        let Refresh { data_dir, cache_dir, cfg } = refresh;
        let local = read_manifest(data_dir);
        let Some(reason) = refresh_reason(local.as_ref(), cfg, now_secs()) else {
            set_status(app, status, DataStatus::UpToDate);
            return false;
        };
        eprintln!("data refresh needed: {reason:?}");
        // Data only stale waits quietly for the pack: no progress bar for a look at its manifest.
        if reason != RefreshReason::Stale {
            set_status(app, status, DataStatus::Updating { message: "Starting".to_string(), percent: 0 });
        }
        let mut last: Option<DataStatus> = None;
        let mut report = |line: &str, fraction: f64| {
            let percent = (fraction.clamp(0.0, 1.0) * 100.0).round() as u8;
            let next = DataStatus::Updating { message: line.to_string(), percent };
            // Downloads report every percent of each file: only forward actual changes. The
            // look for the pack says nothing while the data is only stale.
            if last.as_ref() != Some(&next) && (reason != RefreshReason::Stale || fraction > 0.0) {
                set_status(app, status, next.clone());
                last = Some(next);
            }
        };

        // The ready-made data first (a few MB, published twice a day); building it here, from
        // MTGJSON and Scryfall, is the fallback: when no pack of use can be had. A pack that is
        // the data installed, or itself over a day old once installed (the daily build failed),
        // is waited on: building here takes minutes of a core for prices hours newer at best.
        match fetch_pack(cfg, local.as_ref(), now_secs(), data_dir, cache_dir, &mut report) {
            Ok(PackFetch::Installed(manifest)) => {
                reload_session(app);
                set_status(app, status, DataStatus::UpToDate);
                return refresh_reason(Some(&manifest), cfg, now_secs()).is_some();
            }
            Ok(PackFetch::Unchanged) => {
                eprintln!("data stale, the pack published is installed: waiting for the next one");
                set_status(app, status, DataStatus::UpToDate);
                return true;
            }
            Ok(PackFetch::Unusable) => {}
            Err(e) => eprintln!("ready-made data unavailable: {e}"),
        }
        set_status(app, status, DataStatus::Updating { message: "Starting".to_string(), percent: 0 });

        let opts = BuildOptions {
            sets: cfg.sets.clone(),
            set_types: cfg.set_types.clone(),
            boosters: cfg.boosters.clone(),
            out: data_dir.clone(),
            cache: cache_dir.clone(),
            sim_boosters: cfg.sim_boosters,
            token_foil_one_in: cfg.token_foil_one_in,
            art_cards: cfg.art_cards.clone(),
            crafted_boosters: cfg.crafted_boosters.clone(),
            added_masterpieces: cfg.added_masterpieces.clone(),
        };
        let result = build(&opts, &mut report);
        match result {
            Ok(_) => {
                reload_session(app);
                set_status(app, status, DataStatus::UpToDate);
            }
            Err(e) => {
                eprintln!("data refresh failed: {e}");
                set_status(app, status, DataStatus::Failed { message: e.to_string() });
            }
        }
        false
    }
}
