// Copies the game's textures into static/textures, where the wiki's icons are served from. The
// game's folder stays the only copy in the repository.
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
console.log(`textures copied to ${target}`);
