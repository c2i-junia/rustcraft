import { createConfirm } from '@ui-sdk/svelte/alert-dialog';

/** A question before an action. The dialog stays open, its button pending, while `run` runs;
 * a failure shows in it and leaves it open. */
export interface Question {
	title: string;
	description: string;
	confirmLabel: string;
	/** Red, for what cannot be undone or takes something away. */
	destructive?: boolean;
	run: () => Promise<unknown>;
}

/** Resolves true once `run` has succeeded, false when the question was dismissed. */
export const ask = createConfirm<Question>();
