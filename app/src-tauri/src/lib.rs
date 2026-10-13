//! Tauri shell: exposes the game logic (see `game`) to the React frontend.
//! The frontend only displays; every draw happens in Rust.

mod game;

use game::{Game, OpenedCard, SetSummary};
use std::error::Error;
use std::path::PathBuf;
use std::sync::Mutex;
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
            app.manage(Mutex::new(Game::new(data_dir)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![list_sets, open_booster])
        .run(tauri::generate_context!())
        .expect("error while running Unsealed");
}
