<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Infobox from '#lib/components/Infobox.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import Recipe from '#lib/components/Recipe.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import { duration, percent, range, TIER_LEVEL } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	let item = $derived(data.item);
	let block = $derived(data.block);
	let name = $derived(item.name.en ?? item.id);

	const SHOWN_USES = 12;
	let allUses = $state(false);
	let uses = $derived(allUses ? data.uses : data.uses.slice(0, SHOWN_USES));

	let description = $derived(
		`${name} in Rustcraft: ${[
			data.recipes.length ? 'how to craft it' : null,
			data.blockSources.length || data.mobSources.length ? 'where it drops' : null,
			data.uses.length ? `${data.uses.length} recipes that use it` : null
		]
			.filter(Boolean)
			.join(', ') || 'what it is and what it does'}.`
	);

	let obtained = $derived(
		data.recipes.length +
			data.smeltedFrom.length +
			data.blockSources.length +
			data.mobSources.length +
			data.fishing.length +
			data.alchemyMade.length +
			data.trades.filter((t) => 'item' in t.trade.goods && t.trade.goods.item === item.id).length
	);

	/// What a pickaxe of the right tier is called, for the block's line.
	const tierName = (level: number) => TIER_LEVEL[level] ?? `level ${level}`;
	const seconds = (hits: number, speed: number) => duration(Math.ceil(hits / speed) * 0.05);
</script>

