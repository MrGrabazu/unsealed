use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::collections::HashMap;
use unsealed_core::betting::{
    any_probability, chance, payout_cents, Bet, BetCard, CardFilter, CardType, Chance, Clause, ColorChoice, Compare, Finish,
    SimulatedPacks,
};
use unsealed_core::{BoosterConfig, ColorIndex};

/// Two variants: 3 in 4 packs draw 2 of 4 commons, 1 in 4 also has a foil from a sheet where
/// the Forest "a" weighs 1 of 4 against the mythic "m".
fn config() -> BoosterConfig {
    serde_json::from_str(
        r#"{
          "boosters": [
            { "contents": { "common": 2 }, "weight": 3 },
            { "contents": { "common": 2, "foil": 1 }, "weight": 1 }
          ],
          "boostersTotalWeight": 4,
          "sheets": {
            "common": { "cards": { "a": 1, "b": 1, "c": 1, "d": 1 }, "foil": false, "totalWeight": 4 },
            "foil": { "cards": { "a": 1, "m": 3 }, "foil": true, "totalWeight": 4 }
          }
        }"#,
    )
    .unwrap()
}

fn card(uuid: &str, rarity: &str, types: &[&str], colors: &[&str], price: u32) -> BetCard {
    BetCard {
        uuid: uuid.into(),
        rarity: rarity.into(),
        types: types.iter().map(|t| t.to_string()).collect(),
        colors: colors.iter().map(|c| c.to_string()).collect(),
        price_cents: [price, price * 3],
        ..BetCard::default()
    }
}

fn cards() -> (Vec<BetCard>, HashMap<String, usize>) {
    let cards = vec![
        card("a", "common", &["basic", "land"], &["G"], 10),
        card("b", "common", &["creature"], &["R"], 20),
        card("c", "common", &["instant"], &["U", "R"], 30),
        card("d", "common", &["artifact"], &[], 40),
        card("m", "mythic", &["creature"], &["B"], 1_000),
    ];
    let index = cards.iter().enumerate().map(|(i, c)| (c.uuid.clone(), i)).collect();
    (cards, index)
}

fn cards_bet(compare: Compare, count: u32, filter: CardFilter) -> Bet {
    Bet { clauses: vec![Clause::Cards { compare, count, filter }] }
}

#[test]
fn whether_a_kind_of_card_comes_has_an_exact_chance() {
    let (cfg, (cards, index)) = (config(), cards());
    let card_a = |finish| CardFilter { card: Some("a".into()), finish, ..CardFilter::default() };
    // Plain "a": 2 of 4 equal cards, so 1/2. Foil "a": 1/4 of the packs, then 1/4.
    assert!((any_probability(&cfg, &cards, &index, &card_a(Finish::Normal)) - 0.5).abs() < 1e-9);
    assert!((any_probability(&cfg, &cards, &index, &card_a(Finish::Foil)) - 0.0625).abs() < 1e-9);
    // Either: 1 - (3/4 · 1/2 + 1/4 · 1/2 · 3/4).
    assert!((any_probability(&cfg, &cards, &index, &card_a(Finish::Any)) - 0.53125).abs() < 1e-9);

    // A group of cards: a red card (b or c) among 2 of 4 equal commons: 1 - 1/6.
    let red = CardFilter { colors: vec![ColorChoice::Red], ..CardFilter::default() };
    assert!((any_probability(&cfg, &cards, &index, &red) - 5.0 / 6.0).abs() < 1e-9);
    let packs = SimulatedPacks::default();
    let none = chance(&cards_bet(Compare::AtMost, 0, red.clone()), &cfg, &cards, &index, &packs);
    assert!(matches!(none, Chance::Exact(p) if (p - 1.0 / 6.0).abs() < 1e-9), "{none:?}");
}

#[test]
fn other_bets_are_counted_on_simulated_boosters_and_match_the_draws() {
    let (cfg, (cards, index)) = (config(), cards());
    let mut rng = ChaCha8Rng::seed_from_u64(3);
    let packs = SimulatedPacks::draw(&cfg, &ColorIndex::default(), &index, 40_000, &mut rng).unwrap();
    // Exactly 2 creatures: b in the commons and the foil mythic, 1/4 · 1/2 · 3/4.
    let creatures = CardFilter { types: vec![CardType::Creature], ..CardFilter::default() };
    let two = chance(&cards_bet(Compare::Exactly, 2, creatures.clone()), &cfg, &cards, &index, &packs);
    assert!(matches!(two, Chance::Simulated { .. }));
    assert!((two.probability() - 0.09375).abs() < 0.006, "{two:?}");

    // A combination: a basic land and a booster worth at most 50 (a with b, c or d plain only).
    let land = CardFilter { types: vec![CardType::BasicLand], ..CardFilter::default() };
    let both = Bet {
        clauses: vec![
            Clause::Cards { compare: Compare::AtLeast, count: 1, filter: land },
            Clause::Value { compare: Compare::AtMost, cents: 50 },
        ],
    };
    // In a plain pack, "a" comes in 1/2 of them, with b (30) or c (40) but not d (50 is fine): all count.
    let p = chance(&both, &cfg, &cards, &index, &packs).probability();
    assert!((p - 0.75 * 0.5).abs() < 0.01, "{p}");
}

#[test]
fn filters_read_every_criterion() {
    let (cards, _) = cards();
    let f = |filter: CardFilter, i: usize, foil: bool| filter.matches(&cards[i], foil);
    assert!(f(CardFilter { colors: vec![ColorChoice::Multicolor], ..Default::default() }, 2, false));
    assert!(f(CardFilter { colors: vec![ColorChoice::Colorless], ..Default::default() }, 3, false));
    assert!(!f(CardFilter { colors: vec![ColorChoice::Colorless], ..Default::default() }, 0, false));
    assert!(f(CardFilter { rarities: vec!["mythic".into()], finish: Finish::Foil, ..Default::default() }, 4, true));
    assert!(!f(CardFilter { rarities: vec!["mythic".into()], finish: Finish::Foil, ..Default::default() }, 4, false));
    // The price is the finish's: the foil Forest is worth 30.
    assert!(f(CardFilter { min_cents: Some(25), ..Default::default() }, 0, true));
    assert!(!f(CardFilter { min_cents: Some(25), ..Default::default() }, 0, false));
    assert!(Bet { clauses: vec![] }.validate().is_err());
    assert!(Bet { clauses: vec![Clause::Value { compare: Compare::Exactly, cents: 1 }] }.validate().is_err());
}

#[test]
fn fair_odds_pay_the_stake_divided_by_the_chance() {
    assert_eq!(payout_cents(100, 0.25, 0), 400);
    assert_eq!(payout_cents(100, 0.25, 10), 360);
    assert_eq!(payout_cents(100, 1.0 / 3.0, 0), 300);
}
