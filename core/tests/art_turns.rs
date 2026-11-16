use unsealed_core::art_turns::ArtTurns;

#[test]
fn the_shipped_orientations_are_valid() {
    ArtTurns::from_json(include_str!("../../config/art-orientation.json")).unwrap();
}

#[test]
fn a_card_upright_or_not_listed_has_no_turn() {
    let turns = ArtTurns::from_json(r#"{"cards":{"ATLA/1":"right","ATLA/2":"up","ATLA/50":"flip"}}"#).unwrap();
    assert_eq!(turns.turn("ATLA", "1"), Some("right"));
    assert_eq!(turns.turn("ATLA", "50"), Some("flip"));
    assert_eq!(turns.turn("ATLA", "2"), None);
    assert_eq!(turns.turn("ATLA", "3"), None);
    // A signed version is turned as its card.
    assert_eq!(turns.turn("ATLA", "50s"), Some("flip"));
}

#[test]
fn an_unknown_turn_is_refused() {
    assert!(ArtTurns::from_json(r#"{"cards":{"ATLA/1":"sideways"}}"#).is_err());
    assert!(ArtTurns::from_json(r#"{"cards":{"ATLA":"up"}}"#).is_err());
}
