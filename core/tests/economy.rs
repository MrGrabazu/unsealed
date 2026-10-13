use unsealed_core::catalog::{CardEntry, PriceSet};
use unsealed_core::economy::{charity_available, EconomyConfig};
use unsealed_core::import::prices_by_uuid;

const CONFIG: &str = include_str!("../../config/economy.json");

fn config() -> EconomyConfig {
    EconomyConfig::from_json(CONFIG).expect("shipped economy config is valid")
}

#[test]
fn shipped_config_matches_the_agreed_rules() {
    let c = config();
    assert_eq!(c.starting_balance_cents, 10_000);
    assert_eq!(c.booster_price_cents("BLB", "play", None), 750);
    assert_eq!(c.sell_rate_percent, 80);
    assert_eq!((c.charity.amount_cents, c.charity.max_claims_per_day), (2_000, 1));
}

#[test]
fn booster_price_uses_overrides_then_market_value_then_default() {
    let mut c = config();
    c.booster_prices.overrides.insert("BLB".into(), [("collector".to_string(), 2_500)].into());
    // A fixed override ignores the market.
    assert_eq!(c.booster_price_cents("BLB", "collector", Some(9_000)), 2_500);
    // Otherwise the price follows the expected market value (100% by default).
    assert_eq!(c.booster_price_cents("BLB", "play", Some(743)), 743);
    c.booster_prices.ev_markup_percent = 120;
    assert_eq!(c.booster_price_cents("DSK", "play", Some(1_000)), 1_200);
    // Never below the minimum price.
    assert_eq!(c.booster_price_cents("DSK", "play", Some(10)), 100);
    // Unknown market value: default price.
    assert_eq!(c.booster_price_cents("DSK", "play", None), 750);
}

#[test]
fn sealed_boosters_sell_back_at_the_configured_rate() {
    let c = config();
    assert_eq!(c.booster_sell_value_cents(1_000), 800);
    assert_eq!(c.booster_sell_value_cents(743), 594);
}

#[test]
fn sell_value_is_a_rounded_down_share_of_the_market_price() {
    let c = config();
    assert_eq!(c.sell_value_cents(1_000), 800);
    assert_eq!(c.sell_value_cents(5), 4);
    assert_eq!(c.sell_value_cents(1), 0);
    assert_eq!(c.sell_value_cents(u32::MAX), (u64::from(u32::MAX) * 80 / 100) as u32);
}

#[test]
fn market_price_follows_the_currency_and_finish() {
    let p = PriceSet { usd: Some(100), usd_foil: Some(300), eur: Some(90), eur_foil: None };
    let mut c = config();
    assert_eq!((c.market_price_cents(&p, false), c.market_price_cents(&p, true)), (Some(100), Some(300)));
    c.currency.market = "EUR".into();
    assert_eq!((c.market_price_cents(&p, false), c.market_price_cents(&p, true)), (Some(90), None));
}

