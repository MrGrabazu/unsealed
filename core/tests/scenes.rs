use unsealed_core::scenes::{SceneCell, Scenes, SCENE_LANGUAGES};

const SHIPPED: &str = include_str!("../../config/scenes.json");

#[test]
fn the_shipped_scenes_are_valid_and_named_in_every_language() {
    let scenes = Scenes::from_json(SHIPPED).unwrap();
    let mut count = 0;
    for code in scenes.set_codes() {
        for scene in scenes.of_set(code) {
            count += 1;
            for lang in SCENE_LANGUAGES {
                assert!(scenes_name(&scene.names, lang), "{code}/{} has no {lang} name", scene.id);
            }
        }
    }
    assert!(count >= 24);
    // Bilbo's Birthday Party: six cards, three a row.
    let party = scenes.of_set("LTR").iter().find(|s| s.id == "bilbos-birthday-party").unwrap();
    assert_eq!((party.columns, party.rows(), party.name("fr")), (3, 2, "L'anniversaire de Bilbo"));
    // The Hulks: She-Hulk is in it by her back face, as Jennifer Walters in the barbecue.
    let hulks = scenes.of_set("MSH").iter().find(|s| s.id == "hulks-smash").unwrap();
    assert_eq!(hulks.cells[0], SceneCell { number: "328".into(), back: true });
    let bbq = scenes.of_set("MSH").iter().find(|s| s.id == "avengers-balcony-bbq").unwrap();
    assert_eq!(bbq.cells[0], SceneCell { number: "328".into(), back: false });
}

fn scenes_name(names: &std::collections::BTreeMap<String, String>, lang: &str) -> bool {
    names.get(lang).is_some_and(|n| !n.trim().is_empty())
}

#[test]
fn a_scene_that_does_not_fill_its_rows_or_names_a_face_wrongly_is_refused() {
    let scene = |columns: u32, cells: &str| {
        format!(r#"{{"sets":{{"LTR":[{{"id":"a","name":{{"en":"A"}},"columns":{columns},"cells":[{cells}]}}]}}}}"#)
    };
    assert!(Scenes::from_json(&scene(2, r#""1","2","3","4""#)).is_ok());
    assert!(Scenes::from_json(&scene(2, r#""1","2","3""#)).is_err());
    assert!(Scenes::from_json(&scene(0, r#""1","2""#)).is_err());
    assert!(Scenes::from_json(&scene(2, r#""1","1:front""#)).is_err());
    assert!(Scenes::from_json(&scene(2, r#""1","1""#)).is_err());
    // A card's two faces may be in the same scene.
    assert!(Scenes::from_json(&scene(2, r#""1","1:back""#)).is_ok());
}
