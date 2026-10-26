import type { SetSummary } from "../api";
import { kindLabel, wrapperFinish, type WrapperFinish } from "../boosterKinds";
import { drawSeal, gold, TITLE_FONT } from "../canvasArt";
import type { I18n } from "../i18n";
import { hueOf } from "../Pack";

// The print of the 3D opening's wrapper, drawn on a canvas: the original wrapper (with the
// set's art in a window, the art itself left unaltered) and its tear-off strip. Without
// three.js, so that the reserve can draw a booster's print ahead, before the scene loads.

export interface PackLook {
  hue: number;
  setName: string;
  kindLabel: string;
  /** The booster kind's material (see `wrapperFinish`). */
  finish: WrapperFinish;
  artUrl: string | null;
  /** The expansion symbol, for a set without art. */
  iconUrl: string | null;
  setCode: string;
}

/** The look of a set's booster of a kind. */
export function packLookOf(setCode: string, set: SetSummary | undefined, kind: string, t: I18n["t"]): PackLook {
  return {
    hue: hueOf(setCode),
    setName: set?.name ?? setCode,
    setCode,
    kindLabel: kindLabel(t, kind),
    finish: wrapperFinish(kind),
    artUrl: set?.art?.imageUrl ?? null,
    iconUrl: set?.iconUrl ?? null,
  };
}

/** Prints already drawn, by look: each wrapper is drawn once a session. Until its print is
 * ready the wrapper is a plain colour, close to it on most kinds but black on a collector:
 * drawn anew at each opening (its fonts, its art, then the drawing), its print showed late. */
const drawn = new Map<string, HTMLCanvasElement>();
const drawing = new Map<string, Promise<HTMLCanvasElement>>();
/** At most this many prints kept (some 1.7 MB each). */
const KEPT = 16;

const keyOf = (look: PackLook) =>
  [look.setCode, look.setName, look.kindLabel, look.finish, look.hue, look.artUrl, look.iconUrl].join("|");

/** The wrapper's print, if already drawn. */
export function drawnPack(look: PackLook): HTMLCanvasElement | undefined {
  return drawn.get(keyOf(look));
}

/** The wrapper's print, drawn once and kept; a print whose art could not be loaded (offline,
 * say) is not kept, to be drawn again with it next time. */
export function packCanvas(look: PackLook): Promise<HTMLCanvasElement> {
  const key = keyOf(look);
  const done = drawn.get(key);
  if (done) return Promise.resolve(done);
  let pending = drawing.get(key);
  if (!pending) {
    pending = drawPack(look).then(({ canvas, complete }) => {
      drawing.delete(key);
      if (complete) {
        drawn.set(key, canvas);
        if (drawn.size > KEPT) drawn.delete(drawn.keys().next().value!);
      }
      return canvas;
    });
    drawing.set(key, pending);
  }
  return pending;
}

/** Draws a booster's print ahead, so the 3D opening shows it at once. */
export function preparePack(look: PackLook) {
  void packCanvas(look);
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

/** The art window of a set without art (or whose art failed to load): rays in the set's
 * colours around its expansion symbol (shown unaltered on a light disc) or its code. Says
 * whether the symbol, if any, could be loaded. */
async function drawArtFallback(g: CanvasRenderingContext2D, look: PackLook, win: { x: number; y: number; w: number; h: number }): Promise<boolean> {
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
  return icon != null || !look.iconUrl;
}

/** Brushed silver, as `gold`: the trims of the set boosters. */
function silver(g: CanvasRenderingContext2D, y0: number, y1: number) {
  const gradient = g.createLinearGradient(0, y0, 0, y1);
  gradient.addColorStop(0, "#ffffff");
  gradient.addColorStop(0.35, "#d6dae2");
  gradient.addColorStop(0.55, "#7d838d");
  gradient.addColorStop(0.65, "#c9ced6");
  gradient.addColorStop(1, "#4a5058");
  return gradient;
}

/** The trims' metal for a finish: bronze on matte paper, silver on a set booster, else gold. */
export function trim(g: CanvasRenderingContext2D, finish: WrapperFinish, y0: number, y1: number): CanvasGradient | string {
  if (finish === "matte") return "#b9a37a";
  // On the prism's pearl, ink: gold would not show.
  if (finish === "prism") return "#1a1410";
  return finish === "silver" ? silver(g, y0, y1) : gold(g, y0, y1);
}

/** The holographic rainbow of the collector boosters, across `w`. */
function holo(g: CanvasRenderingContext2D, w: number, h: number) {
  const gradient = g.createLinearGradient(0, 0, w, h);
  ["#ff7eb3", "#ffd36e", "#7effc6", "#7ec8ff", "#c58bff", "#ff7eb3"].forEach((c, i, all) => gradient.addColorStop(i / (all.length - 1), c));
  return gradient;
}

/** A gilded frame inside the wrapper's edges, with a flourish in each corner. */
function drawFrame(g: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, finish: WrapperFinish) {
  g.save();
  g.strokeStyle = trim(g, finish, y, y + h);
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
    g.fillStyle = finish === "silver" ? "#f2f4f8" : finish === "matte" ? "#d8c8a6" : finish === "prism" ? "#1a1410" : "#f5d68a";
    g.beginPath();
    g.arc(20, 20, 4.5, 0, Math.PI * 2);
    g.fill();
    g.restore();
  }
  g.restore();
}

