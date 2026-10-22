// The slime (docs/characters/slime/sheet.md): a squishy blob of glossy gel, drawn in its own style (gel.ts) instead
// of the Klat toon and ink. It melts into a puddle while Claude works, pulls itself back together and bounces when
// you are needed, and jiggles when it lands, when it is petted or poked. Its face is painted on the gel by the
// shader, so it squashes and wobbles with the body. Two droplets on top play the ears; the round arm on its right
// plays the tail (pumps when worked up) and the paw (taps while it waits).
import * as THREE from 'three';
import type { CatPose } from '../../kit/pose';
import { projectToScreen, type CharacterRig } from '../../kit/CharacterRig';
import { CharacterFx, type FxLayout } from '../../kit/fx';
import { applyJumpSquash, BREATH_BODY, breathRate, tapBeat } from '../../kit/motion';
import type { CharacterPalette } from '../../kit/palette';
import { ToonBuilder } from '../../kit/ToonBuilder';
import { createGelMaterials, FACE_LAT, gelHeight, gelRadii, latToT, type GelMaterials, type GelSkin } from './gel';

/** The teal slime. Another skin is another GelSkin and palette; the shape and motion stay. */
export const SLIME_SKIN: GelSkin = {
  top: 0x4ff0e2,
  mid: 0x1bbfd0,
  bottom: 0x2a86de,
  base: 0x8166f2,
  glint: 0xd8fffb,
  line: 0xeafffd,
  eyeDark: 0x1a1f4e,
  eyeLight: 0x4a6cf0,
  blush: 0x8fa2ff,
  mouth: 0x8a2142,
  tongue: 0xff8098,
};

/** For the shared effects: teal bump, blue Zzz. */
export const SLIME_PALETTE: CharacterPalette = {
  main: 0x2fc6cf,
  shade: 0x1d6fb8,
  light: 0xeafffd,
  accent: 0xff8098,
  iris: 0x1a1f4e,
  blush: 0x8fa2ff,
};

const ROOT_X = 1.3;

/** Idle jiggle, so the gel never looks solid; reactions add to it, up to JIGGLE_MAX. */
const JIGGLE_IDLE = 0.25;
const JIGGLE_MAX = 1.6;
/** Hops while worked up (tailUp), in world units. */
const HOP = 0.16;
/** How far the face slides over the gel with the look direction (sphere radians). */
const LOOK: readonly [number, number] = [0.22, 0.14];

const HIT_BOUNDS = {
  awake: new THREE.Box3(new THREE.Vector3(ROOT_X - 1.9, 0, -1.3), new THREE.Vector3(ROOT_X + 2.0, 2.75, 1.5)),
  asleep: new THREE.Box3(new THREE.Vector3(ROOT_X - 2.1, 0, -1.6), new THREE.Vector3(ROOT_X + 2.1, 1.3, 1.9)),
} as const;

/** Head-local here means relative to the top of the gel (the head group sits there). */
const FX: FxLayout = {
  ground: [ROOT_X, 0.01, 0],
  shadowRadius: 1.6,
  auraCenter: [ROOT_X, 2.5, -1.35],
  auraSize: [5.0, 6.0],
  zzz: [2.0, 1.3, 0.3],
  anger: [0.8, -0.3, 0.55],
  hit: [0.05, -0.05, 0.15],
  starOrbit: [0.95, 0.15, 0.6],
  bump: [0.12, -0.08, 0.2],
};

/** The round arms: [height fraction, angle from the right towards the viewer, radius]. */
const ARM_R = [0.24, 0.6, 0.38] as const;
const ARM_L = [0.3, Math.PI - 0.45, 0.26] as const;
/** Droplets on top (the ears): [x, drop below the top, radius]. */
const DROPS = [
  [0.32, 0.05, 0.13],
  [0.6, 0.2, 0.085],
] as const;

const radii: [number, number] = [0, 0];

export class SlimeRig implements CharacterRig {
  readonly object = new THREE.Group();
  readonly hitBounds = HIT_BOUNDS;

  private readonly kit = new ToonBuilder();
  private readonly gel: GelMaterials;
  private readonly root = new THREE.Group();
  /** At the top of the gel: the effects' anchor. */
  private readonly headG = new THREE.Group();
  /** On the face, for the speech bubble. */
  private readonly faceAnchor = new THREE.Object3D();
  private readonly armR: THREE.Mesh;
  private readonly armL: THREE.Mesh;
  private readonly drops: THREE.Mesh[] = [];
  private readonly fx: CharacterFx;

