<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import Recipe from '#lib/components/Recipe.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import { duration } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const EFFECT: Record<string, string> = {
		Healing: '4 health at once',
		NightVision: 'See in the dark',
		WaterBreathing: 'Your air never runs out',
		FireResistance: 'Fire, lava and magma do not hurt',
		Swiftness: '20 % faster',
		Strength: '+3 damage to your blows',
		Regeneration: '1 health every 2.5 s',
		Poison: '1 health every 1.25 s, never the last',
		Slowness: '15 % slower',
		Weakness: '−2 damage to the blows'
	};
</script>

<Page
	title="Alchemy"
	description="Brewing potions in Rustcraft: the alembic, the neutral base, every potion, and how moss and crystal shards change them."
	kicker="Each distillation at the alembic takes {data.seconds} seconds."
>
	{#snippet icon()}<Slot id="Alembic" big link={false} />{/snippet}

	<p>
		Potions are distilled at an <ItemLink id="Alembic" />. It works like a furnace: the ingredient goes on
		top, fuel below, and the vial in the slot on the right, which the ingredient is distilled into. Each
		step takes {data.seconds} seconds and burns fuel like smelting does.
	</p>

	<div class="recipes">
		<Recipe recipe={data.alembic} />
		<Recipe recipe={data.vial} />
	</div>

	<h2>Step by step</h2>
	<ol>
		<li>Craft <ItemLink id="GlassVial" />s and fill them at a water source: you get <ItemLink id="WaterVial" />s.</li>
		<li>
			Distil a <ItemLink id={data.base.ingredient} /> into a water vial: it becomes a <ItemLink
				id={data.base.result}
			/>, the start of every potion.
		</li>
		<li>Distil an ingredient from the table below into the neutral base to make a potion.</li>
		<li>
			Optionally, distil <ItemLink id={data.longer} /> into it to make it last twice as long, or a <ItemLink
				id={data.stronger}
			/> to make it a level stronger for half the time. Each modifier works once per potion; both together
			give level II for the full time.
		</li>
	</ol>

	<h2>Potions</h2>
	<div class="wide">
		<table>
			<thead>
				<tr><th>Potion</th><th>Ingredient</th><th>Effect</th><th class="n">Lasts</th><th>Longer</th><th>Stronger</th></tr>
			</thead>
			<tbody>
				{#each data.potions as p (p.result)}
					<tr>
						<td><ItemLink id={p.result} /></td>
						<td><ItemLink id={p.ingredient} /></td>
						<td><a href="/effects#{p.potion.effect}">{EFFECT[p.potion.effect] ?? p.potion.effect}</a></td>
						<td class="n">{p.potion.seconds ? duration(p.potion.seconds) : 'Instant'}</td>
						<td>{p.potion.longer ? duration(p.potion.seconds * 2) : '–'}</td>
						<td>{p.potion.stronger ? (p.potion.seconds ? `II, ${duration(p.potion.seconds / 2)}` : 'II') : '–'}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>

	<h2>Splash potions</h2>
	<p>
		Poison, slowness and weakness are thrown, not drunk: hold the use button to wind up, release to
		throw. The potion bursts where it lands, over 4 blocks: a player at the heart of the burst gets the
		whole effect, one at its edge half its time. Of the creatures, only poison hurts them, at once.
	</p>

	<h2>Effects that meet</h2>
	<p>
		A stronger effect replaces a weaker one; at the same level, the longer time is kept. A <ItemLink
			id="MilkBucket"
		/> clears every effect, and so does death. See <a href="/effects">status effects</a>.
	</p>
</Page>
