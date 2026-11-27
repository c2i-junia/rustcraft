<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Recipe from '#lib/components/Recipe.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import { ref } from '#lib/refs.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const LABELS: Record<string, string> = {
		Stations: 'Stations and furniture',
		Tools: 'Tools, weapons and armour',
		Materials: 'Materials and food',
		Wood: 'Wood',
		Blocks: 'Building blocks',
		Nature: 'Nature',
		Ores: 'Ores',
		Mobs: 'Spawn eggs'
	};
	let query = $state('');
	let small = $state(false);
	const fold = (s: string) =>
		s
			.normalize('NFD')
			.replace(/[̀-ͯ]/g, '')
			.toLowerCase();
	let shown = $derived.by(() => {
		const q = fold(query.trim());
		return data.recipes.filter((r) => {
			if (small && r.table) return false;
			if (!q) return true;
			const names = [r.output, ...r.cells.filter((c) => c !== null)].map((id) => fold(ref(id)?.name ?? id));
			return names.some((n) => n.includes(q));
		});
	});
</script>

<Page
	title="Crafting"
	description="How crafting works in Rustcraft, and all {data.recipes.length} recipes, searchable by result or ingredient."
	kicker="{data.recipes.length} recipes. Search by what you want, or by what you have."
>
	<p>
		Open your inventory with <kbd>E</kbd>: its 2 × 2 grid makes the small recipes. The bigger ones need the
		3 × 3 grid of a <ItemLink id="CraftingTable" />, which you make from four planks. Put the ingredients in
		the shape shown, anywhere in the grid; a recipe also works mirrored left to right. Take the result
		from the slot on the right; shift-click takes as many as the grid allows.
	</p>
	<p>
		The <strong>item browser</strong> of the inventory lists every recipe too: click a recipe there and it
		fills the grid from your inventory. A cell marked “Any planks” takes planks of any wood, and the
		same goes for cobblestone, wool, raw fish and roast meat.
	</p>

	<h2>Recipes</h2>
	<div class="filters">
		<input type="search" bind:value={query} placeholder="An item to make or an ingredient" aria-label="Filter recipes" />
		<label><input type="checkbox" bind:checked={small} /> Only those of the inventory's 2 × 2 grid</label>
	</div>

	{#each data.order as c (c)}
		{@const list = shown.filter((r) => r.category === c)}
		{#if list.length}
			<h3>{LABELS[c] ?? c} <span class="count">{list.length}</span></h3>
			<div class="recipes">
				{#each list as recipe, i (i)}<Recipe {recipe} />{/each}
			</div>
		{/if}
	{:else}
		<p>No recipe.</p>
	{/each}
	{#if !shown.length}<p>No recipe makes or uses “{query}”.</p>{/if}

	<h2>Families of items</h2>
	<p>Some cells take any item of a family:</p>
	<ul class="families">
		{#each Object.entries(data.tags) as [tag, members] (tag)}
			<li>
				<strong>{ref(`#${tag}`)?.name}</strong>
				<span class="members">{#each members as id (id)}<Slot {id} />{/each}</span>
			</li>
		{/each}
	</ul>
</Page>

<style>
	.filters {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 10px 20px;
	}
	.filters input[type='search'] {
		font: inherit;
		color: var(--fg);
		background: var(--slot-deep);
		border: 2px solid var(--line);
		padding: 8px 12px;
		width: min(420px, 100%);
	}
	.filters input[type='search']:focus {
		outline: none;
		border-color: var(--rust);
	}
	.count {
		color: var(--muted);
		font-size: 0.9rem;
	}
	.families {
		list-style: none;
		padding: 0;
		display: grid;
		gap: 10px;
	}
	.families li {
		display: flex;
		align-items: center;
		gap: 14px;
		flex-wrap: wrap;
	}
	.families strong {
		min-width: 150px;
	}
	.members {
		display: flex;
		flex-wrap: wrap;
		gap: 2px;
	}
</style>
