<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const TIERS = ['The Caves', 'The Depths', 'The Furnace'];
</script>

<Page
	title="The depths"
	description="The three depth tiers of Rustcraft (the Caves, the Depths, the Furnace): their heights in new and old worlds, how much stronger hostiles get, the lava and bedrock levels, and the lush caves, crystal caves and basalt galleries."
>
	{#snippet icon()}<Slot id="Gloomrock" big link={false} />{/snippet}

	<p>
		The underground is cut into three tiers, each harder than the one above. The deeper you go, the
		stronger and more numerous the hostiles, and the better what they drop. The first time you reach a
		tier, its name appears on screen and you earn an <a href="/achievements">achievement</a>.
	</p>

	<h2>Two kinds of world</h2>
	<p>
		A <strong>new world</strong> is deep: its stone gives way to gloomrock under y 0 and goes down to
		bedrock at y −64. A world saved before the deep underground came in stays
		<strong>shallow</strong>, as it was generated: bedrock at y 0. The tiers, the lava and the ores sit
		at different heights in each.
	</p>
	<table>
		<thead>
			<tr><th></th><th>New world</th><th>Old world</th></tr>
		</thead>
		<tbody>
			<tr><td>Bedrock</td><td>y −64, rough up to −60</td><td>y 0</td></tr>
			<tr><td>Lowest caves</td><td>y −59</td><td>y 6</td></tr>
			<tr><td>Lava lakes</td><td>every cave at y −54 and below</td><td>every cave at y 10 and below</td></tr>
			<tr><td><ItemLink id="Gloomrock" /></td><td>below y 0, mixed with stone around −2 to 0</td><td>none</td></tr>
			<tr><td>Deep cave biomes</td><td>y −1 to −56</td><td>none</td></tr>
		</tbody>
	</table>
	<p>
		Caves are carved on floors about 48 blocks apart. Below y 48 they open into chambers, and in a new
		world they widen into big caverns below y 0. Bedrock cannot be broken. Fall below y −114 and you
		leave the world and die.
	</p>

	<h2>The three tiers</h2>
	<p>
		A place counts as underground only with real ground over it: water, leaves and plants don't hide
		the sky, so the bottom of a lake is not in a tier.
	</p>
	<table>
		<thead>
			<tr><th>Tier</th><th>New world</th><th>Old world</th><th class="n">Hostile strength</th></tr>
		</thead>
		<tbody>
			<tr><td>The Caves</td><td>below y 50</td><td>below y 50</td><td class="n">100 %</td></tr>
			<tr><td>The Depths</td><td>below y 0</td><td>below y 20</td><td class="n">150 %</td></tr>
			<tr><td>The Furnace</td><td>below y −40</td><td>below y 12</td><td class="n">200 %</td></tr>
		</tbody>
	</table>

	<h3>What strength changes</h3>
	<p>A hostile keeps the strength of the tier it appeared in. Strength multiplies, rounded up:</p>
	<ul>
		<li>its health, the damage of its blows, and the experience it gives;</li>
		<li>how many hostiles may be around you at once: 10, then 15, then 20 within 64 blocks;</li>
		<li>
			how often they appear: one spawn try every 2 seconds in the Caves, one plus a 50 % chance of a
			second in the Depths, two in the Furnace;
		</li>
		<li>
			how many times its loot is rolled when you kill it: once in the Caves, once plus a 50 % chance of
			twice in the Depths, twice in the Furnace;
		</li>
		<li>how many guards a <a href="/guides/structures">dungeon</a> may hold.</li>
	</ul>
	<p>On top of that, a kill deep down may drop a bonus:</p>
	<table>
		<thead>
			<tr><th>Tier</th><th>Bonus drop</th></tr>
		</thead>
		<tbody>
			<tr><td>The Caves</td><td>none</td></tr>
			<tr><td>The Depths</td><td><ItemLink id="IronIngot" />, 1 in 8</td></tr>
			<tr>
				<td>The Furnace</td>
				<td><ItemLink id="GoldIngot" />, 1 in 6, and <ItemLink id="Diamond" />, 1 in 20</td>
			</tr>
		</tbody>
	</table>

	<h3>The hostiles of the caves, tier by tier</h3>
	<p>Health, damage of a blow at Normal difficulty, and experience:</p>
	<div class="wide">
		<table>
			<thead>
				<tr>
					<th>Hostile</th>
					{#each TIERS as tier (tier)}<th class="n">{tier}</th>{/each}
				</tr>
			</thead>
			<tbody>
				{#each data.hostiles as h (h.id)}
					<tr>
						<td><ItemLink id={h.id} /></td>
						{#each h.tiers as t, i (i)}
							<td class="n">{t.health} HP · {t.damage} dmg · {t.xp} XP</td>
						{/each}
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
	<p>
		Under a roof and below y 50, the usual hostiles are <ItemLink id="Gnawer" label="Gnawers" /> (in packs of 2 to 4)
		and <ItemLink id="Roller" label="Rollers" />. From the Depths down, 1 spawn in 3 is a
		<ItemLink id="Cinderling" /> instead, which throws embers that set you burning.
	</p>

	<h2>Deep cave biomes</h2>
	<p>
		In a new world, the caves between y −1 and −56 belong to one of three biomes, or to none. Find the
		nearest with <code>/locate lush_caves</code>, <code>/locate crystal_caves</code> or
		<code>/locate basalt_galleries</code> (see <a href="/guides/commands">commands</a>).
	</p>

	<h3>Lush caves</h3>
	<p>
		Damp caves carpeted with <ItemLink id="Moss" /> on floors and ceilings, with patches of
		<ItemLink id="Clay" /> on the walls, small pools of water in the floor and
		<ItemLink id="CaveVine" label="cave vines" /> up to 4 blocks long hanging from the ceilings. Peaceful
		<ItemLink id="Mossback" label="Mossbacks" /> graze here; they appear with the cave and drop moss and sometimes
		leather. Moss makes potions last longer at the <a href="/alchemy">alembic</a>, and cave vine
		brews splash poison.
	</p>

	<h3>Crystal caves</h3>
	<p>
		Caverns walled with <ItemLink id="Geodestone" />, studded with glowing
		<ItemLink id="Glowcrystal" />, with <ItemLink id="CrystalCluster" label="crystal clusters" /> growing on the floor. Both
		break with anything and give <ItemLink id="CrystalShard" label="crystal shards" />. <ItemLink id="Shardling" label="Shardlings" /> guard
		these caves: some are there when the cave is found, and more appear in the dark on geodestone.
		The glow of the crystals keeps hostiles from appearing close to them.
	</p>

	<h3>Basalt galleries</h3>
	<p>
		Only below y −18, near the lava: galleries of <ItemLink id="Basalt" />, with
		<ItemLink id="Magma" /> on some floors and lava in some floor pits. <ItemLink id="Slagcrawler" label="Slagcrawlers" />
		appear in the dark on basalt and magma; their bite burns. Standing on magma burns you unless you
		sneak. Magma is the ingredient of the potion of fire resistance.
	</p>

	<h2>Hazards</h2>
	<ul>
		<li>
			<strong>Lava</strong> deals 4 damage at once and 4 more every half second you stay in it, slows
			you to a crawl, and sets you burning for 15 s after you get out. Water puts the burning out, but
			not while you are still in the lava.
		</li>
		<li>
			<strong>Fire</strong> sets you burning for 4 s, 1 damage a second. Cinderling embers and
			Slagcrawler bites burn too.
		</li>
		<li>
			<strong>Water</strong>: you have 15 s of air, then lose 2 health a second.
		</li>
		<li>
			<strong>Falls</strong> deal 1 damage per block beyond 3, and armour does not soften them.
		</li>
	</ul>

	<h2>How to prepare</h2>
	<ul>
		<li>
			<strong>Light.</strong> Any light source within 8 blocks, along each axis, stops hostiles from
			appearing: place a <ItemLink id="Torch" /> or <ItemLink id="Lantern" /> as you go, and you leave
			safe ground behind you.
		</li>
		<li>
			<strong>Armour.</strong> Each point of armour takes 4 % off mob blows, arrows, fire and lava. A
			full iron suit takes 60 % off. See <a href="/armour">armour</a> and
			<a href="/guides/combat">combat</a>.
		</li>
		<li>
			<strong>Fire resistance.</strong> The <ItemLink id="FireResistancePotion" /> (neutral base and
			magma) stops all fire, lava and magma damage for 3 minutes: drink it before the Furnace.
		</li>
		<li>
			<strong>Water breathing.</strong> The <ItemLink id="WaterBreathingPotion" /> (neutral base and
			<ItemLink id="Kelp" />) keeps your air full for 3 minutes.
		</li>
		<li>
			<strong>Night vision.</strong> The <ItemLink id="NightVisionPotion" /> (neutral base and
			<ItemLink id="RawGlowfin" />) brightens the world for 3 minutes.
		</li>
		<li>
			<strong>Water.</strong> A <ItemLink id="WaterBucket" /> puts out burning. A lava source that
			water touches turns into obsidian, flowing lava into cobblestone. Water also kills Cinderlings.
		</li>
		<li>
			<strong>A pickaxe that can mine the way back.</strong> Gold and diamonds need an iron pickaxe; see
			<a href="/guides/ores">ores</a>.
		</li>
		<li>
			<strong>A respawn point.</strong> Use a bedroll before you go down: you come back at it if you
			die. See <a href="/guides/health">health</a>.
		</li>
	</ul>
	<p>
		Down at the level of the lava waits the Ember Mother, in her lair. A
		<ItemLink id="DepthsCompass" /> points to it. See <a href="/guides/ember-mother">the Ember Mother</a>.
	</p>
</Page>
