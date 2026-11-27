<!-- The head of an article: its title, with an icon, and its <title> and description. -->
<script lang="ts">
	import type { Snippet } from 'svelte';

	let {
		title,
		description,
		kicker,
		icon,
		children
	}: { title: string; description: string; kicker?: string; icon?: Snippet; children: Snippet } =
		$props();
</script>

<svelte:head>
	<title>{title} – Rustcraft Wiki</title>
	<meta name="description" content={description} />
	<meta property="og:title" content="{title} – Rustcraft Wiki" />
	<meta property="og:description" content={description} />
	<meta property="og:site_name" content="Rustcraft Wiki" />
</svelte:head>

<article>
	<header>
		{#if icon}<span class="icon">{@render icon()}</span>{/if}
		<div>
			<h1>{title}</h1>
			{#if kicker}<p class="kicker">{kicker}</p>{/if}
		</div>
	</header>
	{@render children()}
</article>

<style>
	header {
		display: flex;
		align-items: center;
		gap: 16px;
		margin-bottom: 22px;
		padding-bottom: 14px;
		border-bottom: 2px solid var(--line);
	}
	h1 {
		margin: 0;
		font-size: clamp(2rem, 4.5vw, 2.8rem);
		font-weight: 700;
		text-shadow: 0 3px 0 var(--rust-deep);
	}
	.kicker {
		margin: 4px 0 0;
		color: var(--muted);
	}
	article :global(h2) {
		font-size: 1.55rem;
		font-weight: 700;
		margin: 36px 0 12px;
	}
	article :global(h3) {
		font-size: 1.15rem;
		font-weight: 500;
		color: var(--rust);
		margin: 24px 0 8px;
	}
	article :global(p),
	article :global(ul:not([class])),
	article :global(ol:not([class])) {
		max-width: 72ch;
	}
	article :global(table) {
		border-collapse: collapse;
		margin: 8px 0 20px;
		font-size: 0.95rem;
	}
	article :global(th),
	article :global(td) {
		padding: 6px 12px;
		border: 1px solid var(--line);
		text-align: left;
		vertical-align: middle;
	}
	article :global(th) {
		background: var(--surface);
		font-weight: 600;
	}
	article :global(td.n),
	article :global(th.n) {
		text-align: right;
		font-variant-numeric: tabular-nums;
	}
	article :global(.wide) {
		overflow-x: auto;
		max-width: 100%;
	}
	article :global(.recipes) {
		display: flex;
		flex-wrap: wrap;
		gap: 14px;
		margin: 8px 0 16px;
	}
	article :global(.note) {
		border-left: 4px solid var(--rust);
		background: var(--surface);
		padding: 10px 16px;
		max-width: 72ch;
	}
	article :global(.note p) {
		margin: 0;
	}
	article::after {
		content: '';
		display: block;
		clear: both;
	}
</style>
