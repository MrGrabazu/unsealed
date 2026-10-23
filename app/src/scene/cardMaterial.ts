import * as THREE from "three";
import { foilEffect } from "../foils";

// The cards of the 3D opening: rigid pieces of cardboard with a thickness, rounded corners,
// a face, a back and an edge. The face of a foil gets an iridescent sheen that shifts with
// the pointer (the "light").

/** Size of a card in scene units (63 × 88 mm) and its thickness. */
export const CARD_W = 2.5;
export const CARD_H = 3.49;
const CARD_DEPTH = 0.025;
const CORNER = 0.12;

/** A card-shaped slab: rounded rectangle extruded by the card's thickness. Its caps get UVs
 * spanning the whole card (0 to 1); group 0 holds both caps, group 1 the edge. */
export function createCardGeometry(): THREE.ExtrudeGeometry {
  const w = CARD_W / 2;
  const h = CARD_H / 2;
  const r = CORNER;
  const shape = new THREE.Shape();
  shape.moveTo(-w + r, -h);
  shape.lineTo(w - r, -h);
  shape.quadraticCurveTo(w, -h, w, -h + r);
  shape.lineTo(w, h - r);
  shape.quadraticCurveTo(w, h, w - r, h);
  shape.lineTo(-w + r, h);
  shape.quadraticCurveTo(-w, h, -w, h - r);
  shape.lineTo(-w, -h + r);
  shape.quadraticCurveTo(-w, -h, -w + r, -h);
  const geometry = new THREE.ExtrudeGeometry(shape, { depth: CARD_DEPTH, bevelEnabled: false, curveSegments: 6 });
  geometry.translate(0, 0, -CARD_DEPTH / 2);
  const pos = geometry.attributes.position;
  const uv = geometry.attributes.uv;
  for (let i = 0; i < pos.count; i++) {
    uv.setXY(i, (pos.getX(i) + w) / CARD_W, (pos.getY(i) + h) / CARD_H);
  }
  return geometry;
}

const VERTEX = /* glsl */ `
varying vec2 vUv;
varying float vFacing;
void main() {
  vUv = uv;
  vFacing = normal.z;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

const FRAGMENT = /* glsl */ `
uniform sampler2D uFront;
uniform sampler2D uBack;
uniform float uHasFront;
uniform float uFoil;
uniform float uKind;
uniform float uTime;
uniform vec2 uLight;
varying vec2 vUv;
varying float vFacing;

vec3 hue(float h) {
  return clamp(abs(mod(h * 6.0 + vec3(0.0, 4.0, 2.0), 6.0) - 3.0) - 1.0, 0.0, 1.0);
}

// Soft light (W3C) of a layer over the card: darkens or lightens a little, keeps the card's
// own colours.
vec3 softLight(vec3 b, vec3 s) {
  vec3 d = mix(sqrt(b), ((16.0 * b - 12.0) * b + 4.0) * b, step(b, vec3(0.25)));
  return mix(b - (1.0 - 2.0 * s) * b * (1.0 - b), b + (2.0 * s - 1.0) * (d - b), step(0.5, s));
}

float hash(vec2 q) { return fract(sin(dot(q, vec2(127.1, 311.7))) * 43758.5453); }
float noise(vec2 q) {
  vec2 i = floor(q), f = fract(q);
  vec2 u = f * f * (3.0 - 2.0 * f);
  return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x), mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

// The five colours of mana, going round.
vec3 mana(float t) {
  t = fract(t) * 5.0;
  vec3 w = vec3(1.0, 0.95, 0.77), u = vec3(0.29, 0.56, 0.89), b = vec3(0.35, 0.23, 0.42), r = vec3(0.89, 0.29, 0.23), g = vec3(0.23, 0.63, 0.29);
  if (t < 1.0) return mix(w, u, t);
  if (t < 2.0) return mix(u, b, t - 1.0);
  if (t < 3.0) return mix(b, r, t - 2.0);
  if (t < 4.0) return mix(r, g, t - 3.0);
  return mix(g, w, t - 4.0);
}

