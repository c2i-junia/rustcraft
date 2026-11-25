<script lang="ts">
	import '@fontsource/pixelify-sans/500.css';
	import '@fontsource/pixelify-sans/700.css';
	import favicon from '#lib/assets/favicon.svg';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
	<meta name="theme-color" content="#16130f" />
</svelte:head>

{@render children()}

<style>
	:global(:root) {
		--bg: #16130f;
		--surface: #211c17;
		--line: #3a322a;
		--fg: #efe8df;
		--muted: #a99d90;
		--rust: #e8834a;
		--rust-deep: #b04a14;
		--display: 'Pixelify Sans', ui-monospace, monospace;
		--body: system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif;
		--mono: ui-monospace, 'SFMono-Regular', Menlo, Consolas, monospace;
		color-scheme: dark;
	}
	:global(body) {
		margin: 0;
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
</style>