  constructor() {
    const kit = this.kit;
    const gel = createGelMaterials(SLIME_SKIN);
    this.gel = gel;
    for (const m of [gel.body, gel.bodyLine, gel.part, gel.partLine]) kit.mat(m);

    this.root.position.x = ROOT_X;
    this.root.rotation.y = -0.22;
    this.object.add(this.root);

    // Body: a finely divided sphere, bent into shape by the gel shader. Never culled: the shader moves it.
    const bodyGeo = kit.geo(new THREE.SphereGeometry(1, 96, 72));
    const body = new THREE.Mesh(bodyGeo, gel.body);
    const line = new THREE.Mesh(bodyGeo, gel.bodyLine);
    body.frustumCulled = line.frustumCulled = false;
    this.root.add(body, line);

    const part = (r: number): THREE.Mesh => {
      const m = new THREE.Mesh(kit.sphere, gel.part);
      m.add(new THREE.Mesh(kit.sphere, gel.partLine));
      m.scale.setScalar(r);
      this.root.add(m);
      return m;
    };
    this.armR = part(ARM_R[2]);
    this.armL = part(ARM_L[2]);
    for (const [, , r] of DROPS) this.drops.push(part(r));

    this.root.add(this.headG, this.faceAnchor);
    this.fx = new CharacterFx(kit, this.object, this.headG, SLIME_PALETTE, FX);
  }

  applyPose(pose: CatPose, t: number): void {
    const sl = pose.sleepy;
    const awake = 1 - sl;
    // Decorative motion (idle wobble, hops) stops under reduceMotion.
    const motion = pose.auraFlicker ? 1 : 0;

    applyJumpSquash(this.root, pose);
    // Worked up, it can't sit still: little hops.
    this.root.position.y += Math.abs(Math.sin(t * 7)) * HOP * pose.tailUp * awake * motion;

    // Melt into a puddle asleep; breathing is a slow swell of the puddle or the blob.
    const melt = sl;
    const puff = pose.puff + Math.sin(t * breathRate(pose)) * BREATH_BODY * 6;
    const jiggle = Math.min(
      JIGGLE_MAX,
      JIGGLE_IDLE * motion +
        Math.abs(pose.squash) * 3 +
        pose.puff * 1.2 +
        pose.happy * (pose.purring ? 1 : 0.4) +
        pose.swish * 1.2 +
        pose.annoyed * 0.5 +
        (pose.tapping ? 0.4 : 0),
    );
    const sway = pose.lx * 0.3 * awake + pose.earTw * 0.08;
    const u = this.gel.uniforms;
    u.uTime.value = t;
    u.uMelt.value = melt;
    u.uPuff.value = puff;
    u.uJiggle.value = jiggle;
    u.uSway.value = sway;

    // The face, painted by the shader: same rules as the Klat face (kit/face.ts) for which eyes show.
    u.uEyes.value.set(pose.eye, pose.happy > 0.5 ? 1 : 0, pose.dizzy > 0.5 ? 1 : 0, awake);
    u.uMouth.value.set(pose.mouth, pose.grumpy);
    u.uLook.value.set(pose.lx * LOOK[0] * awake, -pose.ly * LOOK[1] * awake + pose.yawn * 0.05);

    const h = gelHeight(melt, puff, jiggle, t);
    this.headG.position.set(sway, h, 0);
    const faceT = latToT(FACE_LAT);
    this.faceAnchor.position.set(sway * faceT * faceT, faceT * h, gelRadii(faceT, melt, puff, radii)[1]);

    // Arms: soaked back into the puddle while melted. The right one pumps when worked up and taps while waiting.
    const absorb = Math.max(0, 1 - melt * 1.4);
    const place = (m: THREE.Mesh, [ft, ang, r]: readonly [number, number, number], lift: number) => {
      const [side, depth] = gelRadii(ft, melt, puff, radii);
      m.position.set(Math.cos(ang) * side * 0.93 + sway * ft * ft, ft * h + lift, Math.sin(ang) * depth * 0.93);
      m.scale.setScalar(r * absorb * (1 + pose.puff * 0.15));
      m.visible = absorb > 0.02;
    };
    const tap = pose.tapping ? tapBeat(t) * 0.18 * awake : 0;
    place(this.armR, ARM_R, pose.tailUp * (0.3 + Math.sin(t * 8) * 0.06) * awake + tap + Math.sin(t * 5.2) * pose.swish * 0.06);
    place(this.armL, ARM_L, Math.sin(t * 5.2 + 1) * pose.swish * 0.05);

    // Droplets (the ears): perk up when alert, sink into the gel when not, bob on a twitch.
    for (let i = 0; i < 2; i++) {
      const [x, below, r] = DROPS[i];
      const d = this.drops[i];
      d.position.set(x + sway, h - below - 0.14 + pose.ear * 0.16 + pose.earTw * 0.06 * (i === 0 ? 1 : -1), 0.08);
      d.scale.setScalar(r * Math.max(0, 1 - melt * 1.2) * (1 + pose.puff * 0.25));
      d.visible = d.scale.x > 0.005;
    }

    this.fx.apply(pose, t);
  }

  headScreen(camera: THREE.Camera, width: number, height: number, out: { x: number; y: number }): void {
    projectToScreen(this.faceAnchor, camera, width, height, out);
  }

  dispose(): void {
    this.kit.dispose();
  }
}
