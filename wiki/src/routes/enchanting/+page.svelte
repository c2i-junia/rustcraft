<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import Recipe from '#lib/components/Recipe.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const FITS: Record<string, string> = {
		Efficiency: 'Pickaxes, axes, shovels and hoes',
		Durability: 'Anything that wears out',
		Sharpness: 'Swords, axes and the spear',
		Protection: 'Armour',
		BowPower: 'The bow'
	};
	const NOTE: Record<string, string> = {
		Protection: 'Per piece worn: four pieces at III absorb 12 % more.'
	};
	// Levels to pay to reach each level, from the one below
	const COST = [3, 5, 8];
	// What the merges and repairs before add to the price of the next
	const PENALTY = [0, 1, 3, 7, 15, 31, 63];
</script>

<Page
	title="Enchanting and the anvil"
	description="Rustcraft's engraving table and anvil: every enchantment and its levels, what they cost, how to repair, merge and rename."
	kicker="Both are paid with experience levels, and free in creative mode."
>
	{#snippet icon()}<Slot id="EngravingTable" big link={false} />{/snippet}

	<h2>The engraving table</h2>
	<p>
		Put a tool, a weapon, a piece of armour or a bow in the slot of an <ItemLink id="EngravingTable" />, then
		raise one of the enchantments it takes, a level at a time, up to III. Its wear and its other
		enchantments are kept.
	</p>
	{#if data.table}<div class="recipes"><Recipe recipe={data.table} /></div>{/if}
	<table>
		<thead><tr><th>Level</th><th class="n">Costs</th><th class="n">In all</th></tr></thead>
		<tbody>
			{#each COST as c, i (i)}
				<tr><td>{['I', 'II', 'III'][i]}</td><td class="n">{c} levels</td><td class="n">{COST.slice(0, i + 1).reduce((a, b) => a + b, 0)} levels</td></tr>
			{/each}
		</tbody>
	</table>

	<h2>Enchantments</h2>
	<div class="wide">
		<table>
			<thead><tr><th>Enchantment</th><th>On</th><th>I</th><th>II</th><th>III</th></tr></thead>
			<tbody>
				{#each data.enchantments as e (e.id)}
					<tr id={e.id}>
						<td><strong>{e.name}</strong></td>
						<td>{FITS[e.id] ?? ''}{#if NOTE[e.id]}<br /><small>{NOTE[e.id]}</small>{/if}</td>
						{#each e.levels as l, i (i)}<td>{l}</td>{/each}
					</tr>
				{/each}
			</tbody>
		</table>
	</div>

	<h2>The anvil</h2>
	{#if data.anvil}<div class="recipes"><Recipe recipe={data.anvil} /></div>{/if}
	<p>
		An <ItemLink id="Anvil" /> has two slots: the item to work on the left, and on the right either its
		material or a second item of the same kind. It does three things:
	</p>
	<dl class="jobs">
		<dt>Repair</dt>
		<dd>
			With the item's material on the right. Each unit gives back a quarter of its durability, so four repair
			it fully, and costs 1 level.
		</dd>
		<dt>Merge</dt>
		<dd>
			With a second item of the same kind. Their durability adds up, plus 12 % of a new one, up to full.
			Enchantments of the same level join into the next level, up to III; otherwise the higher one is kept.
			It costs 2 levels, plus the level each enchantment the second item brings ends at.
		</dd>
		<dt>Rename</dt>
		<dd>Type a name of up to 24 letters: 1 level more.</dd>
	</dl>
	<p>
		Each repair or merge makes the next one dearer, as the table shows. A job that would cost more than 39
		levels is refused as too expensive.
	</p>
	<table>
		<thead><tr><th>Times worked before</th>{#each PENALTY as _, i (i)}<th class="n">{i}</th>{/each}</tr></thead>
		<tbody><tr><td>Extra levels</td>{#each PENALTY as p, i (i)}<td class="n">{p}</td>{/each}</tr></tbody>
	</table>

	<h3>Repair materials</h3>
	<table>
		<thead><tr><th>Material</th><th>Repairs</th></tr></thead>
		<tbody>
			{#each data.repairs as [material, ids] (material)}
				<tr>
					<td><ItemLink id={material} /></td>
					<td class="slots">{#each ids as id (id)}<Slot {id} />{/each}</td>
				</tr>
			{/each}
		</tbody>
	</table>
	<p>Experience comes from beating hostiles, mining coal and diamonds, smelting and fishing: see <a href="/guides/progression">progression</a>.</p>
</Page>

<style>
	.jobs {
		max-width: 72ch;
	}
	.jobs dt {
		font-family: var(--display);
		color: var(--rust);
		margin-top: 10px;
	}
	.jobs dd {
		margin: 2px 0 0;
	}
	.slots :global(.slot) {
		margin: 1px;
	}
	small {
		color: var(--muted);
	}
</style>
