<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import { TIER_LEVEL } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<Page
	title="Tools"
	description="Rustcraft's tool tiers from wood to diamond: mining speed, durability, which blocks each pickaxe harvests."
	kicker="A tool breaks the blocks it is made for faster, and wears by 1 for each."
>
	{#snippet icon()}<Slot id="IronPickaxe" big link={false} />{/snippet}

	<p>
		Each kind of tool is made for some blocks: the pickaxe for stone and ores, the axe for wood, the shovel
		for dirt, sand and gravel, the hoe for leaves and plants. On those blocks it multiplies your speed;
		on the others it is no better than a hand. Stone, ores and metal blocks <strong>drop nothing</strong>
		unless broken with a pickaxe of a high enough tier.
	</p>

	<h2>Tiers</h2>
	<div class="wide">
		<table>
			<thead>
				<tr>
					<th>Tier</th>
					{#each data.kinds as k (k)}<th>{k}</th>{/each}
					<th class="n">Speed</th>
					<th class="n">Durability</th>
					<th>Harvests</th>
				</tr>
			</thead>
			<tbody>
				{#each data.tiers as t (t.tier)}
					<tr>
						<td><strong>{t.tier}</strong></td>
						{#each t.kinds as id, i (i)}<td>{#if id}<Slot {id} />{/if}</td>{/each}
						<td class="n">×{t.speed}</td>
						<td class="n">{t.durability}</td>
						<td>Level {t.level}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
	<p>
		Gold is the fastest of all, but lasts 32 blocks and harvests no more than wood. Bronze, made from
		copper and tin, sits between stone and iron: it lasts longer than stone and harvests the same.
		<a href="/enchanting">Efficiency</a> adds half the speed again per level, and Durability half the
		durability.
	</p>

	<h2>Which pickaxe for which block</h2>
	{#each data.needs as ids, level (level)}
		<h3>Level {level}: {TIER_LEVEL[level]} pickaxe or better</h3>
		<p class="slots">{#each ids as id (id)}<Slot {id} />{/each}</p>
	{/each}

	<h2>Other tools</h2>
	<ul>
		<li><ItemLink id="FlintAndSteel" /> lights fires and campfires.</li>
		<li><ItemLink id="FishingRod" />: see <a href="/guides/fishing">fishing</a>.</li>
		<li><ItemLink id="Bucket" /> carries water and lava, and milks cows.</li>
		<li><ItemLink id="Compass" /> points to your bedroll; the <ItemLink id="DepthsCompass" /> to the Ember Mother's lair.</li>
		<li><ItemLink id="PortalGun" /> opens a pair of portals on any flat wall, floor or ceiling, up to 32 blocks away.</li>
	</ul>
</Page>

<style>
	.slots {
		display: flex;
		flex-wrap: wrap;
		gap: 2px;
		max-width: none;
	}
	td {
		padding: 3px 8px;
	}
</style>
