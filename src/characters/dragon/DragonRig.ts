// The dragon (docs/characters/dragon/sheet.md): a chubby baby dragon with the Klat face. Its side frills move
// where the cat's ears do, its wings flap where the cat's tail wags, it thumps its spade tail where the cat
// taps its paw, and it puffs smoke from its nostrils while it yells.
import * as THREE from 'three';
import type { CatPose } from '../../kit/pose';
import { projectToScreen, type CharacterRig } from '../../kit/CharacterRig';
import { Face, type FaceLayout } from '../../kit/face';
import { CharacterFx, type FxLayout } from '../../kit/fx';
import { applyHeadTurn, applyJumpSquash, BREATH_BODY, BREATH_HEAD, BREATH_HEAD_PHASE, breathRate, tapBeat } from '../../kit/motion';
import type { CharacterPalette } from '../../kit/palette';
import { OUTLINE, ToonBuilder, type V3 } from '../../kit/ToonBuilder';
import { ToonTube } from '../../kit/tube';

export const DRAGON_PALETTE: CharacterPalette = {
  main: 0xe58e6b,
  shade: 0xb3604a,
  light: 0xfbe6c2,
  accent: 0xe8677a,
  iris: 0x2ec4b6,
  blush: 0xf59aa8,
};
const P = DRAGON_PALETTE;
const SMOKE = 0xdcdbe6;

const ROOT_X = 1.3;
const HEAD_Y = 2.05;
const HEAD_Z = 0.1;

/** Tail control points: A = awake, Z = curled around the paws while asleep. */
const TAIL_A: readonly V3[] = [[0.5, 0.4, -0.75], [1.3, 0.42, -0.75], [1.75, 0.85, -0.6], [1.75, 1.45, -0.5]];
const TAIL_Z: readonly V3[] = [[0.55, 0.35, -0.7], [1.2, 0.27, -0.1], [0.95, 0.22, 0.72], [0.3, 0.2, 1.1]];

/** Ridge spikes down the back: height on the body and how far each tips back (radians from upright). */
const RIDGE: readonly (readonly [number, number])[] = [
  [1.6, -0.79],
  [1.25, -1.24],
  [0.85, -1.68],
];
/** Belly plates: heights of the lines across the belly. */
const PLATES = [0.62, 0.85, 1.08];
/** Smoke: puffs per nostril, and how fast they rise (cycles a second). */
const PUFFS = 2;
const PUFF_RATE = 1.6;

const HIT_BOUNDS = {
  awake: new THREE.Box3(new THREE.Vector3(ROOT_X - 2.0, 0, -1.2), new THREE.Vector3(ROOT_X + 2.45, 3.7, 1.4)),
  asleep: new THREE.Box3(new THREE.Vector3(ROOT_X - 1.6, 0, -1.2), new THREE.Vector3(ROOT_X + 1.6, 3.0, 1.4)),
} as const;

const NOSTRIL: V3 = [0.1, -0.14, 1.05];

const FACE: FaceLayout = {
  eye: [0.45, 0.1, 0.9],
  eyeYaw: 0.32,
  mouth: [0, -0.33, 1.14],
  mouthOpen: [0, -0.42, 1.1],
  blush: { p: [0.78, -0.18, 0.8], s: [0.17, 0.08, 0.05] },
  muzzle: { p: [0.21, -0.3, 0.9], s: [0.32, 0.23, 0.26] },
};

const FX: FxLayout = {
  ground: [ROOT_X, 0.01, 0],
  shadowRadius: 1.45,
  auraCenter: [ROOT_X, 2.9, -1.35],
  auraSize: [4.6, 6.7],
  zzz: [2.1, 2.6, 0.3],
  anger: [0.86, 0.5, 1.0],
  hit: [0.05, 0.95, 0.25],
  starOrbit: [1.0, 1.2, 0.55],
  bump: [0.18, 0.9, 0.38],
};

const lerp = (a: number, b: number, t: number): number => a + (b - a) * t;
const UP = new THREE.Vector3(0, 1, 0);

interface Side {
  s: number;
  pivot: THREE.Group;
}

export class DragonRig implements CharacterRig {
  readonly object = new THREE.Group();
  readonly hitBounds = HIT_BOUNDS;

  private readonly kit = new ToonBuilder();
  private readonly root = new THREE.Group();
  private readonly bodyG = new THREE.Group();
  private readonly headG = new THREE.Group();
  private readonly body: THREE.Mesh;
  private readonly frills: Side[] = [];
  private readonly wings: Side[] = [];
  private readonly puffs: THREE.Mesh[] = [];
  private readonly tail: ToonTube;
  private readonly spade = new THREE.Group();
  private readonly tailDir = new THREE.Vector3();
  private readonly face: Face;
  private readonly fx: CharacterFx;

