import { invoke as tauriInvoke, type InvokeArgs } from "@tauri-apps/api/core";
import { withCachedImages } from "./images";

/** A command of the game; the card images of its answer come through the cache on disk. */
const invoke = <T>(command: string, args?: InvokeArgs) => tauriInvoke<T>(command, args).then(withCachedImages);

// Mirrors the structs serialized by the Rust side (src-tauri/src/game.rs).
// All amounts are integer cents of the market currency; the interface shows them in Seals.

export interface SetSummary {
  code: string;
  /** In the card language when translated, else in English. */
  name: string;
  englishName: string;
  releaseDate: string;
  boosters: string[];
  /** Art crop of the set's featured card. Must be shown unaltered, with its artist. */
  art: { imageUrl: string; cardName: string; artist: string } | null;
  /** Expansion symbol (black SVG), shown unaltered. */
  iconUrl: string | null;
}

export interface OpenedCard {
  uuid: string;
  name: string;
  number: string;
  rarity: string;
  setCode: string;
  scryfallId: string;
  foil: boolean;
  /** A special foil ("galaxyfoil", "etched"…, see foils.tsx), when drawn foil. */
  foilKind?: string;
  sheet: string;
  imageUrl: string;
  /** Back face, for double-faced cards. */
  backImageUrl: string | null;
  /** Market price, if known. */
  priceCents: number | null;
  /** The card has no market price of its own: the price is estimated. */
  priceEstimated: boolean;
  /** What the Boosterium pays for it (0 = cannot be sold), to sell it right after opening. */
  sellValueCents: number;
  /** First time the player owns this card (in any finish). */
  isNew: boolean;
  /** A foil of a card the player only owned in non-foil until now. */
  newFoil: boolean;
}

export interface CollectionCard {
  uuid: string;
  name: string;
  number: string;
  rarity: string;
  imageUrl: string;
  /** Back face, for double-faced cards. */
  backImageUrl: string | null;
  quantity: number;
  foilQuantity: number;
  /** Its foil's kind, when special (see foils.tsx). */
  foilKind?: string;
  priceCents: number | null;
  foilPriceCents: number | null;
  priceEstimated: boolean;
  /** What the Boosterium pays for one copy (0 = cannot be sold). */
  sellValueCents: number;
  foilSellValueCents: number;
  /** A guest's (or a card from other sets' boosters): the sets whose boosters give it, the
   * first one's file holding it. */
  boosters?: string[];
}

/** A card of a cycle: the best copy owned of its printings, else its regular printing. */
export interface CycleSlot {
  card: CollectionCard;
  owned: boolean;
  /** The copy shown is a foil. */
  foil: boolean;
  /** Owned in its most premium printing and finish. */
  prestige: boolean;
  /** How many printings the slot has, the regular one included. */
  printings: number;
}

/** A cycle of a set (the MTG Wiki's, or one of ours). */
export interface CycleView {
  id: string;
  name: string;
  custom: boolean;
  /** A cycle, or a story told card after card (Story Spotlights). */
  kind: "cycle" | "story";
  owned: number;
  total: number;
  complete: boolean;
  /** Every card in its most premium printing. */
  prestige: boolean;
  slots: CycleSlot[];
}

export interface CyclesView {
  setCode: string;
  completed: number;
  total: number;
  slotsOwned: number;
  slotsTotal: number;
  cycles: CycleView[];
}

/** A card of a scene, at its place. */
export interface SceneCell {
  card: CollectionCard;
  owned: boolean;
  /** A foil copy is owned. */
  foil: boolean;
  /** The scene is on the card's back face. */
  back: boolean;
}

/** One artwork spread over several cards of a set, which form it laid edge to edge. */
export interface SceneView {
  id: string;
  name: string;
  columns: number;
  rows: number;
  owned: number;
  total: number;
  complete: boolean;
  /** Every card owned in foil. */
  allFoil: boolean;
  /** Row by row. */
  cells: SceneCell[];
}

export interface ScenesView {
  setCode: string;
  completed: number;
  total: number;
  cardsOwned: number;
  cardsTotal: number;
  scenes: SceneView[];
}

/** An augment or a host of a set (Unstable), as far as the player is. */
export interface AssemblyPart {
  card: CollectionCard;
  owned: boolean;
  /** A foil copy is owned. */
  foil: boolean;
}

