<script lang="ts">
	import type { Role } from '#lib/api.ts';
	import Badge from './Badge.svelte';

	let { roles }: { roles: Role[] } = $props();
</script>

{#if roles.length === 0}
	<span class="faint">—</span>
{:else}
	<span class="badges">
		{#each roles as role (role)}
			<Badge tone={role === 'admin' ? 'accent' : 'info'}>{role}</Badge>
		{/each}
	</span>
{/if}
