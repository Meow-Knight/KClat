// The snake (docs/characters/snake/sheet.md): a coiled cobra with the Klat face. Its hood spreads where the
// cat's ears perk up, its rattle buzzes where the cat's tail wags, and it flicks its forked tongue where the
// cat taps its paw. Asleep it lays its cheek on its coils and blows a snot bubble.
import * as THREE from 'three';
import type { CatPose } from '../../cat/CatController';
import { projectToScreen, type CharacterRig } from '../../kit/CharacterRig';
import { Face, type FaceLayout } from '../../kit/face';
import { CharacterFx, type FxLayout } from '../../kit/fx';
import { applyHeadTurn, applyJumpSquash, BREATH_BODY, BREATH_HEAD, BREATH_HEAD_PHASE, breathRate, tapBeat } from '../../kit/motion';
import type { CharacterPalette } from '../../kit/palette';
import { OUTLINE, ToonBuilder, type V3 } from '../../kit/ToonBuilder';
import { ToonTube } from '../../kit/tube';

export const SNAKE_PALETTE: CharacterPalette = {
  main: 0x8dbf8a,
  shade: 0x5d8b63,
  light: 0xf3eed2,
  accent: 0xe8677a,
  iris: 0xf59e2e,
  blush: 0xf7a9bc,
};
const P = SNAKE_PALETTE;

const ROOT_X = 1.3;
const HEAD_Y = 2.2;
const HEAD_Z = 0.15;

/** The coils, bottom to top: ring radius, tube radius, height. Each one rests on the one below. */
const COILS: readonly (readonly [number, number, number])[] = [
  [0.8, 0.36, 0.38],
  [0.55, 0.32, 0.95],
  [0.3, 0.28, 1.45],
];
/** Coils are a little flatter than round. */
const COIL_FLAT = 0.85;
/** Diamond spots on the coils' outer sides: coil index and angle from the front (radians). */
const SPOTS: readonly (readonly [number, number])[] = [
  [0, -0.75],
  [0, 0],
  [0, 0.75],
  [1, -0.4],
  [1, 0.45],
];

/** Tail control points: A = raised behind the coils, Z = hugging the bottom coil along the ground. */
const TAIL_A: readonly V3[] = [[0.6, 0.3, -0.5], [1.25, 0.35, -0.55], [1.6, 0.85, -0.45], [1.45, 1.45, -0.35]];
const TAIL_Z: readonly V3[] = [[0.6, 0.25, -0.5], [1.35, 0.2, 0.4], [0.35, 0.18, 1.32], [-0.7, 0.26, 1.1]];

/** Asleep: how far the head sinks and leans forward, and the extra sideways tilt (cheek on the coils). */
const SLEEP_DROP = 0.35;
const SLEEP_LEAN = 0.15;
const SLEEP_TILT = 0.22;
/** Hood folded away behind the head while asleep (fraction it shrinks). */
const SLEEP_HOOD = 0.35;
/** Snot bubble on the right nostril (head-local), and its size range over one breath. */
const BUBBLE_AT: V3 = [0.2, -0.13, 1.08];
const BUBBLE_SIZE: readonly [number, number] = [0.05, 0.17];
const BUBBLE = 0xd8f1f6;

/** Hood behind the head: full width when spread, and how narrow it folds (ear = 0). */
const HOOD_W = 1.7;
const HOOD_FOLDED = 0.55;

const HIT_BOUNDS = {
  awake: new THREE.Box3(new THREE.Vector3(ROOT_X - 1.85, 0, -1), new THREE.Vector3(ROOT_X + 2.15, 3.5, 1.4)),
  asleep: new THREE.Box3(new THREE.Vector3(ROOT_X - 1.6, 0, -1), new THREE.Vector3(ROOT_X + 1.6, 2.9, 1.5)),
} as const;

const FACE: FaceLayout = {
  eye: [0.45, 0.1, 0.9],
  eyeYaw: 0.32,
  mouth: [0, -0.33, 1.1],
  mouthOpen: [0, -0.42, 1.06],
  blush: { p: [0.8, -0.15, 0.8], s: [0.17, 0.08, 0.05] },
  muzzle: { p: [0.17, -0.3, 0.9], s: [0.3, 0.19, 0.22] },
};

const FX: FxLayout = {
  ground: [ROOT_X, 0.01, 0],
  shadowRadius: 1.35,
  auraCenter: [ROOT_X, 2.9, -1.35],
  auraSize: [4.6, 6.7],
  zzz: [2.1, 2.4, 0.3],
  anger: [0.9, 0.55, 0.95],
  hit: [0.05, 0.9, 0.25],
  starOrbit: [1.0, 1.05, 0.55],
  bump: [0.18, 0.85, 0.38],
};

