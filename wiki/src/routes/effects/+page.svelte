<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import { duration } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<Page
	title="Status effects"
	description="Every status effect in Rustcraft: what it does at each level, and the potions, foods and creatures that give it."
	kicker="Effects go up to level IV. The HUD shows them at the top left, with the time left."
>
	<ul>
		<li>A stronger effect replaces a weaker one; at the same level, the longer time is kept; a weaker one is ignored.</li>
		<li>A <ItemLink id="MilkBucket" /> clears them all, and so does death.</li>
		<li>In creative mode, creatures give you no effect, but potions still work.</li>
	</ul>

	{#each data.effects as e (e.id)}
		<section id={e.id}>
			<h2>{e.name.en} <span class="kind" class:good={e.beneficial}>{e.beneficial ? 'Good' : 'Harmful'}</span></h2>
			<p>{e.does.en}</p>
			{#if e.potions.length || e.foods.length || e.mobs.length}
				<p class="from">
					<span>From:</span>
					{#each e.potions as id (id)}<ItemLink {id} />{/each}
					{#each e.foods as f (f.id)}<span><ItemLink id={f.id} /> (level {f.level}, {duration(f.seconds)})</span>{/each}
					{#each e.mobs as m (m.id)}<span>the blow of <ItemLink id={m.id} /> ({m.text})</span>{/each}
				</p>
			{/if}
		</section>
	{/each}
</Page>

<style>
	section {
		scroll-margin-top: 80px;
	}
	.kind {
		font-size: 0.85rem;
		font-family: var(--body);
		font-weight: 500;
		color: var(--bad);
		vertical-align: middle;
		margin-left: 8px;
	}
	.kind.good {
		color: var(--good);
	}
	.from {
		display: flex;
		flex-wrap: wrap;
		gap: 6px 18px;
		color: var(--muted);
	}
</style>
