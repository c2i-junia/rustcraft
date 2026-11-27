<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import { percent } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const total = (pool: (typeof data.pools)[number], night: boolean) =>
		pool.entries.filter((e) => night || !e.night).reduce((s, e) => s + e.weight, 0);
</script>

<Page
	title="Fishing"
	description="How to fish in Rustcraft: casting, the bite, and the odds of every fish, junk and treasure."
>
	{#snippet icon()}<Slot id="FishingRod" big link={false} />{/snippet}

	<p>
		Cast a <ItemLink id="FishingRod" /> into water with the use button, wait for a bite, and draw the
		line in while the float is under. A catch is mostly fish, sometimes junk, and now and then a
		treasure worth the wait.
	</p>

	<h2>How to fish</h2>
	<ol>
		<li>Hold the rod and use it toward water: the float flies out at 11 blocks a second.</li>
		<li>
			Wait. A fish bites after <strong>5 to 25 seconds</strong>: the float dips under the surface.
		</li>
		<li>
			Use the rod again <strong>during the bite</strong>, which lasts about a second. Too early or
			too late, the line comes back empty.
		</li>
	</ol>
	<p>
		The line breaks off by itself past 32 blocks. Each catch wears the rod by 1, and pulling the
		float out of the ground by 2. Every catch also gives 1 to 3 points of experience.
	</p>

	<h2>What you catch</h2>
	{#each data.pools as pool (pool.name)}
		<h3>{pool.name}: {percent(pool.share)} of catches</h3>
		<table>
			<thead>
				<tr><th>Catch</th><th class="n">By day</th><th class="n">By night</th></tr>
			</thead>
			<tbody>
				{#each pool.entries as e (e.item)}
					<tr>
						<td><ItemLink id={e.item} count={e.count} /></td>
						<td class="n">{e.night ? 'Never' : percent((pool.share * e.weight) / total(pool, false))}</td>
						<td class="n">{percent((pool.share * e.weight) / total(pool, true))}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{/each}

	<h2>Tips</h2>
	<ul>
		<li>
			The <ItemLink id="RawGlowfin" /> bites only at night. It is the ingredient of the potion of night
			vision, at the <a href="/alchemy">alembic</a>.
		</li>
		<li>
			Shoals of <ItemLink id="Glintfin" /> and <ItemLink id="Shoalback" /> swim in the seas: beat them
			for fish without a rod.
		</li>
		<li>Raw fish restores little: grill it in a <a href="/smelting">furnace</a> first.</li>
	</ul>
</Page>
