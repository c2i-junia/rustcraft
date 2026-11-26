<script lang="ts">
	import type { RealmState } from '#lib/api.ts';
	import Badge, { type Tone } from './Badge.svelte';

	let { state }: { state: RealmState } = $props();

	const tones: Record<RealmState, Tone> = {
		running: 'ok',
		starting: 'info',
		stopping: 'info',
		stopped: 'muted',
		failed: 'danger',
		archived: 'muted'
	};
</script>

<Badge tone={tones[state]}>{state}</Badge>
