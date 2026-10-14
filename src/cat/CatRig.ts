// The cat's meshes (spec §5), ported 1:1 from docs/prototype/cat-prototype.html.
// No state logic here: applyPose() maps a CatPose onto transforms each frame, allocation-free.
import * as THREE from 'three';
import { KNOCKOUT_IMPACT_S, type CatPose } from './CatController';
import { createOutlineMaterial, createToonGradient } from './materials';
import { AMBER, BLUSH, CREAM, FUR, FUR2, INK, MOUTH, PINK, PUPIL } from './palette';
import { TailTube } from './tail';

type V3 = readonly [number, number, number];
interface MeshOpts {
  p?: V3;
  s?: V3;
  r?: V3;
  /** Outline hull scale; 0 = no outline. Default 1.06. */
  ol?: number;
}

/** Tail control points: A = awake pose, Z = curled around the paws while asleep. */
const TAIL_A: readonly V3[] = [[0.55, 0.45, -0.75], [1.25, 0.55, -0.7], [1.62, 1.15, -0.55], [1.45, 1.8, -0.45]];
const TAIL_Z: readonly V3[] = [[0.55, 0.35, -0.7], [1.15, 0.27, -0.1], [0.9, 0.22, 0.7], [0.25, 0.2, 1.05]];

const ROOT_X = 1.3;

/**
 * Generous world-space boxes around the cat in each pose, for the overlay's click hitbox (spec §9).
 * Static on purpose: the hitbox is sent to Rust only when the layout changes, not every frame.
 */
export const HIT_BOUNDS = {
  awake: new THREE.Box3(new THREE.Vector3(ROOT_X - 1.45, 0, -1), new THREE.Vector3(ROOT_X + 2.35, 3.55, 1.3)),
  asleep: new THREE.Box3(new THREE.Vector3(ROOT_X - 1.45, 0, -1), new THREE.Vector3(ROOT_X + 1.35, 2.95, 1.4)),
} as const;

const HEAD_Y = 2.05;
const HEAD_Z = 0.1;

const lerp = (a: number, b: number, t: number): number => a + (b - a) * t;

/**
 * Super Saiyan aura while the cat is awake: sharp energy spikes shooting up from an egg-shaped outline around
 * the cat (longest above the head, shorter down the shoulders), in two walls (a tall pale outer one and a
 * bright inner one), with rising streaks, a soft halo and white lightning crackling around the body. Spike
 * heights jump at ~14 steps a second, like hand-drawn anime frames. One shader quad behind the cat (one extra
 * draw call), hidden entirely while asleep; it writes premultiplied colour for the transparent window.
 * uMotion = 0 (reduceMotion) freezes it and turns the lightning off.
 * The quad spans world y -0.45..6.25, which uses the stage's headroom above the cat.
 */
const AURA_CENTER: V3 = [ROOT_X, 2.9, -1.35];
const AURA_SIZE: readonly [number, number] = [4.6, 6.7];

const AURA_VERTEX = /* glsl */ `
varying vec2 vUv;
void main() {
  vUv = uv;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

const AURA_FRAGMENT = /* glsl */ `
uniform float uTime;
uniform float uIntensity;
uniform float uMotion;
varying vec2 vUv;

float hash1(float n) { return fract(sin(n * 127.1) * 43758.5453); }

// 0..~1: random height per spike plus a pointed tip.
float spike(float x, float count, float seed, float tick) {
  float s = x * count;
  float i = floor(s);
  float u = fract(s);
  float h = hash1(i * 1.37 + seed + tick * 7.13);
  float tip = 1.0 - abs(u - 0.5) * 2.0;
  return h * 0.45 + tip * (0.3 + 0.25 * h);
}

// One wall of spikes rising from an ellipse (centre cy, radii rx, ry). Returns (fill, halo).
vec2 wall(vec2 p, float rx, float ry, float cy, float count, float seed, float tick, float reach, float grow) {
  float xr = p.x / rx;
  if (abs(xr) >= 1.0) return vec2(0.0);
  float arc = sqrt(1.0 - xr * xr);
  float base = cy + ry * arc;
  float floorY = cy - ry * arc;
  float len = (0.04 + reach * pow(arc, 1.6)) * (0.3 + spike(p.x, count, seed, tick)) * grow;
  float top = base + len;
  float lower = smoothstep(floorY, floorY + 0.12, p.y);
  float fill = smoothstep(0.0, 0.01, top - p.y) * lower;
  float halo = smoothstep(0.07, 0.0, p.y - top) * (1.0 - fill) * lower;
  return vec2(fill, halo);
}

float segDist(vec2 p, vec2 a, vec2 b) {
  vec2 pa = p - a;
  vec2 ba = b - a;
  float h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
  return length(pa - ba * h);
}

