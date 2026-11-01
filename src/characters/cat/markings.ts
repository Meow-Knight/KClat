// Flat markings laid on an ellipsoid's surface (tabby stripes, patches), for the cat breeds. A marking is a thin
// ellipsoid decal half sunk into the surface, turned to face out along the surface normal.
import * as THREE from 'three';
import { OUTLINE, type ToonBuilder, type V3 } from '../../kit/ToonBuilder';

const Z_AXIS = new THREE.Vector3(0, 0, 1);

export interface Surface {
  /** Centre of the ellipsoid in the parent's space. */
  center: V3;
  radii: V3;
}

export interface Marking {
  /** Around the ellipsoid from the front (+z): positive towards +x, π/2 = the right side. */
  theta: number;
  /** Height above the ellipsoid's centre. */
  y: number;
  /** Size: x across the surface, y along it, z the (thin) depth. */
  s: V3;
  /** Turn around the surface normal; positive leans the top towards -x. */
  roll?: number;
}

/** Lays `markings` on `surface` in `color`, without an outline (DNA: decals have none). */
export function addMarkings(kit: ToonBuilder, color: number, parent: THREE.Object3D, surface: Surface, markings: readonly Marking[], ol: number = OUTLINE.none): THREE.Mesh[] {
  const [cx, cy, cz] = surface.center;
  const [a, b, c] = surface.radii;
  const n = new THREE.Vector3();
  const roll = new THREE.Quaternion();
  return markings.map((m) => {
    const k = Math.sqrt(Math.max(0, 1 - (m.y / b) ** 2));
    const x = a * k * Math.sin(m.theta);
    const z = c * k * Math.cos(m.theta);
    n.set(x / (a * a), m.y / (b * b), z / (c * c)).normalize();
    const mesh = kit.mk(kit.sphere, color, parent, { s: m.s, ol });
    mesh.position.set(cx + x, cy + m.y, cz + z).addScaledVector(n, -m.s[2] * 0.3);
    mesh.quaternion.setFromUnitVectors(Z_AXIS, n).multiply(roll.setFromAxisAngle(Z_AXIS, m.roll ?? 0));
    return mesh;
  });
}

/** The angle `theta` for a point at head-local `x` on the front of an ellipsoid, at height `y`. */
export function thetaAt(surface: Surface, x: number, y: number): number {
  const k = Math.sqrt(Math.max(0, 1 - (y / surface.radii[1]) ** 2));
  return Math.asin(Math.max(-1, Math.min(1, x / (surface.radii[0] * k))));
}
