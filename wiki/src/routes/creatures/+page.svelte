<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import { ref } from '#lib/refs.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const INTRO: Record<string, string> = {
		Animals: 'They never attack. Most give food or materials; some can be tamed and ridden.',
		Fish: 'Shoals in the seas: beat them for fish, or use a rod.',
		Hostile: 'They come out in the dark and hunt you. Deeper tiers make them stronger.',
		Boss: 'One per world, at the bottom of it.',
		Villagers: 'They live in villages and trade. Nothing can hurt them.'
	};
</script>

<Page
	title="Creatures"
	description="Every creature of Rustcraft: animals, fish, hostiles, villagers and the Ember Mother, with health, damage and experience."
	kicker="Health is in points: a heart is two. Damage is at Normal difficulty, before armour."
>
	{#each data.groups as g (g)}
		<h2>{g}</h2>
		<p>{INTRO[g]}</p>
		<div class="wide">
			<table>
				<thead>
					<tr>
						<th>Creature</th>
						<th class="n">Health</th>
						{#if g === 'Hostile' || g === 'Boss'}<th class="n">Damage</th><th class="n">XP</th>{/if}
						<th>About</th>
					</tr>
				</thead>
				<tbody>
					{#each data.list.filter((m) => m.group === g) as m (m.id)}
						<tr>
							<td>
								<a class="who" href={ref(`mob:${m.id}`)?.href}>
									<Slot id="mob:{m.id}" link={false} />
									<span>{ref(`mob:${m.id}`)?.name}</span>
								</a>
							</td>
							<td class="n">{m.health}</td>
							{#if g === 'Hostile' || g === 'Boss'}<td class="n">{m.damage ?? '–'}</td><td class="n">{m.xp}</td>{/if}
							<td>{m.summary}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/each}
	<p>
		Hostiles in the Depths have half again their health and damage, and twice in the Furnace: see
		<a href="/guides/depths">the depths</a>.
	</p>
</Page>

<style>
	.who {
		display: flex;
		align-items: center;
		gap: 10px;
		color: var(--fg);
		text-decoration: none;
		white-space: nowrap;
	}
	.who:hover {
		color: var(--rust);
	}
	td {
		padding-block: 4px;
	}
</style>
