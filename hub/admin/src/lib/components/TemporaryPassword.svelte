<script lang="ts">
	import { Button } from '@ui-sdk/svelte/button';
	import {
		Dialog,
		DialogBackdrop,
		DialogClose,
		DialogContent,
		DialogDescription,
		DialogTitle
	} from '@ui-sdk/svelte/dialog';
	import { toast } from '@ui-sdk/svelte/toast';
	import { temporary } from '#lib/temporary.svelte.ts';

	let copied = $state(false);
	// Shown while there is one; only Done closes it, so a stray Escape cannot lose it.
	let open = $state(false);
	// Opens once the question that led here has gone, so that its return of focus does not take
	// focus out of this dialog.
	$effect(() => {
		if (!temporary.value) return;
		copied = false;
		let frame = 0;
		const show = () => {
			if (document.querySelector('[data-slot="alert-dialog-content"]')) frame = requestAnimationFrame(show);
			else open = true;
		};
		show();
		return () => cancelAnimationFrame(frame);
	});

	async function copy() {
		if (!temporary.value) return;
		try {
			await navigator.clipboard.writeText(temporary.value.password);
			copied = true;
		} catch {
			toast.error('Copy failed', { description: 'Select the password and copy it by hand.' });
		}
	}
</script>

<Dialog
	bind:open
	dismissible={false}
	closeOnOutsidePress={false}
	onOpenChangeComplete={(next) => {
		if (!next) temporary.value = null;
	}}
>
	<DialogBackdrop />
	<DialogContent>
		{#if temporary.value}
			<div class="stack-tight">
				<DialogTitle>Temporary password of {temporary.value.username}</DialogTitle>
				<DialogDescription>
					Give it to {temporary.value.username} through a channel you trust; they should change it after
					logging in. <strong>It will not be shown again.</strong>
				</DialogDescription>
			</div>
			<div class="stack-tight">
				<label class="secret-label" for="temporary-password">Temporary password</label>
				<div class="secret">
					<input
						id="temporary-password"
						class="secret-value"
						readonly
						value={temporary.value.password}
						autocomplete="off"
						spellcheck="false"
						onfocus={(event) => event.currentTarget.select()}
					/>
					<Button onclick={copy}>{copied ? 'Copied' : 'Copy'}</Button>
				</div>
			</div>
			<div class="dialog-actions">
				<DialogClose data-variant="solid">Done</DialogClose>
			</div>
		{/if}
	</DialogContent>
</Dialog>

<style>
	.secret-label {
		font-size: 0.8125rem;
		font-weight: 600;
	}
	.secret {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.secret-value {
		flex: 1;
		min-width: 0;
		height: 42px;
		padding: 0 12px;
		border: 1px dashed var(--line-strong);
		border-radius: 4px;
		background: var(--surface-2);
		color: var(--fg);
		font-family: var(--mono);
		font-size: 1rem;
		letter-spacing: 0.04em;
	}
</style>
