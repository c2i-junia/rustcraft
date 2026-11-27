<!-- A crafting recipe as the grid of the game shows it: the cells, an arrow, what it makes. -->
<script lang="ts">
	import Slot from './Slot.svelte';

	type Recipe = {
		output: string;
		count: number;
		width: number;
		height: number;
		cells: (string | null)[];
		table: boolean;
	};
	let { recipe }: { recipe: Recipe } = $props();
	// Drawn in a grid of the station's size, the recipe in its top left corner, as it is put
	let size = $derived(recipe.table ? 3 : 2);
	let grid = $derived(
		Array.from({ length: size * size }, (_, i) => {
			const row = Math.floor(i / size);
			const col = i % size;
			return row < recipe.height && col < recipe.width ? recipe.cells[row * recipe.width + col] : null;
		})
	);
</script>

<figure class="recipe">
	<div class="grid" style:--n={size}>
		{#each grid as cell, i (i)}
			<Slot id={cell} />
		{/each}
	</div>
	<span class="arrow" aria-label="makes">
		<svg viewBox="0 0 22 16" width="33" height="24" aria-hidden="true"
			><path d="M0 6h12V2h2v2h2v2h2v2h2v0h-2v2h-2v2h-2v2h-2v-4H0z" fill="currentColor" /></svg
		>
	</span>
	<Slot id={recipe.output} count={recipe.count} big />
	<figcaption>{recipe.table ? 'Crafting table' : 'Inventory or crafting table'}</figcaption>
</figure>

<style>
	.recipe {
		display: inline-grid;
		grid-template-columns: auto auto auto;
		grid-template-rows: auto auto;
		align-items: center;
		gap: 8px 14px;
		margin: 0;
		padding: 12px 14px 10px;
		background: var(--panel);
		border: 2px solid var(--line);
	}
	.grid {
		display: grid;
		grid-template-columns: repeat(var(--n), 44px);
		gap: 2px;
	}
	.arrow {
		color: var(--muted);
		display: grid;
	}
	figcaption {
		grid-column: 1 / -1;
		font-size: 0.8rem;
		color: var(--muted);
	}
</style>
