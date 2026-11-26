<script lang="ts">
	import { goto } from '$app/navigation';
	import { asset, resolve } from '$app/paths';
	import { page } from '$app/state';
	import { Button } from '@ui-sdk/svelte/button';
	import { Field, FieldLabel } from '@ui-sdk/svelte/field';
	import { Input } from '@ui-sdk/svelte/input';
	import {
		PasswordInput,
		PasswordInputControl,
		PasswordInputToggle
	} from '@ui-sdk/svelte/password-input';
	import { api, ApiError, messageOf } from '#lib/api.ts';
	import { waitFor } from '#lib/format.ts';
	import { session } from '#lib/session.svelte.ts';

	let username = $state('');
	let password = $state('');
	let error = $state<string | null>(null);
	let busy = $state(false);

	/** Only a path of this app, so that the page never sends anyone elsewhere. */
	function next(): string {
		const target = page.url.searchParams.get('next');
		const home = resolve('/');
		if (target && target.startsWith(home) && !target.startsWith('//')) return target;
		return home;
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (busy) return;
		error = null;
		busy = true;
		try {
			await api.login(username.trim(), password);
			password = '';
			session.me = await api.me();
			await goto(next(), { replace: true });
		} catch (failure) {
			password = '';
			if (failure instanceof ApiError) {
				if (failure.code === 'bad_credentials') error = 'Wrong username or password.';
				else if (failure.code === 'rate_limited')
					error = failure.retryAfter
						? `Too many failed attempts. Try again in ${waitFor(failure.retryAfter)}.`
						: failure.message;
				else error = failure.message;
			} else {
				error = messageOf(failure);
			}
		} finally {
			busy = false;
		}
	}
</script>

<svelte:head>
	<title>Log in · Rustcraft hub admin</title>
</svelte:head>

<main class="wrap">
	<form class="card login" onsubmit={submit}>
		<div class="head">
			<img src={asset('favicon.svg')} alt="" width="28" height="28" />
			<div>
				<h1>Rustcraft hub</h1>
				<p class="muted">Administration</p>
			</div>
		</div>

		<Field name="username">
			<FieldLabel>Username</FieldLabel>
			<Input bind:value={username} autocomplete="username" autocapitalize="none" spellcheck={false} required />
		</Field>

		<Field name="password">
			<FieldLabel>Password</FieldLabel>
			<PasswordInput bind:value={password} required>
				<PasswordInputControl />
				<PasswordInputToggle aria-label="Show password" />
			</PasswordInput>
		</Field>

		{#if error}
			<p class="form-error" role="alert">{error}</p>
		{/if}

		<Button type="submit" data-variant="solid" disabled={busy} focusableWhenDisabled>
			{busy ? 'Logging in…' : 'Log in'}
		</Button>

		<p class="note faint">Only accounts with the admin role can log in here.</p>
	</form>
</main>

<style>
	.wrap {
		min-height: 100dvh;
		display: grid;
		place-items: center;
		padding: 24px 16px;
	}
	.login {
		width: min(100%, 360px);
		padding: 28px 24px 24px;
		gap: 16px;
		box-shadow: 0 1px 2px rgb(0 0 0 / 0.04), 0 12px 32px -16px rgb(0 0 0 / 0.2);
	}
	.head {
		display: flex;
		align-items: center;
		gap: 12px;
		margin-bottom: 4px;
	}
	h1 {
		font-family: var(--display);
		font-weight: 600;
		font-size: 1.375rem;
	}
	.note {
		font-size: 0.75rem;
		text-align: center;
	}
	.login :global([data-slot='button']) {
		height: 36px;
	}
</style>
