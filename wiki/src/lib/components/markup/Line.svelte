<!-- A line of the Codex's markup: its runs, emphasis and links. Items and creatures are linked
	with their icon. -->
<script lang="ts">
	import ItemLink from '../ItemLink.svelte';
	import { hrefOf, type Line } from '#lib/markup.ts';

	let { line }: { line: Line } = $props();
</script>

{#each line as s, i (i)}{#if s.link?.kind === 'item'}<ItemLink id={s.link.id} label={s.text} />{:else if s.link?.kind === 'mob'}<ItemLink
			id={`mob:${s.link.id}`}
			label={s.text}
		/>{:else if s.link}<a href={hrefOf(s.link).href}>{s.text ?? hrefOf(s.link).name}</a>{:else if s.strong && s.em}<strong
			><em>{s.text}</em></strong
		>{:else if s.strong}<strong>{s.text}</strong>{:else if s.em}<em>{s.text}</em>{:else}{s.text}{/if}{/each}
