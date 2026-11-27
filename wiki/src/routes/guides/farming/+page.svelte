<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import Recipe from '#lib/components/Recipe.svelte';
	import { num, percent, range } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<Page
	title="Farming"
	description="How to farm in Rustcraft: tilling soil with a hoe, planting rye, carrots and potatoes, growth times, watering by rain or water within 4 blocks, compost, berry bushes, saplings and the trees they grow, tall grass drops and the rain cistern."
>
	{#snippet icon()}<Slot id="Rye" big link={false} />{/snippet}

	<p>
		Three crops grow on tilled soil: <ItemLink id="Rye" />, <ItemLink id="Carrot" /> and
		<ItemLink id="Potato" />. They feed you, feed your animals, and sell to the farmer villager. Berry
		bushes and saplings grow on plain ground.
	</p>

	<h2>Getting seeds</h2>
	<p>
		Break tall grass. It is the only wild source of rye, carrots and potatoes; village fields and
		granaries hold rye too (see <a href="/guides/structures">structures</a>).
	</p>
	<table>
		<thead><tr><th>Tall grass drops</th><th class="n">Chance</th></tr></thead>
		<tbody>
			{#each data.grass as g (g.item)}
				<tr><td><ItemLink id={g.item} /></td><td class="n">{percent(g.share)}</td></tr>
			{/each}
		</tbody>
	</table>

	<h2>Tilling and planting</h2>
	<ol>
		<li>
			Use a hoe on <ItemLink id="Dirt" /> or <ItemLink id="Grass" /> with nothing on top of it. It turns
			into tilled soil, and the hoe wears by 1. Any hoe does it: wood, stone, bronze, iron, gold or
			diamond.
		</li>
		<li>
			Use rye, a carrot or a potato on the tilled soil to plant it. Carrots and potatoes are planted rather
			than eaten when you aim at tilled soil.
		</li>
	</ol>
	<div class="recipes">
		{#each data.hoes as recipe, i (i)}<Recipe {recipe} />{/each}
	</div>
	<p>Breaking tilled soil gives back dirt.</p>

	<h2>Growth</h2>
	<p>
		Crops grow only near a player: within 4 chunks, or your render distance if that is smaller. Each
		stage has a chance to pass on every random tick of its block, which comes about once every 51
		seconds. The times below are averages worked out from those odds; a single crop can be much faster
		or slower.
	</p>
	<table>
		<thead>
			<tr>
				<th>Crop</th>
				<th class="n">Stages</th>
				<th class="n">Average time</th>
				<th class="n">Watered</th>
				<th class="n">Ripe harvest</th>
			</tr>
		</thead>
		<tbody>
			{#each data.crops as c (c.seed)}
				<tr>
					<td><ItemLink id={c.seed} /></td>
					<td class="n">{c.stages}</td>
					<td class="n">{num(c.minutes, 1)} min</td>
					<td class="n">{num(c.minutes / 2, 1)} min</td>
					<td class="n">{range(c.harvest.min, c.harvest.max)}</td>
				</tr>
			{/each}
		</tbody>
	</table>
	<p>
		One stage takes about {num(data.stageMinutes, 1)} minutes, half that when watered. Breaking a crop
		before it is ripe gives back 1 seed, so nothing is lost.
	</p>

	<h3>Watering</h3>
	<p>A watered crop grows <strong>twice as fast</strong>. A crop is watered when either:</p>
	<ul>
		<li>rain falls on it (it sees the sky while it rains or snows), or</li>
		<li>
			water lies within <strong>4 blocks</strong> of its soil along both directions on the ground, at the
			soil's height or one block above.
		</li>
	</ul>
	<p>
		Both together do not make it faster still. A ditch of water through the middle of a field 9 blocks
		wide waters all of it. See <a href="/guides/weather">weather</a> for how often it rains.
	</p>

	<h3>Compost</h3>
	<div class="recipes">
		{#each data.compost as recipe, i (i)}<Recipe {recipe} />{/each}
	</div>
	<p>Use <ItemLink id="Compost" /> on a plant. One compost:</p>
	<ul>
		<li>moves a crop up one stage,</li>
		<li>ripens a berry bush,</li>
		<li>or grows a sapling into its tree at once, half of the time.</li>
	</ul>
	<p>
		<ItemLink id="Fiber" /> comes from tall grass and from reeds.
	</p>

	<h2>What to do with the harvest</h2>
	<ul>
		<li>Three rye make <ItemLink id="RyeBread" />.</li>
		<li>Bake potatoes into <ItemLink id="BakedPotato" label="Baked Potatoes" /> in a <a href="/smelting">furnace</a>.</li>
		<li>
			Feed rye to cows, sheep, goats and chickens; carrots to pigs, rabbits and horses; potatoes to pigs.
			See <a href="/guides/animals">animals</a>.
		</li>
		<li>
			Sell them to a farmer: 12 rye, 10 carrots or 10 potatoes for a coin. See
			<a href="/guides/trading">trading</a>.
		</li>
	</ul>
	<div class="recipes">
		{#each data.bread as recipe, i (i)}<Recipe {recipe} />{/each}
	</div>

	<h2>Berry bushes</h2>
	<p>
		Wild <ItemLink id="BerryBush" label="Berry Bushes" /> grow in forests that are not hot. Break one to take it, and
		plant it on dirt, grass or tilled soil. It ripens on its own in about 4.3 minutes on average (or at
		once with compost). Use a ripe bush to pick 2 <ItemLink id="Berries" />; it then starts over.
		Breaking a bush gives the bush, not its berries.
	</p>

	<h2>Saplings and trees</h2>
	<ul>
		<li>Breaking leaves gives their sapling 1 time in 12.</li>
		<li>
			<ItemLink id="OakLeaves" /> also give an <ItemLink id="AppleSapling" /> 1 time in 60.
		</li>
		<li>
			<ItemLink id="AppleLeaves" /> give an <ItemLink id="Apple" /> 1 time in 4 when broken. Now and then
			an apple also falls by itself from apple leaves with air under them.
		</li>
	</ul>
	<p>
		Plant a sapling on dirt, grass or tilled soil. If it has room, it grows into a tree in about 5
		minutes on average, or at once half of the time with compost.
	</p>
	<table>
		<thead><tr><th>Sapling</th><th>Grows into</th></tr></thead>
		<tbody>
			<tr><td><ItemLink id="OakSapling" /></td><td>An oak, trunk 3 to 5 blocks</td></tr>
			<tr><td><ItemLink id="SpruceSapling" /></td><td>The same shape, in spruce wood and leaves</td></tr>
			<tr><td><ItemLink id="WillowSapling" /></td><td>The same shape, in willow</td></tr>
			<tr><td><ItemLink id="AcaciaSapling" /></td><td>The same shape, in acacia</td></tr>
			<tr><td><ItemLink id="AppleSapling" /></td><td>The same shape, oak logs with apple leaves</td></tr>
			<tr><td><ItemLink id="JungleSapling" /></td><td>A jungle tree</td></tr>
		</tbody>
	</table>

	<h2>Reeds</h2>
	<p>
		<ItemLink id="Reed" label="Reeds" /> grow wild on sand by the sea. They do not grow back or spread; break them for
		<ItemLink id="Fiber" />. There is no sugar cane.
	</p>

	<h2>Rain cistern</h2>
	<div class="recipes">
		{#each data.cistern as recipe, i (i)}<Recipe {recipe} />{/each}
	</div>
	<p>
		A <ItemLink id="RainCistern" /> under the open sky fills up in the rain. Use a
		<ItemLink id="Bucket" /> on a full one to get a <ItemLink id="WaterBucket" />, and a water bucket on an
		empty one to fill it. It gives you water to bring to a field far from any lake.
	</p>
</Page>