/** A set's assemblies: each augment laid over each host makes a creature, assembled once
 * both are owned. */
export interface AssembliesView {
  setCode: string;
  augments: AssemblyPart[];
  hosts: AssemblyPart[];
  assembled: number;
  total: number;
}

/** In a set's completion panel: a guest series its boosters give, with its own binder. */
export interface GuestLink {
  code: string;
  name: string;
  owned: number;
  total: number;
}

export interface CollectionView {
  setCode: string;
  setName: string;
  ownedCount: number;
  totalCount: number;
  completionPercent: number;
  cards: CollectionCard[];
  /** Cards only collector boosters give; they do not count toward completion. */
  extras: CollectionCard[];
  extrasOwned: number;
  /** The set's tokens (one in every booster): outside the completion target. */
  tokens: CollectionCard[];
  tokensOwned: number;
  /** The set's art cards (`foilQuantity`: their gold signed version): outside completion. */
  arts: CollectionCard[];
  artsOwned: number;
  /** Cards of other sets its boosters give, for a set that keeps them (the Mystery
   * Boosters); elsewhere they have the binders of their series (`guestLinks`). */
  guests: CollectionCard[];
  guestsOwned: number;
  /** The set's own cards that only other sets' boosters give: outside completion. */
  elsewhere: CollectionCard[];
  elsewhereOwned: number;
  /** The cards its box toppers give: outside completion. */
  toppers: CollectionCard[];
  toppersOwned: number;
  guestLinks: GuestLink[];
}

/** How far the box of a booster is, when it holds a box topper: boosters opened toward it. */
export interface BoxProgress {
  opened: number;
  /** Boosters in the box. */
  size: number;
  /** Box topper packs the box gives. */
  packs: number;
}

/** Box toppers just given (a box of boosters completed), to show the player. */
export interface ToppersGiven {
  setCode: string;
  setName: string;
  /** The topper's booster kind ("box-topper"…). */
  kind: string;
  quantity: number;
}

/** What the collection earns by itself. */
export interface IncomeView {
  /** What the collection would sell for (real market prices only). */
  collectionValueCents: number;
  /** What it earns each minute, in cents (with their fraction). */
  perMinuteCents: number;
  /** Paid by this call. */
  earnedCents: number;
  /** The time this call paid for, in minutes (capped). */
  minutes: number;
  /** The share of its value it earns a minute (smaller as it grows). */
  percentPerMinute: number;
  /** The most it can earn a minute, the artifacts' bonuses counted (null: no limit). */
  maxPerMinuteCents: number | null;
  offlineCapMinutes: number;
}

// Bets (see `unsealed_core::betting`): every condition of a bet must hold.
export type Compare = "atLeast" | "exactly" | "atMost";
export type Finish = "any" | "normal" | "foil";
export type CardKind = "cards" | "tokens" | "artCards" | "all";
export type CardType =
  | "land"
  | "basicLand"
  | "creature"
  | "instant"
  | "sorcery"
  | "artifact"
  | "enchantment"
  | "planeswalker"
  | "battle";
export type ColorChoice = "white" | "blue" | "black" | "red" | "green" | "colorless" | "multicolor";
export type Treatment = "any" | "fullArt" | "specialFrame";

/** Which cards a condition counts; every criterion is optional. */
export interface CardFilter {
  /** A given card's uuid. */
  card?: string | null;
  kind?: CardKind;
  rarities?: string[];
  types?: CardType[];
  colors?: ColorChoice[];
  finish?: Finish;
  treatment?: Treatment;
  minCents?: number | null;
  maxCents?: number | null;
}

export type Clause =
  | { type: "cards"; compare: Compare; count: number; filter: CardFilter }
  | { type: "value"; compare: Compare; cents: number };

export interface Bet {
  clauses: Clause[];
}

/** A bet of the slip. `active: false`: paused, kept in the slip but not placed. */
export interface BetRequest {
  bet: Bet;
  stakeCents: number;
  active?: boolean;
}

/** A bet kept from one opening to the next: on `setCode`'s boosters only (always for one
 * naming a card), or on every booster when null. */
export interface KeptBet extends BetRequest {
  setCode: string | null;
  /** The card it names, as shown ("Name (#61)"). */
  cardName: string | null;
}

