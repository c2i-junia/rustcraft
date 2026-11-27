<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Infobox from '#lib/components/Infobox.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import { num, percent, range } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	let mob = $derived(data.mob);
	let facts = $derived(data.facts);
	let name = $derived(mob.name.en ?? mob.id);
	let kind = $derived(
		mob.boss ? 'Boss' : mob.hostile ? 'Hostile' : mob.villager ? 'Villager' : mob.aquatic ? 'Fish' : 'Animal'
	);
</script>

<Page
	title={name}
	description="{name} in Rustcraft: {facts?.summary ?? ''} Health, where it lives, what it drops."
	kicker={facts?.summary}
>
	{#snippet icon()}<Slot id="mob:{mob.id}" big link={false} />{/snippet}

	<Infobox title={name}>
		{#snippet picture()}<Slot id="mob:{mob.id}" big link={false} size={48} />{/snippet}
		<dt>Kind</dt>
		<dd>{kind}</dd>
		<dt>Health</dt>
		<dd>{mob.health} ({mob.health / 2} hearts)</dd>
		{#if facts?.damage}<dt>Damage</dt><dd>{facts.damage}</dd>{/if}
		{#if facts?.effect}<dt>Its blow</dt><dd>{facts.effect}</dd>{/if}
		{#if facts?.xp}<dt>Experience</dt><dd>{facts.xp}</dd>{/if}
		{#if facts?.speed}<dt>Speed</dt><dd>{facts.speed}</dd>{/if}
		{#if facts?.sight}<dt>Sees you from</dt><dd>{facts.sight} blocks</dd>{/if}
		<dt>Size</dt>
		<dd>{num(mob.size[0], 1)} × {num(mob.size[1], 1)} blocks</dd>
		{#if mob.tameable}<dt>Tameable</dt><dd>Yes, 1 in 3 per feeding</dd>{/if}
		{#if mob.mount}<dt>Ridden</dt><dd>With a saddle</dd>{/if}
		{#if mob.leashable}<dt>Lead</dt><dd>Yes</dd>{/if}
		{#if mob.egg}<dt>Spawn egg</dt><dd><ItemLink id={mob.egg} label="Egg" /></dd>{/if}
		{#if mob.name.fr && mob.name.fr !== name}<dt>In French</dt><dd>{mob.name.fr}</dd>{/if}
	</Infobox>

	{#if facts}
		<h2>Where it lives</h2>
		<p>{facts.spawns}</p>
	{/if}
	{#if data.habitats.length}
		<table>
			<thead><tr><th>Biome</th><th class="n">Herd</th><th class="n">Chance per chunk</th></tr></thead>
			<tbody>
				{#each data.habitats as h (h.href)}
					<tr>
						<td><a href={h.href}>{h.name}</a></td>
						<td class="n">{range(h.fewest, h.most)}</td>
						<td class="n">{percent(h.chance)}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{/if}

	{#if facts}
		<h2>Behaviour</h2>
		<!-- eslint-disable-next-line svelte/no-at-html-tags -->
		<p>{@html facts.behaviour}</p>
	{/if}

	{#if mob.eats.length}
		<h2>Food</h2>
		<p>
			It eats
			{#each mob.eats as id, i (id)}{i ? (i === mob.eats.length - 1 ? ' and ' : ', ') : ''}<ItemLink {id} />{/each}
			from your hand.
			{#if mob.tameable}Fed while wild, each one has a one in three chance to tame it.{/if}
			Fed as an adult, it looks for a mate for 30 seconds; fed young, it grows up faster.
		</p>
	{/if}

	<h2>Drops</h2>
	{#if data.loot.length}
		<table>
			<thead><tr><th>Item</th><th class="n">Count</th><th class="n">Chance</th></tr></thead>
			<tbody>
				{#each data.loot as l (l.item)}
					<tr>
						<td><ItemLink id={l.item} /></td>
						<td class="n">{range(l.min, l.max)}</td>
						<td class="n">{l.chance ? percent(l.chance) : (l.note ?? '100 %')}</td>
					</tr>
				{/each}
			</tbody>
		</table>
		{#if mob.hostile && !mob.boss}
			<p>
				In the Depths its loot is rolled one and a half times on average and twice in the Furnace,
				with a chance of iron, gold or a diamond on top: see <a href="/guides/depths">the depths</a>.
			</p>
		{:else if mob.animal}
			<p>A young one drops nothing.</p>
		{/if}
	{:else}
		<p>Nothing.</p>
	{/if}

	{#if mob.trades.length}
		<h2>Trades</h2>
		<table>
			<thead><tr><th>You give</th><th>You get</th><th class="n">Per day</th></tr></thead>
			<tbody>
				{#each mob.trades as t, i (i)}
					<tr>
						<td><ItemLink id={t.cost.item} count={t.cost.count} /></td>
						<td>
							{#if 'item' in t.goods}<ItemLink id={t.goods.item} count={t.goods.count} />{:else}Directions
								to the nearest dungeon{/if}
						</td>
						<td class="n">{t.stock}</td>
					</tr>
				{/each}
			</tbody>
		</table>
		<p>The stock comes back each sunrise. See <a href="/guides/trading">villagers and trade</a>.</p>
	{/if}
</Page>
