<script lang="ts">
	import { resolve } from '$app/paths';
	import type { AuditItem } from '#lib/api.ts';
	import { detailPairs } from '#lib/format.ts';
	import Time from './Time.svelte';

	let {
		entries,
		showAccount = true,
		label = 'Audit log'
	}: { entries: AuditItem[]; showAccount?: boolean; label?: string } = $props();

	const destructive = new Set([
		'suspended',
		'password_reset',
		'sessions_ended',
		'role_revoked',
		'realm_archived',
		'realm_stopped',
		'account_deleted',
		'login_failed',
		'admin_login_refused'
	]);
</script>

{#if entries.length === 0}
	<p class="empty">Nothing recorded yet.</p>
{:else}
	<div class="table-wrap">
		<table aria-label={label}>
			<thead>
				<tr>
					<th scope="col">When</th>
					<th scope="col">Action</th>
					{#if showAccount}<th scope="col">Account</th>{/if}
					<th scope="col">By</th>
					<th scope="col">Details</th>
				</tr>
			</thead>
			<tbody>
				{#each entries as entry (entry.id)}
					<tr>
						<td class="nowrap"><Time at={entry.at} /></td>
						<td>
							<code class="action" data-tone={destructive.has(entry.action) ? 'danger' : undefined}
								>{entry.action}</code
							>
						</td>
						{#if showAccount}
							<td>
								{#if entry.account_id && entry.username}
									<a href={resolve('/accounts/[id]', { id: entry.account_id })}>{entry.username}</a>
								{:else}
									<span class="faint">—</span>
								{/if}
							</td>
						{/if}
						<td class="mono-small">{entry.actor}</td>
						<td class="details">
							{#each detailPairs(entry.details) as [key, value] (key)}
								<span class="pair"><span class="faint">{key}</span> {value}</span>
							{/each}
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
{/if}
