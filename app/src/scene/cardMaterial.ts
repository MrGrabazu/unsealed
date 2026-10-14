import * as THREE from "three";

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
uniform float uTime;
uniform vec2 uLight;
varying vec2 vUv;
varying float vFacing;

vec3 hue(float h) {
  return clamp(abs(mod(h * 6.0 + vec3(0.0, 4.0, 2.0), 6.0) - 3.0) - 1.0, 0.0, 1.0);
}

void main() {
  vec4 c;
  if (vFacing > 0.0) {
    if (uHasFront > 0.5) {
      c = texture2D(uFront, vUv);
      if (uFoil > 0.5) {
        // The same foil as the CSS one (.foil-sheen): rainbow bands 18° off the vertical, one
        // period every 60% of the gradient line (2.07 units), drifting a period every 6 s, laid
        // in "overlay" at 75%, plus a white glint at half speed. The light (pointer) nudges it.
        vec2 p = vec2(vUv.x * ${CARD_W.toFixed(3)}, (1.0 - vUv.y) * ${CARD_H.toFixed(3)});
        float along = dot(p, vec2(0.951, 0.309)) / 2.07 - uLight.x * 0.25 + uLight.y * 0.1;
        vec3 rainbow = hue(fract(along - uTime / 6.0));
        vec3 overlay = mix(2.0 * c.rgb * rainbow, 1.0 - 2.0 * (1.0 - c.rgb) * (1.0 - rainbow), step(0.5, c.rgb));
        c.rgb = mix(c.rgb, overlay, 0.75);
        float g = fract(along * 2.0 - uTime / 6.0);
        c.rgb += vec3(0.25) * max(0.0, 1.0 - abs(g - 0.2) / 0.2);
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
  uTime: THREE.IUniform<number>;
  uLight: THREE.IUniform<THREE.Vector2>;
}

/** Materials of one card: [face and back, edge] (the geometry's groups 0 and 1). */
export function createCardMaterials(back: THREE.Texture, foil: boolean) {
  const uniforms: CardUniforms = {
    uFront: { value: null },
    uBack: { value: back },
    uHasFront: { value: 0 },
    uFoil: { value: foil ? 1 : 0 },
    uTime: { value: 0 },
    uLight: { value: new THREE.Vector2() },
  };
  const faces = new THREE.ShaderMaterial({ uniforms, vertexShader: VERTEX, fragmentShader: FRAGMENT });
  // The edge of the cardboard: off-white, like a real card's.
  const edge = new THREE.MeshStandardMaterial({ color: "#d9d4c7", roughness: 0.9 });
  return { faces, edge, uniforms, dispose: () => (faces.dispose(), edge.dispose()) };
}
