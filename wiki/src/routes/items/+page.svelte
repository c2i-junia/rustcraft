<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import { ref } from '#lib/refs.ts';
	import { page } from '$app/state';
	import { replaceState } from '$app/navigation';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const LABELS: Record<string, string> = {
		Blocks: 'Building blocks',
		Wood: 'Wood',
		Nature: 'Nature',
		Ores: 'Ores',
		Materials: 'Materials and food',
		Stations: 'Stations and furniture',
		Tools: 'Tools, weapons and armour',
		Mobs: 'Spawn eggs'
	};

	// Read once the page runs in the browser: a prerendered page has no query
	let category = $state<string | null>(null);
	let tag = $state<string | null>(null);
	let query = $state('');
	$effect(() => {
		category = page.url.searchParams.get('category');
		tag = page.url.searchParams.get('tag');
	});

	function choose(c: string | null) {
		category = c;
		tag = null;
		const url = new URL(page.url.href);
		url.search = c ? `?category=${c}` : '';
		replaceState(url, {});
	}

	const fold = (s: string) =>
		s
			.normalize('NFD')
			.replace(/[̀-ͯ]/g, '')
			.toLowerCase();

	let shown = $derived.by(() => {
		const q = fold(query.trim());
		return data.items.filter((i) => {
			if (tag && !(data.tags[tag] ?? []).includes(i.id)) return false;
			if (category && i.category !== category) return false;
			if (!q) return true;
			const name = ref(i.id)?.name ?? i.id;
			return fold(name).includes(q) || (i.fr !== null && fold(i.fr).includes(q));
		});
	});
	let groups = $derived(
		data.categories
			.map((c) => ({ c, items: shown.filter((i) => i.category === c) }))
			.filter((g) => g.items.length)
	);
</script>

<Page
	title="Items"
	description="Every block and item of Rustcraft, by category: click one for its recipes, its uses and where to find it."
	kicker="{data.items.length} blocks and items. Click one for its recipes, its uses and where to find it."
>
	<div class="filters">
		<input type="search" bind:value={query} placeholder="Filter by name, in English or French" aria-label="Filter items" />
		<div class="chips" role="group" aria-label="Category">
			<button aria-pressed={category === null && !tag} onclick={() => choose(null)}>All</button>
			{#each data.categories as c (c)}
				<button aria-pressed={category === c} onclick={() => choose(c)}>{LABELS[c] ?? c}</button>
			{/each}
		</div>
		{#if tag}
			<p class="tagged">Showing the items any recipe cell of <strong>{tag}</strong> takes. <button class="link" onclick={() => choose(null)}>Show all</button></p>
		{/if}
	</div>

	{#each groups as g (g.c)}
		<h2>{LABELS[g.c] ?? g.c} <span class="count">{g.items.length}</span></h2>
		<ul class="grid">
			{#each g.items as i (i.id)}
				<li>
					<a href={ref(i.id)?.href}>
						<Slot id={i.id} link={false} />
						<span>{ref(i.id)?.name}</span>
					</a>
				</li>
			{/each}
		</ul>
	{:else}
		<p>No item is called “{query}”. Check the spelling, or search in French.</p>
	{/each}
</Page>

<style>
	.filters {
		display: grid;
		gap: 12px;
		margin-bottom: 8px;
	}
	input {
		font: inherit;
		color: var(--fg);
		background: var(--slot-deep);
		border: 2px solid var(--line);
		padding: 8px 12px;
		max-width: 420px;
	}
	input:focus {
		outline: none;
		border-color: var(--rust);
	}
	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}
	.chips button,
	.link {
		font: inherit;
		font-size: 0.92rem;
		color: var(--fg);
		background: var(--slot);
		border: 2px solid var(--line);
		padding: 4px 12px;
		cursor: pointer;
	}
	.chips button[aria-pressed='true'] {
		background: var(--rust);
		border-color: var(--rust);
		color: #1a0e06;
	}
	.chips button:hover {
		border-color: var(--rust);
	}
	.tagged {
		margin: 0;
	}
	.count {
		font-size: 1rem;
		color: var(--muted);
		font-weight: 500;
	}
	.grid {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(210px, 1fr));
		gap: 4px 12px;
	}
	.grid a {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 2px;
		color: var(--fg);
		text-decoration: none;
	}
	.grid a:hover {
		color: var(--rust);
	}
	.grid a:hover :global(.slot) {
		background: var(--slot-hover);
	}
</style>
