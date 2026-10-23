//! How fast the artifacts come: two players simulated over months of play on the real data
//! (`dist/`), economy, achievements and artifacts, with a clock that runs as fast as needed.
//! `cargo test --release -p unsealed-app pace -- --ignored --nocapture`.

use super::*;
use rand::Rng;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use unsealed_core::artifacts::ArtifactsConfig;

/// Days played (`UNSEALED_PACE_DAYS` changes it).
fn days() -> i64 {
    std::env::var("UNSEALED_PACE_DAYS").ok().and_then(|d| d.parse().ok()).unwrap_or(120)
}
const DAY: i64 = 86_400;
const MINUTE: i64 = 60;

struct Profile {
    name: &'static str,
    /// Minutes played a day, in one evening session.
    session_minutes: i64,
    /// Boosters bought and opened at a time (10: opened by batch).
    batch: u32,
    /// Seconds a booster takes, watched or rushed.
    seconds_per_booster: i64,
    /// Share of Collector boosters, of sets released before 2000.
    collector_share: f64,
    old_share: f64,
    /// An artifact on sale is bought once the balance is this many times its price.
    shop_reserve: f64,
}

const CASUAL: Profile = Profile {
    name: "casual",
    session_minutes: 60,
    batch: 1,
    seconds_per_booster: 40,
    collector_share: 0.15,
    old_share: 0.05,
    shop_reserve: 1.5,
};
const GRINDER: Profile = Profile {
    name: "grinder",
    session_minutes: 240,
    batch: 10,
    seconds_per_booster: 4,
    collector_share: 0.3,
    old_share: 0.05,
    shop_reserve: 1.0,
};

fn day_of(now: i64) -> String {
    chrono::DateTime::from_timestamp(now, 0).unwrap().format("%Y-%m-%d").to_string()
}

fn play(profile: &Profile) {
    let dist = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist");
    let start = chrono::NaiveDate::from_ymd_opt(2026, 10, 21).unwrap().and_hms_opt(19, 0, 0).unwrap().and_utc().timestamp();
    let now = Arc::new(AtomicI64::new(start));
    let clock_now = now.clone();
    let mut g = Game::with_parts(
        dist,
        Store::open_in_memory().unwrap(),
        EconomyConfig::from_json(include_str!("../../../../config/economy.json")).unwrap(),
        Engine::from_json(include_str!("../../../../config/achievements.json")).unwrap(),
        ChaCha8Rng::seed_from_u64(11),
        Box::new(move || {
            let at = clock_now.load(Ordering::SeqCst);
            Clock { now: at, day: day_of(at) }
        }),
    )
    .unwrap();
    g.set_artifacts(ArtifactsConfig::from_json(include_str!("../../../../config/artifacts.json")).unwrap());
    let advance = |seconds: i64| now.fetch_add(seconds, Ordering::SeqCst);
    let mut choices = ChaCha8Rng::seed_from_u64(12);

    // The boosters on sale, the newest sets first; the old sets apart.
    g.ensure_prices();
    let released = g.release_dates.clone().unwrap_or_default();
    let mut shop = g.shop().unwrap();
    shop.sort_by(|a, b| released.get(&b.set_code).cmp(&released.get(&a.set_code)));
    let recent: Vec<String> = {
        let mut codes: Vec<String> = Vec::new();
        for item in &shop {
            if !codes.contains(&item.set_code) && codes.len() < 15 {
                codes.push(item.set_code.clone());
            }
        }
        codes
    };
    let old: Vec<String> = shop
        .iter()
        .filter(|i| released.get(&i.set_code).is_some_and(|d| d.as_str() < "2000-01-01"))
        .map(|i| i.set_code.clone())
        .collect();

    let mut known: BTreeMap<String, i64> = BTreeMap::new();
    let mut opened = 0u64;
    println!("\n=== {} ({} min a day, by {}) ===", profile.name, profile.session_minutes, profile.batch);
    let days = days();
    for day in 0..days {
        now.store(start + day * DAY, Ordering::SeqCst);
        let _ = g.collect_income();
        let _ = g.claim_charity();
        let session_end = start + day * DAY + profile.session_minutes * MINUTE;
        let mut income_at = now.load(Ordering::SeqCst);
        let mut touched: HashSet<String> = HashSet::new();
        while now.load(Ordering::SeqCst) < session_end {
            // The artifacts on sale, once they can be afforded comfortably.
            let balance = g.store.balance().unwrap();
            for a in g.artifacts("en").into_iter().filter(|a| a.status == "shop") {
                let price = i64::from(a.price_cents.unwrap_or(0));
                if balance as f64 >= price as f64 * profile.shop_reserve {
                    let _ = g.buy_artifact(&a.id, "en");
                }
            }
            // A booster: a recent set mostly, sometimes an old one; a Collector now and then.
            let pool = if !old.is_empty() && choices.gen_bool(profile.old_share) { &old } else { &recent };
            let set = &pool[choices.gen_range(0..pool.len())];
            let kinds: Vec<&ShopItem> = shop.iter().filter(|i| &i.set_code == set).collect();
            let collector = kinds.iter().find(|i| i.kind == COLLECTOR_KIND);
            let main = kinds.iter().find(|i| i.kind != COLLECTOR_KIND).or(collector);
            let item = match (collector, main) {
                (Some(c), _) if choices.gen_bool(profile.collector_share) => *c,
                (_, Some(m)) => *m,
                _ => continue,
            };
            let price = g.booster_price(&item.set_code, &item.kind);
            if g.store.balance().unwrap() < i64::from(price) * i64::from(profile.batch) {
                // Short of Seals: the duplicates go, then the player waits for the income.
                for code in touched.drain() {
                    let _ = g.sell_duplicates(&code);
                }
                if g.store.balance().unwrap() < i64::from(price) * i64::from(profile.batch) {
                    advance(5 * MINUTE);
                    let _ = g.collect_income();
                    income_at = now.load(Ordering::SeqCst);
                    continue;
                }
            }
            g.buy_boosters(&item.set_code, &item.kind, profile.batch).unwrap();
            if profile.batch == 1 {
                g.open_booster(&item.set_code, &item.kind).unwrap();
            } else {
                g.open_boosters(&item.set_code, &item.kind, profile.batch).unwrap();
            }
            opened += u64::from(profile.batch);
            touched.insert(item.set_code.clone());
            advance(profile.seconds_per_booster * i64::from(profile.batch));
            if now.load(Ordering::SeqCst) - income_at >= 10 * MINUTE {
                let _ = g.collect_income();
                income_at = now.load(Ordering::SeqCst);
            }
        }
        // The evening ends: the duplicates of the sets opened are sold, the best cards kept.
        for code in touched.drain() {
            let _ = g.sell_duplicates(&code);
        }
        g.flush_reveal();
        for (id, _, source) in g.store.artifacts().unwrap() {
            if !known.contains_key(&id) {
                known.insert(id.clone(), day + 1);
                println!("day {:>3}: {id} ({source}) · {} artifacts, {opened} boosters", day + 1, known.len());
            }
        }
        if (day + 1) % 10 == 0 {
            let income = g.collect_income().unwrap();
            println!(
                "-- day {:>3}: {opened} boosters, balance {}, collection {} ({:.0}/min), {} achievements",
                day + 1,
                g.store.balance().unwrap(),
                income.collection_value_cents,
                income.per_minute_cents,
                g.achievements("en").unwrap().unlocked_count
            );
            let mut totals: Vec<(String, i64)> = g.store.ledger_totals().unwrap().into_iter().collect();
            totals.sort_by_key(|(_, amount)| -amount.abs());
            println!("   ledger: {totals:?}");
        }
    }
    let missing: Vec<String> =
        g.artifacts("en").into_iter().filter(|a| !known.contains_key(&a.id)).map(|a| a.id).collect();
    println!("after {days} days: {} artifacts; missing: {missing:?}", known.len());
}