  constructor() {
    const kit = this.kit;
    const SPH = kit.sphere;

    this.root.position.x = ROOT_X;
    this.root.rotation.y = -0.22;
    this.object.add(this.root);

    // Body: the cat's sitting shape, with belly plates, a ridge of spikes and clawed paws
    this.root.add(this.bodyG);
    this.body = kit.mk(SPH, P.main, this.bodyG, { p: [0, 0.95, 0], s: [1.15, 0.95, 1] });
    kit.mk(SPH, P.light, this.bodyG, { p: [0, 0.85, 0.6], s: [0.68, 0.6, 0.5], ol: OUTLINE.none });
    // Each plate line is an arc around the belly's front, sized to the belly's cross-section at its height.
    const ARC = 1.5;
    const plate = kit.geo(new THREE.TorusGeometry(1, 0.035, 6, 24, ARC));
    for (const y of PLATES) {
      const k = Math.sqrt(1 - ((y - 0.85) / 0.6) ** 2) * 1.01;
      kit.mk(plate, P.shade, this.bodyG, {
        p: [0, y, 0.6],
        s: [0.68 * k, 0.5 * k, 1],
        r: [Math.PI / 2, 0, Math.PI / 2 - ARC / 2],
        ol: OUTLINE.none,
      });
    }
    const spike = kit.geo(new THREE.ConeGeometry(0.1, 0.26, 16));
    for (const [y, tip] of RIDGE) {
      const z = -Math.sqrt(1 - ((y - 0.95) / 0.95) ** 2) * 0.97;
      kit.mk(spike, P.shade, this.bodyG, { p: [0, y, z], r: [tip, 0, 0], ol: OUTLINE.muzzle });
    }
    kit.mk(SPH, P.main, this.bodyG, { p: [-0.78, 0.42, 0.25], s: [0.45, 0.4, 0.55] });
    kit.mk(SPH, P.main, this.bodyG, { p: [0.78, 0.42, 0.25], s: [0.45, 0.4, 0.55] });
    for (const s of [-1, 1]) {
      kit.mk(SPH, P.main, this.root, { p: [s * 0.42, 0.17, 0.9], s: [0.3, 0.2, 0.34] });
      for (const dx of [-0.11, 0, 0.11]) {
        kit.mk(SPH, P.light, this.root, { p: [s * 0.42 + dx, 0.13, 1.2], s: [0.05, 0.05, 0.06], ol: OUTLINE.eye });
      }
    }

    // Wings on the back: a shade membrane under a main-colour arm. They reach past the sides of the head,
    // which would hide anything closer in.
    const arm = kit.geo(new THREE.CapsuleGeometry(0.07, 1.05, 4, 10));
    for (const s of [-1, 1]) {
      const pivot = new THREE.Group();
      pivot.position.set(s * 0.6, 1.5, -0.62);
      this.bodyG.add(pivot);
      kit.mk(SPH, P.shade, pivot, { p: [s * 0.85, 0.42, 0], s: [0.72, 0.5, 0.05], r: [0, 0, s * 0.4], ol: OUTLINE.eye });
      for (const [x, y] of [[0.55, 0.02], [0.98, 0.1], [1.35, 0.32]]) {
        kit.mk(SPH, P.shade, pivot, { p: [s * x, y, 0], s: [0.2, 0.17, 0.05], ol: OUTLINE.eye });
      }
      kit.mk(arm, P.main, pivot, { p: [s * 0.75, 0.75, 0.04], r: [0, 0, -s * (Math.PI / 2 - 0.55)] });
      this.wings.push({ s, pivot });
    }

    // Head: horns, side frills, the Klat face on a snout, nostrils, smoke
    const headG = this.headG;
    headG.position.set(0, HEAD_Y, HEAD_Z);
    this.root.add(headG);
    kit.mk(SPH, P.main, headG, { s: [1.3, 1, 1.05] });
    kit.mk(SPH, P.shade, headG, { p: [0, 0.74, 0.66], s: [0.11, 0.11, 0.05], r: [0, 0, Math.PI / 4], ol: OUTLINE.none });
    const horn = kit.geo(new THREE.ConeGeometry(0.13, 0.62, 20));
    const frill = kit.geo(new THREE.ConeGeometry(0.26, 0.62, 24));
    const frillInner = kit.geo(new THREE.ConeGeometry(0.15, 0.4, 24));
    for (const s of [-1, 1]) {
      kit.mk(horn, P.light, headG, { p: [s * 0.42, 1.12, -0.12], r: [-0.45, 0, -s * 0.3], ol: OUTLINE.nose });
      const pivot = new THREE.Group();
      pivot.position.set(s * 1.12, 0.3, -0.15);
      headG.add(pivot);
      kit.mk(frill, P.main, pivot, { p: [s * 0.26, 0, 0], r: [0, 0, -s * (Math.PI / 2)], s: [1, 1, 0.4] });
      kit.mk(frillInner, P.accent, pivot, { p: [s * 0.22, 0, 0.08], r: [0, 0, -s * (Math.PI / 2)], s: [1, 1, 0.25], ol: OUTLINE.none });
      this.frills.push({ s, pivot });
    }
    this.face = new Face(kit, headG, P, FACE);
    for (const s of [-1, 1]) {
      kit.mk(SPH, kit.inkMat, headG, { p: [s * NOSTRIL[0], NOSTRIL[1], NOSTRIL[2]], s: [0.03, 0.022, 0.02] });
      for (let i = 0; i < PUFFS; i++) {
        const puff = kit.mk(SPH, SMOKE, headG, { s: [0, 0, 0], ol: OUTLINE.eye });
        puff.visible = false;
        this.puffs.push(puff);
      }
    }

    // Tail with a spade tip
    this.tail = new ToonTube(kit.toon(P.main), kit.outlineMat);
    this.root.add(this.tail.mesh, this.tail.outline);
    this.root.add(this.spade);
    kit.mk(kit.geo(new THREE.ConeGeometry(0.22, 0.34, 4)), P.shade, this.spade, { p: [0, 0.22, 0], s: [1, 1, 0.45] });
    kit.mk(kit.geo(new THREE.ConeGeometry(0.22, 0.12, 4)), P.shade, this.spade, { p: [0, 0, 0], r: [Math.PI, 0, 0], s: [1, 1, 0.45] });

    this.fx = new CharacterFx(kit, this.object, headG, P, FX);
  }

