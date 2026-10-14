// Character sheet dev page (pnpm dev → /characters.html?c=cat): one rig in ten fixed poses at a fixed time, so
// the picture is the same on every run. Prints a pixel hash per tile: a refactor that must not change how a
// character looks keeps every hash. ?bg=dark renders on a dark desktop colour, ?px=2 at twice the resolution,
// ?poses=0,1 only those poses (0 = asleep, 1 = awake and grumpy).
import * as THREE from 'three';
import { createRig } from '../characters';
import { PICTURE_T, poseWith, SHEET_POSES } from '../characters/poses';
import { CHARACTERS, type CharacterId } from '../settings';

const W = 360;
const H = 330;
const BASE_H = 280;
const params = new URLSearchParams(location.search);
const dark = params.get('bg') === 'dark';
const px = Math.min(4, Math.max(1, Number(params.get('px')) || 1));
const id = (CHARACTERS as readonly string[]).includes(params.get('c') ?? '') ? (params.get('c') as CharacterId) : 'cat';

const picked = params.get('poses')?.split(',').map(Number).filter((i) => SHEET_POSES[i]);
const POSES = picked?.length ? picked.map((i) => SHEET_POSES[i]) : SHEET_POSES;
const COLS = Math.min(5, POSES.length);
const ROWS = Math.ceil(POSES.length / COLS);

document.body.classList.toggle('dark', dark);
const canvas = document.getElementById('sheet') as HTMLCanvasElement;
const renderer = new THREE.WebGLRenderer({ canvas, antialias: false, preserveDrawingBuffer: true });
renderer.setPixelRatio(px);
renderer.setSize(W * COLS, H * ROWS, false);
renderer.setClearColor(dark ? 0x1e1f24 : 0x7f8c8d, 1);
renderer.setScissorTest(true);

// The overlay's framing (CatStage) for a 360×330 stage.
const tan = Math.tan(THREE.MathUtils.degToRad(15));
const camPos = new THREE.Vector3(0.5, 2.3, 11.5);
const target = new THREE.Vector3(0.5, 2.0, 0);
const lift = ((H - BASE_H) / 2) * ((2 * camPos.distanceTo(target) * tan) / BASE_H);
const camera = new THREE.PerspectiveCamera(THREE.MathUtils.radToDeg(2 * Math.atan((tan * H) / BASE_H)), W / H, 0.1, 100);
camera.position.set(camPos.x, camPos.y + lift, camPos.z);
camera.lookAt(target.x, target.y + lift, target.z);
camera.updateProjectionMatrix();

const gl = renderer.getContext();
const pixels = new Uint8Array(W * H * px * px * 4);
const hashes: string[] = [];
POSES.forEach((p, i) => {
  const scene = new THREE.Scene();
  scene.add(new THREE.AmbientLight(0xffffff, 0.95));
  const sun = new THREE.DirectionalLight(0xffffff, 2.3);
  sun.position.set(-3, 5, 6);
  // A fresh rig per tile: the knockout bounce keeps state between frames.
  const rig = createRig(id);
  scene.add(sun, rig.object);
  rig.applyPose(poseWith(p), PICTURE_T);
  const x = (i % COLS) * W;
  const y = (ROWS - 1 - Math.floor(i / COLS)) * H;
  renderer.setViewport(x, y, W, H);
  renderer.setScissor(x, y, W, H);
  renderer.render(scene, camera);
  rig.dispose();
  // FNV-1a over the tile's RGBA.
  gl.readPixels(x * px, y * px, W * px, H * px, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
  let h = 0x811c9dc5;
  for (let j = 0; j < pixels.length; j++) h = Math.imul(h ^ pixels[j], 0x01000193) >>> 0;
  hashes.push(`tile${i}:${h.toString(16)}`);
});
document.getElementById('hashes')!.textContent = hashes.join('\n');
