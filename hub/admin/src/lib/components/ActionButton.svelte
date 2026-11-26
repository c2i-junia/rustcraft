<script lang="ts">
	import { Button } from '@ui-sdk/svelte/button';
	import { Tooltip, TooltipContent, TooltipTrigger } from '@ui-sdk/svelte/tooltip';
	import type { Snippet } from 'svelte';

	interface Props {
		variant?: 'solid' | 'destructive' | 'plain';
		/** Why the action is unavailable: the button stays focusable and says why. */
		blocked?: string | null;
		onclick: () => unknown;
		children: Snippet;
	}

	let { variant = 'plain', blocked = null, onclick, children }: Props = $props();
	const dataVariant = $derived(variant === 'plain' ? undefined : variant);
</script>

{#if blocked}
	<Tooltip>
		<TooltipTrigger data-variant={dataVariant} aria-disabled="true" data-blocked="">
			{@render children()}
		</TooltipTrigger>
		<TooltipContent side="top" sideOffset={6}>{blocked}</TooltipContent>
	</Tooltip>
{:else}
	<Button data-variant={dataVariant} onclick={() => onclick()}>{@render children()}</Button>
{/if}
