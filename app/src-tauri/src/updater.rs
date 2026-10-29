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
use unsealed_core::pack::fetch_pack;
use unsealed_core::update::{refresh_reason, DataConfig};

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

/// Starts the refresh check on its own thread. Never blocks the UI and never panics the app.
pub fn spawn(app: AppHandle, status: SharedStatus, refresh: Arc<Refresh>) {
    std::thread::spawn(move || {
        let Refresh { data_dir, cache_dir, cfg } = &*refresh;
        let local = read_manifest(data_dir);
        let Some(reason) = refresh_reason(local.as_ref(), cfg, now_secs()) else {
            set_status(&app, &status, DataStatus::UpToDate);
            return;
        };
        eprintln!("data refresh needed: {reason:?}");
        set_status(&app, &status, DataStatus::Updating { message: "Starting".to_string(), percent: 0 });
        let mut last: Option<DataStatus> = None;
        let mut report = |line: &str, fraction: f64| {
            let percent = (fraction.clamp(0.0, 1.0) * 100.0).round() as u8;
            let next = DataStatus::Updating { message: line.to_string(), percent };
            // Downloads report every percent of each file: only forward actual changes.
            if last.as_ref() != Some(&next) {
                set_status(&app, &status, next.clone());
                last = Some(next);
            }
        };

        // The ready-made data first (a few MB, published every day); building it here, from
        // MTGJSON and Scryfall, is the fallback: when the pack is of no use or cannot be had.
        match fetch_pack(cfg, local.as_ref(), now_secs(), data_dir, cache_dir, &mut report) {
            Ok(Some(manifest)) if refresh_reason(Some(&manifest), cfg, now_secs()).is_none() => {
                reload_session(&app);
                set_status(&app, &status, DataStatus::UpToDate);
                return;
            }
            // Installed but already a day old (the daily build failed): built here as well.
            Ok(Some(_)) => reload_session(&app),
            Ok(None) => {}
            Err(e) => eprintln!("ready-made data unavailable: {e}"),
        }

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
        };
        let result = build(&opts, &mut report);
        match result {
            Ok(_) => {
                reload_session(&app);
                set_status(&app, &status, DataStatus::UpToDate);
            }
            Err(e) => {
                eprintln!("data refresh failed: {e}");
                set_status(&app, &status, DataStatus::Failed { message: e.to_string() });
            }
        }
    });
}
