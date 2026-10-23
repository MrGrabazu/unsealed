// Card images come from Scryfall's CDN, through the game's cache on disk (the `cardimg`
// protocol, images.rs): kept once seen, shown offline. Sizes: "normal" (488 × 680) for lists,
// "large" (672 × 936) where a card is shown large (the 3D opening, its details), "small"
// (146 × 204) for the thumbnails under ~120 px wide: decoded, an image weighs its pixels
// (about 1.3 MB in "normal", 0.12 MB in "small"), whatever the space it is shown in.

import { convertFileSrc, isTauri } from "@tauri-apps/api/core";

const CDN = "https://cards.scryfall.io/";
/** The cache's address of a CDN path ("http://cardimg.localhost/" on Windows,
 * "cardimg://localhost/" elsewhere); the CDN itself outside the app (the preview). */
const CACHE = isTauri() ? convertFileSrc("", "cardimg") : CDN;

/** A Scryfall CDN image URL, served through the cache (any other string unchanged). */
export const viaCache = (url: string) => (url.startsWith(CDN) && CACHE !== CDN ? CACHE + url.slice(CDN.length) : url);

/** Every CDN image URL of a value from the game (deep), served through the cache. */
export function withCachedImages<T>(value: T): T {
  if (typeof value === "string") return viaCache(value) as T;
  if (Array.isArray(value)) return value.map(withCachedImages) as T;
  if (value && typeof value === "object") {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(value)) out[k] = withCachedImages(v);
    return out as T;
  }
  return value;
}

/** The same image in another size (`url` from the CDN or the cache; unchanged if it is not
 * a card image in "normal"). */
const resized = (url: string, size: string) => url.replace(/^([a-z]+:\/\/[^/]+\/)normal\//, `$1${size}/`);

/** The large version of a card image URL. */
export function largeImage(url: string): string;
export function largeImage(url: string | null): string | null;
export function largeImage(url: string | null): string | null {
  return url == null ? null : resized(url, "large");
}

/** The small version of a card image URL, for a thumbnail under ~120 px wide. */
export function smallImage(url: string): string;
export function smallImage(url: string | null): string | null;
export function smallImage(url: string | null): string | null {
  return url == null ? null : resized(url, "small");
}