void main() {
  vec2 p = vUv * 2.0 - 1.0;                          // -1..1, y up
  float tick = floor(uTime * 14.0) * uMotion;
  float grow = 0.82 + 0.28 * min(uIntensity, 1.5);   // flares make the spikes longer

  vec2 outer = wall(p, 0.9, 0.62, -0.32, 17.0, 3.0, tick, 0.62, grow);
  vec2 inner = wall(p, 0.7, 0.56, -0.34, 26.0, 11.0, tick + 0.5, 0.42, grow);

  // Vertical streaks rushing upwards.
  float column = floor((p.x * 0.5 + 0.5) * 36.0);
  float stripe = 0.72 + 0.28 * sin(p.x * 50.0 + hash1(column) * 6.2832);
  float rise = 0.8 + 0.2 * sin(p.y * 18.0 - uTime * 16.0 * uMotion + hash1(column + 9.0) * 6.2832);

  vec3 pale = vec3(1.0, 0.95, 0.42);
  vec3 gold = vec3(1.0, 0.84, 0.16);
  vec3 white = vec3(1.0, 0.99, 0.86);
  vec3 col = mix(pale, gold, inner.x);
  float a = outer.x * 0.42 * stripe * rise + inner.x * 0.62 * rise + (outer.y + inner.y) * 0.18;
  // White-hot core along the inner wall's spikes.
  col = mix(col, white, inner.x * 0.35 * stripe);

  // Lightning: up to two short zigzag arcs around the body, re-rolled 9 times a second.
  float bolt = 0.0;
  for (int k = 0; k < 2; k++) {
    float seed = floor(uTime * 9.0) + float(k) * 17.0;
    if (uMotion < 0.5 || hash1(seed) < 0.35) continue;
    float ang = hash1(seed + 1.0) * 6.2832;
    vec2 start = vec2(0.0, -0.34) + vec2(cos(ang) * 0.62, sin(ang) * 0.5);
    vec2 dir = normalize(vec2(cos(ang + 1.2), sin(ang + 1.2)));
    vec2 nrm = vec2(-dir.y, dir.x);
    vec2 prev = start;
    for (int j = 1; j <= 5; j++) {
      vec2 next = start + dir * (0.055 * float(j)) + nrm * (hash1(seed + float(j) * 3.1) - 0.5) * 0.11;
      float d = segDist(p, prev, next);
      bolt = max(bolt, smoothstep(0.01, 0.0, d) + 0.45 * smoothstep(0.04, 0.0, d));
      prev = next;
    }
  }
  bolt = min(bolt, 1.0);
  col = mix(col, white, bolt);
  a = max(a, bolt * 0.95);

  a = clamp(a * min(uIntensity, 1.3), 0.0, 1.0);
  gl_FragColor = vec4(col * a, a);
}
`;

/**
 * Floating Zzz, drawn as sprites so they animate with the (capped) render loop. As a CSS animation they ran
 * at the display's refresh rate and cost more CPU than the sleeping cat itself.
 * Units: world; ~0.022 world units per CSS px at the overlay's framing.
 */
const ZZZ_ANCHOR: V3 = [2.1, 2.6, 0.3];
const ZZZ_PERIOD = 3.2;
const ZZZ_DRIFT: readonly [number, number] = [0.57, 1.1];
const ZZZ: readonly { glyph: string; size: number; dx: number; dy: number; delay: number }[] = [
  { glyph: 'z', size: 0.37, dx: 0, dy: 0, delay: 0 },
  { glyph: 'Z', size: 0.46, dx: 0.09, dy: 0.22, delay: 1.05 },
  { glyph: 'z', size: 0.29, dx: -0.04, dy: 0.09, delay: 2.1 },
];

/**
 * Anime angry mark (the "vein" symbol) on the head's top-right corner, between the right ear and the brow,
 * shown when the sleeping cat is poked. A child of the head, so it follows the head. Head-local units.
 */
const ANGER_POS: V3 = [0.86, 0.55, 1.0];
const ANGER_SIZE = 0.46;

/**
 * Knockout (you answered, so something hits the cat): the four objects, drawn from primitives in the cat's toon
 * style. World units; the cat's head is about 2.6 wide.
 */
const WOOD = 0x9c6b3e;
const WOOD_DARK = 0x6e4524;
const SLIPPER_SOLE = 0xf1eee6;
const SLIPPER_CELL = 0xd8d2c4;
const SLIPPER_STRAP = 0x3d8bfd;
const DURIAN = 0xa9c24b;
const DURIAN_SPIKE = 0x7e9937;
const PAN = 0x3b3f4c;
const PAN_INSIDE = 0x596071;
const PAN_HANDLE = 0x23252e;
/** Where things land on the head, in head-local units (the top, a little towards the viewer). */
const HIT_LOCAL: V3 = [0.05, 0.95, 0.25];
/** How far each object's centre sits above the hit point when it lands, so it rests on the head, not in it
 * (stick, slipper, durian, pan). */
const REST_LIFT: readonly number[] = [0.12, 0.1, 0.55, 0.12];
/** Thrown objects come from the left (where the bubble was), the durian drops from above. */
const THROW_FROM: V3 = [-3.8, 2.2, 1.2];
const DROP_FROM: V3 = [0.15, 4.6, 0];
const THROW_ARC = 1.0;
/** After the hit: bounce velocity (world units/s) and gravity. */
const BOUNCE: V3 = [1.7, 3.2, 0.7];
const BOUNCE_DURIAN: V3 = [1.1, 3.4, 0.6];
const BOUNCE_GRAVITY = 13;
const BURST_S = 0.32;
/** Stars circling the knocked-out head (head-local). */
const STARS = 3;
const STAR_ORBIT: V3 = [0.95, 1.12, 0.55];
const STAR_SIZE = 0.3;
const BUMP_POS: V3 = [0.18, 0.9, 0.38];
const BUMP_SIZE = 0.32;

interface Eye {
  s: number;
  iris: THREE.Group;
  pupil: THREE.Group;
  sleepArc: THREE.Mesh;
  happyArc: THREE.Mesh;
  brow: THREE.Mesh;
  /** Knocked out: ✕ eyes. */
  cross: THREE.Group;
}

interface Ear {
  s: number;
  pivot: THREE.Group;
}

export class CatRig {
  /** Add this to the scene: shadow and cat. */
  readonly object = new THREE.Group();
  readonly headG = new THREE.Group();

  private readonly gradient = createToonGradient();
  private readonly outlineMat = createOutlineMaterial();
  private readonly inkMat = new THREE.MeshBasicMaterial({ color: INK });
  private readonly toonMats = new Map<number, THREE.MeshToonMaterial>();
  private readonly materials: THREE.Material[] = [this.outlineMat, this.inkMat];
  private readonly geometries: THREE.BufferGeometry[] = [];
  private readonly sphere: THREE.SphereGeometry;

  private readonly root = new THREE.Group();
  private readonly bodyG = new THREE.Group();
  private readonly shadow: THREE.Mesh;
  private readonly body: THREE.Mesh;
  private readonly pawR: THREE.Mesh;
  private readonly mouthW = new THREE.Group();
  private readonly mouthO: THREE.Mesh;
  private readonly tail: TailTube;
  private readonly tip: THREE.Mesh;
  private readonly zzz: THREE.Sprite[] = [];
  private anger: THREE.Sprite | undefined;
  private readonly aura: THREE.Mesh;
  private readonly auraUniforms = { uTime: { value: 0 }, uIntensity: { value: 0 }, uMotion: { value: 1 } };
  private readonly textures: THREE.Texture[] = [];
  private readonly eyes: Eye[] = [];
  private readonly ears: Ear[] = [];
  private readonly headWorld = new THREE.Vector3();
  /** One group per KNOCKOUT_OBJECTS entry, in world space; at most one is visible. */
  private readonly projectiles: THREE.Group[] = [];
  private readonly bonkTarget = new THREE.Vector3();
  private readonly bonkHit = new THREE.Vector3();
  private bonkHitSet = false;
  private burst: THREE.Sprite | undefined;
  private readonly stars: THREE.Sprite[] = [];
  private readonly bump: THREE.Mesh;

  constructor() {
    this.sphere = this.geo(new THREE.SphereGeometry(1, 40, 28));
    const SPH = this.sphere;

    this.root.position.x = ROOT_X;
    this.root.rotation.y = -0.22;
    this.object.add(this.root);

    const shadowMat = this.mat(new THREE.MeshBasicMaterial({ color: 0x000000, transparent: true, opacity: 0.13 }));
    this.shadow = new THREE.Mesh(this.geo(new THREE.CircleGeometry(1.4, 40)), shadowMat);
    this.shadow.rotation.x = -Math.PI / 2;
    this.shadow.position.set(ROOT_X, 0.01, 0);
    this.object.add(this.shadow);

    // Body
    this.root.add(this.bodyG);
    this.body = this.mk(SPH, FUR, this.bodyG, { p: [0, 0.95, 0], s: [1.15, 0.95, 1] });
    this.mk(SPH, CREAM, this.bodyG, { p: [0, 0.85, 0.6], s: [0.68, 0.6, 0.5], ol: 0 });
    this.mk(SPH, FUR, this.bodyG, { p: [-0.78, 0.42, 0.25], s: [0.45, 0.4, 0.55] });
    this.mk(SPH, FUR, this.bodyG, { p: [0.78, 0.42, 0.25], s: [0.45, 0.4, 0.55] });
    this.mk(SPH, CREAM, this.root, { p: [-0.42, 0.17, 0.9], s: [0.3, 0.2, 0.34] });
    this.pawR = this.mk(SPH, CREAM, this.root, { p: [0.42, 0.17, 0.9], s: [0.3, 0.2, 0.34] });

    // Head
    const headG = this.headG;
    headG.position.set(0, HEAD_Y, HEAD_Z);
    this.root.add(headG);
    this.mk(SPH, FUR, headG, { s: [1.3, 1, 1.05] });
    for (const [x, y, z, rz] of [[0, 0.78, 0.64, 0], [-0.26, 0.72, 0.68, 0.25], [0.26, 0.72, 0.68, -0.25]]) {
      this.mk(SPH, FUR2, headG, { p: [x, y, z], s: [0.06, 0.17, 0.05], r: [0, 0, rz], ol: 0 });
    }
    for (const s of [-1, 1]) {
      this.mk(SPH, CREAM, headG, { p: [s * 0.22, -0.32, 0.9], s: [0.3, 0.22, 0.22], ol: 1.08 });
      this.mk(SPH, BLUSH, headG, { p: [s * 0.78, -0.18, 0.8], s: [0.17, 0.08, 0.05], ol: 0 });
    }
    this.mk(SPH, PINK, headG, { p: [0, -0.17, 1.05], s: [0.1, 0.07, 0.06], ol: 1.18 });

    // Mouth: "ω" when closed, an open oval when yelling
    const arcSmall = this.geo(new THREE.TorusGeometry(0.07, 0.02, 6, 12, Math.PI));
    headG.add(this.mouthW);
    for (const s of [-1, 1]) {
      const half = new THREE.Mesh(arcSmall, this.inkMat);
      half.position.set(s * 0.07, -0.33, 1.1);
      half.rotation.z = Math.PI;
      this.mouthW.add(half);
    }
    this.mouthO = this.mk(SPH, MOUTH, headG, { p: [0, -0.42, 1.06], s: [0.13, 0.12, 0.06], ol: 0 });

    // Whiskers
    const whisker = this.geo(new THREE.CylinderGeometry(0.012, 0.012, 0.6));
    for (const s of [-1, 1]) {
      for (const [y, tilt] of [[-0.26, 0.08], [-0.38, -0.06]]) {
        const m = new THREE.Mesh(whisker, this.inkMat);
        m.position.set(s * 0.66, y, 0.86);
        m.rotation.set(0, s * -0.5, Math.PI / 2 + s * tilt);
        headG.add(m);
      }
    }

    // Eyes and ears
    const arc = this.geo(new THREE.TorusGeometry(0.19, 0.035, 8, 20, Math.PI));
    const capsule = this.geo(new THREE.CapsuleGeometry(0.035, 0.2, 4, 8));
    const pupilMat = this.mat(new THREE.MeshBasicMaterial({ color: PUPIL }));
    const whiteMat = this.mat(new THREE.MeshBasicMaterial({ color: 0xffffff }));
    const earCone = this.geo(new THREE.ConeGeometry(0.36, 0.72, 32));
    const earInner = this.geo(new THREE.ConeGeometry(0.2, 0.42, 32));
    for (const s of [-1, 1]) {
      const eg = new THREE.Group();
      eg.position.set(s * 0.45, 0.08, 0.9);
      eg.rotation.y = s * 0.32;
      headG.add(eg);

      const iris = new THREE.Group();
      eg.add(iris);
      this.mk(SPH, AMBER, iris, { s: [0.24, 0.27, 0.12], ol: 1.12 });
      const pupil = new THREE.Group();
      iris.add(pupil);
      this.mk(SPH, pupilMat, pupil, { p: [0, 0, 0.06], s: [0.17, 0.21, 0.08] });
      this.mk(SPH, whiteMat, pupil, { p: [-0.07, 0.09, 0.13], s: [0.06, 0.06, 0.03] });
      this.mk(SPH, whiteMat, pupil, { p: [0.06, -0.08, 0.13], s: [0.03, 0.03, 0.02] });

      const sleepArc = new THREE.Mesh(arc, this.inkMat); // ∪
      sleepArc.rotation.z = Math.PI;
      sleepArc.position.z = 0.1;
      eg.add(sleepArc);
      const happyArc = new THREE.Mesh(arc, this.inkMat); // ∩
      happyArc.position.set(0, -0.08, 0.1);
      eg.add(happyArc);
      const brow = new THREE.Mesh(capsule, this.inkMat);
      brow.position.z = 0.06;
      eg.add(brow);
      const cross = new THREE.Group();
      cross.position.z = 0.1;
      cross.visible = false;
      for (const r of [Math.PI / 4, -Math.PI / 4]) {
        const bar = new THREE.Mesh(capsule, this.inkMat);
        bar.rotation.z = r;
        bar.scale.set(1.3, 1.25, 1);
        cross.add(bar);
      }
      eg.add(cross);
      this.eyes.push({ s, iris, pupil, sleepArc, happyArc, brow, cross });

      const pivot = new THREE.Group();
      pivot.position.set(s * 0.72, 0.68, 0);
      headG.add(pivot);
      this.mk(earCone, FUR, pivot, { p: [0, 0.3, 0], s: [1, 1, 0.55] });
      this.mk(earInner, PINK, pivot, { p: [0, 0.22, 0.13], s: [1, 1, 0.3], ol: 0 });
      this.ears.push({ s, pivot });
    }

    // Tail
    this.tail = new TailTube(this.toon(FUR), this.outlineMat);
    this.root.add(this.tail.mesh, this.tail.outline);
    this.tip = this.mk(SPH, FUR2, this.root, { s: [0.165, 0.165, 0.165], ol: 1.25 });

    this.buildZzz();
    this.buildAnger();

    // Knockout: the bump (child of the head), the objects, the impact burst and the stars.
    this.bump = this.mk(SPH, FUR, headG, { p: BUMP_POS, s: [0, 0, 0], ol: 1.1 });
    this.mk(SPH, 0xffffff, this.bump, { p: [-0.3, 0.35, 0.55], s: [0.25, 0.2, 0.15], ol: 0 });
    this.bump.visible = false;
    this.buildProjectiles();
    this.buildKnockoutSprites();

    // Aura, behind the cat; drawn first among the transparent objects so the shadow disc lands on top.
    const auraMat = this.mat(
      new THREE.ShaderMaterial({
        uniforms: this.auraUniforms,
        vertexShader: AURA_VERTEX,
        fragmentShader: AURA_FRAGMENT,
        transparent: true,
        depthWrite: false,
        blending: THREE.CustomBlending,
        blendSrc: THREE.OneFactor,
        blendDst: THREE.OneMinusSrcAlphaFactor,
      }),
    );
    this.aura = new THREE.Mesh(this.geo(new THREE.PlaneGeometry(AURA_SIZE[0], AURA_SIZE[1])), auraMat);
    this.aura.position.set(...AURA_CENTER);
    this.aura.renderOrder = -1;
    this.aura.visible = false;
    this.object.add(this.aura);
  }

  /** Port of the prototype's frame(): `t` is seconds, used for breathing, tapping and tail sway. */
  applyPose(pose: CatPose, t: number): void {
    const sl = pose.sleepy;
    const awake = 1 - sl;
    const sq = pose.squash;

    this.root.position.y = pose.jumpY;
    this.root.scale.set(1 + sq * 0.55, 1 - sq, 1 + sq * 0.55);

    const breath = sl > 0.5 ? 1.4 : 2.4;
    this.bodyG.scale.setScalar(1 + pose.puff * 0.08);
    this.body.scale.y = 0.95 * (1 - sl * 0.1) + Math.sin(t * breath) * 0.014;

    const headG = this.headG;
    headG.position.y = HEAD_Y - sl * 0.4 + Math.sin(t * breath - 0.6) * 0.012;
    headG.position.z = HEAD_Z + sl * 0.18;
    headG.rotation.set(
      pose.ly * awake + sl * 0.3 - pose.yawn * 0.38,
      pose.lx * awake,
      sl * 0.18 + pose.happy * 0.2 + (pose.purring ? Math.sin(t * 70) * 0.014 : 0),
    );

    for (const e of this.ears) {
      e.pivot.rotation.z = -e.s * (0.3 + (1 - pose.ear) * 0.5) + pose.earTw * (e.s < 0 ? 1 : 0.4) * 0.3;
      e.pivot.scale.y = 1 + pose.puff * 0.15;
    }

    const ev = pose.eye;
    const happy = pose.happy > 0.5;
    const closed = ev < 0.18 && !happy;
    const knockedOut = pose.dizzy > 0.5;
    for (const e of this.eyes) {
      e.cross.visible = knockedOut;
      e.iris.visible = !happy && !closed && !knockedOut;
      e.sleepArc.visible = closed && !knockedOut;
      e.happyArc.visible = happy && !knockedOut;
      e.iris.scale.set(1 + Math.max(0, ev - 1) * 0.6, Math.max(0.04, Math.min(1.35, ev)), 1);
      e.pupil.position.set(pose.lx * 0.08 * awake, -pose.ly * 0.06 * awake, 0);
      e.brow.rotation.z = Math.PI / 2 + e.s * (pose.grumpy * 0.45 - 0.12);
      e.brow.position.y = 0.36 + Math.max(0, ev - 1) * 0.35 - pose.grumpy * 0.05;
    }

    this.mouthO.visible = pose.mouth > 0.06;
    this.mouthW.visible = !this.mouthO.visible;
    this.mouthO.scale.y = 0.02 + 0.16 * pose.mouth;

    this.pawR.position.y = 0.17 + (pose.tapping ? Math.max(0, Math.sin(t * 10)) * 0.17 : 0) * awake;

    const asleep = sl > 0.5;
    // Asleep, a poke adds a quicker swish on top of the slow sway. Fixed frequency, so changing its amplitude
    // never makes the tail jump.
    const sway = asleep
      ? Math.sin(t * 1.1) * 0.12 + Math.sin(t * 5.2) * pose.swish * 0.75
      : Math.sin(t * (2.6 + pose.tailUp * 2.5)) * (0.5 + pose.tailUp * 0.35);
    const pts = this.tail.points;
    for (let i = 0; i < 4; i++) {
      const a = TAIL_A[i];
      const z = TAIL_Z[i];
      const f = i / 3;
      pts[i].set(
        lerp(a[0] + sway * f * f * 0.9, z[0] + sway * f * 0.3, sl),
        lerp(a[1] + pose.tailUp * f * 0.35, z[1] + pose.swish * f * f * 0.22, sl),
        lerp(a[2], z[2], sl),
      );
    }
    this.tail.update(0.155 + pose.puff * 0.06, 0.195 + pose.puff * 0.06);
    this.tip.position.copy(pts[3]);

    this.shadow.scale.setScalar(Math.max(0.5, 1 - pose.jumpY * 0.35));

    // Aura: follows the jump; flares and the reduceMotion freeze are handled in the shader.
    const aura = pose.aura;
    this.aura.visible = aura > 0.01;
    if (this.aura.visible) {
      this.auraUniforms.uIntensity.value = aura;
      this.auraUniforms.uMotion.value = pose.auraFlicker ? 1 : 0;
      if (pose.auraFlicker) this.auraUniforms.uTime.value = t;
      this.aura.position.y = AURA_CENTER[1] + pose.jumpY; // the shader grows the spikes on flares
    }

    // Angry mark: pops in on the spring's overshoot, throbs like a vein while shown (static under reduceMotion).
    const anger = this.anger;
    if (anger) {
      const a = pose.annoyed;
      anger.visible = a > 0.02;
      if (anger.visible) {
        const throb = pose.auraFlicker ? 1 + 0.1 * Math.max(0, Math.sin(t * 11)) : 1;
        anger.scale.setScalar(ANGER_SIZE * Math.max(0, a) * throb);
        (anger.material as THREE.SpriteMaterial).opacity = Math.min(1, a * 1.6);
      }
    }

    this.applyKnockout(pose, t);

    // Zzz: port of the prototype's CSS keyframes (fade in over 20%, drift up-right, grow .6 → 1.1, fade out).
    for (let i = 0; i < this.zzz.length; i++) {
      const sprite = this.zzz[i];
      sprite.visible = pose.zzz;
      if (!pose.zzz) continue;
      const z = ZZZ[i];
      const p = ((((t - z.delay) % ZZZ_PERIOD) + ZZZ_PERIOD) % ZZZ_PERIOD) / ZZZ_PERIOD;
      (sprite.material as THREE.SpriteMaterial).opacity = p < 0.2 ? p / 0.2 : 1 - (p - 0.2) / 0.8;
      sprite.position.set(ZZZ_ANCHOR[0] + z.dx + ZZZ_DRIFT[0] * p, ZZZ_ANCHOR[1] + z.dy + ZZZ_DRIFT[1] * p, ZZZ_ANCHOR[2]);
      sprite.scale.setScalar(z.size * (0.6 + 0.5 * p));
    }
  }

  /** Flying object, impact burst, stars and bump. Allocation-free. */
  private applyKnockout(pose: CatPose, t: number): void {
    const kind = pose.bonkKind;
    const flying = kind >= 0 && pose.bonkT >= 0;
    for (let i = 0; i < this.projectiles.length; i++) this.projectiles[i].visible = flying && i === kind;
    if (!flying) this.bonkHitSet = false;

    if (flying) {
      const obj = this.projectiles[kind];
      const durian = kind === 2;
      this.headG.localToWorld(this.bonkTarget.set(HIT_LOCAL[0], HIT_LOCAL[1] + REST_LIFT[kind], HIT_LOCAL[2]));
      const bt = pose.bonkT;
      if (bt < KNOCKOUT_IMPACT_S) {
        const u = bt / KNOCKOUT_IMPACT_S;
        const from = durian ? DROP_FROM : THROW_FROM;
        if (durian) {
          // Dropped: falls faster and faster onto the head.
          const k = 1 - u * u;
          obj.position.set(this.bonkTarget.x + from[0] * k, this.bonkTarget.y + from[1] * k, this.bonkTarget.z + from[2] * k);
          obj.rotation.set(0.3, u * 2.4, 0.2);
        } else {
          // Thrown: an arc from the left, tumbling end over end.
          const k = 1 - u;
          obj.position.set(
            this.bonkTarget.x + from[0] * k,
            this.bonkTarget.y + from[1] * k + Math.sin(Math.PI * u) * THROW_ARC,
            this.bonkTarget.z + from[2] * k,
          );
          obj.rotation.set(0.25, 0.4, 1.2 - u * Math.PI * 3.5);
        }
      } else {
        if (!this.bonkHitSet) {
          this.bonkHit.copy(this.bonkTarget);
          this.bonkHitSet = true;
        }
        const v = bt - KNOCKOUT_IMPACT_S;
        const b = durian ? BOUNCE_DURIAN : BOUNCE;
        obj.position.set(
          this.bonkHit.x + b[0] * v,
          this.bonkHit.y + b[1] * v - 0.5 * BOUNCE_GRAVITY * v * v,
          this.bonkHit.z + b[2] * v,
        );
        obj.rotation.z -= durian ? 0.05 : 0.22;
        obj.rotation.x += 0.08;
      }
    }

    const burst = this.burst;
    if (burst) {
      const v = flying ? pose.bonkT - KNOCKOUT_IMPACT_S : -1;
      burst.visible = v >= 0 && v < BURST_S;
      if (burst.visible) {
        burst.position.copy(this.bonkHit);
        burst.scale.setScalar(0.5 + v * 4.5);
        (burst.material as THREE.SpriteMaterial).opacity = 1 - v / BURST_S;
      }
    }

    // Stars orbit the head while it is dizzy (they hold still under reduceMotion).
    const d = pose.dizzy;
    for (let i = 0; i < this.stars.length; i++) {
      const star = this.stars[i];
      star.visible = d > 0.05;
      if (!star.visible) continue;
      const a = (pose.auraFlicker ? t * 5 : 0.6) + (i * Math.PI * 2) / STARS;
      star.position.set(Math.cos(a) * STAR_ORBIT[0], STAR_ORBIT[1] + Math.sin(a) * 0.12, Math.sin(a) * STAR_ORBIT[2]);
      star.scale.setScalar(STAR_SIZE * Math.min(1, d));
      star.material.rotation = pose.auraFlicker ? t * 3 + i : 0;
    }

    const bump = Math.max(0, pose.bump);
    this.bump.visible = bump > 0.02;
    if (this.bump.visible) this.bump.scale.set(BUMP_SIZE * bump, BUMP_SIZE * 1.1 * bump, BUMP_SIZE * bump);
  }

  private buildProjectiles(): void {
    const SPH = this.sphere;
    const add = (g: THREE.Group) => {
      g.visible = false;
      this.object.add(g);
      this.projectiles.push(g);
    };

    // Stick: a knotted branch.
    const stick = new THREE.Group();
    this.mk(this.geo(new THREE.CylinderGeometry(0.075, 0.095, 1.7, 10)), WOOD, stick, { r: [0, 0, Math.PI / 2] });
    this.mk(SPH, WOOD_DARK, stick, { p: [0.35, 0.06, 0.02], s: [0.06, 0.05, 0.05], ol: 0 });
    this.mk(SPH, WOOD_DARK, stick, { p: [-0.45, -0.05, 0.03], s: [0.05, 0.045, 0.05], ol: 0 });
    this.mk(this.geo(new THREE.CylinderGeometry(0.03, 0.04, 0.32, 6)), WOOD, stick, { p: [0.25, 0.18, 0], r: [0, 0, -0.6] });
    add(stick);

    // Slipper (dép tổ ong): an oval sole with honeycomb cells and a blue strap over the toes.
    const slipper = new THREE.Group();
    this.mk(SPH, SLIPPER_SOLE, slipper, { s: [0.36, 0.07, 0.86] });
    for (const [x, z] of [[-0.14, -0.45], [0.14, -0.45], [0, -0.2], [-0.15, 0.05], [0.15, 0.05], [0, 0.3]]) {
      this.mk(SPH, SLIPPER_CELL, slipper, { p: [x, 0.055, z], s: [0.075, 0.02, 0.075], ol: 0 });
    }
    this.mk(this.geo(new THREE.TorusGeometry(0.3, 0.07, 8, 18, Math.PI)), SLIPPER_STRAP, slipper, {
      p: [0, 0.02, 0.4],
      s: [1.15, 0.85, 1],
    });
    add(slipper);

    // Durian: a spiky green ball with a stem.
    const durian = new THREE.Group();
    this.mk(SPH, DURIAN, durian, { s: [0.45, 0.5, 0.45] });
    const spike = this.geo(new THREE.ConeGeometry(0.07, 0.18, 6));
    const up = new THREE.Vector3(0, 1, 0);
    const n = new THREE.Vector3();
    const SPIKES = 46;
    for (let i = 0; i < SPIKES; i++) {
      // Fibonacci sphere: evenly spread directions.
      const y = 1 - (2 * (i + 0.5)) / SPIKES;
      const r = Math.sqrt(1 - y * y);
      const a = i * Math.PI * (3 - Math.sqrt(5));
      n.set(Math.cos(a) * r, y, Math.sin(a) * r);
      const m = this.mk(spike, DURIAN_SPIKE, durian, { p: [n.x * 0.45, n.y * 0.5, n.z * 0.45], ol: 0 });
      m.quaternion.setFromUnitVectors(up, n);
    }
    this.mk(this.geo(new THREE.CylinderGeometry(0.04, 0.06, 0.22, 6)), WOOD_DARK, durian, { p: [0, 0.6, 0] });
    add(durian);

    // Non-stick pan: a dark dish with a lighter inside and a long handle.
    const pan = new THREE.Group();
    this.mk(this.geo(new THREE.CylinderGeometry(0.55, 0.46, 0.13, 28)), PAN, pan);
    this.mk(this.geo(new THREE.CylinderGeometry(0.48, 0.48, 0.02, 28)), PAN_INSIDE, pan, { p: [0, 0.07, 0], ol: 0 });
    this.mk(this.geo(new THREE.CylinderGeometry(0.05, 0.065, 0.8, 8)), PAN_HANDLE, pan, {
      p: [0.92, 0.03, 0],
      r: [0, 0, Math.PI / 2],
    });
    add(pan);
  }

  /** Needs a DOM canvas; skipped outside the browser (unit tests). */
  private buildKnockoutSprites(): void {
    if (typeof document === 'undefined') return;
    const sprite = (map: THREE.Texture, parent: THREE.Object3D) => {
      const s = new THREE.Sprite(this.mat(new THREE.SpriteMaterial({ map, transparent: true, depthTest: false, depthWrite: false })));
      s.renderOrder = 10;
      s.visible = false;
      parent.add(s);
      return s;
    };
    const burstMap = burstTexture();
    this.textures.push(burstMap);
    this.burst = sprite(burstMap, this.object);
    const starMap = starTexture();
    this.textures.push(starMap);
    for (let i = 0; i < STARS; i++) this.stars.push(sprite(starMap, this.headG));
  }

  /** Head centre in CSS px relative to the canvas. Writes into `out`. */
  headScreen(camera: THREE.Camera, width: number, height: number, out: { x: number; y: number }): void {
    this.headG.getWorldPosition(this.headWorld).project(camera);
    out.x = ((this.headWorld.x + 1) / 2) * width;
    out.y = ((1 - this.headWorld.y) / 2) * height;
  }

  dispose(): void {
    this.tail.dispose();
    for (const t of this.textures) t.dispose();
    for (const g of this.geometries) g.dispose();
    for (const m of this.materials) m.dispose();
    this.gradient.dispose();
  }

  private mk(geo: THREE.BufferGeometry, color: number | THREE.Material, parent: THREE.Object3D, o: MeshOpts = {}): THREE.Mesh {
    const toon = typeof color === 'number';
    const m = new THREE.Mesh(geo, toon ? this.toon(color) : color);
    if (o.p) m.position.set(...o.p);
    if (o.s) m.scale.set(...o.s);
    if (o.r) m.rotation.set(...o.r);
    if (toon && o.ol !== 0) {
      const hull = new THREE.Mesh(geo, this.outlineMat);
      hull.scale.setScalar(o.ol ?? 1.06);
      m.add(hull);
    }
    parent.add(m);
    return m;
  }

  /** Needs a DOM canvas; skipped outside the browser (unit tests), where nothing is drawn anyway. */
  private buildZzz(): void {
    if (typeof document === 'undefined') return;
    const textures = new Map<string, THREE.Texture>();
    for (const z of ZZZ) {
      let map = textures.get(z.glyph);
      if (!map) {
        map = glyphTexture(z.glyph);
        textures.set(z.glyph, map);
        this.textures.push(map);
      }
      const sprite = new THREE.Sprite(
        this.mat(new THREE.SpriteMaterial({ map, transparent: true, depthTest: false, depthWrite: false })),
      );
      sprite.renderOrder = 10;
      sprite.visible = false;
      this.zzz.push(sprite);
      this.object.add(sprite);
    }
  }

  private buildAnger(): void {
    if (typeof document === 'undefined') return;
    const map = angerTexture();
    this.textures.push(map);
    const sprite = new THREE.Sprite(
      this.mat(new THREE.SpriteMaterial({ map, transparent: true, depthTest: false, depthWrite: false })),
    );
    sprite.position.set(...ANGER_POS);
    sprite.renderOrder = 10;
    sprite.visible = false;
    this.anger = sprite;
    this.headG.add(sprite);
  }

  /** One toon material per colour, shared across meshes. */
  private toon(color: number): THREE.MeshToonMaterial {
    let m = this.toonMats.get(color);
    if (!m) {
      m = this.mat(new THREE.MeshToonMaterial({ color, gradientMap: this.gradient }));
      this.toonMats.set(color, m);
    }
    return m;
  }

  private mat<M extends THREE.Material>(m: M): M {
    this.materials.push(m);
    return m;
  }

  private geo<G extends THREE.BufferGeometry>(g: G): G {
    this.geometries.push(g);
    return g;
  }
}

/** A letter in FUR2 with a soft white rim (the old CSS text-shadow), so it reads on dark and light desktops. */
function glyphTexture(glyph: string): THREE.CanvasTexture {
  const size = 64;
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = size;
  const ctx = canvas.getContext('2d')!;
  ctx.font = `600 ${size * 0.8}px 'Segoe UI', system-ui, sans-serif`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.lineJoin = 'round';
  ctx.lineWidth = 6;
  ctx.strokeStyle = 'rgba(255, 255, 255, 0.85)';
  ctx.strokeText(glyph, size / 2, size / 2);
  ctx.fillStyle = `#${FUR2.toString(16).padStart(6, '0')}`;
  ctx.fillText(glyph, size / 2, size / 2);
  const tex = new THREE.CanvasTexture(canvas);
  tex.colorSpace = THREE.SRGBColorSpace;
  return tex;
}

