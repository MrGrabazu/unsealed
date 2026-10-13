// English texts: the reference every other language must translate in full (enforced by
// the `Messages` type). `{name}` placeholders are filled at display time; keys ending in
// `_one` / `_other` are plural forms chosen by the count.
//
// Kept untranslated on purpose: the game's name ("Unsealed"), its tagline ("Crack &
// Collect"), the shop's name ("Boosterium"), and the Fan Content Policy notice (legal text,
// see legal.ts).

export const en = {
  // ---- common
  "common.back": "← Back",
  "common.loading": "Loading…",
  "common.cancel": "Cancel",
  "common.ok": "OK",
  "common.none": "—",

  // ---- booster kinds
  "kind.play": "Play booster",
  "kind.draft": "Draft booster",
  "kind.set": "Set booster",
  "kind.collector": "Collector booster",
  "kind.default": "Booster",
  "kind.other": "{kind} booster",

  // ---- rarities (counts in summaries)
  "rarity.common_one": "{count} common",
  "rarity.common_other": "{count} commons",
  "rarity.uncommon_one": "{count} uncommon",
  "rarity.uncommon_other": "{count} uncommons",
  "rarity.rare_one": "{count} rare",
  "rarity.rare_other": "{count} rares",
  "rarity.mythic_one": "{count} mythic",
  "rarity.mythic_other": "{count} mythics",
  "rarity.other_one": "{count} other",
  "rarity.other_other": "{count} others",

  // ---- data refresh
  "data.updating": "Updating card data and prices ({percent}%): {step}…",
  "data.failed": "Could not refresh card data ({reason}). Playing with the data already downloaded.",
  "data.none": "No card data yet. It is downloaded automatically when the app starts with a connection.",

  // ---- home screen
  "home.continue": "Continue",
  "home.newGame": "New game",
  "home.loadGame": "Load a game",
  "home.saveCount_one": "{count} save",
  "home.saveCount_other": "{count} saves",
  "home.options": "Options",
  "home.about": "About",
  "home.newTitle": "New game",
  "home.newHint":
    "A new collection and wallet, starting from scratch. Your other games are kept: each player on this computer can have their own.",
  "home.namePlaceholder": "Name",
  "home.defaultName": "Game {number}",
  "home.start": "Start",
  "home.loadTitle": "Load a game",
  "home.loadHint": "Games are saved automatically. Save points are copies you can come back to.",
  "home.playingNow": "playing now",
  "home.lastPlayed": "last played {date}",
  "home.created": "created {date}",
  "home.load": "Load",
  "home.rename": "Rename",
  "home.delete": "Delete",
  "home.deleteConfirm": "Delete for good?",

  // ---- game screen
  "game.loading": "Loading your game…",
  "game.nameTitle": "The game being played (saved automatically)",
  "game.balanceTitle": "Your balance",
  "game.savePoint": "Save point",
  "game.savePointTitle": "Every action is saved automatically. This keeps a copy to come back to.",
  "game.savePointName": "{game} — {date}",
  "game.savePointCreated": "Save point “{name}” created. Load it any time from the menu.",
  "game.menu": "☰ Menu",
  "tab.boosterium": "Boosterium",
  "tab.open": "Open boosters",
  "tab.collection": "Collection",
  "tab.achievements": "Achievements",
  "tab.stats": "Stats",
  "achievement.unlocked": "Achievement unlocked!",
  "achievement.reward": "Reward: {reward}",

  // ---- rewards
  "reward.cash": "+{amount}",
  "reward.boosters_one": "{count} × {set} {kind}",
  "reward.boosters_other": "{count} × {set} {kind}",
  "reward.newestSet": "newest set",
  "reward.join": " + ",

  // ---- Boosterium
  "shop.charity":
    "You have no money, no unopened booster and no card worth selling. The Boosterium offers {amount} — once per day, and only when you are really stranded.",
  "shop.charityAccept": "Accept {amount}",
  "shop.intro":
    "Buy sealed boosters, then open them. Prices follow the market value of the cards, so unopened boosters can gain or lose value: sell them back, or open them. Sell cards from your collection to fund the next ones.",
  "shop.search": "Search a set (name or code)",
  "shop.allYears": "All years",
  "shop.anyPrice": "Any price",
  "shop.under": "Under {amount}",
  "shop.noMatch": "No booster matches these filters.",
  "shop.released": "released {date}",
  "shop.boosterTypes_one": "{count} booster type",
  "shop.boosterTypes_other": "{count} booster types",
  "shop.from": "from {amount}",
  "shop.sellsBack": "sells back for {amount}",
  "shop.priceTitle": "Follows the market value of the cards",
  "shop.buy": "Buy {count}",
  "shop.buyAndOpen": "Buy & open",
  "art.credit": "Art: {card} by {artist} · © Wizards of the Coast",

  // ---- opening
  "open.noSealed": "You have no sealed booster left.",
  "open.goShopping": "Go to the Boosterium",
  "open.paid": "paid {amount}",
  "open.worth": "worth {amount} if sold",
  "open.openOne": "Open one",
  "open.openMany": "Open ×{count}",
  "open.sellOne": "Sell one ({amount})",
  "open.back": "← Back to my boosters",
  "open.next": "Open the next one ({count} left)",
  "open.soldBooster": "Sold 1 {set} booster for {amount} (profit {profit}).",
  "open.soldBoosterLoss": "Sold 1 {set} booster for {amount} (loss {loss}).",
  "open.revealed": "{revealed} / {total} revealed",
  "open.revealAll": "Reveal all",
  "open.boosterValue": "Booster value: {amount}",
  "open.clickToReveal": "Click the highlighted card to reveal it.",
  "open.new": "NEW",
  "batch.progress": "Booster {current} / {total}",
  "batch.skip": "Skip to summary",
  "batch.summary": "{boosters} boosters opened · {cards} cards",
  "batch.total": "Total value: {amount}",
  "batch.newCards": "{count} new",
  "batch.bestPulls": "Best pulls",

  // ---- price trend
  "trend.tracking": "tracking since today",
  "trend.title": "Last {count} prices, from {first} to {last}",

  // ---- collection
  "collection.allCards": "All cards",
  "collection.owned": "Owned",
  "collection.missing": "Missing",
  "collection.sellDuplicates": "Sell all duplicates",
  "collection.loading": "Loading the collection…",
  "collection.progress": "{owned} / {total} cards ({percent}%)",
  "collection.extras": "Collector extras",
  "collection.extrasProgress": "{owned} / {total} · only in collector boosters, not counted in completion",
  "collection.sell": "Sell {amount}",
  "collection.sellFoil": "Sell foil {amount}",
  "collection.sacrifice": "Sacrifice for {amount}?",
  "collection.sold": "Sold {card} for {amount}.",
  "collection.noDuplicates": "No duplicates to sell.",
  "collection.soldDuplicates_one": "Sold {count} duplicate for {amount}.",
  "collection.soldDuplicates_other": "Sold {count} duplicates for {amount}.",
  "collection.foilBadge": "FOIL",

  // ---- achievements
  "achievements.loading": "Loading achievements…",
  "achievements.all": "All achievements",
  "achievements.unlocked": "Unlocked",
  "achievements.locked": "Still to unlock",
  "achievements.secret": "Secret",
  "achievements.progress": "{unlocked} / {total} unlocked · {left} to go",
  "achievements.secretLeft": " ({count} secret)",
  "achievements.secretText": "Secret achievement. Keep playing to discover it.",
  "achievements.cardNumber": "No. {number}",
  "achievements.cardLabelUnlocked": "{name} (unlocked)",
  "achievements.cardLabelLocked": "{name} (locked)",

  // ---- stats
  "stats.loading": "Loading statistics…",
  "stats.empty": "No booster opened yet. Your statistics will show up here.",
  "stats.boostersOpened": "Boosters opened",
  "stats.cardsPulled": "Cards pulled",
  "stats.foils": " · {count} foil",
  "stats.valuePulled": "Value pulled",
  "stats.valuePulledHint": "Market value of the cards when they were pulled",
  "stats.average": "Average booster",
  "stats.spent": "Spent on boosters",
  "stats.return": "Return on boosters",
  "stats.returnHint": "Value pulled compared with what the boosters cost",
  "stats.earned": "Earned from sales",
  "stats.earnedHint": "Cards and sealed boosters sold to the Boosterium",
  "stats.rewards": "Rewards and help",
  "stats.rewardsHint": "Achievement rewards and daily help",
  "stats.distinct": "Distinct cards owned",
  "stats.collectionValue": "Collection value",
  "stats.collectionValueHint": "Today's market value of the cards you own",
  "stats.netWorth": "Net worth",
  "stats.netWorthHint": "Balance + collection at market value + sealed boosters if sold today",
  "stats.best": "Best booster ever",
  "stats.worst": "Worst booster ever",
  "stats.bestCard": " · best card: {card}",
  "stats.foilSuffix": " (foil)",
  "stats.bestPulls": "Best pulls of all time",
  "stats.bestPullsHint": "Priced when they were pulled.",
  "stats.byBooster": "By booster",
  "stats.colBooster": "Booster",
  "stats.colOpened": "Opened",
  "stats.colTotal": "Total value",
  "stats.colAverage": "Average",
  "stats.colBest": "Best",
  "stats.latest": "Latest openings",
  "stats.colDate": "Date",
  "stats.colValue": "Value",
  "stats.colBestCard": "Best card",

  // ---- options
  "options.title": "Options",
  "options.language": "Language",
  "options.languageHint": "The language of menus and texts.",
  "options.cardLanguage": "Card and set language",
  "options.cardLanguageAuto": "Same as the interface",
  "options.cardLanguageHint":
    "Card names and images, and set names, when they were printed in that language. Everything else stays in English, as with the oldest sets.",
  "options.data": "Game data",
  "options.sets": "Sets",
  "options.boosterTypes": "Booster types",
  "options.pricesFrom": "Card prices from",
  "options.lastUpdate": "Last update",
  "options.never": "never",
  "options.dataHint":
    "Card data and prices are downloaded in the background at launch and refreshed every day. Nothing to do on your side.",
  "options.more": "More options",
  "options.moreHint": "More settings will appear here in future versions.",

  // ---- about
  "about.title": "About Unsealed",
  "about.version": "version {version}",
  "about.gameTitle": "The game",
  "about.game1":
    "Unsealed recreates the thrill of opening trading card boosters, without spending a cent. Buy virtual boosters in the Boosterium, crack them open card by card, build your collection, complete sets and unlock achievements.",
  "about.game2":
    "Pull rates follow the real contents of each booster, and card values follow the real market every day. Like in real life, most boosters are worth less than they cost: the big pulls are what makes them exciting.",
  "about.freeTitle": "Free and virtual, for good",
  "about.free1": "Unsealed is entirely free: no purchase, no microtransaction, no advertising.",
  "about.free2":
    "Its currency, boosters and cards are virtual. They cannot be bought, sold or exchanged for anything real, and they have no monetary value.",
  "about.free3": "Market prices are shown for entertainment only.",
  "about.fcpTitle": "Fan Content Policy",
  "about.fcpOriginal": "Official notice (in English, as required):",
  "about.fcp1":
    "Unsealed is not affiliated with, endorsed, sponsored or approved by Wizards of the Coast. Magic: The Gathering, its card names, card images, mana and expansion symbols are trademarks and copyrighted materials of Wizards of the Coast LLC, used here under the Fan Content Policy. Card art belongs to Wizards of the Coast and to its artists, who are credited wherever art is shown on its own.",
  "about.fcp2": "The achievement cards are original creations of Unsealed and do not reproduce any Wizards of the Coast design.",
  "about.sourcesTitle": "Data sources",
  "about.sources1":
    "Card images, set symbols and prices are provided by Scryfall (scryfall.com). Unsealed is not produced by or endorsed by Scryfall.",
  "about.sources2": "Booster contents and pull rates come from MTGJSON (mtgjson.com).",

  // ---- footer
  "footer.sources": "Card images and prices are provided by Scryfall.",
};

export type MessageKey = keyof typeof en;
/** A complete translation: every English key, nothing more. */
export type Messages = { [K in MessageKey]: string };
