<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<Page
	title="Biomes"
	description="The eighteen biomes of Rustcraft: their ground, their trees, their animals and their villages."
	kicker="Each colour is the biome's colour on the map."
>
	<p>
		The world is cut into biomes by its climate: how hot, how wet, how far inland, how high. Every land
		turns to mountain above height 84 and to high mountain above 100, whatever its climate. Use
		<code>/locate biome &lt;name&gt;</code> to find the nearest one.
	</p>
	<ul class="biomes">
		{#each data.biomes as b (b.href)}
			<li>
				<a href={b.href}>
					<span class="swatch" style:background={b.colour}><Slot id={b.surface} link={false} /></span>
					<span class="text">
						<strong>{b.name}</strong>
						<span>{b.note}</span>
						{#if b.herds.length}
							<span class="herds">
								{#each b.herds as h (h)}<Slot id="mob:{h}" link={false} />{/each}
							</span>
						{/if}
					</span>
				</a>
			</li>
		{/each}
	</ul>
</Page>

<style>
	.biomes {
		list-style: none;
		padding: 0;
		margin: 16px 0 0;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
		gap: 12px;
	}
	.biomes a {
		display: flex;
		gap: 14px;
		height: 100%;
		box-sizing: border-box;
		padding: 12px;
		background: var(--panel);
		border: 2px solid var(--line);
		color: var(--fg);
		text-decoration: none;
	}
	.biomes a:hover {
		border-color: var(--rust);
	}
	.swatch {
		display: grid;
		place-items: center;
		width: 64px;
		height: 64px;
		flex: none;
	}
	.text {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: 0.93rem;
		color: var(--muted);
	}
	strong {
		font-family: var(--display);
		font-size: 1.15rem;
		font-weight: 500;
		color: var(--rust);
	}
	.herds {
		display: flex;
		flex-wrap: wrap;
		gap: 2px;
		margin-top: 4px;
	}
	.herds :global(.slot) {
		width: 30px;
		height: 30px;
	}
	.herds :global(.icon) {
		width: 20px;
		height: 20px;
	}
</style>
