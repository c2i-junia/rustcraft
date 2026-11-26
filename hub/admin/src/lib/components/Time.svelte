<script lang="ts">
	import { fullTime, isoTime, sinceNow } from '#lib/format.ts';

	/** Unix seconds. */
	let { at, exact = false }: { at: number | null; exact?: boolean } = $props();
</script>

{#if at === null}
	<span class="faint">never</span>
{:else}
	<time datetime={isoTime(at)} title={exact ? undefined : fullTime(at)}
		>{exact ? fullTime(at) : sinceNow(at)}</time
	>
{/if}
