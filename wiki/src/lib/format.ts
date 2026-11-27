// How numbers read on the wiki.

/// `0.25` reads `25 %`, `0.0333` reads `3.3 %`.
export function percent(share: number): string {
	const p = share * 100;
	if (p >= 10 || Number.isInteger(p)) return `${Math.round(p)} %`;
	return `${p.toFixed(p < 1 ? 2 : 1).replace(/\.?0+$/, '')} %`;
}

/// Seconds as a player reads them: `0.5 s`, `15 s`, `3 min`, `1 min 30 s`.
export function duration(seconds: number): string {
	if (seconds < 60) return `${+seconds.toFixed(2)} s`;
	const min = Math.floor(seconds / 60);
	const s = Math.round(seconds - min * 60);
	return s ? `${min} min ${s} s` : `${min} min`;
}

/// `1..1` reads `1`, `0..2` reads `0–2`.
export const range = (min: number, max: number) => (min === max ? `${min}` : `${min}–${max}`);

/// A float of the game, cut of its noise: `0.0599999` reads `0.06`.
export const num = (x: number, digits = 2) => +x.toFixed(digits);

export const TIER_LEVEL = ['Wood or gold', 'Stone or bronze', 'Iron', 'Diamond'];
