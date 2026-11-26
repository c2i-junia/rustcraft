<script lang="ts">
	import type { Status } from '#lib/api.ts';
	import Badge, { type Tone } from './Badge.svelte';

	let { status }: { status: Status } = $props();

	const tones: Record<Status, Tone> = { active: 'ok', suspended: 'warn', deleted: 'muted' };
</script>

<Badge tone={tones[status]}>{status}</Badge>
