//! Tauri shell: exposes the game logic (see `game`) to the React frontend.
//! The frontend only displays; every draw, price and payment is computed in Rust.

mod game;
mod saves;
mod store;
mod updater;

use game::{
    AchievementTexts, AchievementView, AchievementsView, BoosterSaleResult, CardDetailsView, CollectionView, DataInfo,
    Game, HistoryView, SaleRequest, SaleResult, SetSummary, ShopItem, StatsView, WalletView,
};
use saves::{SaveInfo, Saves};
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Manager, State};
use unsealed_core::achievements::Engine;
use unsealed_core::artifacts::{ArtifactTranslations, ArtifactsConfig};
use unsealed_core::cycles::Cycles;
use unsealed_core::economy::EconomyConfig;
use unsealed_core::limited::LimitedConfig;
use unsealed_core::scryfall;
use unsealed_core::update::DataConfig;
use updater::{DataStatus, Refresh, SharedStatus};

/// Economy settings shipped with the app. A `economy.json` in the data directory
/// overrides them (this is how published balance changes will reach players).
const DEFAULT_ECONOMY: &str = include_str!("../../../config/economy.json");

/// Achievement definitions shipped with the app, overridable the same way.
const DEFAULT_ACHIEVEMENTS: &str = include_str!("../../../config/achievements.json");

/// Which sets to download and how fresh they must be, overridable the same way.
const DEFAULT_DATA: &str = include_str!("../../../config/data.json");

/// Print runs of the cards that exist in few copies (serialized), overridable the same way.
const DEFAULT_LIMITED: &str = include_str!("../../../config/limited.json");

/// Translations of the achievements (English lives in the definitions), overridable the
/// same way with `achievements.<lang>.json`.
const ACHIEVEMENT_TRANSLATIONS: &[(&str, &str)] = &[
    ("fr", include_str!("../../../config/achievements.fr.json")),
    ("de", include_str!("../../../config/achievements.de.json")),
    ("es", include_str!("../../../config/achievements.es.json")),
    ("it", include_str!("../../../config/achievements.it.json")),
];

/// The artifacts (`artifacts.json`) and their translations (`artifacts.<lang>.json`),
/// overridable the same way.
const DEFAULT_ARTIFACTS: &str = include_str!("../../../config/artifacts.json");
const ARTIFACT_TRANSLATIONS: &[(&str, &str)] = &[
    ("fr", include_str!("../../../config/artifacts.fr.json")),
    ("de", include_str!("../../../config/artifacts.de.json")),
    ("es", include_str!("../../../config/artifacts.es.json")),
    ("it", include_str!("../../../config/artifacts.it.json")),
];

const DEFAULT_CYCLES: &str = include_str!("../../../config/cycles.json");
const DEFAULT_CUSTOM_CYCLES: &str = include_str!("../../../config/cycles.custom.json");
const CYCLE_TRANSLATIONS: &[(&str, &str)] = &[
    ("fr", include_str!("../../../config/cycles.fr.json")),
    ("de", include_str!("../../../config/cycles.de.json")),
    ("es", include_str!("../../../config/cycles.es.json")),
    ("it", include_str!("../../../config/cycles.it.json")),
];

/// What is running: the saves, and the game being played, if any (none on the home screen
/// at launch).
pub(crate) struct Session {
    data_dir: PathBuf,
    saves: Saves,
    current: Option<(SaveInfo, Game)>,
    /// Language of card and set names (a setting of the app, not of a game).
    card_lang: String,
    /// The sets' cycles, read once and shared by the games.
    cycles: Arc<Cycles>,
}

impl Session {
    /// Called after a background data refresh, when a game is open.
    pub(crate) fn reload_data(&mut self) {
        if let Some((_, game)) = self.current.as_mut() {
            game.reload_data();
        }
    }

    fn play(&mut self, id: &str) -> Result<SaveInfo, String> {
        let (info, store) = self.saves.open(id, now())?;
        let economy = load_economy(&self.data_dir).map_err(|e| e.to_string())?;
        let engine = load_achievements(&self.data_dir).map_err(|e| e.to_string())?;
        let mut game = Game::new(self.data_dir.clone(), store, economy, engine)?;
        game.set_achievement_texts(load_achievement_texts(&self.data_dir).map_err(|e| e.to_string())?);
        game.set_limited(load_limited(&self.data_dir).map_err(|e| e.to_string())?);
        game.set_artifacts(load_artifacts(&self.data_dir).map_err(|e| e.to_string())?);
        game.set_artifact_texts(load_artifact_texts(&self.data_dir).map_err(|e| e.to_string())?);
        game.set_cycles(self.cycles.clone());
        game.set_card_language(&self.card_lang);
        self.current = Some((info.clone(), game));
        Ok(info)
    }
}

