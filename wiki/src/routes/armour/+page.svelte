<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<Page
	title="Armour"
	description="Rustcraft armour: the points of each piece, how much damage a suit absorbs, durability, and horse armour."
	kicker="Each point of armour absorbs 4 % of the damage you take."
>
	{#snippet icon()}<Slot id="IronChestplate" big link={false} />{/snippet}

	<p>
		Put armour on in the four armour slots of your inventory. Each point absorbs 4 % of a blow, up to 20
		points; with <a href="/enchanting">Protection</a> on top, no more than 90 % is ever absorbed. Every
		blow it softens wears each piece you wear by 1.
	</p>
	<p>
		Armour softens the blows of creatures, arrows, fire and lava. It does nothing against falls, the void,
		drowning, hunger or poison.
	</p>

	<h2>Suits</h2>
	<div class="wide">
		<table>
			<thead>
				<tr>
					<th>Material</th>
					{#each data.pieces as p (p)}<th>{p}</th>{/each}
					<th class="n">Full suit</th>
				</tr>
			</thead>
			<tbody>
				{#each data.sets as s (s.material)}
					<tr>
						<td><strong>{s.material}</strong></td>
						{#each s.pieces as p (p.id)}
							<td>
								<span class="piece"><Slot id={p.id} /><span>{p.points} pts<br /><small>{p.durability} hits</small></span></span>
							</td>
						{/each}
						<td class="n">{s.total} pts, {s.total * 4} %</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
	<p>
		An <ItemLink id="Anvil" /> repairs each piece with its material: <ItemLink id="Leather" />,
		<ItemLink id="BronzeIngot" />, <ItemLink id="IronIngot" /> or <ItemLink id="Diamond" />.
	</p>

	<h2>Horse armour</h2>
	<p>Use it on a horse you tamed to put it on. Each point absorbs 4 % of the damage it takes.</p>
	<table>
		<thead><tr><th>Armour</th><th class="n">Points</th><th class="n">Absorbs</th></tr></thead>
		<tbody>
			{#each data.horse as h (h.id)}
				<tr><td><ItemLink id={h.id} /></td><td class="n">{h.points}</td><td class="n">{h.points * 4} %</td></tr>
			{/each}
		</tbody>
	</table>
	<p>Horse armour is crafted, and some is found in the chests of dungeons.</p>
</Page>

<style>
	.piece {
		display: flex;
		align-items: center;
		gap: 8px;
		white-space: nowrap;
	}
	small {
		color: var(--muted);
	}
</style>
