<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import Recipe from '#lib/components/Recipe.svelte';
	import Blocks from '#lib/components/markup/Blocks.svelte';
	import type { Block } from '#lib/markup.ts';
	import type { PageProps } from './$types';

	type RecipeData = Parameters<typeof Recipe>[1]['recipe'];

	let { data }: PageProps = $props();
</script>

<Page title={data.title} description={data.description}>
	{#snippet icon()}{#if data.icon}<Slot id={data.icon} big link={false} />{/if}{/snippet}
	<Blocks blocks={data.blocks as Block[]}>
		{#snippet extra(b)}
			{@const group = b as unknown as { type: string; recipes?: RecipeData[] }}
			{#if group.type === 'recipes'}
				<div class="recipes">{#each group.recipes ?? [] as recipe (recipe.output)}<Recipe {recipe} />{/each}</div>
			{/if}
		{/snippet}
	</Blocks>
</Page>
