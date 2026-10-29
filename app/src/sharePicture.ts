// Pictures of a pull, to share (Discord, WhatsApp…): a card on its own, or a whole booster.
// Drawn on a canvas from the card images (served with CORS by the `cardimg` protocol, so the
// canvas can be exported), with the game's name and address. The card images are drawn whole,
// unaltered: their artist and copyright stay visible, as Scryfall asks.
import { invoke } from "@tauri-apps/api/core";
import { TITLE_FONT, drawSeal, gold } from "./canvasArt";

const TEXT_FONT = "Alegreya, Georgia, serif";
const SITE = "mrgrabazu.github.io/unsealed";
/** A card's proportions, and the radius of its corners (a share of its width). */
const CARD_RATIO = 88 / 63;
const CORNER = 0.045;

/** A card of the picture: its image, how it was pulled, and its texts already written. */
export interface ShareCard {
  imageUrl: string;
  name: string;
  foil: boolean;
  /** "Foil", or a special foil's name ("Galaxy foil"). */
  foilLabel: string | null;
  /** "New", "New foil", or nothing. */
  newLabel: string | null;
}

/** A card shared on its own. */
export interface CardShare {
  card: ShareCard;
  /** "Set name · #123 · Rare". */
  subtitle: string;
  /** "Market value" and "1,234 Seals". */
  priceLabel: string;
  priceText: string | null;
  tagline: string;
}

/** A booster shared whole: its cards in the order they came, the best one shown large. */
export interface BoosterShare {
  title: string;
  /** "Value: 1,234 Seals", and the gain or loss on its price. */
  valueText: string;
  gainText: string | null;
  gain: boolean;
  cards: ShareCard[];
  best: number;
  /** "Best card" and its name and price. */
  bestLabel: string;
  bestText: string;
  tagline: string;
}

function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.crossOrigin = "anonymous";
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error(`image not loaded: ${url}`));
    image.src = url;
  });
}

async function fontsReady() {
  await Promise.all(
    [`900 64px ${TITLE_FONT}`, `700 40px ${TITLE_FONT}`, `400 24px ${TEXT_FONT}`, `italic 400 24px ${TEXT_FONT}`].map((f) =>
      document.fonts.load(f).catch(() => []),
    ),
  );
}

function roundedRect(g: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  g.beginPath();
  g.roundRect(x, y, w, h, r);
}

/** The backdrop: dark leather lit from above, framed by a double gold rule. */
function backdrop(g: CanvasRenderingContext2D, w: number, h: number) {
  const light = g.createRadialGradient(w * 0.45, h * 0.3, 40, w * 0.5, h * 0.5, w * 0.75);
  light.addColorStop(0, "#4a2f1b");
  light.addColorStop(0.55, "#22160d");
  light.addColorStop(1, "#0e0906");
  g.fillStyle = light;
  g.fillRect(0, 0, w, h);
  g.strokeStyle = "#d9a441aa";
  g.lineWidth = 2;
  g.strokeRect(14, 14, w - 28, h - 28);
  g.strokeStyle = "#d9a44144";
  g.lineWidth = 1;
  g.strokeRect(22, 22, w - 44, h - 44);
}

/** The game's name: its seal, UNSEALED in gilded capitals, and its tagline beneath. */
function brand(g: CanvasRenderingContext2D, x: number, y: number, size: number) {
  drawSeal(g, x + size * 0.45, y + size * 0.5, size * 0.9);
  g.save();
  g.font = `900 ${size}px ${TITLE_FONT}`;
  g.textBaseline = "alphabetic";
  g.fillStyle = gold(g, y, y + size);
  g.shadowColor = "#000a";
  g.shadowBlur = 8;
  g.fillText("UNSEALED", x + size * 1.05, y + size * 0.86);
  g.restore();
  g.save();
  g.font = `700 ${Math.round(size * 0.26)}px ${TITLE_FONT}`;
  g.fillStyle = "#d9a441";
  g.letterSpacing = `${Math.round(size * 0.08)}px`;
  g.fillText("CRACK & COLLECT", x + size * 1.1, y + size * 1.28);
  g.restore();
}

