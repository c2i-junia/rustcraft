// A guide written in the Codex's markup (`data/codex/guides`): the game shows the same text.
import { error } from '@sveltejs/kit';
import { game, recipesFor, withRefs } from '#lib/server/game.ts';

type Recipe = ReturnType<typeof recipesFor>[number];
import { NAV } from '#lib/nav.ts';
import { plain, type Block } from '#lib/markup.ts';
import type { EntryGenerator } from './$types';

export const entries: EntryGenerator = () => game.guides.map((g) => ({ slug: g.slug }));

export function load({ params }) {
	const guide = game.guides.find((g) => g.slug === params.slug);
	if (!guide) error(404, 'No guide has this name');
	const [head, ...blocks] = guide.en as Block[];
	const title = head?.type === 'heading' ? plain(head.text) : params.slug;
	const link = NAV.flatMap((s) => s.links).find((l) => l.href === `/guides/${params.slug}`);
	// The recipes in a row are shown side by side: the first recipe of each item
	const shown: (Block | { type: 'recipes'; recipes: Recipe[] })[] = [];
	for (const b of blocks) {
		if (b.type !== 'recipe') {
			shown.push(b);
			continue;
		}
		const recipe = recipesFor(b.item)[0];
		if (!recipe) continue;
		const last = shown.at(-1);
		if (last?.type === 'recipes') last.recipes.push(recipe);
		else shown.push({ type: 'recipes', recipes: [recipe] });
	}
	return withRefs({
		title,
		description: link?.blurb ?? title,
		icon: link?.icon ?? null,
		blocks: shown
	});
}
