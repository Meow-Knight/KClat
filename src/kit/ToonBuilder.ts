// Builds meshes the Klat way (spec §5.1): primitives with 3-band toon shading and an ink inverted-hull outline.
// Owns every geometry, material and texture it hands out, so one dispose() frees a whole character.
import * as THREE from 'three';
import { createOutlineMaterial, createToonGradient } from './materials';
import { INK } from './palette';

export type V3 = readonly [number, number, number];

export interface MeshOpts {
  p?: V3;
  s?: V3;
  r?: V3;
  /** Outline hull scale; 0 = no outline. Default OUTLINE.body. */
  ol?: number;
}

/**
 * Outline hull scales. The hull scales with the mesh, so small parts need a bigger factor for the line to read
 * as thick as the body's. Flat decals (belly, blush, markings, inner ears) get no outline at all.
 */
export const OUTLINE = {
  body: 1.06,
  muzzle: 1.08,
  bump: 1.1,
  eye: 1.12,
  nose: 1.18,
  tip: 1.25,
  none: 0,
} as const;

export class ToonBuilder {
  /** Unit sphere: most parts are this, scaled into an ellipsoid. */
  readonly sphere: THREE.SphereGeometry;
  readonly outlineMat = createOutlineMaterial();
  /** Unlit ink, for lines: brows, whiskers, closed eyes, the "ω" mouth. */
  readonly inkMat = new THREE.MeshBasicMaterial({ color: INK });

  private readonly gradient = createToonGradient();
  private readonly toonMats = new Map<number, THREE.MeshToonMaterial>();
  private readonly materials: THREE.Material[] = [this.outlineMat, this.inkMat];
  private readonly geometries: THREE.BufferGeometry[] = [];
  private readonly textures: THREE.Texture[] = [];

  constructor() {
    this.sphere = this.geo(new THREE.SphereGeometry(1, 40, 28));
  }

  /** A mesh in a toon colour (outlined unless `ol` is 0), or in a given material (never outlined). */
  mk(geo: THREE.BufferGeometry, color: number | THREE.Material, parent: THREE.Object3D, o: MeshOpts = {}): THREE.Mesh {
    const toon = typeof color === 'number';
    const m = new THREE.Mesh(geo, toon ? this.toon(color) : color);
    if (o.p) m.position.set(...o.p);
    if (o.s) m.scale.set(...o.s);
    if (o.r) m.rotation.set(...o.r);
    if (toon && o.ol !== 0) {
      const hull = new THREE.Mesh(geo, this.outlineMat);
      hull.scale.setScalar(o.ol ?? OUTLINE.body);
      m.add(hull);
    }
    parent.add(m);
    return m;
  }

  /** One toon material per colour, shared across meshes. */
  toon(color: number): THREE.MeshToonMaterial {
    let m = this.toonMats.get(color);
    if (!m) {
      m = this.mat(new THREE.MeshToonMaterial({ color, gradientMap: this.gradient }));
      this.toonMats.set(color, m);
    }
    return m;
  }

  /** A sprite drawn on top of everything (Zzz, marks, stars), hidden until the pose shows it. */
  sprite(map: THREE.Texture, parent: THREE.Object3D): THREE.Sprite {
    const s = new THREE.Sprite(this.mat(new THREE.SpriteMaterial({ map, transparent: true, depthTest: false, depthWrite: false })));
    s.renderOrder = 10;
    s.visible = false;
    parent.add(s);
    return s;
  }

  mat<M extends THREE.Material>(m: M): M {
    this.materials.push(m);
    return m;
  }

  geo<G extends THREE.BufferGeometry>(g: G): G {
    this.geometries.push(g);
    return g;
  }

  tex<T extends THREE.Texture>(t: T): T {
    this.textures.push(t);
    return t;
  }

  dispose(): void {
    for (const t of this.textures) t.dispose();
    for (const g of this.geometries) g.dispose();
    for (const m of this.materials) m.dispose();
    this.gradient.dispose();
  }
}
