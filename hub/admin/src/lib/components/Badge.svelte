<script lang="ts" module>
	export type Tone = 'ok' | 'warn' | 'danger' | 'info' | 'muted' | 'accent';
</script>

<script lang="ts">
	import type { Snippet } from 'svelte';

	let { tone = 'muted', children }: { tone?: Tone; children: Snippet } = $props();
</script>

<span class="badge" data-tone={tone}>{@render children()}</span>
