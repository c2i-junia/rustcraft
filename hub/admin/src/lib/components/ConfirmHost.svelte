<script lang="ts">
	import {
		AlertDialog,
		AlertDialogBackdrop,
		AlertDialogClose,
		AlertDialogContent,
		AlertDialogDescription,
		AlertDialogTitle
	} from '@ui-sdk/svelte/alert-dialog';
	import type { DialogContentState } from '@ui-sdk/svelte/dialog';
	import { messageOf } from '#lib/api.ts';
	import { ask, type Question } from '#lib/confirm.ts';
</script>

<AlertDialog controller={ask.controller} pendingAnnouncement="Working">
	<AlertDialogBackdrop />
	<AlertDialogContent>
		{#snippet children(state: DialogContentState<Question>)}
			<div class="stack-tight">
				<AlertDialogTitle>{state.payload?.title}</AlertDialogTitle>
				<AlertDialogDescription>{state.payload?.description}</AlertDialogDescription>
			</div>
			<AlertDialogClose
				data-variant={state.payload?.destructive ? 'destructive' : 'solid'}
				value="confirm"
				action={() => state.payload?.run()}
			>
				{#snippet render({ props, state: close })}
					<div class="stack-tight">
						{#if close.error}
							<p class="form-error" role="alert">{messageOf(close.error)}</p>
						{/if}
						<div class="dialog-actions">
							<AlertDialogClose>Cancel</AlertDialogClose>
							<button {...props}>
								{close.pending ? 'Working…' : close.error ? 'Try again' : state.payload?.confirmLabel}
							</button>
						</div>
					</div>
				{/snippet}
			</AlertDialogClose>
		{/snippet}
	</AlertDialogContent>
</AlertDialog>
