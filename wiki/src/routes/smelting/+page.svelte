<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import { duration } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	// One row per result: the ores and the logs share theirs
	let byOutput = $derived(
		Object.entries(
			data.smelts.reduce<Record<string, string[]>>((acc, s) => {
				(acc[s.output] ??= []).push(s.input);
				return acc;
			}, {})
		)
	);
	let fuelGroups = $derived(
		Object.entries(
			data.fuels.reduce<Record<number, string[]>>((acc, f) => {
				(acc[f.seconds] ??= []).push(f.id);
				return acc;
			}, {})
		).sort((a, b) => +b[0] - +a[0])
	);
</script>

<Page
	title="Smelting"
	description="The furnace of Rustcraft: every smelting recipe, every fuel and how long it burns."
	kicker="A furnace smelts one item every {data.cookSeconds} seconds."
>
	{#snippet icon()}<Slot id="Furnace" big link={false} />{/snippet}

	<p>
		Make a <ItemLink id="Furnace" /> from eight <ItemLink id="Cobblestone" /> in a ring, at a crafting table.
		Place it and use it: put what to smelt in the top slot and fuel in the bottom one. Each item takes
		<strong>{data.cookSeconds} seconds</strong>; the result piles up on the right.
	</p>
	<ul>
		<li>Fuel is only lit when there is something to smelt, so none is wasted.</li>
		<li>Out of fuel, the progress of the item in work goes back down, twice as fast as it went up.</li>
		<li>
			A furnace keeps working while you are away, as long as its chunk is loaded. In a chunk too far from
			every player, it waits.
		</li>
		<li>Breaking a furnace drops what is inside.</li>
		<li>Smelting ores and food gives experience as the items come out.</li>
	</ul>

	<h2>Recipes</h2>
	<div class="wide">
		<table>
			<thead><tr><th>Put in</th><th>Get out</th></tr></thead>
			<tbody>
				{#each byOutput as [output, inputs] (output)}
					<tr>
						<td class="ins">{#each inputs as id (id)}<ItemLink {id} />{/each}</td>
						<td><ItemLink id={output} /></td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>

	<h2>Fuels</h2>
	<table>
		<thead><tr><th>Fuel</th><th class="n">Burns</th><th class="n">Smelts</th></tr></thead>
		<tbody>
			{#each fuelGroups as [seconds, ids] (seconds)}
				<tr>
					<td class="ins">{#each ids as id (id)}<Slot {id} />{/each}</td>
					<td class="n">{duration(+seconds)}</td>
					<td class="n">{+(+seconds / data.cookSeconds).toFixed(2)} items</td>
				</tr>
			{/each}
		</tbody>
	</table>
	<p>A <ItemLink id="LavaBucket" /> leaves its <ItemLink id="Bucket" /> behind.</p>
</Page>

<style>
	.ins {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 14px;
		max-width: 640px;
	}
	td.ins {
		display: table-cell;
	}
	td.ins > :global(*) {
		margin: 2px 12px 2px 0;
	}
</style>
