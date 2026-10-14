//! On-demand card details from the Scryfall API: rules text, faces and every printing of a
//! card. Too large to ship for every card, so they are fetched when the player opens a card,
//! and kept in memory for the session.
//!
//! Scryfall asks for at most 10 requests per second and the `User-Agent` / `Accept` headers
//! (sent by `builder::http_get`): requests are spaced by `MIN_INTERVAL`.

use crate::builder::{http_get, http_post_json, BuildError};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const API: &str = "https://api.scryfall.com";
const MIN_INTERVAL: Duration = Duration::from_millis(100);
/// Printings shown for a card (Scryfall's first page holds 175).
const MAX_PRINTS: usize = 175;

#[derive(Debug, Clone, Deserialize)]
pub struct Card {
    pub id: String,
    pub oracle_id: Option<String>,
    pub name: String,
    pub printed_name: Option<String>,
    pub lang: Option<String>,
    pub layout: Option<String>,
    pub mana_cost: Option<String>,
    pub type_line: Option<String>,
    pub printed_type_line: Option<String>,
    pub oracle_text: Option<String>,
    pub printed_text: Option<String>,
    pub flavor_text: Option<String>,
    pub power: Option<String>,
    pub toughness: Option<String>,
    pub loyalty: Option<String>,
    pub defense: Option<String>,
    pub artist: Option<String>,
    pub set: String,
    pub set_name: String,
    pub collector_number: String,
    pub rarity: String,
    pub released_at: Option<String>,
    #[serde(default)]
    pub finishes: Vec<String>,
    #[serde(default)]
    pub prices: Prices,
    pub image_uris: Option<ImageUris>,
    pub card_faces: Option<Vec<Face>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Prices {
    pub usd: Option<String>,
    pub usd_foil: Option<String>,
    pub eur: Option<String>,
    pub eur_foil: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageUris {
    pub normal: Option<String>,
    pub small: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Face {
    pub name: String,
    pub printed_name: Option<String>,
    pub mana_cost: Option<String>,
    pub type_line: Option<String>,
    pub printed_type_line: Option<String>,
    pub oracle_text: Option<String>,
    pub printed_text: Option<String>,
    pub flavor_text: Option<String>,
    pub power: Option<String>,
    pub toughness: Option<String>,
    pub loyalty: Option<String>,
    pub defense: Option<String>,
    pub image_uris: Option<ImageUris>,
}

impl Card {
    /// The card's faces: its `card_faces`, or the card itself as a single face.
    pub fn faces(&self) -> Vec<Face> {
        match &self.card_faces {
            Some(faces) if !faces.is_empty() => faces.clone(),
            _ => vec![Face {
                name: self.name.clone(),
                printed_name: self.printed_name.clone(),
                mana_cost: self.mana_cost.clone(),
                type_line: self.type_line.clone(),
                printed_type_line: self.printed_type_line.clone(),
                oracle_text: self.oracle_text.clone(),
                printed_text: self.printed_text.clone(),
                flavor_text: self.flavor_text.clone(),
                power: self.power.clone(),
                toughness: self.toughness.clone(),
                loyalty: self.loyalty.clone(),
                defense: self.defense.clone(),
                image_uris: self.image_uris.clone(),
            }],
        }
    }

    /// The small image of the card (its front face for double-faced cards).
    pub fn small_image(&self) -> Option<String> {
        let face = || self.card_faces.as_ref()?.first()?.image_uris.as_ref()?.small.clone();
        self.image_uris.as_ref().and_then(|u| u.small.clone()).or_else(face)
    }
}

#[derive(Deserialize)]
struct SearchPage {
    data: Vec<Card>,
}

/// Fetches and caches Scryfall responses for the whole session.
#[derive(Default)]
pub struct Client {
    cards: Mutex<HashMap<String, Card>>,
    prints: Mutex<HashMap<String, Vec<Card>>>,
    last_request: Mutex<Option<Instant>>,
}

impl Client {
    pub fn card(&self, scryfall_id: &str) -> Result<Card, BuildError> {
        if let Some(card) = self.cards.lock().map_err(|_| "cache poisoned")?.get(scryfall_id) {
            return Ok(card.clone());
        }
        let card: Card = self.get(&format!("{API}/cards/{scryfall_id}"))?;
        self.cards.lock().map_err(|_| "cache poisoned")?.insert(scryfall_id.to_string(), card.clone());
        Ok(card)
    }

    /// Every printing of a card (in English), oldest first.
    pub fn prints(&self, oracle_id: &str) -> Result<Vec<Card>, BuildError> {
        if let Some(prints) = self.prints.lock().map_err(|_| "cache poisoned")?.get(oracle_id) {
            return Ok(prints.clone());
        }
        let url = format!("{API}/cards/search?order=released&dir=asc&unique=prints&q=oracleid%3A{oracle_id}");
        let mut prints = self.get::<SearchPage>(&url)?.data;
        prints.truncate(MAX_PRINTS);
        self.prints.lock().map_err(|_| "cache poisoned")?.insert(oracle_id.to_string(), prints.clone());
        Ok(prints)
    }

    /// Whether the images of printings are good scans: Scryfall's `image_status` by id
    /// ("highres_scan", or "lowres", "placeholder", "missing": many translated printings
    /// only have a small scan, or none). Asked in batches (`/cards/collection`, 75 ids each);
    /// ids Scryfall does not know are left out.
    pub fn image_statuses(&self, ids: &[String]) -> Result<HashMap<String, String>, BuildError> {
        #[derive(Deserialize)]
        struct Page {
            data: Vec<Status>,
        }
        #[derive(Deserialize)]
        struct Status {
            id: String,
            #[serde(default)]
            image_status: Option<String>,
        }
        let mut statuses = HashMap::new();
        for chunk in ids.chunks(COLLECTION_BATCH) {
            let identifiers: Vec<_> = chunk.iter().map(|id| serde_json::json!({ "id": id })).collect();
            let body = serde_json::json!({ "identifiers": identifiers }).to_string();
            self.wait_turn()?;
            let page: Page = serde_json::from_reader(http_post_json(&format!("{API}/cards/collection"), &body)?.into_reader())?;
            statuses.extend(page.data.into_iter().map(|s| (s.id, s.image_status.unwrap_or_default())));
        }
        Ok(statuses)
    }

    fn get<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, BuildError> {
        self.wait_turn()?;
        Ok(serde_json::from_reader(http_get(url)?.into_reader())?)
    }

    /// Spaces requests by `MIN_INTERVAL`, as Scryfall asks.
    fn wait_turn(&self) -> Result<(), BuildError> {
        let mut last = self.last_request.lock().map_err(|_| "rate limiter poisoned")?;
        if let Some(wait) = last.map(|t| MIN_INTERVAL.saturating_sub(t.elapsed())) {
            std::thread::sleep(wait);
        }
        *last = Some(Instant::now());
        Ok(())
    }
}

/// Ids per `/cards/collection` request (Scryfall's maximum).
const COLLECTION_BATCH: usize = 75;