/** The anime angry mark: four bent strokes whose corners point at the centre, red with a white rim. */
function angerTexture(): THREE.CanvasTexture {
  const size = 128;
  const c = size / 2;
  const gap = 13;
  const reach = 50;
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = size;
  const ctx = canvas.getContext('2d')!;
  ctx.beginPath();
  for (const [sx, sy] of [[1, 1], [1, -1], [-1, 1], [-1, -1]]) {
    ctx.moveTo(c + sx * gap, c + sy * reach);
    ctx.quadraticCurveTo(c + sx * gap * 0.55, c + sy * gap * 0.55, c + sx * reach, c + sy * gap);
  }
  ctx.lineCap = 'round';
  ctx.lineJoin = 'round';
  ctx.lineWidth = 24;
  ctx.strokeStyle = 'rgba(255, 255, 255, 0.92)';
  ctx.stroke();
  ctx.lineWidth = 13;
  ctx.strokeStyle = '#e5383b';
  ctx.stroke();
  const tex = new THREE.CanvasTexture(canvas);
  tex.colorSpace = THREE.SRGBColorSpace;
  return tex;
}

/** A five-pointed star in amber with an ink rim: the "seeing stars" after a hit. */
function starTexture(): THREE.CanvasTexture {
  const size = 96;
  const c = size / 2;
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = size;
  const ctx = canvas.getContext('2d')!;
  ctx.beginPath();
  for (let i = 0; i < 10; i++) {
    const r = i % 2 === 0 ? 40 : 17;
    const a = -Math.PI / 2 + (i * Math.PI) / 5;
    ctx.lineTo(c + Math.cos(a) * r, c + Math.sin(a) * r);
  }
  ctx.closePath();
  ctx.lineJoin = 'round';
  ctx.lineWidth = 7;
  ctx.strokeStyle = `#${INK.toString(16).padStart(6, '0')}`;
  ctx.stroke();
  ctx.fillStyle = `#${AMBER.toString(16).padStart(6, '0')}`;
  ctx.fill();
  const tex = new THREE.CanvasTexture(canvas);
  tex.colorSpace = THREE.SRGBColorSpace;
  return tex;
}

/** The comic impact burst at the point of the hit: a white jagged star with an amber core. */
function burstTexture(): THREE.CanvasTexture {
  const size = 128;
  const c = size / 2;
  const canvas = document.createElement('canvas');
  canvas.width = canvas.height = size;
  const ctx = canvas.getContext('2d')!;
  const spikes = 12;
  const shape = (outer: number, inner: number) => {
    ctx.beginPath();
    for (let i = 0; i < spikes * 2; i++) {
      const r = i % 2 === 0 ? outer : inner;
      const a = (i * Math.PI) / spikes;
      ctx.lineTo(c + Math.cos(a) * r, c + Math.sin(a) * r);
    }
    ctx.closePath();
  };
  shape(60, 30);
  ctx.lineJoin = 'round';
  ctx.lineWidth = 6;
  ctx.strokeStyle = `#${INK.toString(16).padStart(6, '0')}`;
  ctx.stroke();
  ctx.fillStyle = '#ffffff';
  ctx.fill();
  shape(34, 18);
  ctx.fillStyle = `#${AMBER.toString(16).padStart(6, '0')}`;
  ctx.fill();
  const tex = new THREE.CanvasTexture(canvas);
  tex.colorSpace = THREE.SRGBColorSpace;
  return tex;
}
