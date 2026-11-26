<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { Field, FieldLabel } from '@ui-sdk/svelte/field';
	import { Input, InputClear, InputGroup } from '@ui-sdk/svelte/input';
	import {
		Pagination,
		PaginationEllipsis,
		PaginationItem,
		PaginationLink,
		PaginationList,
		PaginationNext,
		PaginationPrevious
	} from '@ui-sdk/svelte/pagination';
	import { api, messageOf, type AccountItem } from '#lib/api.ts';
	import FilterSelect from '#lib/components/FilterSelect.svelte';
	import RoleBadges from '#lib/components/RoleBadges.svelte';
	import StatusBadge from '#lib/components/StatusBadge.svelte';
	import Time from '#lib/components/Time.svelte';
	import { setQuery } from '#lib/url.ts';

	const PAGE_SIZE = 50;

	const statusItems = [
		{ value: 'all', label: 'Any status' },
		{ value: 'active', label: 'Active' },
		{ value: 'suspended', label: 'Suspended' },
		{ value: 'deleted', label: 'Deleted' }
	];
	const roleItems = [
		{ value: 'all', label: 'Any role' },
		{ value: 'admin', label: 'Admin' },
		{ value: 'host', label: 'Host' }
	];

	const q = $derived(page.url.searchParams.get('q') ?? '');
	const status = $derived(page.url.searchParams.get('status') ?? 'all');
	const role = $derived(page.url.searchParams.get('role') ?? 'all');
	const pageNumber = $derived(Math.max(1, Math.floor(Number(page.url.searchParams.get('page'))) || 1));

	// svelte-ignore state_referenced_locally
	let search = $state(q);
	let accounts = $state<AccountItem[]>([]);
	let total = $state(0);
	let loading = $state(true);
	let error = $state<string | null>(null);

	// Each change of the query loads its page; a slower answer to an older query is dropped.
	let latest = 0;
	$effect(() => {
		const query = {
			q,
			status: status === 'all' ? undefined : status,
			role: role === 'all' ? undefined : role,
			offset: (pageNumber - 1) * PAGE_SIZE,
			limit: PAGE_SIZE
		};
		const ticket = ++latest;
		loading = true;
		api.accounts(query).then(
			(result) => {
				if (ticket !== latest) return;
				accounts = result.accounts;
				total = result.total;
				error = null;
				loading = false;
			},
			(failure) => {
				if (ticket !== latest) return;
				error = messageOf(failure);
				loading = false;
			}
		);
	});

	let timer: ReturnType<typeof setTimeout> | undefined;
	function typed(value: string) {
		clearTimeout(timer);
		timer = setTimeout(() => setQuery({ q: value.trim(), page: null }), 250);
	}

	function open(account: AccountItem) {
		goto(resolve('/accounts/[id]', { id: account.id }));
	}

	const first = $derived(total === 0 ? 0 : (pageNumber - 1) * PAGE_SIZE + 1);
	const last = $derived(Math.min(pageNumber * PAGE_SIZE, total));
</script>

<svelte:head>
	<title>Accounts · Rustcraft hub admin</title>
</svelte:head>

<div class="page">
	<div class="page-head">
		<h1>Accounts</h1>
	</div>

	<section class="card flush">
		<form class="filters" role="search" onsubmit={(event) => event.preventDefault()}>
			<Field class="grow">
				<FieldLabel>Search by name</FieldLabel>
				<InputGroup>
					<Input
						type="search"
						bind:value={search}
						onValueChange={typed}
						placeholder="Part of a username"
						autocomplete="off"
						spellcheck={false}
						maxlength={64}
					/>
					<InputClear aria-label="Clear the search" />
				</InputGroup>
			</Field>
			<FilterSelect
				label="Status"
				items={statusItems}
				value={status}
				onchange={(next) => setQuery({ status: next === 'all' ? null : next, page: null })}
			/>
			<FilterSelect
				label="Role"
				items={roleItems}
				value={role}
				onchange={(next) => setQuery({ role: next === 'all' ? null : next, page: null })}
			/>
		</form>

		{#if error}
			<p class="empty form-error" role="alert">{error}</p>
		{:else if !loading && accounts.length === 0}
			<p class="empty">No account matches.</p>
		{:else}
			<div class="table-wrap" aria-busy={loading}>
				<table aria-label="Accounts">
					<thead>
						<tr>
							<th scope="col">Username</th>
							<th scope="col">Roles</th>
							<th scope="col" class="num">World quota</th>
							<th scope="col">Status</th>
							<th scope="col" class="hide-sm">Created</th>
						</tr>
					</thead>
					<tbody>
						{#each accounts as account (account.id)}
							<tr class="clickable" onclick={() => open(account)}>
								<td>
									<a href={resolve('/accounts/[id]', { id: account.id })} onclick={(e) => e.stopPropagation()}
										>{account.username}</a
									>
								</td>
								<td><RoleBadges roles={account.roles} /></td>
								<td class="num">
									{#if account.roles.includes('host')}{account.realm_quota}{:else}<span class="faint"
											>{account.realm_quota}</span
										>{/if}
								</td>
								<td>
									<StatusBadge status={account.status} />
									{#if account.status_reason}
										<span class="reason" title={account.status_reason}>{account.status_reason}</span>
									{/if}
								</td>
								<td class="hide-sm nowrap"><Time at={account.created_at} /></td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{/if}

		<footer class="table-foot">
			<span aria-live="polite">
				{#if loading}Loading…{:else if total > 0}{first}–{last} of {total}{:else}0 accounts{/if}
			</span>
			{#if total > PAGE_SIZE}
				<Pagination
					aria-label="Pages of accounts"
					page={pageNumber}
					{total}
					pageSize={PAGE_SIZE}
					onPageChange={(next) => setQuery({ page: next === 1 ? null : next })}
				>
					<PaginationList>
						{#snippet children({ items })}
							<PaginationItem>
								<PaginationPrevious aria-label="Previous page">‹</PaginationPrevious>
							</PaginationItem>
							{#each items as item (item)}
								<PaginationItem>
									{#if typeof item === 'number'}
										<PaginationLink page={item} />
									{:else}
										<PaginationEllipsis />
									{/if}
								</PaginationItem>
							{/each}
							<PaginationItem>
								<PaginationNext aria-label="Next page">›</PaginationNext>
							</PaginationItem>
						{/snippet}
					</PaginationList>
				</Pagination>
			{/if}
		</footer>
	</section>
</div>
