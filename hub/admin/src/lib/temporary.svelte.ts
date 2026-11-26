/** A temporary password the hub has just set, until the admin closes its dialog. It lives here,
 * shown by the layout, so that leaving the account's page cannot lose it; `resetting` holds the
 * app on its page while the hub has not answered yet. */
export const temporary = $state<{
	value: { username: string; password: string } | null;
	resetting: boolean;
}>({
	value: null,
	resetting: false
});
