// The tabby, "mèo mướp" (docs/characters/tabby/sheet.md): the cat rig in brown-grey with dark mackerel stripes
// (an "M" on the forehead, cheek stripes, bands down the sides and thighs, a ringed tail with a dark tip), bigger
// ears on a leaner alley-cat build, and a crossed plaster on the temple from its last fight.
import * as THREE from 'three';
import type { CharacterPalette } from '../../kit/palette';
import { OUTLINE } from '../../kit/ToonBuilder';
import { CAT_FACE, CAT_FX, CatRig, ROOT_X, type CatLook } from '../cat/CatRig';
import { addMarkings, thetaAt, type Marking, type Surface } from '../cat/markings';

export const TABBY_PALETTE: CharacterPalette = {
  main: 0xb39e80,
  shade: 0x6b5844,
  light: 0xf6eedf,
  accent: 0xe08a7b,
  iris: 0x9bc83a,
  blush: 0xf4b0a2,
};

const HEAD: Surface = { center: [0, 0, 0], radii: [1.27, 1, 1.05] };
const BODY: Surface = { center: [0, 0.95, 0], radii: [1.1, 0.95, 0.98] };
const THIGH = (s: number): Surface => ({ center: [s * 0.78, 0.42, 0.25], radii: [0.45, 0.4, 0.55] });

/** The forehead "M": outer legs splay out, inner strokes meet in a V. */
const M: Marking[] = [
  [-0.3, 0.7, 0.17, -0.3],
  [-0.1, 0.67, 0.15, 0.5],
  [0.1, 0.67, 0.15, -0.5],
  [0.3, 0.7, 0.17, 0.3],
].map(([x, y, len, roll]) => ({ theta: thetaAt(HEAD, x, y), y, s: [0.055, len, 0.05], roll }));

const both = (f: (s: number) => Marking[]): Marking[] => [...f(1), ...f(-1)];

const CHEEKS = both((s) => [
  { theta: s * 1.05, y: 0.03, s: [0.22, 0.035, 0.05], roll: s * 0.18 },
  { theta: s * 1.1, y: -0.12, s: [0.2, 0.035, 0.05], roll: s * 0.05 },
]);

const SIDES = both((s) => [1.0, 1.45, 1.9, 2.35].map((th) => ({ theta: s * th, y: 0.22, s: [0.08, 0.42, 0.06], roll: s * 0.1 })));

const thighStripes = (s: number): Marking[] => [1.25, 1.75].map((th) => ({ theta: s * th, y: 0.06, s: [0.06, 0.27, 0.05] }));

export const TABBY_LOOK: CatLook = {
  palette: TABBY_PALETTE,
  body: BODY.radii,
  head: HEAD.radii,
  face: CAT_FACE,
  fx: CAT_FX,
  hitBounds: {
    awake: new THREE.Box3(new THREE.Vector3(ROOT_X - 1.45, 0, -1), new THREE.Vector3(ROOT_X + 2.35, 3.7, 1.3)),
    asleep: new THREE.Box3(new THREE.Vector3(ROOT_X - 1.45, 0, -1), new THREE.Vector3(ROOT_X + 1.35, 3.0, 1.4)),
  },
  ears: { pivot: [0.7, 0.7, 0], size: 1.12, tilt: 0.28 },
  tail: {
    awake: [[0.55, 0.45, -0.75], [1.25, 0.55, -0.7], [1.62, 1.15, -0.55], [1.45, 1.8, -0.45]],
    asleep: [[0.55, 0.35, -0.7], [1.15, 0.27, -0.1], [0.9, 0.22, 0.7], [0.25, 0.2, 1.05]],
    radius: 0.15,
    rings: [0.3, 0.48, 0.66, 0.84],
  },
  decorate({ kit, palette, head, body }) {
    addMarkings(kit, palette.shade, head, HEAD, [...M, ...CHEEKS]);
    addMarkings(kit, palette.shade, body, BODY, SIDES);
    for (const s of [-1, 1]) addMarkings(kit, palette.shade, body, THIGH(s), thighStripes(s));

    // Crossed plaster on the right temple, above the eye and clear of the brow.
    const plaster = kit.geo(new THREE.CapsuleGeometry(0.065, 0.3, 4, 10));
    const theta = 0.82;
    const y = 0.5;
    const k = Math.sqrt(1 - y * y);
    const [a, , c] = HEAD.radii;
    const n = new THREE.Vector3((a * k * Math.sin(theta)) / (a * a), y, (c * k * Math.cos(theta)) / (c * c)).normalize();
    const patch = new THREE.Group();
    patch.position.set(a * k * Math.sin(theta), y, c * k * Math.cos(theta)).addScaledVector(n, 0.01);
    patch.quaternion.setFromUnitVectors(new THREE.Vector3(0, 0, 1), n);
    head.add(patch);
    for (const roll of [0.75, -0.75]) {
      kit.mk(plaster, palette.light, patch, { s: [1, 1, 0.35], r: [0, 0, roll], ol: OUTLINE.muzzle });
    }
  },
};

export class TabbyRig extends CatRig {
  constructor() {
    super(TABBY_LOOK);
  }
}
