// Motion every Klat character shares (spec §6.3): jump with squash & stretch, breathing, and how the head
// turns, droops and tilts. A character applies these to its own root and head; species parts do the rest.
import type * as THREE from 'three';
import type { CatPose } from '../cat/CatController';

/** Jump height and volume-preserving squash. The root's origin is at the feet, so squashing never sinks. */
export function applyJumpSquash(root: THREE.Object3D, pose: CatPose): void {
  const sq = pose.squash;
  root.position.y = pose.jumpY;
  root.scale.set(1 + sq * 0.55, 1 - sq, 1 + sq * 0.55);
}

/** Breathing in rad/s: slow asleep, quicker awake. The body swells by BREATH_BODY, the head bobs by BREATH_HEAD. */
export function breathRate(pose: CatPose): number {
  return pose.sleepy > 0.5 ? 1.4 : 2.4;
}
export const BREATH_BODY = 0.014;
export const BREATH_HEAD = 0.012;
/** The head bobs a little after the body. */
export const BREATH_HEAD_PHASE = -0.6;

/** Head rotation: follows the cursor while awake, droops asleep, tips back on a yawn, tilts and shivers when petted. */
export function applyHeadTurn(head: THREE.Object3D, pose: CatPose, t: number): void {
  const sl = pose.sleepy;
  const awake = 1 - sl;
  head.rotation.set(
    pose.ly * awake + sl * 0.3 - pose.yawn * 0.38,
    pose.lx * awake,
    sl * 0.18 + pose.happy * 0.2 + (pose.purring ? Math.sin(t * 70) * 0.014 : 0),
  );
}

/** The impatient beat for `tapping` (a paw, a tongue, a tail tip): 0..1, about 1.6 beats a second. */
export function tapBeat(t: number): number {
  return Math.max(0, Math.sin(t * 10));
}
