// The markup of the texts of the Codex (`shared/src/codex/markup.rs`), as the export writes it:
// blocks of lines, lines of runs. Text written once for the game and for the wiki.

export type Target =
	| { kind: 'item' | 'mob' | 'effect'; id: string }
	| { kind: 'biome' | 'guide' | 'page'; id: string };

export type Span = { text?: string; em?: boolean; strong?: boolean; link?: Target };
export type Line = Span[];

export type Block =
	| { type: 'heading'; level: number; text: Line }
	| { type: 'paragraph'; text: Line }
	| { type: 'list'; ordered: boolean; items: Line[] }
	| { type: 'table'; head: Line[] | null; numeric: boolean[]; rows: Line[][] }
	| { type: 'recipe'; item: string }
	| { type: 'computed'; name: string };

const spaced = (s: string) => s.replace(/[_-]/g, ' ');

/// Where a link that is not to an item or a creature goes, and the words it shows without its own.
export function hrefOf(t: Target): { href: string; name: string } {
	switch (t.kind) {
		case 'biome':
			return { href: `/biomes/${t.id.replace(/_/g, '-')}`, name: spaced(t.id) };
		case 'guide':
			return { href: `/guides/${t.id}`, name: spaced(t.id) };
		case 'page':
			return { href: `/${t.id}`, name: spaced(t.id) };
		case 'effect':
			return { href: `/effects#${t.id}`, name: t.id.replace(/([a-z])([A-Z])/g, '$1 $2') };
		default:
			return { href: '#', name: t.id };
	}
}

/// The words of a line without their marks.
export const plain = (line: Line | null | undefined) => (line ?? []).map((s) => s.text ?? s.link?.id ?? '').join('');