void main() {
  vec4 c;
  if (vFacing > 0.0) {
    if (uHasFront > 0.5) {
      c = texture2D(uFront, vUv);
      if (uFoil > 0.5) {
        // The same foil as the CSS one (.foil-sheen): a faint rainbow (bands 18° off the
        // vertical, one period every 2.07 units, drifting a period every 9 s) in "soft-light",
        // alive only in a broad band of light, with a soft white glare. Here the light follows
        // the pointer: tilting the card moves the band, as with a real one. The special foils
        // (uKind, see FOIL_KINDS) change it as foils.css does.
        vec2 p = vec2(vUv.x * ${CARD_W.toFixed(3)}, (1.0 - vUv.y) * ${CARD_H.toFixed(3)});
        float along = dot(p, vec2(0.951, 0.309));
        float lightAt = 1.4 + uLight.x * 1.6 - uLight.y * 0.6 + sin(uTime * 0.6) * 0.3;
        float band = exp(-pow((along - lightAt) / 0.7, 2.0));
        vec3 rainbow = mix(vec3(0.5), hue(fract(along / 2.07 - uTime / 9.0)), 0.8);
        vec3 base = c.rgb;
        int kind = int(uKind + 0.5);
        if (kind == 1) {
          // Etched: no rainbow, a matte metal shine and a fine engraving, only in the light.
          vec3 metal = mix(vec3(0.5), vec3(1.0, 0.96, 0.86), band);
          c.rgb = mix(base, softLight(base, metal), 0.15 + 0.6 * band);
          float hatch = step(0.82, fract((p.x + p.y) * 38.0)) + step(0.86, fract((p.x - p.y) * 30.0));
          c.rgb += vec3(0.1) * hatch * band + vec3(0.1, 0.09, 0.07) * band;
        } else if (kind == 5 || kind == 6) {
          // Gilded and silver: a metal over the card, its shine in the light, no rainbow.
          vec3 tint = kind == 5 ? vec3(1.0, 0.8, 0.42) : vec3(0.8, 0.83, 0.88);
          c.rgb = mix(base, softLight(base, mix(tint * 0.7, tint, band)), 0.55 + 0.35 * band);
          c.rgb += (kind == 5 ? vec3(0.22, 0.17, 0.08) : vec3(0.18)) * band;
        } else {
          vec3 layer = rainbow;
          float strength = 0.4 + 0.5 * band;
          if (kind == 4) {
            // Double rainbow: two rainbows crossing.
            float other = dot(p, vec2(0.5, -0.866));
            layer = mix(vec3(0.5), (hue(fract(along / 2.07 - uTime / 9.0)) + hue(fract(other / 2.07 + uTime / 9.0))) * 0.5, 0.95);
            strength = 0.6 + 0.3 * band;
          } else if (kind == 7) {
            // Fracture: shards (two crossed grids), each with its own reflection.
            vec2 a = mat2(0.94, -0.34, 0.34, 0.94) * p * 1.6, b = mat2(0.5, 0.87, -0.87, 0.5) * p * 1.3;
            float shard = hash(floor(a) + floor(b) * 7.0);
            layer = mix(vec3(0.5), hue(fract(along / 2.07 - uTime / 9.0 + shard)), 0.85);
            strength = 0.5 + 0.4 * band;
            float edge = min(min(fract(a.x), 1.0 - fract(a.x)), min(fract(b.y), 1.0 - fract(b.y)));
            c.rgb += vec3(0.5) * (1.0 - smoothstep(0.0, 0.03, edge)) * (0.35 + 0.65 * band);
          } else if (kind == 8) {
            // Halo: iridescent rings turning around the middle.
            vec2 q = p - vec2(${(CARD_W / 2).toFixed(3)}, ${(CARD_H * 0.45).toFixed(3)});
            float ring = 0.6 + 0.4 * cos(length(q) * 28.0);
            layer = mix(vec3(0.5), hue(fract(atan(q.y, q.x) / 3.1416 + uTime / 14.0)), 0.85 * ring);
            strength = 0.55 + 0.3 * band;
          } else if (kind == 9) {
            // Mana: the five colours of mana, swirling.
            vec2 q = p - vec2(${(CARD_W / 2).toFixed(3)}, ${(CARD_H / 2).toFixed(3)});
            layer = mix(vec3(0.5), mana(atan(q.y, q.x) / 6.2832 + uTime / 14.0), 0.85);
          } else if (kind == 10) {
            // Ripple: rings widening from the middle.
            vec2 q = p - vec2(${(CARD_W / 2).toFixed(3)}, ${(CARD_H / 2).toFixed(3)});
            strength *= 0.5 + 0.5 * smoothstep(0.6, 1.0, cos((length(q) - uTime * 0.35) * 30.0));
          } else if (kind == 11) {
            // Oil slick: dark, shifting swirls of colour.
            float n = noise(p * 2.2 + vec2(uTime * 0.05, 0.0)) + 0.5 * noise(p * 5.0);
            layer = mix(vec3(0.45), hue(fract(n * 0.8 + uTime / 12.0)), 0.9);
            strength = 0.55 + 0.3 * band;
          } else if (kind == 3) {
            // Surge: the usual strength, with fine bright waves rising up the card.
            float wave = fract(length((p - vec2(${(CARD_W / 2).toFixed(3)}, ${(CARD_H * 1.3).toFixed(3)})) * vec2(0.42, 1.0)) / 0.09 + uTime / 1.6);
            c.rgb += vec3(0.55) * (1.0 - smoothstep(0.0, 0.1, abs(wave - 0.5))) * (0.6 + 0.4 * band);
          }
          c.rgb = mix(c.rgb, softLight(c.rgb, layer), strength);
          if (kind == 2) {
            // Galaxy: a nebula's tint and a field of stars, each twinkling at its own pace.
            c.rgb = mix(c.rgb, softLight(c.rgb, vec3(0.45, 0.4, 0.75)), 0.35);
            vec2 cell = floor(p * 26.0);
            float h = hash(cell);
            vec2 at = (cell + 0.2 + 0.6 * vec2(h, hash(cell + 3.1))) / 26.0;
            float size = h > 0.97 ? 0.012 : 0.006;
            float twinkle = 0.5 + 0.5 * sin(uTime * (1.5 + 2.0 * h) + h * 40.0);
            c.rgb += vec3(0.95, 0.95, 1.0) * step(0.78, h) * (1.0 - smoothstep(0.0, size, length(p - at))) * twinkle;
          } else if (kind == 12) {
            // Textured: a relief catching the light.
            c.rgb += vec3(0.22) * (noise(p * 18.0) - 0.5) * (0.4 + band);
          } else if (kind == 13) {
            // Neon ink: neon glows pulsing.
            float pulse = 0.5 + 0.5 * sin(uTime * 2.8);
            c.rgb += hue(fract(uTime / 6.0)) * 0.25 * pulse * exp(-length(p - vec2(0.6, 1.0)) * 1.4)
              + hue(fract(uTime / 6.0 + 0.4)) * 0.25 * (1.0 - pulse) * exp(-length(p - vec2(1.9, 2.3)) * 1.4);
          }
        }
        c.rgb += vec3(0.18) * band;
      }
    } else {
      c = texture2D(uBack, vUv);
    }
  } else {
    // Seen from behind: the back, mirrored.
    c = texture2D(uBack, vec2(1.0 - vUv.x, vUv.y));
  }
  gl_FragColor = c;
  #include <colorspace_fragment>
}
`;

export interface CardUniforms {
  [name: string]: THREE.IUniform;
  uFront: THREE.IUniform<THREE.Texture | null>;
  uBack: THREE.IUniform<THREE.Texture | null>;
  uHasFront: THREE.IUniform<number>;
  uFoil: THREE.IUniform<number>;
  uKind: THREE.IUniform<number>;
  uTime: THREE.IUniform<number>;
  uLight: THREE.IUniform<THREE.Vector2>;
}

/** The shader's number for each look of foils.tsx (`foilEffect`); 0 is the usual foil. */
const KINDS: Record<string, number> = {
  plain: 0, etched: 1, galaxy: 2, surge: 3, rainbow: 4, gilded: 5, silver: 6, fracture: 7, halo: 8, mana: 9, ripple: 10, oil: 11,
  textured: 12, neon: 13,
};

/** Materials of one card: [face and back, edge] (the geometry's groups 0 and 1). */
export function createCardMaterials(back: THREE.Texture, foil: boolean, foilKind?: string | null) {
  const uniforms: CardUniforms = {
    uFront: { value: null },
    uBack: { value: back },
    uHasFront: { value: 0 },
    uFoil: { value: foil ? 1 : 0 },
    uKind: { value: KINDS[foilEffect(foilKind)] ?? 0 },
    uTime: { value: 0 },
    uLight: { value: new THREE.Vector2() },
  };
  const faces = new THREE.ShaderMaterial({ uniforms, vertexShader: VERTEX, fragmentShader: FRAGMENT });
  // The edge of the cardboard: off-white, like a real card's.
  const edge = new THREE.MeshStandardMaterial({ color: "#d9d4c7", roughness: 0.9 });
  return { faces, edge, uniforms, dispose: () => (faces.dispose(), edge.dispose()) };
}
