// The slime's look (docs/characters/slime/sheet.md): glossy jelly instead of the Klat toon bands and ink lines.
// One shader does the shape, the skin and the face. The vertex shader turns a unit sphere into a squat gumdrop and
// bends it every frame (melt into a puddle, jiggle, sway); the fragment shader paints a colour gradient, soft
// spots, bubbles, a cool rim, highlights, and the face itself. The face is painted on the gel, so it stretches,
// squashes and wobbles with the body and can never be hidden by it. A skin is just a GelSkin.
import * as THREE from 'three';

/** A slime skin: colours by role, as sRGB hex (the shader writes them straight to the screen). */
export interface GelSkin {
  top: number;
  mid: number;
  bottom: number;
  /** Tint along the very bottom edge. */
  base: number;
  /** Rim light and bubble glints. */
  glint: number;
  /** The sticker outline round the body. */
  line: number;
  /** Eyes: dark top, lighter bottom; also the face's lines. */
  eyeDark: number;
  eyeLight: number;
  blush: number;
  /** Inside of the open mouth, and the tongue. */
  mouth: number;
  tongue: number;
}

/** Squat gumdrop proportions: half-width, height and half-depth of the awake body (world units). */
export const GEL_SIZE = { x: 1.2, y: 2.15, z: 1.12 } as const;
/** How much a full melt flattens (fraction of the height lost) and spreads (extra width at the bottom / top). */
const MELT_FLAT = 0.68;
const MELT_SPREAD_TOP = 0.2;
const MELT_SPREAD_BOTTOM = 0.45;
/** Jiggle stretches the height by up to this much per unit of jiggle. */
const JIGGLE_H = 0.03;
/** Latitude of the face's centre on the unit sphere the body is bent from (radians). */
export const FACE_LAT = 0.25;
/** Sphere radians to face units (about world units on the awake body). */
const FACE_SCALE = 0.9;

/** Width factor up the body: wide at the bottom, rounder at the top (t = 0 bottom .. 1 top). */
function widthAt(t: number, melt: number, puff: number): number {
  return (1.2 - 0.2 * t) * (1 + melt * (MELT_SPREAD_TOP + MELT_SPREAD_BOTTOM * (1 - t))) * (1 + puff * 0.08);
}

/** Height fraction of a point on the sphere at latitude `lat`: the cap below y = -0.6 is pressed into the base. */
export const latToT = (lat: number): number => (Math.max(Math.sin(lat), -0.6) + 0.6) / 1.6;

/** The body's current height. CPU twin of h in shape(). */
export function gelHeight(melt: number, puff: number, jiggle: number, time: number): number {
  return GEL_SIZE.y * (1 - MELT_FLAT * melt) * (1 + puff * 0.06) * (1 + jiggle * JIGGLE_H * Math.sin(time * 13));
}

/** Radii of the body's cross-section at height fraction t: [sideways, front-to-back]. CPU twin of shape(). */
export function gelRadii(t: number, melt: number, puff: number, out: [number, number]): [number, number] {
  const y = t * 1.6 - 0.6;
  const r = Math.sqrt(Math.max(0, 1 - y * y)) * widthAt(t, melt, puff);
  out[0] = r * GEL_SIZE.x;
  out[1] = r * GEL_SIZE.z;
  return out;
}

const f = (n: number): string => n.toFixed(3);

const SHAPE = /* glsl */ `
uniform float uTime;
uniform float uMelt;
uniform float uPuff;
uniform float uJiggle;
uniform float uSway;
uniform float uOutline;
uniform float uDeform;

float widthAt(float t) {
  return (1.2 - 0.2 * t) * (1.0 + uMelt * (${f(MELT_SPREAD_TOP)} + ${f(MELT_SPREAD_BOTTOM)} * (1.0 - t))) * (1.0 + uPuff * 0.08);
}

// Unit sphere point -> body point. t (0 bottom .. 1 top) is written to tOut.
vec3 shape(vec3 p, out float tOut) {
  float y = max(p.y, -0.6);
  float t = (y + 0.6) / 1.6;
  tOut = t;
  float ang = atan(p.z, p.x);
  // Gel wobble: waves round the body and up it, stronger higher up.
  float wob = sin(ang * 3.0 + uTime * 9.0) * 0.5 + sin(t * 7.0 - uTime * 11.0) * 0.5;
  float w = widthAt(t) * (1.0 + wob * uJiggle * 0.045 * (0.3 + t));
  float h = ${f(GEL_SIZE.y)} * (1.0 - ${f(MELT_FLAT)} * uMelt) * (1.0 + uPuff * 0.06)
          * (1.0 + uJiggle * ${f(JIGGLE_H)} * sin(uTime * 13.0));
  return vec3(p.x * w * ${f(GEL_SIZE.x)} + uSway * t * t, t * h, p.z * w * ${f(GEL_SIZE.z)});
}
`;

