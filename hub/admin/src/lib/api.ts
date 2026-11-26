// The hub's admin API, under /v1/admin on the same origin. The session is an HttpOnly cookie
// the page never sees: every request goes with credentials 'same-origin' and the header the
// hub asks for, and nothing is ever stored by the page.

import { goto } from '$app/navigation';
import { resolve } from '$app/paths';
import { page } from '$app/state';
import { ask } from './confirm.ts';
import { temporary } from './temporary.svelte.ts';

export type Status = 'active' | 'suspended' | 'deleted';
export type Role = 'admin' | 'host';
export type RealmState = 'stopped' | 'starting' | 'running' | 'stopping' | 'failed' | 'archived';
export type SessionKind = 'game' | 'admin_web';

export interface Me {
	id: string;
	username: string;
}

export interface Overview {
	accounts: Record<Status, number>;
	admins: number;
	hosts: number;
	sessions: Record<SessionKind, number>;
	signups: { last_24h: number; last_7d: number };
	failed_logins_24h: number;
	realms: Record<RealmState, number>;
	running: number;
	max_running: number;
}

export interface AccountItem {
	id: string;
	username: string;
	status: Status;
	status_reason: string | null;
	roles: Role[];
	realm_quota: number;
	created_at: number;
}

export interface SessionItem {
	id: string;
	kind: SessionKind;
	device_name: string | null;
	created_at: number;
	last_used_at: number;
	expires_at: number;
}

export interface RealmItem {
	id: string;
	name: string;
	owner: { id: string; username: string };
	state: RealmState;
	slot: number | null;
	port: number | null;
	players: number;
	last_played_at: number | null;
	failed_reason: string | null;
	created_at: number;
	state_changed_at: number;
}

export interface AuditItem {
	id: number;
	at: number;
	actor: string;
	action: string;
	account_id: string | null;
	username: string | null;
	details: Record<string, unknown>;
}

export interface AccountDetail extends AccountItem {
	deleted_at: number | null;
	sessions: SessionItem[];
	active_certificates: number;
	realms: RealmItem[];
	audit: AuditItem[];
}

/** An error the hub answered with, or a failed request. */
export class ApiError extends Error {
	constructor(
		readonly status: number,
		readonly code: string,
		message: string,
		/** Seconds to wait, from Retry-After on a 429. */
		readonly retryAfter: number | null = null
	) {
		super(message);
	}
}

const REQUEST_TIMEOUT_MS = 30_000;

/** A request that got no answer: timed out, or the hub could not be reached. */
function failed(error: unknown): ApiError {
	if (error instanceof DOMException && (error.name === 'TimeoutError' || error.name === 'AbortError'))
		return new ApiError(0, 'timeout', 'The hub did not answer within 30 seconds. Try again.');
	return new ApiError(0, 'network', 'The hub cannot be reached. Check your connection.');
}

type Query = Record<string, string | number | null | undefined>;

async function request<T>(method: string, path: string, body?: unknown, query?: Query): Promise<T> {
	let url = `/v1/admin${path}`;
	if (query) {
		const params = new URLSearchParams();
		for (const [key, value] of Object.entries(query)) {
			if (value !== null && value !== undefined && value !== '') params.set(key, String(value));
		}
		const search = params.toString();
		if (search) url += `?${search}`;
	}
	const headers: Record<string, string> = { 'X-Rustcraft-Admin': '1', Accept: 'application/json' };
	if (body !== undefined) headers['Content-Type'] = 'application/json';

	// Every request ends, answered or not: a pending action (a reset holds navigation) must not
	// wait forever on a hub that hangs.
	const signal = AbortSignal.timeout(REQUEST_TIMEOUT_MS);
	let response: Response;
	try {
		response = await fetch(url, {
			method,
			credentials: 'same-origin',
			cache: 'no-store',
			headers,
			body: body === undefined ? undefined : JSON.stringify(body),
			signal
		});
	} catch (error) {
		throw failed(error);
	}

	if (response.ok) {
		if (response.status === 204 || response.status === 202) return undefined as T;
		let text: string;
		try {
			text = await response.text();
		} catch (error) {
			throw failed(error);
		}
		return (text ? JSON.parse(text) : undefined) as T;
	}

	let code = 'internal';
	let message = `The hub answered ${response.status}.`;
	try {
		const error = (await response.json()) as { error?: string; message?: string };
		if (error.error) code = error.error;
		if (error.message) message = error.message;
	} catch {
		// Not the hub's JSON: keep the status.
	}
	const retry = Number(response.headers.get('Retry-After'));
	const failure = new ApiError(
		response.status,
		code,
		message,
		Number.isFinite(retry) && retry > 0 ? retry : null
	);
	if (response.status === 401 && code === 'unauthorized') toLogin();
	throw failure;
}

/** Back to the login page, which comes back here after. The session is gone, so nothing of it
 * stays: no pending reset holds the way, no question stays open, no temporary password is left. */
export function toLogin() {
	temporary.resetting = false;
	temporary.value = null;
	ask.controller.close();
	const login = resolve('/login');
	if (page.url.pathname === login) return;
	const back = page.url.pathname + page.url.search;
	goto(`${login}?next=${encodeURIComponent(back)}`, { replace: true });
}

export const api = {
	login: (username: string, password: string) =>
		request<{ username: string }>('POST', '/session', { username, password }),
	logout: () => request<void>('DELETE', '/session'),
	me: () => request<Me>('GET', '/me'),
	overview: () => request<Overview>('GET', '/overview'),

	accounts: (query: { q?: string; status?: string; role?: string; offset?: number; limit?: number }) =>
		request<{ total: number; accounts: AccountItem[] }>('GET', '/accounts', undefined, query),
	account: (id: string) => request<AccountDetail>('GET', `/accounts/${encodeURIComponent(id)}`),
	grant: (id: string, role: Role, realmQuota?: number) =>
		request<void>(
			'PUT',
			`/accounts/${encodeURIComponent(id)}/roles/${role}`,
			realmQuota === undefined ? {} : { realm_quota: realmQuota }
		),
	revoke: (id: string, role: Role) =>
		request<void>('DELETE', `/accounts/${encodeURIComponent(id)}/roles/${role}`),
	setQuota: (id: string, realmQuota: number) =>
		request<void>('PUT', `/accounts/${encodeURIComponent(id)}/quota`, { realm_quota: realmQuota }),
	suspend: (id: string, reason: string) =>
		request<void>('POST', `/accounts/${encodeURIComponent(id)}/suspend`, { reason }),
	unsuspend: (id: string) => request<void>('POST', `/accounts/${encodeURIComponent(id)}/unsuspend`),
	resetPassword: (id: string) =>
		request<{ password: string }>('POST', `/accounts/${encodeURIComponent(id)}/reset-password`),
	endSessions: (id: string) =>
		request<void>('DELETE', `/accounts/${encodeURIComponent(id)}/sessions`),

	realms: (state?: string) => request<{ realms: RealmItem[] }>('GET', '/realms', undefined, { state }),
	stopRealm: (id: string) => request<void>('POST', `/realms/${encodeURIComponent(id)}/stop`),
	archiveRealm: (id: string) => request<void>('DELETE', `/realms/${encodeURIComponent(id)}`),

	audit: (query: { account?: string; action?: string; before?: number; limit?: number }) =>
		request<{ entries: AuditItem[]; next_before: number | null }>('GET', '/audit', undefined, query)
};

/** The message to show for a failed call. */
export function messageOf(error: unknown): string {
	if (error instanceof ApiError) return error.message;
	if (error instanceof Error) return error.message;
	return 'Something went wrong.';
}
