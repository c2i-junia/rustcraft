<script lang="ts">
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { Button } from '@ui-sdk/svelte/button';
	import { Meter, MeterIndicator, MeterLabel, MeterTrack, MeterValue } from '@ui-sdk/svelte/meter';
	import { api, messageOf, type AuditItem, type Overview, type RealmState } from '#lib/api.ts';
	import AuditTable from '#lib/components/AuditTable.svelte';

	let overview = $state<Overview | null>(null);
	let recent = $state<AuditItem[]>([]);
	let error = $state<string | null>(null);

	const worldStates: RealmState[] = ['running', 'starting', 'stopping', 'stopped', 'failed', 'archived'];

	async function load() {
		error = null;
		try {
			const [next, audit] = await Promise.all([api.overview(), api.audit({ limit: 10 })]);
			overview = next;
			recent = audit.entries;
		} catch (failure) {
			error = messageOf(failure);
		}
	}

	onMount(load);
</script>

<svelte:head>
	<title>Overview · Rustcraft hub admin</title>
</svelte:head>

<div class="page">
	<div class="page-head">
		<h1>Overview</h1>
		<Button onclick={load}>Refresh</Button>
	</div>

	{#if error}
		<p class="card form-error" role="alert">{error}</p>
	{/if}

	{#if overview}
		<section class="grid" aria-label="Counts">
			<a class="card stat link" href={resolve('/accounts')}>
				<span class="label">Accounts</span>
				<span class="value">{overview.accounts.active}</span>
				<span class="parts">
					<span>active</span>
					<span><b>{overview.accounts.suspended}</b> suspended</span>
					<span><b>{overview.accounts.deleted}</b> deleted</span>
				</span>
			</a>
			<div class="card stat">
				<span class="label">Roles</span>
				<span class="value">{overview.admins}<span class="unit">admin{overview.admins === 1 ? '' : 's'}</span></span>
				<span class="parts"><span><b>{overview.hosts}</b> host{overview.hosts === 1 ? '' : 's'}</span></span>
			</div>
			<div class="card stat">
				<span class="label">Live sessions</span>
				<span class="value">{overview.sessions.game}<span class="unit">in game</span></span>
				<span class="parts"><span><b>{overview.sessions.admin_web}</b> on this admin</span></span>
			</div>
			<div class="card stat">
				<span class="label">Sign-ups</span>
				<span class="value">{overview.signups.last_24h}<span class="unit">in 24 h</span></span>
				<span class="parts"><span><b>{overview.signups.last_7d}</b> in 7 days</span></span>
			</div>
			<a class="card stat link" href="{resolve('/audit')}?action=login_failed">
				<span class="label">Failed logins</span>
				<span class="value" class:alarm={overview.failed_logins_24h > 0}
					>{overview.failed_logins_24h}<span class="unit">in 24 h</span></span
				>
				<span class="parts"><span>wrong passwords, any account</span></span>
			</a>
		</section>

		<section class="grid two">
			<a class="card worlds link" href={resolve('/worlds')}>
				<header><h2>Worlds</h2></header>
				<Meter
					value={overview.running}
					max={Math.max(overview.max_running, 1)}
					format={{ style: 'decimal' }}
				>
					<div class="meter-head">
						<MeterLabel>Slots in use</MeterLabel>
						<span class="meter-value"><MeterValue /> of {overview.max_running}</span>
					</div>
					<MeterTrack>
						<MeterIndicator />
					</MeterTrack>
				</Meter>
				<ul class="states">
					{#each worldStates as state (state)}
						<li data-state={state}>
							<span class="n">{overview.realms[state]}</span>
							<span class="muted">{state}</span>
						</li>
					{/each}
				</ul>
			</a>
		</section>

		<section class="card flush">
			<header>
				<h2>Latest in the audit log</h2>
				<a href={resolve('/audit')}>All entries</a>
			</header>
			<AuditTable entries={recent} label="Latest audit entries" />
		</section>
	{:else if !error}
		<p class="muted" aria-live="polite">Loading…</p>
	{/if}
</div>

<style>
	.link {
		color: inherit;
		text-decoration: none;
		transition: border-color 120ms;
	}
	.link:hover {
		border-color: var(--line-strong);
		text-decoration: none;
	}
	.unit {
		margin-inline-start: 0.3em;
		font-size: 0.875rem;
		font-weight: 500;
		color: var(--muted);
	}
	.alarm {
		color: var(--warn);
	}
	.meter-head {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		margin-bottom: 6px;
		font-size: 0.8125rem;
	}
	.meter-value {
		font-variant-numeric: tabular-nums;
		color: var(--muted);
	}
	.states {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(90px, 1fr));
		gap: 8px;
		margin: 0;
		padding: 0;
		list-style: none;
	}
	.states li {
		display: flex;
		flex-direction: column;
		padding: 8px 10px;
		border-radius: 5px;
		background: var(--surface-2);
		font-size: 0.8125rem;
	}
	.states .n {
		font-size: 1.125rem;
		font-weight: 650;
		font-variant-numeric: tabular-nums;
	}
	.states li[data-state='running'] .n {
		color: var(--ok);
	}
	.states li[data-state='failed'] .n {
		color: var(--danger);
	}
</style>
