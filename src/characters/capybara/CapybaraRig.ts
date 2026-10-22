// The capybara (docs/characters/capybara/sheet.md): the unbothered king. A pear-shaped golden-brown blob, head
// melting into a wider body, with a big round dark snout, short round ears, stubby legs and dark toed feet, and
// a blade of grass in the corner of its mouth. Asleep it floats on a swim ring that puffs up round its waist,
// lying back with its chin up, bobbing and chewing; awake it sits bolt upright with the Klat
// scowl. No tail: the ring rocks when poked asleep, and awake the grass cocks up and flicks while it waits.
import * as THREE from 'three';
import type { CatPose } from '../../kit/pose';
import { projectToScreen, type CharacterRig } from '../../kit/CharacterRig';
import { Face, type FaceLayout } from '../../kit/face';
import { CharacterFx, type FxLayout } from '../../kit/fx';
import { applyHeadTurn, applyJumpSquash, BREATH_BODY, BREATH_HEAD, BREATH_HEAD_PHASE, breathRate, tapBeat } from '../../kit/motion';
import type { CharacterPalette } from '../../kit/palette';
import { OUTLINE, ToonBuilder, type V3 } from '../../kit/ToonBuilder';

export const CAPYBARA_PALETTE: CharacterPalette = {
  main: 0xd9a55e,
  /** Dark brown: the snout, inner ears, feet. */
  shade: 0x85562f,
  /** Only the ring's bands: the capybara has no pale belly. */
  light: 0xf6ebdc,
  /** The swim ring, and the hatching on the blush. */
  accent: 0xef6f5e,
  /** Also the grass. */
  iris: 0x7bc043,
  blush: 0xf4a08a,
};
const P = CAPYBARA_PALETTE;

const ROOT_X = 1.3;
const HEAD_Y = 2.2;
const HEAD_Z = 0.15;

/** Body wider than the head (the pear shape), head sunk into it so the two read as one blob. */
const BODY: V3 = [1.3, 0.95, 1.1];
const HEAD: V3 = [1.08, 0.82, 0.95];
/** The big round snout disc (head-local centre and radii). */
const SNOUT_AT: V3 = [0, -0.28, 0.8];
const SNOUT: V3 = [0.42, 0.44, 0.25];

/** Asleep the body leans back around a point at ring height, and the head drops back with its chin up. */
const LEAN_PIVOT = 0.6;
const SLEEP_LEAN = 0.3;
const SLEEP_DROP = 0.4;
const SLEEP_BACK = 0.3;
const SLEEP_CHIN_UP = 0.55;
/** Floating: how far the whole capybara bobs and rocks asleep. */
const FLOAT_BOB = 0.06;
const FLOAT_ROCK = 0.035;

/** The swim ring round the waist: height, ring radius, tube radius, and how much shallower front to back. */
const RING_Y = 0.8;
const RING_R = 1.45;
const RING_TUBE = 0.3;
const RING_DEPTH = 0.9;
/** The ring puffs up between these sleepy values: it is only there while floating. */
const RING_FROM = 0.45;
const RING_TO = 0.9;
/** Its ink outline is a fatter torus (a scaled copy would thin the line on the tube's top). */
const RING_LINE = 0.055;
/** Four light bands round the ring: their angles, and how long each band is (radians). */
const BANDS = [0.35, 0.35 + Math.PI / 2, 0.35 + Math.PI, 0.35 + (3 * Math.PI) / 2];
const BAND_ARC = 0.42;

/** Front legs hang from a hip pivot (the right one lifts to tap). */
const LEG_PIVOT: V3 = [0.3, 0.85, 0.72];
const LEG_LEN = 0.75;

/** The grass blade: where it sits in the mouth (head-local, right corner) and how it leans out. */
const GRASS_AT: V3 = [0.2, -0.42, 0.99];
const GRASS_LEAN = -1.4;
/** Chewing while asleep, rad/s. */
const CHEW = 7;

const HIT_BOUNDS = {
  awake: new THREE.Box3(new THREE.Vector3(ROOT_X - 1.55, 0, -1.2), new THREE.Vector3(ROOT_X + 1.75, 3.35, 1.4)),
  asleep: new THREE.Box3(new THREE.Vector3(ROOT_X - 1.9, 0, -1.4), new THREE.Vector3(ROOT_X + 1.9, 2.9, 1.7)),
} as const;

/** Small wide-set eyes above the snout; the snout is its own dark disc, so no muzzle puffs and no nose. */
const FACE: FaceLayout = {
  eye: [0.5, 0.22, 0.72],
  eyeYaw: 0.48,
  eyeScale: 0.9,
  mouth: [0, -0.42, 1.045],
  mouthOpen: [0, -0.5, 1.03],
  blush: { p: [0.68, -0.07, 0.7], s: [0.17, 0.09, 0.05] },
};

