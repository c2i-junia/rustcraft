<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const HOW: Record<string, string> = {
		Night: 'Be alive from the first minute of a night until dawn. A night slept away does not count.',
		Caves: 'Under a roof of real ground, below y 50.',
		Depths: 'Below y 0 in a new world, y 20 in an older one.',
		FurnaceTier: 'Below y −40 in a new world, y 12 in an older one.',
		Boss: 'Be in the lair when she falls: everyone inside earns it.',
		Animal: 'Taming a wolf, a cat or a steed counts, and so does breeding two animals.'
	};
</script>

<Page
	title="Achievements"
	description="All {data.list.length} achievements of Rustcraft, in order, and how to earn each one."
	kicker="{data.list.length} achievements, earned in survival only. The pause menu shows yours."
>
	<ol class="list">
		{#each data.list as a (a.id)}
			<li>
				<Slot id={a.icon} />
				<div>
					<strong>{a.title.en}</strong>
					<span>{a.goal.en}.</span>
					{#if HOW[a.id]}<small>{HOW[a.id]}</small>{/if}
				</div>
			</li>
		{/each}
	</ol>
</Page>

<style>
	.list {
		list-style: none;
		padding: 0;
		margin: 0;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
		gap: 10px;
	}
	.list li {
		display: flex;
		gap: 14px;
		align-items: flex-start;
		padding: 10px 12px;
		background: var(--panel);
		border: 2px solid var(--line);
	}
	.list div {
		display: flex;
		flex-direction: column;
	}
	strong {
		font-family: var(--display);
		font-weight: 500;
		color: var(--rust);
		font-size: 1.1rem;
	}
	small {
		color: var(--muted);
		margin-top: 2px;
	}
</style>
