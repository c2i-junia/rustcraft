<script lang="ts">
	import '@fontsource/pixelify-sans/500.css';
	import '@fontsource/pixelify-sans/700.css';
	import favicon from '#lib/assets/favicon.svg';
	import Search from '#lib/components/Search.svelte';
	import { NAV } from '#lib/nav.ts';
	import { page } from '$app/state';
	import { afterNavigate } from '$app/navigation';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
	let menuOpen = $state(false);
	afterNavigate(() => (menuOpen = false));

	const current = (href: string) =>
		page.url.pathname === href || (href !== '/' && page.url.pathname.startsWith(href + '/'));
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<meta name="theme-color" content="#16130f" />
</svelte:head>

<a class="skip" href="#content">Skip to the article</a>

<header class="top">
	<a class="brand" href="/">Rustcraft <span>Wiki</span></a>
	<div class="search"><Search /></div>
	<button
		class="menu"
		aria-expanded={menuOpen}
		aria-controls="sidebar"
		onclick={() => (menuOpen = !menuOpen)}>Menu</button
	>
	<a class="game" href="https://rustcraft.dev">Get the game</a>
</header>

<div class="frame">
	<nav id="sidebar" class:open={menuOpen} aria-label="Wiki">
		{#each NAV as section (section.title)}
			<h2>{section.title}</h2>
			<ul>
				{#each section.links as link (link.href)}
					<li>
						<a href={link.href} aria-current={current(link.href) ? 'page' : undefined}>{link.title}</a>
					</li>
				{/each}
			</ul>
		{/each}
	</nav>

	<main id="content">
		{@render children()}
	</main>
</div>

<footer>
	<p>
		The numbers on this wiki are read from the game's code: if one is wrong, the game changed. The wiki
		is open source, like <a href="https://github.com/c2i-junia/rustcraft">Rustcraft</a>.
	</p>
	<p>
		Textures from the <a href="https://github.com/NovaWostra/Pixel-Perfection-Chorus-Eddit">Pixel Perfection</a>
		resource pack by XSSheep and Nova_Wostra. Not affiliated with Mojang or Microsoft.
	</p>
</footer>

<style>
	:global(:root) {
		--bg: #16130f;
		--surface: #211c17;
		--panel: #1c1813;
		--line: #3a322a;
		--fg: #efe8df;
		--muted: #a99d90;
		--rust: #e8834a;
		--rust-deep: #b04a14;
		--slot: #2c2621;
		--slot-hover: #3b332b;
		--slot-deep: #0d0b09;
		--slot-lit: #52463b;
		--good: #8fc76a;
		--bad: #e26b5b;
		--display: 'Pixelify Sans', ui-monospace, monospace;
		--body: system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif;
		--mono: ui-monospace, 'SFMono-Regular', Menlo, Consolas, monospace;
		color-scheme: dark;
	}
	:global(body) {
		margin: 0;
		font-variant-ligatures: no-common-ligatures;
		background: var(--bg);
		color: var(--fg);
		font-family: var(--body);
		font-size: 1.0625rem;
		line-height: 1.6;
	}
	:global(a) {
		color: var(--rust);
	}
	:global(:focus-visible) {
		outline: 2px solid var(--rust);
		outline-offset: 3px;
	}
	:global(h1, h2, h3) {
		font-family: var(--display);
		line-height: 1.15;
		text-wrap: balance;
		/* The pixel font draws "fi" as one glyph that reads as an R */
		font-variant-ligatures: none;
	}
	:global(code) {
		font-family: var(--mono);
		font-size: 0.9em;
		background: var(--surface);
		padding: 0.05em 0.3em;
		border: 1px solid var(--line);
	}
	:global(kbd) {
		font-family: var(--display);
		font-size: 0.9em;
		padding: 0 0.4em;
		background: var(--slot);
		border: 2px solid;
		border-color: var(--slot-lit) var(--slot-deep) var(--slot-deep) var(--slot-lit);
		white-space: nowrap;
	}

	.skip {
		position: absolute;
		left: -9999px;
	}
	.skip:focus {
		left: 12px;
		top: 12px;
		z-index: 10;
		background: var(--bg);
		padding: 8px 12px;
	}

	.top {
		position: sticky;
		top: 0;
		z-index: 5;
		display: flex;
		align-items: center;
		gap: 20px;
		padding: 10px max(20px, env(safe-area-inset-left, 0px));
		background: rgb(22 19 15 / 0.94);
		backdrop-filter: blur(6px);
		border-bottom: 2px solid var(--line);
	}
	.brand {
		font-family: var(--display);
		font-variant-ligatures: none;
		font-weight: 700;
		font-size: 1.35rem;
		color: var(--fg);
		text-decoration: none;
		white-space: nowrap;
		text-shadow: 0 2px 0 var(--rust-deep);
	}
	.brand span {
		color: var(--rust);
		text-shadow: none;
	}
	.search {
		flex: 1;
		max-width: 520px;
	}
	.game {
		margin-left: auto;
		font-family: var(--display);
		color: var(--fg);
		text-decoration: none;
		white-space: nowrap;
	}
	.game:hover {
		color: var(--rust);
	}
	.menu {
		display: none;
		font: inherit;
		font-family: var(--display);
		color: var(--fg);
		background: var(--slot);
		border: 2px solid var(--line);
		padding: 4px 12px;
	}

	.frame {
		display: grid;
		grid-template-columns: 230px minmax(0, 1fr);
		gap: 40px;
		max-width: 1320px;
		margin: 0 auto;
		padding: 0 max(20px, env(safe-area-inset-left, 0px));
		box-sizing: border-box;
	}
	nav {
		position: sticky;
		top: 64px;
		align-self: start;
		max-height: calc(100vh - 64px);
		overflow-y: auto;
		padding: 20px 4px 40px 0;
		font-size: 0.95rem;
		scrollbar-width: thin;
	}
	nav h2 {
		font-size: 0.95rem;
		font-weight: 500;
		color: var(--muted);
		margin: 18px 0 4px;
	}
	nav h2:first-child {
		margin-top: 0;
	}
	nav ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	nav a {
		display: block;
		padding: 3px 10px;
		color: var(--fg);
		text-decoration: none;
		border-left: 2px solid var(--line);
	}
	nav a:hover {
		color: var(--rust);
		border-left-color: var(--rust);
	}
	nav a[aria-current='page'] {
		color: var(--rust);
		border-left: 4px solid var(--rust);
		padding-left: 8px;
		background: var(--surface);
	}
	main {
		padding: 24px 0 64px;
		min-width: 0;
	}

	footer {
		border-top: 2px solid var(--line);
		padding: 24px max(20px, env(safe-area-inset-left, 0px)) 40px;
		color: var(--muted);
		font-size: 0.9rem;
		max-width: 1320px;
		margin: 0 auto;
		box-sizing: border-box;
	}
	footer p {
		margin: 0 0 4px;
		max-width: 75ch;
	}

	@media (max-width: 900px) {
		.frame {
			grid-template-columns: minmax(0, 1fr);
		}
		.menu {
			display: block;
		}
		.game {
			display: none;
		}
		.top {
			flex-wrap: wrap;
			gap: 10px 14px;
		}
		.search {
			order: 3;
			flex-basis: 100%;
			max-width: none;
		}
		.menu {
			margin-left: auto;
		}
		nav {
			display: none;
		}
		nav.open {
			display: block;
			position: static;
			max-height: none;
			border-bottom: 2px solid var(--line);
		}
	}
</style>
