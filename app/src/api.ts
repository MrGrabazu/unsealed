import { invoke } from "@tauri-apps/api/core";

// Mirrors the structs serialized by the Rust side (src-tauri/src/game.rs).

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
  /** Market price in USD cents, if known. */
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
}

export interface CollectionView {
  setCode: string;
  setName: string;
  ownedCount: number;
  totalCount: number;
  completionPercent: number;
  cards: CollectionCard[];
}

export const getCollection = (setCode: string) =>
  invoke<CollectionView>("get_collection", { setCode });

export const listSets = () => invoke<SetSummary[]>("list_sets");

export const openBooster = (setCode: string, kind: string) =>
  invoke<OpenedCard[]>("open_booster", { setCode, kind });

export const formatUsd = (cents: number) => `$${(cents / 100).toFixed(2)}`;