const FX: FxLayout = {
  ground: [ROOT_X, 0.01, 0],
  shadowRadius: 1.6,
  auraCenter: [ROOT_X, 2.9, -1.35],
  auraSize: [4.8, 6.7],
  zzz: [2.0, 2.4, 0.3],
  anger: [0.72, 0.5, 0.8],
  hit: [0.05, 0.8, 0.2],
  starOrbit: [0.85, 0.95, 0.5],
  bump: [0.15, 0.78, 0.33],
};

const smooth = (a: number, b: number, x: number): number => {
  const t = Math.min(1, Math.max(0, (x - a) / (b - a)));
  return t * t * (3 - 2 * t);
};

interface Ear {
  s: number;
  pivot: THREE.Group;
}

export class CapybaraRig implements CharacterRig {
  readonly object = new THREE.Group();
  readonly hitBounds = HIT_BOUNDS;

  private readonly kit = new ToonBuilder();
  private readonly root = new THREE.Group();
  /** Pivot the body leans back around while asleep. */
  private readonly lean = new THREE.Group();
  private readonly bodyG = new THREE.Group();
  private readonly body: THREE.Mesh;
  private readonly legs: THREE.Group[] = [];
  private readonly headG = new THREE.Group();
  private readonly snout: THREE.Mesh;
  private readonly grass = new THREE.Group();
  private readonly ring = new THREE.Group();
  private readonly ears: Ear[] = [];
  private readonly face: Face;
  private readonly fx: CharacterFx;

  constructor() {
    const kit = this.kit;
    const SPH = kit.sphere;
    /** A dark foot with three toes along its front edge. */
    const foot = (parent: THREE.Object3D, p: V3) => {
      const g = new THREE.Group();
      g.position.set(...p);
      parent.add(g);
      kit.mk(SPH, P.shade, g, { s: [0.22, 0.12, 0.26] });
      for (const x of [-0.09, 0, 0.09]) kit.mk(SPH, P.shade, g, { p: [x, -0.02, 0.24], s: [0.065, 0.06, 0.065], ol: OUTLINE.eye });
    };

    this.root.position.x = ROOT_X;
    this.root.rotation.y = -0.22;
    this.object.add(this.root);

    // Body: a wide pear with big haunches on the sides and the hind feet peeking out under them
    this.lean.position.y = LEAN_PIVOT;
    this.root.add(this.lean);
    this.bodyG.position.y = -LEAN_PIVOT;
    this.lean.add(this.bodyG);
    this.body = kit.mk(SPH, P.main, this.bodyG, { p: [0, 0.95, 0], s: BODY });
    for (const s of [-1, 1]) {
      kit.mk(SPH, P.main, this.bodyG, { p: [s * 0.88, 0.5, 0.15], s: [0.5, 0.5, 0.62] });
      foot(this.bodyG, [s * 0.98, 0.1, 0.5]);
    }

    // Front legs: stubby columns ending in dark toed feet
    for (const s of [-1, 1]) {
      const leg = new THREE.Group();
      leg.position.set(s * LEG_PIVOT[0], LEG_PIVOT[1], LEG_PIVOT[2]);
      this.root.add(leg);
      kit.mk(SPH, P.main, leg, { p: [0, -0.4, 0.1], s: [0.27, 0.46, 0.3] });
      foot(leg, [0, -LEG_LEN, 0.3]);
      this.legs.push(leg);
    }

    // Swim ring: accent with four light bands, its own fat outline
    this.ring.position.y = RING_Y;
    this.root.add(this.ring);
    const shape = new THREE.Group();
    shape.rotation.x = Math.PI / 2;
    shape.scale.set(1, RING_DEPTH, 1);
    this.ring.add(shape);
    const tube = kit.mk(kit.geo(new THREE.TorusGeometry(RING_R, RING_TUBE, 18, 56)), P.accent, shape, { ol: OUTLINE.none });
    tube.add(new THREE.Mesh(kit.geo(new THREE.TorusGeometry(RING_R, RING_TUBE + RING_LINE, 18, 56)), kit.outlineMat));
    const band = kit.geo(new THREE.TorusGeometry(RING_R, RING_TUBE * 1.02, 18, 10, BAND_ARC));
    for (const a of BANDS) kit.mk(band, P.light, shape, { r: [0, 0, a - BAND_ARC / 2], ol: OUTLINE.none });

    // Head: short round ears, the dark snout disc with nostrils and a philtrum, the Klat face, hatched blush
    const headG = this.headG;
    this.root.add(headG);
    kit.mk(SPH, P.main, headG, { s: HEAD });
    this.snout = kit.mk(SPH, P.shade, headG, { p: SNOUT_AT, s: SNOUT, ol: OUTLINE.muzzle });
    for (const s of [-1, 1]) {
      kit.mk(SPH, kit.inkMat, headG, { p: [s * 0.085, -0.1, 1.02], s: [0.05, 0.022, 0.02], r: [0, 0, s * -0.45] });
    }
    kit.mk(kit.geo(new THREE.CapsuleGeometry(0.016, 0.18, 4, 8)), kit.inkMat, headG, { p: [0, -0.25, 1.05] });
    this.face = new Face(kit, headG, P, FACE);
    const hatch = kit.geo(new THREE.CapsuleGeometry(0.012, 0.07, 4, 6));
    const [bx, by, bz] = FACE.blush.p;
    for (const s of [-1, 1]) {
      for (const dx of [-0.06, 0, 0.06]) {
        kit.mk(hatch, P.accent, headG, { p: [s * bx + dx, by, bz + 0.055], r: [0, 0, -0.5], ol: OUTLINE.none });
      }
    }

    for (const s of [-1, 1]) {
      const pivot = new THREE.Group();
      pivot.position.set(s * 0.7, 0.62, -0.05);
      pivot.rotation.y = s * 0.35;
      headG.add(pivot);
      kit.mk(SPH, P.main, pivot, { p: [0, 0.13, 0], s: [0.21, 0.24, 0.12], ol: OUTLINE.muzzle });
      kit.mk(SPH, P.shade, pivot, { p: [0, 0.11, 0.08], s: [0.12, 0.15, 0.05], ol: OUTLINE.none });
      this.ears.push({ s, pivot });
    }

    // Grass: a stalk with a leaf off its side. Thin, so the outline is a hull scaled across the stalk only.
    this.grass.position.set(...GRASS_AT);
    headG.add(this.grass);
    const stalk = kit.geo(new THREE.CapsuleGeometry(0.035, 0.95, 4, 8));
    const leaf = kit.geo(new THREE.CapsuleGeometry(0.03, 0.3, 4, 8));
    for (const [geo, p, rz] of [[stalk, [0, 0.5, 0], 0], [leaf, [0.09, 0.68, 0], -0.6]] as const) {
      const m = kit.mk(geo, P.iris, this.grass, { p, r: [0, 0, rz], ol: OUTLINE.none });
      const hull = new THREE.Mesh(geo, kit.outlineMat);
      hull.scale.set(1.7, 1.06, 1.7);
      m.add(hull);
    }

    this.fx = new CharacterFx(kit, this.object, headG, P, FX);
  }

