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

/// Times the download of a booster's worth of large card images (from the set file of
/// `dist/sets/FRA.json`): one after another, then all at once, as the 3D opening asks.
#[test]
#[ignore = "needs a connection to cards.scryfall.io and the data built in dist/"]
fn times_card_image_downloads() {
    use std::time::Instant;
    let file = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../dist/sets/FRA.json")).unwrap();
    let set: unsealed_core::catalog::SetFile = serde_json::from_str(&file).unwrap();
    let paths: Vec<String> = set
        .cards
        .iter()
        .map(|c| format!("large/front/{}/{}/{}.jpg", &c.scryfall_id[..1], &c.scryfall_id[1..2], c.scryfall_id))
        .collect();
    let time = |label: &str, batch: &[String], parallel: bool| {
        let start = Instant::now();
        if parallel {
            std::thread::scope(|s| {
                for p in batch {
                    s.spawn(move || unsealed_core::scryfall::card_image(p).unwrap());
                }
            });
        } else {
            for p in batch {
                let one = Instant::now();
                let bytes = unsealed_core::scryfall::card_image(p).unwrap();
                println!("  {} KB in {:?}", bytes.len() / 1024, one.elapsed());
            }
        }
        println!("{label}: {} images in {:?}", batch.len(), start.elapsed());
    };
    time("one after another", &paths[0..5], false);
    time("all at once", &paths[5..20], true);
}

/// Each step of an image download with ureq, to tell where the time goes.
#[test]
#[ignore = "needs a connection to cards.scryfall.io and the data built in dist/"]
fn traces_card_image_downloads() {
    use std::io::Read;
    use std::time::{Duration, Instant};
    let file = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../dist/sets/FRA.json")).unwrap();
    let set: unsealed_core::catalog::SetFile = serde_json::from_str(&file).unwrap();
    let agent = ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(3)).timeout_read(Duration::from_secs(10)).build();
    for c in &set.cards[40..48] {
        let url = format!("https://cards.scryfall.io/large/front/{}/{}/{}.jpg", &c.scryfall_id[..1], &c.scryfall_id[1..2], c.scryfall_id);
        let start = Instant::now();
        match agent.get(&url).set("User-Agent", "Unsealed/0.1 (unofficial fan project)").call() {
            Ok(response) => {
                let headers = start.elapsed();
                let mut bytes = Vec::new();
                let read = response.into_reader().read_to_end(&mut bytes);
                println!("headers after {headers:?}, {} KB after {:?} ({read:?})", bytes.len() / 1024, start.elapsed());
            }
            Err(e) => println!("failed after {:?}: {e}", start.elapsed()),
        }
    }
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
