import { invoke } from "@tauri-apps/api/core";

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
  sheet: string;
  imageUrl: string;
  /** Back face, for double-faced cards. */
  backImageUrl: string | null;
  /** Market price, if known. */
  priceCents: number | null;
  /** First time the player owns this card (in any finish). */
  isNew: boolean;
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
  priceCents: number | null;
  foilPriceCents: number | null;
  /** What the Boosterium pays for one copy (0 = cannot be sold). */
  sellValueCents: number;
  foilSellValueCents: number;
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
  currency: { sealsPerCent: number; silverFrom: number; goldFrom: number };
  charity: { amountCents: number; available: boolean };
  sealed: SealedStack[];
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
  balanceCents: number;
  /** Distinct cards owned (any finish). */
  distinctCards: number;
  /** Today's market value of the collection. */
  collectionValueCents: number;
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
  artist: string | null;
  canBeFoil: boolean;
  canBeNonfoil: boolean;
  priceCents: number | null;
  foilPriceCents: number | null;
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
/** `lang`: the interface language, for the achievements' names and descriptions. */
export const getAchievements = (lang: string) => invoke<AchievementsView>("get_achievements", { lang });
export const takeUnlocks = (lang: string) => invoke<AchievementView[]>("take_unlocks", { lang });
export const listSets = () => invoke<SetSummary[]>("list_sets");
export const getWallet = () => invoke<WalletView>("get_wallet");
export const getShop = () => invoke<ShopItem[]>("get_shop");
export const buyBoosters = (setCode: string, kind: string, quantity: number) =>
  invoke<void>("buy_boosters", { setCode, kind, quantity });
export const sellBoosters = (setCode: string, kind: string, quantity: number) =>
  invoke<BoosterSaleResult>("sell_boosters", { setCode, kind, quantity });
export const openBooster = (setCode: string, kind: string) =>
  invoke<OpenedCard[]>("open_booster", { setCode, kind });
export const openBoosters = (setCode: string, kind: string, quantity: number) =>
  invoke<OpenedCard[][]>("open_boosters", { setCode, kind, quantity });
export const getCollection = (setCode: string) =>
  invoke<CollectionView>("get_collection", { setCode });
export const getCardDetails = (setCode: string, uuid: string) =>
  invoke<CardDetailsView>("get_card_details", { setCode, uuid });
export const sellCards =(requests: SaleRequest[]) => invoke<SaleResult>("sell_cards", { requests });
export const sellDuplicates = (setCode: string) => invoke<SaleResult>("sell_duplicates", { setCode });
export const claimCharity = () => invoke<void>("claim_charity");
export const getStats = () => invoke<StatsView>("get_stats");
