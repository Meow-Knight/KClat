// The cat (spec §5), ported 1:1 from docs/prototype/cat-prototype.html. The face, effects and shared motion
// come from the character kit (src/kit); this file builds and moves what only cats have: body, head, whiskers,
// ears and tail. Breeds of cat (british/, tabby/) are the same rig with another CatLook: proportions, colours,
// tail, and their own markings.
import * as THREE from 'three';
import type { CatPose } from '../../kit/pose';
import { CAT_PALETTE } from './palette';
import { projectToScreen, type CharacterRig } from '../../kit/CharacterRig';
import { Face, type FaceLayout } from '../../kit/face';
import { CharacterFx, type FxLayout } from '../../kit/fx';
import { applyHeadTurn, applyJumpSquash, BREATH_BODY, BREATH_HEAD, BREATH_HEAD_PHASE, breathRate, tapBeat } from '../../kit/motion';
import type { CharacterPalette } from '../../kit/palette';
import { OUTLINE, ToonBuilder, type V3 } from '../../kit/ToonBuilder';
import { ToonTube } from '../../kit/tube';

export const ROOT_X = 1.3;
const HEAD_Y = 2.05;
const HEAD_Z = 0.1;

/** The groups a breed's markings and extras hang from. */
export interface CatParts {
  kit: ToonBuilder;
  palette: CharacterPalette;
  /** Head-local (origin at the head's centre). */
  head: THREE.Group;
  /** Body, belly and thighs; puffs up with the body. */
  body: THREE.Group;
}

/** What makes one cat look unlike another. Everything else (the face, motion, effects) is shared. */
export interface CatLook {
  palette: CharacterPalette;
  /** Body ellipsoid radii (cat: 1.15, 0.95, 1). */
  body: V3;
  /** Head ellipsoid radii (cat: 1.3, 1, 1.05). */
  head: V3;
  face: FaceLayout;
  fx: FxLayout;
  hitBounds: Readonly<Record<'awake' | 'asleep', THREE.Box3>>;
  ears: {
    /** Right ear's pivot, head-local. */
    pivot: V3;
    /** Uniform size of the ear (cat: 1). */
    size: number;
    /** Outward tilt when alert; folding down adds 0.5 (cat: 0.3). */
    tilt: number;
  };
  tail: {
    /** Control points awake, and curled around the paws asleep. */
    awake: readonly V3[];
    asleep: readonly V3[];
    radius: number;
    /** Colour of the round tip. Default palette.shade. */
    tip?: number;
    /** Rings in palette.shade at these curve parameters (0 = base, 1 = tip). */
    rings?: readonly number[];
  };
  /** Markings and extras, added right after the head (before the face, so the face draws over them). */
  decorate?(parts: CatParts): void;
}

export const CAT_FACE: FaceLayout = {
  eye: [0.45, 0.08, 0.9],
  eyeYaw: 0.32,
  mouth: [0, -0.33, 1.1],
  mouthOpen: [0, -0.42, 1.06],
  blush: { p: [0.78, -0.18, 0.8], s: [0.17, 0.08, 0.05] },
  muzzle: { p: [0.22, -0.32, 0.9], s: [0.3, 0.22, 0.22] },
  nose: { p: [0, -0.17, 1.05], s: [0.1, 0.07, 0.06] },
};

/**
 * The aura's quad spans world y -0.45..6.25, which uses the stage's headroom above the cat. The Zzz drift up
 * from beside the head; the angry mark sits between the right ear and the brow.
 */
export const CAT_FX: FxLayout = {
  ground: [ROOT_X, 0.01, 0],
  shadowRadius: 1.4,
  auraCenter: [ROOT_X, 2.9, -1.35],
  auraSize: [4.6, 6.7],
  zzz: [2.1, 2.6, 0.3],
  anger: [0.86, 0.55, 1.0],
  hit: [0.05, 0.95, 0.25],
  starOrbit: [0.95, 1.12, 0.55],
  bump: [0.18, 0.9, 0.38],
};

