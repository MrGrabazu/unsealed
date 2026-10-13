//! Tauri shell: exposes the game logic (see `game`) to the React frontend.
//! The frontend only displays; every draw happens in Rust.

mod game;
mod store;

use game::{CollectionView, Game, OpenedCard, SetSummary};
use std::error::Error;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use store::Store;
use tauri::{Manager, State};

type GameState = Mutex<Game>;

fn locked<'a>(state: &'a State<'_, GameState>) -> Result<std::sync::MutexGuard<'a, Game>, String> {
    state.lock().map_err(|_| "game state is poisoned".to_string())
}

#[tauri::command]
fn list_sets(state: State<'_, GameState>) -> Result<Vec<SetSummary>, String> {
    locked(&state)?.list_sets()
}

#[tauri::command]
fn open_booster(
    state: State<'_, GameState>,
    set_code: String,
    kind: String,
) -> Result<Vec<OpenedCard>, String> {
    locked(&state)?.open_booster(&set_code, &kind)
}

#[tauri::command]
fn get_collection(state: State<'_, GameState>, set_code: String) -> Result<CollectionView, String> {
    locked(&state)?.collection(&set_code)
}

/// Where the player's save lives. `UNSEALED_DB` overrides everything; debug builds
/// use the repository's `.dev-data/` so development never touches a real save.
fn resolve_db_path(app: &tauri::AppHandle) -> Result<PathBuf, Box<dyn Error>> {
    let path = if let Ok(path) = std::env::var("UNSEALED_DB") {
        PathBuf::from(path)
    } else if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.dev-data/unsealed.sqlite")
    } else {
        app.path().app_data_dir()?.join("unsealed.sqlite")
    };
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    Ok(path)
}

/// Where the downloaded data lives. `UNSEALED_DATA_DIR` overrides everything; debug
/// builds default to the repository's `dist/` (output of `data-builder`).
fn resolve_data_dir(app: &tauri::AppHandle) -> Result<PathBuf, Box<dyn Error>> {
    if let Ok(dir) = std::env::var("UNSEALED_DATA_DIR") {
        return Ok(dir.into());
    }
    if cfg!(debug_assertions) {
        return Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist"));
    }
    Ok(app.path().app_data_dir()?.join("data"))
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = resolve_data_dir(app.handle())?;
            let store = Store::open(&resolve_db_path(app.handle())?)?;
            app.manage(Mutex::new(Game::new(data_dir, store)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![list_sets, open_booster, get_collection])
        .run(tauri::generate_context!())
        .expect("error while running Unsealed");
}
