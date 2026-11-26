<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { Button } from '@ui-sdk/svelte/button';
	import {
		Dialog,
		DialogBackdrop,
		DialogClose,
		DialogContent,
		DialogDescription,
		DialogTitle
	} from '@ui-sdk/svelte/dialog';
	import { Field, FieldDescription, FieldError, FieldLabel } from '@ui-sdk/svelte/field';
	import { Input } from '@ui-sdk/svelte/input';
	import {
		NumberInput,
		NumberInputControl,
		NumberInputDecrement,
		NumberInputGroup,
		NumberInputIncrement
	} from '@ui-sdk/svelte/number-input';
	import { Tabs, TabsList, TabsPanel, TabsTab } from '@ui-sdk/svelte/tabs';
	import { toast } from '@ui-sdk/svelte/toast';
	import { api, ApiError, messageOf, type AccountDetail } from '#lib/api.ts';
	import { ask } from '#lib/confirm.ts';
	import ActionButton from '#lib/components/ActionButton.svelte';
	import AuditTable from '#lib/components/AuditTable.svelte';
	import RealmStateBadge from '#lib/components/RealmStateBadge.svelte';
	import StatusBadge from '#lib/components/StatusBadge.svelte';
	import Time from '#lib/components/Time.svelte';
	import { plural } from '#lib/format.ts';
	import { session } from '#lib/session.svelte.ts';

	const id = $derived(page.params.id ?? '');

	let account = $state<AccountDetail | null>(null);
	let missing = $state(false);
	let error = $state<string | null>(null);

	async function load() {
		const wanted = id;
		try {
			const next = await api.account(wanted);
			if (wanted !== id) return;
			account = next;
			quota = next.realm_quota;
			error = null;
		} catch (failure) {
			if (wanted !== id) return;
			if (failure instanceof ApiError && failure.status === 404) missing = true;
			else error = messageOf(failure);
		}
	}

	$effect(() => {
		void id;
		account = null;
		missing = false;
		load();
	});

	const self = $derived(account !== null && session.me?.id === account.id);
	const deleted = $derived(account?.status === 'deleted');
	const isAdmin = $derived(account?.roles.includes('admin') ?? false);
	const isHost = $derived(account?.roles.includes('host') ?? false);

	function blockedFor(selfReason: string | null): string | null {
		if (deleted) return 'This account is deleted: nothing can be changed.';
		if (self && selfReason) return selfReason;
		return null;
	}

	/** Asks, runs, then tells and reloads. */
	async function confirmThen(
		question: Parameters<typeof ask>[0],
		done: string
	): Promise<void> {
		if (await ask(question)) {
			toast.success(done);
			await load();
		}
	}

	// Roles

	function grantAdmin() {
		if (!account) return;
		const { id, username } = account;
		confirmThen(
			{
				title: `Make ${username} an admin?`,
				description: `${username} will be able to log in here and change every account, world and role, including yours.`,
				confirmLabel: 'Grant admin',
				run: () => api.grant(id, 'admin')
			},
			`${username} is now an admin.`
		);
	}

	function revoke(role: 'admin' | 'host') {
		if (!account) return;
		const { id, username } = account;
		confirmThen(
			{
				title: `Revoke ${role} from ${username}?`,
				description:
					role === 'admin'
						? `${username} loses access to this admin at once, sessions here included.`
						: `${username} can no longer create or start hosted worlds. Existing worlds stay.`,
				confirmLabel: `Revoke ${role}`,
				destructive: true,
				run: () => api.revoke(id, role)
			},
			`${username} is no longer ${role}.`
		);
	}

	let grantOpen = $state(false);
	let grantQuota = $state<number | null>(1);
	let grantError = $state<string | null>(null);

	async function grantHost(event: SubmitEvent) {
		event.preventDefault();
		if (!account || grantQuota === null) return;
		grantError = null;
		try {
			await api.grant(account.id, 'host', grantQuota);
			grantOpen = false;
			toast.success(`${account.username} is now a host, with ${plural(grantQuota, 'world')}.`);
			await load();
		} catch (failure) {
			grantError = messageOf(failure);
		}
	}

	// Quota

	let quota = $state<number | null>(null);
	const quotaChanged = $derived(account !== null && quota !== null && quota !== account.realm_quota);

	async function saveQuota() {
		if (!account || quota === null) return;
		try {
			await api.setQuota(account.id, quota);
			toast.success(`Quota of ${account.username} set to ${plural(quota, 'world')}.`);
			await load();
		} catch (failure) {
			toast.error('The quota was not changed', { description: messageOf(failure) });
		}
	}

	// Suspension

	let suspendOpen = $state(false);
	let reason = $state('');
	let suspendError = $state<string | null>(null);

	async function suspend(event: SubmitEvent) {
		event.preventDefault();
		if (!account) return;
		suspendError = null;
		try {
			await api.suspend(account.id, reason.trim());
			suspendOpen = false;
			reason = '';
			toast.success(`${account.username} is suspended.`, {
				description: 'Their sessions ended and their certificates were revoked.'
			});
			await load();
		} catch (failure) {
			suspendError = messageOf(failure);
		}
	}

	async function unsuspend() {
		if (!account) return;
		try {
			await api.unsuspend(account.id);
			toast.success(`${account.username} can log in again.`);
			await load();
		} catch (failure) {
			toast.error('Not unsuspended', { description: messageOf(failure) });
		}
	}

	// Password and sessions

	let temporary = $state<{ username: string; password: string } | null>(null);
	let passwordOpen = $state(false);
	let copied = $state(false);

	async function resetPassword() {
		if (!account) return;
		const { id, username } = account;
		let password = '';
		const done = await ask({
			title: `Reset the password of ${username}?`,
			description: `The current password stops working, every session of ${username} ends and their certificates are revoked. You will see a temporary password once.`,
			confirmLabel: 'Reset password',
			destructive: true,
			run: async () => {
				password = (await api.resetPassword(id)).password;
			}
		});
		if (!done) return;
		temporary = { username, password };
		copied = false;
		passwordOpen = true;
		await load();
	}

	async function copy() {
		if (!temporary) return;
		try {
			await navigator.clipboard.writeText(temporary.password);
			copied = true;
		} catch {
			toast.error('Copy failed', { description: 'Select the password and copy it by hand.' });
		}
	}

	function endSessions() {
		if (!account) return;
		const { id, username } = account;
		confirmThen(
			{
				title: `End every session of ${username}?`,
				description: `${username} is logged out of the game and of this admin everywhere, and their certificates are revoked. Their password stays.`,
				confirmLabel: 'End sessions',
				destructive: true,
				run: () => api.endSessions(id)
			},
			`Every session of ${username} ended.`
		);
	}
