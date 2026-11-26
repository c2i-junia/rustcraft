// Times from the hub are unix seconds.

const absolute = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' });
const relative = new Intl.RelativeTimeFormat('en', { numeric: 'auto', style: 'short' });

const units: [Intl.RelativeTimeFormatUnit, number][] = [
	['year', 365 * 24 * 3600],
	['month', 30 * 24 * 3600],
	['week', 7 * 24 * 3600],
	['day', 24 * 3600],
	['hour', 3600],
	['minute', 60]
];

export function fullTime(seconds: number): string {
	return absolute.format(new Date(seconds * 1000));
}

export function isoTime(seconds: number): string {
	return new Date(seconds * 1000).toISOString();
}

/** "3 hr. ago", "in 5 min.", "now". */
export function sinceNow(seconds: number, now = Date.now() / 1000): string {
	const delta = seconds - now;
	for (const [unit, size] of units) {
		if (Math.abs(delta) >= size) return relative.format(Math.round(delta / size), unit);
	}
	return 'just now';
}

/** "in 12 minutes", for a wait in seconds. */
export function waitFor(seconds: number): string {
	if (seconds < 60) return `${seconds} second${seconds === 1 ? '' : 's'}`;
	const minutes = Math.ceil(seconds / 60);
	return `${minutes} minute${minutes === 1 ? '' : 's'}`;
}

/** The details of an audit row as short "key: value" pairs, without the admin's id. */
export function detailPairs(details: Record<string, unknown>): [string, string][] {
	return Object.entries(details)
		.filter(([key]) => key !== 'admin_id')
		.map(([key, value]) => [
			key.replaceAll('_', ' '),
			typeof value === 'string' ? value : JSON.stringify(value)
		]);
}

export function plural(count: number, one: string, many = `${one}s`): string {
	return `${count} ${count === 1 ? one : many}`;
}
