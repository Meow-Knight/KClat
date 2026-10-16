// The Klat face: what makes a character recognisably Klat whatever its species (spec §5.3).
// Eyes with four looks (open, asleep ∪, happy ∩, knocked out ✕), ink brows, the "ω" mouth that opens into a
// dark oval, blush, and an optional muzzle and nose. A character only places it on its head (FaceLayout);
// what is inside each eye and how expressions map onto it stay fixed.
import * as THREE from 'three';
import type { CatPose } from './pose';
import { HIGHLIGHT, MOUTH, PUPIL, type CharacterPalette } from './palette';
import { OUTLINE, type ToonBuilder, type V3 } from './ToonBuilder';

/** Head-local placement of the face. Parts given for the right side (+x) are mirrored to the left. */
export interface FaceLayout {
  /** Centre of the right eye. */
  eye: V3;
  /** How far each eye turns outwards around y, following the head's curve. */
  eyeYaw: number;
  /** Uniform size of each eye (default 1). Keeps everything inside the eye in proportion. */
  eyeScale?: number;
  /** Centre of the closed "ω" mouth. */
  mouth: V3;
  /** Centre of the open mouth (the dark oval), just below the "ω". */
  mouthOpen: V3;
  /** Right blush oval. */
  blush: { p: V3; s: V3 };
  /** Right muzzle puff, in palette.light. Omit for a face without a muzzle. */
  muzzle?: { p: V3; s: V3 };
  /** Nose, in palette.accent. Omit for none. */
  nose?: { p: V3; s: V3 };
}

interface Eye {
  s: number;
  iris: THREE.Group;
  pupil: THREE.Group;
  sleepArc: THREE.Mesh;
  happyArc: THREE.Mesh;
  brow: THREE.Mesh;
  cross: THREE.Group;
}

export class Face {
  private readonly eyes: Eye[] = [];
  private readonly mouthW = new THREE.Group();
  private readonly mouthO: THREE.Mesh;

  constructor(kit: ToonBuilder, head: THREE.Object3D, palette: CharacterPalette, layout: FaceLayout) {
    const SPH = kit.sphere;

    for (const s of [-1, 1]) {
      if (layout.muzzle) {
        const { p, s: sc } = layout.muzzle;
        kit.mk(SPH, palette.light, head, { p: [s * p[0], p[1], p[2]], s: sc, ol: OUTLINE.muzzle });
      }
      const { p, s: sc } = layout.blush;
      kit.mk(SPH, palette.blush, head, { p: [s * p[0], p[1], p[2]], s: sc, ol: OUTLINE.none });
    }
    if (layout.nose) kit.mk(SPH, palette.accent, head, { ...layout.nose, ol: OUTLINE.nose });

    // Mouth: "ω" when closed, an open oval when yelling
    const [mx, my, mz] = layout.mouth;
    const arcSmall = kit.geo(new THREE.TorusGeometry(0.07, 0.02, 6, 12, Math.PI));
    head.add(this.mouthW);
    for (const s of [-1, 1]) {
      const half = new THREE.Mesh(arcSmall, kit.inkMat);
      half.position.set(mx + s * 0.07, my, mz);
      half.rotation.z = Math.PI;
      this.mouthW.add(half);
    }
    this.mouthO = kit.mk(SPH, MOUTH, head, { p: layout.mouthOpen, s: [0.13, 0.12, 0.06], ol: OUTLINE.none });

    const arc = kit.geo(new THREE.TorusGeometry(0.19, 0.035, 8, 20, Math.PI));
    const capsule = kit.geo(new THREE.CapsuleGeometry(0.035, 0.2, 4, 8));
    const pupilMat = kit.mat(new THREE.MeshBasicMaterial({ color: PUPIL }));
    const whiteMat = kit.mat(new THREE.MeshBasicMaterial({ color: HIGHLIGHT }));
    const [ex, ey, ez] = layout.eye;
    for (const s of [-1, 1]) {
      const eg = new THREE.Group();
      eg.position.set(s * ex, ey, ez);
      eg.rotation.y = s * layout.eyeYaw;
      if (layout.eyeScale !== undefined) eg.scale.setScalar(layout.eyeScale);
      head.add(eg);

      const iris = new THREE.Group();
      eg.add(iris);
      kit.mk(SPH, palette.iris, iris, { s: [0.24, 0.27, 0.12], ol: OUTLINE.eye });
      const pupil = new THREE.Group();
      iris.add(pupil);
      kit.mk(SPH, pupilMat, pupil, { p: [0, 0, 0.06], s: [0.17, 0.21, 0.08] });
      // Two highlights, the same on both eyes: a big one top-left, a small one bottom-right.
      kit.mk(SPH, whiteMat, pupil, { p: [-0.07, 0.09, 0.13], s: [0.06, 0.06, 0.03] });
      kit.mk(SPH, whiteMat, pupil, { p: [0.06, -0.08, 0.13], s: [0.03, 0.03, 0.02] });

      const sleepArc = new THREE.Mesh(arc, kit.inkMat); // ∪
      sleepArc.rotation.z = Math.PI;
      sleepArc.position.z = 0.1;
      eg.add(sleepArc);
      const happyArc = new THREE.Mesh(arc, kit.inkMat); // ∩
      happyArc.position.set(0, -0.08, 0.1);
      eg.add(happyArc);
      const brow = new THREE.Mesh(capsule, kit.inkMat);
      brow.position.z = 0.06;
      eg.add(brow);
      const cross = new THREE.Group(); // ✕
      cross.position.z = 0.1;
      cross.visible = false;
      for (const r of [Math.PI / 4, -Math.PI / 4]) {
        const bar = new THREE.Mesh(capsule, kit.inkMat);
        bar.rotation.z = r;
        bar.scale.set(1.3, 1.25, 1);
        cross.add(bar);
      }
      eg.add(cross);
      this.eyes.push({ s, iris, pupil, sleepArc, happyArc, brow, cross });
    }
  }

  /** Eyes, brows and mouth for this frame. Allocation-free. */
  apply(pose: CatPose): void {
    const awake = 1 - pose.sleepy;
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
  }

  /** True while the mouth is open (for parts that come out of it, like a tongue). */
  get mouthOpen(): boolean {
    return this.mouthO.visible;
  }
}
