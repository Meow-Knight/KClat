// The puppy (docs/characters/puppy/sheet.md): a self-appointed guard dog. A chubby sandy pup with long brown
// floppy ears, a brown patch over one eye, a cream muzzle with a big dark button nose, a red polka-dot bandana
// and a stubby tail with a cream tip that wags fast. The tongue is always out a little (a "blep"); impatient,
// it pants. Asleep it lies over a bone between its front paws, tongue still out.
import * as THREE from 'three';
import type { CatPose } from '../../kit/pose';
import { projectToScreen, type CharacterRig } from '../../kit/CharacterRig';
import { Face, type FaceLayout } from '../../kit/face';
import { CharacterFx, type FxLayout } from '../../kit/fx';
import { applyHeadTurn, applyJumpSquash, BREATH_BODY, BREATH_HEAD, BREATH_HEAD_PHASE, breathRate, tapBeat } from '../../kit/motion';
import { HIGHLIGHT, MOUTH, type CharacterPalette } from '../../kit/palette';
import { OUTLINE, ToonBuilder, type V3 } from '../../kit/ToonBuilder';
import { ToonTube } from '../../kit/tube';

export const PUPPY_PALETTE: CharacterPalette = {
  /** Sandy fawn. */
  main: 0xe2b47c,
  /** Brown: the floppy ears, the eye patch, the Zzz. */
  shade: 0xa9713f,
  /** Cream: muzzle, belly, paws, tail tip, the bone, the bandana's dots. */
  light: 0xf7eedf,
  /** The bandana. The nose is dark (MOUTH), not accent. */
  accent: 0xe5484d,
  iris: 0x4fb0e8,
  /** Also the tongue. */
  blush: 0xf7a1b0,
};
const P = PUPPY_PALETTE;

const ROOT_X = 1.3;
const HEAD_Y = 2.05;
const HEAD_Z = 0.1;
const BODY: V3 = [1.12, 0.95, 1];
const HEAD: V3 = [1.32, 1, 1.05];

/** Asleep the head drops lower and further forward than the cat's: chin on the bone. */
const SLEEP_DROP = 0.55;
const SLEEP_FWD = 0.3;

/** The bandana: a collar round the neck at this height, and a bib (a flattened cone) hanging off the front. */
const BANDANA_Y = 1.5;
const BIB_R = 0.6;
const BIB_H = 0.78;

/** Floppy ear: pivot at the side of the head (head-local, right), the flap hangs below it. */
const EAR_PIVOT: V3 = [1.08, 0.5, 0.05];
const EAR: V3 = [0.32, 0.62, 0.17];
/** Hanging close to the head asleep, flared out when alert. */
const EAR_DOWN = 0.12;
const EAR_ALERT = 0.42;

/** The tongue hangs from just under the mouth's right half. */
const TONGUE_AT: V3 = [0.06, -0.42, 1.08];
const TONGUE: V3 = [0.085, 0.13, 0.05];

/** The bone between the front paws, only there asleep (grows in between these sleepy values). */
const BONE_AT: V3 = [0.05, 0.2, 1.25];
const BONE_FROM = 0.45;
const BONE_TO = 0.9;

const TAIL_AWAKE: readonly V3[] = [[0.6, 0.45, -0.7], [1.15, 0.6, -0.75], [1.42, 1.0, -0.65], [1.4, 1.38, -0.55]];
const TAIL_ASLEEP: readonly V3[] = [[0.55, 0.35, -0.7], [1.1, 0.27, -0.2], [1.0, 0.22, 0.45], [0.55, 0.2, 0.8]];
const TAIL_R = 0.17;

const HIT_BOUNDS = {
  awake: new THREE.Box3(new THREE.Vector3(ROOT_X - 1.7, 0, -1), new THREE.Vector3(ROOT_X + 1.9, 3.2, 1.4)),
  asleep: new THREE.Box3(new THREE.Vector3(ROOT_X - 1.7, 0, -1), new THREE.Vector3(ROOT_X + 1.7, 2.8, 1.6)),
} as const;

/** The cat's face with a bigger muzzle; the button nose is built here (dark, not accent). */
const FACE: FaceLayout = {
  eye: [0.45, 0.1, 0.9],
  eyeYaw: 0.32,
  eyeScale: 1.05,
  mouth: [0, -0.4, 1.13],
  mouthOpen: [0, -0.48, 1.1],
  blush: { p: [0.8, -0.2, 0.78], s: [0.17, 0.08, 0.05] },
  muzzle: { p: [0.2, -0.32, 0.9], s: [0.33, 0.25, 0.25] },
};
const NOSE_AT: V3 = [0, -0.16, 1.12];
const NOSE: V3 = [0.16, 0.11, 0.09];

const FX: FxLayout = {
  ground: [ROOT_X, 0.01, 0],
  shadowRadius: 1.45,
  auraCenter: [ROOT_X, 2.85, -1.35],
  auraSize: [5.0, 6.6],
  zzz: [2.2, 2.5, 0.3],
  anger: [0.86, 0.62, 0.95],
  hit: [0.05, 0.95, 0.25],
  starOrbit: [1.05, 1.08, 0.6],
  bump: [0.2, 0.9, 0.4],
};

