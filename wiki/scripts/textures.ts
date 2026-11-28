// Copies the game's textures into static/textures, where the wiki's icons are served from, and the
// portraits of the creatures the Codex shows into static/creatures. The game's folder stays the
// only copy in the repository.
import { cpSync, mkdirSync, rmSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const source = fileURLToPath(new URL('../../data/graphics/base_textures/', import.meta.url));
const target = fileURLToPath(new URL('../static/textures/', import.meta.url));

rmSync(target, { recursive: true, force: true });
mkdirSync(target, { recursive: true });
for (const folder of ['blocks', 'items']) {
	cpSync(source + folder, target + folder, {
		recursive: true,
		filter: (path) => !path.endsWith('.ron')
	});
}
const portraits = fileURLToPath(new URL('../../data/graphics/creatures/', import.meta.url));
const shown = fileURLToPath(new URL('../static/creatures/', import.meta.url));
rmSync(shown, { recursive: true, force: true });
cpSync(portraits, shown, { recursive: true });
console.log(`textures and portraits copied to ${target} and ${shown}`);
