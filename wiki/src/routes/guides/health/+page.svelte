<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';

	// A fall of `d` blocks takes one point per block beyond 3, rounded
	const fallDamage = (d: number) => Math.max(0, Math.round(d - 3));
	const FALLS = [3, 4, 5, 8, 10, 13, 15, 18, 20, 23];

	const EXHAUSTION: [string, string][] = [
		['Sprinting on the ground', '0.1 per block'],
		['Moving in water or lava', '0.01 per block'],
		['Jumping', '0.05'],
		['Jumping while sprinting', '0.2'],
		['Breaking a block', '0.005'],
		['A blow that lands on a creature', '0.1'],
		['Taking damage', '0.1 per hit'],
		['Healing 1 point of health', '6']
	];

	const DEATHS: [string, string][] = [
		['A fall', 'You hit the ground too hard'],
		['A fall within 3 s of a hit by a creature', 'A … threw you off a ledge'],
		['A creature', 'A … got you'],
		['An arrow', '… shot you, or An arrow got you'],
		['Fire or burning', 'You went up in flames'],
		['Lava', 'You tried to swim in lava'],
		['No air', 'You ran out of air'],
		['No food', 'You starved to death'],
		['The void', 'You fell out of the world']
	];
</script>

<Page
	title="Health and hunger"
	description="How health, hunger and saturation work in Rustcraft: regeneration, exhaustion costs, starvation, falls, drowning, fire and lava, death drops and respawning at a bedroll."
