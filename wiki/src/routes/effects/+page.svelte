<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import { duration } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const DOES: Record<string, string> = {
		Poison: 'Takes 1 health every 1.25 s at level I, twice as often each level up. It never takes the last point.',
		Burn: 'Takes 1 health every second at level I, twice as often each level up. It can kill, and counts as fire.',
		Slowness: 'You walk 15 % slower per level, never below a fifth of your speed.',
		Weakness: 'Your blows do 2 less damage per level, never less than 1.',
		Regeneration: 'Gives back 1 health every 2.5 s at level I, twice as often each level up.',
		Healing: 'Gives back 4 health per level at once.',
		NightVision: 'The world is drawn bright, even in the dark.',
		WaterBreathing: 'Your air never runs out under water.',
		FireResistance: 'Fire, lava and magma no longer hurt you.',
		Swiftness: 'You walk 20 % faster per level.',
		Strength: 'Your blows do 3 more damage per level.'
	};
</script>

<Page
	title="Status effects"
	description="Every status effect in Rustcraft: what it does at each level, and the potions, foods and creatures that give it."
	kicker="Effects go up to level IV. The HUD shows them at the top left, with the time left."
>
	<ul>
		<li>A stronger effect replaces a weaker one; at the same level, the longer time is kept; a weaker one is ignored.</li>
		<li>A <ItemLink id="MilkBucket" /> clears them all, and so does death.</li>
		<li>In creative mode, creatures give you no effect, but potions still work.</li>
	</ul>

	{#each data.effects as e (e.id)}
		<section id={e.id}>
			<h2>{e.name.en} <span class="kind" class:good={e.beneficial}>{e.beneficial ? 'Good' : 'Harmful'}</span></h2>
			<p>{DOES[e.id] ?? ''}</p>
			{#if e.potions.length || e.foods.length || e.mobs.length}
				<p class="from">
					<span>From:</span>
					{#each e.potions as id (id)}<ItemLink {id} />{/each}
					{#each e.foods as f (f.id)}<span><ItemLink id={f.id} /> (level {f.level}, {duration(f.seconds)})</span>{/each}
					{#each e.mobs as m (m.id)}<span>the blow of <ItemLink id={m.id} /> ({m.text})</span>{/each}
				</p>
			{/if}
		</section>
	{/each}
</Page>

<style>
	section {
		scroll-margin-top: 80px;
	}
	.kind {
		font-size: 0.85rem;
		font-family: var(--body);
		font-weight: 500;
		color: var(--bad);
		vertical-align: middle;
		margin-left: 8px;
	}
	.kind.good {
		color: var(--good);
	}
	.from {
		display: flex;
		flex-wrap: wrap;
		gap: 6px 18px;
		color: var(--muted);
	}
</style>
