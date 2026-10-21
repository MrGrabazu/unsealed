use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::collections::{BTreeSet, HashSet};
use unsealed_core::artifacts::{ArtifactSource, ArtifactTranslations, ArtifactsConfig, SoldCard};

const CONFIG: &str = include_str!("../../config/artifacts.json");
const TRANSLATIONS: [(&str, &str); 4] = [
    ("fr", include_str!("../../config/artifacts.fr.json")),
    ("de", include_str!("../../config/artifacts.de.json")),
    ("es", include_str!("../../config/artifacts.es.json")),
    ("it", include_str!("../../config/artifacts.it.json")),
];
const ACHIEVEMENTS: &str = include_str!("../../config/achievements.json");

/// A non-foil card of a recent set.
fn sold(rarity: &str, market_cents: u32) -> SoldCard<'_> {
    SoldCard { rarity, market_cents, foil: false, released: "2024-01-01" }
}

fn owned(ids: &[&str]) -> HashSet<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

/// The `{cents:N}` tokens of a text, which every translation must keep.
fn tokens(text: &str) -> BTreeSet<String> {
    text.match_indices("{cents:").map(|(i, _)| text[i..].split('}').next().unwrap_or("").to_string()).collect()
}

#[test]
fn the_shipped_artifacts_are_valid_and_translated() {
    let config = ArtifactsConfig::from_json(CONFIG).unwrap();
    assert!(config.artifacts.len() >= 15);
    let ids: BTreeSet<&str> = config.artifacts.iter().map(|a| a.id.as_str()).collect();
    for (lang, text) in TRANSLATIONS {
        let translations: ArtifactTranslations = serde_json::from_str(text).unwrap();
        let translated: BTreeSet<&str> = translations.keys().map(String::as_str).collect();
        assert_eq!(translated, ids, "{lang} must translate exactly the artifacts defined");
        for a in &config.artifacts {
            assert_eq!(tokens(&translations[&a.id].effect), tokens(&a.effect), "{lang}: {} keeps its amounts", a.id);
        }
    }
    // An artifact given by an achievement names one that exists.
    let achievements: serde_json::Value = serde_json::from_str(ACHIEVEMENTS).unwrap();
    let achievement_ids: HashSet<&str> =
        achievements["achievements"].as_array().unwrap().iter().filter_map(|a| a["id"].as_str()).collect();
    for a in &config.artifacts {
        if let ArtifactSource::Achievement { id } = &a.source {
            assert!(achievement_ids.contains(id.as_str()), "{} is given by unknown achievement {id}", a.id);
        }
        assert!(!a.effect.contains('$'), "{}: amounts are written as {{cents:N}}", a.id);
    }
}