#[test]
fn rejects_invalid_configs() {
    let bad = |from: &str, to: &str| EconomyConfig::from_json(&CONFIG.replace(from, to)).is_err();
    assert!(bad(r#""USD""#, r#""GBP""#));
    assert!(bad(r#""sellRatePercent": 80"#, r#""sellRatePercent": 120"#));
    assert!(bad(r#""defaultCents": 750"#, r#""defaultCents": 0"#));
    assert!(bad(r#""boosterSellRatePercent": 80"#, r#""boosterSellRatePercent": 101"#));
    assert!(bad(r#""sealsPerCent": 1"#, r#""sealsPerCent": 0"#));
}

#[test]
fn undoing_a_sale_costs_more_as_time_goes_by() {
    let c = config().cancellation;
    let minutes = |m: i64| m * 60;
    assert_eq!(c.sale_penalty_percent(minutes(0)), 5);
    assert_eq!(c.sale_penalty_percent(minutes(5)), 5);
    assert_eq!(c.sale_penalty_percent(minutes(6)), 10);
    assert_eq!(c.sale_penalty_percent(minutes(60 * 20)), 25);
    assert_eq!(c.sale_penalty_percent(minutes(60 * 24 * 2)), 50);
    assert_eq!(c.sale_penalty_percent(minutes(60 * 24 * 10)), 75);
    assert_eq!(c.sale_penalty_percent(minutes(60 * 24 * 60)), 90);
    // Sold for 800: undoing it right away costs 840.
    assert_eq!(c.sale_cancel_cost_cents(800, 30), 840);
}

#[test]
fn a_purchase_can_only_be_undone_right_away() {
    let c = config().cancellation;
    assert_eq!(c.purchase_refund_cents(1_000, 60), Some(950));
    assert_eq!(c.purchase_refund_cents(1_000, 5 * 60), Some(950));
    assert_eq!(c.purchase_refund_cents(1_000, 5 * 60 + 1), None);
    // An economy file without the section gets the same defaults.
    let start = CONFIG.find(r#",
  "cancellation""#).unwrap();
    let without = format!("{}\n}}", &CONFIG[..start]);
    let defaults = EconomyConfig::from_json(&without).unwrap().cancellation;
    assert_eq!(defaults.sale_penalty_percent(0), 5);
    assert!(bad_cancellation(r#""penaltyPercent": 50"#, r#""penaltyPercent": 150"#));
    assert!(bad_cancellation(r#""withinMinutes": 60,"#, r#""withinMinutes": 2,"#));
}

fn bad_cancellation(from: &str, to: &str) -> bool {
    EconomyConfig::from_json(&CONFIG.replace(from, to)).is_err()
}

#[test]
fn seals_default_to_one_per_cent() {
    let c = config();
    assert_eq!(c.currency.seals_per_cent, 1.0);
    // An older config naming the market currency `code` still loads, with the defaults.
    let old = EconomyConfig::from_json(&CONFIG.replace(
        r#""market": "USD", "sealsPerCent": 1"#,
        r#""code": "USD", "symbol": "$""#,
    ))
    .unwrap();
    assert_eq!((old.currency.market.as_str(), old.currency.seals_per_cent), ("USD", 1.0));
}

#[test]
fn charity_only_when_stuck_and_once_per_day() {
    let charity = config().charity;
    // Broke, nothing to sell, nothing to open, nothing claimed yet.
    assert!(charity_available(&charity, 0, 400, false, false, 0));
    // Cannot afford a booster either: 3.00 < 4.00.
    assert!(charity_available(&charity, 300, 400, false, false, 0));
    // Can still buy a booster.
    assert!(!charity_available(&charity, 400, 400, false, false, 0));
    // Has cards worth selling: the player must sacrifice one.
    assert!(!charity_available(&charity, 0, 400, true, false, 0));
    // Has an unopened booster.
    assert!(!charity_available(&charity, 0, 400, false, true, 0));
    // Already claimed today.
    assert!(!charity_available(&charity, 0, 400, false, false, 1));
}

#[test]
fn prices_are_rekeyed_by_uuid_including_double_faced_cards() {
    let card = |uuid: &str, scryfall_id: &str| CardEntry {
        uuid: uuid.into(),
        set_code: "TST".into(),
        scryfall_id: scryfall_id.into(),
        name: uuid.into(),
        number: "1".into(),
        rarity: "common".into(),
        colors: vec![],
        finishes: vec![],
        layout: "transform".into(),
        side: None,
        artist: None,
        foreign: Default::default(),
    };
    let by_scryfall = [("s1".to_string(), PriceSet { usd: Some(25), ..Default::default() })].into();
    let cards = [card("front", "s1"), card("back", "s1"), card("other", "s2")];
    let prices = prices_by_uuid(&by_scryfall, &cards);
    assert_eq!(prices.len(), 2);
    assert_eq!(prices["front"].usd, Some(25));
    assert_eq!(prices["back"].usd, Some(25));
}
