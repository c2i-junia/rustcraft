import { goto } from '$app/navigation';
import { page } from '$app/state';

/** Changes the query of the current page in place, so that filters survive a reload and Back. */
export function setQuery(changes: Record<string, string | number | null>) {
	const url = new URL(page.url.href);
	for (const [key, value] of Object.entries(changes)) {
		if (value === null || value === '') url.searchParams.delete(key);
		else url.searchParams.set(key, String(value));
	}
	return goto(url.pathname + url.search, { replace: true, reset: false });
}
