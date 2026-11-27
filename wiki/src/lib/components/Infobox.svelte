<!-- The box at the side of an article, as in the game's tooltips: a picture, a title, rows. -->
<script lang="ts">
	import type { Snippet } from 'svelte';

	let { title, picture, children }: { title: string; picture?: Snippet; children: Snippet } = $props();
</script>

<aside class="infobox" aria-label="{title} at a glance">
	<h2>{title}</h2>
	{#if picture}<div class="picture">{@render picture()}</div>{/if}
	<dl>{@render children()}</dl>
</aside>

<style>
	.infobox {
		float: right;
		clear: right;
		width: 300px;
		margin: 0 0 20px 28px;
		background: #1a1210;
		border: 2px solid #2b0f5a;
		outline: 2px solid #120b08;
		box-shadow: 6px 6px 0 rgb(0 0 0 / 0.4);
		/* The tooltip of the game: dark, framed in purple */
		border-image: linear-gradient(#5b2ab0, #2a0f63) 1;
	}
	.infobox h2 {
		margin: 0;
		padding: 10px 14px 6px;
		font-size: 1.2rem;
		font-weight: 700;
		text-align: center;
	}
	.picture {
		display: grid;
		place-items: center;
		padding: 8px 0 14px;
	}
	dl {
		margin: 0;
		padding: 0 14px 12px;
		display: grid;
		grid-template-columns: auto 1fr;
		gap: 5px 14px;
		font-size: 0.93rem;
	}
	dl :global(dt) {
		color: var(--muted);
	}
	dl :global(dd) {
		margin: 0;
		text-align: right;
	}
	@media (max-width: 700px) {
		.infobox {
			float: none;
			width: auto;
			margin: 0 0 24px;
		}
	}
</style>