/** The colors of the tags: a new card's wax, a foil's pearl. */
const NEW_TAG = { fill: "#a61f28", ink: "#fff3e0" };
const FOIL_TAG = { fill: "#e8dcff", ink: "#3a1d6e" };

/** A card, whole, its corners rounded; a foil gets a rainbow halo around it. With `tags`, its
 * tags lie over its top (a new card's on the left, a foil's on the right): at the bottom, they
 * would hide its artist or its copyright. The image itself is never altered. */
function drawCard(
  g: CanvasRenderingContext2D,
  image: HTMLImageElement | null,
  card: ShareCard,
  x: number,
  y: number,
  w: number,
  tags = true,
) {
  const h = w * CARD_RATIO;
  const r = w * CORNER;
  g.save();
  if (card.foil) {
    const halo = g.createLinearGradient(x, y, x + w, y + h);
    ["#ff8ad8", "#a98bff", "#7fd8ff", "#9dffb0", "#ffe08a"].forEach((c, i, all) => halo.addColorStop(i / (all.length - 1), c));
    g.shadowColor = "#c9a8ffcc";
    g.shadowBlur = w * 0.12;
    roundedRect(g, x - w * 0.015, y - w * 0.015, w * 1.03, h + w * 0.03, r * 1.2);
    g.fillStyle = halo;
    g.fill();
  } else {
    g.shadowColor = "#000c";
    g.shadowBlur = w * 0.08;
    g.shadowOffsetY = w * 0.02;
    roundedRect(g, x, y, w, h, r);
    g.fillStyle = "#000";
    g.fill();
  }
  g.restore();
  g.save();
  roundedRect(g, x, y, w, h, r);
  g.clip();
  if (image) g.drawImage(image, x, y, w, h);
  else {
    g.fillStyle = "#1a120c";
    g.fillRect(x, y, w, h);
  }
  g.restore();
  if (!tags) return;
  const size = Math.max(10, Math.round(w * 0.065));
  if (card.newLabel) tag(g, card.newLabel, x + w * 0.05, y + w * 0.05, size, NEW_TAG);
  if (card.foilLabel) tag(g, card.foilLabel, x + w * 0.95, y + w * 0.05, size, FOIL_TAG, true);
}

/** A small pill's width, at `size`. */
function tagWidth(g: CanvasRenderingContext2D, text: string, size: number) {
  g.save();
  g.font = `700 ${size}px ${TITLE_FONT}`;
  const w = g.measureText(text).width + size * 1.1;
  g.restore();
  return w;
}

/** A small pill, from `x` (or ending at `x`, `fromRight`). Returns its width. */
function tag(
  g: CanvasRenderingContext2D,
  text: string,
  x: number,
  y: number,
  size: number,
  { fill, ink }: { fill: string; ink: string },
  fromRight = false,
) {
  const w = tagWidth(g, text, size);
  g.save();
  g.font = `700 ${size}px ${TITLE_FONT}`;
  const h = size * 1.5;
  if (fromRight) x -= w;
  roundedRect(g, x, y, w, h, h / 2);
  g.fillStyle = fill;
  g.shadowColor = "#000a";
  g.shadowBlur = 6;
  g.fill();
  g.shadowBlur = 0;
  g.fillStyle = ink;
  g.textBaseline = "middle";
  g.fillText(text, x + size * 0.55, y + h / 2 + 1);
  g.restore();
  return w;
}

/** Text wrapped on at most `lines` lines, the last one cut with an ellipsis. Returns the y
 * below it, and where its last line is (its baseline and the width it takes). */
function wrapped(g: CanvasRenderingContext2D, text: string, x: number, y: number, width: number, lineHeight: number, lines: number) {
  const words = text.split(/\s+/);
  const out: string[] = [];
  let line = "";
  for (const word of words) {
    const next = line ? `${line} ${word}` : word;
    if (g.measureText(next).width > width && line) {
      out.push(line);
      line = word;
    } else line = next;
  }
  if (line) out.push(line);
  const shown = out.slice(0, lines);
  if (out.length > lines) {
    let last = shown[lines - 1];
    while (last && g.measureText(`${last}…`).width > width) last = last.slice(0, -1);
    shown[lines - 1] = `${last}…`;
  }
  shown.forEach((l, i) => g.fillText(l, x, y + i * lineHeight));
  const last = shown.length - 1;
  return {
    bottom: y + shown.length * lineHeight,
    lastBaseline: y + Math.max(0, last) * lineHeight,
    lastWidth: last >= 0 ? g.measureText(shown[last]).width : 0,
  };
}

