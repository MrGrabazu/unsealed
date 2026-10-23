// The special foils (galaxy, surge, etched…): their look and their name. A card's foil kind
// comes from the game (`foilKind`, from MTGJSON's promo types and finishes); the effects are
// in foils.css, validated on the mockup .cache/da/v9.html. They are layers over the card,
// never a change to its image.

/** How each kind is shown: its effect (`fx-*` of foils.css; "plain" = the usual foil) and
 * its name on the badge. The names stay in English, as the cards and the players say them. */
const LOOKS: Record<string, { fx: string; label: string }> = {
  etched: { fx: "etched", label: "Etched" },
  galaxyfoil: { fx: "galaxy", label: "Galaxy foil" },
  surgefoil: { fx: "surge", label: "Surge foil" },
  doublerainbow: { fx: "rainbow", label: "Double rainbow" },
  textured: { fx: "textured", label: "Textured" },
  fracturefoil: { fx: "fracture", label: "Fracture foil" },
  halofoil: { fx: "halo", label: "Halo foil" },
  oilslick: { fx: "oil", label: "Oil slick" },
  gilded: { fx: "gilded", label: "Gilded" },
  silverfoil: { fx: "silver", label: "Silver foil" },
  manafoil: { fx: "mana", label: "Mana foil" },
  neonink: { fx: "neon", label: "Neon ink" },
  ripplefoil: { fx: "ripple", label: "Ripple foil" },
  // No look of their own (nothing readable on a thumbnail): the usual foil, with their name.
  stepandcompleat: { fx: "plain", label: "Step-and-compleat" },
  raisedfoil: { fx: "plain", label: "Raised foil" },
  firstplacefoil: { fx: "plain", label: "First place foil" },
  invisibleink: { fx: "plain", label: "Invisible ink" },
  facetfoil: { fx: "plain", label: "Facet foil" },
  chocobotrackfoil: { fx: "plain", label: "Chocobo track foil" },
  cosmicfoil: { fx: "plain", label: "Cosmic foil" },
  rainbowfoil: { fx: "rainbow", label: "Rainbow foil" },
  singularityfoil: { fx: "galaxy", label: "Singularity foil" },
};

/** The name of a special foil, for a badge or a sheet (null for the plain foil). */
export function foilLabel(kind: string | null | undefined): string | null {
  return kind ? (LOOKS[kind]?.label ?? null) : null;
}

/** The effect of a foil kind ("plain" for the usual one). */
export function foilEffect(kind: string | null | undefined): string {
  return (kind && LOOKS[kind]?.fx) || "plain";
}

/** The sheen over a card's foil copy: the usual rainbow, the look of its special foil, or for
 * an art card the gold of its signature. With `badge`, a special foil's name in the corner
 * (where "Foil" is in the collection). */
export function FoilSheen({ rarity, kind, badge = true }: { rarity: string; kind?: string | null; badge?: boolean }) {
  ensureArt();
  if (rarity === "art") return <span className="foil-sheen signed" />;
  const fx = foilEffect(kind);
  const label = foilLabel(kind);
  return (
    <>
      <span className={`foil-sheen fx-${fx}`} />
      {fx === "galaxy" && <span className="foil-stars" />}
      {badge && label && <span className={`badge-foil b-${fx}`}>{label}</span>}
    </>
  );
}

// ---- the drawn parts of the effects: star fields, shards, relief, oil (SVG, seeded, so
// always the same), given to foils.css as custom properties, once.

let drawn = false;
function ensureArt() {
  if (drawn || typeof document === "undefined") return;
  drawn = true;
  const style = document.createElement("style");
  style.dataset.foils = "";
  style.textContent = `:root {
  --foil-stars-1: ${svgUrl(stars(11, 70, 2))};
  --foil-stars-2: ${svgUrl(stars(23, 70, 2))};
  --foil-stars-3: ${svgUrl(stars(37, 40, 3))};
  --foil-shards: ${svgUrl(shards())};
  --foil-relief: ${svgUrl(RELIEF)};
  --foil-oil: ${svgUrl(OIL)};
}`;
  document.head.appendChild(style);
}