/** A bet left out of an opening (no stake taken), and why. */
export interface SkippedBet {
  bet: Bet;
  /** Nothing to price on this booster, a stake under the minimum, or no money for it. */
  reason: "unpriced" | "stake" | "funds";
}

/** What a bet slip needs to know of a booster. */
export interface BetContext {
  boosterPriceCents: number;
  houseMarginPercent: number;
  minStakeCents: number;
}

/** How likely a bet is: `null` when it cannot be priced (certain either way, or too rare). */
export interface BetChance {
  probability: number | null;
  /** Worked out from the sheets (else over simulated boosters). */
  exact: boolean;
  /** Why there are no odds: won by every booster of this kind, by none, or too rare (or too
   * sure) for the simulated boosters. */
  unpriced: "always" | "never" | "simulation" | null;
}

export interface SettledBet {
  bet: Bet;
  stakeCents: number;
  /** The chance it had of winning. */
  probability: number;
  won: boolean;
  /** What it pays back, the stake included (0 when lost). */
  payoutCents: number;
}

/** An opened booster: its cards, the bets it settled, and those left out (not priceable on
 * this booster: no stake taken). */
export interface OpenedBooster {
  cards: OpenedCard[];
  bets: SettledBet[];
  skipped: SkippedBet[];
  /** The artifact found in it, if any: shown at the end of the opening. */
  artifact: ArtifactView | null;
}

/** An artifact of the cabinet. One neither owned nor on sale keeps its secrets: no name,
 * effect nor flavour, only its drawing (shown as a silhouette) and its source's kind. */
export interface ArtifactView {
  id: string;
  number: number;
  rarity: "common" | "uncommon" | "rare" | "mythic";
  glyph: string;
  status: "owned" | "shop" | "hidden";
  name: string | null;
  effect: string | null;
  flavor: string | null;
  source: "shop" | "booster" | "achievement" | "allOthers";
  priceCents: number | null;
  acquiredAt: number | null;
}

/** A binder on the library's shelves: a set the player owns cards of. */
export interface LibraryBinder {
  setCode: string;
  ownedCount: number;
  totalCount: number;
  completionPercent: number;
  raresOwned: number;
  raresTotal: number;
  mythicsOwned: number;
  mythicsTotal: number;
  extrasOwned: number;
  extrasTotal: number;
  tokensOwned: number;
  tokensTotal: number;
  artsOwned: number;
  artsTotal: number;
  guestsOwned: number;
  guestsTotal: number;
  copies: number;
  foilCopies: number;
  /** Market value of every copy, and what the Boosterium would pay for them. */
  valueCents: number;
  sellValueCents: number;
  /** The most valuable card owned (finest finish owned). */
  bestCard: { name: string; foil: boolean; priceCents: number } | null;
  boostersOpened: number;
}

/** An artist with cards in the sets offered. */
export interface ArtistSummary {
  name: string;
  cardCount: number;
  ownedCount: number;
}

/** One of an artist's cards, with the set it comes from. */
export interface ArtistCard extends CollectionCard {
  setCode: string;
  setName: string;
  releaseDate: string;
}

/** An artist's collection, oldest card first. */
export interface ArtistView {
  name: string;
  ownedCount: number;
  totalCount: number;
  completionPercent: number;
  cards: ArtistCard[];
}

export interface SealedStack {
  setCode: string;
  setName: string;
  kind: string;
  quantity: number;
  /** What the player paid for the whole stack (0 for free boosters). */
  costCents: number;
  /** Current price of one booster. */
  priceCents: number;
  /** What the Boosterium pays today for one booster. */
  sellValueCents: number;
  /** Recent prices of one booster, oldest first. */
  history: number[];
  /** A box topper's pack: given with a box, no price, never sold. */
  topper: boolean;
  /** When its box holds a box topper: how far the next box is. */
  boxProgress?: BoxProgress;
}

export interface BoosterSaleResult {
  quantity: number;
  totalCents: number;
  /** Proceeds minus the cost of the sold boosters. */
  profitCents: number;
}

export interface WalletView {
  balanceCents: number;
  /** How amounts are shown (in Seals, see money.tsx). */
  currency: { sealsPerCent: number };
  charity: { amountCents: number; available: boolean };
  sealed: SealedStack[];
  /** What the artifacts owned change in amounts the interface works out itself. */
  /** What the artifacts change in amounts the interface works out: the duplicates' bonus, and
   * the best share of a card's market price it can reach. */
  artifactPerks: { duplicatesPercent: number; maxSellRatePercent: number; crowned: boolean };
}

