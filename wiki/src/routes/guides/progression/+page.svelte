<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import Recipe from '#lib/components/Recipe.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	// What a pickaxe of each mining level opens up (the required tools of the ore blocks)
	const MINES = ['Stone, coal, copper', '+ tin, iron', '+ gold, diamond', '+ obsidian'];
</script>

<Page
	title="Progression"
	description="The road through Rustcraft from wood to the Ember Mother: tool and armour tiers, bronze, iron and diamond, the three depth tiers, the furnace, enchanting, alchemy, the depths compass, and all 21 achievements in order."
>
	{#snippet icon()}<Slot id="DepthsCompass" big link={false} />{/snippet}

	<p>
		Rustcraft has no levels to grind: you get stronger by making better tools and armour, and the
		better materials lie deeper. The story ends at the bottom of the world, in the lair of the
		<ItemLink id="EmberMother" />. This page is the map of that road; each step links to the guide
		that covers it.
	</p>

	<h2>Tool tiers</h2>
	<p>
		Every tool comes in each material. A pickaxe can only mine an ore if its tier is high enough:
		mined with a lower one, the block breaks and drops nothing. Speed is how many times faster than
		your bare hand it breaks the blocks it is made for.
	</p>
	<div class="wide">
		<table>
			<thead>
				<tr>
					<th>Tier</th><th>Made of</th><th>Pickaxe mines</th><th class="n">Speed</th><th class="n">Uses</th><th class="n">Sword damage</th>
				</tr>
			</thead>
			<tbody>
				{#each data.tools as t (t.tier)}
					<tr>
						<td><ItemLink id={t.pickaxe} label={t.tier} /></td>
						<td><ItemLink id={t.material} /></td>
						<td>{MINES[t.level]}</td>
						<td class="n">×{t.speed}</td>
						<td class="n">{t.durability}</td>
						<td class="n">{t.damage}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
	<p>
		Gold is the odd one: the fastest tool of all, but it wears out after 32 uses and mines only what
		wood mines. Bronze is a step between stone and iron: it mines the same ores as stone, but lasts
		longer and digs faster. Each pickaxe also mines everything the tiers under it mine.
	</p>
	<p>
		See <a href="/tools">Tools</a> and <a href="/weapons">Weapons</a> for every item, and
		<a href="/guides/ores">Ores</a> for where each ore lies.
	</p>

	<h2>Armour tiers</h2>
	<p>
		Each point of armour takes 4 % off the blows of creatures, arrows, fire and lava. Falls,
		drowning, hunger and poison go through it. Every piece wears by 1 each time it softens a blow.
	</p>
	<div class="wide">
		<table>
			<thead>
				<tr><th>Material</th><th>Helmet</th><th>Chestplate</th><th>Leggings</th><th>Boots</th><th class="n">Full suit</th><th class="n">Blows absorbed</th></tr>
			</thead>
			<tbody>
				{#each data.armour as a (a.material)}
					<tr>
						<td>{a.material}</td>
						{#each a.pieces as p (p.id)}
							<td><ItemLink id={p.id} label={`${p.points} pt${p.points > 1 ? 's' : ''}, ${p.durability}`} /></td>
						{/each}
						<td class="n">{a.points}</td>
						<td class="n">{a.points * 4} %</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
	<p>
		Each cell gives the armour points and the durability of the piece. <ItemLink id="Leather" /> comes
		from cows. There is no gold armour. See <a href="/armour">Armour</a>.
	</p>

	<h2>The road, step by step</h2>

	<h3>1. Wood and stone</h3>
	<p>
		Your first day: logs, a crafting table, a wooden pickaxe, then stone tools and a
		<ItemLink id="Furnace" />. A furnace smelts one item every 10 seconds and is the start of every
		metal. Follow <a href="/guides/first-day">Your first day</a>.
	</p>

	<h3>2. Bronze</h3>
	<p>
		<ItemLink id="CopperOre" /> is common from height 96 down, and any pickaxe mines it.
		<ItemLink id="TinOre" /> needs a stone pickaxe. Smelt both, then mix 3 copper ingots and 1 tin
		ingot into 4 bronze ingots, in the inventory grid.
	</p>
	<div class="recipes"><Recipe recipe={data.recipes.bronze} /></div>
	<p>Bronze gives the first metal tools and armour.</p>

	<h3>3. Iron</h3>
	<p>
		<ItemLink id="IronOre" /> needs a stone or bronze pickaxe and lies from height 72 down to −24.
		Iron opens up the bucket, the shield, the lantern and the <ItemLink id="Anvil" />, and an iron
		pickaxe mines gold and diamond.
	</p>

	<h3>4. Into the depths</h3>
	<p>
		The underground is split into three tiers. Each one deeper makes hostiles stronger: more health,
		harder blows, more of them, and better loot. The first time you reach a tier, its name shows on the screen.
	</p>
	<table>
		<thead><tr><th>Tier</th><th>Below height</th><th class="n">Hostile strength</th><th>Found there</th></tr></thead>
		<tbody>
			<tr><td>The Caves</td><td class="n">50</td><td class="n">100 %</td><td>Coal, copper, tin, iron; crypts</td></tr>
			<tr><td>The Depths</td><td class="n">0</td><td class="n">150 %</td><td>Gloomrock, gold, the deep cave biomes, Cinderlings</td></tr>
			<tr><td>The Furnace</td><td class="n">−40</td><td class="n">200 %</td><td>Diamond, lava lakes from −54, sanctuaries, the lair</td></tr>
		</tbody>
	</table>
	<p>
		A tier only counts with rock over your head: a deep sea floor is not underground. In a world made
		before the deep underground existed, the Depths start below 20 and the Furnace below 12. See
		<a href="/guides/depths">The depths</a>.
	</p>

	<h3>5. Gold and diamond</h3>
	<p>
		<ItemLink id="GoldOre" /> lies from −8 down to −60, most of it below −32. <ItemLink
			id="DiamondOre"
		/> lies below 16 but is mostly found below −42, in the Furnace tier. Both need an iron pickaxe.
		Diamond makes the best tools and armour, and with gold, the depths compass.
	</p>

	<h3>6. Enchanting</h3>
	<p>
		The engraving table needs a diamond and <ItemLink id="Obsidian" />, which only a diamond pickaxe
		mines. Obsidian forms where water meets a lava source.
	</p>
	<div class="recipes"><Recipe recipe={data.recipes.engraving} /></div>
	<p>
		Enchantments cost experience levels: 3 for level I, 5 more for II, 8 more for III. See
		<a href="/enchanting">Enchanting</a>.
	</p>

	<h3>7. Alchemy</h3>
	<p>
		The alembic brews potions: healing, night vision, water breathing, fire resistance, swiftness,
		strength, regeneration, and splash potions to throw. Fire resistance is the one to bring to the
		Furnace tier.
	</p>
	<div class="recipes"><Recipe recipe={data.recipes.alembic} /></div>
	<p>See <a href="/alchemy">Alchemy</a>.</p>

	<h3>8. The depths compass</h3>
	<p>
		Four gold ingots around a diamond. It points to the lair of the Ember Mother, the one lair of the
		world, and spins once you stand at its centre.
	</p>
	<div class="recipes"><Recipe recipe={data.recipes.compass} /></div>

	<h3>9. The lair</h3>
	<p>
		The lair is a great cavern at the lava level, with the Ember Mother asleep on an island in a lava
		lake. She wakes when you enter, has 200 health (80 more for each extra player), throws embers,
		summons <ItemLink id="Cinderling" />s, and charges once she is down to half. Beaten, she drops the
		Ember Heart and turns the lair's lava to obsidian. Prepare with diamond gear and fire
		resistance: see <a href="/guides/ember-mother">The Ember Mother</a>.
	</p>

	<h2>Along the way</h2>
	<ul>
		<li><a href="/guides/structures">Structures</a>: villages, ruins, crypts and sanctuaries hold metal, tools and diamonds.</li>
		<li><a href="/guides/trading">Trading</a>: villagers sell iron tools and armour for <ItemLink id="Coin" />s.</li>
		<li><a href="/guides/farming">Farming</a> and <a href="/guides/animals">Animals</a>: steady food, leather and wool.</li>
		<li><a href="/guides/combat">Combat</a>: bows, shields, critical hits.</li>
	</ul>

	<h2>Achievements</h2>
	<p>
		The 21 achievements follow the same road, and the game lists them in this order. They are earned
		in Survival only; open them from the game menu (<kbd>Esc</kbd>), under Progress. See
		<a href="/achievements">Achievements</a>.
	</p>
	<ol class="achievements">
		{#each data.achievements as a (a.id)}
			<li>
				<Slot id={a.icon} link={false} />
				<span><strong>{a.title}</strong><br />{a.goal}</span>
			</li>
		{/each}
	</ol>
	<p>
		A night slept away does not count for <em>Still Here at Dawn</em>: you must be alive from the
		start of the night to dawn. <em>The Last Ember</em> goes to every player in the lair when the
		Ember Mother falls.
	</p>
</Page>

<style>
	.achievements {
		list-style: none;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
		gap: 10px 20px;
		counter-reset: step;
	}
	.achievements li {
		display: flex;
		align-items: center;
		gap: 12px;
		counter-increment: step;
	}
	.achievements li::before {
		content: counter(step);
		width: 2ch;
		text-align: right;
		color: var(--muted);
		font-variant-numeric: tabular-nums;
	}
</style>
