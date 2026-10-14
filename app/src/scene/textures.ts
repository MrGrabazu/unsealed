import * as THREE from "three";
import { SEAL_PATH } from "../logoSvg";

// Textures of the 3D opening, drawn on canvases: the original wrapper (with the set's art
// in a window, the art itself left unaltered), its tear-off strip, the cards' back, and the
// glow behind a big pull. Card faces come from Scryfall's CDN, which allows cross-origin use.

const loader = new THREE.TextureLoader();
loader.setCrossOrigin("anonymous");

function canvasTexture(canvas: HTMLCanvasElement): THREE.CanvasTexture {
  const texture = new THREE.CanvasTexture(canvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  texture.anisotropy = 4;
  return texture;
}

function loadImage(url: string): Promise<HTMLImageElement | null> {
  return new Promise((resolve) => {
    const img = new Image();
    img.crossOrigin = "anonymous";
    img.onload = () => resolve(img);
    img.onerror = () => resolve(null);
    img.src = url;
  });
}

/** A card face from Scryfall. */
export function loadCardTexture(url: string): Promise<THREE.Texture> {
  return new Promise((resolve, reject) => {
    loader.load(
      url,
      (texture) => {
        texture.colorSpace = THREE.SRGBColorSpace;
        texture.anisotropy = 8;
        resolve(texture);
      },
      undefined,
      reject,
    );
  });
}

export interface PackLook {
  hue: number;
  setName: string;
  kindLabel: string;
  artUrl: string | null;
  /** The expansion symbol, for a set without art. */
  iconUrl: string | null;
  setCode: string;
}

/** The art window of a set without art (or whose art failed to load): rays in the set's
 * colours around its expansion symbol (shown unaltered on a light disc) or its code. */
async function drawArtFallback(g: CanvasRenderingContext2D, look: PackLook, win: { x: number; y: number; w: number; h: number }) {
  const { hue } = look;
  const cx = win.x + win.w / 2;
  const cy = win.y + win.h / 2;
  g.save();
  g.beginPath();
  g.rect(win.x, win.y, win.w, win.h);
  g.clip();
  const base = g.createRadialGradient(cx, cy, 10, cx, cy, win.h * 0.75);
  base.addColorStop(0, `hsl(${hue} 85% 62%)`);
  base.addColorStop(0.5, `hsl(${(hue + 30) % 360} 75% 34%)`);
  base.addColorStop(1, `hsl(${(hue + 60) % 360} 70% 14%)`);
  g.fillStyle = base;
  g.fillRect(win.x, win.y, win.w, win.h);
  // Rays.
  const rays = 24;
  for (let i = 0; i < rays; i++) {
    const a0 = (i / rays) * Math.PI * 2;
    const a1 = a0 + Math.PI / rays;
    g.fillStyle = `hsl(${(hue + (i % 2) * 40) % 360} 90% 75% / 0.13)`;
    g.beginPath();
    g.moveTo(cx, cy);
    g.lineTo(cx + Math.cos(a0) * win.h, cy + Math.sin(a0) * win.h);
    g.lineTo(cx + Math.cos(a1) * win.h, cy + Math.sin(a1) * win.h);
    g.fill();
  }
  // Sparkles.
  let seed = hue + 1;
  const random = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
  for (let i = 0; i < 40; i++) {
    g.fillStyle = `hsl(${hue} 100% 92% / ${0.25 + random() * 0.5})`;
    g.beginPath();
    g.arc(win.x + random() * win.w, win.y + random() * win.h, 1 + random() * 2.5, 0, Math.PI * 2);
    g.fill();
  }
  const icon = look.iconUrl ? await loadImage(look.iconUrl) : null;
  const radius = 110;
  g.fillStyle = `hsl(${hue} 30% 94%)`;
  g.shadowColor = `hsl(${hue} 90% 70%)`;
  g.shadowBlur = 40;
  g.beginPath();
  g.arc(cx, cy, radius, 0, Math.PI * 2);
  g.fill();
  g.shadowBlur = 0;
  if (icon) {
    const size = radius * 1.25;
    const ratio = (icon.width || 1) / (icon.height || 1);
    const [iw, ih] = ratio > 1 ? [size, size / ratio] : [size * ratio, size];
    g.drawImage(icon, cx - iw / 2, cy - ih / 2, iw, ih);
  } else {
    g.fillStyle = `hsl(${hue} 60% 22%)`;
    g.textAlign = "center";
    g.textBaseline = "middle";
    g.font = `900 64px ${TITLE_FONT}`;
    g.fillText(look.setCode.toUpperCase(), cx, cy + 4, radius * 1.7);
    g.textBaseline = "alphabetic";
  }
  g.restore();
}

const TITLE_FONT = "Cinzel, Georgia, serif";

/** A gold gradient running down `y0`..`y1`, like the logo's gilding. */
function gold(g: CanvasRenderingContext2D, y0: number, y1: number) {
  const gradient = g.createLinearGradient(0, y0, 0, y1);
  gradient.addColorStop(0, "#fff4c8");
  gradient.addColorStop(0.35, "#f0c35e");
  gradient.addColorStop(0.55, "#a8742a");
  gradient.addColorStop(0.65, "#e9c271");
  gradient.addColorStop(1, "#6d4512");
  return gradient;
}

/** A gilded frame inside the wrapper's edges, with a flourish in each corner. */
function drawFrame(g: CanvasRenderingContext2D, x: number, y: number, w: number, h: number) {
  g.save();
  g.strokeStyle = gold(g, y, y + h);
  g.lineWidth = 4;
  g.strokeRect(x, y, w, h);
  g.lineWidth = 1.5;
  g.globalAlpha = 0.7;
  g.strokeRect(x + 9, y + 9, w - 18, h - 18);
  g.globalAlpha = 1;
  for (const [cx, cy, sx, sy] of [[x, y, 1, 1], [x + w, y, -1, 1], [x, y + h, 1, -1], [x + w, y + h, -1, -1]]) {
    g.save();
    g.translate(cx, cy);
    g.scale(sx, sy);
    g.lineWidth = 3;
    g.beginPath();
    g.moveTo(0, 46);
    g.quadraticCurveTo(0, 0, 46, 0);
    g.moveTo(14, 30);
    g.quadraticCurveTo(14, 14, 30, 14);
    g.stroke();
    g.fillStyle = "#f5d68a";
    g.beginPath();
    g.arc(20, 20, 4.5, 0, Math.PI * 2);
    g.fill();
    g.restore();
  }
  g.restore();
}

/** The logo's wax seal, a U pressed in it, at `cx`, `cy`, `size` wide. */
function drawSeal(g: CanvasRenderingContext2D, cx: number, cy: number, size: number) {
  const outline = new Path2D(SEAL_PATH);
  g.save();
  g.translate(cx, cy);
  g.scale(size / 130, size / 130);
  g.save();
  g.translate(2, 6);
  g.fillStyle = "#1b0f05aa";
  g.fill(outline);
  g.restore();
  const wax = g.createRadialGradient(-20, -25, 5, 0, 0, 80);
  wax.addColorStop(0, "#e2545a");
  wax.addColorStop(0.45, "#a61f28");
  wax.addColorStop(1, "#4f080e");
  g.fillStyle = wax;
  g.fill(outline);
  g.strokeStyle = "#3d060b";
  g.lineWidth = 2.5;
  g.stroke(outline);
  g.beginPath();
  g.arc(0, 0, 45, 0, Math.PI * 2);
  g.strokeStyle = gold(g, -45, 45);
  g.lineWidth = 3;
  g.stroke();
  g.font = `900 84px ${TITLE_FONT}`;
  g.textAlign = "center";
  g.fillStyle = gold(g, -30, 30);
  g.fillText("U", 0, 29);
  g.restore();
}

/** The wrapper's body, in the game's Grimoire style: foil in the set's colour inside a gilded
 * frame, the set's name engraved, its art set in gold, the booster kind on a leather ribbon,
 * and the logo's wax seal. */
export async function packTexture(look: PackLook): Promise<THREE.CanvasTexture> {
  const w = 512;
  const h = 860;
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const g = canvas.getContext("2d")!;
  const { hue } = look;
  // Canvas text only uses fonts already loaded.
  await Promise.all([document.fonts.load(`700 34px Cinzel`), document.fonts.load(`900 84px Cinzel`)]).catch(() => {});

  const bg = g.createLinearGradient(0, 0, w, h);
  bg.addColorStop(0, `hsl(${hue} 70% 46%)`);
  bg.addColorStop(0.5, `hsl(${(hue + 25) % 360} 75% 24%)`);
  bg.addColorStop(1, `hsl(${hue} 65% 38%)`);
  g.fillStyle = bg;
  g.fillRect(0, 0, w, h);
  // A sheen across the wrapper.
  const sheen = g.createLinearGradient(0, h * 0.2, w, h * 0.5);
  sheen.addColorStop(0, "#fff0");
  sheen.addColorStop(0.5, "#ffffff2a");
  sheen.addColorStop(1, "#fff0");
  g.fillStyle = sheen;
  g.fillRect(0, 0, w, h);
  // Fine diagonal ridges, like a foil wrapper.
  g.strokeStyle = `hsl(${hue} 70% 80% / 0.08)`;
  g.lineWidth = 2;
  for (let x = -h; x < w; x += 9) {
    g.beginPath();
    g.moveTo(x, h);
    g.lineTo(x + h, 0);
    g.stroke();
  }
  // Crimped bottom edge.
  for (let x = 0; x < w; x += 8) {
    g.fillStyle = x % 16 === 0 ? `hsl(${hue} 35% 62%)` : `hsl(${hue} 40% 38%)`;
    g.fillRect(x, h - 46, 8, 46);
  }

  drawFrame(g, 14, 14, w - 28, h - 74);

  g.textAlign = "center";
  g.shadowColor = "#000";
  g.shadowBlur = 6;
  g.shadowOffsetY = 2;
  // The set's name: one line, or two smaller ones when it is long.
  const nameWidth = w - 120;
  g.font = `700 30px ${TITLE_FONT}`;
  if (g.measureText(look.setName).width <= nameWidth) {
    g.fillStyle = gold(g, 48, 80);
    g.fillText(look.setName, w / 2, 76, nameWidth);
  } else {
    g.font = `700 22px ${TITLE_FONT}`;
    const words = look.setName.split(" ");
    let first = words.shift() ?? "";
    while (words.length > 0 && g.measureText(`${first} ${words[0]}`).width <= nameWidth) first += ` ${words.shift()}`;
    g.fillStyle = gold(g, 44, 66);
    g.fillText(first, w / 2, 62, nameWidth);
    g.fillStyle = gold(g, 70, 92);
    g.fillText(words.join(" "), w / 2, 88, nameWidth);
  }
  g.shadowOffsetY = 0;

  // The art window.
  const win = { x: 44, y: 100, w: w - 88, h: 500 };
  g.shadowBlur = 0;
  g.fillStyle = "#000";
  g.fillRect(win.x, win.y, win.w, win.h);
  const art = look.artUrl ? await loadImage(look.artUrl) : null;
  if (art) {
    // Cover the window (the art crop is only framed, never altered).
    const scale = Math.max(win.w / art.width, win.h / art.height);
    const sw = win.w / scale;
    const sh = win.h / scale;
    g.drawImage(art, (art.width - sw) / 2, (art.height - sh) / 2, sw, sh, win.x, win.y, win.w, win.h);
  } else {
    await drawArtFallback(g, look, win);
  }
  // The art set in gold (framed only: the art itself is never altered).
  g.strokeStyle = "#1b0f05";
  g.lineWidth = 10;
  g.strokeRect(win.x, win.y, win.w, win.h);
  g.strokeStyle = gold(g, win.y, win.y + win.h);
  g.lineWidth = 5;
  g.strokeRect(win.x - 1, win.y - 1, win.w + 2, win.h + 2);

  // The booster kind on a leather ribbon with swallowtail ends.
  const ribbon = { y: 618, h: 48, half: 170 };
  g.beginPath();
  g.moveTo(w / 2 - ribbon.half - 24, ribbon.y);
  g.lineTo(w / 2 + ribbon.half + 24, ribbon.y);
  g.lineTo(w / 2 + ribbon.half + 8, ribbon.y + ribbon.h / 2);
  g.lineTo(w / 2 + ribbon.half + 24, ribbon.y + ribbon.h);
  g.lineTo(w / 2 - ribbon.half - 24, ribbon.y + ribbon.h);
  g.lineTo(w / 2 - ribbon.half - 8, ribbon.y + ribbon.h / 2);
  g.closePath();
  const leather = g.createLinearGradient(0, ribbon.y, 0, ribbon.y + ribbon.h);
  leather.addColorStop(0, "#4a2f1b");
  leather.addColorStop(1, "#24160c");
  g.fillStyle = leather;
  g.shadowColor = "#000";
  g.shadowBlur = 10;
  g.fill();
  g.shadowBlur = 0;
  g.strokeStyle = gold(g, ribbon.y, ribbon.y + ribbon.h);
  g.lineWidth = 2.5;
  g.stroke();
  g.font = `700 24px ${TITLE_FONT}`;
  g.fillStyle = gold(g, ribbon.y + 12, ribbon.y + 40);
  g.fillText(look.kindLabel.toUpperCase(), w / 2, ribbon.y + 34, ribbon.half * 2 - 20);

  // The seal, and the game's name.
  drawSeal(g, w / 2, 712, 66);
  g.font = `700 19px ${TITLE_FONT}`;
  g.fillStyle = gold(g, 752, 770);
  g.fillText("U N S E A L E D", w / 2, 768);
  return canvasTexture(canvas);
}

/** The crimped strip at the top of the wrapper, torn off to open it: the set's colour,
 * crimped, with a gilded seam along its lower edge. */
export function stripTexture(hue: number): THREE.CanvasTexture {
  const canvas = document.createElement("canvas");
  canvas.width = 512;
  canvas.height = 48;
  const g = canvas.getContext("2d")!;
  for (let x = 0; x < 512; x += 8) {
    g.fillStyle = x % 16 === 0 ? `hsl(${hue} 35% 64%)` : `hsl(${hue} 40% 40%)`;
    g.fillRect(x, 0, 8, 48);
  }
  g.fillStyle = gold(g, 36, 44);
  g.fillRect(0, 38, 512, 5);
  return canvasTexture(canvas);
}

/** The cards' back: Unsealed's own design (not Magic's). */
export function cardBackTexture(): THREE.CanvasTexture {
  const canvas = document.createElement("canvas");
  canvas.width = 250;
  canvas.height = 350;
  const g = canvas.getContext("2d")!;
  g.fillStyle = "#0d1124";
  g.fillRect(0, 0, 250, 350);
  for (let y = -250; y < 350; y += 20) {
    g.fillStyle = "#26304f";
    g.beginPath();
    g.moveTo(0, y);
    g.lineTo(250, y + 250);
    g.lineTo(250, y + 260);
    g.lineTo(0, y + 10);
    g.fill();
  }
  g.strokeStyle = "#0d1124";
  g.lineWidth = 16;
  g.strokeRect(0, 0, 250, 350);
  g.fillStyle = "#e0a526";
  g.font = "700 110px system-ui, sans-serif";
  g.textAlign = "center";
  g.textBaseline = "middle";
  g.fillText("U", 125, 180);
  return canvasTexture(canvas);
}

/** A soft round glow, coloured at display time. */
export function glowTexture(): THREE.CanvasTexture {
  const canvas = document.createElement("canvas");
  canvas.width = 256;
  canvas.height = 256;
  const g = canvas.getContext("2d")!;
  const gradient = g.createRadialGradient(128, 128, 10, 128, 128, 128);
  gradient.addColorStop(0, "rgba(255,255,255,1)");
  gradient.addColorStop(0.45, "rgba(255,255,255,0.35)");
  gradient.addColorStop(1, "rgba(255,255,255,0)");
  g.fillStyle = gradient;
  g.fillRect(0, 0, 256, 256);
  return canvasTexture(canvas);
}
