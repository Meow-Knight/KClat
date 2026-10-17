// Effects every Klat character gets: the ground shadow, the Super Saiyan aura, floating Zzz, the anime angry
// mark, and the knockout (a thrown object, the impact burst, stars circling the head, a bump).
// A character only says where they go around its body (FxLayout); their look stays fixed.
import * as THREE from 'three';
import { KNOCKOUT_IMPACT_S, KNOCKOUT_OBJECTS, type CatPose, type KnockoutObject } from './pose';
import { SHADOW_OPACITY, type CharacterPalette } from './palette';
import { angerTexture, burstTexture, glyphTexture, starTexture } from './textures';
import { OUTLINE, type ToonBuilder, type V3 } from './ToonBuilder';

/** Where the effects sit. World units unless marked head-local; for scale, the cat's head is about 2.6 wide. */
export interface FxLayout {
  /** Ground point under the character (the shadow's centre). */
  ground: V3;
  shadowRadius: number;
  /** Aura quad behind the character: centre and [width, height]. */
  auraCenter: V3;
  auraSize: readonly [number, number];
  /** Where the Zzz start drifting up from. */
  zzz: V3;
  /** Head-local: the angry mark, at a top corner of the head. */
  anger: V3;
  /** Head-local: where thrown objects land (the top of the head, a little towards the viewer). */
  hit: V3;
  /** Head-local: radii [x, y, z] of the stars' orbit (y is the orbit's height). */
  starOrbit: V3;
  /** Head-local: the bump left by the hit. */
  bump: V3;
}

/**
 * Super Saiyan aura while awake: sharp energy spikes shooting up from an egg-shaped outline around the
 * character (longest above the head, shorter down the shoulders), in two walls (a tall pale outer one and a
 * bright inner one), with rising streaks, a soft halo and white lightning crackling around the body. Spike
 * heights jump at ~14 steps a second, like hand-drawn anime frames. One shader quad behind the character (one
 * extra draw call), hidden entirely while asleep; it writes premultiplied colour for the transparent window.
 * uMotion = 0 (reduceMotion) freezes it and turns the lightning off.
 */
const AURA_VERTEX = /* glsl */ `
varying vec2 vUv;
void main() {
  vUv = uv;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

const AURA_FRAGMENT = /* glsl */ `
uniform float uTime;
uniform float uIntensity;
uniform float uMotion;
varying vec2 vUv;

float hash1(float n) { return fract(sin(n * 127.1) * 43758.5453); }

// 0..~1: random height per spike plus a pointed tip.
float spike(float x, float count, float seed, float tick) {
  float s = x * count;
  float i = floor(s);
  float u = fract(s);
  float h = hash1(i * 1.37 + seed + tick * 7.13);
  float tip = 1.0 - abs(u - 0.5) * 2.0;
  return h * 0.45 + tip * (0.3 + 0.25 * h);
}

// One wall of spikes rising from an ellipse (centre cy, radii rx, ry). Returns (fill, halo).
vec2 wall(vec2 p, float rx, float ry, float cy, float count, float seed, float tick, float reach, float grow) {
  float xr = p.x / rx;
  if (abs(xr) >= 1.0) return vec2(0.0);
  float arc = sqrt(1.0 - xr * xr);
  float base = cy + ry * arc;
  float floorY = cy - ry * arc;
  float len = (0.04 + reach * pow(arc, 1.6)) * (0.3 + spike(p.x, count, seed, tick)) * grow;
  float top = base + len;
  float lower = smoothstep(floorY, floorY + 0.12, p.y);
  float fill = smoothstep(0.0, 0.01, top - p.y) * lower;
  float halo = smoothstep(0.07, 0.0, p.y - top) * (1.0 - fill) * lower;
  return vec2(fill, halo);
}

float segDist(vec2 p, vec2 a, vec2 b) {
  vec2 pa = p - a;
  vec2 ba = b - a;
  float h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
  return length(pa - ba * h);
}

