<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import { percent, range } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const dungeon = (name: string) => data.dungeons.find((d) => d.name === name)!;
</script>

{#snippet lootTable(loot: { item: string; chance: number; min: number; max: number }[])}
	<table>
		<thead>
			<tr><th>Item</th><th class="n">Chance</th><th class="n">Count</th></tr>
		</thead>
		<tbody>
			{#each loot as row (row.item)}
				<tr>
					<td><ItemLink id={row.item} /></td>
					<td class="n">{percent(row.chance)}</td>
					<td class="n">{range(row.min, row.max)}</td>
				</tr>
			{/each}
		</tbody>
	</table>
{/snippet}

<Page
	title="Structures"
	description="Every structure of Rustcraft: villages and their styles, the ruin, crypt and sanctuary dungeons with their guards and loot, shipwrecks, treasure maps and buried treasure, and how to find them with /locate."
>
	{#snippet icon()}<Slot id="Chest" big link={false} />{/snippet}

	<p>
		The world holds villages on the surface, three kinds of dungeon from the surface down to the lava,
		shipwrecks on the sea floor with treasure buried near them, and the lair of the Ember Mother. Their
		chests are filled when the world is generated.
	</p>

	<h2>How loot works</h2>
	<p>
		Each chest and barrel has 27 slots. Every row of its table is rolled on its own: first the chance,
		then a count between the two numbers. A chest can hold many rows or none. What a container holds is
		fixed by the world's seed and its position.
	</p>

	<h2>Villages</h2>
	<p>
		There is at most one village in each 512 × 512 block area, with a chance of 4 in 5. A village
		needs dry, fairly flat land below height 76, well inside a biome that has a village style, away
		from giant jungle trees. It reaches at most 64 blocks from its centre.
	</p>

	<h3>Styles</h3>
	<table>
		<thead>
			<tr><th>Style</th><th>Biomes</th><th>Built of</th></tr>
		</thead>
		<tbody>
			{#each data.styles as s (s.style)}
				<tr>
					<td>{s.style}</td>
					<td>
						{#each s.biomes as b, i (b.slug)}{i ? ', ' : ''}<a href="/biomes/{b.slug}">{b.name}</a>{/each}
					</td>
					<td>{s.look}</td>
				</tr>
			{/each}
		</tbody>
	</table>
	<p>Jungles, mountains, ice spikes, seas and rivers have no villages.</p>

	<h3>Buildings</h3>
	<p>
		A well stands in a square at the centre. Streets run out from 2 to 4 sides of it, branching up to 5
		times, with a lamp every 9 blocks. Up to 14 buildings line them:
	</p>
	<table>
		<thead>
			<tr><th>Building</th><th class="n">How common</th><th class="n">Most per village</th></tr>
		</thead>
		<tbody>
			<tr><td>Small house</td><td class="n">40</td><td class="n">any</td></tr>
			<tr><td>Field (tilled soil with rye)</td><td class="n">15</td><td class="n">4</td></tr>
			<tr><td>Big house</td><td class="n">14</td><td class="n">any</td></tr>
			<tr><td>Granary</td><td class="n">10</td><td class="n">2</td></tr>
			<tr><td>Pen</td><td class="n">9</td><td class="n">2</td></tr>
			<tr><td>Forge</td><td class="n">7</td><td class="n">1</td></tr>
		</tbody>
	</table>
	<p>"How common" is the weight each building is drawn with, out of 95.</p>

	<h3>Pens</h3>
	<p>Each pen holds 2 to 4 animals, picked from those of the village's style:</p>
	<ul>
		<li>Plains: <ItemLink id="Sheep" />, <ItemLink id="Pig" />, <ItemLink id="Chicken" />, <ItemLink id="Cow" /></li>
		<li>Savanna: <ItemLink id="Cow" />, <ItemLink id="Goat" />, <ItemLink id="Chicken" /></li>
		<li>Desert: <ItemLink id="Goat" />, <ItemLink id="Chicken" /></li>
		<li>Snowy: <ItemLink id="Sheep" />, <ItemLink id="Rabbit" /></li>
	</ul>

	<h3>Villagers</h3>
	<p>
		Every house, small or big, has one villager. Their trades follow the order the houses were built:
		<ItemLink id="Farmer" />, <ItemLink id="Smith" />, <ItemLink id="Cartographer" />, then again. They
		wander round their home by day and go in at night. Nothing can hurt them and hostiles ignore them.
		The cartographer sells directions to the nearest dungeon. See <a href="/guides/trading">trading</a>.
	</p>

	<h3>Village loot</h3>
	{#each data.village as v (v.name)}
		<h4>{v.name}</h4>
		{@render lootTable(v.loot)}
	{/each}

	<h2>Dungeons</h2>
	<p>
		Each 256 × 256 block area may hold one dungeon of each kind. Their loot gets better the deeper they
		lie. Worlds saved before dungeons existed have none.
	</p>
	<table>
		<thead>
			<tr>
				<th>Dungeon</th>
				<th class="n">Chance per area</th>
				<th>Where</th>
				<th>Containers</th>
				<th>Guards</th>
			</tr>
		</thead>
		<tbody>
			<tr>
				<td>Ruin</td>
				<td class="n">50 %</td>
				<td>On flat, dry land, below the mountains, away from villages</td>
				<td>3 chests, 3 barrels</td>
				<td>4 <ItemLink id="Bramble" />, 3 <ItemLink id="Gnawer" /></td>
			</tr>
			<tr>
				<td>Crypt</td>
				<td class="n">75 %</td>
				<td>Off a cave wall, y 4 to 50 (y 20 to 50 in an old world)</td>
				<td>2 chests, 1 barrel</td>
				<td>6 <ItemLink id="Gnawer" /></td>
			</tr>
			<tr>
				<td>Sanctuary</td>
				<td class="n">60 %</td>
				<td>Off a cave by the lava, y −55 to −47 (y 9 to 17 in an old world)</td>
				<td>3 chests</td>
				<td>4 <ItemLink id="Roller" />, 2 <ItemLink id="Cinderling" /></td>
			</tr>
		</tbody>
	</table>
	<ul>
		<li>
			<strong>Ruin</strong>: a roofless hall with broken walls. A <ItemLink id="Ladder" /> goes down to a
			cellar.
		</li>
		<li><strong>Crypt</strong>: a nave between two rows of tombs, with a chamber behind it.</li>
		<li>
			<strong>Sanctuary</strong>: a hall of <ItemLink id="Obsidian" /> pillars between two channels of
			lava, an altar at the end. It also holds a <ItemLink id="GoldBlock" /> and two
			<ItemLink id="BronzeBlock" />. It lies in the <a href="/guides/depths">Furnace tier</a>, so its
			guards are twice as strong.
		</li>
	</ul>
	<p>The area that holds the Ember Mother's lair has no crypt and no sanctuary.</p>

	<h3>Guards</h3>
	<p>
		Each guard has its own spot in the dungeon's plan. A dungeon wakes while a player is within 36
		blocks of it. Every 2 seconds it then has a 1 in 4 chance to send a guard to one of its spots,
		more in the deeper tiers (times the tier's strength).
	</p>
	<ul>
		<li>Guards need no darkness, unlike other hostiles.</li>
		<li>
			A guard never appears within 8 blocks of a player, nor where a player can see the spot.
		</li>
		<li>
			A dungeon holds at most 5 hostiles at once (fewer if it has fewer spots), times the tier's
			strength.
		</li>
		<li>Nothing spawns on Peaceful.</li>
	</ul>
	<div class="note">
		<p>
			<strong>To stop the guards</strong>, put a <ItemLink id="Torch" />, a <ItemLink id="Lantern" />
			or a lit <ItemLink id="Campfire" /> within 3 blocks of each spot, or fill the spot with any
			block.
		</p>
	</div>

	<h3>Ruin loot</h3>
	{@render lootTable(dungeon('Ruin').loot)}
	<h3>Crypt loot</h3>
	{@render lootTable(dungeon('Crypt').loot)}
	<h3>Sanctuary loot</h3>
	{@render lootTable(dungeon('Sanctuary').loot)}

	<h2>Shipwrecks and buried treasure</h2>
	<p>
		Each 320 × 320 block area has a 70 % chance of a shipwreck, on a sea floor 9 to 40 blocks deep.
		Its chest, under the deck of the stern, always holds a <ItemLink id="TreasureMap" /> besides its
		loot.
	</p>
	<p>
		The map leads to a chest buried 2 blocks under the ground, 24 to 72 blocks from the wreck, on a
		shore or in shallow water when one is near. While you carry the map, the spot is marked with a red
		cross named "X" on your maps. Dig there to find the chest.
	</p>
	{#each data.sea as s (s.name)}
		<h3>{s.name}</h3>
		{@render lootTable(s.loot)}
	{/each}

	<h2>The Ember Mother's lair</h2>
	<p>
		Each world with dungeons has one lair, a great cavern at the level of the lava around a lake of lava, where
		the boss of the game sleeps. A <ItemLink id="DepthsCompass" /> points to it. See
		<a href="/guides/ember-mother">the Ember Mother</a>.
	</p>

	<h2>Finding structures</h2>
	<p>
		Type <code>/locate</code> followed by a name to get the nearest one (see
		<a href="/guides/commands">commands</a>):
	</p>
	<table>
		<thead>
			<tr><th>Structure</th><th>Command</th></tr>
		</thead>
		<tbody>
			<tr><td>Village</td><td><code>/locate village</code></td></tr>
			<tr><td>Ruin</td><td><code>/locate ruin</code></td></tr>
			<tr><td>Crypt</td><td><code>/locate crypt</code></td></tr>
			<tr><td>Sanctuary</td><td><code>/locate sanctuary</code></td></tr>
			<tr><td>Shipwreck</td><td><code>/locate shipwreck</code></td></tr>
		</tbody>
	</table>
	<p>
		Without commands, explore: a village is added to your waypoints when you come within 48 blocks of
		it, a dungeon within 8 blocks, the lair within 40. The cartographer also sells directions to the
		nearest dungeon.
	</p>
</Page>
