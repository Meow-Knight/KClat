// The white British Shorthair (docs/characters/british/sheet.md): the cat rig with a rounder, wider head, small
// round ears set wide apart, a chunkier body, odd eyes (one copper, one blue) and a long plush tail that stands
// tall like a question mark awake and wraps all the way around the paws asleep.
import * as THREE from 'three';
import type { CharacterPalette } from '../../kit/palette';
import { CAT_FX, CatRig, ROOT_X, type CatLook } from '../cat/CatRig';

const FUR = 0xe1e4ea;

export const BRITISH_PALETTE: CharacterPalette = {
  main: FUR,
  shade: 0xa4abbb,
  light: 0xfbf2e6,
  accent: 0xf4a1b3,
  iris: 0xf08c2e,
  irisLeft: 0x4aa6f0,
  blush: 0xf8bccb,
};

export const BRITISH_LOOK: CatLook = {
  palette: BRITISH_PALETTE,
  body: [1.22, 0.95, 1.04],
  head: [1.42, 1.02, 1.08],
  face: {
    eye: [0.5, 0.06, 0.93],
    eyeYaw: 0.34,
    eyeScale: 1.1,
    mouth: [0, -0.35, 1.15],
    mouthOpen: [0, -0.45, 1.11],
    blush: { p: [0.88, -0.2, 0.8], s: [0.18, 0.08, 0.05] },
    muzzle: { p: [0.24, -0.34, 0.93], s: [0.33, 0.25, 0.24] },
    nose: { p: [0, -0.18, 1.1], s: [0.1, 0.07, 0.06] },
  },
  fx: {
    ...CAT_FX,
    shadowRadius: 1.5,
    auraCenter: [ROOT_X + 0.1, 2.9, -1.35],
    auraSize: [5, 6.7],
    zzz: [2.25, 2.6, 0.3],
    anger: [0.95, 0.55, 1.0],
    starOrbit: [1.05, 1.12, 0.55],
    bump: [0.2, 0.93, 0.38],
  },
  hitBounds: {
    awake: new THREE.Box3(new THREE.Vector3(ROOT_X - 1.6, 0, -1), new THREE.Vector3(ROOT_X + 3.0, 3.9, 1.3)),
    asleep: new THREE.Box3(new THREE.Vector3(ROOT_X - 1.6, 0, -1), new THREE.Vector3(ROOT_X + 1.6, 2.95, 1.55)),
  },
  ears: { pivot: [0.84, 0.66, -0.05], size: 0.84, tilt: 0.48 },
  tail: {
    awake: [[0.55, 0.45, -0.75], [1.7, 0.55, -0.7], [1.95, 1.85, -0.55], [2.3, 3.0, -0.45]],
    asleep: [[0.55, 0.35, -0.7], [1.4, 0.27, 0.05], [0.8, 0.24, 1.2], [-0.55, 0.22, 1.25]],
    radius: 0.17,
    tip: FUR,
  },
};

export class BritishRig extends CatRig {
  constructor() {
    super(BRITISH_LOOK);
  }
}
