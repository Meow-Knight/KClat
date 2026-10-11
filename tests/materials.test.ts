import { describe, expect, it } from 'vitest';
import * as THREE from 'three';
import { createToonGradient, toonGradientData } from '../src/cat/materials';

describe('toon gradient', () => {
  it('encodes the three spec bands as opaque grey texels', () => {
    expect(Array.from(toonGradientData())).toEqual([95, 95, 95, 255, 180, 180, 180, 255, 255, 255, 255, 255]);
  });

  it('uses nearest filtering so bands stay hard', () => {
    const tex = createToonGradient();
    expect(tex.image.width).toBe(3);
    expect(tex.minFilter).toBe(THREE.NearestFilter);
    expect(tex.magFilter).toBe(THREE.NearestFilter);
  });
});