const VERTEX = /* glsl */ `
${SHAPE}
uniform float uPartT;
varying vec3 vSph;
varying vec3 vNormalV;
varying vec3 vViewPos;
varying float vT;

void main() {
  vec3 p = normalize(position);
  vec3 q;
  vec3 n;
  float t;
  if (uDeform > 0.5) {
    // Normal from two nearby points on the sphere, bent the same way.
    vec3 a = normalize(cross(p, abs(p.y) < 0.99 ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0)));
    vec3 b = cross(p, a);
    float ta;
    q = shape(p, t);
    vec3 qa = shape(normalize(p + a * 0.01), ta) - q;
    vec3 qb = shape(normalize(p + b * 0.01), ta) - q;
    n = normalize(cross(qa, qb));
  } else {
    q = position;
    n = p;
    t = uPartT + p.y * 0.12;
  }
  q += n * uOutline;
  vSph = p;
  vT = t;
  vNormalV = normalize(normalMatrix * n);
  vec4 mv = modelViewMatrix * vec4(q, 1.0);
  vViewPos = mv.xyz;
  gl_Position = projectionMatrix * mv;
}
`;

const SPOTS = 4;
const BUBBLES = 4;

const FRAGMENT = /* glsl */ `
uniform vec3 uTop;
uniform vec3 uMid;
uniform vec3 uBottom;
uniform vec3 uBase;
uniform vec3 uGlint;
uniform vec3 uEyeDark;
uniform vec3 uEyeLight;
uniform vec3 uBlush;
uniform vec3 uMouthCol;
uniform vec3 uTongue;
uniform vec4 uSpots[${SPOTS}];
uniform vec4 uBubbles[${BUBBLES}];
uniform float uPattern;
uniform float uFaceOn;
// Face state: eye openness, happy (0/1), knocked out (0/1), awake.
uniform vec4 uEyes;
// Mouth openness, grumpiness.
uniform vec2 uMouth;
// Where the face looks: shifts it over the gel (sphere radians).
uniform vec2 uLook;
varying vec3 vSph;
varying vec3 vNormalV;
varying vec3 vViewPos;
varying float vT;

// 1 inside a soft disc of angular radius r around dir, on the unit sphere.
float disc(vec3 dir, float r, float soft) {
  float a = acos(clamp(dot(vSph, dir), -1.0, 1.0));
  return smoothstep(r, r * soft, a);
}

// Signed-distance helpers for the face (d < 0 inside), antialiased by the screen-space rate of change.
float aa(float d) {
  float w = fwidth(d) * 0.75;
  return 1.0 - smoothstep(-w, w, d);
}
float segment(vec2 p, vec2 a, vec2 b) {
  vec2 pa = p - a;
  vec2 ba = b - a;
  return length(pa - ba * clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0));
}
// Distance to the lower (down = 1) or upper (down = -1) half of a circle.
float halfArc(vec2 p, vec2 c, float r, float down) {
  vec2 q = p - c;
  if (q.y * down > 0.0) return length(vec2(abs(q.x) - r, q.y));
  return abs(length(q) - r);
}
float ellipse(vec2 p, vec2 r) {
  return (length(p / r) - 1.0) * min(r.x, r.y);
}

vec3 eye(vec3 col, vec2 e) {
  float open = uEyes.x;
  if (uEyes.z > 0.5) {
    float d = min(segment(e, vec2(-0.11, -0.11), vec2(0.11, 0.11)), segment(e, vec2(-0.11, 0.11), vec2(0.11, -0.11)));
    return mix(col, uEyeDark, aa(d - 0.032));
  }
  if (uEyes.y > 0.5) return mix(col, uEyeDark, aa(halfArc(e, vec2(0.0, -0.06), 0.15, -1.0) - 0.032));
  if (open < 0.18) return mix(col, uEyeDark, aa(halfArc(e, vec2(0.0, 0.05), 0.15, 1.0) - 0.032));
  // Cute, not mean: a grumpy squint (eye 0.62) only lowers the lid a little.
  float lid = open < 1.0 ? mix(0.55, 1.0, smoothstep(0.18, 1.0, open)) : min(1.2, open);
  vec2 r = vec2(0.19 * (1.0 + max(0.0, open - 1.0) * 0.5), 0.235 * lid);
  float d = ellipse(e, r);
  // Dark at the top, a blue glow at the bottom, a darker rim; two big highlights and a sparkle.
  vec3 c = mix(uEyeDark, uEyeLight, smoothstep(0.1, -0.95, e.y / r.y) * 0.85);
  c = mix(c, uEyeDark * 0.7, smoothstep(-0.035, 0.0, d));
  vec2 s = r / vec2(0.19, 0.235);
  c = mix(c, vec3(1.0), aa(length((e - vec2(-0.065, 0.075) * s) / s) - 0.072));
  c = mix(c, vec3(1.0), aa(length((e - vec2(0.07, -0.085) * s) / s) - 0.034));
  c = mix(c, vec3(1.0), aa(length((e - vec2(0.03, 0.12) * s) / s) - 0.018) * 0.9);
  return mix(col, c, aa(d));
}

vec3 face(vec3 col, vec2 p) {
  float awake = uEyes.w;
  float grumpy = uMouth.y * awake;
  // Blush
  for (int i = 0; i < 2; i++) {
    float s = i == 0 ? -1.0 : 1.0;
    float b = 1.0 - smoothstep(0.5, 1.0, length((p - vec2(s * 0.6, -0.1)) / vec2(0.15, 0.085)));
    col = mix(col, uBlush, b * 0.7);
  }
  for (int i = 0; i < 2; i++) {
    float s = i == 0 ? -1.0 : 1.0;
    col = eye(col, p - vec2(s * 0.38, 0.1));
    // Little cross brows only when grumpy: cute, not mean.
    float brow = segment(p, vec2(s * 0.48, 0.42), vec2(s * 0.3, 0.42 - grumpy * 0.06));
    col = mix(col, uEyeDark, aa(brow - 0.022) * smoothstep(0.15, 0.4, grumpy));
  }
  // Mouth: a "D" with a tongue when open, else a small smile, or the Klat "ω" when grumpy.
  vec2 m = p - vec2(0.0, -0.12);
  float open = uMouth.x;
  if (open > 0.06) {
    vec2 r = vec2(0.12 + 0.05 * open, 0.06 + 0.16 * open);
    float d = max(ellipse(m, r), m.y);
    vec3 c = mix(uMouthCol, uTongue, aa(length(m - vec2(0.0, -r.y * 0.95)) - r.x * 0.62));
    c = mix(c, uEyeDark, smoothstep(-0.022, 0.0, d));
    col = mix(col, c, aa(d));
  } else {
    float smile = halfArc(m, vec2(0.0, 0.035), 0.075, 1.0);
    vec2 w = vec2(abs(m.x) - 0.055, m.y);
    float omega = halfArc(w, vec2(0.0, 0.03), 0.055, 1.0);
    col = mix(col, uEyeDark, aa(mix(smile, omega, step(0.5, grumpy)) - 0.02));
  }
  return col;
}

void main() {
  vec3 n = normalize(vNormalV);
  vec3 v = normalize(-vViewPos);
  // The overlay's sun, (-3, 5, 6), as seen from the camera.
  vec3 L = normalize(vec3(-0.45, 0.62, 0.64));

  vec3 col = mix(uBottom, uMid, smoothstep(0.0, 0.45, vT));
  col = mix(col, uTop, smoothstep(0.45, 1.0, vT));
  col = mix(uBase, col, smoothstep(0.0, 0.14, vT));

  // Soft blobs floating inside the gel, and round bubbles with a bright edge and a glint.
  float spot = 0.0;
  for (int i = 0; i < ${SPOTS}; i++) spot = max(spot, disc(uSpots[i].xyz, uSpots[i].w, 0.3));
  col = mix(col, col * 0.8, spot * 0.45 * uPattern);
  for (int i = 0; i < ${BUBBLES}; i++) {
    vec3 d = uBubbles[i].xyz;
    float r = uBubbles[i].w;
    float inside = disc(d, r, 0.85);
    float edge = inside - disc(d, r * 0.7, 0.8);
    float glint = disc(normalize(d + vec3(-0.35, 0.45, 0.25) * r), r * 0.32, 0.5);
    col = mix(col, mix(col, uGlint, 0.3), inside * uPattern);
    col = mix(col, uGlint, (edge * 0.45 + glint) * uPattern);
  }

  // Soft wrapped light, light pooling in the middle like translucent jelly, and a cool rim.
  float facing = max(dot(n, v), 0.0);
  col *= 0.68 + 0.42 * (dot(n, L) * 0.5 + 0.5);
  col += col * pow(facing, 3.0) * 0.2;
  col = mix(col, uGlint, pow(1.0 - facing, 2.6) * 0.5);

  // The face, painted on the gel (sphere longitude / latitude around the front).
  if (uFaceOn > 0.5 && vSph.z > 0.0) {
    vec2 p = vec2(atan(vSph.x, vSph.z) - uLook.x, asin(clamp(vSph.y, -1.0, 1.0)) - ${f(FACE_LAT)} - uLook.y);
    col = face(col, p * ${f(FACE_SCALE)});
  }

  // Highlights: a big soft window streak and a small one on the upper left, clear of the face.
  vec2 hn = n.xy;
  float hl = smoothstep(0.15, 0.1, length((hn - vec2(-0.72, 0.52)) * vec2(1.0, 1.7)));
  hl = max(hl, smoothstep(0.06, 0.035, length(hn - vec2(-0.36, 0.78))) * 0.95);
  col = mix(col, vec3(1.0), hl * 0.85);

  gl_FragColor = vec4(col, 1.0);
}
`;

