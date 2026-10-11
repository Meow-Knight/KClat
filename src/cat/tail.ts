// Tail tube that is allocated once and rewritten in place every frame (spec §6.3).
// The prototype rebuilt a TubeGeometry per frame (~12 MB/s of garbage). Here the centreline, frames and
// vertices all live in preallocated typed arrays, and the spline is evaluated inline: THREE.CatmullRomCurve3
// boxes a float per coefficient on every getPoint(), which was the largest allocation left in the frame.
import * as THREE from 'three';

const TUBULAR = 24;
const RADIAL = 10;
const RING = RADIAL + 1;
const RINGS = TUBULAR + 1;
const SEGMENTS = 3; // 4 control points, open curve

function tubeIndex(): number[] {
  const index: number[] = [];
  for (let j = 1; j <= TUBULAR; j++) {
    for (let i = 1; i <= RADIAL; i++) {
      const a = RING * (j - 1) + (i - 1);
      const b = RING * j + (i - 1);
      const c = RING * j + i;
      const d = RING * (j - 1) + i;
      index.push(a, b, d, b, c, d);
    }
  }
  return index;
}

function tubeGeometry(withNormals: boolean): THREE.BufferGeometry {
  const geo = new THREE.BufferGeometry();
  const count = RINGS * RING;
  geo.setAttribute('position', new THREE.BufferAttribute(new Float32Array(count * 3), 3).setUsage(THREE.DynamicDrawUsage));
  if (withNormals) {
    geo.setAttribute('normal', new THREE.BufferAttribute(new Float32Array(count * 3), 3).setUsage(THREE.DynamicDrawUsage));
  }
  geo.setIndex(tubeIndex());
  return geo;
}

export class TailTube {
  /** The 4 control points; write them, then call update(). */
  readonly points: THREE.Vector3[] = [new THREE.Vector3(), new THREE.Vector3(), new THREE.Vector3(), new THREE.Vector3()];
  readonly mesh: THREE.Mesh;
  readonly outline: THREE.Mesh;

  /** Control points with the two extrapolated ends: [p(-1), p0, p1, p2, p3, p(4)] × xyz. */
  private readonly ctrl = new Float64Array(6 * 3);
  /** Cubic coefficients per segment and axis: [seg][axis][c0..c3]. */
  private readonly coef = new Float64Array(SEGMENTS * 3 * 4);
  private readonly center = new Float64Array(RINGS * 3);
  private readonly tangent = new Float64Array(RINGS * 3);
  private readonly normal = new Float64Array(RINGS * 3);
  private readonly binormal = new Float64Array(RINGS * 3);
  private readonly cos = new Float64Array(RING);
  private readonly sin = new Float64Array(RING);

  constructor(material: THREE.Material, outlineMaterial: THREE.Material) {
    this.mesh = new THREE.Mesh(tubeGeometry(true), material);
    this.outline = new THREE.Mesh(tubeGeometry(false), outlineMaterial);
    // The tube moves every frame; a stale bounding sphere would cull it.
    this.mesh.frustumCulled = false;
    this.outline.frustumCulled = false;
    // Same ring parameterisation as THREE.TubeGeometry (note -cos): it fixes the winding so front faces face out.
    for (let i = 0; i < RING; i++) {
      const a = (i / RADIAL) * Math.PI * 2;
      this.cos[i] = -Math.cos(a);
      this.sin[i] = Math.sin(a);
    }
  }

  update(radius: number, outlineRadius: number): void {
    this.fitSpline();
    this.sampleCenterline();
    this.computeFrames();
    this.writeRing(this.mesh.geometry, radius, true);
    this.writeRing(this.outline.geometry, outlineRadius, false);
  }

  dispose(): void {
    this.mesh.geometry.dispose();
    this.outline.geometry.dispose();
  }

  /** Centripetal Catmull-Rom, matching THREE.CatmullRomCurve3 (open curve, ends extrapolated). */
  private fitSpline(): void {
    const c = this.ctrl;
    const pts = this.points;
    for (let i = 0; i < 4; i++) {
      c[(i + 1) * 3] = pts[i].x;
      c[(i + 1) * 3 + 1] = pts[i].y;
      c[(i + 1) * 3 + 2] = pts[i].z;
    }
    for (let a = 0; a < 3; a++) {
      c[a] = 2 * c[3 + a] - c[6 + a];
      c[15 + a] = 2 * c[12 + a] - c[9 + a];
    }
    for (let s = 0; s < SEGMENTS; s++) {
      // Segment s runs from control point s to s+1; its neighbours sit at ctrl indices s..s+3.
      const i0 = s * 3;
      const i1 = i0 + 3;
      const i2 = i0 + 6;
      const i3 = i0 + 9;
      let dt0 = Math.pow(distSq(c, i0, i1), 0.25);
      let dt1 = Math.pow(distSq(c, i1, i2), 0.25);
      let dt2 = Math.pow(distSq(c, i2, i3), 0.25);
      if (dt1 < 1e-4) dt1 = 1;
      if (dt0 < 1e-4) dt0 = dt1;
      if (dt2 < 1e-4) dt2 = dt1;
      for (let a = 0; a < 3; a++) {
        const x0 = c[i0 + a];
        const x1 = c[i1 + a];
        const x2 = c[i2 + a];
        const x3 = c[i3 + a];
        const t1 = ((x1 - x0) / dt0 - (x2 - x0) / (dt0 + dt1) + (x2 - x1) / dt1) * dt1;
        const t2 = ((x2 - x1) / dt1 - (x3 - x1) / (dt1 + dt2) + (x3 - x2) / dt2) * dt1;
        const k = (s * 3 + a) * 4;
        this.coef[k] = x1;
        this.coef[k + 1] = t1;
        this.coef[k + 2] = -3 * x1 + 3 * x2 - 2 * t1 - t2;
        this.coef[k + 3] = 2 * x1 - 2 * x2 + t1 + t2;
      }
    }
  }