type SessionState = Mutex<Session>;

/// Where the display chosen (full screen or a window) is kept, for the window to open in it.
fn display_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("display.json"))
}

/// Whether the window opens in full screen: yes unless the player chose a window.
fn display_fullscreen(app: &tauri::AppHandle) -> bool {
    display_path(app)
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|v| v["fullscreen"].as_bool())
        .unwrap_or(true)
}

/// Remembers the display chosen, for the next launch.
#[tauri::command]
fn set_display_fullscreen(app: tauri::AppHandle, on: bool) -> Result<(), String> {
    let path = display_path(&app).ok_or("no configuration directory")?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    fs::write(path, serde_json::json!({ "fullscreen": on }).to_string()).map_err(|e| e.to_string())
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn session<'a>(state: &'a State<'_, SessionState>) -> Result<std::sync::MutexGuard<'a, Session>, String> {
    state.lock().map_err(|_| "game state is poisoned".to_string())
}

/// Runs `f` on the game being played.
fn with_game<T>(state: &State<'_, SessionState>, f: impl FnOnce(&mut Game) -> Result<T, String>) -> Result<T, String> {
    let mut session = session(state)?;
    let (_, game) = session.current.as_mut().ok_or("no game is loaded")?;
    f(game)
}

// ---- games and saves ----------------------------------------------------------------

#[tauri::command]
fn list_saves(state: State<'_, SessionState>) -> Result<Vec<SaveInfo>, String> {
    Ok(session(&state)?.saves.list())
}

/// The game being played, if any.
#[tauri::command]
fn current_save(state: State<'_, SessionState>) -> Result<Option<SaveInfo>, String> {
    Ok(session(&state)?.current.as_ref().map(|(info, _)| info.clone()))
}

#[tauri::command]
fn new_game(state: State<'_, SessionState>, name: String) -> Result<SaveInfo, String> {
    let mut session = session(&state)?;
    let info = session.saves.create(&name, now())?;
    session.play(&info.id)
}

#[tauri::command]
fn load_game(state: State<'_, SessionState>, id: String) -> Result<SaveInfo, String> {
    session(&state)?.play(&id)
}

/// Manual save: copies the game being played into a new save point.
#[tauri::command]
fn create_save_point(state: State<'_, SessionState>, name: String) -> Result<SaveInfo, String> {
    let session = session(&state)?;
    let (info, _) = session.current.as_ref().ok_or("no game is loaded")?;
    session.saves.save_point(&info.id, &name, now())
}

#[tauri::command]
fn rename_save(state: State<'_, SessionState>, id: String, name: String) -> Result<SaveInfo, String> {
    let mut session = session(&state)?;
    let info = session.saves.rename(&id, &name)?;
    if let Some((current, _)) = session.current.as_mut().filter(|(c, _)| c.id == id) {
        current.name = info.name.clone();
    }
    Ok(info)
}

/// Deletes a save for good. Deleting the game being played closes it first.
#[tauri::command]
fn delete_save(state: State<'_, SessionState>, id: String) -> Result<(), String> {
    let mut session = session(&state)?;
    if session.current.as_ref().is_some_and(|(c, _)| c.id == id) {
        session.current = None;
    }
    session.saves.delete(&id)
}

// ---- data -------------------------------------------------------------------------

#[tauri::command]
fn get_data_status(status: State<'_, SharedStatus>) -> DataStatus {
    status.lock().map(|s| s.clone()).unwrap_or(DataStatus::Checking)
}

/// Prepares the data again after a failure (the player was offline…).
#[tauri::command]
fn retry_data_update(app: tauri::AppHandle, status: State<'_, SharedStatus>, refresh: State<'_, Arc<Refresh>>) {
    updater::retry(app, status.inner().clone(), refresh.inner().clone());
}

#[tauri::command]
fn get_data_info(state: State<'_, SessionState>) -> Result<DataInfo, String> {
    game::data_info(&session(&state)?.data_dir)
}

#[tauri::command]
fn list_sets(state: State<'_, SessionState>) -> Result<Vec<SetSummary>, String> {
    let session = session(&state)?;
    game::read_sets(&session.data_dir, &session.card_lang, &game::system_clock().day)
}

/// Language of card names, card images and set names. Unknown languages mean English.
#[tauri::command]
fn set_card_language(state: State<'_, SessionState>, lang: String) -> Result<(), String> {
    let mut session = session(&state)?;
    session.card_lang = if game::is_card_language(&lang) { lang } else { "en".to_string() };
    let lang = session.card_lang.clone();
    if let Some((_, game)) = session.current.as_mut() {
        game.set_card_language(&lang);
    }
    Ok(())
}

// ---- the game being played ----------------------------------------------------------

#[tauri::command]
fn get_wallet(state: State<'_, SessionState>) -> Result<WalletView, String> {
    with_game(&state, |g| g.wallet())
}

#[tauri::command]
fn get_shop(state: State<'_, SessionState>) -> Result<Vec<ShopItem>, String> {
    with_game(&state, |g| g.shop())
}

#[tauri::command]
fn sell_boosters(
    state: State<'_, SessionState>,
    set_code: String,
    kind: String,
    quantity: u32,
) -> Result<BoosterSaleResult, String> {
    with_game(&state, |g| g.sell_boosters(&set_code, &kind, quantity))
}

#[tauri::command]
fn buy_boosters(state: State<'_, SessionState>, set_code: String, kind: String, quantity: u32) -> Result<(), String> {
    with_game(&state, |g| g.buy_boosters(&set_code, &kind, quantity))
}

/// `animated`: the player watches the opening (false for a quick opening).
#[tauri::command]
fn open_booster(
    state: State<'_, SessionState>,
    set_code: String,
    kind: String,
    animated: bool,
    bets: Vec<game::BetRequest>,
) -> Result<game::OpenedBooster, String> {
    with_game(&state, |g| g.open_booster_with_bets(&set_code, &kind, animated, &bets))
}

#[tauri::command]
fn open_boosters(
    state: State<'_, SessionState>,
    set_code: String,
    kind: String,
    quantity: u32,
    bets: Vec<game::BetRequest>,
) -> Result<Vec<game::OpenedBooster>, String> {
    with_game(&state, |g| g.open_boosters_with_bets(&set_code, &kind, quantity, &bets))
}

/// The sets pinned at the top of the Boosterium.
#[tauri::command]
fn get_pinned_sets(state: State<'_, SessionState>) -> Result<Vec<String>, String> {
    with_game(&state, |g| g.pinned_sets())
}

#[tauri::command]
fn set_pinned_sets(state: State<'_, SessionState>, codes: Vec<String>) -> Result<(), String> {
    with_game(&state, |g| g.set_pinned_sets(&codes))
}

/// Pays what the collection earned since the last call.
#[tauri::command]
fn collect_income(state: State<'_, SessionState>) -> Result<game::IncomeView, String> {
    with_game(&state, |g| g.collect_income())
}

/// What a bet slip needs to know of a booster (its price, the house's margin, the minimum stake).
#[tauri::command]
fn get_bet_context(state: State<'_, SessionState>, set_code: String, kind: String) -> Result<game::BetContext, String> {
    with_game(&state, |g| g.bet_context(&set_code, &kind))
}

/// How likely a bet is on a booster of this kind.
#[tauri::command]
fn get_bet_chance(
    state: State<'_, SessionState>,
    set_code: String,
    kind: String,
    bet: unsealed_core::betting::Bet,
) -> Result<game::BetChance, String> {
    with_game(&state, |g| g.bet_chance(&set_code, &kind, &bet))
}

/// The bets kept from one opening to the next, with the game.
#[tauri::command]
fn get_kept_bets(state: State<'_, SessionState>) -> Result<Vec<game::KeptBet>, String> {
    with_game(&state, |g| g.kept_bets())
}

#[tauri::command]
fn set_kept_bets(state: State<'_, SessionState>, bets: Vec<game::KeptBet>) -> Result<(), String> {
    with_game(&state, |g| g.set_kept_bets(&bets))
}

/// The player turned over `count` cards of the booster being opened: their achievements
/// happen now (those of the whole booster with the last card).
#[tauri::command]
fn reveal_cards(state: State<'_, SessionState>, count: usize) -> Result<(), String> {
    with_game(&state, |g| {
        g.reveal_cards(count);
        Ok(())
    })
}

#[tauri::command]
fn get_collection(state: State<'_, SessionState>, set_code: String) -> Result<CollectionView, String> {
    with_game(&state, |g| g.collection(&set_code))
}

/// Asks Scryfall which translated printings of a set have a good image (many have only a
/// small scan, or none), so the game shows those and the English image of the others.
/// Returns how many good ones were found; 0 in English or when all are known already.
#[tauri::command]
async fn check_set_images(
    state: State<'_, SessionState>,
    scryfall: State<'_, Arc<scryfall::Client>>,
    set_code: String,
) -> Result<usize, String> {
    let ids = with_game(&state, |g| g.unchecked_images(&set_code))?;
    if ids.is_empty() {
        return Ok(0);
    }
    let client = scryfall.inner().clone();
    let asked = ids.clone();
    let statuses = tauri::async_runtime::spawn_blocking(move || client.image_statuses(&asked))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    with_game(&state, |g| Ok(g.record_image_quality(&ids, &statuses)))
}

/// Every card owned, across the sets (the collection's "all sets" list).
#[tauri::command]
fn get_owned_cards(state: State<'_, SessionState>) -> Result<Vec<game::ArtistCardView>, String> {
    with_game(&state, |g| g.owned_everywhere())
}

/// The library's binders: one per set the player owns cards of.
#[tauri::command]
fn get_library(state: State<'_, SessionState>) -> Result<Vec<game::LibraryBinder>, String> {
    with_game(&state, |g| g.library())
}

#[tauri::command]
fn list_artists(state: State<'_, SessionState>) -> Result<Vec<game::ArtistSummary>, String> {
    with_game(&state, |g| g.artists())
}

/// The guest series with a binder of their own (Special Guests, The List…).
#[tauri::command]
fn list_guest_series(state: State<'_, SessionState>) -> Result<Vec<game::SetSummary>, String> {
    with_game(&state, |g| Ok(g.guest_series()))
}

#[tauri::command]
fn get_artist_collection(state: State<'_, SessionState>, name: String) -> Result<game::ArtistView, String> {
    with_game(&state, |g| g.artist_collection(&name))
}

#[tauri::command]
fn sell_cards(state: State<'_, SessionState>, requests: Vec<SaleRequest>) -> Result<SaleResult, String> {
    with_game(&state, |g| g.sell_cards(&requests))
}

#[tauri::command]
fn sell_duplicates(state: State<'_, SessionState>, set_code: String) -> Result<SaleResult, String> {
    with_game(&state, |g| g.sell_duplicates(&set_code))
}

#[tauri::command]
fn claim_charity(state: State<'_, SessionState>) -> Result<(), String> {
    with_game(&state, |g| g.claim_charity())
}

/// Every achievement, with its texts in `lang` (a language code such as "fr").
#[tauri::command]
fn get_achievements(state: State<'_, SessionState>, lang: String) -> Result<AchievementsView, String> {
    with_game(&state, |g| g.achievements(&lang))
}

/// Achievements unlocked since the last call, with their texts in `lang`.
#[tauri::command]
fn take_unlocks(state: State<'_, SessionState>, lang: String) -> Result<Vec<AchievementView>, String> {
    with_game(&state, |g| Ok(g.take_unlocks(&lang)))
}

/// A set's cycles, as far as the player is, their names in `lang`.
#[tauri::command]
fn get_cycles(state: State<'_, SessionState>, set_code: String, lang: String) -> Result<game::CyclesView, String> {
    with_game(&state, |g| g.cycles(&set_code, &lang))
}

/// The cabinet of curiosities: every artifact, owned, on sale or still hidden.
#[tauri::command]
fn get_artifacts(state: State<'_, SessionState>, lang: String) -> Result<Vec<game::ArtifactView>, String> {
    with_game(&state, |g| Ok(g.artifacts(&lang)))
}

#[tauri::command]
fn buy_artifact(state: State<'_, SessionState>, id: String, lang: String) -> Result<game::ArtifactView, String> {
    with_game(&state, |g| g.buy_artifact(&id, &lang))
}

/// Artifacts given since the last call (by achievements), to celebrate.
#[tauri::command]
fn take_given_artifacts(state: State<'_, SessionState>, lang: String) -> Result<Vec<game::ArtifactView>, String> {
    with_game(&state, |g| Ok(g.take_given_artifacts(&lang)))
}

#[tauri::command]
fn get_stats(state: State<'_, SessionState>) -> Result<StatsView, String> {
    with_game(&state, |g| g.stats())
}

/// The wallet's movements, newest first (`limit` of them, after `offset`).
#[tauri::command]
fn get_history(state: State<'_, SessionState>, limit: u32, offset: u32) -> Result<HistoryView, String> {
    with_game(&state, |g| g.history(limit.min(200), offset))
}

/// Undoes a sale (for a penalty) or a recent purchase; returns the money moved (signed).
#[tauri::command]
fn undo_transaction(state: State<'_, SessionState>, id: i64) -> Result<i64, String> {
    with_game(&state, |g| g.undo_transaction(id))
}

/// A card's details. Scryfall is asked on a worker thread, without holding the session:
/// offline, the local part comes back alone (`online` is false).
#[tauri::command]
async fn get_card_details(
    state: State<'_, SessionState>,
    scryfall: State<'_, Arc<scryfall::Client>>,
    set_code: String,
    uuid: String,
) -> Result<CardDetailsView, String> {
    let (mut view, query) = with_game(&state, |g| g.card_details_local(&set_code, &uuid))?;
    let client = scryfall.inner().clone();
    let remote = tauri::async_runtime::spawn_blocking(move || {
        let card = client.card(&query.english_id).ok()?;
        let printed = query.localized_id.as_deref().and_then(|id| client.card(id).ok());
        let printings = card.oracle_id.as_deref().and_then(|o| client.prints(o).ok()).unwrap_or_default();
        Some((card, printed, printings))
    })
    .await
    .map_err(|e| e.to_string())?;
    if let Some((card, printed, printings)) = remote {
        with_game(&state, |g| {
            g.card_details_complete(&mut view, &card, printed.as_ref(), &printings);
            Ok(())
        })?;
    }
    Ok(view)
}

// ---- setup --------------------------------------------------------------------------

/// Where the player's games live. `UNSEALED_SAVES_DIR` overrides everything; debug builds
/// use the repository's `.dev-data/` so development never touches real saves.
fn resolve_saves_dir(app: &tauri::AppHandle) -> Result<PathBuf, Box<dyn Error>> {
    if let Ok(dir) = std::env::var("UNSEALED_SAVES_DIR") {
        return Ok(dir.into());
    }
    Ok(app_dir(app)?.join("saves"))
}

/// The single save of the versions without games, adopted as the first game.
fn legacy_db_path(app: &tauri::AppHandle) -> Result<PathBuf, Box<dyn Error>> {
    Ok(app_dir(app)?.join("unsealed.sqlite"))
}

fn app_dir(app: &tauri::AppHandle) -> Result<PathBuf, Box<dyn Error>> {
    if cfg!(debug_assertions) {
        return Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.dev-data"));
    }
    Ok(app.path().app_data_dir()?)
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

fn load_achievements(data_dir: &Path) -> Result<Engine, Box<dyn Error>> {
    let json =
        fs::read_to_string(data_dir.join("achievements.json")).unwrap_or_else(|_| DEFAULT_ACHIEVEMENTS.to_string());
    Ok(Engine::from_json(&json)?)
}

fn load_limited(data_dir: &Path) -> Result<LimitedConfig, Box<dyn Error>> {
    let json = fs::read_to_string(data_dir.join("limited.json")).unwrap_or_else(|_| DEFAULT_LIMITED.to_string());
    Ok(LimitedConfig::from_json(&json)?)
}

fn load_achievement_texts(data_dir: &Path) -> Result<AchievementTexts, Box<dyn Error>> {
    let mut texts = AchievementTexts::default();
    for (lang, shipped) in ACHIEVEMENT_TRANSLATIONS {
        let json = fs::read_to_string(data_dir.join(format!("achievements.{lang}.json")))
            .unwrap_or_else(|_| shipped.to_string());
        texts.add_language(lang, &json).map_err(|e| format!("achievements.{lang}.json: {e}"))?;
    }
    Ok(texts)
}

fn load_artifacts(data_dir: &Path) -> Result<ArtifactsConfig, Box<dyn Error>> {
    let json = fs::read_to_string(data_dir.join("artifacts.json")).unwrap_or_else(|_| DEFAULT_ARTIFACTS.to_string());
    Ok(ArtifactsConfig::from_json(&json)?)
}

fn load_artifact_texts(data_dir: &Path) -> Result<HashMap<String, ArtifactTranslations>, Box<dyn Error>> {
    let mut texts = HashMap::new();
    for (lang, shipped) in ARTIFACT_TRANSLATIONS {
        let json = fs::read_to_string(data_dir.join(format!("artifacts.{lang}.json"))).unwrap_or_else(|_| shipped.to_string());
        let parsed: ArtifactTranslations = serde_json::from_str(&json).map_err(|e| format!("artifacts.{lang}.json: {e}"))?;
        texts.insert(lang.to_string(), parsed);
    }
    Ok(texts)
}

/// The cycles: the MTG Wiki's and ours, their names translated. A file of the same name in
/// the data directory replaces the shipped one.
fn load_cycles(data_dir: &Path) -> Result<Cycles, Box<dyn Error>> {
    let read = |name: &str, shipped: &str| fs::read_to_string(data_dir.join(name)).unwrap_or_else(|_| shipped.to_string());
    let wiki = read("cycles.json", DEFAULT_CYCLES);
    let custom = read("cycles.custom.json", DEFAULT_CUSTOM_CYCLES);
    let texts: Vec<(&str, String)> =
        CYCLE_TRANSLATIONS.iter().map(|(lang, shipped)| (*lang, read(&format!("cycles.{lang}.json"), shipped))).collect();
    let texts: Vec<(&str, &str)> = texts.iter().map(|(lang, text)| (*lang, text.as_str())).collect();
    Ok(Cycles::from_json(&wiki, &custom, &texts)?)
}

fn load_data_config(data_dir: &Path) -> Result<DataConfig, Box<dyn Error>> {
    let json = fs::read_to_string(data_dir.join("data.json")).unwrap_or_else(|_| DEFAULT_DATA.to_string());
    Ok(DataConfig::from_json(&json)?)
}

/// Where the large raw downloads (MTGJSON sets, Scryfall bulk file) are kept.
fn resolve_cache_dir(app: &tauri::AppHandle) -> Result<PathBuf, Box<dyn Error>> {
    if cfg!(debug_assertions) {
        return Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.cache"));
    }
    Ok(app.path().app_cache_dir()?)
}

pub fn run() {
    tauri::Builder::default()
        // Opens links (the community's Discord) in the player's browser.
        .plugin(tauri_plugin_opener::init())
        // New versions of the app, published on GitHub, offered from the home screen; the
        // app restarts once one is installed.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // The window opens hidden, then shows in the display the player chose: never full
            // screen for a moment before going back to a window.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_fullscreen(display_fullscreen(app.handle()));
                let _ = window.show();
            }
            let data_dir = resolve_data_dir(app.handle())?;
            // Fail early on a broken configuration rather than when a game is loaded.
            load_economy(&data_dir)?;
            load_achievements(&data_dir)?;
            load_achievement_texts(&data_dir)?;
            load_limited(&data_dir)?;
            load_artifacts(&data_dir)?;
            load_artifact_texts(&data_dir)?;
            let data_config = load_data_config(&data_dir)?;

            let saves = Saves::new(resolve_saves_dir(app.handle())?)?;
            if let Some(adopted) = saves.adopt_legacy(&legacy_db_path(app.handle())?, now())? {
                eprintln!("previous save adopted as game {}", adopted.id);
            }
            app.manage(Mutex::new(Session {
                data_dir: data_dir.clone(),
                saves,
                current: None,
                card_lang: "en".to_string(),
                cycles: Arc::new(load_cycles(&data_dir)?),
            }));

            app.manage(Arc::new(scryfall::Client::default()));

            let status: SharedStatus = Arc::new(Mutex::new(DataStatus::Checking));
            app.manage(status.clone());
            let refresh = Arc::new(Refresh { data_dir, cache_dir: resolve_cache_dir(app.handle())?, cfg: data_config });
            app.manage(refresh.clone());
            if std::env::var("UNSEALED_NO_UPDATE").is_ok() {
                *status.lock().expect("fresh mutex") = DataStatus::UpToDate;
            } else {
                updater::spawn(app.handle().clone(), status, refresh);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_saves,
            current_save,
            new_game,
            load_game,
            create_save_point,
            rename_save,
            delete_save,
            get_data_status,
            retry_data_update,
            get_data_info,
            list_sets,
            set_card_language,
            get_wallet,
            get_shop,
            sell_boosters,
            buy_boosters,
            open_booster,
            open_boosters,
            reveal_cards,
            get_collection,
            list_artists,
            list_guest_series,
            get_artist_collection,
            check_set_images,
            get_owned_cards,
            get_library,
            collect_income,
            get_pinned_sets,
            set_pinned_sets,
            get_bet_context,
            get_bet_chance,
            get_kept_bets,
            set_kept_bets,
            sell_cards,
            sell_duplicates,
            claim_charity,
            get_achievements,
            take_unlocks,
            get_cycles,
            get_artifacts,
            buy_artifact,
            take_given_artifacts,
            set_display_fullscreen,
            get_stats,
            get_card_details,
            get_history,
            undo_transaction,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Unsealed");
}