/** The game's address and the Fan Content notice, at the bottom. */
function footer(g: CanvasRenderingContext2D, w: number, h: number, tagline: string) {
  g.save();
  g.textBaseline = "alphabetic";
  g.font = `700 20px ${TITLE_FONT}`;
  g.fillStyle = "#f0c35e";
  g.textAlign = "right";
  g.fillText(SITE, w - 44, h - 62);
  g.font = `italic 400 17px ${TEXT_FONT}`;
  g.fillStyle = "#cdbb9b";
  g.fillText(tagline, w - 44, h - 86);
  g.font = `400 13px ${TEXT_FONT}`;
  g.fillStyle = "#8f7c62";
  g.fillText("Unofficial Fan Content permitted under the Fan Content Policy. Not approved/endorsed by Wizards.", w - 44, h - 42);
  g.restore();
}

function toBlob(canvas: HTMLCanvasElement): Promise<Blob> {
  return new Promise((resolve, reject) =>
    canvas.toBlob((blob) => (blob ? resolve(blob) : reject(new Error("the picture could not be made"))), "image/png"),
  );
}

/** A card on its own: the card large on the left, the game's name, the card's name, set,
 * finish and price on the right. */
export async function drawCardShare(share: CardShare): Promise<Blob> {
  await fontsReady();
  const image = await loadImage(share.card.imageUrl).catch(() => null);
  const [w, h] = [1200, 700];
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const g = canvas.getContext("2d")!;
  backdrop(g, w, h);
  const cardH = 600;
  const cardW = cardH / CARD_RATIO;
  // No tag over the card: they follow its name.
  drawCard(g, image, share.card, 70, (h - cardH) / 2, cardW, false);

  const x = 70 + cardW + 70;
  const width = w - x - 60;
  brand(g, x, 58, 58);
  g.fillStyle = "#d9a44166";
  g.fillRect(x, 160, width, 1);
  g.textBaseline = "alphabetic";
  g.font = `700 44px ${TITLE_FONT}`;
  g.fillStyle = "#f5e6c4";
  const name = wrapped(g, share.card.name, x, 222, width, 52, 2);
  let y = name.bottom;
  // Its tags right after its name ("Ancient Tomb  Foil"), or on the next line if they don't fit.
  const tags = [
    share.card.newLabel && { text: share.card.newLabel, colors: NEW_TAG },
    share.card.foilLabel && { text: share.card.foilLabel, colors: FOIL_TAG },
  ].filter((t): t is { text: string; colors: typeof NEW_TAG } => !!t);
  if (tags.length > 0) {
    const size = 20;
    const gap = 10;
    const tagsWidth = tags.reduce((n, t) => n + tagWidth(g, t.text, size) + gap, 0);
    const inline = name.lastWidth + 16 + tagsWidth <= width;
    let tx = inline ? x + name.lastWidth + 16 : x;
    // Centred on the name's capitals (about 0.7 of its size above its baseline).
    const ty = inline ? name.lastBaseline - 44 * 0.36 - size * 0.75 : name.bottom - 20;
    for (const t of tags) tx += tag(g, t.text, tx, ty, size, t.colors) + gap;
    if (!inline) y = ty + size * 1.5 + 30;
  }
  g.font = `400 25px ${TEXT_FONT}`;
  g.fillStyle = "#cdbb9b";
  y = wrapped(g, share.subtitle, x, y + 8, width, 32, 2).bottom;
  if (share.priceText) {
    g.font = `700 16px ${TITLE_FONT}`;
    g.fillStyle = "#a8957a";
    g.letterSpacing = "3px";
    g.fillText(share.priceLabel.toUpperCase(), x, y + 46);
    g.letterSpacing = "0px";
    drawSeal(g, x + 22, y + 88, 44, true);
    g.font = `900 46px ${TITLE_FONT}`;
    g.fillStyle = gold(g, y + 64, y + 108);
    g.fillText(share.priceText, x + 56, y + 104);
  }
  footer(g, w, h, share.tagline);
  return toBlob(canvas);
}

