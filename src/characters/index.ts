// The characters you can pick in Settings. Each one is a rig on the shared kit (src/kit) driven by the same
// CatController; the default is the cat. Designs: docs/characters/<id>/sheet.md.
import { CatRig } from './cat/CatRig';
import type { CharacterRig } from '../kit/CharacterRig';
import type { CharacterId } from '../settings';
import { DragonRig } from './dragon/DragonRig';
import { SnakeRig } from './snake/SnakeRig';

const RIGS: Record<CharacterId, () => CharacterRig> = {
  cat: () => new CatRig(),
  snake: () => new SnakeRig(),
  dragon: () => new DragonRig(),
};

export function createRig(id: CharacterId): CharacterRig {
  return (RIGS[id] ?? RIGS.cat)();
}
