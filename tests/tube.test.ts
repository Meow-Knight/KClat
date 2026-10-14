import { describe, expect, it } from 'vitest';
import * as THREE from 'three';
import { ToonTube } from '../src/kit/tube';

type Pts = [number, number, number][];
const AWAKE: Pts = [[0.55, 0.45, -0.75], [1.25, 0.55, -0.7], [1.62, 1.15, -0.55], [1.45, 1.8, -0.45]];
const ASLEEP: Pts = [[0.55, 0.35, -0.7], [1.15, 0.27, -0.1], [0.9, 0.22, 0.7], [0.25, 0.2, 1.05]];
const REPEATED: Pts = [[0, 0, 0], [0, 0, 0], [1, 1, 0], [1, 1, 0]];

function makeTail(pts: Pts = AWAKE): ToonTube {
  const tail = new ToonTube(new THREE.MeshBasicMaterial(), new THREE.MeshBasicMaterial());
  pts.forEach((p, i) => tail.points[i].set(...p));
  return tail;
}

describe('ToonTube', () => {
  it('reuses the same buffers across updates', () => {
    const tail = makeTail();
    tail.update(0.155, 0.195);
    const pos = tail.mesh.geometry.attributes.position;
    const arr = pos.array;
    tail.points[3].x += 0.5;
    tail.update(0.2, 0.25);
    expect(tail.mesh.geometry.attributes.position).toBe(pos);
    expect(tail.mesh.geometry.attributes.position.array).toBe(arr);
  });

  it.each([
    ['awake', AWAKE],
    ['asleep', ASLEEP],
    ['repeated points', REPEATED],
  ] as const)('%s: every ring vertex sits at the radius around THREE.CatmullRomCurve3', (_name, pts) => {
    const tail = makeTail(pts);
    tail.update(0.155, 0.195);
    const curve = new THREE.CatmullRomCurve3(tail.points);
    const check = (geo: THREE.BufferGeometry, radius: number): void => {
      const pos = geo.attributes.position;
      const ring = 11;
      const rings = pos.count / ring;
      const c = new THREE.Vector3();
      const v = new THREE.Vector3();
      for (let j = 0; j < rings; j++) {
        curve.getPoint(j / (rings - 1), c);
        for (let i = 0; i < ring; i++) {
          v.fromBufferAttribute(pos, j * ring + i);
          expect(v.distanceTo(c)).toBeCloseTo(radius, 4);
        }
      }
    };
    check(tail.mesh.geometry, 0.155);
    check(tail.outline.geometry, 0.195);
  });

  it('normals are unit length and point away from the centreline', () => {
    const tail = makeTail();
    tail.update(0.155, 0.195);
    const nor = tail.mesh.geometry.attributes.normal;
    const n = new THREE.Vector3();
    for (let k = 0; k < nor.count; k++) {
      n.fromBufferAttribute(nor, k);
      expect(n.length()).toBeCloseTo(1, 4);
    }
  });

  it('triangles wind counter-clockwise seen from outside (front faces face out)', () => {
    const tail = makeTail();
    tail.update(0.155, 0.195);
    const geo = tail.mesh.geometry;
    const pos = geo.attributes.position;
    const nor = geo.attributes.normal;
    const idx = geo.index!;
    const [a, b, c, n, face] = [new THREE.Vector3(), new THREE.Vector3(), new THREE.Vector3(), new THREE.Vector3(), new THREE.Vector3()];
    for (let f = 0; f < idx.count; f += 3) {
      a.fromBufferAttribute(pos, idx.getX(f));
      b.fromBufferAttribute(pos, idx.getX(f + 1));
      c.fromBufferAttribute(pos, idx.getX(f + 2));
      face.crossVectors(b.sub(a), c.sub(a));
      n.fromBufferAttribute(nor, idx.getX(f));
      expect(face.dot(n)).toBeGreaterThan(0);
    }
  });

  it('flags the buffers for upload and is never frustum-culled', () => {
    const tail = makeTail();
    const pos = tail.mesh.geometry.attributes.position as THREE.BufferAttribute;
    const before = pos.version;
    tail.update(0.155, 0.195);
    expect(pos.version).toBeGreaterThan(before);
    expect(tail.mesh.frustumCulled).toBe(false);
    expect(tail.outline.frustumCulled).toBe(false);
  });
});
