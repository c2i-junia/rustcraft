<script lang="ts">
	import { page } from '$app/state';
</script>

<svelte:head><title>Not found – Rustcraft Wiki</title></svelte:head>

<h1>{page.status === 404 ? 'No page here' : 'Something broke'}</h1>
<p>
	{#if page.status === 404}
		Nothing lives at <code>{page.url.pathname}</code>. Try the search box at the top, or start from the
		<a href="/">home page</a>, <a href="/items">all items</a> or <a href="/creatures">all creatures</a>.
	{:else}
		{page.error?.message}
	{/if}
</p>

<style>
	h1 {
		font-size: 2.6rem;
		text-shadow: 0 3px 0 var(--rust-deep);
	}
</style>
