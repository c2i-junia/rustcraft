<!-- The search box: it fetches the index on first use, then matches as you type. "/" focuses
     it, the arrows choose, Enter opens. -->
<script lang="ts">
	import { goto } from '$app/navigation';

	type Entry = { t: string; f?: string; h: string; k: string; i?: string };
	let index = $state<Entry[] | null>(null);
	let query = $state('');
	let results = $state<Entry[]>([]);
	let active = $state(0);
	let open = $state(false);
	let input: HTMLInputElement;

	async function load() {
		index ??= await (await fetch('/search.json')).json();
		run();
	}

	const fold = (s: string) =>
		s
			.normalize('NFD')
			.replace(/[̀-ͯ]/g, '')
			.toLowerCase();

	function score(e: Entry, q: string): number {
		let best = 0;
		for (const name of [e.t, e.f]) {
			if (!name) continue;
			const n = fold(name);
			if (n === q) best = Math.max(best, 100);
			else if (n.startsWith(q)) best = Math.max(best, 80 - n.length / 10);
			else if (n.split(/\s+/).some((w) => w.startsWith(q))) best = Math.max(best, 60 - n.length / 10);
			else if (n.includes(q)) best = Math.max(best, 40 - n.length / 10);
		}
		// The guides first among equals: they are what a question usually wants
		return best && (e.k === 'Guide' ? best + 5 : best);
	}

	function run() {
		const q = fold(query.trim());
		if (!index || !q) {
			results = [];
			return;
		}
		results = index
			.map((e) => [score(e, q), e] as const)
			.filter(([s]) => s > 0)
			.sort((a, b) => b[0] - a[0])
			.slice(0, 10)
			.map(([, e]) => e);
		active = 0;
	}

	function choose(e: Entry | undefined) {
		if (!e) return;
		query = '';
		results = [];
		open = false;
		input.blur();
		goto(e.h);
	}

	function keydown(ev: KeyboardEvent) {
		if (ev.key === 'ArrowDown') {
			active = Math.min(active + 1, results.length - 1);
			ev.preventDefault();
		} else if (ev.key === 'ArrowUp') {
			active = Math.max(active - 1, 0);
			ev.preventDefault();
		} else if (ev.key === 'Enter') {
			choose(results[active]);
		} else if (ev.key === 'Escape') {
			query = '';
			results = [];
			input.blur();
		}
	}

	function global(ev: KeyboardEvent) {
		const target = ev.target as HTMLElement;
		if (ev.key === '/' && !['INPUT', 'TEXTAREA'].includes(target.tagName)) {
			ev.preventDefault();
			input.focus();
		}
	}
</script>

<svelte:window onkeydown={global} />

<div class="box" role="combobox" aria-expanded={open && results.length > 0} aria-haspopup="listbox" aria-controls="search-results" aria-owns="search-results">
	<input
		bind:this={input}
		bind:value={query}
		oninput={run}
		onfocus={() => {
			open = true;
			load();
		}}
		onblur={() => setTimeout(() => (open = false), 150)}
		onkeydown={keydown}
		type="search"
		placeholder="Search items, creatures, guides  /"
		aria-label="Search the wiki"
		aria-autocomplete="list"
		aria-activedescendant={results.length ? `result-${active}` : undefined}
		autocomplete="off"
		spellcheck="false"
	/>
	{#if open && query.trim()}
		<ul id="search-results" role="listbox">
			{#each results as e, i (e.h)}
				<li id="result-{i}" role="option" aria-selected={i === active}>
					<a
						href={e.h}
						onmousedown={(ev) => {
							ev.preventDefault();
							choose(e);
						}}
						onmouseenter={() => (active = i)}
					>
						{#if e.i}<span class="ic" style:background-image="url({e.i})"></span>{:else}<span class="ic"></span>{/if}
						<span class="t">{e.t}</span>
						<span class="k">{e.k}</span>
					</a>
				</li>
			{:else}
				<li class="none">{index ? `Nothing is called "${query.trim()}".` : 'Loading…'}</li>
			{/each}
		</ul>
	{/if}
</div>

<style>
	.box {
		position: relative;
	}
	input {
		width: 100%;
		box-sizing: border-box;
		font: inherit;
		font-size: 0.98rem;
		color: var(--fg);
		background: var(--slot-deep);
		border: 2px solid var(--line);
		padding: 7px 12px;
	}
	input::placeholder {
		color: var(--muted);
	}
	input:focus {
		outline: none;
		border-color: var(--rust);
	}
	ul {
		position: absolute;
		left: 0;
		right: 0;
		top: calc(100% + 4px);
		margin: 0;
		padding: 4px;
		list-style: none;
		background: var(--surface);
		border: 2px solid var(--line);
		box-shadow: 6px 6px 0 rgb(0 0 0 / 0.45);
		z-index: 20;
	}
	li a {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 5px 8px;
		color: var(--fg);
		text-decoration: none;
	}
	li[aria-selected='true'] a {
		background: var(--slot-hover);
		color: var(--rust);
	}
	.ic {
		width: 24px;
		height: 24px;
		flex: none;
		background-size: contain;
		background-position: center;
		background-repeat: no-repeat;
		image-rendering: pixelated;
	}
	.t {
		flex: 1;
	}
	.k {
		font-size: 0.8rem;
		color: var(--muted);
	}
	.none {
		padding: 8px;
		color: var(--muted);
	}
</style>
