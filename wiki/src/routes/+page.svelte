<script lang="ts">
	import Slot from '#lib/components/Slot.svelte';
	import { NAV } from '#lib/nav.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
	const description =
		'The players’ wiki of Rustcraft: every block, item, recipe and creature, read from the game itself, and guides from the first night to the Ember Mother.';
</script>

<svelte:head>
	<title>Rustcraft Wiki</title>
	<meta name="description" content={description} />
	<meta property="og:title" content="Rustcraft Wiki" />
	<meta property="og:description" content={description} />
	<meta property="og:site_name" content="Rustcraft Wiki" />
	<meta property="og:image" content="https://wiki.rustcraft.dev/images/world.webp" />
</svelte:head>

<section class="hero">
	<img src="/images/world.webp" alt="" width="1600" height="621" />
	<div class="text">
		<h1>Rustcraft Wiki</h1>
		<p>
			Every block, item, recipe and creature of the game, read from its code, and guides from your first
			night to the Ember Mother.
		</p>
		<p class="counts">
			<a href="/items"><strong>{data.counts.items}</strong> items</a>
			<a href="/crafting"><strong>{data.counts.recipes}</strong> recipes</a>
			<a href="/creatures"><strong>{data.counts.creatures}</strong> creatures</a>
			<a href="/biomes"><strong>{data.counts.biomes}</strong> biomes</a>
		</p>
		<p class="hint">Press <kbd>/</kbd> to search, in English or in French.</p>
	</div>
</section>

<div class="new">
	<h2>New to the game?</h2>
	<p>
		Start with <a href="/guides/first-day">your first day</a>: wood, tools, a shelter and a bed before the first
		night falls. Then follow <a href="/guides/progression">the progression</a> down to the depths.
	</p>
</div>

{#each NAV as section (section.title)}
	<section class="portal">
		<h2>{section.title}</h2>
		<ul>
			{#each section.links as link (link.href)}
				<li>
					<a href={link.href}>
						{#if link.icon}<Slot id={link.icon} link={false} />{/if}
						<span><strong>{link.title}</strong>{#if link.blurb}<span>{link.blurb}</span>{/if}</span>
					</a>
				</li>
			{/each}
		</ul>
	</section>
{/each}

<style>
	.hero {
		position: relative;
		display: grid;
		align-items: end;
		min-height: 340px;
		margin: 0 0 28px;
		overflow: hidden;
		border: 2px solid var(--line);
	}
	.hero img {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		object-fit: cover;
		image-rendering: pixelated;
	}
	.hero::after {
		content: '';
		position: absolute;
		inset: 0;
		background: linear-gradient(to top, var(--bg) 6%, rgb(22 19 15 / 0.78) 45%, rgb(22 19 15 / 0.1) 85%);
	}
	.text {
		position: relative;
		z-index: 1;
		padding: 80px 28px 24px;
	}
	h1 {
		margin: 0 0 10px;
		font-size: clamp(2.6rem, 7vw, 4.6rem);
		line-height: 0.95;
		text-shadow: 0 4px 0 var(--rust-deep);
	}
	.text p {
		margin: 0;
		max-width: 40rem;
		font-size: 1.12rem;
	}
	.counts {
		display: flex;
		flex-wrap: wrap;
		gap: 8px 22px;
		margin-top: 14px !important;
	}
	.counts a {
		color: var(--fg);
		text-decoration: none;
	}
	.counts a:hover {
		color: var(--rust);
	}
	.counts strong {
		font-family: var(--display);
		font-size: 1.5rem;
		color: var(--rust);
		margin-right: 4px;
	}
	.hint {
		margin-top: 10px !important;
		color: var(--muted);
		font-size: 0.95rem !important;
	}
	.new {
		border-left: 4px solid var(--rust);
		background: var(--surface);
		padding: 12px 20px;
		margin-bottom: 8px;
	}
	.new h2 {
		margin: 0 0 4px;
		font-size: 1.3rem;
	}
	.new p {
		margin: 0;
	}
	.portal h2 {
		font-size: 1.5rem;
		margin: 32px 0 12px;
	}
	.portal ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
		gap: 8px 20px;
	}
	.portal a {
		display: flex;
		gap: 12px;
		align-items: flex-start;
		padding: 6px;
		color: var(--fg);
		text-decoration: none;
	}
	.portal a:hover strong {
		color: var(--rust);
	}
	.portal a:hover :global(.slot) {
		background: var(--slot-hover);
	}
	.portal a > span {
		display: flex;
		flex-direction: column;
	}
	.portal strong {
		font-weight: 600;
	}
	.portal a > span > span {
		font-size: 0.92rem;
		color: var(--muted);
	}
</style>
