<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import Recipe from '#lib/components/Recipe.svelte';
	import Infobox from '#lib/components/Infobox.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<Page
	title="The Ember Mother"
	description="How to find and beat the Ember Mother, Rustcraft's boss: the depths compass, her lair at the lava level, her health per player, both phases of the fight with their damage, the Cinderlings she calls, what to bring, and the Ember Heart."
>
	{#snippet icon()}<Slot id="EmberHeart" big link={false} />{/snippet}

	<Infobox title="Ember Mother">
		{#snippet picture()}<Slot id="EmberHeart" big link={false} />{/snippet}
		<dt>Health</dt><dd>{data.base}, +80 per extra player</dd>
		<dt>Blow</dt><dd>7, every 1.5 s</dd>
		<dt>Ember</dt><dd>2, and burning</dd>
		<dt>Experience</dt><dd>500</dd>
		<dt>Drops</dt><dd><ItemLink id="EmberHeart" /></dd>
		<dt>Where</dt><dd>Her lair, at the lava level</dd>
	</Infobox>

	<p>
		The <ItemLink id="EmberMother" /> sleeps on an island in a lake of lava, in a cavern at the bottom of
		the world. There is one in every world that has dungeons, and once beaten she never comes back. She
		is the hardest fight in the game: come with your best armour and fire resistance.
	</p>

	<h2>Finding the lair</h2>
	<p>
		Craft a <ItemLink id="DepthsCompass" />: its needle points to the lair. It spins in a world without
		one, and once you stand within 4 blocks of the lair's centre. Getting one earns the achievement
		<a href="/achievements">It Points Down</a>.
	</p>
	<div class="recipes">
		{#each data.compass as recipe, i (i)}<Recipe {recipe} />{/each}
	</div>
	<ul>
		<li>
			The lair lies a few hundred blocks from the centre of the world: one of its coordinates, x or z, is
			between 320 and 704 blocks from 0. It is placed under high ground.
		</li>
		<li>
			Its floor is at the lava level: y −54 in a new world (y 10 in a world from before the deep
			underground). Dig down toward the needle, or follow the caves, which open onto the lair where they
			meet it. See <a href="/guides/depths">the depths</a> for what waits on the way.
		</li>
		<li>
			Coming within 40 blocks of it marks it on your map. Entering it earns
			<a href="/achievements">At Her Door</a>.
		</li>
	</ul>

	<h2>The lair</h2>
	<ul>
		<li>A round cavern 30 blocks in radius and 16 high, its walls wavering out to 36 blocks.</li>
		<li>
			In the middle, a lake of lava about 12 blocks in radius, with an island of 4 blocks where she sleeps.
			A causeway 3 blocks wide crosses the lake from two opposite sides (east and west).
		</li>
		<li>The ground around the lake is obsidian and slate.</li>
		<li>
			Eight braziers, a <ItemLink id="Campfire" /> on a pillar of <ItemLink id="HewnStone" />, stand in a
			ring 25 blocks from the centre.
		</li>
		<li>No creature spawns inside but the Cinderlings she calls.</li>
	</ul>

	<h2>Waking and sleeping</h2>
	<ul>
		<li>
			She wakes when a player in survival enters the lair. Creative players do not wake her, and she never
			wakes on Peaceful.
		</li>
		<li>Asleep, nothing hurts her.</li>
		<li>
			If nobody is inside for <strong>5 seconds</strong>, she falls asleep again, <strong>fully
			healed</strong>, and her Cinderlings, embers and fires vanish. Leaving to heal costs you the fight.
		</li>
		<li>Her health bar shows to every player in the lair or within 6 blocks of it.</li>
	</ul>

	<h3>Health and players</h3>
	<p>
		She has {data.base} health, plus 80 for each player beyond the first, counting the most players that
		were inside at once since she woke. More players also let her call more Cinderlings.
	</p>
	<table>
		<thead>
			<tr><th class="n">Players</th><th class="n">Health</th><th class="n">Cinderlings at once</th></tr>
		</thead>
		<tbody>
			{#each data.scaling as s (s.players)}
				<tr><td class="n">{s.players}{s.players === 5 ? ' or more' : ''}</td><td class="n">{s.health}{s.players === 5 ? ' +80 each' : ''}</td><td class="n">{s.cinderlings}</td></tr>
			{/each}
		</tbody>
	</table>

	<h2>Phase 1: the siege</h2>
	<p>While she has more than half her health, she fights from afar in cycles of 15 seconds.</p>
	<ul>
		<li>
			For 9 seconds she hovers on the shore, out of the lake, about 9 blocks from the nearest player.
			Every 2.25 seconds she throws a volley of <strong>3 embers</strong>: one at you, two within 1.5 blocks
			of you.
		</li>
		<li>
			An ember deals <strong>2</strong> (at Normal), sets you burning for 3 seconds, then gives you Burn I
			for 6 seconds. A shield raised toward it stops all of it.
		</li>
		<li>
			For the last <strong>6 seconds</strong> of each cycle she comes down to the ground and rests. This
			is your window for melee blows.
		</li>
		<li>
			Every 8 seconds she calls a <ItemLink id="Cinderling" /> out of the lake, unless enough are already
			out (see the table above).
		</li>
	</ul>

	<h3>Cinderlings</h3>
	<p>
		They hover 2 blocks up, keep about 6 blocks away and throw embers of their own. They have 10 health.
		Touching water kills them on the spot. Kill them between her volleys, or they pile up while you fight.
	</p>

	<h2>Phase 2: fury</h2>
	<p>Below half her health she stops calling Cinderlings and comes at you.</p>
	<ul>
		<li>
			She charges at 3.8 blocks a second and follows you up to 10 blocks above the ground, so pillars do not
			save you.
		</li>
		<li>
			In reach (2.4 blocks) she strikes for <strong>7</strong> with a strong knockback, every 1.5 seconds.
			Her blow knocks a raised shield down for 5 seconds.
		</li>
		<li>
			Every 2.5 seconds she sets fire to up to 10 cells of the floor 1.5 to 5 blocks around her. At most
			40 of her fires burn at once; the oldest go out.
		</li>
		<li>If you are more than 6 blocks away, she still throws volleys of embers at you.</li>
	</ul>

	<h3>At all times</h3>
	<ul>
		<li>Blows do not knock her back.</li>
		<li>
			She destroys every block in her way above the floor, except chests, barrels, furnaces, alembics and
			bedrock. You cannot wall her off.
		</li>
		<li>She stays within 24 blocks of the lair's centre.</li>
	</ul>

	<h3>Damage by difficulty</h3>
	<table>
		<thead><tr><th>Attack</th><th class="n">Easy</th><th class="n">Normal</th><th class="n">Hard</th></tr></thead>
		<tbody>
			<tr><td>Her blow</td><td class="n">4</td><td class="n">7</td><td class="n">10</td></tr>
			<tr><td>Her ember</td><td class="n">2</td><td class="n">2</td><td class="n">3</td></tr>
		</tbody>
	</table>
	<p>Armour takes its share off both. See <a href="/guides/combat">combat</a>.</p>

	<h2>How to fight her</h2>
	<h3>What to bring</h3>
	<ul>
		<li>
			<strong>Armour:</strong> a full suit of diamond (<ItemLink id="DiamondChestplate" /> and the rest)
			absorbs 76 % of each hit; iron, 60 %. See <a href="/armour">armour</a>.
		</li>
		<li>
			<strong><ItemLink id="FireResistancePotion" label="Potions of Fire Resistance" />:</strong> they make you immune to lava, fire and the
			burning of her embers (not the 2 of the hit itself). One lasts 180 seconds, 360 brewed with
			<ItemLink id="Moss" />. Bring several. See <a href="/alchemy">alchemy</a>.
		</li>
		<li>
			<strong><ItemLink id="HealingPotion" label="Potions of Healing" /></strong> and <ItemLink id="RegenerationPotion" label="Potions of Regeneration" />, since you
			cannot leave to heal without her healing too.
		</li>
		<li>
			<strong>Food</strong> that fills you up, like <ItemLink id="Stew" /> or <ItemLink id="RoastBeef" />:
			health only comes back on its own with a nearly full food bar.
		</li>
		<li>
			<strong>A <ItemLink id="Bow" /> and plenty of <ItemLink id="Arrow" label="Arrows" /></strong>, for the first phase,
			when she keeps away.
		</li>
		<li>
			<strong>A <ItemLink id="Shield" /></strong> to stop the embers. It will not hold her blow in fury.
		</li>
		<li>
			<strong>Your best sword</strong>, like a <ItemLink id="DiamondSword" />, with Sharpness if you can,
			and a <ItemLink id="StrengthPotion" /> for the fury.
		</li>
		<li>
			<strong>A <ItemLink id="WaterBucket" /></strong>: it kills a Cinderling it touches, and water poured
			on still lava turns it to obsidian.
		</li>
		<li>
			<strong>A <ItemLink id="Bedroll" /></strong> placed near the lair, so you come back close if you die.
			Your items drop where you fell and vanish after 5 minutes.
		</li>
	</ul>

	<h3>Tactics</h3>
	<ol>
		<li>Drink fire resistance before you step in, and stay on the shore, off the causeway.</li>
		<li>In the first phase, raise your shield toward her volleys and shoot her with the bow.</li>
		<li>When she lands to rest, run in and strike until she lifts off.</li>
		<li>Kill each Cinderling as it comes, before the next one joins it.</li>
		<li>
			In fury, keep moving away from her fires, strike after each of her blows, and drink healing early.
			She cannot be outrun for long: finish it.
		</li>
	</ol>

	<h2>Rewards</h2>
	<ul>
		<li>
			She drops the <ItemLink id="EmberHeart" />, a block that gives light 12, and an orb of
			<strong>500 experience</strong>.
		</li>
		<li>All the lava of the lair turns to <ItemLink id="Obsidian" />.</li>
		<li>Her Cinderlings, embers and fires vanish.</li>
		<li>
			Every player inside the lair sees a victory message and earns
			<a href="/achievements">The Last Ember</a>.
		</li>
		<li>She never comes back in that world.</li>
	</ul>
</Page>