  applyPose(pose: CatPose, t: number): void {
    const sl = pose.sleepy;
    const awake = 1 - sl;
    // Decorative motion (floating, chewing) stops under reduceMotion.
    const motion = pose.auraFlicker ? 1 : 0;

    // Asleep it floats: bobs up and rocks a little on the ring.
    applyJumpSquash(this.root, pose);
    this.root.position.y += (0.5 + 0.5 * Math.sin(t * 1.3)) * FLOAT_BOB * sl * motion;
    this.root.rotation.z = Math.sin(t * 0.9) * FLOAT_ROCK * sl * motion;

    const breath = breathRate(pose);
    this.bodyG.scale.setScalar(1 + pose.puff * 0.08);
    this.body.scale.y = BODY[1] * (1 - sl * 0.06) + Math.sin(t * breath) * BREATH_BODY;
    this.lean.rotation.x = -sl * SLEEP_LEAN;

    // The head lies back with its chin up instead of drooping forward.
    const headG = this.headG;
    headG.position.y = HEAD_Y - sl * SLEEP_DROP + Math.sin(t * breath + BREATH_HEAD_PHASE) * BREATH_HEAD;
    headG.position.z = HEAD_Z - sl * SLEEP_BACK;
    applyHeadTurn(headG, pose, t);
    headG.rotation.x -= sl * SLEEP_CHIN_UP;

    for (const e of this.ears) {
      e.pivot.rotation.z = -e.s * (0.1 + (1 - pose.ear) * 0.6) + pose.earTw * (e.s < 0 ? 1 : 0.4) * 0.35;
      e.pivot.scale.setScalar(1 + pose.puff * 0.2);
    }

    this.face.apply(pose);

    // Chewing asleep: the snout and the grass work up and down. Impatient: the grass flicks like a toothpick.
    const chew = Math.sin(t * CHEW) * sl * motion;
    this.snout.scale.y = SNOUT[1] * (1 + chew * 0.05);
    const flick = pose.tapping ? tapBeat(t) * awake : 0;
    // Worked up (tailUp), the grass cocks upwards like a cigar.
    this.grass.rotation.z = GRASS_LEAN + chew * 0.18 + flick * 0.5 + pose.puff * 0.4 + pose.tailUp * 0.3 * awake;
    this.grass.rotation.x = -0.3 + chew * 0.1;

    // The right front foot taps while impatient.
    this.legs[1].rotation.x = pose.tapping ? -tapBeat(t) * 0.35 * awake : 0;

    // The ring puffs up round the waist as it settles in, and rocks when poked.
    const inflate = smooth(RING_FROM, RING_TO, sl);
    this.ring.visible = inflate > 0.02;
    const rock = Math.sin(t * 5.2) * pose.swish * 0.09;
    this.ring.rotation.set(rock, 0, rock * 0.6);
    this.ring.scale.setScalar(Math.max(0.02, inflate) * (1 + pose.puff * 0.06));

    this.fx.apply(pose, t);
  }

  headScreen(camera: THREE.Camera, width: number, height: number, out: { x: number; y: number }): void {
    projectToScreen(this.headG, camera, width, height, out);
  }

  dispose(): void {
    this.kit.dispose();
  }
}
