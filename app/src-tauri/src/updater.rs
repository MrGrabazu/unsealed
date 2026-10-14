//! Background data refresh. At launch a thread checks the age of the local data and, when it is
//! too old (or incomplete), rebuilds it from MTGJSON and Scryfall. The game stays playable on the
//! previous data meanwhile; the new files are swapped in atomically by the builder.

use crate::Session;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};
use unsealed_core::builder::{build, BuildOptions};
use unsealed_core::catalog::Manifest;
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

/// Starts the refresh check on its own thread. Never blocks the UI and never panics the app.
pub fn spawn(app: AppHandle, status: SharedStatus, data_dir: PathBuf, cache_dir: PathBuf, cfg: DataConfig) {
    std::thread::spawn(move || {
        let local = read_manifest(&data_dir);
        let Some(reason) = refresh_reason(local.as_ref(), &cfg, now_secs()) else {
            set_status(&app, &status, DataStatus::UpToDate);
            return;
        };
        eprintln!("data refresh needed: {reason:?}");
        set_status(&app, &status, DataStatus::Updating { message: "Starting".to_string(), percent: 0 });

        let opts = BuildOptions {
            sets: cfg.sets.clone(),
            set_types: cfg.set_types.clone(),
            boosters: cfg.boosters.clone(),
            out: data_dir,
            cache: cache_dir,
            sim_boosters: cfg.sim_boosters,
            token_foil_one_in: cfg.token_foil_one_in,
        };
        let mut last: Option<DataStatus> = None;
        let result = build(&opts, &mut |line, fraction| {
            let percent = (fraction.clamp(0.0, 1.0) * 100.0).round() as u8;
            let next = DataStatus::Updating { message: line.to_string(), percent };
            // Downloads report every percent of each file: only forward actual changes.
            if last.as_ref() != Some(&next) {
                set_status(&app, &status, next.clone());
                last = Some(next);
            }
        });
        match result {
            Ok(_) => {
                // Drop what the game cached from the old files, without holding its lock during the build.
                if let Some(session) = app.try_state::<Mutex<Session>>() {
                    if let Ok(mut session) = session.lock() {
                        session.reload_data();
                    }
                }
                set_status(&app, &status, DataStatus::UpToDate);
            }
            Err(e) => {
                eprintln!("data refresh failed: {e}");
                set_status(&app, &status, DataStatus::Failed { message: e.to_string() });
            }
        }
    });
}
