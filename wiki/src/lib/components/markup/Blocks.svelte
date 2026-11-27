<!-- The blocks of a text of the Codex's markup. A recipe or a table the game works out is drawn
	by the page, through `extra`. -->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import Line from './Line.svelte';
	import type { Block } from '#lib/markup.ts';

	let { blocks, extra }: { blocks: Block[]; extra?: Snippet<[Block]> } = $props();
</script>

{#each blocks as b, i (i)}
	{#if b.type === 'heading'}
		{#if b.level === 1}<h1><Line line={b.text} /></h1>
		{:else if b.level === 2}<h2><Line line={b.text} /></h2>
		{:else}<h3><Line line={b.text} /></h3>{/if}
	{:else if b.type === 'paragraph'}
		<p><Line line={b.text} /></p>
	{:else if b.type === 'list'}
		{#if b.ordered}
			<ol>{#each b.items as item, j (j)}<li><Line line={item} /></li>{/each}</ol>
		{:else}
			<ul>{#each b.items as item, j (j)}<li><Line line={item} /></li>{/each}</ul>
		{/if}
	{:else if b.type === 'table'}
		<table>
			{#if b.head}
				<thead><tr>{#each b.head as cell, j (j)}<th class:n={b.numeric[j]}><Line line={cell} /></th>{/each}</tr></thead>
			{/if}
			<tbody>
				{#each b.rows as row, j (j)}
					<tr>{#each row as cell, k (k)}<td class:n={b.numeric[k]}><Line line={cell} /></td>{/each}</tr>
				{/each}
			</tbody>
		</table>
	{:else if extra}
		{@render extra(b)}
	{/if}
{/each}