<Page title={name} {description} kicker={item.name.fr && item.name.fr !== name ? `In French: ${item.name.fr}` : undefined}>
	{#snippet icon()}<Slot id={item.id} big link={false} />{/snippet}

	<Infobox title={name}>
		{#snippet picture()}<Slot id={item.id} big link={false} size={48} />{/snippet}
		<dt>Category</dt>
		<dd><a href="/items?category={item.category}">{item.category}</a></dd>
		<dt>Stacks to</dt>
		<dd>{item.maxStack}</dd>
		{#if item.tool}
			<dt>Tool</dt>
			<dd>{item.tool.tier} {item.tool.kind.toLowerCase()}</dd>
			<dt>Mining speed</dt>
			<dd>×{item.tool.speed}</dd>
			<dt>Harvests</dt>
			<dd>up to {tierName(item.tool.miningLevel).toLowerCase()} blocks</dd>
		{/if}
		{#if item.attackDamage > 1}
			<dt>Damage</dt>
			<dd>{item.attackDamage}</dd>
		{/if}
		{#if item.armour}
			<dt>Armour</dt>
			<dd>{item.armour.points} points ({item.armour.points * 4} % absorbed)</dd>
		{/if}
		{#if item.horseArmour}
			<dt>Horse armour</dt>
			<dd>{item.horseArmour} points</dd>
		{/if}
		{#if item.durability}
			<dt>Durability</dt>
			<dd>{item.durability}</dd>
		{/if}
		{#if item.food}
			<dt>Food</dt>
			<dd>+{item.food.hunger} hunger, +{item.food.saturation} saturation</dd>
		{/if}
		{#if item.foodEffect}
			<dt>Effect</dt>
			<dd>
				<a href="/effects#{item.foodEffect.effect}">{item.foodEffect.effect}</a>
				{item.foodEffect.level > 1 ? 'II' : 'I'}, {duration(item.foodEffect.seconds)}
			</dd>
		{/if}
		{#if item.potion}
			<dt>Effect</dt>
			<dd>
				<a href="/effects#{item.potion.effect}">{item.potion.effect}</a>{item.potion.seconds
					? `, ${duration(item.potion.seconds)}`
					: ''}
			</dd>
			<dt>Thrown</dt>
			<dd>{item.potion.splash ? 'Yes, 4-block burst' : 'No, drunk'}</dd>
		{/if}
		{#if block}
			{#if data.breakSeconds !== null}
				<dt>Breaks by hand in</dt>
				<dd>{duration(data.breakSeconds)}</dd>
			{:else}
				<dt>Breaks</dt>
				<dd>Never</dd>
			{/if}
			{#if block.requiredTool}
				<dt>Needs</dt>
				<dd>
					{tierName(block.requiredTool.level)}
					{block.requiredTool.kind.toLowerCase()} or better
				</dd>
			{:else if block.fasterWith.length}
				<dt>Faster with</dt>
				<dd>{block.fasterWith.map((k) => k.toLowerCase()).join(', ')}</dd>
			{/if}
			{#if block.light}
				<dt>Light</dt>
				<dd>{block.light} of 15</dd>
			{/if}
		{/if}
		{#if item.fuelSeconds}
			<dt>Fuel</dt>
			<dd>
				{duration(item.fuelSeconds)} ({+(item.fuelSeconds / data.cookSeconds).toFixed(2)} items)
			</dd>
		{/if}
		<dt>Command</dt>
		<dd><code>/give {item.slug} 1</code></dd>
	</Infobox>

	{#if data.notes}
		<!-- eslint-disable-next-line svelte/no-at-html-tags -->
		<div class="notes">{@html data.notes}</div>
	{/if}

	{#if data.spawns}
		<p class="note">
			<span>Used on the ground, it brings out a <ItemLink id={data.spawns} />. Spawn eggs come from the
				item browser of creative mode.</span>
		</p>
	{/if}

	<h2>How to get it</h2>
	{#if !obtained}
		<p>
			{#if item.category === 'Mobs' || item.id.endsWith('SpawnEgg')}
				Only in creative mode, from the item browser, or with <code>/give {item.slug}</code>.
			{:else if block && block.drops.some((d) => d.item === item.id)}
				Break it where the world puts it: it drops itself.
			{:else}
				No recipe makes it and nothing drops it: get it in creative mode or with
				<code>/give {item.slug}</code>.
			{/if}
		</p>
	{/if}

	{#if data.recipes.length}
		<h3>Crafting</h3>
		<div class="recipes">
			{#each data.recipes as recipe, i (i)}<Recipe {recipe} />{/each}
		</div>
	{/if}

	{#if data.smeltedFrom.length}
		<h3>Smelting</h3>
		<p>
			Smelt
			{#each data.smeltedFrom as id, i (id)}{i ? (i === data.smeltedFrom.length - 1 ? ' or ' : ', ') : ''}<ItemLink {id} />{/each}
			in a <ItemLink id="Furnace" />: {duration(data.cookSeconds)} each.
		</p>
	{/if}

	{#if data.alchemyMade.length}
		<h3>Alchemy</h3>
		{#each data.alchemyMade as a (a.ingredient)}
			<p>
				Distil <ItemLink id={a.ingredient} /> into <ItemLink id={a.vial} /> in an <ItemLink
					id="Alembic"
				/>: {duration(data.distillSeconds)}.
			</p>
		{/each}
	{/if}

	{#if data.blockSources.length}
		<h3>Breaking blocks</h3>
		<table>
			<thead><tr><th>Block</th><th class="n">Count</th><th class="n">Chance</th></tr></thead>
			<tbody>
				{#each data.blockSources as s (s.block)}
					<tr>
						<td>{#if s.item}<ItemLink id={s.item} />{:else}{s.block}{/if}</td>
						<td class="n">{s.count}</td>
						<td class="n">{percent(s.chance)}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{/if}

	{#if data.mobSources.length}
		<h3>Creatures</h3>
		<table>
			<thead><tr><th>Creature</th><th class="n">Count</th><th class="n">Chance</th></tr></thead>
			<tbody>
				{#each data.mobSources as s (s.mob)}
					<tr>
						<td><ItemLink id={s.mob} /></td>
						<td class="n">{range(s.min, s.max)}</td>
						<td class="n">{s.chance ? percent(s.chance) : (s.note ?? '100 %')}</td>
					</tr>
				{/each}
			</tbody>
		</table>
		<p>Creatures of the deeper tiers roll their loot more than once: see <a href="/guides/depths">the depths</a>.</p>
	{/if}

	{#if data.fishing.length}
		<h3>Fishing</h3>
		{#each data.fishing as f (f.pool)}
			<p>
				A <ItemLink id="FishingRod" /> brings back {f.count > 1 ? `${f.count} of them` : 'one'} in
				{percent((f.share * f.weight) / f.total)} of catches{f.night
					? ', and only at night'
					: ''}. See <a href="/guides/fishing">fishing</a>.
			</p>
		{/each}
	{/if}

	{#if data.trades.length}
		<h3>Trading</h3>
		<table>
			<thead><tr><th>Villager</th><th>Gives</th><th>For</th><th class="n">Per day</th></tr></thead>
			<tbody>
				{#each data.trades as t, i (i)}
					<tr>
						<td><ItemLink id={t.mob} /></td>
						<td>
							{#if 'item' in t.trade.goods}<ItemLink id={t.trade.goods.item} count={t.trade.goods.count} />{:else}Directions to a dungeon{/if}
						</td>
						<td><ItemLink id={t.trade.cost.item} count={t.trade.cost.count} /></td>
						<td class="n">{t.trade.stock}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{/if}

	{#if block && block.drops.length && !block.drops.every((d) => d.item === item.id && d.count === 1)}
		<h2>What it drops</h2>
		<p>Broken{block.requiredTool ? ' with the right pickaxe' : ''}, it gives one of:</p>
		<ul>
			{#each block.drops as d (d.item)}
				<li>
					<ItemLink id={d.item} count={d.count} />
					{#if block.drops.length > 1}({percent(
							d.chance / block.drops.reduce((s, x) => s + x.chance, 0)
						)}){/if}
				</li>
			{/each}
		</ul>
	{/if}

	{#if block && data.breakSeconds !== null && data.bestTools.length}
		<h2>Breaking it</h2>
		<div class="wide">
			<table>
				<thead><tr><th>Tool</th><th class="n">Time</th><th>Drops?</th></tr></thead>
				<tbody>
					<tr>
						<td>Bare hands</td>
						<td class="n">{seconds(block.breakTime ?? 0, 1)}</td>
						<td>{block.requiredTool ? 'No' : 'Yes'}</td>
					</tr>
					{#each data.bestTools as t (t.id)}
						<tr>
							<td><ItemLink id={t.id} /></td>
							<td class="n">{seconds(block.breakTime ?? 0, t.speed)}</td>
							<td>{t.harvests ? 'Yes' : 'No'}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
		<p>Each level of <a href="/enchanting">Efficiency</a> adds half the tool's speed again.</p>
	{/if}

	{#if data.uses.length || item.smeltsInto || data.alchemyUses.length || data.modifier || item.fuelSeconds || data.eatenBy.length || data.repairs.length}
		<h2>What it is for</h2>
	{/if}

	{#if data.uses.length}
		<h3>Crafting ({data.uses.length} {data.uses.length === 1 ? 'recipe' : 'recipes'})</h3>
		<div class="recipes">
			{#each uses as recipe, i (i)}<Recipe {recipe} />{/each}
		</div>
		{#if data.uses.length > SHOWN_USES && !allUses}
			<button class="more" onclick={() => (allUses = true)}>Show all {data.uses.length} recipes</button>
		{/if}
	{/if}

	{#if item.smeltsInto}
		<h3>Smelting</h3>
		<p>A <ItemLink id="Furnace" /> turns it into <ItemLink id={item.smeltsInto} /> in {duration(data.cookSeconds)}.</p>
	{/if}

	{#if item.fuelSeconds}
		<h3>Fuel</h3>
		<p>
			In a furnace it burns {duration(item.fuelSeconds)}: enough to smelt
			{+(item.fuelSeconds / data.cookSeconds).toFixed(2)}
			{item.fuelSeconds === data.cookSeconds ? 'item' : 'items'}.
		</p>
	{/if}

	{#if data.alchemyUses.length || data.modifier}
		<h3>Alchemy</h3>
		{#each data.alchemyUses as a (a.vial + a.ingredient)}
			<p>
				{#if a.ingredient === item.id}
					Distilled into <ItemLink id={a.vial} />, it makes <ItemLink id={a.result} />.
				{:else}
					Distil <ItemLink id={a.ingredient} /> into it to make <ItemLink id={a.result} />.
				{/if}
			</p>
		{/each}
		{#if data.modifier === 'longer'}
			<p>Distilled into a potion, it makes the potion last twice as long.</p>
		{:else if data.modifier === 'stronger'}
			<p>Distilled into a potion, it makes the potion a level stronger, for half the time.</p>
		{/if}
	{/if}

	{#if data.eatenBy.length}
		<h3>Feeding animals</h3>
		<p>
			{#each data.eatenBy as id, i (id)}{i ? (i === data.eatenBy.length - 1 ? ' and ' : ', ') : ''}<ItemLink {id} />{/each}
			eat it from your hand: see <a href="/guides/animals">taming and breeding</a>.
		</p>
	{/if}

	{#if data.repairs.length}
		<h3>Repairing</h3>
		<p>
			At an <ItemLink id="Anvil" />, it repairs
			{#each data.repairs as id, i (id)}{i ? (i === data.repairs.length - 1 ? ' and ' : ', ') : ''}<ItemLink {id} />{/each}.
		</p>
	{/if}

	{#if data.enchantments.length || data.ownRepair}
		<h2>Enchanting and repair</h2>
		{#if data.enchantments.length}
			<p>
				It takes
				{#each data.enchantments as e, i (e.id)}{i ? (i === data.enchantments.length - 1 ? ' and ' : ', ') : ''}<a href="/enchanting#{e.id}">{e.name}</a>{/each}
				at an <ItemLink id="EngravingTable" />.
			</p>
		{/if}
		{#if data.ownRepair}
			<p>An <ItemLink id="Anvil" /> repairs it with <ItemLink id={data.ownRepair} />: each one gives back a quarter of its durability.</p>
		{/if}
	{/if}
</Page>

<style>
	.more {
		font: inherit;
		font-family: var(--display);
		color: var(--fg);
		background: var(--slot);
		border: 2px solid var(--line);
		padding: 6px 14px;
		cursor: pointer;
	}
	.more:hover {
		border-color: var(--rust);
	}
	.notes {
		max-width: 72ch;
	}
</style>
