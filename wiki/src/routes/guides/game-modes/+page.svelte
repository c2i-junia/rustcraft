<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	// shared/src/world/hostiles.rs: `Difficulty::strike_damage`
	const easy = (d: number) => Math.min(d, Math.floor(d / 2) + 1);
	const hard = (d: number) => Math.floor((d * 3) / 2);
</script>

<Page
	title="Game modes"
	description="Survival and Creative in Rustcraft, the four difficulties and how much hostiles hurt in each, the world types (Default, Large biomes, Archipelago, Single biome), and how to switch."
>
	<p>
		A world is created with a game mode, a difficulty and a world type, all chosen in the
		<strong>New world</strong> dialog of the Singleplayer screen (<kbd>Ctrl</kbd> + <kbd>N</kbd>).
		The game mode and the difficulty can be changed later with a command; the world type cannot.
	</p>

	<h2>Survival and Creative</h2>
	<p>There are two game modes. Survival is the default.</p>
	<table>
		<thead><tr><th></th><th>Survival</th><th>Creative</th></tr></thead>
		<tbody>
			<tr><td>Damage</td><td>Falls, creatures, fire, lava, drowning and hunger hurt you</td><td>Nothing hurts you</td></tr>
			<tr><td>Hunger and air</td><td>Food goes down, air runs out under water</td><td>Never</td></tr>
			<tr><td>Health</td><td>Comes back only while food is 18 or more</td><td>Comes back on its own, 1 point every 4 s</td></tr>
			<tr><td>Flying</td><td>No</td><td>Yes, <kbd>F</kbd> toggles it; you fly through blocks</td></tr>
			<tr><td>Items</td><td>Mined, crafted, found</td><td>Taken from the item browser beside the inventory</td></tr>
			<tr><td>Enchanting and the anvil</td><td>Cost experience levels</td><td>Free</td></tr>
			<tr><td>Bows and crossbows</td><td>Use an arrow or bolt per shot</td><td>Need no ammunition</td></tr>
			<tr><td>Effects from creatures</td><td>Poison, slowness and others apply</td><td>Never; potions still work</td></tr>
			<tr><td>Death</td><td>You drop everything you carry</td><td>You drop nothing</td></tr>
			<tr><td>Achievements</td><td>Earned</td><td>Not earned; only play time is counted</td></tr>
		</tbody>
	</table>
	<p>
		In Creative, the catalog of the item browser gives items: a click puts a full stack on the
		cursor, a right click one item, and <kbd>Shift</kbd> + click a full stack into your inventory. A
		stack dropped back on the catalog is destroyed. Switching from Creative to Survival while flying
		puts you back on your feet.
	</p>

	<h3>Switching mode</h3>
	<p>
		Type <code>/gamemode creative</code> or <code>/gamemode survival</code> in the chat
		(<kbd>T</kbd>). It changes your own mode only. In a single-player world you can always use it;
		on a server it needs operator rights. See <a href="/guides/commands">Commands</a>.
	</p>
	<p>
		Each player's mode is saved with them. The mode chosen for the world is the one new players
		start in.
	</p>

	<h2>Difficulty</h2>
	<p>
		The difficulty belongs to the world and applies to everyone in it. It changes how hard hostile
		creatures strike, and whether they exist at all. Normal is the default.
	</p>
	<table>
		<thead><tr><th>Difficulty</th><th>What it does</th></tr></thead>
		<tbody>
			<tr><td>Peaceful</td><td>No hostile creatures. Those already in the world are removed. The Ember Mother never wakes.</td></tr>
			<tr><td>Easy</td><td>A blow of <var>d</var> deals half of it plus 1, rounded down, and never more than <var>d</var>.</td></tr>
			<tr><td>Normal</td><td>Blows deal their full damage.</td></tr>
			<tr><td>Hard</td><td>Blows deal half as much again, rounded down.</td></tr>
		</tbody>
	</table>
	<p>What some hostiles' blows become, in points of health (you have 20):</p>
	<table>
		<thead>
			<tr><th>Creature</th><th class="n">Easy</th><th class="n">Normal</th><th class="n">Hard</th></tr>
		</thead>
		<tbody>
			{#each data.blows as b (b.mob)}
				<tr>
					<td><ItemLink id={b.mob} /></td>
					<td class="n">{easy(b.damage)}</td>
					<td class="n">{b.damage}</td>
					<td class="n">{hard(b.damage)}</td>
				</tr>
			{/each}
		</tbody>
	</table>
	<p>
		Hostiles also hit harder the deeper you go: see <a href="/guides/depths">The depths</a>. Armour
		reduces these blows (see <a href="/armour">Armour</a>). Difficulty does not change falls,
		fire, drowning or hunger.
	</p>
	<p>
		Change it with <code>/difficulty peaceful</code>, <code>easy</code>, <code>normal</code> or
		<code>hard</code>. On Peaceful, the hostiles go on the next tick.
	</p>

	<h2>World types</h2>
	<table>
		<thead><tr><th>Type</th><th>What the world looks like</th></tr></thead>
		<tbody>
			<tr><td>Default</td><td>Continents, seas and every <a href="/biomes">biome</a> at their usual size.</td></tr>
			<tr><td>Large biomes</td><td>Biomes twice as large, and the continents with them.</td></tr>
			<tr><td>Archipelago</td><td>Islands scattered over a wide sea.</td></tr>
			<tr><td>Single biome</td><td>One biome everywhere.</td></tr>
		</tbody>
	</table>
	<p>
		The New world dialog offers Default, Large biomes and Archipelago. A single-biome world is made
		by a dedicated server, when it creates its world:
	</p>
	<pre><code>rustcraft-server --world-type single-biome --biome desert</code></pre>
	<p>
		The biome is named by its key, such as <code>plains</code>, <code>jungle</code> or
		<code>ice_spikes</code>. The server's <code>--world-type</code>, <code>--game-mode</code> and
		<code>--difficulty</code> options only apply to a world it creates: an existing world keeps its
		own. See <a href="/guides/multiplayer">Multiplayer</a>.
	</p>
	<p>
		The seed field of the New world dialog takes a number from 0 to 4294967295; left empty, the seed
		is random.
	</p>

	<h2>See also</h2>
	<ul>
		<li><a href="/guides/first-day">Your first day</a> in Survival</li>
		<li><a href="/guides/health">Health and hunger</a></li>
		<li><a href="/guides/controls">Controls</a></li>
	</ul>
</Page>
