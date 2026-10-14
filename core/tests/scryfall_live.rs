//! Checks the Scryfall client against the real API. Needs a connection, so it only runs on
//! demand: `cargo test -p unsealed-core --test scryfall_live -- --ignored`.

use unsealed_core::scryfall::Client;

#[test]
#[ignore = "needs a connection to api.scryfall.com"]
fn fetches_a_double_faced_card_and_its_printings() {
    let client = Client::default();
    // Loyal Cathar // Unhallowed Cathar (Dark Ascension), a transforming card.
    let card = client.card("cb09041b-4d09-4cae-9e85-b859edae885b").unwrap();
    let faces = card.faces();
    assert_eq!(faces.len(), 2, "{faces:?}");
    assert!(faces.iter().all(|f| f.type_line.is_some()));
    let oracle = card.oracle_id.clone().expect("an oracle id");

    let prints = client.prints(&oracle).unwrap();
    assert!(prints.iter().any(|p| p.id == card.id), "the card is among its printings");
    assert!(prints.iter().all(|p| p.small_image().is_some()));
    println!("{} printings, first in {}", prints.len(), prints[0].set_name);
    // Served from the cache the second time.
    assert_eq!(client.card(&card.id).unwrap().name, card.name);
}

#[test]
#[ignore = "needs a connection to api.scryfall.com"]
fn tells_the_quality_of_translated_images() {
    let client = Client::default();
    // French printings of Duskmourn cards: Get Out (no image) and Disorienting Choice (a
    // small scan); an English one, well scanned.
    let ids = [
        "a6e1af02-3c08-4dda-9006-70bfbb089429",
        "9309b66a-2f13-4fed-b63d-57c205d6909b",
        "5d9471c8-2db9-4ce3-a1e5-42b85134cb8e",
    ]
    .map(String::from);
    let statuses = client.image_statuses(&ids).unwrap();
    assert_eq!(statuses[&ids[0]], "missing");
    assert_eq!(statuses[&ids[1]], "lowres");
    assert_eq!(statuses[&ids[2]], "highres_scan");
}
