// What the stage needs from a character's rig. CatController (the shared brain) drives every rig with the
// same CatPose; a rig only decides how its body shows it.
import * as THREE from 'three';
import type { CatPose } from './pose';

export interface CharacterRig {
  /** Add this to the scene. */
  readonly object: THREE.Object3D;
  /**
   * Generous world-space boxes around the character in each pose, for the overlay's click hitbox (spec §9).
   * Static on purpose: the hitbox is sent to Rust only when the layout changes, not every frame.
   */
  readonly hitBounds: Readonly<Record<'awake' | 'asleep', THREE.Box3>>;
  /** `t` is seconds, for breathing, tapping and sway. Allocation-free. */
  applyPose(pose: CatPose, t: number): void;
  /** Head centre in CSS px relative to the canvas. Writes into `out`. */
  headScreen(camera: THREE.Camera, width: number, height: number, out: { x: number; y: number }): void;
  dispose(): void;
}

const probe = new THREE.Vector3();

/** headScreen() for a rig whose head is `head`. */
export function projectToScreen(
  head: THREE.Object3D,
  camera: THREE.Camera,
  width: number,
  height: number,
  out: { x: number; y: number },
): void {
  head.getWorldPosition(probe).project(camera);
  out.x = ((probe.x + 1) / 2) * width;
  out.y = ((1 - probe.y) / 2) * height;
}
