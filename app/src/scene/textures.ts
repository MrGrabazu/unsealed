import * as THREE from "three";

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
    g.font = "800 72px system-ui, 'Segoe UI', sans-serif";
    g.fillText(look.setCode.toUpperCase(), cx, cy + 4, radius * 1.7);
    g.textBaseline = "alphabetic";
  }
  g.restore();
}

/** The wrapper's body: metallic background, set name, art window, booster kind, brand. */
export async function packTexture(look: PackLook): Promise<THREE.CanvasTexture> {
  const w = 512;
  const h = 860;
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const g = canvas.getContext("2d")!;
  const { hue } = look;

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

  g.textAlign = "center";
  g.fillStyle = "#fff";
  g.shadowColor = "#000";
  g.shadowBlur = 8;
  g.font = "700 34px system-ui, 'Segoe UI', sans-serif";
  g.fillText(look.setName, w / 2, 70, w - 60);

  // The art window.
  const win = { x: 34, y: 100, w: w - 68, h: 520 };
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
  g.strokeStyle = `hsl(${hue} 50% 72%)`;
  g.lineWidth = 6;
  g.strokeRect(win.x, win.y, win.w, win.h);

  g.fillStyle = "#0009";
  g.fillRect(w / 2 - 150, 650, 300, 46);
  g.fillStyle = "#fff";
  g.font = "700 26px system-ui, 'Segoe UI', sans-serif";
  g.fillText(look.kindLabel.toUpperCase(), w / 2, 682, 280);

  g.fillStyle = `hsl(${hue} 70% 86%)`;
  g.shadowBlur = 10;
  g.font = "800 46px system-ui, 'Segoe UI', sans-serif";
  g.fillText("UNSEALED", w / 2, 770);
  return canvasTexture(canvas);
}

/** The crimped strip at the top of the wrapper, torn off to open it. */
export function stripTexture(hue: number): THREE.CanvasTexture {
  const canvas = document.createElement("canvas");
  canvas.width = 512;
  canvas.height = 48;
  const g = canvas.getContext("2d")!;
  for (let x = 0; x < 512; x += 8) {
    g.fillStyle = x % 16 === 0 ? `hsl(${hue} 35% 64%)` : `hsl(${hue} 40% 40%)`;
    g.fillRect(x, 0, 8, 48);
  }
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
