import { useId, type ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import type { AchievementView } from "./api";
import { useI18n } from "./i18n";
import { useAmountsInText } from "./money";

// Achievement cards are drawn here, in SVG, with Unsealed's own visual identity: a framed
// medallion with a banner. They deliberately share nothing with the layout of any trading
// card game (no title bar, art window, type line or text box), so the same cards can sit
// next to cards of any license.

interface Palette {
  bg1: string;
  bg2: string;
  accent: string;
  glow: string;
}

// Must stay in sync with PALETTES in core/src/achievements.rs.
const PALETTES: Record<string, Palette> = {
  ember: { bg1: "#3a0f0a", bg2: "#7a2412", accent: "#ffb347", glow: "#ff6a3d" },
  ocean: { bg1: "#071b33", bg2: "#0e4a7a", accent: "#6fd3ff", glow: "#2aa9ff" },
  forest: { bg1: "#0b2616", bg2: "#1f6b3a", accent: "#a6f0a0", glow: "#4cd964" },
  royal: { bg1: "#1a1040", bg2: "#4a2c9a", accent: "#d9b8ff", glow: "#9b6bff" },
  onyx: { bg1: "#101014", bg2: "#2a2a33", accent: "#b8b8c8", glow: "#6a6a80" },
  gold: { bg1: "#3a2a06", bg2: "#8a6414", accent: "#ffe08a", glow: "#ffc93c" },
  rose: { bg1: "#3a0a22", bg2: "#8a1f55", accent: "#ffb3d1", glow: "#ff5fa2" },
  ice: { bg1: "#0b2330", bg2: "#2d6b8a", accent: "#d6f6ff", glow: "#9fe8ff" },
};

/** Metal of the rim, by rarity: the rarer the card, the more precious the metal (pewter,
 * silver, gold, fire). */
const FRAMES: Record<AchievementView["rarity"], [string, string]> = {
  common: ["#d4c3a0", "#7a6a4e"],
  uncommon: ["#dff0f6", "#6f93a8"],
  rare: ["#ffe9a8", "#b3822a"],
  mythic: ["#ffc08a", "#c4501a"],
};

const PIPS: Record<AchievementView["rarity"], number> = { common: 1, uncommon: 2, rare: 3, mythic: 4 };

// Emblems are drawn in a 100x100 box. Must stay in sync with EMBLEMS in core/src/achievements.rs.
const EMBLEMS: Record<string, (c: Palette) => ReactNode> = {
  star: () => <polygon points="50,6 62,37 94,38 69,58 78,91 50,72 22,91 31,58 6,38 38,37" />,
  crown: () => (
    <>
      <path d="M12 76 L8 28 L32 50 L50 16 L68 50 L92 28 L88 76 Z" />
      <rect x="12" y="80" width="76" height="10" rx="2" />
    </>
  ),
  flame: (c) => (
    <>
      <path d="M50 4 C58 24 80 34 78 60 C76 80 64 94 50 94 C34 94 22 80 24 60 C25 48 34 42 40 30 C42 42 48 46 52 46 C55 34 48 22 50 4 Z" />
      <path d="M50 52 C58 62 64 68 62 78 C60 86 56 90 50 90 C42 90 38 84 40 76 C41 70 46 66 50 52 Z" fill={c.bg1} stroke="none" />
    </>
  ),
  coin: (c) => (
    <>
      <circle cx="50" cy="50" r="42" />
      <circle cx="50" cy="50" r="32" fill="none" stroke={c.bg1} strokeWidth="4" />
      <polygon points="50,28 56,44 73,45 60,55 65,72 50,62 35,72 40,55 27,45 44,44" fill={c.bg1} stroke="none" />
    </>
  ),
  key: (c) => (
    <g transform="rotate(-45 50 50)">
      <circle cx="26" cy="50" r="20" />
      <circle cx="26" cy="50" r="8" fill={c.bg1} stroke="none" />
      <rect x="44" y="45" width="50" height="10" />
      <rect x="76" y="55" width="8" height="14" />
      <rect x="62" y="55" width="8" height="10" />
    </g>
  ),
  eye: (c) => (
    <>
      <path d="M4 50 C22 20 78 20 96 50 C78 80 22 80 4 50 Z" />
      <circle cx="50" cy="50" r="17" fill={c.bg1} stroke="none" />
      <circle cx="50" cy="50" r="8" fill={c.accent} stroke="none" />
    </>
  ),
  gem: (c) => (
    <>
      <polygon points="26,12 74,12 96,40 50,92 4,40" />
      <path d="M4 40 H96 M26 12 L38 40 L50 92 L62 40 L74 12" fill="none" stroke={c.bg1} strokeWidth="3" />
    </>
  ),
  book: (c) => (
    <>
      <path d="M6 20 L48 28 L48 90 L6 82 Z" />
      <path d="M94 20 L52 28 L52 90 L94 82 Z" />
      <path d="M14 36 L40 41 M14 48 L40 53 M14 60 L40 65" stroke={c.bg1} strokeWidth="3" fill="none" />
    </>
  ),
  hourglass: () => (
    <path d="M20 6 H80 V16 C80 36 60 46 52 50 C60 54 80 64 80 84 V94 H20 V84 C20 64 40 54 48 50 C40 46 20 36 20 16 Z" />
  ),
  trophy: () => (
    <>
      <path d="M28 8 H72 V38 C72 56 62 66 50 68 C38 66 28 56 28 38 Z" />
      <path d="M28 16 H10 C10 36 18 46 32 48 M72 16 H90 C90 36 82 46 68 48" fill="none" strokeWidth="6" />
      <rect x="43" y="68" width="14" height="14" />
      <rect x="28" y="82" width="44" height="10" rx="3" />
    </>
  ),
  pack: (c) => (
    <>
      <polygon points="22,14 29,6 36,14 43,6 50,14 57,6 64,14 71,6 78,14 78,86 71,94 64,86 57,94 50,86 43,94 36,86 29,94 22,86" />
      <polygon points="50,32 56,45 70,46 59,55 63,69 50,61 37,69 41,55 30,46 44,45" fill={c.bg1} stroke="none" />
    </>
  ),
  skull: (c) => (
    <>
      <path d="M50 6 C26 6 12 24 12 44 C12 56 18 63 27 67 V82 H73 V67 C82 63 88 56 88 44 C88 24 74 6 50 6 Z" />
      <circle cx="35" cy="46" r="10" fill={c.bg1} stroke="none" />
      <circle cx="65" cy="46" r="10" fill={c.bg1} stroke="none" />
      <polygon points="50,56 44,68 56,68" fill={c.bg1} stroke="none" />
      <path d="M38 82 V92 M50 82 V92 M62 82 V92" stroke={c.bg1} strokeWidth="4" />
    </>
  ),
  bolt: () => <polygon points="60,4 20,54 44,54 36,96 80,40 54,40" />,
  scale: () => (
    <>
      <rect x="47" y="12" width="6" height="68" />
      <rect x="20" y="22" width="60" height="6" />
      <path d="M8 60 A16 16 0 0 0 40 60 Z M60 60 A16 16 0 0 0 92 60 Z" />
      <path d="M24 28 L10 60 M24 28 L38 60 M76 28 L62 60 M76 28 L90 60" fill="none" strokeWidth="2.5" />
      <rect x="32" y="82" width="36" height="8" rx="3" />
    </>
  ),
  cards: (c) => (
    <>
      <rect x="20" y="14" width="46" height="66" rx="6" transform="rotate(-16 43 47)" opacity="0.55" />
      <rect x="34" y="14" width="46" height="66" rx="6" transform="rotate(16 57 47)" opacity="0.75" />
      <rect x="27" y="18" width="46" height="66" rx="6" />
      <polygon points="50,36 55,48 68,49 58,57 61,69 50,62 39,69 42,57 32,49 45,48" fill={c.bg1} stroke="none" />
    </>
  ),
  chart: () => (
    <>
      <rect x="10" y="62" width="16" height="28" />
      <rect x="34" y="46" width="16" height="44" />
      <rect x="58" y="30" width="16" height="60" />
      <polyline points="8,40 32,22 52,32 82,8" fill="none" strokeWidth="5" />
      <polygon points="92,2 94,24 74,14" />
    </>
  ),
  question: () => (
    <text x="50" y="80" fontSize="96" fontWeight="700" textAnchor="middle" stroke="none">
      ?
    </text>
  ),
};

function hash(text: string): number {
  let h = 2166136261;
  for (const ch of text) {
    h ^= ch.charCodeAt(0);
    h = Math.imul(h, 16777619);
  }
  return h >>> 0;
}

const CX = 315;
const CY = 318;
const R = 168;

/** Procedural medallion background, one of five patterns chosen from the achievement id. */
function Pattern({ id, color }: { id: string; color: string }) {
  const h = hash(id);
  const rot = h % 360;
  switch (h % 5) {
    case 0: // rays
      return (
        <>
          {Array.from({ length: 18 }, (_, i) => {
            const a = ((i * 20 + rot) * Math.PI) / 180;
            const b = a + (10 * Math.PI) / 180;
            const p = (t: number) => `${CX + R * 1.2 * Math.cos(t)},${CY + R * 1.2 * Math.sin(t)}`;
            return <polygon key={i} points={`${CX},${CY} ${p(a)} ${p(b)}`} fill={color} opacity={0.2} />;
          })}
        </>
      );
    case 1: // rings
      return (
        <>
          {Array.from({ length: 8 }, (_, i) => (
            <circle key={i} cx={CX} cy={CY} r={26 + i * 26} fill="none" stroke={color} strokeWidth={5} opacity={0.22} />
          ))}
        </>
      );
    case 2: // dots
      return (
        <>
          {Array.from({ length: 11 }, (_, row) =>
            Array.from({ length: 12 }, (_, col) => (
              <circle key={`${row}-${col}`} cx={CX - 200 + col * 38 + (row % 2) * 19} cy={CY - 200 + row * 38} r={5} fill={color} opacity={0.3} />
            )),
          )}
        </>
      );
    case 3: // stripes
      return (
        <g transform={`rotate(${rot % 90} ${CX} ${CY})`}>
          {Array.from({ length: 16 }, (_, i) => (
            <rect key={i} x={CX - 260} y={CY - 260 + i * 34} width={520} height={12} fill={color} opacity={0.2} />
          ))}
        </g>
      );
    default: // waves
      return (
        <>
          {Array.from({ length: 8 }, (_, i) => {
            const y = CY - 190 + i * 52;
            return (
              <path
                key={i}
                d={`M ${CX - 220} ${y} q 55 -34 110 0 t 110 0 t 110 0 t 110 0`}
                fill="none"
                stroke={color}
                strokeWidth={6}
                opacity={0.25}
              />
            );
          })}
        </>
      );
  }
}

/** Splits text into at most `maxLines` lines of about `width` characters. */
function wrap(text: string, width: number, maxLines: number): string[] {
  const lines: string[] = [];
  let line = "";
  for (const word of text.split(" ")) {
    if (line && (line + " " + word).length > width) {
      lines.push(line);
      line = word;
    } else {
      line = line ? line + " " + word : word;
    }
  }
  if (line) lines.push(line);
  if (lines.length > maxLines) {
    lines.length = maxLines;
    lines[maxLines - 1] = lines[maxLines - 1].replace(/.{0,2}$/, "…");
  }
  return lines;
}

/** The card's body, with rounded corners; the rarity's ribbon goes over its lower edge. */
const BODY = { x: 8, y: 8, w: 614, h: 832, r: 30 };
const INNER = "M44 40 H586 Q590 40 590 44 V804 Q590 808 586 808 H44 Q40 808 40 804 V44 Q40 40 44 40 Z";
/** The theme's filigree corner (as on its panels), in a 26-unit box. */
const CORNER = "M2 24V9Q2 2 9 2H24";
const CORNER_IN = "M6 20V12Q6 6 12 6H20";

const RARITY_KEYS = {
  common: "rarityName.common",
  uncommon: "rarityName.uncommon",
  rare: "rarityName.rare",
  mythic: "rarityName.mythic",
} as const;

/** One of the four filigree corners, `sx`/`sy` mirroring it. */
function Corner({ x, y, sx, sy, color }: { x: number; y: number; sx: number; sy: number; color: string }) {
  return (
    <g transform={`translate(${x} ${y}) scale(${3 * sx} ${3 * sy})`} fill="none" stroke={color} strokeWidth="1.3">
      <path d={CORNER} />
      <path d={CORNER_IN} opacity="0.55" />
      <circle cx="9.5" cy="9.5" r="1.9" fill={color} stroke="none" />
    </g>
  );
}

/** A locked card's colour: mostly grey, and darker (CSS `grayscale(0.85) brightness(0.55)`).
 * Worked out on each colour instead of filtering the drawn card: a filter on a hundred SVG
 * cards made the cabinet lag, and these steps being linear, the result is the same. */
function dim(hex: string): string {
  const full = hex.length === 4 ? hex.replace(/[0-9a-f]/gi, (d) => d + d) : hex;
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(full.slice(i, i + 2), 16));
  const s = 0.15;
  const mix = [
    (0.2126 + 0.7874 * s) * r + (0.7152 - 0.7152 * s) * g + (0.0722 - 0.0722 * s) * b,
    (0.2126 - 0.2126 * s) * r + (0.7152 + 0.2848 * s) * g + (0.0722 - 0.0722 * s) * b,
    (0.2126 - 0.2126 * s) * r + (0.7152 - 0.7152 * s) * g + (0.0722 + 0.9278 * s) * b,
  ];
  return "#" + mix.map((v) => Math.round(Math.min(255, v * 0.55)).toString(16).padStart(2, "0")).join("");
}

