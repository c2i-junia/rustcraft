<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import Recipe from '#lib/components/Recipe.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

{#snippet biomeList(list: { name: string; slug: string }[])}
	{#each list as b, i (b.slug)}{i ? (i === list.length - 1 ? ' and ' : ', ') : ''}<a href="/biomes/{b.slug}">{b.name}</a>{/each}
{/snippet}

<Page
	title="Day, night and weather"
	description="How time and weather work in Rustcraft: the 20-minute day, when hostiles come out, skipping the night in a bedroll, rain, snow and thunderstorms, what rain does, and the /time and /weather commands."
>
	{#snippet icon()}<Slot id="Bedroll" big link={false} />{/snippet}

	<p>
		A day in Rustcraft lasts 20 minutes: 10 of night, then 10 of day. Every now and then the sky
		clouds over for rain or a thunderstorm.
	</p>

	<h2>The day</h2>
	<p>The cycle starts at sunset:</p>
	<table>
		<thead>
			<tr><th>Into the cycle</th><th>What happens</th></tr>
		</thead>
		<tbody>
			<tr><td>0 min</td><td>Sunset: night begins</td></tr>
			<tr><td>5 min</td><td>Midnight</td></tr>
			<tr><td>10 min</td><td>Sunrise: day begins</td></tr>
			<tr><td>15 min</td><td>Noon</td></tr>
			<tr><td>20 min</td><td>Sunset again</td></tr>
		</tbody>
	</table>
	<p>
		A new world starts 75 seconds after sunrise, so your first day leaves you a little under 9 minutes
		before dark. See <a href="/guides/first-day">your first day</a>.
	</p>

	<h2>When hostiles come out</h2>
	<p>A hostile needs a dark spot to appear:</p>
	<ul>
		<li>under the open sky, only at night;</li>
		<li>under a roof or underground, at any time of day;</li>
		<li>
			and never with a light source (a <ItemLink id="Torch" />, a lantern, a campfire, a lit furnace,
			fire, lava…) within 8 blocks along each axis.
		</li>
	</ul>
	<p>
		At night, <ItemLink id="Stumpling" label="Stumplings" /> appear on grass, and in woods half of them are
		<ItemLink id="Bramble" label="Brambles" /> instead. Above y 110, half of the night spawns are
		<ItemLink id="GustOwl" label="Gust Owls" />. <ItemLink id="Drowner" label="Drowners" /> need no darkness: they rise from water at least
		3 blocks deep, twice as often at night. At dawn, Stumplings sink back into the ground. The other
		hostiles stay until they are killed or you go far away (96 blocks).
	</p>
	<p>
		Staying alive from the first minute of a night until dawn earns <em>Still Here at Dawn</em>, but
		not if the night was slept away.
	</p>

	<h2>Sleeping</h2>
	<p>
		Use a <ItemLink id="Bedroll" /> to make it your respawn point, at any time of day. Used at night, you
		also fall asleep. When every player online is asleep, the night ends at once and the sun rises.
	</p>
	<div class="recipes"><Recipe recipe={data.bedroll} /></div>
	<p>Any colour of wool makes a bedroll of that colour.</p>

	<h2>Weather</h2>
	<p>A new world starts clear. Then the weather changes on its own:</p>
	<table>
		<thead>
			<tr><th>Weather</th><th>Lasts</th><th>Comes next</th></tr>
		</thead>
		<tbody>
			<tr><td>Clear</td><td>10 to 100 min</td><td>Rain (2 in 3) or a thunderstorm (1 in 3)</td></tr>
			<tr><td>Rain</td><td>5 to 15 min</td><td>Clear</td></tr>
			<tr><td>Thunderstorm</td><td>5 to 15 min</td><td>Clear</td></tr>
		</tbody>
	</table>

	<h3>Lightning</h3>
	<p>
		During a thunderstorm, lightning strikes every 3 to 15 seconds, 16 to 160 blocks from a player. It
		is seen and heard up to 1500 blocks away. Lightning is only a sight and a sound: it hurts no one and
		sets nothing on fire.
	</p>

	<h3>Rain and snow</h3>
	<ul>
		<li>Snow falls instead of rain in {@render biomeList(data.snow)}.</li>
		<li>Nothing falls in {@render biomeList(data.none)}.</li>
		<li>Everywhere else it rains, and the rain turns to snow above about y 122, on the high peaks.</li>
	</ul>
	<p>
		Rain and snow are only how the weather looks. Its effects are the same in every biome, deserts
		included, wherever a block has nothing but sky above it.
	</p>

	<h3>What rain does</h3>
	<p>Under the open sky, while it rains or storms:</p>
	<ul>
		<li>it puts out a burning player, unless you are still in lava;</li>
		<li>it puts out fire blocks;</li>
		<li>
			it waters crops, which grow twice as fast, as they do with water within 4 blocks. Both together
			are no faster. See <a href="/guides/farming">farming</a>;
		</li>
		<li>it kills <ItemLink id="Cinderling" label="Cinderlings" />, which then drop nothing;</li>
		<li>
			it fills a <ItemLink id="RainCistern" />. Use a <ItemLink id="Bucket" /> on a full one to get a
			<ItemLink id="WaterBucket" />: water even where there is none around;
		</li>
		<li>
			it puts out a <ItemLink id="Campfire" />. Light it again with a torch, a
			<ItemLink id="Flint" /> or <ItemLink id="FlintAndSteel" />.
		</li>
	</ul>

	<h2>Commands</h2>
	<p>
		On a server these need an operator; in a single-player world you can always use them. See
		<a href="/guides/commands">commands</a>.
	</p>
	<table>
		<thead>
			<tr><th>Command</th><th>What it does</th></tr>
		</thead>
		<tbody>
			<tr><td><code>/time set day</code></td><td>Moves the clock forward to the next noon</td></tr>
			<tr><td><code>/time set night</code></td><td>Moves the clock forward to the next midnight</td></tr>
			<tr><td><code>/weather clear</code></td><td>Clears the sky</td></tr>
			<tr><td><code>/weather rain</code></td><td>Starts rain</td></tr>
			<tr><td><code>/weather thunder</code></td><td>Starts a thunderstorm</td></tr>
		</tbody>
	</table>
	<p>
		The clock only goes forward. A weather set by command lasts as long as a natural one of its kind.
	</p>
</Page>