const lerp = (a: number, b: number, t: number): number => a + (b - a) * t;
const UP = new THREE.Vector3(0, 1, 0);

export class SnakeRig implements CharacterRig {
  readonly object = new THREE.Group();
  readonly hitBounds = HIT_BOUNDS;

  private readonly kit = new ToonBuilder();
  private readonly root = new THREE.Group();
  private readonly bodyG = new THREE.Group();
  private readonly headG = new THREE.Group();
  private readonly hood: THREE.Mesh;
  private readonly tongue = new THREE.Group();
  private readonly bubble: THREE.Mesh;
  private readonly tail: ToonTube;
  private readonly rattle = new THREE.Group();
  private readonly tailDir = new THREE.Vector3();
  private readonly face: Face;
  private readonly fx: CharacterFx;

  constructor() {
    const kit = this.kit;
    const SPH = kit.sphere;

    this.root.position.x = ROOT_X;
    this.root.rotation.y = -0.22;
    this.object.add(this.root);

    // Coils, with diamond spots on their sides
    this.root.add(this.bodyG);
    for (const [ring, tube, y] of COILS) {
      const geo = kit.geo(new THREE.TorusGeometry(ring, tube, 16, 40));
      kit.mk(geo, P.main, this.bodyG, { p: [0, y, 0], r: [Math.PI / 2, 0, 0], s: [1, 1, COIL_FLAT] });
    }
    for (const [i, a] of SPOTS) {
      const [ring, tube, y] = COILS[i];
      const d = ring + tube * 0.93;
      kit.mk(SPH, P.shade, this.bodyG, {
        p: [Math.sin(a) * d, y, Math.cos(a) * d],
        s: [0.15, 0.1, 0.06],
        r: [0, a, Math.PI / 4],
        ol: OUTLINE.none,
      });
    }
    // Neck rising from the top coil, with a light throat
    kit.mk(SPH, P.main, this.bodyG, { p: [0, 1.75, 0.05], s: [0.42, 0.6, 0.42] });
    kit.mk(SPH, P.light, this.bodyG, { p: [0, 1.7, 0.3], s: [0.3, 0.45, 0.2], ol: OUTLINE.none });

    // Head: hood behind it, the Klat face, nostrils, tongue
    const headG = this.headG;
    headG.position.set(0, HEAD_Y, HEAD_Z);
    this.root.add(headG);
    // The hood's pale front shows as a band between the head and the hood's rim.
    this.hood = kit.mk(SPH, P.main, headG, { p: [0, -0.15, -0.42], s: [HOOD_W, 1.25, 0.22] });
    kit.mk(SPH, P.light, this.hood, { p: [0, -0.04, 0.9], s: [0.84, 0.84, 0.3], ol: OUTLINE.none });
    kit.mk(SPH, P.main, headG, { s: [1.35, 0.92, 1.05] });
    for (const [x, y, z, rz] of [[0, 0.72, 0.6, 0], [-0.3, 0.64, 0.66, 0.5], [0.3, 0.64, 0.66, -0.5]]) {
      kit.mk(SPH, P.shade, headG, { p: [x, y, z], s: [0.12, 0.08, 0.05], r: [0, 0, rz + Math.PI / 4], ol: OUTLINE.none });
    }
    this.face = new Face(kit, headG, P, FACE);
    for (const s of [-1, 1]) kit.mk(SPH, kit.inkMat, headG, { p: [s * 0.07, -0.17, 1.04], s: [0.025, 0.02, 0.02] });
    this.bubble = kit.mk(SPH, BUBBLE, headG, { p: BUBBLE_AT, ol: OUTLINE.eye });
    kit.mk(SPH, 0xffffff, this.bubble, { p: [-0.35, 0.35, 0.6], s: [0.25, 0.25, 0.2], ol: OUTLINE.none });
    this.bubble.visible = false;

    // Points down over the chin: straight at the viewer it would hide behind the mouth.
    this.tongue.position.set(0, -0.39, 1.04);
    this.tongue.rotation.x = 1.0;
    this.tongue.visible = false;
    headG.add(this.tongue);
    const stem = kit.geo(new THREE.CapsuleGeometry(0.028, 0.26, 4, 8));
    const fork = kit.geo(new THREE.CapsuleGeometry(0.022, 0.09, 4, 8));
    kit.mk(stem, P.accent, this.tongue, { p: [0, 0, 0.15], r: [Math.PI / 2, 0, 0], ol: OUTLINE.none });
    for (const s of [-1, 1]) {
      kit.mk(fork, P.accent, this.tongue, { p: [s * 0.04, 0, 0.33], r: [Math.PI / 2, 0, s * 0.55], ol: OUTLINE.none });
    }

    // Tail with a rattle at the tip (three shade beads, smallest at the end)
    this.tail = new ToonTube(kit.toon(P.main), kit.outlineMat);
    this.root.add(this.tail.mesh, this.tail.outline);
    this.root.add(this.rattle);
    for (const [y, r] of [[0.06, 0.15], [0.2, 0.125], [0.32, 0.095]]) {
      kit.mk(SPH, P.shade, this.rattle, { p: [0, y, 0], s: [r, r * 0.75, r], ol: OUTLINE.eye });
    }

    this.fx = new CharacterFx(kit, this.object, headG, P, FX);
  }