#[test]
#[ignore = "needs the data built in dist/, and minutes of play"]
fn pace_of_a_casual_player() {
    play(&CASUAL);
}

#[test]
#[ignore = "needs the data built in dist/, and minutes of play"]
fn pace_of_a_grinder() {
    play(&GRINDER);
}

/// The chance of "at least one foil card" on every booster on sale, and which are left
/// unpriced (certain, or impossible). `cargo test --release -p unsealed-app foil_bet -- --ignored --nocapture`.
#[test]
#[ignore = "needs the data built in dist/"]
fn foil_bet_on_every_booster() {
    use unsealed_core::betting::{Bet, CardFilter, Clause, Compare, Finish};
    let dist = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dist");
    let mut g = Game::with_parts(
        dist,
        Store::open_in_memory().unwrap(),
        EconomyConfig::from_json(include_str!("../../../../config/economy.json")).unwrap(),
        Engine::from_json(include_str!("../../../../config/achievements.json")).unwrap(),
        ChaCha8Rng::seed_from_u64(11),
        Box::new(|| Clock { now: 1_792_000_000, day: "2026-10-23".into() }),
    )
    .unwrap();
    let bet = Bet {
        clauses: vec![Clause::Cards { compare: Compare::AtLeast, count: 1, filter: CardFilter { finish: Finish::Foil, ..Default::default() } }],
    };
    let mut by_case: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for item in g.shop().unwrap() {
        let chance = g.bet_chance(&item.set_code, &item.kind, &bet).unwrap();
        let case = match chance.probability {
            Some(_) => "priced",
            None => "unpriced",
        };
        by_case.entry(case).or_default().push(format!("{} {} {:?}", item.set_code, item.kind, chance.probability.map(|p| format!("{:.4}%", p * 100.0))));
    }
    for (case, items) in &by_case {
        println!("\n{case} ({}):", items.len());
        for i in items {
            println!("  {i}");
        }
    }
}
