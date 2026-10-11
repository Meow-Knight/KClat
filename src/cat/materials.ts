import * as THREE from 'three';
import { INK } from './palette';

// 3-step toon ramp (spec §5.1): one RGBA texel per band, nearest-filtered so the bands stay hard.
export const TOON_STEPS = [95, 180, 255] as const;

export function toonGradientData(): Uint8Array {
  const data = new Uint8Array(TOON_STEPS.length * 4);
  TOON_STEPS.forEach((v, i) => data.set([v, v, v, 255], i * 4));
  return data;
}

export function createToonGradient(): THREE.DataTexture {
  const tex = new THREE.DataTexture(toonGradientData(), TOON_STEPS.length, 1);
  tex.minFilter = THREE.NearestFilter;
  tex.magFilter = THREE.NearestFilter;
  tex.needsUpdate = true;
  return tex;
}

// Inverted-hull outline: render the back faces of a slightly larger clone in ink.
export function createOutlineMaterial(): THREE.MeshBasicMaterial {
  return new THREE.MeshBasicMaterial({ color: INK, side: THREE.BackSide });
}
