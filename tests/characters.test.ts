import { describe, expect, it } from 'vitest';
import type { CatPose } from '../src/kit/pose';
import { createRig } from '../src/characters';
import { PICTURE_T, poseWith, SHEET_POSES } from '../src/cat/poses';
import { CHARACTERS, DEFAULT_SETTINGS, type CharacterId } from '../src/settings';

/** Every sheet pose, plus each knockout object after the hit and the tapping beat at its peak. */
const POSES: Partial<CatPose>[] = [
  ...SHEET_POSES,
  ...[0, 1, 2, 3].map((k) => ({ dizzy: 1, bump: 1, bonkKind: k, bonkT: 0.5 })),
  { sleepy: 0, tapping: true, puff: 1 },
];

describe('characters', () => {
  it('the cat is the default', () => {
    expect(DEFAULT_SETTINGS.character).toBe('cat');
    expect(CHARACTERS[0]).toBe('cat');
  });

  it.each(CHARACTERS)('%s builds, takes every pose and frees itself', (id) => {
    const rig = createRig(id);
    for (const p of POSES) rig.applyPose(poseWith(p), PICTURE_T);
    let meshes = 0;
    rig.object.traverse(() => meshes++);
    expect(meshes).toBeGreaterThan(30);
    expect(rig.hitBounds.awake.isEmpty()).toBe(false);
    expect(rig.hitBounds.asleep.isEmpty()).toBe(false);
    rig.dispose();
  });

  it('an unknown id (a newer settings file) falls back to the cat', () => {
    const rig = createRig('unicorn' as CharacterId);
    expect(rig.constructor.name).toBe(createRig('cat').constructor.name);
  });
});
