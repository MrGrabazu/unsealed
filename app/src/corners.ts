// The corners of a card, measured on Scryfall's images (a share of the card's width). Alpha's
// cards were cut with rounder corners than every later printing. In CSS, `--card-radius` (in
// styles.css) holds the usual corners, and `alpha-corners` on a card's container (or anything
// around it) gives it Alpha's.

export const CORNER = 0.0412;
export const ALPHA_CORNER = 0.0687;

const isAlpha = (setCode: string | null | undefined) => setCode?.toUpperCase() === "LEA";

/** The radius of a card's corners, as a share of its width. */
export const cornerOf = (setCode: string | null | undefined) => (isAlpha(setCode) ? ALPHA_CORNER : CORNER);

/** The class giving Alpha's corners to the cards inside an element. */
export const cornersClass = (setCode: string | null | undefined) => (isAlpha(setCode) ? "alpha-corners" : "");