>
	{#snippet icon()}<Slot id="RoastBeef" big link={false} />{/snippet}

	<p>
		In survival you have <strong>10 hearts</strong> (20 points of health) and
		<strong>10 drumsticks</strong> of food (20 points). Food is what brings health back, and
		everything you do slowly uses it up. In creative you take no damage and never get hungry.
	</p>

	<h2>Health</h2>
	<p>
		Health comes back by itself, <strong>1 point every 4 seconds</strong>, but only while your food
		is at <strong>18 or more</strong> (9 full drumsticks). Each point healed this way costs 6
		exhaustion, so healing is what empties a full food bar fastest. Below 18 food, nothing heals
		you but potions and food effects.
	</p>
	<ul>
		<li>
			A <ItemLink id="HealingPotion" /> gives 4 points at once per level; a
			<ItemLink id="RegenerationPotion" /> heals over time.
		</li>
		<li>
			<ItemLink id="FishChowder" /> gives Regeneration II for 10 s, <ItemLink id="GrilledGlowfin" /> Regeneration
			I for 8 s. See <a href="/effects">status effects</a>.
		</li>
	</ul>

	<h3>Invulnerability</h3>
	<p>
		After a hit you ignore the next ones for <strong>0.5 seconds</strong>. The void, starvation
		and poison do not wait: they hurt you even then.
	</p>

	<h2>Hunger</h2>
	<p>
		Hunger has two parts. <strong>Food</strong> is the row of drumsticks you see.
		<strong>Saturation</strong> is a hidden reserve on top of it, spent first. You start, and
		respawn, with full food and 5 saturation.
	</p>
	<p>
		Your efforts add up as <strong>exhaustion</strong>. Every 4 exhaustion takes 1 point of
		saturation, or 1 point of food once the saturation is gone.
	</p>
	<table>
		<thead><tr><th>What you do</th><th class="n">Exhaustion</th></tr></thead>
		<tbody>
			{#each EXHAUSTION as [what, cost] (what)}
				<tr><td>{what}</td><td class="n">{cost}</td></tr>
			{/each}
		</tbody>
	</table>
	<p>
		Walking costs nothing, and neither does flying. Sprinting 40 blocks costs one point of
		saturation or food.
	</p>

	<h3>Sprinting and starving</h3>
	<ul>
		<li>You need <strong>7 food or more</strong> to sprint: at 6 or less (3 drumsticks) you cannot.</li>
		<li>
			With <strong>no food at all</strong> you starve: 1 damage every 4 seconds. Starvation
			<strong>can kill you</strong>.
		</li>
	</ul>

	<h2>Eating</h2>
	<p>
		Hold the food and press the use button (right click). Eating is instant: one item per use.
	</p>
	<ul>
		<li>You cannot eat with a full food bar, unless the food also gives an effect.</li>
		<li>
			Food goes up to 20. Saturation goes up, but never above your food: eat when your bar is low to
			keep all of it.
		</li>
		<li>
			Soups and stews such as <ItemLink id="MushroomSoup" /> give back their <ItemLink id="Bowl" />.
		</li>
		<li>
			A <ItemLink id="MilkBucket" /> clears every status effect and gives back the <ItemLink
				id="Bucket"
			/>. It cannot be drunk when you have no effect.
		</li>
	</ul>
	<p>What each food restores is on the <a href="/food">food page</a>.</p>

	<h2>Falls</h2>
	<p>
		A fall of 3 blocks or less does no harm. Beyond that you take <strong>1 point per block</strong>,
		rounded to the nearest block, so a fall of 23 blocks kills you from full health. Armour does not
		soften a landing.
	</p>
	<table>
		<thead><tr><th class="n">Fall (blocks)</th><th class="n">Damage</th><th class="n">Hearts</th></tr></thead>
		<tbody>
			{#each FALLS as d (d)}
				<tr>
					<td class="n">{d}</td>
					<td class="n">{fallDamage(d)}</td>
					<td class="n">{fallDamage(d) >= 20 ? 'All 10' : fallDamage(d) / 2}</td>
				</tr>
			{/each}
		</tbody>
	</table>
	<ul>
		<li>Landing in water, climbing a <ItemLink id="Ladder" /> or flying resets the fall.</li>
		<li>Riding a <ItemLink id="Horse" /> takes 3 blocks off your fall.</li>
		<li>
			A fatal fall within 3 seconds of a hit by a creature is blamed on that creature: "threw you off
			a ledge".
		</li>
	</ul>

	<h2>Air and drowning</h2>
	<p>
		You hold your breath for <strong>15 seconds</strong>, shown as 10 bubbles of 1.5 s each. Air
		runs out only while your <em>eyes</em> are in water or lava, and comes back in 2.5 s once you
		breathe again. With no air left you take <strong>2 damage every second</strong>.
	</p>
	<p>
		A <ItemLink id="WaterBreathingPotion" /> stops the air from running out. Sneaking lowers your eyes,
		so it can put your head under a surface you would otherwise breathe above.
	</p>

	<h2>Fire, lava and magma</h2>
	<table>
		<thead><tr><th>Source</th><th>Damage</th><th>Burning afterwards</th></tr></thead>
		<tbody>
			<tr>
				<td>Touching fire</td>
				<td>1 at once, then 1 every second while you burn</td>
				<td>At least 4 s</td>
			</tr>
			<tr>
				<td>Lava</td>
				<td>4 at once, then 4 every 0.5 s while you are in it</td>
				<td>At least 15 s</td>
			</tr>
			<tr>
				<td><ItemLink id="Magma" /></td>
				<td>As touching fire, while you stand on it without sneaking</td>
				<td>At least 4 s</td>
			</tr>
		</tbody>
	</table>
	<ul>
		<li>Lava also slows you to 30% of your speed.</li>
		<li>
			Touching water puts the fire out, and so does rain on your head under the open sky. Neither
			works while you are still in lava.
		</li>
		<li>Armour does reduce fire and lava damage.</li>
		<li>A <ItemLink id="FireResistancePotion" /> makes you immune to fire, lava and magma.</li>
	</ul>

	<h2>The void</h2>
	<p>
		Falling below <strong>Y −114</strong>, 50 blocks under the bottom of the world, kills you at
		once in survival, whatever your health and armour. In creative you are simply brought back to
		the world spawn.
	</p>

	<h2>Death</h2>
	<p>The death screen says what killed you:</p>
	<table>
		<thead><tr><th>Cause</th><th>Message</th></tr></thead>
		<tbody>
			{#each DEATHS as [cause, message] (cause)}
				<tr><td>{cause}</td><td>{message}</td></tr>
			{/each}
		</tbody>
	</table>
	<p>Poison alone never kills: it stops at 1 point of health.</p>

	<h3>What you drop</h3>
	<ul>
		<li>
			Everything you carried: every inventory slot, your armour, the crafting grid and the stack on
			your cursor. It lies where you died for <strong>5 minutes</strong>, then vanishes.
		</li>
		<li>
			All your experience is lost. An orb worth <strong>half of it</strong>, at most 400 points,
			is left where you died.
		</li>
		<li>The place of your death is marked as a waypoint on your map.</li>
		<li>In creative, nothing drops.</li>
	</ul>

	<h2>Respawn and the bedroll</h2>
	<p>
		You come back with full health, full food, 5 saturation and full air, no longer burning and with
		no status effects.
	</p>
	<p>
		You respawn at your <ItemLink id="Bedroll" /> if you have used one, or else at the world spawn.
		Using a bedroll, by day or by night, makes it your respawn point ("You will come back here").
		If the bedroll has been broken since, you go back to the world spawn.
	</p>
	<p>
		At night, using a bedroll also puts you to sleep. Once every player online has slept that night,
		the time jumps to sunrise. A night slept away does not count for the "Still Here at Dawn"
		achievement.
	</p>

	<h2>Tips</h2>
	<ul>
		<li>Keep your food at 18 or above before a fight, or you will not heal between blows.</li>
		<li>Roasted meat fills far more than raw meat: cook it in a <a href="/smelting">furnace</a>.</li>
		<li>
			Pour water down a long drop, or climb down a ladder, rather than jumping. See also
			<a href="/guides/combat">combat</a> for armour and <a href="/guides/depths">the depths</a> for the
			lava lakes.
		</li>
	</ul>
</Page>
