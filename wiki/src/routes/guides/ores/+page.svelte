<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import { range, TIER_LEVEL } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const minus = (n: number) => `${n}`.replace('-', '−');
	const span = (s: { from: number; to: number }) => `${minus(s.from)} to ${minus(s.to)}`;
</script>

<Page
	title="Ores"
	description="Where every ore lies in Rustcraft: coal, copper, tin, iron, gold and diamond heights in new and old worlds, how common they are, which pickaxe mines them and what they smelt into."
>
	{#snippet icon()}<Slot id="DiamondOre" big link={false} />{/snippet}

	<p>
		Ores are scattered through the stone (and the <ItemLink id="Gloomrock" /> under it) in small
		clusters and straight veins. Each ore has its own band of heights: coal, copper and tin are
		common and high, iron sits in the middle, gold and diamonds are deep and rare.
	</p>
	<p>
		The heights depend on the world. A <strong>new world</strong> is deep: its bedrock is at y −64.
		A world saved before the deep underground came in is <strong>shallow</strong>: its bedrock is at
		y 0, and its ores are squeezed into the stone above it. See <a href="/guides/depths">the depths</a>
		for the layers of each kind.
	</p>

	<h2>Every ore</h2>
	<p>
		"Tries" is how many clusters or veins of the ore start in each 16 × 16 × 16 chunk of stone in its
		band. A vein can stray up to 7 blocks from where it starts, so ore turns up a little beyond the
		band.
	</p>
	<div class="wide">
		<table>
			<thead>
				<tr>
					<th>Ore</th>
					<th>New world: y</th>
					<th class="n">Tries</th>
					<th>Old world: y</th>
					<th class="n">Tries</th>
					<th class="n">Cluster</th>
					<th class="n">Vein</th>
					<th class="n">Veins</th>
				</tr>
			</thead>
			<tbody>
				{#each data.ores as ore (ore.block)}
					<tr>
						<td><ItemLink id={ore.block} /></td>
						<td>{span(ore.deep)}</td>
						<td class="n">{ore.deep.tries}</td>
						<td>{span(ore.shallow)}</td>
						<td class="n">{ore.shallow.tries}</td>
						<td class="n">{range(...ore.cluster)}</td>
						<td class="n">{ore.vein ? range(...ore.vein) : '–'}</td>
						<td class="n">{Math.round(ore.veins * 100)} %</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
	<p>
		In a new world, gold and diamonds are not spread evenly over their band. Gold keeps 4 tries in 5
		at y −32 and below, but only 2 in 5 above. Diamonds keep 4 tries in 5 at y −42 and below, and only
		6 in 100 above: almost all of them lie between y −58 and −42.
	</p>
	<p>
		Copper and tin never take the place of another ore. The other ores can: a rarer ore wins a block
		that two of them reach.
	</p>

	<h2>Mining and smelting</h2>
	<p>
		Every ore needs a pickaxe of at least the tier below. With a weaker one, or with no pickaxe, the
		block breaks and drops nothing.
	</p>
	<table>
		<thead>
			<tr><th>Ore</th><th>Pickaxe needed</th><th>Drops</th><th>Smelts into</th></tr>
		</thead>
		<tbody>
			{#each data.ores as ore (ore.block)}
				<tr>
					<td><ItemLink id={ore.block} /></td>
					<td>{ore.level === null ? 'Any' : TIER_LEVEL[ore.level]}</td>
					<td><ItemLink id={ore.drop} /></td>
					<td>{#if ore.smelted}<ItemLink id={ore.smelted} />{:else}–{/if}</td>
				</tr>
			{/each}
		</tbody>
	</table>
	<ul>
		<li>
			Coal ore and diamond ore give their resource straight away, and experience with it: 1 point for
			coal, 5 for a diamond.
		</li>
		<li>
			Copper, tin, iron and gold ore drop themselves. Smelt them in a <ItemLink id="Furnace" /> (10 s
			each); their experience comes out of the furnace instead.
		</li>
		<li>
			Tin needs a stone pickaxe, so a <ItemLink id="StonePickaxe" /> comes before bronze: three copper
			ingots and one tin ingot make 4 <ItemLink id="BronzeIngot" />.
		</li>
		<li>Gold and diamonds both need an <ItemLink id="IronPickaxe" />.</li>
	</ul>

	<h2>Slate and limestone</h2>
	<p>
		Two rocks grow in large pockets of 20 to 27 blocks rather than in clusters, in both kinds of
		world. They take any pickaxe and drop themselves, for building.
	</p>
	<table>
		<thead>
			<tr><th>Rock</th><th>y</th><th class="n">Pockets per chunk</th></tr>
		</thead>
		<tbody>
			<tr><td><ItemLink id="Slate" /></td><td>5 to 30</td><td class="n">12</td></tr>
			<tr><td><ItemLink id="Limestone" /></td><td>40 to 100</td><td class="n">8</td></tr>
		</tbody>
	</table>
	<p>
		Below y 0 in a new world the rock is <ItemLink id="Gloomrock" />. Mined, it drops
		<ItemLink id="CobbledGloomrock" />, which smelts back into gloomrock.
	</p>

	<h2>Finding diamonds</h2>
	<ul>
		<li>
			In a new world, dig to <strong>y −42 and below</strong>, down to −58. That is inside the
			<a href="/guides/depths">Furnace tier</a> (below y −40), where hostiles are twice as strong, and
			just above the lava that fills every cave at y −54 and below.
		</li>
		<li>In an old world, diamonds lie between y 5 and 16.</li>
		<li>
			A diamond cluster is 2 to 6 blocks and there is at most one try per chunk: mine long tunnels at
			one height rather than digging down.
		</li>
		<li>
			Diamonds also turn up in chests. A sanctuary chest holds 1 to 3 diamonds 55 % of the time, a
			buried treasure 60 % of the time (see <a href="/guides/structures">structures</a>). A hostile
			killed in the Furnace tier drops one 1 time in 20.
		</li>
	</ul>

	<h2>Finding gold</h2>
	<ul>
		<li>
			In a new world, gold lies from y −60 to −8, and is twice as common at y −32 and below. In an old
			world, look between y 5 and 32.
		</li>
		<li>
			Hostiles killed in the Furnace tier drop a <ItemLink id="GoldIngot" /> 1 time in 6, and the chests
			of sanctuaries and buried treasures are rich in gold ingots.
		</li>
	</ul>

	<h2>Gravel and flint</h2>
	<p>
		<ItemLink id="Gravel" /> breaks with anything, faster with a shovel, and drops a
		<ItemLink id="Flint" /> instead of itself 1 time in 10. Gravel covers the beds of deeper seas and
		lakes. Flint makes <ItemLink id="Arrow" label="arrows" />, the <ItemLink id="Spear" /> and
		<ItemLink id="FlintAndSteel" />.
	</p>
	<p>
		Flint also drops from <ItemLink id="Roller" label="Rollers" /> (1 in 5) and <ItemLink id="Cinderling" label="Cinderlings" /> (1 in 10).
	</p>
</Page>