/** A whole booster: the game's name and the booster's, its value, its best card large on the
 * left with its name, and every card of the booster on the right, in the order they came. */
export async function drawBoosterShare(share: BoosterShare): Promise<Blob> {
  await fontsReady();
  const images = await Promise.all(share.cards.map((c) => loadImage(c.imageUrl).catch(() => null)));
  const [w, h] = [1400, 820];
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const g = canvas.getContext("2d")!;
  backdrop(g, w, h);

  brand(g, 52, 44, 48);
  g.textBaseline = "alphabetic";
  g.textAlign = "right";
  g.font = `700 32px ${TITLE_FONT}`;
  g.fillStyle = "#f5e6c4";
  g.fillText(share.title, w - 52, 82);
  g.font = `400 24px ${TEXT_FONT}`;
  g.fillStyle = "#e8d8b4";
  const value = share.gainText ? `${share.valueText} · ` : share.valueText;
  const gainWidth = share.gainText ? g.measureText(share.gainText).width : 0;
  g.fillText(value, w - 52 - gainWidth, 118);
  if (share.gainText) {
    g.fillStyle = share.gain ? "#8fd18a" : "#e8857a";
    g.fillText(share.gainText, w - 52, 118);
  }
  g.textAlign = "left";
  g.fillStyle = "#d9a44166";
  g.fillRect(40, 146, w - 80, 1);

  // The best card, large, with its name beneath.
  const top = 176;
  const bestH = 500;
  const bestW = bestH / CARD_RATIO;
  const best = share.cards[share.best];
  if (best) {
    drawCard(g, images[share.best], best, 60, top, bestW);
    g.font = `700 14px ${TITLE_FONT}`;
    g.fillStyle = "#a8957a";
    g.letterSpacing = "3px";
    g.fillText(share.bestLabel.toUpperCase(), 60, top + bestH + 36);
    g.letterSpacing = "0px";
    g.font = `400 22px ${TEXT_FONT}`;
    g.fillStyle = "#f5e6c4";
    wrapped(g, share.bestText, 60, top + bestH + 66, bestW, 28, 2);
  }

  // Every card, as large as the room allows.
  const area = { x: 60 + bestW + 50, y: top, w: w - (60 + bestW + 50) - 52, h: h - top - 130 };
  const gap = 12;
  const n = share.cards.length;
  const rowHeight = (size: number) => size * CARD_RATIO;
  let cell = 0;
  let cols = 1;
  for (let c = 1; c <= n; c++) {
    const rows = Math.ceil(n / c);
    const byWidth = (area.w - (c - 1) * gap) / c;
    const byHeight = (area.h - (rows - 1) * gap) / rows / CARD_RATIO;
    const size = Math.min(byWidth, byHeight);
    if (size > cell) [cell, cols] = [size, c];
  }
  const rows = Math.ceil(n / cols);
  const gridW = cols * cell + (cols - 1) * gap;
  const gridH = rows * rowHeight(cell) + (rows - 1) * gap;
  const x0 = area.x + (area.w - gridW) / 2;
  const y0 = area.y + (area.h - gridH) / 2;
  share.cards.forEach((card, i) => {
    const [col, row] = [i % cols, Math.floor(i / cols)];
    drawCard(g, images[i], card, x0 + col * (cell + gap), y0 + row * (rowHeight(cell) + gap), cell);
  });

  footer(g, w, h, share.tagline);
  return toBlob(canvas);
}

/** A player's statistics: a few big numbers, and their finest pulls of all time. */
export interface StatsShare {
  title: string;
  /** The big numbers, as on the Stats screen (each with a line beneath). */
  tiles: { label: string; value: string; sub: string | null }[];
  pullsLabel: string;
  pulls: { card: ShareCard; priceText: string }[];
  tagline: string;
}

/** The statistics: the game's name and the title, the big numbers in medallions, then the
 * finest pulls side by side, each with its price. */