void main() {
  vec2 p = vUv * 2.0 - 1.0;                          // -1..1, y up
  float tick = floor(uTime * 14.0) * uMotion;
  float grow = 0.82 + 0.28 * min(uIntensity, 1.5);   // flares make the spikes longer

  vec2 outer = wall(p, 0.9, 0.62, -0.32, 17.0, 3.0, tick, 0.62, grow);
  vec2 inner = wall(p, 0.7, 0.56, -0.34, 26.0, 11.0, tick + 0.5, 0.42, grow);

  // Vertical streaks rushing upwards.
  float column = floor((p.x * 0.5 + 0.5) * 36.0);
  float stripe = 0.72 + 0.28 * sin(p.x * 50.0 + hash1(column) * 6.2832);
  float rise = 0.8 + 0.2 * sin(p.y * 18.0 - uTime * 16.0 * uMotion + hash1(column + 9.0) * 6.2832);

  vec3 pale = vec3(1.0, 0.95, 0.42);
  vec3 gold = vec3(1.0, 0.84, 0.16);
  vec3 white = vec3(1.0, 0.99, 0.86);
  vec3 col = mix(pale, gold, inner.x);
  float a = outer.x * 0.42 * stripe * rise + inner.x * 0.62 * rise + (outer.y + inner.y) * 0.18;
  // White-hot core along the inner wall's spikes.
  col = mix(col, white, inner.x * 0.35 * stripe);

  // Lightning: up to two short zigzag arcs around the body, re-rolled 9 times a second.
  float bolt = 0.0;
  for (int k = 0; k < 2; k++) {
    float seed = floor(uTime * 9.0) + float(k) * 17.0;
    if (uMotion < 0.5 || hash1(seed) < 0.35) continue;
    float ang = hash1(seed + 1.0) * 6.2832;
    vec2 start = vec2(0.0, -0.34) + vec2(cos(ang) * 0.62, sin(ang) * 0.5);
    vec2 dir = normalize(vec2(cos(ang + 1.2), sin(ang + 1.2)));
    vec2 nrm = vec2(-dir.y, dir.x);
    vec2 prev = start;
    for (int j = 1; j <= 5; j++) {
      vec2 next = start + dir * (0.055 * float(j)) + nrm * (hash1(seed + float(j) * 3.1) - 0.5) * 0.11;
      float d = segDist(p, prev, next);
      bolt = max(bolt, smoothstep(0.01, 0.0, d) + 0.45 * smoothstep(0.04, 0.0, d));
      prev = next;
    }
  }
  bolt = min(bolt, 1.0);
  col = mix(col, white, bolt);
  a = max(a, bolt * 0.95);

  a = clamp(a * min(uIntensity, 1.3), 0.0, 1.0);
  gl_FragColor = vec4(col * a, a);
}
`;

/**
 * Floating Zzz, drawn as sprites so they animate with the (capped) render loop. As a CSS animation they ran
 * at the display's refresh rate and cost more CPU than the sleeping cat itself.
 * Units: world; ~0.022 world units per CSS px at the overlay's framing.
 */
const ZZZ_PERIOD = 3.2;
const ZZZ_DRIFT: readonly [number, number] = [0.57, 1.1];
const ZZZ: readonly { glyph: string; size: number; dx: number; dy: number; delay: number }[] = [
  { glyph: 'z', size: 0.37, dx: 0, dy: 0, delay: 0 },
  { glyph: 'Z', size: 0.46, dx: 0.09, dy: 0.22, delay: 1.05 },
  { glyph: 'z', size: 0.29, dx: -0.04, dy: 0.09, delay: 2.1 },
];

const ANGER_SIZE = 0.46;

/** Knockout objects (you answered, so something hits the character), drawn from primitives in the toon style. */
const WOOD = 0x9c6b3e;
const WOOD_DARK = 0x6e4524;
const SLIPPER_SOLE = 0xf1eee6;
const SLIPPER_CELL = 0xd8d2c4;
const SLIPPER_STRAP = 0x3d8bfd;
const DURIAN = 0xa9c24b;
const DURIAN_SPIKE = 0x7e9937;
const PAN = 0x3b3f4c;
const PAN_INSIDE = 0x596071;
const PAN_HANDLE = 0x23252e;
const FISH = 0x7fa6c4;
const FISH_BELLY = 0xe9eef0;
const FISH_FIN = 0x5b84a6;
const DUCK = 0xffd23f;
const DUCK_WING = 0xf2b705;
const DUCK_BEAK = 0xff8c1a;
const MUG = 0xe8533f;
const COFFEE = 0x5a3a22;
const BREAD = 0xe3a24a;
const BREAD_CUT = 0xf6d68f;
const HERB = 0x5fb04a;
const STRAW = 0xe8d39a;
const STRAW_RIDGE = 0xc9ad6c;
const IRON = 0x4a4f5c;
const IRON_TOP = 0x737a8a;

/**
 * How each object flies. `throw`: an arc from the left, tumbling end over end. `drop`: falls from above onto the
 * head. `frisbee`: skims in flat from the left, spinning like a frisbee.
 */
type Flight = 'throw' | 'drop' | 'frisbee';
interface FlightSpec {
  flight: Flight;
  /** How far the centre sits above the hit point when it lands, so it rests on the head, not in it. */
  lift: number;
  /** Velocity (world units/s) as it bounces off. */
  bounce: V3;
  /** Tumble per frame after the hit (radians). */
  spin: number;
  /** `drop` only: how far it turns about the vertical on the way down (radians). */
  turn?: number;
}
const BOUNCE: V3 = [1.7, 3.2, 0.7];
const THROWN = (lift: number): FlightSpec => ({ flight: 'throw', lift, bounce: BOUNCE, spin: 0.22 });
const FLIGHT: Record<KnockoutObject, FlightSpec> = {
  stick: THROWN(0.12),
  slipper: THROWN(0.1),
  durian: { flight: 'drop', lift: 0.55, bounce: [1.1, 3.4, 0.6], spin: 0.05, turn: 2.4 },
  pan: THROWN(0.12),
  fish: THROWN(0.2),
  // Rubber: bounces off higher than anything else.
  duck: { flight: 'throw', lift: 0.3, bounce: [1.3, 4.6, 0.7], spin: 0.3 },
  mug: THROWN(0.25),
  banhmi: THROWN(0.2),
  nonla: { flight: 'frisbee', lift: 0.2, bounce: [2.4, 2.2, 0.6], spin: 0.35 },
  // Heavy: a dull thud, then it slides off. Barely turns, so the horn stays in profile.
  anvil: { flight: 'drop', lift: 0.42, bounce: [0.6, 1.2, 0.3], spin: 0.03, turn: -0.3 },
};
/** Thrown objects come from the left (where the bubble was), dropped ones from above. Offsets from the hit. */
const THROW_FROM: V3 = [-3.8, 2.2, 1.2];
const DROP_FROM: V3 = [0.15, 4.6, 0];
const THROW_ARC = 1.0;
const FRISBEE_ARC = 0.35;
/** After the hit: gravity on the bounce. */
const BOUNCE_GRAVITY = 13;
const BURST_S = 0.32;
const STARS = 3;
const STAR_SIZE = 0.3;
const BUMP_SIZE = 0.32;

export class CharacterFx {
  private readonly shadow: THREE.Mesh;
  private readonly aura: THREE.Mesh;
  private readonly auraUniforms = { uTime: { value: 0 }, uIntensity: { value: 0 }, uMotion: { value: 1 } };
  private readonly zzz: THREE.Sprite[] = [];
  private anger: THREE.Sprite | undefined;
  /** One group per KNOCKOUT_OBJECTS entry, in world space; at most one is visible. */
  private readonly projectiles: THREE.Group[] = [];
  private readonly bonkTarget = new THREE.Vector3();
  private readonly bonkHit = new THREE.Vector3();
  private bonkHitSet = false;
  private burst: THREE.Sprite | undefined;
  private readonly stars: THREE.Sprite[] = [];
  private readonly bump: THREE.Mesh;

  constructor(
    private readonly kit: ToonBuilder,
    /** The character's top-level group (world space). */
    private readonly world: THREE.Object3D,
    private readonly head: THREE.Object3D,
    palette: CharacterPalette,
    private readonly layout: FxLayout,
  ) {
    const shadowMat = kit.mat(new THREE.MeshBasicMaterial({ color: 0x000000, transparent: true, opacity: SHADOW_OPACITY }));
    this.shadow = new THREE.Mesh(kit.geo(new THREE.CircleGeometry(layout.shadowRadius, 40)), shadowMat);
    this.shadow.rotation.x = -Math.PI / 2;
    this.shadow.position.set(...layout.ground);
    world.add(this.shadow);

    const browser = typeof document !== 'undefined';
    if (browser) {
      const glyphs = new Map<string, THREE.Texture>();
      for (const z of ZZZ) {
        let map = glyphs.get(z.glyph);
        if (!map) {
          map = kit.tex(glyphTexture(z.glyph, palette.shade));
          glyphs.set(z.glyph, map);
        }
        this.zzz.push(kit.sprite(map, world));
      }
      this.anger = kit.sprite(kit.tex(angerTexture()), head);
      this.anger.position.set(...layout.anger);
    }

    this.bump = kit.mk(kit.sphere, palette.main, head, { p: layout.bump, s: [0, 0, 0], ol: OUTLINE.bump });
    kit.mk(kit.sphere, 0xffffff, this.bump, { p: [-0.3, 0.35, 0.55], s: [0.25, 0.2, 0.15], ol: OUTLINE.none });
    this.bump.visible = false;
    this.buildProjectiles();
    if (browser) {
      this.burst = kit.sprite(kit.tex(burstTexture()), world);
      const starMap = kit.tex(starTexture());
      for (let i = 0; i < STARS; i++) this.stars.push(kit.sprite(starMap, head));
    }

    // Aura, behind the character; drawn first among the transparent objects so the shadow disc lands on top.
    const auraMat = kit.mat(
      new THREE.ShaderMaterial({
        uniforms: this.auraUniforms,
        vertexShader: AURA_VERTEX,
        fragmentShader: AURA_FRAGMENT,
        transparent: true,
        depthWrite: false,
        blending: THREE.CustomBlending,
        blendSrc: THREE.OneFactor,
        blendDst: THREE.OneMinusSrcAlphaFactor,
      }),
    );
    this.aura = new THREE.Mesh(kit.geo(new THREE.PlaneGeometry(layout.auraSize[0], layout.auraSize[1])), auraMat);
    this.aura.position.set(...layout.auraCenter);
    this.aura.renderOrder = -1;
    this.aura.visible = false;
    world.add(this.aura);
  }

  /** `t` is seconds. Allocation-free. */
  apply(pose: CatPose, t: number): void {
    this.shadow.scale.setScalar(Math.max(0.5, 1 - pose.jumpY * 0.35));

    // Aura: follows the jump; flares and the reduceMotion freeze are handled in the shader.
    const aura = pose.aura;
    this.aura.visible = aura > 0.01;
    if (this.aura.visible) {
      this.auraUniforms.uIntensity.value = aura;
      this.auraUniforms.uMotion.value = pose.auraFlicker ? 1 : 0;
      if (pose.auraFlicker) this.auraUniforms.uTime.value = t;
      this.aura.position.y = this.layout.auraCenter[1] + pose.jumpY; // the shader grows the spikes on flares
    }

    // Angry mark: pops in on the spring's overshoot, throbs like a vein while shown (static under reduceMotion).
    const anger = this.anger;
    if (anger) {
      const a = pose.annoyed;
      anger.visible = a > 0.02;
      if (anger.visible) {
        const throb = pose.auraFlicker ? 1 + 0.1 * Math.max(0, Math.sin(t * 11)) : 1;
        anger.scale.setScalar(ANGER_SIZE * Math.max(0, a) * throb);
        (anger.material as THREE.SpriteMaterial).opacity = Math.min(1, a * 1.6);
      }
    }

    this.applyKnockout(pose, t);

    // Zzz: port of the prototype's CSS keyframes (fade in over 20%, drift up-right, grow .6 → 1.1, fade out).
    const anchor = this.layout.zzz;
    for (let i = 0; i < this.zzz.length; i++) {
      const sprite = this.zzz[i];
      sprite.visible = pose.zzz;
      if (!pose.zzz) continue;
      const z = ZZZ[i];
      const p = ((((t - z.delay) % ZZZ_PERIOD) + ZZZ_PERIOD) % ZZZ_PERIOD) / ZZZ_PERIOD;
      (sprite.material as THREE.SpriteMaterial).opacity = p < 0.2 ? p / 0.2 : 1 - (p - 0.2) / 0.8;
      sprite.position.set(anchor[0] + z.dx + ZZZ_DRIFT[0] * p, anchor[1] + z.dy + ZZZ_DRIFT[1] * p, anchor[2]);
      sprite.scale.setScalar(z.size * (0.6 + 0.5 * p));
    }
  }

  /** Flying object, impact burst, stars and bump. */
  private applyKnockout(pose: CatPose, t: number): void {
    const kind = pose.bonkKind;
    const flying = kind >= 0 && pose.bonkT >= 0;
    for (let i = 0; i < this.projectiles.length; i++) this.projectiles[i].visible = flying && i === kind;
    if (!flying) this.bonkHitSet = false;

    if (flying) {
      const obj = this.projectiles[kind];
      const spec = FLIGHT[KNOCKOUT_OBJECTS[kind]];
      const hit = this.layout.hit;
      this.head.localToWorld(this.bonkTarget.set(hit[0], hit[1] + spec.lift, hit[2]));
      const bt = pose.bonkT;
      if (bt < KNOCKOUT_IMPACT_S) {
        const u = bt / KNOCKOUT_IMPACT_S;
        if (spec.flight === 'drop') {
          // Dropped: falls faster and faster onto the head.
          const k = 1 - u * u;
          const from = DROP_FROM;
          obj.position.set(this.bonkTarget.x + from[0] * k, this.bonkTarget.y + from[1] * k, this.bonkTarget.z + from[2] * k);
          obj.rotation.set(0.3, u * (spec.turn ?? 0), 0.2);
        } else {
          const k = 1 - u;
          const from = THROW_FROM;
          const frisbee = spec.flight === 'frisbee';
          obj.position.set(
            this.bonkTarget.x + from[0] * k,
            this.bonkTarget.y + from[1] * k + Math.sin(Math.PI * u) * (frisbee ? FRISBEE_ARC : THROW_ARC),
            this.bonkTarget.z + from[2] * k,
          );
          // A frisbee stays flat (tilted towards the viewer) and spins about its axis; the rest tumble end over end.
          if (frisbee) obj.rotation.set(0.4, u * Math.PI * 6, -0.15);
          else obj.rotation.set(0.25, 0.4, 1.2 - u * Math.PI * 3.5);
        }
      } else {
        if (!this.bonkHitSet) {
          this.bonkHit.copy(this.bonkTarget);
          this.bonkHitSet = true;
        }
        const v = bt - KNOCKOUT_IMPACT_S;
        const b = spec.bounce;
        obj.position.set(
          this.bonkHit.x + b[0] * v,
          this.bonkHit.y + b[1] * v - 0.5 * BOUNCE_GRAVITY * v * v,
          this.bonkHit.z + b[2] * v,
        );
        if (spec.flight === 'frisbee') {
          obj.rotation.y += spec.spin;
          obj.rotation.z -= 0.03;
        } else {
          obj.rotation.z -= spec.spin;
          obj.rotation.x += 0.08;
        }
      }
    }

    const burst = this.burst;
    if (burst) {
      const v = flying ? pose.bonkT - KNOCKOUT_IMPACT_S : -1;
      burst.visible = v >= 0 && v < BURST_S;
      if (burst.visible) {
        burst.position.copy(this.bonkHit);
        burst.scale.setScalar(0.5 + v * 4.5);
        (burst.material as THREE.SpriteMaterial).opacity = 1 - v / BURST_S;
      }
    }

    // Stars orbit the head while it is dizzy (they hold still under reduceMotion).
    const d = pose.dizzy;
    const orbit = this.layout.starOrbit;
    for (let i = 0; i < this.stars.length; i++) {
      const star = this.stars[i];
      star.visible = d > 0.05;
      if (!star.visible) continue;
      const a = (pose.auraFlicker ? t * 5 : 0.6) + (i * Math.PI * 2) / STARS;
      star.position.set(Math.cos(a) * orbit[0], orbit[1] + Math.sin(a) * 0.12, Math.sin(a) * orbit[2]);
      star.scale.setScalar(STAR_SIZE * Math.min(1, d));
      star.material.rotation = pose.auraFlicker ? t * 3 + i : 0;
    }

    const bump = Math.max(0, pose.bump);
    this.bump.visible = bump > 0.02;
    if (this.bump.visible) this.bump.scale.set(BUMP_SIZE * bump, BUMP_SIZE * 1.1 * bump, BUMP_SIZE * bump);
  }

  /** One group per KNOCKOUT_OBJECTS entry, in that order. */
  private buildProjectiles(): void {
    const kit = this.kit;
    const SPH = kit.sphere;
    const builders: Record<KnockoutObject, (g: THREE.Group) => void> = {
      // Stick: a knotted branch.
      stick: (g) => {
        kit.mk(kit.geo(new THREE.CylinderGeometry(0.075, 0.095, 1.7, 10)), WOOD, g, { r: [0, 0, Math.PI / 2] });
        kit.mk(SPH, WOOD_DARK, g, { p: [0.35, 0.06, 0.02], s: [0.06, 0.05, 0.05], ol: 0 });
        kit.mk(SPH, WOOD_DARK, g, { p: [-0.45, -0.05, 0.03], s: [0.05, 0.045, 0.05], ol: 0 });
        kit.mk(kit.geo(new THREE.CylinderGeometry(0.03, 0.04, 0.32, 6)), WOOD, g, { p: [0.25, 0.18, 0], r: [0, 0, -0.6] });
      },

      // Slipper (dép tổ ong): an oval sole with honeycomb cells and a blue strap over the toes.
      slipper: (g) => {
        kit.mk(SPH, SLIPPER_SOLE, g, { s: [0.36, 0.07, 0.86] });
        for (const [x, z] of [[-0.14, -0.45], [0.14, -0.45], [0, -0.2], [-0.15, 0.05], [0.15, 0.05], [0, 0.3]]) {
          kit.mk(SPH, SLIPPER_CELL, g, { p: [x, 0.055, z], s: [0.075, 0.02, 0.075], ol: 0 });
        }
        kit.mk(kit.geo(new THREE.TorusGeometry(0.3, 0.07, 8, 18, Math.PI)), SLIPPER_STRAP, g, {
          p: [0, 0.02, 0.4],
          s: [1.15, 0.85, 1],
        });
      },

      // Durian: a spiky green ball with a stem.
      durian: (g) => {
        kit.mk(SPH, DURIAN, g, { s: [0.45, 0.5, 0.45] });
        const spike = kit.geo(new THREE.ConeGeometry(0.07, 0.18, 6));
        const up = new THREE.Vector3(0, 1, 0);
        const n = new THREE.Vector3();
        const SPIKES = 46;
        for (let i = 0; i < SPIKES; i++) {
          // Fibonacci sphere: evenly spread directions.
          const y = 1 - (2 * (i + 0.5)) / SPIKES;
          const r = Math.sqrt(1 - y * y);
          const a = i * Math.PI * (3 - Math.sqrt(5));
          n.set(Math.cos(a) * r, y, Math.sin(a) * r);
          const m = kit.mk(spike, DURIAN_SPIKE, g, { p: [n.x * 0.45, n.y * 0.5, n.z * 0.45], ol: 0 });
          m.quaternion.setFromUnitVectors(up, n);
        }
        kit.mk(kit.geo(new THREE.CylinderGeometry(0.04, 0.06, 0.22, 6)), WOOD_DARK, g, { p: [0, 0.6, 0] });
      },

      // Non-stick pan: a dark dish with a lighter inside and a long handle.
      pan: (g) => {
        kit.mk(kit.geo(new THREE.CylinderGeometry(0.55, 0.46, 0.13, 28)), PAN, g);
        kit.mk(kit.geo(new THREE.CylinderGeometry(0.48, 0.48, 0.02, 28)), PAN_INSIDE, g, { p: [0, 0.07, 0], ol: 0 });
        kit.mk(kit.geo(new THREE.CylinderGeometry(0.05, 0.065, 0.8, 8)), PAN_HANDLE, g, {
          p: [0.92, 0.03, 0],
          r: [0, 0, Math.PI / 2],
        });
      },

      // Fish: a plump blue-grey fish with a pale belly, a forked tail, a fin on top and one round eye.
      fish: (g) => {
        kit.mk(SPH, FISH, g, { s: [0.6, 0.26, 0.17] });
        kit.mk(SPH, FISH_BELLY, g, { p: [0.05, -0.09, 0], s: [0.5, 0.16, 0.172], ol: 0 });
        const fin = kit.geo(new THREE.ConeGeometry(0.2, 0.36, 4));
        for (const tilt of [0.45, -0.45]) {
          kit.mk(fin, FISH_FIN, g, { p: [-0.68, tilt * 0.3, 0], s: [1, 1, 0.3], r: [0, 0, -Math.PI / 2 + tilt] });
        }
        kit.mk(fin, FISH_FIN, g, { p: [-0.05, 0.27, 0], s: [0.8, 0.5, 0.3], r: [0, 0, 0.5] });
        kit.mk(SPH, 0xffffff, g, { p: [0.38, 0.06, 0.12], s: [0.075, 0.075, 0.05], ol: OUTLINE.eye });
        kit.mk(SPH, kit.inkMat, g, { p: [0.4, 0.06, 0.16], s: [0.04, 0.04, 0.02] });
      },

      // Rubber duck (rubber duck debugging): yellow body and head, orange beak, a wing and a dot eye.
      duck: (g) => {
        kit.mk(SPH, DUCK, g, { s: [0.42, 0.3, 0.33] });
        kit.mk(SPH, DUCK, g, { p: [0.24, 0.36, 0], s: [0.22, 0.22, 0.22] });
        kit.mk(SPH, DUCK, g, { p: [-0.38, 0.14, 0], s: [0.12, 0.09, 0.1], r: [0, 0, 0.6] });
        kit.mk(SPH, DUCK_WING, g, { p: [-0.04, 0.05, 0.28], s: [0.2, 0.12, 0.07], r: [0, 0, 0.25], ol: 0 });
        kit.mk(SPH, DUCK_BEAK, g, { p: [0.46, 0.33, 0], s: [0.13, 0.05, 0.1], ol: OUTLINE.nose });
        for (const z of [0.15, -0.15]) kit.mk(SPH, kit.inkMat, g, { p: [0.33, 0.43, z], s: [0.035, 0.04, 0.03] });
      },

      // Coffee mug: a red mug with a handle, still full of coffee.
      mug: (g) => {
        kit.mk(kit.geo(new THREE.CylinderGeometry(0.28, 0.25, 0.6, 24)), MUG, g);
        kit.mk(kit.geo(new THREE.CylinderGeometry(0.24, 0.24, 0.02, 24)), COFFEE, g, { p: [0, 0.295, 0], ol: 0 });
        kit.mk(kit.geo(new THREE.TorusGeometry(0.15, 0.045, 8, 16)), MUG, g, { p: [0.3, 0, 0], s: [0.9, 1.1, 1] });
      },

      // Bánh mì: a golden baguette with three cuts along the top and herbs sticking out of one end.
      banhmi: (g) => {
        kit.mk(SPH, BREAD, g, { s: [0.72, 0.21, 0.24] });
        for (const x of [-0.34, 0, 0.34]) {
          kit.mk(SPH, BREAD_CUT, g, { p: [x, 0.17, 0], s: [0.15, 0.04, 0.05], r: [0, 0.7, 0], ol: 0 });
        }
        kit.mk(SPH, HERB, g, { p: [0.66, 0.06, 0.04], s: [0.12, 0.07, 0.09], r: [0, 0, 0.5] });
        kit.mk(SPH, HERB, g, { p: [0.64, 0.02, -0.08], s: [0.1, 0.06, 0.07], r: [0, 0, -0.3] });
      },

      // Nón lá: a wide straw cone with two ridges round it.
      nonla: (g) => {
        const H = 0.42;
        const R = 0.78;
        kit.mk(kit.geo(new THREE.ConeGeometry(R, H, 32)), STRAW, g);
        for (const y of [-0.1, 0.03]) {
          const r = (R * (H / 2 - y)) / H;
          kit.mk(kit.geo(new THREE.TorusGeometry(r, 0.014, 6, 40)), STRAW_RIDGE, g, { p: [0, y, 0], r: [Math.PI / 2, 0, 0], ol: 0 });
        }
      },

      // Anvil: the cartoon classic, a flat top with a horn, a narrow waist and a wide foot.
      anvil: (g) => {
        kit.mk(kit.geo(new THREE.BoxGeometry(0.72, 0.16, 0.44)), IRON, g, { p: [0, -0.33, 0] });
        kit.mk(kit.geo(new THREE.BoxGeometry(0.34, 0.26, 0.3)), IRON, g, { p: [0, -0.13, 0] });
        kit.mk(kit.geo(new THREE.BoxGeometry(0.9, 0.2, 0.44)), IRON, g, { p: [-0.05, 0.1, 0] });
        kit.mk(kit.geo(new THREE.BoxGeometry(0.88, 0.02, 0.42)), IRON_TOP, g, { p: [-0.05, 0.205, 0], ol: 0 });
        kit.mk(kit.geo(new THREE.ConeGeometry(0.1, 0.42, 12)), IRON, g, { p: [0.6, 0.12, 0], r: [0, 0, -Math.PI / 2] });
      },
    };
    for (const kind of KNOCKOUT_OBJECTS) {
      const g = new THREE.Group();
      builders[kind](g);
      g.visible = false;
      this.world.add(g);
      this.projectiles.push(g);
    }
  }
}
