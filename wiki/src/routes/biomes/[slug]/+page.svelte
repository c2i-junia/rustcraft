<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Infobox from '#lib/components/Infobox.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import { percent, range } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	let b = $derived(data.biome);
	const chance = (c: { edge: number; heart: number }) =>
		c.edge === c.heart ? percent(c.heart) : `${percent(c.edge)} at its edge, ${percent(c.heart)} at its heart`;
	const TREE_NAMES: Record<string, string> = {
		Oak: 'Oak',
		BigOak: 'Big oak',
		Jungle: 'Jungle tree',
		Bush: 'Jungle bush',
		Acacia: 'Acacia'
	};
	const RAIN: Record<string, string> = { Rain: 'Rain', Snow: 'Snow', None: 'Never' };
	let plants = $derived(
		[
			b.flora.flowers ? { id: 'Dandelion', text: `Flowers (dandelions and poppies), ${percent(b.flora.flowers)} of columns` } : null,
			b.flora.tall_grass.heart ? { id: 'TallGrass', text: `Tall grass, ${chance(b.flora.tall_grass)}` } : null,
			b.flora.ferns.heart ? { id: 'Fern', text: `Ferns, ${chance(b.flora.ferns)}` } : null,
			b.flora.cacti ? { id: 'Cactus', text: `Cacti, ${percent(b.flora.cacti)} of sand columns` } : null,
			b.flora.ice_spikes ? { id: 'Ice', text: `Spikes of ice, ${percent(b.flora.ice_spikes)} of columns` } : null
		].filter((p) => p !== null)
	);
</script>

<Page title={data.name} description="The {data.name} biome of Rustcraft: {data.note}" kicker={data.note}>
	{#snippet icon()}<span class="swatch" style:background={data.colour}><Slot id={b.ground.surface} link={false} /></span>{/snippet}

	<Infobox title={data.name}>
		{#snippet picture()}<span class="swatch big" style:background={data.colour}><Slot id={b.ground.surface} big link={false} /></span>{/snippet}
		<dt>Surface</dt>
		<dd><ItemLink id={b.ground.surface} /></dd>
		<dt>Below</dt>
		<dd><ItemLink id={b.ground.sub_surface} /></dd>
		<dt>Rain</dt>
		<dd>{RAIN[b.look.precipitation] ?? b.look.precipitation}</dd>
		{#if b.ground.frozen}<dt>Water</dt><dd>Frozen at the surface</dd>{/if}
		<dt>Villages</dt>
		<dd>{b.village ? `${b.village} style` : 'None'}</dd>
		<dt>Locate</dt>
		<dd><code>/locate biome {b.key.replace('rustcraft:', '')}</code></dd>
	</Infobox>

	{#if data.parents.length || data.border}
		<p>
			{#if data.border}It forms the border of <a href={data.border.href}>{data.border.name}</a>.{/if}
			{#each data.parents as p (p.href)}
				It appears inside or above <a href={p.href}>{p.name}</a>.
			{/each}
		</p>
	{/if}

	<h2>Trees and plants</h2>
	{#if !data.trees.length && !plants.length && !b.flora.giant_trees}
		<p>Nothing grows here.</p>
	{:else}
		<ul>
			{#each data.trees as t (t.tree)}
				<li>
					{#if t.sapling}<ItemLink id={t.sapling} label={TREE_NAMES[t.tree] ?? t.tree} />{:else}{TREE_NAMES[t.tree] ?? t.tree}{/if},
					{chance(t.chance)} of columns
				</li>
			{/each}
			{#if b.flora.giant_trees}<li>Giant jungle trees deep inside, their trunks two blocks wide</li>{/if}
			{#each plants as p (p.id)}
				<li><ItemLink id={p.id} label={p.text} /></li>
			{/each}
		</ul>
	{/if}

	<h2>Animals</h2>
	{#if b.herds.length}
		<table>
			<thead><tr><th>Animal</th><th class="n">Herd</th><th class="n">Chance per chunk</th></tr></thead>
			<tbody>
				{#each b.herds as h (h.kind)}
					<tr>
						<td><ItemLink id={h.kind} /></td>
						<td class="n">{range(h.fewest, h.most)}</td>
						<td class="n">{percent(h.chance)}</td>
					</tr>
				{/each}
			</tbody>
		</table>
		<p>A herd is rolled once per chunk when the world is made, on open ground above the sea.</p>
	{:else}
		<p>No herds live here.</p>
	{/if}

	{#if data.subBiomes.length || data.altitude.length}
		<h2>Nearby biomes</h2>
		<ul>
			{#each data.subBiomes as s (s.href)}
				<li><a href={s.href}>{s.name}</a> takes {percent(s.share)} of it, in patches.</li>
			{/each}
			{#each data.altitude as a (a.href)}
				<li>Above height {a.height}, it becomes <a href={a.href}>{a.name}</a>.</li>
			{/each}
		</ul>
	{/if}
</Page>

<style>
	.swatch {
		display: inline-grid;
		place-items: center;
		width: 64px;
		height: 64px;
	}
	.swatch.big {
		width: 96px;
		height: 96px;
	}
</style>