export interface ShopItem {
  setCode: string;
  setName: string;
  kind: string;
  priceCents: number;
  /** What the Boosterium would pay today for such a booster. */
  sellBackCents: number;
  /** Recent prices, oldest first. */
  history: number[];
  /** When its box holds a box topper: how far the next box is. */
  boxProgress?: BoxProgress;
}

export interface SaleRequest {
  uuid: string;
  foil: boolean;
  quantity: number;
}

export interface SaleResult {
  soldCount: number;
  totalCents: number;
}

export interface AchievementView {
  id: string;
  /** Position in the achievement collection (1-based). */
  number: number;
  name: string;
  /** Hidden (null) for a locked secret achievement. */
  description: string | null;
  secret: boolean;
  rarity: "common" | "uncommon" | "rare" | "mythic";
  emblem: string;
  palette: string;
  unlocked: boolean;
  unlockedAt: number | null;
  progress: { current: number; goal: number } | null;
  reward: RewardView | null;
}

/** What an achievement pays, worded by the interface in its language. */
export interface RewardView {
  cashCents: number;
  /** `setName` is null for "the newest set" when no data is available yet. */
  sealed: { setName: string | null; kind: string; quantity: number } | null;
}

export interface AchievementsView {
  total: number;
  unlockedCount: number;
  secretRemaining: number;
  items: AchievementView[];
}

export interface OpeningView {
  id: number;
  /** Unix time, in seconds. */
  openedAt: number;
  setCode: string;
  setName: string;
  kind: string;
  /** Market value of its cards when it was opened. */
  valueCents: number;
  /** Its most valuable card, priced when it was pulled. */
  bestCard: OpenedCard | null;
}

export interface BoosterStatsView {
  setCode: string;
  setName: string;
  kind: string;
  boosters: number;
  valueCents: number;
  averageCents: number;
  bestValueCents: number;
}

export interface StatsView {
  boostersOpened: number;
  cardsPulled: number;
  foilsPulled: number;
  /** Market value of every pulled card, when it was pulled. */
  valuePulledCents: number;
  spentOnBoostersCents: number;
  earnedFromCardsCents: number;
  earnedFromBoostersCents: number;
  rewardsCents: number;
  charityCents: number;
  /** Staked on bets, and what the winning ones paid back. */
  betsStakedCents: number;
  betsWonCents: number;
  balanceCents: number;
  /** Distinct cards owned (any finish). */
  distinctCards: number;
  /** Today's market value of the collection. */
  collectionValueCents: number;
  /** What the Boosterium would pay for it, on real market prices only (as in the header). */
  collectionSellValueCents: number;
  /** What the Boosterium would pay today for the sealed boosters. */
  sealedValueCents: number;
  bestBooster: OpeningView | null;
  worstBooster: OpeningView | null;
  recentOpenings: OpeningView[];
  /** Cards priced when they were pulled. */
  bestPulls: { card: OpenedCard; openedAt: number }[];
  boosters: BoosterStatsView[];
}

/** A card's details: local data, plus Scryfall's when it could be reached (`online`). */
export type Reading = "right" | "left" | "flip";

export interface CardDetailsView {
  uuid: string;
  setCode: string;
  setName: string;
  name: string;
  /** The card language, or "en" when the card was not printed in it. */
  shownLanguage: string;
  number: string;
  rarity: string;
  imageUrl: string;
  backImageUrl: string | null;
  /** How the front is turned to be read, when printed sideways: a quarter turn clockwise
   * ("right": split cards and Rooms, planes, battles), anticlockwise ("left": aftermath), or
   * half a turn ("flip": Kamigawa flip cards). */
  reading?: Reading;
  artist: string | null;
  canBeFoil: boolean;
  canBeNonfoil: boolean;
  /** Its foil's kind, when special (see foils.tsx). */
  foilKind?: string;
  priceCents: number | null;
  foilPriceCents: number | null;
  priceEstimated: boolean;
  sellValueCents: number;
  foilSellValueCents: number;
  owned: number;
  ownedFoil: number;
  faces: {
    name: string;
    manaCost: string | null;
    typeLine: string | null;
    text: string | null;
    flavorText: string | null;
    /** "3/4", a loyalty or a defense. */
    stats: string | null;
  }[];
  printings: {
    setCode: string;
    setName: string;
    number: string;
    rarity: string;
    releasedAt: string | null;
    imageUrl: string | null;
    /** Today's market prices on Scryfall. */
    priceCents: number | null;
    foilPriceCents: number | null;
    inGame: boolean;
    owned: number;
    ownedFoil: number;
    current: boolean;
  }[];
  online: boolean;
}