const same = (hex: string) => hex;

/** The colours of a card: as they are once unlocked, dimmed before. */
const toneOf = (item: AchievementView) => (item.unlocked ? same : dim);

/** The card's art, everything but its text (an image cannot use the page's fonts): the rim
 * in the rarity's metal, the leather, the corners, the plate and pips, the medallion between
 * its wings, the ornament under the name at `ornamentY`, and the ribbon. Drawn as an image of
 * its own, its ids clash with no other card's. */
function CardArt({ item, ornamentY }: { item: AchievementView; ornamentY: number }) {
  const c = toneOf(item);
  const base = PALETTES[item.palette] ?? PALETTES.onyx;
  const palette: Palette = { bg1: c(base.bg1), bg2: c(base.bg2), accent: c(base.accent), glow: c(base.glow) };
  const metal = c(FRAMES[item.rarity][0]);
  const metalDark = c(FRAMES[item.rarity][1]);
  const ink = c("#1a100a");
  const emblem = EMBLEMS[item.emblem] ?? EMBLEMS.star;
  const uid = "art";

  return (
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 630 880" width="630" height="880">
      <defs>
        <linearGradient id={`${uid}m`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor={metal} />
          <stop offset="0.5" stopColor={metalDark} />
          <stop offset="1" stopColor={metal} />
        </linearGradient>
        <linearGradient id={`${uid}l`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor={c("#3a2618")} />
          <stop offset="1" stopColor={ink} />
        </linearGradient>
        {/* The grain of the leather. */}
        <pattern id={`${uid}grain`} width="14" height="14" patternUnits="userSpaceOnUse">
          <circle cx="3" cy="4" r="1.1" fill={c("#ffffff")} opacity="0.05" />
          <circle cx="10" cy="10" r="1.3" fill="#000000" opacity="0.22" />
          <circle cx="11" cy="3" r="0.8" fill={c("#ffffff")} opacity="0.04" />
        </pattern>
        <linearGradient id={`${uid}b`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor={palette.bg2} />
          <stop offset="1" stopColor={palette.bg1} />
        </linearGradient>
        <radialGradient id={`${uid}g`} cx="0.5" cy="0.5" r="0.5">
          <stop offset="0" stopColor={palette.glow} stopOpacity="0.8" />
          <stop offset="1" stopColor={palette.glow} stopOpacity="0" />
        </radialGradient>
        <clipPath id={`${uid}c`}>
          <circle cx={CX} cy={CY} r={R} />
        </clipPath>
      </defs>

      {/* The body: the rim in the rarity's metal, the leather inside, its grain. */}
      <rect x={BODY.x} y={BODY.y} width={BODY.w} height={BODY.h} rx={BODY.r} fill={`url(#${uid}m)`} />
      <rect x={BODY.x + 3} y={BODY.y + 3} width={BODY.w - 6} height={BODY.h - 6} rx={BODY.r - 3} fill="none" stroke="#000" strokeOpacity="0.45" strokeWidth="2" />
      <path d={INNER} fill={`url(#${uid}l)`} />
      <path d={INNER} fill={`url(#${uid}grain)`} />
      {/* A double gilded rule, and the stitching between them. */}
      <path d={INNER} fill="none" stroke={metal} strokeWidth="3" opacity="0.9" />
      <rect x="58" y="58" width="514" height="732" rx="6" fill="none" stroke={metal} strokeWidth="1.5" opacity="0.45" />
      <rect x="50" y="50" width="530" height="748" rx="5" fill="none" stroke={metal} strokeWidth="1.2" strokeDasharray="7 6" opacity="0.35" />
      <Corner x={64} y={64} sx={1} sy={1} color={metal} />
      <Corner x={566} y={64} sx={-1} sy={1} color={metal} />
      <Corner x={64} y={784} sx={1} sy={-1} color={metal} />
      <Corner x={566} y={784} sx={-1} sy={-1} color={metal} />

      {/* The number on a small plate, and the rarity's pips. */}
      <rect x="146" y="76" width="128" height="40" rx="6" fill={ink} stroke={metal} strokeWidth="1.5" opacity="0.95" />
      {Array.from({ length: PIPS[item.rarity] }, (_, i) => (
        <rect key={i} x={470 - i * 30} y="88" width="16" height="16" fill={metal} stroke={ink} strokeWidth="1.5" transform={`rotate(45 ${478 - i * 30} 96)`} />
      ))}

      {/* The medallion between two gilded wings. */}
      <circle cx={CX} cy={CY} r={R + 34} fill={`url(#${uid}g)`} opacity={item.unlocked ? 1 : 0.4} />
      {[-1, 1].map((side) => (
        <g key={side} transform={`translate(${CX} ${CY}) scale(${side} 1)`} fill="none" stroke={metal} strokeLinecap="round">
          <path d={`M${R + 14} -24 C${R + 70} -40 ${R + 92} -10 ${R + 104} 22`} strokeWidth="5" />
          <path d={`M${R + 10} 6 C${R + 54} 0 ${R + 74} 22 ${R + 82} 46`} strokeWidth="4" opacity="0.8" />
          <path d={`M${R + 4} 36 C${R + 38} 36 ${R + 54} 54 ${R + 58} 70`} strokeWidth="3" opacity="0.6" />
          <circle cx={R + 104} cy="22" r="6" fill={metal} stroke="none" />
        </g>
      ))}
      <circle cx={CX} cy={CY} r={R + 10} fill={ink} stroke={`url(#${uid}m)`} strokeWidth="14" />
      <circle cx={CX} cy={CY} r={R} fill={`url(#${uid}b)`} />
      <g clipPath={`url(#${uid}c)`}>
        <Pattern id={item.id} color={palette.accent} />
        <circle cx={CX} cy={CY} r={R * 0.75} fill={`url(#${uid}g)`} />
      </g>
      <circle cx={CX} cy={CY} r={R - 12} fill="none" stroke={metal} strokeWidth="2" opacity="0.55" />
      {/* Studs around the rim of the medallion. */}
      {Array.from({ length: 12 }, (_, i) => {
        const a = (i * 30 * Math.PI) / 180;
        return <circle key={i} cx={CX + (R + 10) * Math.cos(a)} cy={CY + (R + 10) * Math.sin(a)} r="3.5" fill={ink} opacity="0.6" />;
      })}
      <g
        transform={`translate(${CX - 92} ${CY - 92}) scale(1.84)`}
        fill={palette.accent}
        stroke={palette.bg1}
        strokeWidth="2.5"
        strokeLinejoin="round"
        strokeLinecap="round"
        style={{ filter: `drop-shadow(0 0 6px ${palette.glow})` }}
      >
        {emblem(palette)}
      </g>

      {/* The ornament between the name and the goal. */}
      <g transform={`translate(${CX} ${ornamentY})`} stroke={metal} fill={metal}>
        <line x1="-120" y1="0" x2="-14" y2="0" strokeWidth="1.5" opacity="0.7" />
        <line x1="14" y1="0" x2="120" y2="0" strokeWidth="1.5" opacity="0.7" />
        <rect x="-7" y="-7" width="14" height="14" transform="rotate(45)" stroke="none" />
      </g>

      {/* The rarity's ribbon over the lower edge. */}
      <path d="M150 812 H480 L504 836 L480 860 H150 L126 836 Z" fill="#000" opacity="0.4" transform="translate(0 4)" />
      <path d="M150 812 H480 L504 836 L480 860 H150 L126 836 Z" fill={`url(#${uid}m)`} />
      <path d="M160 818 H470 L488 836 L470 854 H160 L142 836 Z" fill="none" stroke={ink} strokeWidth="1.5" opacity="0.5" />
    </svg>
  );
}

/** The art of each look already drawn, as an image URL: decoded once by the browser, where
 * a hundred live SVG cards of a few hundred shapes each made the cabinet lag. */
const arts = new Map<string, string>();

function artUrl(item: AchievementView, ornamentY: number): string {
  const key = [item.id, item.palette, item.emblem, item.rarity, item.unlocked, ornamentY].join("|");
  let url = arts.get(key);
  if (!url) {
    url = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(renderToStaticMarkup(<CardArt item={item} ornamentY={ornamentY} />))}`;
    arts.set(key, url);
  }
  return url;
}

/** An achievement card, as on the game's tooled leather: a rim in the rarity's metal, the
 * theme's filigree corners, a medallion (the achievement's own colours, pattern and emblem)
 * between two gilded wings, its name, its goal or progress, and the rarity on a ribbon over
 * the lower edge. Its art is an image (`CardArt`), its text a light SVG layer over it, and
 * the holographic sheen of an unlocked mythic a layer slid by CSS (`.ach-holo`). `lite`: a
 * card of the cabinet, without the shadow filter of a card shown alone. */
export function AchievementCard({ item, lite = false }: { item: AchievementView; lite?: boolean }) {
  const uid = useId().replace(/:/g, "");
  const c = toneOf(item);
  const metal = c(FRAMES[item.rarity][0]);
  const { t, formatNumber } = useI18n();
  const amountsInText = useAmountsInText();
  const hidden = item.secret && !item.unlocked;
  const showProgress = !item.unlocked && item.progress != null;

  const name = hidden ? "???" : item.name;
  // Cinzel's capitals are about 0.78em wide with the letter spacing; the plate holds ~500px,
  // on two lines when the name is long.
  const nameLines = name.length > 18 ? wrap(name.toUpperCase(), 18, 2) : [name.toUpperCase()];
  const longest = Math.max(...nameLines.map((l) => l.length));
  const nameSize = Math.max(28, Math.min(46, Math.floor(500 / longest / 0.78)));
  const text = hidden ? t("achievements.secretText") : amountsInText(item.description ?? "");
  // The progress (count and bar) takes the place of a line.
  const lines = wrap(text, 34, showProgress ? 2 : 3);
  const ratio = item.progress && item.progress.goal > 0 ? Math.min(1, item.progress.current / item.progress.goal) : 0;
  const nameY = 560;
  const textY = nameY + nameLines.length * nameSize + 34;

  return (
    <div
      className={`ach-card ${item.unlocked ? "unlocked" : "locked"} ${item.rarity}${lite ? " lite" : ""}`}
      role="img"
      aria-label={t(item.unlocked ? "achievements.cardLabelUnlocked" : "achievements.cardLabelLocked", { name })}
    >
      <img src={artUrl(item, textY - 26)} alt="" draggable={false} />
      <svg className="ach-text" viewBox="0 0 630 880" aria-hidden="true">
        <defs>
          <clipPath id={`${uid}i`}>
            <path d={INNER} />
          </clipPath>
        </defs>
        <text x="210" y="104" fontSize="23" fontFamily="Cinzel, serif" fontWeight="700" letterSpacing="2" textAnchor="middle" fill={metal}>
          {t("achievements.cardNumber", { number: String(item.number).padStart(2, "0") })}
        </text>
        {/* The name (its hard shadow drawn under it, cheaper than a filter), then the goal (or
            the progress while locked). */}
        <g fontSize={nameSize} fontFamily="Cinzel, serif" fontWeight="800" textAnchor="middle" letterSpacing="1.5">
          {nameLines.map((line, i) => (
            <g key={i}>
              <text x={CX} y={nameY + i * nameSize * 1.05 + 2} fill="#000">
                {line}
              </text>
              <text x={CX} y={nameY + i * nameSize * 1.05} fill={c("#fff1c7")}>
                {line}
              </text>
            </g>
          ))}
        </g>
        <g clipPath={`url(#${uid}i)`}>
          {lines.map((line, i) => (
            <text key={i} x={CX} y={textY + 6 + i * 30} fontSize="25" fontFamily="Alegreya, serif" textAnchor="middle" fill={c("#e8d8b4")} opacity="0.92">
              {line}
            </text>
          ))}
          {showProgress && item.progress && (
            <>
              <rect x="140" y="738" width="350" height="14" rx="7" fill={c("#120b07")} stroke={metal} strokeWidth="1.5" />
              <rect x="140" y="738" width={350 * ratio} height="14" rx="7" fill={metal} />
              <text x={CX} y="778" fontSize="19" fontFamily="Cinzel, serif" textAnchor="middle" fill={c("#cdbb9b")} letterSpacing="1">
                {formatNumber(item.progress.current)} / {formatNumber(item.progress.goal)}
              </text>
            </>
          )}
        </g>
        {/* The rarity, on its ribbon. */}
        <text x={CX} y="845" fontSize="24" fontFamily="Cinzel, serif" fontWeight="800" letterSpacing="4" textAnchor="middle" fill={c("#2a1606")}>
          {t(RARITY_KEYS[item.rarity]).toUpperCase()}
        </text>
      </svg>
      {item.rarity === "mythic" && item.unlocked && <span className="ach-holo" />}
    </div>
  );
}