const lerp = (a: number, b: number, t: number): number => a + (b - a) * t;
const smooth = (a: number, b: number, x: number): number => {
  const t = Math.min(1, Math.max(0, (x - a) / (b - a)));
  return t * t * (3 - 2 * t);
};

interface Ear {
  s: number;
  pivot: THREE.Group;
}

export class PuppyRig implements CharacterRig {
  readonly object = new THREE.Group();
  readonly hitBounds = HIT_BOUNDS;

  private readonly kit = new ToonBuilder();
  private readonly root = new THREE.Group();
  private readonly bodyG = new THREE.Group();
  private readonly body: THREE.Mesh;
  private readonly headG = new THREE.Group();
  private readonly ears: Ear[] = [];
  private readonly tongue = new THREE.Group();
  private readonly bone = new THREE.Group();
  private readonly tail: ToonTube;
  private readonly tip: THREE.Mesh;
  private readonly face: Face;
  private readonly fx: CharacterFx;

  constructor() {
    const kit = this.kit;
    const SPH = kit.sphere;

    this.root.position.x = ROOT_X;
    this.root.rotation.y = -0.22;
    this.object.add(this.root);

    // Body: egg, cream belly, haunches, cream front paws
    this.root.add(this.bodyG);
    this.body = kit.mk(SPH, P.main, this.bodyG, { p: [0, 0.95, 0], s: BODY });
    kit.mk(SPH, P.light, this.bodyG, { p: [0, 0.7, 0.6], s: [0.64, 0.5, 0.46], ol: OUTLINE.none });
    for (const s of [-1, 1]) {
      kit.mk(SPH, P.main, this.bodyG, { p: [s * 0.78, 0.42, 0.25], s: [0.47, 0.42, 0.56] });
      kit.mk(SPH, P.light, this.root, { p: [s * 0.42, 0.17, 0.9], s: [0.31, 0.2, 0.35] });
    }

    // Bandana: a rounded red bib under the chin with cream polka dots, and a knot on the right
    kit.mk(kit.geo(new THREE.TorusGeometry(1, 0.1, 10, 40)), P.accent, this.bodyG, {
      p: [0, BANDANA_Y, 0.02],
      r: [Math.PI / 2 - 0.12, 0, 0],
      s: [0.93, 0.86, 1],
    });
    const bib = kit.mk(kit.geo(new THREE.ConeGeometry(BIB_R, BIB_H, 32)), P.accent, this.bodyG, {
      p: [0, BANDANA_Y - 0.42, 0.9],
      r: [Math.PI + 0.22, 0, 0],
      s: [1, 1, 0.4],
    });
    // Dots on the bib's front face. The cone is flipped tip-down, so its front is local -z; radius at local y.
    for (const [x, y] of [[-0.17, -0.1], [0.17, -0.1], [0, 0.06], [-0.32, -0.24], [0.32, -0.24]] as const) {
      const r = BIB_R * (0.5 - y / BIB_H);
      if (Math.abs(x) > r * 0.8) continue;
      kit.mk(kit.sphere, P.light, bib, { p: [x, y, -Math.sqrt(r * r - x * x) - 0.01], s: [0.055, 0.055, 0.05], ol: OUTLINE.none });
    }
    // The knot at the right side of the neck, two little ends hanging off it.
    kit.mk(SPH, P.accent, this.bodyG, { p: [0.8, BANDANA_Y - 0.04, 0.45], s: [0.13, 0.11, 0.11], ol: OUTLINE.eye });
    for (const [rz, dx] of [[0.5, -0.06], [-0.35, 0.09]] as const) {
      kit.mk(SPH, P.accent, this.bodyG, { p: [0.8 + dx, BANDANA_Y - 0.2, 0.47], s: [0.06, 0.13, 0.05], r: [0, 0, rz], ol: OUTLINE.eye });
    }

    // Head: eye patch, the Klat face with a cream muzzle, a dark button nose, the tongue, floppy ears
    const headG = this.headG;
    headG.position.set(0, HEAD_Y, HEAD_Z);
    this.root.add(headG);
    kit.mk(SPH, P.main, headG, { s: HEAD });
    // Patch round the viewer's left eye: a thin decal turned to the head's surface there.
    kit.mk(SPH, P.shade, headG, { p: [-0.43, 0.13, 0.82], s: [0.4, 0.42, 0.16], r: [-0.08, -0.38, 0.25], ol: OUTLINE.none });
    this.face = new Face(kit, headG, P, FACE);
    kit.mk(SPH, MOUTH, headG, { p: NOSE_AT, s: NOSE, ol: OUTLINE.nose });
    kit.mk(SPH, kit.mat(new THREE.MeshBasicMaterial({ color: HIGHLIGHT })), headG, {
      p: [NOSE_AT[0] - 0.05, NOSE_AT[1] + 0.04, NOSE_AT[2] + 0.08],
      s: [0.04, 0.025, 0.02],
    });

    this.tongue.position.set(...TONGUE_AT);
    headG.add(this.tongue);
    kit.mk(SPH, P.blush, this.tongue, { p: [0, -TONGUE[1] * 0.7, 0], s: TONGUE, ol: OUTLINE.eye });
    kit.mk(kit.geo(new THREE.CapsuleGeometry(0.008, 0.08, 4, 6)), kit.inkMat, this.tongue, { p: [0, -TONGUE[1] * 0.75, TONGUE[2] + 0.002] });

    const [ex, ey, ez] = EAR_PIVOT;
    for (const s of [-1, 1]) {
      const pivot = new THREE.Group();
      pivot.position.set(s * ex, ey, ez);
      pivot.rotation.y = s * 0.25;
      headG.add(pivot);
      kit.mk(SPH, P.shade, pivot, { p: [s * 0.08, -EAR[1] * 0.78, 0], s: EAR, r: [0, 0, s * 0.12] });
      this.ears.push({ s, pivot });
    }

    // Tail: short and fat with a cream tip
    this.tail = new ToonTube(kit.toon(P.main), kit.outlineMat);
    this.root.add(this.tail.mesh, this.tail.outline);
    this.tip = kit.mk(SPH, P.light, this.root, { s: [0.19, 0.19, 0.19], ol: OUTLINE.tip });

    // Bone: a cream bar with two knobs at each end
    this.bone.position.set(...BONE_AT);
    this.bone.rotation.y = 0.15;
    this.root.add(this.bone);
    kit.mk(kit.geo(new THREE.CapsuleGeometry(0.085, 0.62, 4, 12)), P.light, this.bone, { r: [0, 0, Math.PI / 2] });
    for (const x of [-0.42, 0.42]) {
      for (const y of [-0.08, 0.08]) kit.mk(SPH, P.light, this.bone, { p: [x, y, 0], s: [0.12, 0.12, 0.12], ol: OUTLINE.muzzle });
    }

    this.fx = new CharacterFx(kit, this.object, headG, P, FX);
  }