/** The wallet's movements, newest first. */
export interface HistoryView {
  entries: HistoryEntry[];
  hasMore: boolean;
}

export interface HistoryEntry {
  /** Unix time, in seconds. */
  at: number;
  /** "booster_purchase", "card_sale", "booster_sale", "sale_undone", "purchase_undone",
   * "charity", "achievement_reward", "starting_balance"... */
  kind: string;
  /** Signed, as in the wallet. */
  amountCents: number;
  /** Purchases and sales recorded in detail (not the oldest ones). */
  transactionId: number | null;
  items: {
    /** The card's name, or the set's name for boosters. */
    name: string;
    setCode: string;
    boosterKind: string | null;
    foil: boolean;
    quantity: number;
    unitCents: number;
  }[];
  undoneAt: number | null;
  undoCents: number | null;
  /** Whether, and at what price, it can be undone now. */
  undo: {
    /** What undoing a sale costs, or undoing a purchase refunds. */
    amountCents: number;
    penaltyPercent: number;
    affordable: boolean;
    /** For a purchase: until when (Unix time, seconds). */
    until: number | null;
  } | null;
}

/** A game (its own collection, wallet and achievements), saved automatically. */
export interface SaveInfo {
  id: string;
  name: string;
  /** Unix times, in seconds. */
  createdAt: number;
  lastPlayedAt: number;
}

export interface DataInfo {
  setCount: number;
  boosterCount: number;
  pricesUpdatedAt: string | null;
  /** Unix time of the last data build, in seconds. */
  builtAt: number | null;
}

export type DataStatus =
  | { state: "checking" }
  | { state: "upToDate" }
  | { state: "updating"; message: string; /** Estimate, 0 to 100. */ percent: number }
  | { state: "failed"; message: string };

export const listSaves = () => invoke<SaveInfo[]>("list_saves");
export const currentSave = () => invoke<SaveInfo | null>("current_save");
export const newGame = (name: string) => invoke<SaveInfo>("new_game", { name });
export const loadGame = (id: string) => invoke<SaveInfo>("load_game", { id });
export const createSavePoint = (name: string) => invoke<SaveInfo>("create_save_point", { name });
export const renameSave = (id: string, name: string) => invoke<SaveInfo>("rename_save", { id, name });
export const deleteSave = (id: string) => invoke<void>("delete_save", { id });
export const getDataInfo = () => invoke<DataInfo>("get_data_info");
/** Language of card names, card images and set names (English when not printed in it). */
export const setCardLanguage = (lang: string) => invoke<void>("set_card_language", { lang });
export const getDataStatus = () => invoke<DataStatus>("get_data_status");
/** Prepares the data again after a failure (nothing happens otherwise). */
export const retryDataUpdate = () => invoke<void>("retry_data_update");
/** `lang`: the interface language, for the achievements' names and descriptions. */
export const getAchievements = (lang: string) => invoke<AchievementsView>("get_achievements", { lang });
export const takeUnlocks = (lang: string) => invoke<AchievementView[]>("take_unlocks", { lang });
export const takeBoxToppers = () => invoke<ToppersGiven[]>("take_box_toppers");
export const getArtifacts = (lang: string) => invoke<ArtifactView[]>("get_artifacts", { lang });
export const buyArtifact = (id: string, lang: string) => invoke<ArtifactView>("buy_artifact", { id, lang });
/** Artifacts given since the last call (by achievements), to celebrate. */
export const takeGivenArtifacts = (lang: string) => invoke<ArtifactView[]>("take_given_artifacts", { lang });
export const listSets = () => invoke<SetSummary[]>("list_sets");
export const getWallet = () => invoke<WalletView>("get_wallet");
export const getShop = () => invoke<ShopItem[]>("get_shop");
export const buyBoosters = (setCode: string, kind: string, quantity: number) =>
  invoke<void>("buy_boosters", { setCode, kind, quantity });
