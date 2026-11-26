<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { Button } from '@ui-sdk/svelte/button';
	import { toast } from '@ui-sdk/svelte/toast';
	import { api, messageOf, type RealmItem } from '#lib/api.ts';
	import { ask } from '#lib/confirm.ts';
	import ActionButton from '#lib/components/ActionButton.svelte';
	import FilterSelect from '#lib/components/FilterSelect.svelte';
	import RealmStateBadge from '#lib/components/RealmStateBadge.svelte';
	import Time from '#lib/components/Time.svelte';
	import { setQuery } from '#lib/url.ts';

	const stateItems = [
		{ value: 'all', label: 'All but archived' },
		{ value: 'running', label: 'Running' },
		{ value: 'starting', label: 'Starting' },
		{ value: 'stopping', label: 'Stopping' },
		{ value: 'stopped', label: 'Stopped' },
		{ value: 'failed', label: 'Failed' },
		{ value: 'archived', label: 'Archived' }
	];

	const filter = $derived(page.url.searchParams.get('state') ?? 'all');

	let realms = $state<RealmItem[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	let latest = 0;
	async function load() {
		const ticket = ++latest;
		const state = filter === 'all' ? undefined : filter;
		loading = true;
		try {
			const result = await api.realms(state);
			if (ticket !== latest) return;
			realms = result.realms;
			error = null;
		} catch (failure) {
			if (ticket !== latest) return;
			error = messageOf(failure);
		} finally {
			if (ticket === latest) loading = false;
		}
	}

	$effect(() => {
		void filter;
		load();
	});

	const live = new Set(['starting', 'running', 'stopping']);

	async function stop(realm: RealmItem) {
		const ok = await ask({
			title: `Stop “${realm.name}”?`,
			description: `The world of ${realm.owner.username} shuts down${realm.players > 0 ? `, and its ${realm.players} player${realm.players === 1 ? '' : 's'} are disconnected` : ''}. Its owner can start it again.`,
			confirmLabel: 'Stop world',
			destructive: true,
			run: () => api.stopRealm(realm.id)
		});
		if (ok) {
			toast.success(`“${realm.name}” is stopping.`, { description: 'It stops in the background.' });
			await load();
		}
	}

	async function archive(realm: RealmItem) {
		const ok = await ask({
			title: `Archive “${realm.name}”?`,
			description: `The world of ${realm.owner.username} leaves every list at once and its files are moved aside, as when its owner deletes it. It cannot be started again.`,
			confirmLabel: 'Archive world',
			destructive: true,
			run: () => api.archiveRealm(realm.id)
		});
		if (ok) {
			toast.success(`“${realm.name}” is archived.`);
			await load();
		}
	}
</script>

<svelte:head>
	<title>Worlds · Rustcraft hub admin</title>
</svelte:head>

<div class="page">
	<div class="page-head">
		<h1>Worlds</h1>
		<Button onclick={load}>Refresh</Button>
	</div>

	<section class="card flush">
		<div class="filters">
			<FilterSelect
				label="State"
				items={stateItems}
				value={filter}
				onchange={(next) => setQuery({ state: next === 'all' ? null : next })}
			/>
		</div>

		{#if error}
			<p class="empty form-error" role="alert">{error}</p>
		{:else if !loading && realms.length === 0}
			<p class="empty">
				{filter === 'all' ? 'No hosted world yet.' : `No world is ${filter}.`}
			</p>
		{:else}
			<div class="table-wrap" aria-busy={loading}>
				<table aria-label="Worlds">
					<thead>
						<tr>
							<th scope="col">Name</th>
							<th scope="col">Owner</th>
							<th scope="col">State</th>
							<th scope="col" class="num hide-sm">Slot / port</th>
							<th scope="col" class="num">Players</th>
							<th scope="col" class="hide-sm">Last played</th>
							<th scope="col"><span class="visually-hidden">Actions</span></th>
						</tr>
					</thead>
					<tbody>
						{#each realms as realm (realm.id)}
							<tr>
								<td>{realm.name}</td>
								<td>
									<a href={resolve('/accounts/[id]', { id: realm.owner.id })}>{realm.owner.username}</a>
								</td>
								<td>
									<RealmStateBadge state={realm.state} />
									{#if realm.failed_reason}
										<span class="reason" title={realm.failed_reason}>{realm.failed_reason}</span>
									{/if}
								</td>
								<td class="num hide-sm">
									{#if realm.slot !== null}{realm.slot}{#if realm.port !== null}<span class="faint"> / {realm.port}</span>{/if}{:else}<span class="faint">—</span>{/if}
								</td>
								<td class="num">{realm.players}</td>
								<td class="hide-sm nowrap"><Time at={realm.last_played_at} /></td>
								<td class="actions">
									{#if live.has(realm.state)}
										<ActionButton variant="destructive" onclick={() => stop(realm)}>Stop</ActionButton>
									{/if}
									{#if realm.state !== 'archived'}
										<ActionButton variant="destructive" onclick={() => archive(realm)}>Archive</ActionButton>
									{/if}
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{/if}
		<footer class="table-foot">
			<span aria-live="polite">{loading ? 'Loading…' : `${realms.length} world${realms.length === 1 ? '' : 's'}`}</span>
		</footer>
	</section>
</div>