/** The grey cat: three stripes on the forehead and a dark tail tip. */
export const CAT_LOOK: CatLook = {
  palette: CAT_PALETTE,
  body: [1.15, 0.95, 1],
  head: [1.3, 1, 1.05],
  face: CAT_FACE,
  fx: CAT_FX,
  hitBounds: {
    awake: new THREE.Box3(new THREE.Vector3(ROOT_X - 1.45, 0, -1), new THREE.Vector3(ROOT_X + 2.35, 3.55, 1.3)),
    asleep: new THREE.Box3(new THREE.Vector3(ROOT_X - 1.45, 0, -1), new THREE.Vector3(ROOT_X + 1.35, 2.95, 1.4)),
  },
  ears: { pivot: [0.72, 0.68, 0], size: 1, tilt: 0.3 },
  tail: {
    awake: [[0.55, 0.45, -0.75], [1.25, 0.55, -0.7], [1.62, 1.15, -0.55], [1.45, 1.8, -0.45]],
    asleep: [[0.55, 0.35, -0.7], [1.15, 0.27, -0.1], [0.9, 0.22, 0.7], [0.25, 0.2, 1.05]],
    radius: 0.155,
  },
  decorate({ kit, palette, head }) {
    for (const [x, y, z, rz] of [[0, 0.78, 0.64, 0], [-0.26, 0.72, 0.68, 0.25], [0.26, 0.72, 0.68, -0.25]]) {
      kit.mk(kit.sphere, palette.shade, head, { p: [x, y, z], s: [0.06, 0.17, 0.05], r: [0, 0, rz], ol: OUTLINE.none });
    }
  },
};

/** Tail-ring torus at radius 1 (scaled to the tail every frame): a band ~0.11 wide along the tail. */
const RING_TUBE = 0.2;
const Z_AXIS = new THREE.Vector3(0, 0, 1);

const lerp = (a: number, b: number, t: number): number => a + (b - a) * t;

interface Ear {
  s: number;
  pivot: THREE.Group;
}

export class CatRig implements CharacterRig {
  readonly object = new THREE.Group();
  readonly headG = new THREE.Group();
  readonly hitBounds: Readonly<Record<'awake' | 'asleep', THREE.Box3>>;

  private readonly kit = new ToonBuilder();
  private readonly root = new THREE.Group();
  private readonly bodyG = new THREE.Group();
  private readonly body: THREE.Mesh;
  private readonly pawR: THREE.Mesh;
  private readonly tail: ToonTube;
  private readonly tip: THREE.Mesh;
  private readonly rings: THREE.Mesh[] = [];
  private readonly ears: Ear[] = [];
  private readonly face: Face;
  private readonly fx: CharacterFx;
  private readonly ringPoint = new THREE.Vector3();
  private readonly ringTangent = new THREE.Vector3();

  constructor(private readonly look: CatLook = CAT_LOOK) {
    const kit = this.kit;
    const SPH = kit.sphere;
    const P = look.palette;
    this.hitBounds = look.hitBounds;

    this.root.position.x = ROOT_X;
    this.root.rotation.y = -0.22;
    this.object.add(this.root);

    // Body
    this.root.add(this.bodyG);
    this.body = kit.mk(SPH, P.main, this.bodyG, { p: [0, 0.95, 0], s: look.body });
    kit.mk(SPH, P.light, this.bodyG, { p: [0, 0.85, 0.6], s: [0.68, 0.6, 0.5], ol: OUTLINE.none });
    kit.mk(SPH, P.main, this.bodyG, { p: [-0.78, 0.42, 0.25], s: [0.45, 0.4, 0.55] });
    kit.mk(SPH, P.main, this.bodyG, { p: [0.78, 0.42, 0.25], s: [0.45, 0.4, 0.55] });
    kit.mk(SPH, P.light, this.root, { p: [-0.42, 0.17, 0.9], s: [0.3, 0.2, 0.34] });
    this.pawR = kit.mk(SPH, P.light, this.root, { p: [0.42, 0.17, 0.9], s: [0.3, 0.2, 0.34] });

    // Head: markings, the Klat face, whiskers
    const headG = this.headG;
    headG.position.set(0, HEAD_Y, HEAD_Z);
    this.root.add(headG);
    kit.mk(SPH, P.main, headG, { s: look.head });
    look.decorate?.({ kit, palette: P, head: headG, body: this.bodyG });
    this.face = new Face(kit, headG, P, look.face);
    const whisker = kit.geo(new THREE.CylinderGeometry(0.012, 0.012, 0.6));
    for (const s of [-1, 1]) {
      for (const [y, tilt] of [[-0.26, 0.08], [-0.38, -0.06]]) {
        const m = new THREE.Mesh(whisker, kit.inkMat);
        m.position.set(s * (look.head[0] - 0.64), y, 0.86);
        m.rotation.set(0, s * -0.5, Math.PI / 2 + s * tilt);
        headG.add(m);
      }
    }

    // Ears
    const earCone = kit.geo(new THREE.ConeGeometry(0.36, 0.72, 32));
    const earInner = kit.geo(new THREE.ConeGeometry(0.2, 0.42, 32));
    const [ex, ey, ez] = look.ears.pivot;
    for (const s of [-1, 1]) {
      const pivot = new THREE.Group();
      pivot.position.set(s * ex, ey, ez);
      headG.add(pivot);
      const ear = new THREE.Group();
      ear.scale.setScalar(look.ears.size);
      pivot.add(ear);
      kit.mk(earCone, P.main, ear, { p: [0, 0.3, 0], s: [1, 1, 0.55] });
      kit.mk(earInner, P.accent, ear, { p: [0, 0.22, 0.13], s: [1, 1, 0.3], ol: OUTLINE.none });
      this.ears.push({ s, pivot });
    }

    // Tail
    this.tail = new ToonTube(kit.toon(P.main), kit.outlineMat);
    this.root.add(this.tail.mesh, this.tail.outline);
    this.tip = kit.mk(SPH, look.tail.tip ?? P.shade, this.root, { s: [0.165, 0.165, 0.165], ol: OUTLINE.tip });
    if (look.tail.rings) {
      const ring = kit.geo(new THREE.TorusGeometry(1, RING_TUBE, 6, 24));
      for (let i = 0; i < look.tail.rings.length; i++) this.rings.push(kit.mk(ring, P.shade, this.root, { ol: OUTLINE.none }));
    }

    this.fx = new CharacterFx(kit, this.object, headG, P, look.fx);
  }