/** The wrapper's body, in the game's Grimoire style: foil in the set's colour inside a gilded
 * frame, the set's name engraved, its art set in gold, the booster kind on a leather ribbon,
 * and the logo's wax seal. `complete`: its art (or symbol) did load. */
async function drawPack(look: PackLook): Promise<{ canvas: HTMLCanvasElement; complete: boolean }> {
  const w = 512;
  const h = 860;
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const g = canvas.getContext("2d")!;
  const { hue, finish } = look;
  // Canvas text only uses fonts already loaded; the art loads meanwhile.
  const artLoading = look.artUrl ? loadImage(look.artUrl) : Promise.resolve(null);
  await Promise.all([document.fonts.load(`700 34px Cinzel`), document.fonts.load(`900 84px Cinzel`)]).catch(() => {});

  // The material: matte paper (draft), foil in the set's colour (play), a brighter foil (set),
  // black lacquer (collector), iridescent pearl (prism: a box topper).
  const bg = g.createLinearGradient(0, 0, w, h);
  const stops: [number, string][] =
    finish === "matte"
      ? [[0, `hsl(${hue} 26% 32%)`], [0.5, `hsl(${hue} 28% 20%)`], [1, `hsl(${hue} 24% 26%)`]]
      : finish === "collector"
        ? [[0, "#2a241d"], [0.5, "#0a0806"], [1, "#1c1813"]]
        : finish === "prism"
          ? [[0, "#f2eee8"], [0.55, "#cfc9c0"], [1, "#ece6de"]]
          : finish === "silver"
            ? [[0, `hsl(${hue} 70% 54%)`], [0.5, `hsl(${(hue + 25) % 360} 75% 28%)`], [1, `hsl(${hue} 65% 46%)`]]
            : [[0, `hsl(${hue} 70% 46%)`], [0.5, `hsl(${(hue + 25) % 360} 75% 24%)`], [1, `hsl(${hue} 65% 38%)`]];
  for (const [at, color] of stops) bg.addColorStop(at, color);
  g.fillStyle = bg;
  g.fillRect(0, 0, w, h);
  if (finish === "prism") {
    // The pearl's iridescence: a pastel rainbow turning from the top left (still here; the
    // wrapper in the reserve has it turn).
    const iris = g.createConicGradient(0.6, w * 0.3, h * 0.2);
    ["#ffb3d1", "#ffe9a8", "#b3ffdc", "#b3dcff", "#d9b3ff", "#ffb3d1"].forEach((c, i, all) => iris.addColorStop(i / (all.length - 1), c));
    g.globalAlpha = 0.62;
    g.fillStyle = iris;
    g.fillRect(0, 0, w, h);
    g.globalAlpha = 1;
  }
  if (finish !== "matte") {
    // A sheen across the wrapper (brighter on a set booster).
    const sheen = g.createLinearGradient(0, h * 0.2, w, h * 0.5);
    sheen.addColorStop(0, "#fff0");
    sheen.addColorStop(0.5, finish === "silver" ? "#ffffff44" : "#ffffff2a");
    sheen.addColorStop(1, "#fff0");
    g.fillStyle = sheen;
    g.fillRect(0, 0, w, h);
    // Fine diagonal ridges, like a foil wrapper.
    g.strokeStyle = finish === "collector" ? "#ffffff0d" : finish === "prism" ? "#ffffff30" : `hsl(${hue} 70% 80% / 0.08)`;
    g.lineWidth = 2;
    for (let x = -h; x < w; x += 9) {
      g.beginPath();
      g.moveTo(x, h);
      g.lineTo(x + h, 0);
      g.stroke();
    }
  }
  // Crimped bottom edge: gilded on a collector booster.
  const crimp = edgeColours(hue, finish);
  for (let x = 0; x < w; x += 8) {
    g.fillStyle = x % 16 === 0 ? crimp[0] : crimp[1];
    g.fillRect(x, h - 46, 8, 46);
  }
  if (finish === "collector" || finish === "prism") {
    // A holographic rule just inside the edges.
    g.strokeStyle = holo(g, w, h);
    g.lineWidth = 5;
    g.strokeRect(6, 6, w - 12, h - 58);
  } else if (finish === "silver") {
    g.strokeStyle = "#e8e8f0aa";
    g.lineWidth = 3;
    g.strokeRect(6, 6, w - 12, h - 58);
  }

  drawFrame(g, 14, 14, w - 28, h - 74, finish);

  g.textAlign = "center";
  // On the prism's pearl, the name in ink with a light shadow; gold in the shadow elsewhere.
  const prism = finish === "prism";
  g.shadowColor = prism ? "#ffffffaa" : "#000";
  g.shadowBlur = prism ? 2 : 6;
  g.shadowOffsetY = prism ? 1 : 2;
  const nameInk = (y0: number, y1: number) => (prism ? "#2a2018" : gold(g, y0, y1));
  // The set's name: one line, or two smaller ones when it is long.
  const nameWidth = w - 120;
  g.font = `700 30px ${TITLE_FONT}`;
  if (g.measureText(look.setName).width <= nameWidth) {
    g.fillStyle = nameInk(48, 80);
    g.fillText(look.setName, w / 2, 76, nameWidth);
  } else {
    g.font = `700 22px ${TITLE_FONT}`;
    const words = look.setName.split(" ");
    let first = words.shift() ?? "";
    while (words.length > 0 && g.measureText(`${first} ${words[0]}`).width <= nameWidth) first += ` ${words.shift()}`;
    g.fillStyle = nameInk(44, 66);
    g.fillText(first, w / 2, 62, nameWidth);
    g.fillStyle = nameInk(70, 92);
    g.fillText(words.join(" "), w / 2, 88, nameWidth);
  }
  g.shadowOffsetY = 0;

  // The art window.
  const win = { x: 44, y: 100, w: w - 88, h: 500 };
  g.shadowBlur = 0;
  g.fillStyle = "#000";
  g.fillRect(win.x, win.y, win.w, win.h);
  const art = await artLoading;
  let complete = true;
  if (art) {
    // Cover the window (the art crop is only framed, never altered).
    const scale = Math.max(win.w / art.width, win.h / art.height);
    const sw = win.w / scale;
    const sh = win.h / scale;
    g.drawImage(art, (art.width - sw) / 2, (art.height - sh) / 2, sw, sh, win.x, win.y, win.w, win.h);
  } else {
    complete = (await drawArtFallback(g, look, win)) && !look.artUrl;
  }
  // The art set in gold (framed only: the art itself is never altered).
  g.strokeStyle = "#1b0f05";
  g.lineWidth = 10;
  g.strokeRect(win.x, win.y, win.w, win.h);
  g.strokeStyle = trim(g, finish, win.y, win.y + win.h);
  g.lineWidth = 5;
  if (finish === "collector" || finish === "prism") {
    g.shadowColor = finish === "prism" ? "#c58bff" : "#f5b84a";
    g.shadowBlur = 18;
  }
  g.strokeRect(win.x - 1, win.y - 1, win.w + 2, win.h + 2);
  g.shadowBlur = 0;

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
  // Leather (plain on matte paper), brushed silver on a set booster, a rainbow on a collector.
  const leather = g.createLinearGradient(0, ribbon.y, 0, ribbon.y + ribbon.h);
  if (finish === "silver") {
    leather.addColorStop(0, "#9aa0ab");
    leather.addColorStop(1, "#4a5058");
  } else if (finish === "matte") {
    leather.addColorStop(0, "#33251a");
    leather.addColorStop(1, "#22180f");
  } else {
    leather.addColorStop(0, "#4a2f1b");
    leather.addColorStop(1, "#24160c");
  }
  g.fillStyle = finish === "collector" ? holo(g, w, 0) : finish === "prism" ? "#14100c" : leather;
  g.shadowColor = "#000";
  g.shadowBlur = 10;
  g.fill();
  g.shadowBlur = 0;
  // The prism's black ribbon: a gold rule and gold letters.
  g.strokeStyle = prism ? gold(g, ribbon.y, ribbon.y + ribbon.h) : trim(g, finish, ribbon.y, ribbon.y + ribbon.h);
  g.lineWidth = 2.5;
  g.stroke();
  g.font = `700 24px ${TITLE_FONT}`;
  g.fillStyle = finish === "collector" ? "#1a0d03" : finish === "silver" ? "#ffffff" : finish === "matte" ? "#d8c8a6" : gold(g, ribbon.y + 12, ribbon.y + 40);
  g.fillText(look.kindLabel.toUpperCase(), w / 2, ribbon.y + 34, ribbon.half * 2 - 20);

  // The seal (gold on a collector booster or a box topper), and the game's name.
  drawSeal(g, w / 2, 712, 66, finish === "collector" || prism);
  g.font = `700 19px ${TITLE_FONT}`;
  g.fillStyle = trim(g, finish, 752, 770);
  g.fillText("U N S E A L E D", w / 2, 768);
  return { canvas, complete };
}

/** The two colours of a crimped edge: the set's colour, muted on matte paper, gilded on a
 * collector booster. */
export function edgeColours(hue: number, finish: WrapperFinish): [string, string] {
  if (finish === "collector") return ["#e2b45a", "#b07d2c"];
  if (finish === "prism") return ["#f2eee8", "#c9c3ba"];
  if (finish === "matte") return [`hsl(${hue} 22% 46%)`, `hsl(${hue} 25% 28%)`];
  return [`hsl(${hue} 35% 64%)`, `hsl(${hue} 40% 40%)`];
}
