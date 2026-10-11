// The cat's meshes (spec §5), ported 1:1 from docs/prototype/cat-prototype.html.
// No state logic here: applyPose() maps a CatPose onto transforms each frame, allocation-free.
import * as THREE from 'three';
import type { CatPose } from './CatController';
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

interface Eye {
  s: number;
  iris: THREE.Group;
  pupil: THREE.Group;
  sleepArc: THREE.Mesh;
  happyArc: THREE.Mesh;
  brow: THREE.Mesh;
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
  private readonly textures: THREE.Texture[] = [];
  private readonly eyes: Eye[] = [];
  private readonly ears: Ear[] = [];
  private readonly headWorld = new THREE.Vector3();

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
      this.eyes.push({ s, iris, pupil, sleepArc, happyArc, brow });

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
    for (const e of this.eyes) {
      e.iris.visible = !happy && !closed;
      e.sleepArc.visible = closed;
      e.happyArc.visible = happy;
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
    const sway = Math.sin(t * (asleep ? 1.1 : 2.6 + pose.tailUp * 2.5)) * (asleep ? 0.12 : 0.5 + pose.tailUp * 0.35);
    const pts = this.tail.points;
    for (let i = 0; i < 4; i++) {
      const a = TAIL_A[i];
      const z = TAIL_Z[i];
      const f = i / 3;
      pts[i].set(
        lerp(a[0] + sway * f * f * 0.9, z[0] + sway * f * 0.3, sl),
        lerp(a[1] + pose.tailUp * f * 0.35, z[1], sl),
        lerp(a[2], z[2], sl),
      );
    }
    this.tail.update(0.155 + pose.puff * 0.06, 0.195 + pose.puff * 0.06);
    this.tip.position.copy(pts[3]);

    this.shadow.scale.setScalar(Math.max(0.5, 1 - pose.jumpY * 0.35));

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
