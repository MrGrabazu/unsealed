import { SEAL_PATH } from "./logoSvg";

// Drawings on canvases shared by the 3D opening (scene/textures.ts) and the rest of the game,
// kept apart from three.js so that the classic opening does not load it: the gilding, the
// logo's wax seal and the cards' back.

export const TITLE_FONT = "Cinzel, Georgia, serif";

/** A gold gradient running down `y0`..`y1`, like the logo's gilding. */
export function gold(g: CanvasRenderingContext2D, y0: number, y1: number) {
  const gradient = g.createLinearGradient(0, y0, 0, y1);
  gradient.addColorStop(0, "#fff4c8");
  gradient.addColorStop(0.35, "#f0c35e");
  gradient.addColorStop(0.55, "#a8742a");
  gradient.addColorStop(0.65, "#e9c271");
  gradient.addColorStop(1, "#6d4512");
  return gradient;
}

/** The logo's wax seal, a U pressed in it, at `cx`, `cy`, `size` wide. */
export function drawSeal(g: CanvasRenderingContext2D, cx: number, cy: number, size: number) {
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

/** Size of the cards' back (63 × 88, like a card). */
const BACK_W = 630;
const BACK_H = 880;

/** The cards' back: Unsealed's own design, evoking the familiar brown back of a card game
 * without copying it (no logo, no lettering, no gems of Magic's): marbled brown swirls, a dark
 * oval ringed with gold, the logo's wax seal and the game's name. */
function drawCardBack(canvas: HTMLCanvasElement) {
  canvas.width = BACK_W;
  canvas.height = BACK_H;
  const g = canvas.getContext("2d")!;
  const cx = BACK_W / 2;
  const cy = BACK_H / 2;

  const base = g.createRadialGradient(cx, cy, 40, cx, cy, 560);
  base.addColorStop(0, "#6b4524");
  base.addColorStop(0.6, "#4a2c14");
  base.addColorStop(1, "#24130a");
  g.fillStyle = base;
  g.fillRect(0, 0, BACK_W, BACK_H);

  // Marbled swirls turning around the centre, always the same (a fixed seed).
  let seed = 7;
  const random = () => (seed = (seed * 16807) % 2147483647) / 2147483647;
  const tones = ["#8a5a2b", "#3a2210", "#a06c38", "#2b180a", "#7a4f26"];
  g.lineCap = "round";
  for (let i = 0; i < 170; i++) {
    const start = random() * Math.PI * 2;
    const radius = 60 + random() * 520;
    const sweep = 0.6 + random() * 1.4;
    g.strokeStyle = tones[Math.floor(random() * tones.length)];
    g.globalAlpha = 0.12 + random() * 0.22;
    g.lineWidth = 4 + random() * 26;
    g.beginPath();
    for (let s = 0; s <= 24; s++) {
      const a = start + (sweep * s) / 24;
      const r = radius * (1 - 0.18 * (s / 24));
      const x = cx + Math.cos(a) * r * 0.82;
      const y = cy + Math.sin(a) * r;
      if (s === 0) g.moveTo(x, y);
      else g.lineTo(x, y);
    }
    g.stroke();
  }
  g.globalAlpha = 1;

  // Darker edges, then a fine gilded line inside them.
  const vignette = g.createRadialGradient(cx, cy, 260, cx, cy, 620);
  vignette.addColorStop(0, "#00000000");
  vignette.addColorStop(1, "#000000aa");
  g.fillStyle = vignette;
  g.fillRect(0, 0, BACK_W, BACK_H);
  g.strokeStyle = gold(g, 0, BACK_H);
  g.globalAlpha = 0.55;
  g.lineWidth = 3;
  g.strokeRect(26, 26, BACK_W - 52, BACK_H - 52);
  g.globalAlpha = 1;

  // The oval: dark leather ringed with gold.
  const rx = 228;
  const ry = 330;
  g.save();
  g.shadowColor = "#000000cc";
  g.shadowBlur = 30;
  const leather = g.createRadialGradient(cx, cy - 80, 30, cx, cy, ry);
  leather.addColorStop(0, "#4a2c16");
  leather.addColorStop(1, "#160b04");
  g.fillStyle = leather;
  g.beginPath();
  g.ellipse(cx, cy, rx, ry, 0, 0, Math.PI * 2);
  g.fill();
  g.restore();
  g.strokeStyle = gold(g, cy - ry, cy + ry);
  g.lineWidth = 12;
  g.beginPath();
  g.ellipse(cx, cy, rx, ry, 0, 0, Math.PI * 2);
  g.stroke();
  g.lineWidth = 2.5;
  g.beginPath();
  g.ellipse(cx, cy, rx - 20, ry - 20, 0, 0, Math.PI * 2);
  g.stroke();

  drawSeal(g, cx, cy - 40, 300);
  g.font = `900 58px ${TITLE_FONT}`;
  g.textAlign = "center";
  g.textBaseline = "alphabetic";
  g.fillStyle = "#1b0f05";
  g.fillText("UNSEALED", cx, cy + 192);
  g.fillStyle = gold(g, cy + 145, cy + 192);
  g.fillText("UNSEALED", cx, cy + 188);
}

/** The back, drawn once and shared. Drawn again once the title font is loaded, so that its
 * lettering is in Cinzel. */
export const cardBackCanvas = (() => {
  const canvas = document.createElement("canvas");
  drawCardBack(canvas);
  return canvas;
})();
export const cardBackReady: Promise<HTMLCanvasElement> = document.fonts
  .load(`900 58px ${TITLE_FONT}`)
  .catch(() => [])
  .then(() => {
    drawCardBack(cardBackCanvas);
    return cardBackCanvas;
  });

/** The cards' back as an image (the classic opening). */
export function cardBackUrl(): Promise<string> {
  return cardBackReady.then((canvas) => canvas.toDataURL("image/jpeg", 0.9));
}
