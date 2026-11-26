<script lang="ts">
	import '@fontsource/pixelify-sans/600.css';
	import '@ui-sdk/core/web.css';
	import '../app.css';
	import { beforeNavigate, goto } from '$app/navigation';
	import { asset, resolve } from '$app/paths';
	import { page } from '$app/state';
	import { Button } from '@ui-sdk/svelte/button';
	import {
		Toast,
		ToastClose,
		ToastDescription,
		ToastRegion,
		ToastTitle,
		toast
	} from '@ui-sdk/svelte/toast';
	import { api, ApiError, messageOf } from '#lib/api.ts';
	import ConfirmHost from '#lib/components/ConfirmHost.svelte';
	import TemporaryPassword from '#lib/components/TemporaryPassword.svelte';
	import { temporary } from '#lib/temporary.svelte.ts';
	import { session } from '#lib/session.svelte.ts';
	import type { LayoutProps } from './$types';

	let { children }: LayoutProps = $props();

	const onLogin = $derived(page.route.id === '/login');
	/** Why /me failed: refused (403, the hub's message) or unreachable (anything else). */
	let failure = $state<{ refused: boolean; message: string } | null>(null);
	let attempt = $state(0);

	const nav = [
		{ href: resolve('/'), label: 'Overview', match: (id: string) => id === '/' },
		{ href: resolve('/accounts'), label: 'Accounts', match: (id: string) => id.startsWith('/accounts') },
		{ href: resolve('/worlds'), label: 'Worlds', match: (id: string) => id === '/worlds' },
		{ href: resolve('/audit'), label: 'Audit log', match: (id: string) => id === '/audit' }
	];

	// Who is logged in, once per session of the page; a 401 sends to the login page.
	$effect(() => {
		void attempt;
		if (onLogin || session.me) return;
		failure = null;
		api.me().then(
			(me) => (session.me = me),
			(error) => {
				if (error instanceof ApiError && error.status === 403)
					failure = { refused: true, message: error.message };
				else if (!(error instanceof ApiError && error.status === 401))
					failure = { refused: false, message: messageOf(error) };
			}
		);
	});

	// Leaving while the hub resets a password would lose the only copy of the new one.
	// The login page always passes: it is where a lost session leads.
	beforeNavigate((navigation) => {
		if (temporary.resetting && navigation.to?.route.id !== '/login') navigation.cancel();
	});

	/** Leaves only once the hub has ended the session: a failed logout must not look done. */
	async function logout() {
		try {
			await api.logout();
		} catch (error) {
			// A 401 means there is no session left: as good as logged out.
			if (!(error instanceof ApiError && error.status === 401)) {
				toast.error('Not logged out', { description: messageOf(error) });
				return;
			}
		}
		session.me = null;
		failure = null;
		temporary.value = null;
		temporary.resetting = false;
		await goto(resolve('/login'), { replace: true });
	}
</script>

<svelte:head>
	<title>Rustcraft hub admin</title>
</svelte:head>

{#if onLogin}
	{@render children()}
{:else}
	<a class="skip" href="#main">Skip to content</a>
	<header class="bar">
		<div class="inner">
			<a class="brand" href={resolve('/')}>
				<img class="mark" src={asset('favicon.svg')} alt="" width="18" height="18" />
				<span class="name">Rustcraft hub</span>
				<span class="tag">admin</span>
			</a>
			<nav aria-label="Main">
				{#each nav as item (item.href)}
					<a
						href={item.href}
						aria-current={page.route.id && item.match(page.route.id) ? 'page' : undefined}
						>{item.label}</a
					>
				{/each}
			</nav>
			<div class="who">
				{#if session.me}
					<span class="me" title="Logged in as {session.me.username}"
						><span class="visually-hidden">Logged in as </span>{session.me.username}</span
					>
				{/if}
				<Button onclick={logout}>Log out</Button>
			</div>
		</div>
	</header>

	<main id="main" tabindex="-1">
		{#if failure}
			<div class="page">
				<div class="card refused" role="alert">
					{#if failure.refused}
						<h1>No access</h1>
						<p>{failure.message}</p>
						<div><Button data-variant="solid" onclick={logout}>Log in again</Button></div>
					{:else}
						<h1>The hub did not answer</h1>
						<p>{failure.message}</p>
						<div><Button data-variant="solid" onclick={() => attempt++}>Retry</Button></div>
					{/if}
				</div>
			</div>
		{:else if session.me}
			{@render children()}
		{:else}
			<div class="page"><p class="muted" aria-live="polite">Loading…</p></div>
		{/if}
	</main>
{/if}

<ConfirmHost />
{#if !onLogin}<TemporaryPassword />{/if}

<ToastRegion side="bottom" align="end">
	{#snippet children()}
		<Toast>
			<div class="stack-tight toast-text">
				<ToastTitle />
				<ToastDescription />
			</div>
			<ToastClose aria-label="Dismiss" />
		</Toast>
	{/snippet}
</ToastRegion>

<style>
	.skip {
		position: absolute;
		left: 8px;
		top: -40px;
		z-index: 100;
		padding: 6px 10px;
		border-radius: var(--radius);
		background: var(--accent);
		color: var(--accent-fg);
	}
	.skip:focus {
		top: 8px;
	}

	.bar {
		position: sticky;
		top: 0;
		z-index: 20;
		background: color-mix(in oklab, var(--surface) 92%, transparent);
		backdrop-filter: blur(8px);
		border-bottom: 1px solid var(--line);
	}
	.inner {
		max-width: 1200px;
		margin: 0 auto;
		padding: 0 max(16px, env(safe-area-inset-left));
		display: flex;
		align-items: center;
		gap: 8px 24px;
		min-height: 56px;
	}
	.brand {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		color: var(--fg);
		text-decoration: none;
		white-space: nowrap;
	}
	.mark {
		width: 18px;
		height: 18px;
	}
	.name {
		font-family: var(--display);
		font-weight: 600;
		font-size: 1.125rem;
		letter-spacing: 0.01em;
	}
	.tag {
		padding: 1px 6px;
		border-radius: 4px;
		background: var(--surface-2);
		color: var(--muted);
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.06em;
	}
	nav {
		display: flex;
		gap: 2px;
		flex: 1;
		min-width: 0;
	}
	nav a {
		padding: 6px 10px;
		border-radius: 5px;
		color: var(--muted);
		font-weight: 500;
		white-space: nowrap;
		text-decoration: none;
	}
	nav a:hover {
		color: var(--fg);
		background: var(--surface-2);
	}
	nav a[aria-current='page'] {
		color: var(--fg);
		background: var(--surface-2);
		box-shadow: inset 0 -2px 0 var(--accent);
	}
	.who {
		display: flex;
		align-items: center;
		gap: 10px;
		margin-inline-start: auto;
	}
	.me {
		color: var(--muted);
		font-weight: 500;
		max-width: 16ch;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	main {
		outline: none;
	}
	.refused {
		max-width: 480px;
	}
	.toast-text {
		flex: 1;
	}

	@media (max-width: 760px) {
		.inner {
			flex-wrap: wrap;
			padding-top: 8px;
		}
		nav {
			order: 3;
			flex-basis: 100%;
			overflow-x: auto;
			margin: 0 -6px;
			padding-bottom: 6px;
		}
		.me {
			max-width: 10ch;
		}
	}
	@media (max-width: 430px) {
		.tag {
			display: none;
		}
	}
</style>