  applyPose(pose: CatPose, t: number): void {
    const sl = pose.sleepy;
    const awake = 1 - sl;
    const motion = pose.auraFlicker; // false under reduceMotion

    applyJumpSquash(this.root, pose);
    const breath = breathRate(pose);
    this.bodyG.scale.setScalar(1 + pose.puff * 0.08);
    this.body.scale.y = 0.95 * (1 - sl * 0.1) + Math.sin(t * breath) * BREATH_BODY;

    const headG = this.headG;
    headG.position.y = HEAD_Y - sl * 0.4 + Math.sin(t * breath + BREATH_HEAD_PHASE) * BREATH_HEAD;
    headG.position.z = HEAD_Z + sl * 0.18;
    applyHeadTurn(headG, pose, t);

    // Frills: lifted when alert, drooping when not; they twitch like ears.
    for (const f of this.frills) {
      f.pivot.rotation.z = f.s * (0.25 - (1 - pose.ear) * 0.75) + pose.earTw * (f.s < 0 ? 1 : 0.4) * 0.3;
      f.pivot.scale.setScalar(1 + pose.puff * 0.15);
    }

    // Wings: swept back and folded asleep, spread when startled, flapping while worked up.
    const flap = motion ? awake * pose.tailUp * Math.sin(t * 14) * 0.35 : 0;
    for (const w of this.wings) {
      w.pivot.rotation.set(0, -w.s * (0.25 + sl * 1.0), w.s * (flap + pose.puff * 0.3 - sl * 0.2));
      w.pivot.scale.setScalar(1 - sl * 0.2 + pose.puff * 0.1);
    }

    this.face.apply(pose);

    // Smoke from the nostrils while yelling, the anime "hmph": puffs jet down and out, swell and shrink away.
    const smoking = motion && this.face.mouthOpen && sl < 0.5 && pose.yawn < 0.3;
    for (let i = 0; i < this.puffs.length; i++) {
      const puff = this.puffs[i];
      puff.visible = smoking;
      if (!smoking) continue;
      const s = i < PUFFS ? -1 : 1;
      const p = (t * PUFF_RATE + (i % PUFFS) / PUFFS + (s > 0 ? 0.25 : 0)) % 1;
      puff.position.set(s * (NOSTRIL[0] + 0.05 + p * 0.75), NOSTRIL[1] - 0.04 - p * 0.35, NOSTRIL[2] + 0.04 + p * 0.15);
      puff.scale.setScalar(0.04 + 0.11 * Math.sin(Math.PI * p));
    }

    const asleep = sl > 0.5;
    const sway = asleep
      ? Math.sin(t * 1.1) * 0.12 + Math.sin(t * 5.2) * pose.swish * 0.75
      : Math.sin(t * (2.6 + pose.tailUp * 2.5)) * (0.4 + pose.tailUp * 0.3);
    // Impatient: the tip lifts and slaps down on the beat.
    const thump = pose.tapping ? tapBeat(t) * 0.35 * awake : 0;
    const pts = this.tail.points;
    for (let i = 0; i < 4; i++) {
      const a = TAIL_A[i];
      const z = TAIL_Z[i];
      const f = i / 3;
      pts[i].set(
        lerp(a[0] + sway * f * f * 0.9, z[0] + sway * f * 0.3, sl),
        lerp(a[1] + (pose.tailUp * 0.35 + thump) * f * f, z[1] + pose.swish * f * f * 0.22, sl),
        lerp(a[2], z[2], sl),
      );
    }
    this.tail.update(0.17 + pose.puff * 0.06, 0.21 + pose.puff * 0.06);
    this.tailDir.subVectors(pts[3], pts[2]).normalize();
    this.spade.quaternion.setFromUnitVectors(UP, this.tailDir);
    this.spade.position.copy(pts[3]);

    this.fx.apply(pose, t);
  }

  headScreen(camera: THREE.Camera, width: number, height: number, out: { x: number; y: number }): void {
    projectToScreen(this.headG, camera, width, height, out);
  }

  dispose(): void {
    this.tail.dispose();
    this.kit.dispose();
  }
}