export const sellBoosters = (setCode: string, kind: string, quantity: number) =>
  invoke<BoosterSaleResult>("sell_boosters", { setCode, kind, quantity });
/** `animated`: the player watches the opening (false for a quick opening). `bets`: placed on
 * the booster, their stakes paid now and their winnings once every card is seen. */
export const openBooster = (setCode: string, kind: string, animated: boolean, bets: BetRequest[]) =>
  invoke<OpenedBooster>("open_booster", { setCode, kind, animated, bets });
/** Several boosters, the same `bets` on each. */
export const openBoosters = (setCode: string, kind: string, quantity: number, bets: BetRequest[]) =>
  invoke<OpenedBooster[]>("open_boosters", { setCode, kind, quantity, bets });
/** The sets pinned at the top of the Boosterium, in the order they were pinned. */
export const getPinnedSets = () => invoke<string[]>("get_pinned_sets");
export const setPinnedSets = (codes: string[]) => invoke<void>("set_pinned_sets", { codes });
/** Pays what the collection earned since the last call. */
export const collectIncome = () => invoke<IncomeView>("collect_income");
export const getBetContext =(setCode: string, kind: string) => invoke<BetContext>("get_bet_context", { setCode, kind });
export const getBetChance = (setCode: string, kind: string, bet: Bet) =>
  invoke<BetChance>("get_bet_chance", { setCode, kind, bet });
export const getKeptBets = () => invoke<KeptBet[]>("get_kept_bets");
export const setKeptBets = (bets: KeptBet[]) => invoke<void>("set_kept_bets", { bets });
/** The player turned over `count` cards of the booster being opened (their achievements
 * happen then). A large count reveals everything left. */
export const revealCards = (count: number) => invoke<void>("reveal_cards", { count });
export const getCollection = (setCode: string) =>
  invoke<CollectionView>("get_collection", { setCode });
/** Asks which translated images of a set are good scans (the others show in English);
 * returns how many good ones were found (0 when nothing was left to check). */
export const checkSetImages = (setCode: string) => invoke<number>("check_set_images", { setCode });
/** Downloads, in the background, the card images of the sealed boosters in stock (how many
 * images were asked for). */
export const prefetchSealedImages = () => invoke<number>("prefetch_sealed_images");
/** Every card owned, across the sets, with the set it comes from. */
export const getOwnedCards = () => invoke<ArtistCard[]>("get_owned_cards");
export const getLibrary = () => invoke<LibraryBinder[]>("get_library");
/** A set's cycles, as far as the player is, their names in `lang`. */
export const getCycles = (setCode: string, lang: string) => invoke<CyclesView>("get_cycles", { setCode, lang });

/** A set's scenes, as far as the player is, their names in `lang`. */
export const getScenes = (setCode: string, lang: string) => invoke<ScenesView>("get_scenes", { setCode, lang });
export const getAssemblies = (setCode: string) => invoke<AssembliesView>("get_assemblies", { setCode });
export const listArtists = () => invoke<ArtistSummary[]>("list_artists");
/** The guest series with a binder of their own (no boosters, no art). */
export const listGuestSeries = () => invoke<SetSummary[]>("list_guest_series");
export const getArtistCollection = (name: string) => invoke<ArtistView>("get_artist_collection", { name });
export const getHistory =(limit: number, offset: number) => invoke<HistoryView>("get_history", { limit, offset });
/** Returns the money moved (negative: what undoing a sale cost). */
export const undoTransaction = (id: number) => invoke<number>("undo_transaction", { id });
export const getCardDetails =(setCode: string, uuid: string) =>
  invoke<CardDetailsView>("get_card_details", { setCode, uuid });
export const sellCards =(requests: SaleRequest[]) => invoke<SaleResult>("sell_cards", { requests });
/** Sells a binder's duplicates (one copy of each card kept), saying which copies went. */
export const sellDuplicates = (setCode: string) =>
  invoke<SaleResult & { sold: SaleRequest[] }>("sell_duplicates", { setCode });
/** What selling a binder's duplicates would bring in, without selling them. */
export const getDuplicatesValue = (setCode: string) => invoke<SaleResult>("get_duplicates_value", { setCode });
export const claimCharity = () => invoke<void>("claim_charity");
export const getStats = () => invoke<StatsView>("get_stats");
