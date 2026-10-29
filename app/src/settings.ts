// Settings of the app (not of a game), kept in the browser storage of the window.

const OPENING_3D = "unsealed.opening3d";

/** Whether boosters open in the 3D scene (else the classic animation). On by default. */
export const getOpening3d = () => localStorage.getItem(OPENING_3D) !== "off";
export const setOpening3d = (on: boolean) => localStorage.setItem(OPENING_3D, on ? "on" : "off");

const SEASONAL = "unsealed.seasonal";

/** Whether the game dresses up for the feasts of the year (seasons.tsx). On by default. */
export const getSeasonalOn = () => localStorage.getItem(SEASONAL) !== "off";
export const setSeasonalOn = (on: boolean) => localStorage.setItem(SEASONAL, on ? "on" : "off");

const COLLECTION_VIEW = "unsealed.collectionView";

/** How the collection shows a set: a binder of 9-card pages (the default), or a grid. */
export type CollectionLayout = "grid" | "binder";
export const getCollectionLayout = (): CollectionLayout =>
  localStorage.getItem(COLLECTION_VIEW) === "grid" ? "grid" : "binder";
export const setCollectionLayout = (layout: CollectionLayout) => localStorage.setItem(COLLECTION_VIEW, layout);

/** Whether this machine can draw the 3D scene (kept apart from it: three.js loads lazily). */
export function canUseWebGL(): boolean {
  try {
    return !!document.createElement("canvas").getContext("webgl2");
  } catch {
    return false;
  }
}