const svgUrl = (svg: string) => `url("data:image/svg+xml,${encodeURIComponent(svg)}")`;

/** A seeded random number generator: the same drawing on every card and every launch. */
function seeded(seed: number) {
  return () => (seed = (seed * 16807) % 2147483647) / 2147483647;
}

/** A field of stars at random, of all sizes, a few with the four-pointed glint of a real
 * galaxy foil; drawn on the card's whole size, so that no pattern repeats. */
function stars(seed: number, count: number, glints: number): string {
  const rnd = seeded(seed);
  const parts: string[] = [];
  const tints = ["#ffffff", "#dfe8ff", "#ffe9f8", "#cfd8ff"];
  for (let i = 0; i < count; i++) {
    const [x, y] = [rnd() * 630, rnd() * 880];
    const r = rnd() < 0.85 ? 0.9 + rnd() * 1.4 : 2.4 + rnd() * 2;
    const tint = tints[Math.floor(rnd() * 4)];
    parts.push(`<circle cx='${x.toFixed(0)}' cy='${y.toFixed(0)}' r='${r.toFixed(1)}' fill='${tint}' opacity='${(0.6 + rnd() * 0.4).toFixed(2)}'/>`);
  }
  for (let i = 0; i < glints; i++) {
    const [x, y, s] = [rnd() * 630, rnd() * 880, 9 + rnd() * 12].map((v) => Math.round(v));
    parts.push(
      `<path d='M${x} ${y - s}Q${x} ${y} ${x + s} ${y}Q${x} ${y} ${x} ${y + s}Q${x} ${y} ${x - s} ${y}Q${x} ${y} ${x} ${y - s}Z' fill='white'/>` +
        `<circle cx='${x}' cy='${y}' r='${(s / 3).toFixed(1)}' fill='white' opacity='0.5'/>`,
    );
  }
  return `<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 630 880' preserveAspectRatio='none'><defs><filter id='g'><feGaussianBlur stdDeviation='0.6'/></filter></defs><g filter='url(#g)'>${parts.join("")}</g></svg>`;
}

/** The cracks of a fracture foil: fine lines, each with a soft glow. */
function shards(): string {
  const rnd = seeded(7);
  const lines: string[] = [];
  for (let i = 0; i < 20; i++) {
    const [x1, y1] = [rnd() * 63, rnd() * 88];
    const a = rnd() * Math.PI;
    const len = 30 + rnd() * 60;
    lines.push(`<line x1='${x1.toFixed(1)}' y1='${y1.toFixed(1)}' x2='${(x1 + Math.cos(a) * len).toFixed(1)}' y2='${(y1 + Math.sin(a) * len).toFixed(1)}'/>`);
  }
  const all = lines.join("");
  return `<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 63 88' preserveAspectRatio='none'><filter id='b'><feGaussianBlur stdDeviation='0.35'/></filter><g stroke='white' fill='none'><g stroke-width='1' stroke-opacity='0.5' filter='url(#b)'>${all}</g><g stroke-width='0.3'>${all}</g></g></svg>`;
}

/** A textured foil's relief: noise, lit from the top left. */
const RELIEF =
  "<svg xmlns='http://www.w3.org/2000/svg' width='240' height='240'><filter id='f'><feTurbulence type='fractalNoise' baseFrequency='0.045' numOctaves='3' seed='4'/><feDiffuseLighting lighting-color='white' surfaceScale='3'><feDistantLight azimuth='225' elevation='45'/></feDiffuseLighting></filter><rect width='100%' height='100%' filter='url(#f)'/></svg>";

/** An oil slick: coloured swirls, their hues turning slowly. */
const OIL =
  "<svg xmlns='http://www.w3.org/2000/svg' width='300' height='300'><filter id='o'><feTurbulence type='turbulence' baseFrequency='0.012' numOctaves='2' seed='9'/><feColorMatrix type='hueRotate' values='0'><animate attributeName='values' from='0' to='360' dur='12s' repeatCount='indefinite'/></feColorMatrix><feColorMatrix type='saturate' values='3'/></filter><rect width='100%' height='100%' filter='url(#o)'/></svg>";
