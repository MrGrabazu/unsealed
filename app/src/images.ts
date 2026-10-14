// Card images come from Scryfall's CDN in its "normal" size (488 × 680), light enough for
// lists and thumbnails. Where a card is shown large (the 3D opening, its details), the
// "large" size (672 × 936) keeps it sharp.

/** The large version of a Scryfall card image URL (unchanged if it is not one). */
export function largeImage(url: string): string;
export function largeImage(url: string | null): string | null;
export function largeImage(url: string | null): string | null {
  return url?.replace("://cards.scryfall.io/normal/", "://cards.scryfall.io/large/") ?? null;
}
