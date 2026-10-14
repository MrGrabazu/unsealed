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
        // The same foil as the CSS one (.foil-sheen): a faint rainbow (bands 18° off the
        // vertical, one period every 2.07 units, drifting a period every 9 s) in "soft-light",
        // alive only in a broad band of light, with a soft white glare. Here the light follows
        // the pointer: tilting the card moves the band, as with a real one.
        vec2 p = vec2(vUv.x * ${CARD_W.toFixed(3)}, (1.0 - vUv.y) * ${CARD_H.toFixed(3)});
        float along = dot(p, vec2(0.951, 0.309));
        vec3 rainbow = mix(vec3(0.5), hue(fract(along / 2.07 - uTime / 9.0)), 0.8);
        // Soft light (W3C): darkens or lightens a little, keeps the card's own colours.
        vec3 d = mix(sqrt(c.rgb), ((16.0 * c.rgb - 12.0) * c.rgb + 4.0) * c.rgb, step(c.rgb, vec3(0.25)));
        vec3 soft = mix(c.rgb - (1.0 - 2.0 * rainbow) * c.rgb * (1.0 - c.rgb), c.rgb + (2.0 * rainbow - 1.0) * (d - c.rgb), step(0.5, rainbow));
        float lightAt = 1.4 + uLight.x * 1.6 - uLight.y * 0.6 + sin(uTime * 0.6) * 0.3;
        float band = exp(-pow((along - lightAt) / 0.7, 2.0));
        c.rgb = mix(c.rgb, soft, 0.4 + 0.5 * band);
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
