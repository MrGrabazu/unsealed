import { useId, type ReactNode } from "react";
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

/** Metal of the frame, by rarity: the rarer the card, the more precious the metal. */
const FRAMES: Record<AchievementView["rarity"], [string, string]> = {
  common: ["#b9ad96", "#6e6554"],
  uncommon: ["#cfe3ee", "#6f93a8"],
  rare: ["#ffe08a", "#b3822a"],
  mythic: ["#ffd1f0", "#c04fa6"],
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
const CY = 372;
const R = 196;

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

const OUTER = "M56 0 H574 L630 56 V824 L574 880 H56 L0 824 V56 Z";
const INNER = "M74 30 H556 L600 74 V806 L556 850 H74 L30 806 V74 Z";

export function AchievementCard({ item }: { item: AchievementView }) {
  const uid = useId().replace(/:/g, "");
  const palette = PALETTES[item.palette] ?? PALETTES.onyx;
  const [frameLight, frameDark] = FRAMES[item.rarity];
  const emblem = EMBLEMS[item.emblem] ?? EMBLEMS.star;
  const { t } = useI18n();
  const amountsInText = useAmountsInText();
  const hidden = item.secret && !item.unlocked;

  const name = hidden ? "???" : item.name;
  // Bold capitals are about 0.72em wide, plus 1.5px of letter spacing; the banner holds ~470px.
  const nameSize = Math.max(24, Math.min(44, Math.floor((470 / name.length - 1.5) / 0.72)));
  const text = hidden ? t("achievements.secretText") : amountsInText(item.description ?? "");
  const lines = wrap(text, 32, 3);
  const ratio = item.progress && item.progress.goal > 0 ? item.progress.current / item.progress.goal : 0;

  return (
    <svg
      viewBox="0 0 630 880"
      className={`ach-card ${item.unlocked ? "unlocked" : "locked"}`}
      role="img"
      aria-label={t(item.unlocked ? "achievements.cardLabelUnlocked" : "achievements.cardLabelLocked", { name })}
    >
      <defs>
        <linearGradient id={`${uid}f`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor={frameLight} />
          <stop offset="1" stopColor={frameDark} />
        </linearGradient>
        <linearGradient id={`${uid}b`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor={palette.bg2} />
          <stop offset="1" stopColor={palette.bg1} />
        </linearGradient>
        <radialGradient id={`${uid}g`} cx="0.5" cy="0.5" r="0.5">
          <stop offset="0" stopColor={palette.glow} stopOpacity="0.85" />
          <stop offset="1" stopColor={palette.glow} stopOpacity="0" />
        </radialGradient>
        <linearGradient id={`${uid}h`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0.2" stopColor="#ff7bd0" stopOpacity="0" />
          <stop offset="0.4" stopColor="#ff7bd0" stopOpacity="0.55" />
          <stop offset="0.5" stopColor="#7bd0ff" stopOpacity="0.55" />
          <stop offset="0.6" stopColor="#b6ff7b" stopOpacity="0.55" />
          <stop offset="0.8" stopColor="#b6ff7b" stopOpacity="0" />
          <animateTransform attributeName="gradientTransform" type="translate" from="-1 -1" to="1 1" dur="4s" repeatCount="indefinite" />
        </linearGradient>
        <clipPath id={`${uid}c`}>
          <circle cx={CX} cy={CY} r={R} />
        </clipPath>
        <clipPath id={`${uid}i`}>
          <path d={INNER} />
        </clipPath>
      </defs>

      {/* Frame, then the inner panel */}
      <path d={OUTER} fill={`url(#${uid}f)`} />
      <path d={INNER} fill={`url(#${uid}b)`} stroke={frameLight} strokeWidth="4" />
      <path d="M92 48 H538 L582 92 V788 L538 832 H92 L48 788 V92 Z" fill="none" stroke={frameLight} strokeWidth="1.5" opacity="0.5" />
      {[[56, 56], [574, 56], [56, 824], [574, 824]].map(([x, y]) => (
        <circle key={`${x}-${y}`} cx={x} cy={y} r="7" fill={frameDark} stroke={frameLight} strokeWidth="2" />
      ))}

      {/* Card number and rarity */}
      <text x="96" y="92" fontSize="30" letterSpacing="3" fill={palette.accent} opacity="0.9">
        {t("achievements.cardNumber", { number: String(item.number).padStart(2, "0") })}
      </text>
      {Array.from({ length: PIPS[item.rarity] }, (_, i) => (
        <rect
          key={i}
          x={530 - i * 30}
          y="68"
          width="16"
          height="16"
          fill={frameLight}
          transform={`rotate(45 ${538 - i * 30} 76)`}
        />
      ))}

      {/* Medallion */}
      <circle cx={CX} cy={CY} r={R + 26} fill={`url(#${uid}g)`} />
      <circle cx={CX} cy={CY} r={R} fill={palette.bg1} />
      <g clipPath={`url(#${uid}c)`}>
        <Pattern id={item.id} color={palette.accent} />
        <circle cx={CX} cy={CY} r={R * 0.75} fill={`url(#${uid}g)`} />
      </g>
      <circle cx={CX} cy={CY} r={R} fill="none" stroke={`url(#${uid}f)`} strokeWidth="12" />
      <circle cx={CX} cy={CY} r={R - 14} fill="none" stroke={frameLight} strokeWidth="2" opacity="0.6" />
      <g
        transform={`translate(${CX - 105} ${CY - 105}) scale(2.1)`}
        fill={palette.accent}
        stroke={palette.bg1}
        strokeWidth="2.5"
        strokeLinejoin="round"
        strokeLinecap="round"
        style={{ filter: `drop-shadow(0 0 6px ${palette.glow})` }}
      >
        {emblem(palette)}
      </g>

      {/* Banner with the name */}
      <path d="M40 612 H590 L566 660 L590 708 H40 L64 660 Z" fill={`url(#${uid}f)`} />
      <path d="M62 620 H568 L550 660 L568 700 H62 L80 660 Z" fill={palette.bg1} opacity="0.9" />
      <text x={CX} y={660 + nameSize * 0.35} fontSize={nameSize} fontWeight="700" textAnchor="middle" letterSpacing="1.5" fill="#fff6e6">
        {name.toUpperCase()}
      </text>

      {/* Goal, or progress while locked */}
      <g clipPath={`url(#${uid}i)`}>
        {lines.map((line, i) => (
          <text key={i} x={CX} y={752 + i * 34} fontSize="26" textAnchor="middle" fill={palette.accent} opacity="0.95">
            {line}
          </text>
        ))}
        {!item.unlocked && item.progress && (
          <>
            <rect x="130" y="826" width="370" height="12" rx="6" fill={palette.bg1} stroke={frameLight} strokeWidth="1.5" />
            <rect x="130" y="826" width={370 * ratio} height="12" rx="6" fill={palette.accent} />
          </>
        )}
      </g>

      {/* Holographic sheen on the rarest cards */}
      {item.rarity === "mythic" && item.unlocked && (
        <path d={OUTER} fill={`url(#${uid}h)`} style={{ mixBlendMode: "color-dodge" }} />
      )}
    </svg>
  );
}
