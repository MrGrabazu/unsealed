//! Tauri shell: exposes the game logic (see `game`) to the React frontend.
//! The frontend only displays; every draw, price and payment is computed in Rust.

mod game;
mod store;

use game::{CollectionView, Game, OpenedCard, SaleRequest, SaleResult, SetSummary, ShopItem, WalletView};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use store::Store;
use tauri::{Manager, State};
use unsealed_core::economy::EconomyConfig;

type GameState = Mutex<Game>;

/// Economy settings shipped with the app. A `economy.json` in the data directory
/// overrides them (this is how published balance changes will reach players).
const DEFAULT_ECONOMY: &str = include_str!("../../../config/economy.json");

fn locked<'a>(state: &'a State<'_, GameState>) -> Result<std::sync::MutexGuard<'a, Game>, String> {
    state.lock().map_err(|_| "game state is poisoned".to_string())
}

#[tauri::command]
fn list_sets(state: State<'_, GameState>) -> Result<Vec<SetSummary>, String> {
    locked(&state)?.list_sets()
}

#[tauri::command]
fn get_wallet(state: State<'_, GameState>) -> Result<WalletView, String> {
    locked(&state)?.wallet()
}

#[tauri::command]
fn get_shop(state: State<'_, GameState>) -> Result<Vec<ShopItem>, String> {
    locked(&state)?.shop()
}

#[tauri::command]
fn buy_boosters(
    state: State<'_, GameState>,
    set_code: String,
    kind: String,
    quantity: u32,
) -> Result<(), String> {
    locked(&state)?.buy_boosters(&set_code, &kind, quantity)
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

#[tauri::command]
fn sell_cards(state: State<'_, GameState>, requests: Vec<SaleRequest>) -> Result<SaleResult, String> {
    locked(&state)?.sell_cards(&requests)
}

#[tauri::command]
fn sell_duplicates(state: State<'_, GameState>, set_code: String) -> Result<SaleResult, String> {
    locked(&state)?.sell_duplicates(&set_code)
}

#[tauri::command]
fn claim_charity(state: State<'_, GameState>) -> Result<(), String> {
    locked(&state)?.claim_charity()
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

fn load_economy(data_dir: &Path) -> Result<EconomyConfig, Box<dyn Error>> {
    let json = fs::read_to_string(data_dir.join("economy.json")).unwrap_or_else(|_| DEFAULT_ECONOMY.to_string());
    Ok(EconomyConfig::from_json(&json)?)
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = resolve_data_dir(app.handle())?;
            let economy = load_economy(&data_dir)?;
            let store = Store::open(&resolve_db_path(app.handle())?)?;
            app.manage(Mutex::new(Game::new(data_dir, store, economy)?));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_sets,
            get_wallet,
            get_shop,
            buy_boosters,
            open_booster,
            get_collection,
            sell_cards,
            sell_duplicates,
            claim_charity,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Unsealed");
}
