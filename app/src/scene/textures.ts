import * as THREE from "three";
import type { WrapperFinish } from "../boosterKinds";
import { cardBackCanvas, cardBackReady } from "../canvasArt";
import { drawnPack, edgeColours, packCanvas, trim, type PackLook } from "./packArt";

export type { PackLook } from "./packArt";

// Textures of the 3D opening, drawn on canvases: the wrapper's print (`packArt.ts`), its
// tear-off strip, the cards' back, and the glow behind a big pull. Card faces come from
// Scryfall's CDN, which allows cross-origin use.

const loader = new THREE.TextureLoader();
loader.setCrossOrigin("anonymous");

function canvasTexture(canvas: HTMLCanvasElement): THREE.CanvasTexture {
  const texture = new THREE.CanvasTexture(canvas);
  texture.colorSpace = THREE.SRGBColorSpace;
  texture.anisotropy = 4;
  return texture;
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

/** The wrapper's body, its print drawn once a session (see `packCanvas`). */
export async function packTexture(look: PackLook): Promise<THREE.CanvasTexture> {
  return canvasTexture(await packCanvas(look));
}

/** The wrapper's body at once, when its print is drawn already (ahead, by the reserve). */
export function drawnPackTexture(look: PackLook): THREE.CanvasTexture | null {
  const canvas = drawnPack(look);
  return canvas ? canvasTexture(canvas) : null;
}

/** The crimped strip at the top of the wrapper, torn off to open it: the set's colour (or the
 * finish's), crimped, with a seam along its lower edge in the finish's metal. */
export function stripTexture(hue: number, finish: WrapperFinish): THREE.CanvasTexture {
  const canvas = document.createElement("canvas");
  canvas.width = 512;
  canvas.height = 48;
  const g = canvas.getContext("2d")!;
  const [light, dark] = edgeColours(hue, finish);
  for (let x = 0; x < 512; x += 8) {
    g.fillStyle = x % 16 === 0 ? light : dark;
    g.fillRect(x, 0, 8, 48);
  }
  g.fillStyle = trim(g, finish, 36, 44);
  g.fillRect(0, 38, 512, 5);
  return canvasTexture(canvas);
}

/** The cards' back (see `canvasArt.ts`) as a texture, redrawn once its font is loaded. */
export function cardBackTexture(): THREE.CanvasTexture {
  const texture = canvasTexture(cardBackCanvas);
  void cardBackReady.then(() => (texture.needsUpdate = true));
  return texture;
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