  applyPose(pose: CatPose, t: number): void {
    const sl = pose.sleepy;
    const awake = 1 - sl;

    applyJumpSquash(this.root, pose);
    const breath = breathRate(pose);
    this.bodyG.scale.set(1 + pose.puff * 0.06, (1 - sl * 0.06) * (1 + pose.puff * 0.06) + Math.sin(t * breath) * BREATH_BODY, 1 + pose.puff * 0.06);

    // Asleep, the head leans its cheek on the top coil.
    const headG = this.headG;
    const breathe = Math.sin(t * breath + BREATH_HEAD_PHASE);
    headG.position.y = HEAD_Y - sl * SLEEP_DROP + breathe * BREATH_HEAD;
    headG.position.z = HEAD_Z + sl * SLEEP_LEAN;
    applyHeadTurn(headG, pose, t);
    headG.rotation.z += sl * SLEEP_TILT;

    // Snot bubble: swells and shrinks with each breath once fast asleep (not when poked, not under reduceMotion).
    this.bubble.visible = pose.zzz && sl > 0.9 && pose.annoyed < 0.05;
    if (this.bubble.visible) this.bubble.scale.setScalar(lerp(BUBBLE_SIZE[0], BUBBLE_SIZE[1], 0.5 + 0.5 * breathe));

    // Hood: spreads when alert (ear), flares when startled (puff), shivers on a twitch.
    const fold = 1 - sl * SLEEP_HOOD;
    this.hood.scale.x = HOOD_W * (HOOD_FOLDED + (1 - HOOD_FOLDED) * pose.ear) * (1 + pose.puff * 0.15) * fold;
    this.hood.scale.y = 1.25 * (1 + pose.puff * 0.1) * fold;
    this.hood.rotation.z = pose.earTw * 0.12;

    this.face.apply(pose);

    // Tongue: flicks while impatient, hangs out wiggling while yelling.
    const out = this.face.mouthOpen ? 1 : pose.tapping ? tapBeat(t) : 0;
    const reach = out * awake;
    this.tongue.visible = reach > 0.05;
    if (this.tongue.visible) {
      this.tongue.scale.set(1, 1, reach);
      this.tongue.rotation.y = this.face.mouthOpen ? Math.sin(t * 30) * 0.25 : 0;
    }

    const asleep = sl > 0.5;
    const sway = asleep
      ? Math.sin(t * 1.1) * 0.1 + Math.sin(t * 5.2) * pose.swish * 0.6
      : Math.sin(t * (2.2 + pose.tailUp * 2)) * (0.3 + pose.tailUp * 0.25);
    const pts = this.tail.points;
    for (let i = 0; i < 4; i++) {
      const a = TAIL_A[i];
      const z = TAIL_Z[i];
      const f = i / 3;
      pts[i].set(
        lerp(a[0] + sway * f * f * 0.7, z[0] + sway * f * 0.3, sl),
        lerp(a[1] + pose.tailUp * f * 0.35, z[1] + pose.swish * f * f * 0.22, sl),
        lerp(a[2], z[2], sl),
      );
    }
    this.tail.update(0.13 + pose.puff * 0.04, 0.165 + pose.puff * 0.04);

    // Rattle: points along the tail's end and buzzes while the snake is worked up (or poked asleep).
    this.tailDir.subVectors(pts[3], pts[2]).normalize();
    this.rattle.quaternion.setFromUnitVectors(UP, this.tailDir);
    // auraFlicker is false under reduceMotion: no buzz then.
    const buzz = pose.auraFlicker ? (awake * pose.tailUp + pose.swish) * Math.sin(t * 45) * 0.05 : 0;
    this.rattle.position.set(pts[3].x + buzz, pts[3].y, pts[3].z);
    this.rattle.scale.setScalar(1 - sl * 0.25);

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