const OUTLINE_FRAGMENT = /* glsl */ `
uniform vec3 uLine;
void main() { gl_FragColor = vec4(uLine, 1.0); }
`;

/** sRGB hex as a raw vec3: the shader writes colours straight out, so they must not be linearised. */
function rgb(hex: number): THREE.Vector3 {
  return new THREE.Vector3(((hex >> 16) & 255) / 255, ((hex >> 8) & 255) / 255, (hex & 255) / 255);
}

const dir = (x: number, y: number, z: number, r: number) => new THREE.Vector4(...new THREE.Vector3(x, y, z).normalize().toArray(), r);
/** Spots and bubbles, as directions on the unit sphere the body is bent from (+z faces the viewer). Clear of the face. */
const SPOT_DIRS = [dir(0.6, -0.5, 0.65, 0.24), dir(-0.7, -0.45, 0.55, 0.22), dir(0.95, 0.15, 0.3, 0.2), dir(-0.2, -0.75, 0.6, 0.16)];
const BUBBLE_DIRS = [dir(0.78, -0.05, 0.62, 0.06), dir(-0.62, -0.25, 0.75, 0.05), dir(0.35, -0.55, 0.76, 0.045), dir(0.55, 0.55, 0.62, 0.035)];

export interface GelMaterials {
  /** Shared by the body and its outline: the rig writes the motion and face uniforms each frame. */
  readonly uniforms: {
    uTime: THREE.IUniform<number>;
    uMelt: THREE.IUniform<number>;
    uPuff: THREE.IUniform<number>;
    uJiggle: THREE.IUniform<number>;
    uSway: THREE.IUniform<number>;
    uEyes: THREE.IUniform<THREE.Vector4>;
    uMouth: THREE.IUniform<THREE.Vector2>;
    uLook: THREE.IUniform<THREE.Vector2>;
  };
  readonly body: THREE.ShaderMaterial;
  readonly bodyLine: THREE.ShaderMaterial;
  /** Small round parts (arms, droplets): the same skin on an unbent sphere, no face. */
  readonly part: THREE.ShaderMaterial;
  readonly partLine: THREE.ShaderMaterial;
}

