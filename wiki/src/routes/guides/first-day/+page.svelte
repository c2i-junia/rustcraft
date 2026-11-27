<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import Recipe from '#lib/components/Recipe.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<Page
	title="Your first day"
	description="A step-by-step start in Rustcraft survival: wood, a crafting table, wooden and stone tools, food, torches, a shelter, a bedroll, and the first night."
>
	{#snippet icon()}<Slot id="WoodenPickaxe" big link={false} />{/snippet}

	<p>
		A new world starts in the morning, 75 seconds after sunrise. The day lasts 10 minutes, so you
		have a little under 9 minutes of light before the first night. This guide takes you from bare
		hands to a lit shelter and a bedroll in that time.
	</p>
	<p>
		The keys below are the defaults: see <a href="/guides/controls">Controls</a> to change them.
	</p>

	<h2>1. Punch a tree</h2>
	<p>
		Hold the left mouse button on a tree trunk to break it. Logs break by hand, and an axe only makes
		it faster. Gather a few <ItemLink id="OakLog" />: 5 or 6 is enough to start.
	</p>
	<p>
		While you walk, break some tall grass too. About one time in three it drops <ItemLink id="Fiber" />, which you will
		need for rope.
	</p>

	<h2>2. Planks, sticks and a crafting table</h2>
	<p>
		Open your inventory with <kbd>E</kbd>. It has a 2×2 crafting grid: one log makes 4 planks, two
		planks make 4 sticks, and four planks make a crafting table.
	</p>
	<div class="recipes">
		{#each data.wood as recipe (recipe.output)}<Recipe {recipe} />{/each}
	</div>
	<p>
		Place the crafting table with the right mouse button and right-click it to open its 3×3 grid.
		Every tool needs it. Any kind of planks works where the recipe says "any planks".
	</p>

	<h2>3. Wooden tools</h2>
	<p>
		Make a wooden pickaxe first: you need one to mine stone, and nothing else is a pickaxe at this
		point. Holding your first tool earns the achievement <em>Better Than Bare Hands</em>.
	</p>
	<div class="recipes">
		{#each data.woodTools as recipe (recipe.output)}<Recipe {recipe} />{/each}
	</div>
	<p>
		Wooden tools wear out after 59 uses, so do not spend them on dirt. Mining stone or cobblestone
		with your hand takes time and gives you nothing.
	</p>

	<h2>4. Stone tools and a furnace</h2>
	<p>
		Dig into a hillside or down a few blocks to reach <ItemLink id="Stone" />. Mined with a pickaxe,
		it drops <ItemLink id="Cobblestone" />. Collect at least 14: 3 for a pickaxe, 2 for a sword, 8
		for a furnace, and one to spare.
	</p>
	<div class="recipes">
		{#each data.stoneTools as recipe (recipe.output)}<Recipe {recipe} />{/each}
	</div>
	<p>
		Stone tools last 131 uses and work twice as fast as wooden ones. A stone pickaxe also mines iron
		and tin ore, which wood cannot. Look for <ItemLink id="CoalOre" /> in the stone: any pickaxe mines
		it, and the coal makes torches.
	</p>

	<h2>5. Light</h2>
	<p>
		Hostile creatures only spawn in the dark: at night under the open sky, or under a roof or in a
		cave at any time. No hostile spawns within 8 blocks of a light, in any direction. A torch, a
		lantern, a campfire, a lit furnace, fire, lava and a few glowing blocks all count.
	</p>
	<p>One coal or one charcoal and a stick make 4 torches.</p>
	<div class="recipes">
		{#each data.torches as recipe (recipe.cells.join())}<Recipe {recipe} />{/each}
	</div>
	<p>
		No coal yet? Put logs in the furnace: each log turns into <ItemLink id="Charcoal" />, and logs or
		planks can be burned as fuel to start it.
	</p>

	<h2>6. Food</h2>
	<p>
		Your food bar has 20 points. Health only comes back on its own while food is 18 or more, and you
		cannot sprint at 6 or less. At 0 you take 1 damage every 4 seconds, and starving can kill you.
	</p>
	<p>
		Easy food on the first day comes from animals: a <ItemLink id="Pig" /> drops 1 to 3 raw pork, a
		<ItemLink id="Cow" /> 1 to 3 raw beef, a <ItemLink id="Sheep" /> 1 or 2 raw mutton, a
		<ItemLink id="Chicken" /> 1 raw chicken. Raw meat restores little; cooked, it is worth far more:
	</p>
	<table>
		<thead><tr><th>Food</th><th class="n">Raw</th><th class="n">Cooked</th></tr></thead>
		<tbody>
			<tr><td><ItemLink id="RawPork" /> or <ItemLink id="RawBeef" /></td><td class="n">3</td><td class="n">8</td></tr>
			<tr><td><ItemLink id="RawMutton" /></td><td class="n">2</td><td class="n">6</td></tr>
			<tr><td><ItemLink id="RawChicken" /></td><td class="n">2</td><td class="n">6</td></tr>
		</tbody>
	</table>
	<p>
		Cook in the furnace (10 seconds per item), or on a campfire. A lit campfire cooks a piece of raw
		food at once when you right-click it while holding the food. It also gives light.
	</p>
	<div class="recipes"><Recipe recipe={data.campfire} /></div>
	<p>
		Any kind of log works for the campfire, as long as all three are the same. <ItemLink
			id="Apple"
		/> and <ItemLink id="Berries" /> are food you can eat as they are. See <a href="/food">Food</a> and
		<a href="/guides/health">Health and hunger</a> for more.
	</p>

	<h2>7. A shelter</h2>
	<p>
		Any closed room works: walls of planks, cobblestone or dirt, a roof, and a torch inside so that
		nothing spawns there. Dig into a hill if you are short of time. A door needs six oak planks, and
		a chest keeps what you do not want to carry.
	</p>
	<div class="recipes">
		{#each data.shelter as recipe (recipe.output)}<Recipe {recipe} />{/each}
	</div>
	<p class="note">
		<ItemLink id="Stumpling" />s, which spawn on grass at night, open doors. Light the ground around
		your door too, or close the entrance with a block.
	</p>

	<h2>8. A bedroll</h2>
	<p>
		A bedroll needs 3 wool and 2 rope. Wool comes from sheep: kill one for 1 wool, or right-click an
		adult sheep with almost any item to shear it for 1 or 2, and its wool grows back. Three fiber make
		2 rope.
	</p>
	<div class="recipes">
		{#each data.bedroll as recipe (recipe.output)}<Recipe {recipe} />{/each}
	</div>
	<p>
		Place the bedroll and right-click it. That sets your respawn point there ("You will come back
		here") and earns <em>Somewhere to Come Back To</em>. If the bedroll is broken, you come back at
		the world spawn instead. Wool of other colours makes bedrolls of those colours.
	</p>
	<p>
		At night, right-clicking it also puts you to sleep. When every player online is asleep, the
		night is skipped and the sun rises.
	</p>

	<h2>9. The first night</h2>
	<p>
		The night lasts 10 minutes. Hostiles spawn 24 to 64 blocks away from you, so they come to you from
		the dark:
	</p>
	<ul>
		<li><ItemLink id="Stumpling" /> on grass at night. It sinks into the ground at dawn.</li>
		<li><ItemLink id="Bramble" /> in the woods at night. It never moves, but poisons you if you come close.</li>
		<li><ItemLink id="Gnawer" /> in caves below height 50, in packs of 2 to 4.</li>
	</ul>
	<p>You have three ways through it:</p>
	<ul>
		<li>
			<strong>Sleep</strong> on your bedroll. It is the safest, but a night slept away does not count
			for the achievement <em>Still Here at Dawn</em>.
		</li>
		<li>
			<strong>Stay awake</strong> in your lit shelter and do something useful: smelt, craft, or dig a
			mine from inside. Being alive from nightfall to dawn earns <em>Still Here at Dawn</em>.
		</li>
		<li>
			<strong>Fight</strong> with your stone sword. Your first kill earns <em>Holding Your Ground</em>.
			See <a href="/guides/combat">Combat</a>.
		</li>
	</ul>
	<p>
		If you die, you drop everything you carry where you fell, and your death point is saved on your
		map. Items on the ground disappear after 5 minutes, so go back quickly.
	</p>

	<h2>What next</h2>
	<ul>
		<li><a href="/guides/progression">Progression</a>: the road from stone to the Ember Mother.</li>
		<li><a href="/guides/ores">Ores</a>: where copper, tin and iron are found.</li>
		<li><a href="/guides/farming">Farming</a>: food that does not run away.</li>
		<li><a href="/guides/animals">Animals</a>: taming, breeding and riding.</li>
		<li><a href="/crafting">Crafting</a>: every recipe of the game.</li>
	</ul>
</Page>
