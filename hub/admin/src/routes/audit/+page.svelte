<script lang="ts">
	import { onDestroy } from 'svelte';
	import { page } from '$app/state';
	import { Button } from '@ui-sdk/svelte/button';
	import { Field, FieldLabel } from '@ui-sdk/svelte/field';
	import { Input, InputClear, InputGroup } from '@ui-sdk/svelte/input';
	import { api, messageOf, type AuditItem } from '#lib/api.ts';
	import AuditTable from '#lib/components/AuditTable.svelte';
	import FilterSelect from '#lib/components/FilterSelect.svelte';
	import { setQuery } from '#lib/url.ts';

	const PAGE_SIZE = 50;

	const actions = [
		'signup',
		'login',
		'login_failed',
		'password_changed',
		'recovery_used',
		'certificate_issued',
		'account_deleted',
		'admin_login',
		'admin_login_refused',
		'admin_logout',
		'role_granted',
		'role_revoked',
		'realm_quota_set',
		'suspended',
		'unsuspended',
		'password_reset',
		'sessions_ended',
		'realm_stopped',
		'realm_archived'
	];
	const actionItems = [
		{ value: 'all', label: 'Any action' },
		...actions.map((action) => ({ value: action, label: action }))
	];

	const accountFilter = $derived(page.url.searchParams.get('account') ?? '');
	const action = $derived(page.url.searchParams.get('action') ?? 'all');

	// svelte-ignore state_referenced_locally
	let accountText = $state(accountFilter);
	let entries = $state<AuditItem[]>([]);
	let nextBefore = $state<number | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);

	let latest = 0;
	async function fetchPage(before?: number) {
		const ticket = ++latest;
		loading = true;
		try {
			const result = await api.audit({
				account: accountFilter || undefined,
				action: action === 'all' ? undefined : action,
				before,
				limit: PAGE_SIZE
			});
			if (ticket !== latest) return;
			entries = before === undefined ? result.entries : [...entries, ...result.entries];
			nextBefore = result.next_before;
			error = null;
		} catch (failure) {
			if (ticket !== latest) return;
			error = messageOf(failure);
		} finally {
			if (ticket === latest) loading = false;
		}
	}

	$effect(() => {
		void accountFilter;
		void action;
		fetchPage();
	});

	let timer: ReturnType<typeof setTimeout> | undefined;
	// A search typed just before leaving must not land in the next page's URL.
	onDestroy(() => clearTimeout(timer));
	function typed(value: string) {
		clearTimeout(timer);
		timer = setTimeout(() => setQuery({ account: value.trim() }), 300);
	}
</script>

<svelte:head>
	<title>Audit log · Rustcraft hub admin</title>
</svelte:head>

<div class="page">
	<div class="page-head">
		<h1>Audit log</h1>
		<Button onclick={() => fetchPage()}>Refresh</Button>
	</div>

	<section class="card flush">
		<form class="filters" role="search" onsubmit={(event) => event.preventDefault()}>
			<Field class="grow">
				<FieldLabel>Account</FieldLabel>
				<InputGroup>
					<Input
						type="search"
						bind:value={accountText}
						onValueChange={typed}
						placeholder="Exact username or account id"
						autocomplete="off"
						spellcheck={false}
					/>
					<InputClear aria-label="Clear the account" />
				</InputGroup>
			</Field>
			<FilterSelect
				label="Action"
				items={actionItems}
				value={action}
				onchange={(next) => setQuery({ action: next === 'all' ? null : next })}
			/>
		</form>

		{#if error}
			<p class="empty form-error" role="alert">{error}</p>
		{:else if loading && entries.length === 0}
			<p class="empty" aria-live="polite">Loading…</p>
		{:else}
			<AuditTable {entries} />
		{/if}

		<footer class="table-foot">
			<span aria-live="polite">{entries.length} entr{entries.length === 1 ? 'y' : 'ies'} shown</span>
			{#if nextBefore !== null}
				<Button onclick={() => fetchPage(nextBefore ?? undefined)} disabled={loading}>
					{loading ? 'Loading…' : 'Load more'}
				</Button>
			{/if}
		</footer>
	</section>
</div>
