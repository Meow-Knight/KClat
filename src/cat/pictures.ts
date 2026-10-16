// Pictures of each character for the Settings window, drawn from the real rigs so they never go stale.
// One WebGL renderer draws them all into PNG data URLs and is then thrown away: the window keeps no GPU
// context open. Loaded on demand, so three.js stays out of the Settings window's first paint.
import * as THREE from 'three';
import { CHARACTERS, type CharacterId } from '../settings';
import { createRig } from '../characters';
import { ASLEEP_POSE, GRUMPY_POSE, PICTURE_T, poseWith } from './poses';

export interface CharacterPictures {
  awake: string;
  asleep: string;
}

/** CSS px of one picture; drawn at twice that for sharp edges on high-DPI screens. */
export const PICTURE_SIZE = { w: 150, h: 140 } as const;
const SCALE = 2;
/** Framing: centred on the character (root x 1.3), from the ground to above the horns, wings included. */
const TARGET = new THREE.Vector3(1.3, 1.85, 0);
const DISTANCE = 12;
const VIEW_HEIGHT = 4.7;

let cache: Promise<Record<CharacterId, CharacterPictures>> | undefined;

/** Draws every character awake and asleep once per window; later calls get the same pictures. */
export function characterPictures(): Promise<Record<CharacterId, CharacterPictures>> {
  cache ??= new Promise((resolve, reject) => {
    // After the window has painted: drawing takes a moment.
    requestAnimationFrame(() => {
      try {
        resolve(draw());
      } catch (e) {
        cache = undefined;
        reject(e);
      }
    });
  });
  return cache;
}

function draw(): Record<CharacterId, CharacterPictures> {
  const canvas = document.createElement('canvas');
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: true, preserveDrawingBuffer: true });
  try {
    renderer.setPixelRatio(SCALE);
    renderer.setSize(PICTURE_SIZE.w, PICTURE_SIZE.h, false);
    renderer.setClearColor(0x000000, 0);
    const camera = new THREE.PerspectiveCamera(
      THREE.MathUtils.radToDeg(2 * Math.atan(VIEW_HEIGHT / 2 / DISTANCE)),
      PICTURE_SIZE.w / PICTURE_SIZE.h,
      0.1,
      100,
    );
    camera.position.set(TARGET.x, TARGET.y + 0.05, DISTANCE);
    camera.lookAt(TARGET);

    const shot = (id: CharacterId, pose: Parameters<typeof poseWith>[0]): string => {
      const scene = new THREE.Scene();
      // The overlay's lights (CatStage).
      scene.add(new THREE.AmbientLight(0xffffff, 0.95));
      const sun = new THREE.DirectionalLight(0xffffff, 2.3);
      sun.position.set(-3, 5, 6);
      const rig = createRig(id);
      scene.add(sun, rig.object);
      rig.applyPose(poseWith(pose), PICTURE_T);
      renderer.render(scene, camera);
      rig.dispose();
      return canvas.toDataURL('image/png');
    };

    const out = {} as Record<CharacterId, CharacterPictures>;
    for (const id of CHARACTERS) out[id] = { awake: shot(id, GRUMPY_POSE), asleep: shot(id, ASLEEP_POSE) };
    return out;
  } finally {
    renderer.dispose();
    renderer.forceContextLoss();
  }
}
