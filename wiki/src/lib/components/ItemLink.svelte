<!-- A link to an item or a creature in running text, its small icon before its name. -->
<script lang="ts">
	import { ref } from '#lib/refs.ts';
	import Icon from './Icon.svelte';

	let { id, count, label }: { id: string; count?: number; label?: string } = $props();
	let r = $derived(ref(id) ?? ref(`mob:${id}`));
</script>

{#if r}
	<a class="link" href={r.href}
		><Icon {r} size={20} /><span>{label ?? r.name}</span>{#if count && count > 1}<span class="times">×{count}</span>{/if}</a
	>
{:else}
	<!-- No ref: the page's load did not name this id (see README, "Writing a page") -->
	<span data-missing-ref={id}>{label ?? id}</span>
{/if}

<style>
	.link {
		display: inline-flex;
		align-items: center;
		gap: 0.35em;
		vertical-align: bottom;
		text-decoration: none;
	}
	.link span {
		text-decoration: underline;
		text-decoration-thickness: 1px;
		text-underline-offset: 3px;
		text-decoration-color: color-mix(in srgb, currentColor 40%, transparent);
	}
	.link .times {
		text-decoration: none;
		color: var(--fg);
	}
	.link:hover span {
		text-decoration-color: currentColor;
	}
</style>
