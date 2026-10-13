import { invoke } from "@tauri-apps/api/core";

// Mirrors the structs serialized by the Rust side (src-tauri/src/game.rs).
// All amounts are integer cents of the configured currency.

export interface SetSummary {
  code: string;
  name: string;
  releaseDate: string;
  boosters: string[];
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
}

export interface SealedStack {
  setCode: string;
  setName: string;
  kind: string;
  quantity: number;
}

export interface WalletView {
  balanceCents: number;
  currency: { code: string; symbol: string };
  charity: { amountCents: number; available: boolean };
  sealed: SealedStack[];
}

export interface ShopItem {
  setCode: string;
  setName: string;
  kind: string;
  priceCents: number;
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

export const listSets = () => invoke<SetSummary[]>("list_sets");
export const getWallet = () => invoke<WalletView>("get_wallet");
export const getShop = () => invoke<ShopItem[]>("get_shop");
export const buyBoosters = (setCode: string, kind: string, quantity: number) =>
  invoke<void>("buy_boosters", { setCode, kind, quantity });
export const openBooster = (setCode: string, kind: string) =>
  invoke<OpenedCard[]>("open_booster", { setCode, kind });
export const getCollection = (setCode: string) =>
  invoke<CollectionView>("get_collection", { setCode });
export const sellCards = (requests: SaleRequest[]) => invoke<SaleResult>("sell_cards", { requests });
export const sellDuplicates = (setCode: string) => invoke<SaleResult>("sell_duplicates", { setCode });
export const claimCharity = () => invoke<void>("claim_charity");
