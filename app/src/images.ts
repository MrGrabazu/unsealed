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

/** The width of the thumbnails the cache makes (`THUMB_WIDTH` of images.rs). */
const THUMB_WIDTH = 320;
/** The images the cache makes thumbnails of, by size: their width over their height. */
const THUMBED: Record<string, number> = { normal: 488 / 680, large: 672 / 936, art_crop: 626 / 457 };

/** The image to show in a box of `width` × `height` CSS pixels: its thumbnail (320 px wide,
 * made by the cache, images.rs) when that covers the box on this screen, itself otherwise. A
 * "normal" card image decoded weighs 1.3 MB, its thumbnail 0.6 MB. Only for the images the
 * cache serves (never in the preview, outside the app). */
export function fittedImage(url: string, width: number, height: number): string {
  if (CACHE === CDN || !url.startsWith(CACHE)) return url;
  const aspect = THUMBED[url.slice(CACHE.length).split("/", 1)[0]];
  if (aspect == null) return url;
  // An image covering its box (`object-fit: cover`) may need more than the box's width.
  const needed = Math.max(width, height * aspect) * window.devicePixelRatio;
  return needed > 0 && needed <= THUMB_WIDTH ? `${CACHE}thumb/${url.slice(CACHE.length)}` : url;
}

/** How long images loaded ahead are held decoded, at most (milliseconds). */
const AHEAD_HELD_MS = 3 * 60_000;
/** The images loaded ahead, held so the WebView keeps them decoded until they are shown. */
let heldAhead: HTMLImageElement[] = [];
let releaseAhead: ReturnType<typeof setTimeout> | undefined;

/** Loads and decodes, ahead of time, card images about to be shown in a list (an opening's
 * summary, while the booster is unwrapped): each in "normal" and as its thumbnail, the one
 * shown depending on the size of its box, which does not exist yet (`fittedImage`). Asked
 * only when shown, the cache could still be downloading an image or making its thumbnail, and
 * the cards stayed black a while. The images of the previous call are let go. */
export function loadAhead(urls: (string | null | undefined)[]): void {
  const shown = urls.filter((url): url is string => url != null);
  heldAhead = [...new Set(shown.flatMap((url) => [url, fittedImage(url, 1, 1)]))].map((src) => {
    const image = new Image();
    image.src = src;
    void image.decode().catch(() => {});
    return image;
  });
  clearTimeout(releaseAhead);
  releaseAhead = setTimeout(() => (heldAhead.length = 0), AHEAD_HELD_MS);
}

/** The small version of a card image URL, for a thumbnail under ~120 px wide. */
export function smallImage(url: string): string;
export function smallImage(url: string | null): string | null;
export function smallImage(url: string | null): string | null {
  return url == null ? null : resized(url, "small");
}