</script>

<svelte:head>
	<title>{account?.username ?? 'Account'} · Rustcraft hub admin</title>
</svelte:head>

<div class="page">
	<nav class="crumb" aria-label="Breadcrumb"><a href={resolve('/accounts')}>Accounts</a> /</nav>

	{#if missing}
		<div class="card"><h1>No such account</h1><p class="muted">It may have been removed.</p></div>
	{:else if error && !account}
		<p class="card form-error" role="alert">{error}</p>
	{:else if !account}
		<p class="muted" aria-live="polite">Loading…</p>
	{:else}
		<div class="page-head">
			<div class="title">
				<h1>{account.username}</h1>
				<StatusBadge status={account.status} />
				{#each account.roles as role (role)}
					<span class="badge" data-tone={role === 'admin' ? 'accent' : 'info'}>{role}</span>
				{/each}
				{#if self}<span class="faint">(you)</span>{/if}
			</div>
			<p class="sub">
				Joined <Time at={account.created_at} exact /> · <span class="mono-small">{account.id}</span>
			</p>
		</div>

		{#if account.status === 'suspended'}
			<div class="banner" role="status">
				<div>
					<strong>Suspended.</strong>
					{account.status_reason ?? 'No reason was given.'}
				</div>
				<ActionButton variant="solid" blocked={blockedFor(null)} onclick={unsuspend}>Unsuspend</ActionButton>
			</div>
		{:else if account.status === 'deleted'}
			<div class="banner muted-banner" role="status">
				<div>
					<strong>Deleted</strong>
					{#if account.deleted_at}<Time at={account.deleted_at} />{/if}. The account is kept for the
					record; nothing can be changed.
				</div>
			</div>
		{/if}

		<div class="grid two">
			<section class="card" aria-labelledby="roles-title">
				<header><h2 id="roles-title">Roles</h2></header>
				<div class="role-row">
					<div>
						<div class="role-name">Admin</div>
						<p class="muted small">Logs in here and changes everything.</p>
					</div>
					{#if isAdmin}
						<ActionButton
							variant="destructive"
							blocked={blockedFor('You cannot revoke your own admin role.')}
							onclick={() => revoke('admin')}>Revoke</ActionButton
						>
					{:else}
						<ActionButton blocked={blockedFor(null)} onclick={grantAdmin}>Grant</ActionButton>
					{/if}
				</div>
				<div class="role-row">
					<div>
						<div class="role-name">Host</div>
						<p class="muted small">Creates and runs hosted worlds, up to a quota.</p>
					</div>
					{#if isHost}
						<ActionButton variant="destructive" blocked={blockedFor(null)} onclick={() => revoke('host')}
							>Revoke</ActionButton
						>
					{:else}
						<ActionButton
							blocked={blockedFor(null)}
							onclick={() => {
								grantQuota = 1;
								grantError = null;
								grantOpen = true;
							}}>Grant</ActionButton
						>
					{/if}
				</div>
				<form
					class="quota"
					onsubmit={(event) => {
						event.preventDefault();
						saveQuota();
					}}
				>
					<Field disabled={deleted}>
						<FieldLabel>World quota</FieldLabel>
						<div class="row">
							<NumberInput bind:value={quota} min={0} max={100} step={1}>
								<NumberInputGroup>
									<NumberInputDecrement aria-label="Fewer worlds" />
									<NumberInputControl />
									<NumberInputIncrement aria-label="More worlds" />
								</NumberInputGroup>
							</NumberInput>
							<Button type="submit" disabled={!quotaChanged || deleted}>Save</Button>
						</div>
						<FieldDescription
							>How many worlds {account.username} may own, 0 to 100.{isHost
								? ''
								: ' It counts once the account is a host.'}</FieldDescription
						>
					</Field>
				</form>
			</section>

			<section class="card" aria-labelledby="security-title">
				<header><h2 id="security-title">Access</h2></header>
				<dl class="facts">
					<dt>Live sessions</dt>
					<dd>{account.sessions.length}</dd>
					<dt>Certificates</dt>
					<dd>{plural(account.active_certificates, 'active certificate')}</dd>
					<dt>Worlds</dt>
					<dd>{account.realms.filter((realm) => realm.state !== 'archived').length} owned</dd>
				</dl>
				<div class="danger-zone">
					{#if account.status !== 'suspended'}
						<ActionButton
							variant="destructive"
							blocked={blockedFor('You cannot suspend yourself.')}
							onclick={() => {
								reason = '';
								suspendError = null;
								suspendOpen = true;
							}}>Suspend…</ActionButton
						>
					{/if}
					<ActionButton
						variant="destructive"
						blocked={blockedFor('You cannot reset your own password here.')}
						onclick={resetPassword}>Reset password…</ActionButton
					>
					<ActionButton
						variant="destructive"
						blocked={blockedFor('You cannot end your own sessions here.')}
						onclick={endSessions}>End all sessions…</ActionButton
					>
				</div>
			</section>
		</div>

		<section class="card flush">
			<Tabs defaultValue="sessions">
				<TabsList aria-label="Details of the account">
					<TabsTab value="sessions">Sessions <span class="count">{account.sessions.length}</span></TabsTab>
					<TabsTab value="worlds">Worlds <span class="count">{account.realms.length}</span></TabsTab>
					<TabsTab value="audit">Audit log <span class="count">{account.audit.length}</span></TabsTab>
				</TabsList>
				<TabsPanel value="sessions">
					{#if account.sessions.length === 0}
						<p class="empty">No live session.</p>
					{:else}
						<div class="table-wrap">
							<table aria-label="Live sessions">
								<thead>
									<tr>
										<th scope="col">Kind</th>
										<th scope="col">Device</th>
										<th scope="col">Started</th>
										<th scope="col">Last used</th>
										<th scope="col">Expires</th>
									</tr>
								</thead>
								<tbody>
									{#each account.sessions as item (item.id)}
										<tr>
											<td>
												<span class="badge" data-tone={item.kind === 'admin_web' ? 'accent' : 'info'}
													>{item.kind === 'admin_web' ? 'web admin' : 'game'}</span
												>
											</td>
											<td>{item.device_name ?? '—'}</td>
											<td class="nowrap"><Time at={item.created_at} /></td>
											<td class="nowrap"><Time at={item.last_used_at} /></td>
											<td class="nowrap"><Time at={item.expires_at} /></td>
										</tr>
									{/each}
								</tbody>
							</table>
						</div>
					{/if}
				</TabsPanel>
				<TabsPanel value="worlds">
					{#if account.realms.length === 0}
						<p class="empty">{account.username} owns no world.</p>
					{:else}
						<div class="table-wrap">
							<table aria-label="Worlds owned">
								<thead>
									<tr>
										<th scope="col">Name</th>
										<th scope="col">State</th>
										<th scope="col" class="num">Players</th>
										<th scope="col">Last played</th>
										<th scope="col">Created</th>
									</tr>
								</thead>
								<tbody>
									{#each account.realms as realm (realm.id)}
										<tr>
											<td>{realm.name}</td>
											<td>
												<RealmStateBadge state={realm.state} />
												{#if realm.failed_reason}<span class="reason" title={realm.failed_reason}
														>{realm.failed_reason}</span
													>{/if}
											</td>
											<td class="num">{realm.players}</td>
											<td class="nowrap"><Time at={realm.last_played_at} /></td>
											<td class="nowrap"><Time at={realm.created_at} /></td>
										</tr>
									{/each}
								</tbody>
							</table>
						</div>
					{/if}
				</TabsPanel>
				<TabsPanel value="audit">
					<AuditTable entries={account.audit} showAccount={false} label="Audit log of the account" />
					<div class="table-foot">
						<span>The latest 50 entries.</span>
						<a href="{resolve('/audit')}?account={encodeURIComponent(account.username)}">All of them</a>
					</div>
				</TabsPanel>
			</Tabs>
		</section>

		<!-- Grant host, with its quota -->
		<Dialog bind:open={grantOpen}>
			<DialogBackdrop />
			<DialogContent>
				<div class="stack-tight">
					<DialogTitle>Make {account.username} a host</DialogTitle>
					<DialogDescription>A host creates and runs worlds on the hub, as many as the quota.</DialogDescription>
				</div>
				<form class="stack" onsubmit={grantHost}>
					<Field invalid={grantError ? true : undefined}>
						<FieldLabel>World quota</FieldLabel>
						<NumberInput bind:value={grantQuota} min={0} max={100} step={1} required>
							<NumberInputGroup>
								<NumberInputDecrement aria-label="Fewer worlds" />
								<NumberInputControl />
								<NumberInputIncrement aria-label="More worlds" />
							</NumberInputGroup>
						</NumberInput>
						<FieldError match>{grantError}</FieldError>
					</Field>
					<div class="dialog-actions">
						<DialogClose>Cancel</DialogClose>
						<Button type="submit" data-variant="solid" disabled={grantQuota === null}>Grant host</Button>
					</div>
				</form>
			</DialogContent>
		</Dialog>

		<!-- Suspend, with a reason -->
		<Dialog bind:open={suspendOpen}>
			<DialogBackdrop />
			<DialogContent>
				<div class="stack-tight">
					<DialogTitle>Suspend {account.username}?</DialogTitle>
					<DialogDescription>
						{account.username} is logged out everywhere, their certificates are revoked, and they cannot log
						in until you unsuspend them. They see the reason when they try.
					</DialogDescription>
				</div>
				<form class="stack" onsubmit={suspend}>
					<Field invalid={suspendError ? true : undefined}>
						<FieldLabel>Reason</FieldLabel>
						<Input
							bind:value={reason}
							required
							maxlength={200}
							autocomplete="off"
							onValueChange={() => (suspendError = null)}
						/>
						<FieldDescription>Up to 200 characters. {200 - reason.length} left.</FieldDescription>
						<FieldError match="valueMissing">Give a reason.</FieldError>
						{#if suspendError}<FieldError match>{suspendError}</FieldError>{/if}
					</Field>
					<div class="dialog-actions">
						<DialogClose>Cancel</DialogClose>
						<Button type="submit" data-variant="destructive">Suspend</Button>
					</div>
				</form>
			</DialogContent>
		</Dialog>
	{/if}
</div>

<!-- The temporary password, shown once -->
<Dialog
	bind:open={passwordOpen}
	closeOnOutsidePress={false}
	onOpenChangeComplete={(open) => {
		if (!open) temporary = null;
	}}
>
	<DialogBackdrop />
	<DialogContent>
		{#if temporary}
			<div class="stack-tight">
				<DialogTitle>Temporary password of {temporary.username}</DialogTitle>
				<DialogDescription>
					Give it to {temporary.username} through a channel you trust; they should change it after logging in.
					<strong>It will not be shown again.</strong>
				</DialogDescription>
			</div>
			<div class="secret">
				<code aria-label="Temporary password">{temporary.password}</code>
				<Button onclick={copy}>{copied ? 'Copied' : 'Copy'}</Button>
			</div>
			<div class="dialog-actions">
				<DialogClose data-variant="solid">Done</DialogClose>
			</div>
		{/if}
	</DialogContent>
</Dialog>

<style>
	.small {
		font-size: 0.8125rem;
	}
	.banner {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding: 12px 16px;
		border: 1px solid color-mix(in oklab, var(--warn) 40%, transparent);
		border-radius: var(--radius);
		background: color-mix(in oklab, var(--warn) 10%, var(--surface));
	}
	.muted-banner {
		border-color: var(--line);
		background: var(--surface-2);
	}
	.role-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding-bottom: 12px;
		border-bottom: 1px solid var(--line);
	}
	.role-name {
		font-weight: 600;
	}
	.quota :global([data-slot='number-input-group']) {
		width: 130px;
	}
	.danger-zone {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		padding-top: 12px;
		border-top: 1px solid var(--line);
	}
	.count {
		margin-inline-start: 4px;
		padding: 0 6px;
		border-radius: 999px;
		background: var(--surface-2);
		color: var(--muted);
		font-size: 0.75rem;
		font-variant-numeric: tabular-nums;
	}
	.card :global([data-slot='tabs-list']) {
		padding: 0 8px;
		border-bottom: 1px solid var(--line);
		overflow-x: auto;
	}
	.secret {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.secret code {
		flex: 1;
		padding: 10px 12px;
		border: 1px dashed var(--line-strong);
		font-size: 1rem;
		letter-spacing: 0.04em;
		user-select: all;
		overflow-wrap: anywhere;
	}
</style>