export async function drawStatsShare(share: StatsShare): Promise<Blob> {
  await fontsReady();
  const images = await Promise.all(share.pulls.map((p) => loadImage(p.card.imageUrl).catch(() => null)));
  const [w, h] = [1400, 820];
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const g = canvas.getContext("2d")!;
  backdrop(g, w, h);

  brand(g, 52, 44, 48);
  g.textBaseline = "alphabetic";
  g.textAlign = "right";
  g.font = `700 32px ${TITLE_FONT}`;
  g.fillStyle = "#f5e6c4";
  g.fillText(share.title, w - 52, 98);
  g.textAlign = "left";
  g.fillStyle = "#d9a44166";
  g.fillRect(40, 146, w - 80, 1);

  // The big numbers, in medallions across the width.
  const tileGap = 20;
  const tileW = (w - 104 - (share.tiles.length - 1) * tileGap) / Math.max(1, share.tiles.length);
  const tileY = 172;
  const tileH = 150;
  share.tiles.forEach((tile, i) => {
    const x = 52 + i * (tileW + tileGap);
    g.save();
    roundedRect(g, x, tileY, tileW, tileH, 10);
    g.fillStyle = "#2a1b11d9";
    g.shadowColor = "#000a";
    g.shadowBlur = 12;
    g.fill();
    g.shadowBlur = 0;
    g.strokeStyle = "#d9a44166";
    g.lineWidth = 1;
    g.stroke();
    g.restore();
    g.font = `700 14px ${TITLE_FONT}`;
    g.fillStyle = "#a8957a";
    g.letterSpacing = "2px";
    wrapped(g, tile.label.toUpperCase(), x + 18, tileY + 32, tileW - 36, 18, 1);
    g.letterSpacing = "0px";
    // As large as the medallion allows ("1,234,567 Seals" is long).
    let size = 36;
    do g.font = `900 ${size}px ${TITLE_FONT}`;
    while (g.measureText(tile.value).width > tileW - 36 && --size > 18);
    g.fillStyle = gold(g, tileY + 48, tileY + 88);
    g.fillText(tile.value, x + 18, tileY + 84);
    if (tile.sub) {
      g.font = `400 18px ${TEXT_FONT}`;
      g.fillStyle = "#cdbb9b";
      wrapped(g, tile.sub, x + 18, tileY + 114, tileW - 36, 22, 2);
    }
  });

  // The finest pulls, side by side, their price beneath.
  if (share.pulls.length > 0) {
    g.font = `700 16px ${TITLE_FONT}`;
    g.fillStyle = "#a8957a";
    g.letterSpacing = "3px";
    g.fillText(share.pullsLabel.toUpperCase(), 52, 366);
    g.letterSpacing = "0px";
    // Their prices end above the footer's lines (at the bottom right).
    const cardH = 290;
    const cardW = cardH / CARD_RATIO;
    const gap = 28;
    const rowW = share.pulls.length * cardW + (share.pulls.length - 1) * gap;
    const x0 = (w - rowW) / 2;
    const y0 = 382;
    share.pulls.forEach((pull, i) => {
      const x = x0 + i * (cardW + gap);
      drawCard(g, images[i], pull.card, x, y0, cardW);
      g.font = `700 20px ${TITLE_FONT}`;
      g.fillStyle = gold(g, y0 + cardH + 14, y0 + cardH + 36);
      g.textAlign = "center";
      g.fillText(pull.priceText, x + cardW / 2, y0 + cardH + 34);
      g.textAlign = "left";
    });
  }

  footer(g, w, h, share.tagline);
  return toBlob(canvas);
}

/** Puts the picture in the clipboard, to paste it anywhere. */
export async function copyPicture(blob: Blob): Promise<void> {
  await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
}

/** Saves the picture in the player's pictures (Unsealed folder); returns where. */
export async function savePicture(blob: Blob, name: string): Promise<string> {
  // A plain file name (sent in a header, which takes ASCII only).
  const plain = name
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/[^A-Za-z0-9 .\-]+/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 80);
  const bytes = new Uint8Array(await blob.arrayBuffer());
  return invoke<string>("save_share_image", bytes, { headers: { name: plain || "Unsealed" } });
}
