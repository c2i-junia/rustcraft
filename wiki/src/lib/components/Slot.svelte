<!-- A slot of the inventory, with an item or a family of items in it, and how many. A family
     shows each of its members in turn, as the item browser of the game does. -->
<script lang="ts">
	import { ref } from '#lib/refs.ts';
	import Icon from './Icon.svelte';

	let {
		id,
		count = 1,
		size = 32,
		big = false,
		link = true
	}: { id: string | null; count?: number; size?: number; big?: boolean; link?: boolean } = $props();

	let r = $derived(id ? ref(id) : undefined);
	let label = $derived(r ? (count > 1 ? `${count} × ${r.name}` : r.name) : undefined);
</script>

{#if !id}
	<span class="slot" class:big aria-hidden="true"></span>
{:else if link && r}
	<a class="slot" class:big href={r.href} title={label} aria-label={label}>
		<Icon {r} size={big ? size * 1.5 : size} />
		{#if count > 1}<span class="count">{count}</span>{/if}
	</a>
{:else}
	<span class="slot" class:big title={label}>
		<Icon {r} size={big ? size * 1.5 : size} />
		{#if count > 1}<span class="count">{count}</span>{/if}
	</span>
{/if}

<style>
	.slot {
		position: relative;
		display: inline-grid;
		place-items: center;
		width: 44px;
		height: 44px;
		box-sizing: border-box;
		background: var(--slot);
		border: 2px solid;
		border-color: var(--slot-deep) var(--slot-lit) var(--slot-lit) var(--slot-deep);
		text-decoration: none;
		flex: none;
	}
	.slot.big {
		width: 64px;
		height: 64px;
	}
	a.slot:hover {
		background: var(--slot-hover);
	}
	.count {
		position: absolute;
		right: 2px;
		bottom: -2px;
		font-family: var(--display);
		font-weight: 700;
		font-size: 0.95rem;
		color: #fff;
		text-shadow: 2px 2px 0 #3e3e3e;
		pointer-events: none;
	}
</style>
