<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import { duration, num } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<Page
	title="Food"
	description="Every food of Rustcraft: how much hunger and saturation it restores, how to cook it, and which foods give effects."
	kicker="Your hunger bar holds 20 points: ten drumsticks."
>
	{#snippet icon()}<Slot id="RoastBeef" big link={false} />{/snippet}

	<p>
		Eat by using food in your hand: it is eaten at once. <strong>Hunger</strong> is the bar you see;
		<strong>saturation</strong> is a hidden reserve that runs out first, so foods high in saturation keep
		you full longer. You heal only while your hunger is at 18 or more, cannot sprint at 6 or less, and
		starve at 0. You cannot eat with a full bar, unless the food gives an effect. See
		<a href="/guides/health">health and hunger</a>.
	</p>

	<div class="wide">
		<table>
			<thead>
				<tr><th>Food</th><th class="n">Hunger</th><th class="n">Saturation</th><th>How to get it</th><th>Notes</th></tr>
			</thead>
			<tbody>
				{#each data.foods as f (f.id)}
					<tr>
						<td><ItemLink id={f.id} /></td>
						<td class="n">{f.hunger}</td>
						<td class="n">{num(f.saturation)}</td>
						<td>
							{#if f.cooked}Smelt <ItemLink id={f.cooked} />{:else if f.crafted}<a href="/items/{f.id.replace(/([a-z])([A-Z])/g, '$1-$2').toLowerCase()}">Crafted</a>{:else}Found{/if}
						</td>
						<td>
							{#if f.effect}<a href="/effects#{f.effect.effect}">{f.effect.effect} {f.effect.level > 1 ? 'II' : 'I'}</a>, {duration(f.effect.seconds)}.{/if}
							{#if f.remains}Leaves a <ItemLink id={f.remains} />.{/if}
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
	<p>
		A <ItemLink id="MilkBucket" /> restores nothing but clears every status effect, and leaves the bucket.
	</p>
</Page>