#[test]
fn invalid_configs_are_refused() {
    let bad = |from: &str, to: &str| ArtifactsConfig::from_json(&CONFIG.replacen(from, to, 1)).is_err();
    assert!(bad(r#""id": "silver-key""#, r#""id": "bronze-key""#), "duplicate id");
    assert!(bad(r#""requires": ["bronze-key"]"#, r#""requires": ["copper-key"]"#), "unknown requirement");
    assert!(bad(r#""priceCents": 25000"#, r#""priceCents": 0"#), "free in the shop");
    assert!(bad(r#""oneIn": 30"#, r#""oneIn": 0"#), "never found");
    assert!(bad(r#""releasedBefore": "2000-01-01""#, r#""releasedBefore": "2000""#), "invalid date");
    assert!(bad(r#""rarity": "rare""#, r#""rarity": "legendary""#), "unknown rarity");
}

#[test]
fn effects_add_up_and_the_best_sell_rate_wins() {
    let config = ArtifactsConfig::from_json(CONFIG).unwrap();
    let none = config.bonuses(&HashSet::new());
    assert_eq!((none.sell_rate(80, sold("rare", 100)), none.booster_discount("collector", "2024-01-01")), (80, 0));
    assert_eq!((none.charity_factor, none.undo_penalty_percent, none.income_bonus(3), none.income_factor), (1, 100, 0, 1));

    let b = config.bonuses(&owned(&["jeweler-loupe", "speculator-eye", "calligrapher-quill"]));
    assert_eq!(b.sell_rate(80, sold("rare", 100)), 85);
    assert_eq!(b.sell_rate(80, sold("common", 100)), 80);
    assert_eq!(b.sell_rate(80, sold("common", 5000)), 85); // worth 50 Seals or more
    assert_eq!(b.sell_rate(80, sold("token", 10)), 100);

    // The five keys: half price on collectors, and only on them; the token on every booster;
    // the archivist's seal on old sets only.
    let keys = ["bronze-key", "silver-key", "gold-key", "diamond-key", "obsidian-key"];
    let b = config.bonuses(&owned(&[&keys[..], &["merchant-token", "archivist-seal"]].concat()));
    assert_eq!(b.booster_discount("collector", "2024-01-01"), 55);
    assert_eq!(b.booster_discount("play", "2024-01-01"), 5);
    assert_eq!(b.booster_discount("default", "1994-06-01"), 15);

    let b = config.bonuses(&owned(&["collector-lectern", "velvet-purse"]));
    assert!(b.counts_completed_sets());
    assert_eq!((b.income_bonus(0), b.income_bonus(5), b.income_bonus(50)), (25, 35, 65)); // +2 % a set, 40 % at most
    // The shepherd's star on top of the lectern; the dragon's egg doubles it all.
    let b = config.bonuses(&owned(&["collector-lectern", "shepherd-star", "dragon-egg"]));
    assert_eq!((b.income_bonus(10), b.income_bonus(50), b.income_factor), (30, 70, 2));

    // Sell rates by finish and by the age of the set; an unknown date is never old.
    let b = config.bonuses(&owned(&["lapidary-prism", "antiquarian-monocle", "grimoire-of-myths"]));
    let card = |rarity, foil, released| SoldCard { rarity, market_cents: 100, foil, released };
    assert_eq!(b.sell_rate(80, card("common", true, "2024-01-01")), 90);
    assert_eq!(b.sell_rate(80, card("common", false, "2024-01-01")), 80);
    assert_eq!(b.sell_rate(80, card("common", false, "1994-06-01")), 90);
    assert_eq!(b.sell_rate(80, card("common", false, "")), 80);
    assert_eq!(b.sell_rate(80, card("mythic", false, "2024-01-01")), 90);
}

#[test]
fn the_monocle_hides_only_in_old_boosters_and_the_egg_waits_for_the_keys() {
    let config = ArtifactsConfig::from_json(CONFIG).unwrap();
    let mut rng = ChaCha8Rng::seed_from_u64(5);
    let mut recent = HashSet::new();
    for _ in 0..20_000 {
        if let Some(a) = config.roll_booster("collector", "2024-01-01", &recent, &mut rng) {
            assert_ne!(a.id, "antiquarian-monocle");
            recent.insert(a.id.clone());
        }
    }
    // Every key found (20,000 collectors), and then the egg.
    assert!(recent.contains("obsidian-key") && recent.contains("dragon-egg"));
    let mut old = HashSet::new();
    for _ in 0..5_000 {
        if let Some(a) = config.roll_booster("default", "1994-06-01", &old, &mut rng) {
            assert_ne!(a.id, "dragon-egg");
            old.insert(a.id.clone());
        }
    }
    assert!(old.contains("antiquarian-monocle"));
}

#[test]
fn each_key_is_found_only_by_the_bearer_of_the_ones_before() {
    let config = ArtifactsConfig::from_json(CONFIG).unwrap();
    let mut rng = ChaCha8Rng::seed_from_u64(3);
    let mut found = Vec::new();
    let mut mine = HashSet::new();
    let mut opened = 0u32;
    // Opening collector boosters until the five keys are found: always in their order.
    while found.len() < 5 && opened < 200_000 {
        opened += 1;
        if let Some(a) = config.roll_booster("collector", "2024-01-01", &mine, &mut rng) {
            if a.id.ends_with("-key") {
                found.push(a.id.clone());
            }
            mine.insert(a.id.clone());
        }
    }
    assert_eq!(found, ["bronze-key", "silver-key", "gold-key", "diamond-key", "obsidian-key"]);
    // Hundreds of collectors (about 855 expected): a reward of weeks, not of the first hour.
    assert!(opened > 300, "found in {opened} boosters");

    // A play booster never gives a key; nothing is found twice.
    let mut rng = ChaCha8Rng::seed_from_u64(4);
    let mut play = HashSet::new();
    for _ in 0..50_000 {
        if let Some(a) = config.roll_booster("play", "2024-01-01", &play, &mut rng) {
            assert!(!a.id.ends_with("-key") && !play.contains(&a.id) && a.id != "calligrapher-quill");
            play.insert(a.id.clone());
        }
    }
}

#[test]
fn the_crown_comes_once_every_other_artifact_is_owned() {
    let config = ArtifactsConfig::from_json(CONFIG).unwrap();
    let mut all: HashSet<String> = config.artifacts.iter().map(|a| a.id.clone()).filter(|id| id != "counter-crown").collect();
    assert_eq!(config.all_others_due(&all).map(|a| a.id.as_str()), Some("counter-crown"));
    all.remove("bronze-key");
    assert!(config.all_others_due(&all).is_none());
    let lectern: Vec<&str> = config.for_achievement("set-complete").map(|a| a.id.as_str()).collect();
    assert_eq!(lectern, ["collector-lectern"]);
}
