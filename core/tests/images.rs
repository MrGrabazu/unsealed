use unsealed_core::scryfall::is_card_image_path;

#[test]
fn the_image_cache_fetches_card_images_and_nothing_else() {
    let id = "ab12cd34-0000-4000-8000-123456789abc";
    for version in ["small", "normal", "large", "art_crop"] {
        assert!(is_card_image_path(&format!("{version}/front/a/b/{id}.jpg")), "{version}");
    }
    assert!(is_card_image_path(&format!("normal/back/a/b/{id}.jpg")));
    assert!(is_card_image_path(&format!("png/front/a/b/{id}.png")));
    // The folders are the id's first two characters.
    assert!(!is_card_image_path(&format!("normal/front/c/d/{id}.jpg")));
    for bad in [
        "".to_string(),
        format!("normal/front/a/b/{id}.exe"),
        format!("normal/front/a/b/../../{id}.jpg"),
        format!("huge/front/a/b/{id}.jpg"),
        format!("normal/side/a/b/{id}.jpg"),
        "normal/front/a/b/ab-not-an-id.jpg".to_string(),
        format!("https://evil.example/normal/front/a/b/{id}.jpg"),
    ] {
        assert!(!is_card_image_path(&bad), "{bad}");
    }
}