  applyPose(pose: CatPose, t: number): void {
    const sl = pose.sleepy;
    const awake = 1 - sl;
    const motion = pose.auraFlicker ? 1 : 0;
    const pant = pose.tapping ? tapBeat(t) * awake : 0;

    applyJumpSquash(this.root, pose);
    const breath = breathRate(pose);
    this.bodyG.scale.setScalar(1 + pose.puff * 0.08);
    // Panting heaves the chest a little.
    this.body.scale.y = BODY[1] * (1 - sl * 0.1) + Math.sin(t * breath) * BREATH_BODY + pant * 0.02;

    const headG = this.headG;
    headG.position.y = HEAD_Y - sl * SLEEP_DROP + Math.sin(t * breath + BREATH_HEAD_PHASE) * BREATH_HEAD;
    headG.position.z = HEAD_Z + sl * SLEEP_FWD;
    applyHeadTurn(headG, pose, t);

    // Ears hang; alert they flare out, a twitch flips the left one up, puff throws both out.
    for (const e of this.ears) {
      const lift = EAR_DOWN + pose.ear * (EAR_ALERT - EAR_DOWN) * awake + pose.puff * 0.45 + pose.earTw * (e.s < 0 ? 0.5 : 0.15);
      e.pivot.rotation.z = e.s * lift;
    }

    this.face.apply(pose);

    // The blep: a little tongue always out; impatient it pants in and out, petted it lolls further.
    const out = 0.65 + pant * 0.6 + pose.happy * 0.35 + pose.mouth * 0.2;
    this.tongue.scale.set(1, out, 1);
    this.tongue.rotation.z = Math.sin(t * 10) * 0.12 * pant;

    // The tail wags fast (faster and wider worked up); asleep it curls round the paws and thumps when poked.
    const sway = sl > 0.5
      ? Math.sin(t * 1.1) * 0.1 * motion + Math.sin(t * 5.2) * pose.swish * 0.7
      : Math.sin(t * (7 + pose.tailUp * 7)) * (0.32 + pose.tailUp * 0.3);
    const pts = this.tail.points;
    for (let i = 0; i < 4; i++) {
      const a = TAIL_AWAKE[i];
      const z = TAIL_ASLEEP[i];
      const f = i / 3;
      pts[i].set(
        lerp(a[0] + sway * f * f * 0.8, z[0] + sway * f * 0.3, sl),
        lerp(a[1] + pose.tailUp * f * 0.3, z[1] + pose.swish * f * f * 0.2, sl),
        lerp(a[2], z[2], sl),
      );
    }
    const radius = TAIL_R + pose.puff * 0.06;
    this.tail.update(radius, radius + 0.04);
    this.tip.position.copy(pts[3]);

    const grow = smooth(BONE_FROM, BONE_TO, sl);
    this.bone.visible = grow > 0.02;
    this.bone.scale.setScalar(Math.max(0.02, grow));

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
