use unsealed_core::lore::{text, Lore};

const SHIPPED: &str = include_str!("../../config/lore.json");

#[test]
fn the_shipped_lore_is_valid_and_written_in_every_language() {
    let lore = Lore::from_json(SHIPPED).unwrap();
    let dsk = lore.of_set("DSK").unwrap();
    assert_eq!(text(&dsk.plane, "fr"), "Duskmourn");
    // The first chapter is open from the start; the last asks for Valgavoth himself.
    assert!(dsk.chapters[0].keys.is_empty());
    assert!(dsk.chapters.last().unwrap().keys.iter().any(|k| k == "120"));
    // Every set tells its story in the same number of paragraphs in each language.
    for code in lore.set_codes() {
        for chapter in &lore.of_set(code).unwrap().chapters {
            let counts: Vec<usize> = ["en", "fr", "de", "es", "it"].iter().map(|&l| chapter.paragraphs(l).len()).collect();
            assert!(counts.iter().all(|&n| n == counts[0]), "{code}: {:?} {counts:?}", chapter.title.get("en"));
        }
    }
}

#[test]
fn a_text_missing_in_a_language_or_a_chapter_without_illustration_is_refused() {
    let all = |s: &str| format!(r#"{{"en":"{s}","fr":"{s}","de":"{s}","es":"{s}","it":"{s}"}}"#);
    let paragraphs = r#"{"en":["a"],"fr":["a"],"de":["a"],"es":["a"],"it":["a"]}"#;
    let set = |plane: &str, illustration: &str, chapter_text: &str| {
        format!(
            r#"{{"sets":{{"DSK":{{"plane":{plane},"illustration":"271","summary":{s},"characters":[{{"card":"120","name":{s},"role":{s}}}],
            "chapters":[{{"title":{s},"illustration":"{illustration}","keys":["120"],"text":{chapter_text}}}]}}}}}}"#,
            s = all("x")
        )
    };
    assert!(Lore::from_json(&set(&all("D"), "120", paragraphs)).is_ok());
    assert!(Lore::from_json(&set(r#"{"en":"D","fr":"D"}"#, "120", paragraphs)).is_err());
    assert!(Lore::from_json(&set(&all("D"), "", paragraphs)).is_err());
    assert!(Lore::from_json(&set(&all("D"), "120", r#"{"en":["a"],"fr":[],"de":["a"],"es":["a"],"it":["a"]}"#)).is_err());
}