  /** Port of the prototype's frame(). */
  applyPose(pose: CatPose, t: number): void {
    const look = this.look;
    const sl = pose.sleepy;
    const awake = 1 - sl;

    applyJumpSquash(this.root, pose);
    const breath = breathRate(pose);
    this.bodyG.scale.setScalar(1 + pose.puff * 0.08);
    this.body.scale.y = look.body[1] * (1 - sl * 0.1) + Math.sin(t * breath) * BREATH_BODY;

    const headG = this.headG;
    headG.position.y = HEAD_Y - sl * 0.4 + Math.sin(t * breath + BREATH_HEAD_PHASE) * BREATH_HEAD;
    headG.position.z = HEAD_Z + sl * 0.18;
    applyHeadTurn(headG, pose, t);

    const tilt = look.ears.tilt;
    for (const e of this.ears) {
      e.pivot.rotation.z = -e.s * (tilt + (1 - pose.ear) * 0.5) + pose.earTw * (e.s < 0 ? 1 : 0.4) * 0.3;
      e.pivot.scale.y = 1 + pose.puff * 0.15;
    }

    this.face.apply(pose);

    this.pawR.position.y = 0.17 + (pose.tapping ? tapBeat(t) * 0.17 : 0) * awake;

    const asleep = sl > 0.5;
    // Asleep, a poke adds a quicker swish on top of the slow sway. Fixed frequency, so changing its amplitude
    // never makes the tail jump.
    const sway = asleep
      ? Math.sin(t * 1.1) * 0.12 + Math.sin(t * 5.2) * pose.swish * 0.75
      : Math.sin(t * (2.6 + pose.tailUp * 2.5)) * (0.5 + pose.tailUp * 0.35);
    const pts = this.tail.points;
    for (let i = 0; i < 4; i++) {
      const a = look.tail.awake[i];
      const z = look.tail.asleep[i];
      const f = i / 3;
      pts[i].set(
        lerp(a[0] + sway * f * f * 0.9, z[0] + sway * f * 0.3, sl),
        lerp(a[1] + pose.tailUp * f * 0.35, z[1] + pose.swish * f * f * 0.22, sl),
        lerp(a[2], z[2], sl),
      );
    }
    const radius = look.tail.radius + pose.puff * 0.06;
    this.tail.update(radius, radius + 0.04);
    this.tip.position.copy(pts[3]);

    const rings = look.tail.rings;
    if (rings) {
      // The ring's outer edge stays just inside the outline (radius + 0.04), so the tail's ink line is unbroken.
      const k = (radius + 0.032) / (1 + RING_TUBE);
      for (let i = 0; i < this.rings.length; i++) {
        const m = this.rings[i];
        this.tail.sample(rings[i], this.ringPoint, this.ringTangent);
        m.position.copy(this.ringPoint);
        m.quaternion.setFromUnitVectors(Z_AXIS, this.ringTangent);
        m.scale.set(k, k, 0.28);
      }
    }

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
