<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Infobox from '#lib/components/Infobox.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import Line from '#lib/components/markup/Line.svelte';
	import { num, percent, range } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	let mob = $derived(data.mob);
	let facts = $derived(data.facts);
	let name = $derived(mob.name.en ?? mob.id);
	// The coats of `shared/src/world/mobs.rs` (`Coat::base`) and the model of each
	const COATS = [
		{ name: 'Bay', model: 'Horse', speed: 8.5, endurance: 8 },
		{ name: 'Chestnut', model: 'HorseChestnut', speed: 9.5, endurance: 6.5 },
		{ name: 'Black', model: 'HorseBlack', speed: 10, endurance: 5 },
		{ name: 'Grey', model: 'HorseGrey', speed: 8, endurance: 11 },
		{ name: 'White', model: 'HorseWhite', speed: 9, endurance: 8.5 },
		{ name: 'Dun', model: 'HorseDun', speed: 7.5, endurance: 13 }
	];
	let kind = $derived(
		mob.boss ? 'Boss' : mob.hostile ? 'Hostile' : mob.villager ? 'Villager' : mob.aquatic ? 'Fish' : 'Animal'
	);
</script>

<Page
	title={name}
	description="{name} in Rustcraft: {facts.summary} Health, where it lives, what it drops."
	kicker={facts.summary || undefined}
>
	{#snippet icon()}<Slot id="mob:{mob.id}" big link={false} />{/snippet}

	<Infobox title={name}>
		{#snippet picture()}<img class="portrait" src="/creatures/{mob.id}.png" alt="A {name.toLowerCase()}, as the game draws it" width="220" height="220" />{/snippet}
		<dt>Kind</dt>
		<dd>{kind}</dd>
		<dt>Health</dt>
		<dd>{mob.health} ({mob.health / 2} hearts)</dd>
		{#if facts.damage}<dt>Damage</dt><dd>{facts.damage}</dd>{/if}
		{#if facts.effect}<dt>Its blow</dt><dd>{facts.effect}</dd>{/if}
		{#if facts.xp}<dt>Experience</dt><dd>{facts.xp}</dd>{/if}
		{#if facts.speed}<dt>Speed</dt><dd>{facts.speed}</dd>{/if}
		{#if facts.sight}<dt>Sees you from</dt><dd>{facts.sight} blocks</dd>{/if}
		<dt>Size</dt>
		<dd>{num(mob.size[0], 1)} × {num(mob.size[1], 1)} blocks</dd>
		{#if mob.tameable}<dt>Tameable</dt><dd>Yes, 1 in 3 per feeding</dd>{/if}
		{#if mob.mount}<dt>Ridden</dt><dd>With a saddle</dd>{/if}
		{#if mob.leashable}<dt>Lead</dt><dd>Yes</dd>{/if}
		{#if mob.egg}<dt>Spawn egg</dt><dd><ItemLink id={mob.egg} label="Egg" /></dd>{/if}
		{#if mob.name.fr && mob.name.fr !== name}<dt>In French</dt><dd>{mob.name.fr}</dd>{/if}
	</Infobox>

	{#if facts.spawns}
		<h2>Where it lives</h2>
		<p><Line line={facts.spawns} /></p>
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

	{#if facts.behaviour}
		<h2>Behaviour</h2>
		<p><Line line={facts.behaviour} /></p>
	{/if}

	{#if mob.id === 'Horse'}
		<h2>Coats</h2>
		<p>
			Each coat has its own average speed and endurance at a gallop; a wild horse is born within 1 block a
			second and 2 seconds of them. Its jump is drawn apart, from 1.3 to 3.2 blocks. See
			<a href="/guides/animals">riding</a>.
		</p>
		<ul class="coats">
			{#each COATS as c (c.name)}
				<li>
					<img src="/creatures/{c.model}.png" alt="A {c.name.toLowerCase()} horse" width="128" height="128" />
					<strong>{c.name}</strong>
					<span>{c.speed} blocks/s, {c.endurance} s</span>
				</li>
			{/each}
		</ul>
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
						<td class="n">{l.chance ? percent(l.chance) : (l.note ?? (l.min === 0 ? 'Not always' : 'Always'))}</td>
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

<style>
	.portrait {
		width: 220px;
		height: 220px;
		background: radial-gradient(circle at 50% 60%, rgb(255 240 220 / 0.08), transparent 65%);
	}
	.coats {
		list-style: none;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
		gap: 10px;
	}
	.coats li {
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
		padding: 8px;
		background: var(--panel);
		border: 2px solid var(--line);
	}
	.coats span {
		font-size: 0.9rem;
		color: var(--muted);
	}
</style>
