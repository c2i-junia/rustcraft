<!-- An item's icon as the game draws it: its 16x16 texture grown without smoothing, or for a
     block of a whole cube, the cube seen from above at an angle. -->
<script lang="ts">
	import type { Ref } from '#lib/refs.ts';

	let { r, size = 32 }: { r: Ref | undefined; size?: number } = $props();

	const url = (path: string) => `url(/textures/${path})`;
	// A grey texture tinted: the texture's own shape, in the colour multiplied with it
	const paint = (path: string, tint: string | null) =>
		tint
			? `background-image:${url(path)};background-color:${tint};background-blend-mode:multiply;mask-image:${url(path)};`
			: `background-image:${url(path)};`;
	let side = $derived(r?.icon ?? null);
	let top = $derived(r?.top ?? side);
	// The side of a grass block keeps its dirt: only its top is tinted
	let sideTint = $derived(r?.id === 'Grass' ? null : (r?.tint ?? null));
</script>

{#if r?.picture}
	<span class="icon picture" style:--s="{size}px" style:background-image="url({r.picture})" aria-hidden="true"></span>
{:else if !r || !side}
	<span class="icon missing" style:--s="{size}px" aria-hidden="true">?</span>
{:else if r.cube}
	<span class="icon cube" style:--s="{size}px" aria-hidden="true">
		<span class="body">
			<span class="face top" style={paint(top!, r.tint)}></span>
			<span class="face left" style={paint(side, sideTint)}></span>
			<span class="face right" style={paint(side, sideTint)}></span>
		</span>
	</span>
{:else}
	<span class="icon flat" style:--s="{size}px" style={paint(side, r.tint)} aria-hidden="true"></span>
{/if}

<style>
	.icon {
		display: inline-block;
		width: var(--s);
		height: var(--s);
		flex: none;
		vertical-align: middle;
	}
	.flat,
	.face {
		image-rendering: pixelated;
		/* An animated texture is a strip of frames: its first one is shown */
		background-size: 100% auto;
		background-repeat: no-repeat;
		mask-size: 100% auto;
		mask-repeat: no-repeat;
	}
	.picture {
		background-size: contain;
		background-position: center;
		background-repeat: no-repeat;
	}
	.missing {
		display: inline-grid;
		place-items: center;
		font-family: var(--display);
		color: var(--muted);
		background: var(--slot-deep);
	}
	/* The three faces seen by the player's eye in the inventory: 30 degrees down, turned 45 */
	.cube {
		--f: calc(var(--s) * 0.66);
		position: relative;
	}
	.body {
		position: absolute;
		left: 50%;
		top: 50%;
		width: var(--f);
		height: var(--f);
		margin: calc(var(--f) / -2);
		transform-style: preserve-3d;
		transform: rotateX(-30deg) rotateY(45deg);
	}
	.face {
		position: absolute;
		inset: 0;
		backface-visibility: hidden;
	}
	.top {
		transform: rotateX(90deg) translateZ(calc(var(--f) / 2));
	}
	/* Turned 45 degrees, the face toward -x shows on the left and the front on the right */
	.left {
		transform: rotateY(-90deg) translateZ(calc(var(--f) / 2));
		filter: brightness(0.82);
	}
	.right {
		transform: translateZ(calc(var(--f) / 2));
		filter: brightness(0.62);
	}
</style>
