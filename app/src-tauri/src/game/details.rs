//! A card's detail view: what the game knows locally (images, prices, copies owned), then
//! what Scryfall adds when it can be reached (rules text of each face, every printing).

use super::{localized, Game};
use serde::Serialize;
use std::collections::HashMap;
use unsealed_core::import::parse_cents;
use unsealed_core::scryfall;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardDetailsView {
    pub uuid: String,
    pub set_code: String,
    pub set_name: String,
    pub name: String,
    /// The language the card is shown in: the card language, or "en" when it was not
    /// printed in that language.
    pub shown_language: String,
    pub number: String,
    pub rarity: String,
    pub image_url: String,
    pub back_image_url: Option<String>,
    /// How its image (front) is turned to be read, when printed sideways (see `reading_turn`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reading: Option<&'static str>,
    pub artist: Option<String>,
    pub can_be_foil: bool,
    pub can_be_nonfoil: bool,
    /// Its foil's kind, when special (see `CardEntry::foil_kind`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub foil_kind: Option<String>,
    pub price_cents: Option<u32>,
    pub foil_price_cents: Option<u32>,
    /// The card has no market price of its own: its prices are estimated.
    pub price_estimated: bool,
    pub sell_value_cents: u32,
    pub foil_sell_value_cents: u32,
    pub owned: u32,
    pub owned_foil: u32,
    /// Rules text of each face; empty when Scryfall could not be reached.
    pub faces: Vec<FaceView>,
    /// Every printing of the card, oldest first; empty when Scryfall could not be reached.
    pub printings: Vec<PrintingView>,
    /// Whether Scryfall's details could be fetched.
    pub online: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FaceView {
    pub name: String,
    pub mana_cost: Option<String>,
    pub type_line: Option<String>,
    pub text: Option<String>,
    pub flavor_text: Option<String>,
    /// "3/4", a loyalty or a defense, when the face has one.
    pub stats: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintingView {
    pub set_code: String,
    pub set_name: String,
    pub number: String,
    pub rarity: String,
    pub released_at: Option<String>,
    pub image_url: Option<String>,
    /// Today's market prices on Scryfall, in cents of the market currency.
    pub price_cents: Option<u32>,
    pub foil_price_cents: Option<u32>,
    /// The set is offered by the Boosterium.
    pub in_game: bool,
    pub owned: u32,
    pub owned_foil: u32,
    /// The printing the details are about.
    pub current: bool,
}

/// How a card printed sideways is turned to be read (Scryfall's images are always upright, as
/// printed): "right" (a quarter turn clockwise) for split cards (Duskmourn's Rooms among
/// them), planes and the front of battles; "left" for an aftermath card's second half; "flip"
/// (half a turn) for a Kamigawa flip card's second half. None for any other card.
fn reading_turn(layout: &str, types: &[String]) -> Option<&'static str> {
    match layout {
        "split" | "planar" => Some("right"),
        "aftermath" => Some("left"),
        "flip" => Some("flip"),
        _ if types.iter().any(|t| t == "battle") => Some("right"),
        _ => None,
    }
}

/// What to ask Scryfall for a card.
pub struct DetailsQuery {
    /// The English printing (for the list of printings).
    pub english_id: String,
    /// The printing in the card language, when it exists (for the printed text).
    pub localized_id: Option<String>,
}

impl Game {
    /// The local part of a card's details, and what to ask Scryfall to complete them.
    pub fn card_details_local(&mut self, set_code: &str, uuid: &str) -> Result<(CardDetailsView, DetailsQuery), String> {
        self.ensure_loaded(set_code)?;
        self.ensure_prices();
        let owned = self.owned_with_tokens(set_code)?;
        let set = &self.sets[set_code];
        let card = &set.file.cards[*set.index.get(uuid).ok_or_else(|| format!("card {uuid} not in {set_code}"))?];
        let shown = localized(card, &self.card_lang, &self.image_quality, false);
        let have = owned.get(uuid).copied().unwrap_or_default();
        let query = DetailsQuery {
            english_id: card.scryfall_id.clone(),
            localized_id: (shown.scryfall_id != card.scryfall_id).then(|| shown.scryfall_id.clone()),
        };
        let view = CardDetailsView {
            uuid: uuid.to_string(),
            set_code: card.set_code.clone(),
            set_name: set.file.names.get(&self.card_lang).unwrap_or(&set.file.name).clone(),
            name: shown.name,
            shown_language: shown.lang,
            number: card.number.clone(),
            rarity: card.rarity.clone(),
            image_url: shown.image_url,
            back_image_url: shown.back_image_url,
            reading: reading_turn(&card.layout, &card.types),
            artist: card.artist.clone(),
            can_be_foil: card.finishes.iter().any(|f| f == "foil" || f == "etched"),
            can_be_nonfoil: card.finishes.iter().any(|f| f == "nonfoil"),
            foil_kind: card.foil_kind.clone(),
            price_cents: self.market_price(uuid, false),
            foil_price_cents: self.market_price(uuid, true),
            price_estimated: self.price_estimated(uuid),
            sell_value_cents: self.sale_value(uuid, false),
            foil_sell_value_cents: self.sale_value(uuid, true),
            owned: have.normal,
            owned_foil: have.foil,
            faces: Vec::new(),
            printings: Vec::new(),
            online: false,
        };
        Ok((view, query))
    }

    /// Completes the details with Scryfall's data: the faces' text (as printed in the card
    /// language when that printing was fetched) and every printing, with the copies owned
    /// of those the game offers.
    pub fn card_details_complete(
        &mut self,
        view: &mut CardDetailsView,
        card: &scryfall::Card,
        printed: Option<&scryfall::Card>,
        printings: &[scryfall::Card],
    ) {
        let text_source = printed.unwrap_or(card);
        view.faces = text_source.faces().into_iter().map(face_view).collect();
        view.online = true;

        let offered: Vec<String> = self.list_sets().unwrap_or_default().into_iter().map(|s| s.code).collect();
        let eur = self.economy.currency.market == "EUR";
        // Copies owned, by set then by Scryfall id.
        let mut owned_by_set: HashMap<String, HashMap<String, (u32, u32)>> = HashMap::new();
        view.printings = printings
            .iter()
            .map(|p| {
                let code = p.set.to_uppercase();
                let in_game = offered.contains(&code);
                let (owned, owned_foil) = if in_game {
                    let owned = owned_by_set.entry(code.clone()).or_insert_with(|| self.owned_by_scryfall_id(&code));
                    owned.get(&p.id).copied().unwrap_or_default()
                } else {
                    (0, 0)
                };
                let (price, foil) =
                    if eur { (&p.prices.eur, &p.prices.eur_foil) } else { (&p.prices.usd, &p.prices.usd_foil) };
                PrintingView {
                    set_code: code,
                    set_name: p.set_name.clone(),
                    number: p.collector_number.clone(),
                    rarity: p.rarity.clone(),
                    released_at: p.released_at.clone(),
                    image_url: p.small_image(),
                    price_cents: price.as_deref().and_then(parse_cents),
                    foil_price_cents: foil.as_deref().and_then(parse_cents),
                    in_game,
                    owned,
                    owned_foil,
                    current: p.id == card.id,
                }
            })
            .collect();
    }

    /// Copies owned of a set's own cards, by Scryfall id: (normal, foil).
    fn owned_by_scryfall_id(&mut self, set_code: &str) -> HashMap<String, (u32, u32)> {
        if self.ensure_loaded(set_code).is_err() {
            return HashMap::new();
        }
        let owned = self.store.owned_in_set(set_code).unwrap_or_default();
        let set = &self.sets[set_code];
        set.file
            .cards
            .iter()
            .filter(|c| c.set_code == set_code)
            .filter_map(|c| owned.get(&c.uuid).map(|o| (c.scryfall_id.clone(), (o.normal, o.foil))))
            .collect()
    }
}

fn face_view(face: scryfall::Face) -> FaceView {
    let stats = match (&face.power, &face.toughness) {
        (Some(p), Some(t)) => Some(format!("{p}/{t}")),
        _ => face.loyalty.clone().or_else(|| face.defense.clone()),
    };
    FaceView {
        name: face.printed_name.unwrap_or(face.name),
        mana_cost: face.mana_cost.filter(|m| !m.is_empty()),
        type_line: face.printed_type_line.or(face.type_line),
        text: face.printed_text.or(face.oracle_text).filter(|t| !t.is_empty()),
        flavor_text: face.flavor_text,
        stats,
    }
}

#[cfg(test)]
mod tests {
    use super::reading_turn;

    #[test]
    fn cards_printed_sideways_say_how_they_are_turned_to_be_read() {
        let types = |t: &[&str]| t.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(reading_turn("split", &types(&["enchantment"])), Some("right"));
        assert_eq!(reading_turn("planar", &types(&["plane"])), Some("right"));
        assert_eq!(reading_turn("transform", &types(&["battle"])), Some("right"));
        assert_eq!(reading_turn("aftermath", &types(&["sorcery"])), Some("left"));
        assert_eq!(reading_turn("flip", &types(&["creature"])), Some("flip"));
        assert_eq!(reading_turn("transform", &types(&["creature"])), None);
        assert_eq!(reading_turn("normal", &types(&["creature"])), None);
    }
}
