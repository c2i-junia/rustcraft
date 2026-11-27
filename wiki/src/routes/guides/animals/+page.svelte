<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import Recipe from '#lib/components/Recipe.svelte';
	import { percent } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<Page
	title="Taming, breeding and riding"
	description="Rustcraft animals: what each one eats, taming wolves, cats and horses, breeding and growing young, shearing, milking, leads, horse coats and stats, saddles, horse armour, pack chests and boats."
>
	{#snippet icon()}<Slot id="Saddle" big link={false} />{/snippet}

	<p>
		Every farm animal eats something you can grow or hunt. Feed it to make it breed, to make a young
		one grow faster, or, for wolves, cats and steeds, to tame it. A tamed horse with a saddle is the
		fastest way to cross land; a boat is the way across water.
	</p>

	<h2>What each animal eats</h2>
	<p>
		Hold the food and use it on the animal (right click). Holding the button feeds again every half
		second. One item is used per feeding, and only when the animal takes it.
	</p>
	<table>
		<thead>
			<tr><th>Animal</th><th>Food</th><th>Tameable</th></tr>
		</thead>
		<tbody>
			{#each data.animals as a (a.id)}
				<tr>
					<td><ItemLink id={a.id} /></td>
					<td>
						{#each a.eats as food (food)}<Slot id={food} />{/each}
					</td>
					<td>{a.tameable ? 'Yes' : 'No'}</td>
				</tr>
			{/each}
		</tbody>
	</table>

	<h2>Taming</h2>
	<p>
		<ItemLink id="Wolf" />, <ItemLink id="Cat" />, <ItemLink id="Horse" />,
		<ItemLink id="Donkey" /> and <ItemLink id="Mule" /> can be tamed. Feed a wild one its food: each
		feeding has a <strong>1 in 3</strong> chance to tame it, and the food is eaten either way. A tamed
		animal belongs to the player who fed it. A wild animal that was fleeing from you stops once tamed.
	</p>
	<p>
		Your first tamed animal, or your first young one born, earns the achievement
		<a href="/achievements">Good Company</a>.
	</p>

	<h3>Wolves and cats</h3>
	<ul>
		<li>
			A tamed wolf or cat follows you. It walks to you once you are more than 6 blocks away, stops at
			2.5 blocks, and is brought to your side if it falls more than 24 blocks behind while you stand on
			the ground.
		</li>
		<li>Use a tamed wolf or cat to make it sit, and again to make it follow.</li>
		<li>
			<strong>Wolves fight for you.</strong> A wolf hunts what you strike within 24 blocks, and any hostile
			that goes at you within 12 blocks. It runs at 4.5 blocks a second, bites for 4 every second, and
			gives up a prey more than 24 blocks from it or from you.
		</li>
		<li>
			<strong>Cats scare hostiles.</strong> <ItemLink id="Gnawer" label="Gnawers" /> and <ItemLink id="GustOwl" label="Gust Owls" /> keep
			8 blocks away from a tamed cat.
		</li>
	</ul>

	<h3>Healing pets</h3>
	<p>
		A hurt tamed wolf, horse, donkey or mule eats its food to heal instead of breeding. Tamed cats are
		not healed this way.
	</p>
	<table>
		<thead><tr><th>Food</th><th class="n">Health back</th></tr></thead>
		<tbody>
			<tr><td>Roast meat</td><td class="n">6</td></tr>
			<tr><td><ItemLink id="Carrot" /> or <ItemLink id="Apple" /></td><td class="n">4</td></tr>
			<tr><td>Raw meat</td><td class="n">3</td></tr>
			<tr><td>Anything else it eats (<ItemLink id="Rye" />)</td><td class="n">2</td></tr>
		</tbody>
	</table>

	<h2>Breeding</h2>
	<ol>
		<li>Feed an adult its food. It wants a mate for <strong>30 seconds</strong>.</li>
		<li>
			Feed a second adult of the same kind. Two animals that want a mate look for each other within 8
			blocks, walk together, and a young one is born once they are 1.5 blocks apart.
		</li>
		<li>Each parent must wait <strong>5 minutes</strong> before it breeds again.</li>
	</ol>
	<ul>
		<li>
			Wolves, cats, horses, donkeys and mules breed only once tamed, and both parents must belong to the
			same player. Their young one belongs to that player too.
		</li>
		<li>
			A <ItemLink id="Horse" /> and a <ItemLink id="Donkey" /> give a <ItemLink id="Mule" />. Mules never
			breed.
		</li>
		<li>
			A young animal grows up in <strong>10 minutes</strong>. Each time you feed it, it grows up 1 minute
			sooner.
		</li>
		<li>An animal that already wants a mate, or bred not long ago, refuses its food.</li>
	</ul>

	<h2>Wool, milk and eggs</h2>
	<ul>
		<li>
			<strong>Shearing.</strong> Use an adult <ItemLink id="Sheep" /> with almost anything in hand (not
			food, a bow, a fishing rod or a spawn egg) for 1 to 2 <ItemLink id="Wool" />. Its wool grows
			back after it grazes. A sheared sheep drops no wool when killed.
		</li>
		<li>
			<strong>Milking.</strong> Use a <ItemLink id="Bucket" /> on an adult <ItemLink id="Cow" /> for a
			<ItemLink id="MilkBucket" />. Drinking it removes every status effect.
		</li>
		<li>
			<strong>Eggs.</strong> Each adult <ItemLink id="Chicken" /> lays an <ItemLink id="Egg" /> about every
			5 minutes, by chance. Cook it into a <ItemLink id="FriedEgg" /> in a furnace.
		</li>
	</ul>

	<h2>Leads</h2>
	<div class="recipes">
		{#each data.lead as recipe, i (i)}<Recipe {recipe} />{/each}
	</div>
	<ul>
		<li>
			Use a <ItemLink id="Lead" /> on any land animal to lead it. It follows when more than 4 blocks
			away, and the lead snaps past 12 blocks, falling where the animal is.
		</li>
		<li>
			Use a <ItemLink id="Fence" /> post within 5 blocks to tie the animals you lead to it. Use a lead on
			a tied animal to take it back in hand.
		</li>
		<li>Use an animal you lead to take the lead off: it goes back to your inventory.</li>
	</ul>

	<h2>Horses</h2>
	<p>
		To ride, a horse, donkey or mule must be <strong>tamed by you</strong>, <strong>adult</strong> and
		<strong>saddled</strong>. Use a <ItemLink id="Saddle" /> on it, then use it to climb on. Press
		<kbd>Left Shift</kbd> to get off. Unlike wolves and cats, a steed does not follow you: it waits
		where you left it.
	</p>
	<div class="recipes">
		{#each data.saddle as recipe, i (i)}<Recipe {recipe} />{/each}
	</div>
	<p>
		Saddles also turn up in shipwreck chests, ruins and crypts, and as a fishing treasure. See
		<a href="/guides/structures">structures</a> and <a href="/guides/fishing">fishing</a>.
	</p>

	<h3>Riding</h3>
	<ul>
		<li>
			<strong>Gallop:</strong> hold <kbd>Left Ctrl</kbd> to go 1.6 times faster, as long as the horse's
			breath lasts. Its breath comes back at half the rate it is used, and a winded horse gallops again
			once it has 30 % of it back.
		</li>
		<li>
			<strong>Jump:</strong> hold <kbd>Space</kbd> and let go. Held 1 second, the horse jumps its highest;
			a quick tap gives 30 % of that.
		</li>
		<li>A horse steps up a whole block without jumping.</li>
		<li>It goes slower backwards (0.4 times), sideways (0.5) and in water (0.35).</li>
		<li>Your fall damage counts 3 blocks less while you ride.</li>
		<li>A hostile's blow aimed at you is shared: your steed takes half of it, rounded up.</li>
	</ul>

	<h3>Coats and stats</h3>
	<p>
		Every horse has a speed, a jump height and an endurance (how long it gallops). Its coat tells its
		speed and endurance on average: a wild horse is within 1 block a second and 2 seconds of the values
		below. Its jump is anywhere in the whole range, whatever the coat.
	</p>
	<table>
		<thead>
			<tr><th>Coat</th><th class="n">Speed (blocks/s)</th><th class="n">Endurance (s)</th></tr>
		</thead>
		<tbody>
			{#each data.coats as c (c.coat)}
				<tr><td>{c.coat}</td><td class="n">{c.speed}</td><td class="n">{c.endurance}</td></tr>
			{/each}
		</tbody>
	</table>
	<table>
		<thead>
			<tr><th></th><th class="n">Speed (blocks/s)</th><th class="n">Jump (blocks)</th><th class="n">Endurance (s)</th></tr>
		</thead>
		<tbody>
			<tr><td>Any horse</td><td class="n">6–12</td><td class="n">1.3–3.2</td><td class="n">3–16</td></tr>
			<tr><td>Wild donkey</td><td class="n">6–7</td><td class="n">1.3–1.6</td><td class="n">11–15</td></tr>
		</tbody>
	</table>
	<p>
		<strong>Inheritance.</strong> A foal gets the average of its parents' speed, jump and endurance,
		give or take 0.6 blocks a second, 0.25 blocks and 1.2 seconds. Its coat is one parent's, but 1 time in
		10 it is a random coat. Breed your best horses together to raise better ones.
	</p>

	<h3>Horse armour</h3>
	<p>
		Use horse armour on an adult horse you own to put it on; using another piece swaps them. Only horses
		wear it, not donkeys or mules. Each point takes 4 % off the blows the horse takes. Horse armour is not
		crafted: it is found in ruins and crypts.
	</p>
	<table>
		<thead><tr><th>Armour</th><th class="n">Points</th><th class="n">Damage taken off</th></tr></thead>
		<tbody>
			{#each data.armour as a (a.id)}
				<tr><td><ItemLink id={a.id} /></td><td class="n">{a.points}</td><td class="n">{percent(a.points * 0.04)}</td></tr>
			{/each}
		</tbody>
	</table>

	<h2>Donkeys and mules with chests</h2>
	<p>
		Use a <ItemLink id="Chest" /> on an adult <ItemLink id="Donkey" /> or <ItemLink id="Mule" /> you own
		to strap a 27-slot chest on its back. Sneak and use the animal to open it, within 5 blocks. Only its
		owner can open it. If the animal dies, the chest's contents drop where it fell, and so do its saddle
		and lead.
	</p>

	<h2>Boats</h2>
	<div class="recipes">
		{#each data.boat as recipe, i (i)}<Recipe {recipe} />{/each}
	</div>
	<p>
		Use a <ItemLink id="Boat" /> on water to put it down, then use it to get in. It has two seats: the
		driver at the back and a passenger at the front. It goes 8 blocks a second on water and 1 on land.
		Break it to get it back.
	</p>
</Page>