/** Outline width in world units (the body) and as a fraction of a part's radius. */
const BODY_LINE = 0.05;
const PART_LINE = 0.1;

export function createGelMaterials(skin: GelSkin): GelMaterials {
  const motion = {
    uTime: { value: 0 },
    uMelt: { value: 0 },
    uPuff: { value: 0 },
    uJiggle: { value: 0 },
    uSway: { value: 0 },
    uEyes: { value: new THREE.Vector4(1, 0, 0, 1) },
    uMouth: { value: new THREE.Vector2() },
    uLook: { value: new THREE.Vector2() },
  };
  const shared = {
    ...motion,
    uTop: { value: rgb(skin.top) },
    uMid: { value: rgb(skin.mid) },
    uBottom: { value: rgb(skin.bottom) },
    uBase: { value: rgb(skin.base) },
    uGlint: { value: rgb(skin.glint) },
    uLine: { value: rgb(skin.line) },
    uEyeDark: { value: rgb(skin.eyeDark) },
    uEyeLight: { value: rgb(skin.eyeLight) },
    uBlush: { value: rgb(skin.blush) },
    uMouthCol: { value: rgb(skin.mouth) },
    uTongue: { value: rgb(skin.tongue) },
    uSpots: { value: SPOT_DIRS },
    uBubbles: { value: BUBBLE_DIRS },
    uOutline: { value: 0 },
    uDeform: { value: 1 },
    uPartT: { value: 0 },
    uPattern: { value: 1 },
    uFaceOn: { value: 1 },
  };
  const make = (extra: Record<string, THREE.IUniform>, fragment: string, side: THREE.Side) =>
    new THREE.ShaderMaterial({ uniforms: { ...shared, ...extra }, vertexShader: VERTEX, fragmentShader: fragment, side });
  const part = { uDeform: { value: 0 }, uPartT: { value: 0.4 }, uPattern: { value: 0 }, uFaceOn: { value: 0 } };
  return {
    uniforms: motion,
    body: make({}, FRAGMENT, THREE.FrontSide),
    bodyLine: make({ uOutline: { value: BODY_LINE } }, OUTLINE_FRAGMENT, THREE.BackSide),
    part: make(part, FRAGMENT, THREE.FrontSide),
    partLine: make({ ...part, uOutline: { value: PART_LINE } }, OUTLINE_FRAGMENT, THREE.BackSide),
  };
}
