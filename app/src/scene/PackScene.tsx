import { Canvas, useFrame, useThree, type ThreeEvent } from "@react-three/fiber";
import { useEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";
import type { OpenedCard } from "../api";
import { isBigCard } from "../BatchOpening";
import { largeImage } from "../images";
import { revealSound, sounds } from "../sound";
import { CARD_H, CARD_W, createCardGeometry, createCardMaterials } from "./cardMaterial";
import { cardBackTexture, drawnPackTexture, glowTexture, loadCardTexture, packTexture, stripTexture, type PackLook } from "./textures";

// The 3D opening. The pack tilts with the pointer, shakes when held and moved, and its top
// strip tears off as the pointer slides along it, peeling back from where the tear started.
// Out come the cards, back up as from a real booster, rigid like real cardboard; once in place
// the pile is turned over in one go, and the top one is slid off the pile
// (dragged in any direction, or clicked) to uncover the next, in a real booster's order (the
// rare, then the foils, the token last). The last one
// leaves on its own after a moment, and the scene makes room for the list of the cards.

const PACK_W = 2.7;
const PACK_H = 4.3;
const STRIP_H = 0.34;
const STRIP_COLUMNS = 48;
/** Distance a card must be slid to leave the pile (scene units). */
const SLIDE_AWAY = 1.3;
/** Once torn: time for the cards to slide out of the wrapper, then for the wrapper to fall
 * away while the pile settles (seconds). */
const CARDS_OUT = 0.85;
const PACK_FALLS = 0.6;
/** The pile comes out back up, as from a real booster, then is turned over in one go to show
 * its first card (seconds). */
const PILE_TURNS = 0.5;
const SETTLED = CARDS_OUT + PACK_FALLS + PILE_TURNS;
/** Out of the wrapper, the pile is brought closer to the player: its cards read better. */
const CARD_ZOOM = 1.5;
/** How long the last card stays before leaving (seconds). */
const LAST_CARD_STAYS = 2.2;

const raycaster = new THREE.Raycaster();
const plane = new THREE.Plane(new THREE.Vector3(0, 0, 1), 0);

/** Where the pointer (client coordinates) meets the plane z = `z`, in world space. */
function pointOnPlane(clientX: number, clientY: number, camera: THREE.Camera, dom: HTMLElement, z: number) {
  const rect = dom.getBoundingClientRect();
  const ndc = new THREE.Vector2(((clientX - rect.left) / rect.width) * 2 - 1, -((clientY - rect.top) / rect.height) * 2 + 1);
  raycaster.setFromCamera(ndc, camera);
  plane.constant = -z;
  return raycaster.ray.intersectPlane(plane, new THREE.Vector3()) ?? new THREE.Vector3();
}

/** Follows a drag outside the mesh that started it, until the button is released. */
function followDrag(onMove: (e: PointerEvent) => void, onUp: (e: PointerEvent) => void) {
  const up = (e: PointerEvent) => {
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", up);
    onUp(e);
  };
  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", up);
}

const ease = (t: number) => 1 - Math.pow(1 - Math.min(1, Math.max(0, t)), 3);

// ---- the pack ------------------------------------------------------------------------------

function Pack({ look, onTorn, leaving, tearRequest }: { look: PackLook; onTorn: () => void; leaving: boolean; tearRequest: number }) {
  const camera = useThree((s) => s.camera);
  const dom = useThree((s) => s.gl.domElement);
  const group = useRef<THREE.Group>(null);
  const stripMesh = useRef<THREE.Mesh>(null);
  const bodyMaterial = useRef<THREE.MeshStandardMaterial>(null);
  // The print: at once when the reserve drew it ahead, else as soon as it is drawn.
  const drawnBody = useMemo(() => drawnPackTexture(look), [look]);
  const [loaded, setLoaded] = useState<THREE.Texture | null>(null);
  const body = drawnBody ?? loaded;
  const strip = useMemo(() => stripTexture(look.hue, look.finish), [look.hue, look.finish]);

  const bodyGeometry = useMemo(() => {
    // A slightly puffed pillow, like a sealed wrapper.
    const g = new THREE.PlaneGeometry(PACK_W, PACK_H, 24, 36);
    const pos = g.attributes.position;
    for (let i = 0; i < pos.count; i++) {
      const x = pos.getX(i) / (PACK_W / 2);
      const y = pos.getY(i) / (PACK_H / 2);
      pos.setZ(i, 0.14 * (1 - x * x) * (1 - Math.pow(Math.abs(y), 6)));
    }
    g.computeVertexNormals();
    return g;
  }, []);
  // The strip is one piece, finely divided so it can bend as it tears; its lower edge is
  // ragged, where it comes apart from the wrapper.
  const stripGeometry = useMemo(() => new THREE.PlaneGeometry(PACK_W, STRIP_H, STRIP_COLUMNS, 3), []);
  const stripBase = useMemo(() => {
    const pos = stripGeometry.attributes.position;
    const base = new Float32Array(pos.array.length);
    base.set(pos.array as Float32Array);
    for (let i = 0; i < pos.count; i++) {
      if (base[i * 3 + 1] < -STRIP_H / 2 + 1e-4) base[i * 3 + 1] += (i % 2 === 0 ? 1 : -1) * 0.025;
    }
    return base;
  }, [stripGeometry]);

  useEffect(() => () => drawnBody?.dispose(), [drawnBody]);
  useEffect(() => {
    if (drawnBody) return;
    let texture: THREE.Texture | null = null;
    let live = true;
    void packTexture(look).then((t) => {
      texture = t;
      if (live) setLoaded(t);
      else t.dispose();
    });
    return () => {
      live = false;
      texture?.dispose();
    };
  }, [look, drawnBody]);
  useEffect(() => () => [strip, bodyGeometry, stripGeometry].forEach((r) => r.dispose()), [strip, bodyGeometry, stripGeometry]);

  // Shake: the pack follows the held pointer on a spring.
  const shake = useRef({ target: new THREE.Vector2(), pos: new THREE.Vector2(), vel: new THREE.Vector2() });
  // Tear: started from one side (`dir`), the tear front runs along the strip. `auto` when a
  // click tore it: the front then runs on its own.
  const tear = useRef({ dir: 0, front: 0, offAt: null as number | null, auto: false, sounded: false });
  const clock = useRef(0);
  const leftAt = useRef<number | null>(null);

  /** A click (no drag) on the pack tears it open by itself, from the side clicked. */
  function autoTear(x: number) {
    const t = tear.current;
    if (t.offAt !== null || t.auto) return;
    if (t.dir === 0) t.dir = x < 0 ? 1 : -1;
    t.auto = true;
  }

  // Asked from outside the scene (a button, a key): torn by itself, from the left.
  useEffect(() => {
    if (tearRequest > 0) autoTear(-1);
  }, [tearRequest]);

  function startShake(e: ThreeEvent<PointerEvent>) {
    e.stopPropagation();
    const start = pointOnPlane(e.nativeEvent.clientX, e.nativeEvent.clientY, camera, dom, 0);
    let moved = 0;
    followDrag(
      (m) => {
        const p = pointOnPlane(m.clientX, m.clientY, camera, dom, 0);
        moved = Math.max(moved, p.distanceTo(start));
        shake.current.target.set(
          THREE.MathUtils.clamp((p.x - start.x) * 0.35, -0.6, 0.6),
          THREE.MathUtils.clamp((p.y - start.y) * 0.35, -0.6, 0.6),
        );
      },
      () => {
        shake.current.target.set(0, 0);
        if (moved < 0.1) autoTear(start.x);
      },
    );
  }

  function startTear(e: ThreeEvent<PointerEvent>) {
    e.stopPropagation();
    const t = tear.current;
    if (t.offAt !== null) return;
    const start = pointOnPlane(e.nativeEvent.clientX, e.nativeEvent.clientY, camera, dom, 0);
    let moved = false;
    followDrag(
      (m) => {
        const p = pointOnPlane(m.clientX, m.clientY, camera, dom, 0);
        const g = group.current;
        const x = g ? g.worldToLocal(p.clone()).x : p.x;
        // The side the pointer moves to decides where the tear starts (the other end).
        if (t.dir === 0 && Math.abs(p.x - start.x) > 0.05) t.dir = p.x > start.x ? 1 : -1;
        if (t.dir === 0) return;
        moved = true;
        const along = t.dir > 0 ? x + PACK_W / 2 : PACK_W / 2 - x;
        t.front = Math.max(t.front, along);
      },
      () => {
        if (!moved) autoTear(start.x);
      },
    );
  }

  useFrame((state, delta) => {
    clock.current += delta;
    const g = group.current;
    if (!g) return;
    // Tilt towards the pointer.
    g.rotation.y = THREE.MathUtils.lerp(g.rotation.y, state.pointer.x * 0.35, 0.08);
    g.rotation.x = THREE.MathUtils.lerp(g.rotation.x, -state.pointer.y * 0.25, 0.08);
    // Shake spring.
    const s = shake.current;
    s.vel.add(s.target.clone().sub(s.pos).multiplyScalar(0.18)).multiplyScalar(0.78);
    s.pos.add(s.vel);
    g.position.set(s.pos.x, s.pos.y, 0);
    g.rotation.z = -s.vel.x * 0.6;

    // The strip: behind the tear front it peels back, curling more the further from the front.
    const t = tear.current;
    if (t.auto) t.front = Math.max(t.front, 0.4) + delta * PACK_W * 1.8;
    // The rip sounds as the strip starts giving way.
    if (t.dir !== 0 && t.front > 0.15 && !t.sounded) {
      t.sounded = true;
      sounds.tear();
    }
    if (t.front >= PACK_W && t.offAt === null) {
      t.offAt = clock.current;
      onTorn();
    }
    const pos = stripGeometry.attributes.position;
    const bottom = -STRIP_H / 2;
    for (let i = 0; i < pos.count; i++) {
      const x = stripBase[i * 3];
      const y = stripBase[i * 3 + 1];
      const along = t.dir >= 0 ? x + PACK_W / 2 : PACK_W / 2 - x;
      const behind = t.dir === 0 ? 0 : Math.max(0, t.front - along);
      const angle = Math.min(2.4, behind * 1.6);
      const r = y - bottom;
      pos.setXYZ(i, x, bottom + r * Math.cos(angle) + behind * 0.12, r * Math.sin(angle) + behind * 0.06);
    }
    pos.needsUpdate = true;
    const sm = stripMesh.current;
    if (sm && t.offAt !== null) {
      // Torn off entirely: the strip falls away in one piece.
      const k = clock.current - t.offAt;
      sm.position.set(t.dir * k * 1.4, PACK_H / 2 + STRIP_H / 2 + k * 1.6 - k * k * 4, 0.05 + k * 1.5);
      sm.rotation.z = t.dir * k * 1.8;
      sm.visible = k < 1;
    }

    // Leaving: once the cards are out of it, the empty wrapper falls away.
    if (leaving) {
      leftAt.current ??= clock.current;
      const k = Math.min(1, Math.max(0, (clock.current - leftAt.current - CARDS_OUT) / PACK_FALLS));
      g.position.y -= k * k * 6;
      g.rotation.z += k * k * 0.5;
      if (bodyMaterial.current) bodyMaterial.current.opacity = 1 - k * k;
    }
  });

  return (
    <group ref={group}>
      <mesh geometry={bodyGeometry} onPointerDown={startShake}>
        <meshStandardMaterial
          // A new material once the print is ready: three.js compiles a material's shader
          // once, so a map added to a material already drawn without one never shows.
          key={body ? "printed" : "plain"}
          ref={bodyMaterial}
          map={body}
          color={body ? "#ffffff" : look.finish === "collector" ? "#14110d" : look.finish === "prism" ? "#e6e0d8" : `hsl(${look.hue}, 65%, 40%)`}
          // A little of the print glows by itself: without an environment to reflect, a
          // metallic wrapper turns grey.
          emissiveMap={body}
          emissive={body ? "#ffffff" : "#000000"}
          emissiveIntensity={0.35}
          metalness={0.15}
          roughness={0.35}
          transparent
        />
      </mesh>
      <mesh
        ref={stripMesh}
        geometry={stripGeometry}
        position={[0, PACK_H / 2 + STRIP_H / 2, 0.01]}
        onPointerDown={startTear}
      >
        <meshStandardMaterial map={strip} metalness={0.4} roughness={0.3} side={THREE.DoubleSide} />
      </mesh>
    </group>
  );
}

// ---- the cards -----------------------------------------------------------------------------

/** The top card's motion: held, sliding back to the pile, or leaving it. */
type Slide =
  | { kind: "idle" }
  | { kind: "held" }
  | { kind: "animate"; from: THREE.Vector2; to: THREE.Vector2; start: number; duration: number; done: () => void };

const GLOW = { mythic: new THREE.Color("#e2682c"), rare: new THREE.Color("#e0c15a") };

// Sparkles around a foil card: twinkling star points scattered along its rim.
const SPARKLES = 56;

function createSparkles() {
  const positions = new Float32Array(SPARKLES * 3);
  const phases = new Float32Array(SPARKLES);
  const w = CARD_W / 2;
  const h = CARD_H / 2;
  for (let i = 0; i < SPARKLES; i++) {
    // A point on the rim (by length), pushed a little outwards.
    let d = Math.random() * 2 * (CARD_W + CARD_H);
    const out = 0.04 + Math.random() * 0.28;
    let x: number;
    let y: number;
    if (d < CARD_W) [x, y] = [d - w, -h - out];
    else if ((d -= CARD_W) < CARD_H) [x, y] = [w + out, d - h];
    else if ((d -= CARD_H) < CARD_W) [x, y] = [w - d, h + out];
    else [x, y] = [-w - out, h - (d - CARD_W)];
    positions.set([x, y, 0.06], i * 3);
    phases[i] = Math.random();
  }
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
  geometry.setAttribute("aPhase", new THREE.BufferAttribute(phases, 1));
  const material = new THREE.ShaderMaterial({
    uniforms: { uTime: { value: 0 }, uOpacity: { value: 0 }, uSize: { value: 0.5 } },
    vertexShader: /* glsl */ `
      attribute float aPhase;
      uniform float uTime;
      uniform float uSize;
      varying float vAlpha;
      varying float vPhase;
      void main() {
        vPhase = aPhase;
        vAlpha = pow(0.5 + 0.5 * sin(uTime * (2.0 + aPhase * 2.0) + aPhase * 6.2832), 4.0);
        vec4 mv = modelViewMatrix * vec4(position, 1.0);
        gl_PointSize = uSize * (0.4 + vAlpha) * 300.0 / -mv.z;
        gl_Position = projectionMatrix * mv;
      }`,
    fragmentShader: /* glsl */ `
      uniform float uOpacity;
      uniform float uTime;
      varying float vAlpha;
      varying float vPhase;
      vec3 hue(float h) {
        return clamp(abs(mod(h * 6.0 + vec3(0.0, 4.0, 2.0), 6.0) - 3.0) - 1.0, 0.0, 1.0);
      }
      void main() {
        vec2 p = gl_PointCoord - 0.5;
        float core = max(0.0, 1.0 - length(p) * 4.0);
        float rays = max(0.0, 1.0 - abs(p.x) * 14.0) * max(0.0, 1.0 - abs(p.y) * 2.0)
                   + max(0.0, 1.0 - abs(p.y) * 14.0) * max(0.0, 1.0 - abs(p.x) * 2.0);
        float a = (core * core + rays * 0.7) * vAlpha * uOpacity;
        vec3 color = mix(vec3(1.0), hue(fract(vPhase + uTime * 0.1)), 0.45);
        gl_FragColor = vec4(color * a, a);
      }`,
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
  });
  return { geometry, material };
}

function Cards({
  cards,
  top,
  onNext,
  onFinished,
  onShown,
  slideRequest,
}: {
  cards: OpenedCard[];
  top: number;
  /** The top card has left the pile. */
  onNext: () => void;
  /** The last card has left too. */
  onFinished: () => void;
  /** The card of this index is seen on top of the pile; null once it starts leaving. */
  onShown?: (index: number | null) => void;
  /** Bumped to send the top card off without a drag (Space). */
  slideRequest: number;
}) {
  const camera = useThree((s) => s.camera);
  const dom = useThree((s) => s.gl.domElement);
  const back = useMemo(() => cardBackTexture(), []);
  const glow = useMemo(() => glowTexture(), []);
  const geometry = useMemo(() => createCardGeometry(), []);
  const materials = useMemo(() => cards.map((c) => createCardMaterials(back, c.foil, c.foilKind)), [cards, back]);
  const sparkles = useMemo(() => createSparkles(), []);
  const stack = useRef<THREE.Group>(null);
  const meshes = useRef<(THREE.Mesh | null)[]>([]);
  const glowMesh = useRef<THREE.Mesh>(null);
  const foilGlowMesh = useRef<THREE.Mesh>(null);
  const sparklePoints = useRef<THREE.Points>(null);
  const clock = useRef(0);
  const appearedAt = useRef<number | null>(null);
  // The first card counts as on top once the pile has settled.
  const topSince = useRef(SETTLED);
  const finishing = useRef(false);
  const firstShown = useRef(false);
  const turnSounded = useRef(false);
  // The card on top, moved on as soon as one leaves: waiting for React to pass the new `top`
  // would show the card that left back on the pile for a frame or two.
  const current = useRef(top);
  // The top card is on its way out (its glow and sparkles fade).
  const leaving = useRef(false);
  if (top > current.current) current.current = top;
  /** The pile is out of the wrapper and in place: its cards can be slid. */
  const settled = () => appearedAt.current != null && clock.current - appearedAt.current >= SETTLED;

  // Card faces load in the background; the back shows until then.
  useEffect(() => {
    let live = true;
    const loaded: THREE.Texture[] = [];
    cards.forEach((card, i) => {
      void loadCardTexture(largeImage(card.imageUrl))
        .then((t) => {
          if (!live) return t.dispose();
          loaded.push(t);
          materials[i].uniforms.uFront.value = t;
          materials[i].uniforms.uHasFront.value = 1;
        })
        .catch(() => {});
    });
    return () => {
      live = false;
      loaded.forEach((t) => t.dispose());
      materials.forEach((m) => m.dispose());
    };
  }, [cards, materials]);
  useEffect(
    () => () => [back, glow, geometry, sparkles.geometry, sparkles.material].forEach((r) => r.dispose()),
    [back, glow, geometry, sparkles],
  );

  const slide = useRef<Slide>({ kind: "idle" });
  const offset = useRef(new THREE.Vector2());
  const stackZ = (k: number) => 0.03 * (cards.length - k);

  function animateTo(to: THREE.Vector2, duration: number, done: () => void) {
    slide.current = { kind: "animate", from: offset.current.clone(), to, start: clock.current, duration, done };
  }

  /** Sends the top card off the pile in `direction`. */
  function sendOff(direction: THREE.Vector2, duration: number) {
    const dir = direction.lengthSq() > 1e-6 ? direction.clone().normalize() : new THREE.Vector2(1, 0.35).normalize();
    leaving.current = true;
    onShown?.(null);
    sounds.slide();
    animateTo(offset.current.clone().add(dir.multiplyScalar(8)), duration, () => {
      slide.current = { kind: "idle" };
      offset.current.set(0, 0);
      leaving.current = false;
      if (current.current >= cards.length - 1) {
        // Hidden until the scene goes away.
        current.current = cards.length;
        onFinished();
      } else {
        current.current += 1;
        topSince.current = clock.current;
        revealSound(cards[current.current]);
        onShown?.(current.current);
        onNext();
      }
    });
  }

  // Space: the top card slides off on its own.
  useEffect(() => {
    if (slideRequest === 0 || slide.current.kind !== "idle" || !settled()) return;
    sendOff(new THREE.Vector2(1, 0.35), 0.45);
  }, [slideRequest]);

  function grab(e: ThreeEvent<PointerEvent>, k: number) {
    if (k !== current.current || !settled()) return;
    e.stopPropagation();
    if (slide.current.kind !== "idle") return;
    // The pile is zoomed: pointer moves (in the world) become moves in the pile's own units.
    const zoom = stack.current?.scale.x ?? 1;
    const z = stackZ(k) * zoom;
    const start = pointOnPlane(e.nativeEvent.clientX, e.nativeEvent.clientY, camera, dom, z);
    const startedAt = clock.current;
    let last = start.clone();
    let lastAt = startedAt;
    const velocity = new THREE.Vector2();
    slide.current = { kind: "held" };
    followDrag(
      (m) => {
        const p = pointOnPlane(m.clientX, m.clientY, camera, dom, z);
        offset.current.set((p.x - start.x) / zoom, (p.y - start.y) / zoom);
        const dt = Math.max(1e-3, clock.current - lastAt);
        velocity.set((p.x - last.x) / dt, (p.y - last.y) / dt);
        last = p;
        lastAt = clock.current;
      },
      () => {
        const moved = offset.current.length();
        if (moved < 0.08) sendOff(new THREE.Vector2(1, 0.35), 0.45); // a click
        else if (moved > SLIDE_AWAY || velocity.length() > 6) sendOff(offset.current, 0.35);
        else animateTo(new THREE.Vector2(), 0.25, () => (slide.current = { kind: "idle" }));
      },
    );
  }

  useFrame((state, delta) => {
    clock.current += delta;
    appearedAt.current ??= clock.current;
    const since = clock.current - appearedAt.current;
    // The first card shows once the pile has settled.
    if (!firstShown.current && since >= SETTLED) {
      firstShown.current = true;
      revealSound(cards[0]);
      onShown?.(0);
    }

    // The whole pile tilts together with the pointer: no card reveals another by itself. While
    // in the wrapper, it tilts as much as the wrapper does, to stay inside it.
    const s = stack.current;
    if (s) {
      const inside = since < CARDS_OUT;
      s.rotation.y = THREE.MathUtils.lerp(s.rotation.y, state.pointer.x * (inside ? 0.35 : 0.25), 0.08);
      s.rotation.x = THREE.MathUtils.lerp(s.rotation.x, -state.pointer.y * (inside ? 0.25 : 0.18), 0.08);
      // Brought closer as the empty wrapper falls away.
      s.scale.setScalar(1 + (CARD_ZOOM - 1) * ease((since - CARDS_OUT) / PACK_FALLS));
    }

    const mode = slide.current;
    if (mode.kind === "animate") {
      const t = ease((clock.current - mode.start) / mode.duration);
      offset.current.copy(mode.from).lerp(mode.to, t);
      if (t >= 1) mode.done();
    }

    const top = current.current;
    const isLast = top === cards.length - 1;

    // Back up until the pile is in place, then turned over as one block (about its middle).
    const turnStart = CARDS_OUT + PACK_FALLS;
    if (!turnSounded.current && since >= turnStart) {
      turnSounded.current = true;
      sounds.flip();
    }
    const turn = Math.PI * (1 - ease((since - turnStart) / PILE_TURNS));

    // The last card stays a moment, then leaves on its own.
    if (isLast && !finishing.current && slide.current.kind === "idle" && clock.current - topSince.current > LAST_CARD_STAYS) {
      finishing.current = true;
      sendOff(new THREE.Vector2(0, 1), 0.6);
    }

    materials.forEach(({ uniforms }) => {
      uniforms.uTime.value = clock.current;
      uniforms.uLight.value.set(state.pointer.x, state.pointer.y);
    });

    // Out of the pack: the pile, behind the wrapper's front, slides up through the torn top;
    // then, as the empty wrapper falls away, it comes forward and settles.
    const settle = ease((since - CARDS_OUT) / PACK_FALLS);
    const depth = (k: number) => THREE.MathUtils.lerp(-0.2 - 0.01 * k, stackZ(k), settle);
    const middle = (depth(0) + depth(cards.length - 1)) / 2;
    meshes.current.forEach((mesh, k) => {
      if (!mesh) return;
      mesh.visible = k >= top;
      const out = ease((since - (cards.length - k) * 0.012) / CARDS_OUT);
      // Turned over about the pile's middle: back up, its order reversed, then face up.
      const dz = depth(k) - middle;
      const base = new THREE.Vector3(
        dz * Math.sin(turn),
        THREE.MathUtils.lerp(-0.4 + out * 1.4, 0, settle),
        middle + dz * Math.cos(turn),
      );
      mesh.rotation.y = turn;
      if (k === top) {
        mesh.position.set(base.x + offset.current.x, base.y + offset.current.y, base.z + Math.min(0.3, offset.current.length() * 0.1));
        mesh.rotation.z = -offset.current.x * 0.08;
      } else {
        mesh.position.copy(base);
        mesh.rotation.z = 0;
      }
    });

    // Effects of the card on top only: the ones below are not seen yet.
    const card = cards[top];
    const topMesh = meshes.current[top];
    const shown = Math.min(1, (clock.current - topSince.current) * 1.5);
    const on = card != null && !leaving.current;

    // A big pull lights up behind the pile when it comes on top.
    const g = glowMesh.current;
    if (g && card) {
      const color = card.rarity === "mythic" ? GLOW.mythic : isBigCard(card) ? GLOW.rare : null;
      const material = g.material as THREE.MeshBasicMaterial;
      if (color) material.color.copy(color);
      const target = color && on ? 0.55 + 0.15 * Math.sin(clock.current * 3) : 0;
      material.opacity = THREE.MathUtils.lerp(material.opacity, Math.min(target, shown), 0.15);
      const pulse = 1 + 0.04 * Math.sin(clock.current * 3);
      g.scale.set(pulse, pulse, 1);
    }

    // A foil shines: a halo shifting through the rainbow and sparkles along its rim, following
    // the card as it is slid.
    const foilOn = on && card.foil ? shown : 0;
    const fg = foilGlowMesh.current;
    if (fg) {
      const material = fg.material as THREE.MeshBasicMaterial;
      material.color.setHSL((clock.current * 0.12) % 1, 0.85, 0.6);
      material.opacity = THREE.MathUtils.lerp(material.opacity, foilOn * (0.5 + 0.12 * Math.sin(clock.current * 2.2)), 0.12);
    }
    const sp = sparklePoints.current;
    if (sp) {
      sparkles.material.uniforms.uTime.value = clock.current;
      const opacity = sparkles.material.uniforms.uOpacity;
      opacity.value = THREE.MathUtils.lerp(opacity.value, foilOn, 0.15);
      if (topMesh) {
        sp.position.copy(topMesh.position);
        sp.rotation.z = topMesh.rotation.z;
        fg?.position.set(topMesh.position.x, topMesh.position.y, -0.08);
      }
    }
  });

  return (
    <group ref={stack} position={[0, 0.1, 0]}>
      <mesh ref={glowMesh} position={[0, 0, -0.1]}>
        <planeGeometry args={[CARD_W * 2.4, CARD_H * 2.1]} />
        <meshBasicMaterial map={glow} transparent opacity={0} blending={THREE.AdditiveBlending} depthWrite={false} />
      </mesh>
      <mesh ref={foilGlowMesh} position={[0, 0, -0.08]}>
        <planeGeometry args={[CARD_W * 1.9, CARD_H * 1.65]} />
        <meshBasicMaterial map={glow} transparent opacity={0} blending={THREE.AdditiveBlending} depthWrite={false} />
      </mesh>
      <points ref={sparklePoints} geometry={sparkles.geometry} material={sparkles.material} renderOrder={2} />
      {cards.map((card, k) => (
        <mesh
          key={`${card.uuid}-${k}`}
          ref={(m) => {
            meshes.current[k] = m;
          }}
          geometry={geometry}
          material={[materials[k].faces, materials[k].edge]}
          // Cards that left still sit under the pointer (hidden): they let the event through.
          onPointerDown={(e) => grab(e, k)}
        />
      ))}
    </group>
  );
}

// ---- the scene -----------------------------------------------------------------------------

interface Props {
  look: PackLook;
  cards: OpenedCard[] | null;
  /** Index of the card on top of the pile (those before it have left). */
  top: number;
  onTorn: () => void;
  onNext: () => void;
  onFinished: () => void;
  /** The card of this index is seen on top of the pile; null once it starts leaving. */
  onShown?: (index: number | null) => void;
  slideRequest: number;
  /** Bumped to tear the pack open without touching it. */
  tearRequest: number;
}

export function PackScene({ look, cards, top, onTorn, onNext, onFinished, onShown, slideRequest, tearRequest }: Props) {
  // Once the cards are drawn they slide out of the wrapper, which then falls away.
  const [packGone, setPackGone] = useState(false);
  useEffect(() => {
    if (!cards) return setPackGone(false);
    const timer = setTimeout(() => setPackGone(true), SETTLED * 1000 + 100);
    return () => clearTimeout(timer);
  }, [cards]);

  return (
    // A wide enough view for the zoomed cards to keep a margin above and below (with the
    // frame's height, they show as large as before, the pack a little smaller).
    <Canvas className="pack-scene" flat dpr={[1, 2]} camera={{ position: [0, 0, 9.5], fov: 40 }}>
      <ambientLight intensity={0.9} />
      <directionalLight position={[3, 4, 6]} intensity={1.6} />
      <PointerLight />
      {!packGone && <Pack look={look} onTorn={onTorn} leaving={cards != null} tearRequest={tearRequest} />}
      {cards && (
        <Cards cards={cards} top={top} onNext={onNext} onFinished={onFinished} onShown={onShown} slideRequest={slideRequest} />
      )}
    </Canvas>
  );
}

/** A light that follows the pointer, for glints on the wrapper. */
function PointerLight() {
  const light = useRef<THREE.PointLight>(null);
  useFrame((state) => light.current?.position.set(state.pointer.x * 4, state.pointer.y * 3, 3));
  return <pointLight ref={light} intensity={18} distance={12} color="#fff6e0" />;
}