  /** Uniform in curve parameter (like CatmullRomCurve3.getPoint), not arc length. */
  private sampleCenterline(): void {
    for (let j = 0; j < RINGS; j++) {
      const p = (j / TUBULAR) * SEGMENTS;
      let seg = Math.floor(p);
      let w = p - seg;
      if (seg >= SEGMENTS) {
        seg = SEGMENTS - 1;
        w = 1;
      }
      for (let a = 0; a < 3; a++) {
        const k = (seg * 3 + a) * 4;
        const cf = this.coef;
        this.center[j * 3 + a] = cf[k] + w * (cf[k + 1] + w * (cf[k + 2] + w * cf[k + 3]));
      }
    }
  }

  /** Tangents by central differences, normals by parallel transport (projection) from a fixed reference. */
  private computeFrames(): void {
    const { center: c, tangent: t, normal: n, binormal: b } = this;
    for (let j = 0; j < RINGS; j++) {
      const p = Math.max(0, j - 1) * 3;
      const q = Math.min(TUBULAR, j + 1) * 3;
      const o = j * 3;
      t[o] = c[q] - c[p];
      t[o + 1] = c[q + 1] - c[p + 1];
      t[o + 2] = c[q + 2] - c[p + 2];
      normalize(t, o);
    }
    // Start from world Z (or Y when the tail points along Z) so the seam does not jump between frames.
    const useY = Math.abs(t[2]) > 0.9;
    n[0] = 0;
    n[1] = useY ? 1 : 0;
    n[2] = useY ? 0 : 1;
    projectOut(n, 0, n, 0, t, 0);
    for (let j = 1; j < RINGS; j++) projectOut(n, j * 3, n, (j - 1) * 3, t, j * 3);
    for (let j = 0; j < RINGS; j++) {
      const o = j * 3;
      b[o] = t[o + 1] * n[o + 2] - t[o + 2] * n[o + 1];
      b[o + 1] = t[o + 2] * n[o] - t[o] * n[o + 2];
      b[o + 2] = t[o] * n[o + 1] - t[o + 1] * n[o];
    }
  }

  private writeRing(geo: THREE.BufferGeometry, radius: number, withNormals: boolean): void {
    const pos = geo.attributes.position as THREE.BufferAttribute;
    const nor = withNormals ? (geo.attributes.normal as THREE.BufferAttribute) : null;
    const pa = pos.array as Float32Array;
    const na = nor ? (nor.array as Float32Array) : null;
    const { center: c, normal: n, binormal: b } = this;
    for (let j = 0; j < RINGS; j++) {
      const o = j * 3;
      for (let i = 0; i < RING; i++) {
        const cs = this.cos[i];
        const sn = this.sin[i];
        const nx = cs * n[o] + sn * b[o];
        const ny = cs * n[o + 1] + sn * b[o + 1];
        const nz = cs * n[o + 2] + sn * b[o + 2];
        const k = (j * RING + i) * 3;
        pa[k] = c[o] + radius * nx;
        pa[k + 1] = c[o + 1] + radius * ny;
        pa[k + 2] = c[o + 2] + radius * nz;
        if (na) {
          na[k] = nx;
          na[k + 1] = ny;
          na[k + 2] = nz;
        }
      }
    }
    pos.needsUpdate = true;
    if (nor) nor.needsUpdate = true;
  }
}

function distSq(c: Float64Array, i: number, j: number): number {
  const dx = c[i] - c[j];
  const dy = c[i + 1] - c[j + 1];
  const dz = c[i + 2] - c[j + 2];
  return dx * dx + dy * dy + dz * dz;
}

function normalize(v: Float64Array, o: number): void {
  const len = Math.sqrt(v[o] * v[o] + v[o + 1] * v[o + 1] + v[o + 2] * v[o + 2]) || 1;
  v[o] /= len;
  v[o + 1] /= len;
  v[o + 2] /= len;
}

/** out = normalize(v - t·(v·t)), all as xyz triples at the given offsets. */
function projectOut(out: Float64Array, oo: number, v: Float64Array, vo: number, t: Float64Array, to: number): void {
  const d = v[vo] * t[to] + v[vo + 1] * t[to + 1] + v[vo + 2] * t[to + 2];
  out[oo] = v[vo] - t[to] * d;
  out[oo + 1] = v[vo + 1] - t[to + 1] * d;
  out[oo + 2] = v[vo + 2] - t[to + 2] * d;
  normalize(out, oo);
}
