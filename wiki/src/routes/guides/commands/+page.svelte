<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const COMMANDS: [string, string][] = [
		['/time set <day|night>', 'Jumps forward to the next noon or midnight.'],
		['/give <item> <count>', 'Gives you items.'],
		['/gamemode <creative|survival>', 'Changes your own game mode.'],
		['/weather <clear|rain|thunder>', 'Sets the weather.'],
		['/difficulty <peaceful|easy|normal|hard>', 'Sets the difficulty of the world.'],
		['/locate <place>', 'Finds the nearest biome, village, dungeon, shipwreck or cave biome.'],
		['/list', 'Names the players online. Open to everyone.'],
		['/op <player>, /deop <player>', 'Makes a player an operator, or no longer one.'],
		['/kick <player> [reason]', 'Disconnects a player.'],
		['/ban <player> [reason], /pardon <player>', 'Bans a player, or lifts the ban.'],
		['/whitelist <on|off|list>, /whitelist <add|remove> <player>', 'Manages the whitelist.']
	];

	const PLACES: [string, string, string][] = [
		['village', 'A village', 'Its centre'],
		['ruin', 'A ruin', 'Its door'],
		['crypt', 'A crypt', 'Its door'],
		['sanctuary', 'A sanctuary', 'Its door'],
		['shipwreck', 'A shipwreck', 'Its chest'],
		['lush_caves', 'Lush caves', 'A spot in them'],
		['crystal_caves', 'Crystal caves', 'A spot in them'],
		['basalt_galleries', 'Basalt galleries', 'A spot in them']
	];
</script>

<Page
	title="Commands"
	description="Every chat command of Rustcraft with its syntax and examples: /give and item names, /time, /weather, /gamemode, /difficulty, /locate with biome keys, and the operator commands."
>
	{#snippet icon()}<Slot id="Compass" big link={false} />{/snippet}

	<p>
		Commands are typed in the chat: press <kbd>T</kbd>, or <kbd>/</kbd> to open the chat with the
		slash already typed. As you type, the chat suggests command names, arguments and item names: take
		a suggestion with <kbd>Tab</kbd>, move between them with <kbd>↑</kbd><kbd>↓</kbd>. A line it
		already knows to be wrong shows what the server would answer.
	</p>

	<h2>Who can use them</h2>
	<ul>
		<li><strong>In singleplayer</strong> you can run every command.</li>
		<li>
			<strong>On a server</strong>, only <code>/list</code> is open to everyone. Every other command,
			those added by mods included, is for <strong>operators</strong>; others get "You must be an
			operator to use /…". See <a href="/guides/multiplayer">multiplayer</a> to become one.
		</li>
		<li>The answer to a command goes only to whoever typed it.</li>
	</ul>

	<h2>All commands</h2>
	<div class="wide">
		<table>
			<thead><tr><th>Syntax</th><th>What it does</th></tr></thead>
			<tbody>
				{#each COMMANDS as [syntax, what] (syntax)}
					<tr><td><code>{syntax}</code></td><td>{what}</td></tr>
				{/each}
			</tbody>
		</table>
	</div>
	<p>
		Words in <code>&lt;angle brackets&gt;</code> are required; <code>[reason]</code> may be left
		out. <code>a|b</code> means one of the words.
	</p>

	<h2>/give</h2>
	<pre><code>/give oak_log 64
/give iron_ingot 10
/give diamond_sword 1</code></pre>
	<p>
		The count is required, and must be a whole number above 0. The items go to you, the player who
		typed the command. What does not fit in your inventory falls at your feet, up to another 36
		stacks; anything beyond that is lost, and the answer says how many.
	</p>
	<h3>Item names</h3>
	<p>
		An item's name is its English name in lower case, with underscores for spaces:
		<ItemLink id="OakLog" /> is <code>oak_log</code>, <ItemLink id="IronIngot" /> is
		<code>iron_ingot</code>. Case does not matter, and <code>OakLog</code> works too. The chat
		suggests names as you type them.
	</p>
	<p>A few items have a command name that differs from the name you see:</p>
	<div class="wide">
		<table>
			<thead><tr><th>Item</th><th>Command name</th></tr></thead>
			<tbody>
				{#each data.renamed as r (r.item)}
					<tr><td><ItemLink id={r.item} /></td><td><code>{r.command}</code></td></tr>
				{/each}
			</tbody>
		</table>
	</div>
	<p>Every item is on the <a href="/items">items page</a>.</p>

	<h2>/time</h2>
	<p>
		<code>/time set day</code> jumps to the next noon, <code>/time set night</code> to the next
		midnight. The clock never goes back, and on a server the time changes for everyone. See <a href="/guides/weather">day, night and weather</a>.
	</p>

	<h2>/weather</h2>
	<p>
		<code>/weather clear</code>, <code>/weather rain</code> or <code>/weather thunder</code> starts
		that weather for the length it would last on its own, then the cycle goes on as usual. It applies
		to the whole world.
	</p>

	<h2>/gamemode</h2>
	<p>
		<code>/gamemode creative</code> or <code>/gamemode survival</code> changes your own game mode
		only; there is no adventure mode. Case does not matter. Leaving creative while flying lands you.
		See <a href="/guides/game-modes">game modes</a>.
	</p>

	<h2>/difficulty</h2>
	<p>
		<code>/difficulty peaceful</code>, <code>easy</code>, <code>normal</code> or <code>hard</code>
		sets the difficulty of the world. Peaceful removes every hostile creature on the next tick. How
		difficulty changes the damage of hostiles is on the <a href="/guides/combat">combat page</a>.
	</p>

	<h2>/locate</h2>
	<p>
		<code>/locate</code> finds the nearest place of a kind to you and answers with its coordinates
		and distance, for example "The nearest village is at 412 -880, 976 blocks away". A search may take
		a moment; the answer comes when it is done.
	</p>
	<table>
		<thead><tr><th>Command</th><th>Finds</th><th>The coordinates of</th></tr></thead>
		<tbody>
			<tr>
				<td><code>/locate biome &lt;biome&gt;</code></td>
				<td>A biome, within 4096 blocks</td>
				<td>A column of it (X and Z)</td>
			</tr>
			{#each PLACES as [word, what, spot] (word)}
				<tr>
					<td><code>/locate {word}</code></td>
					<td>{what}, within {word === 'village' ? 4096 : 2048} blocks</td>
					<td>{spot}{word === 'village' ? ' (X and Z)' : ' (X, Y and Z)'}</td>
				</tr>
			{/each}
		</tbody>
	</table>
	<ul>
		<li>Lush caves, crystal caves and basalt galleries exist only in deep worlds.</li>
		<li>A world created without villages or dungeons answers that it has none.</li>
		<li>See <a href="/guides/structures">structures</a> for what villages and dungeons hold.</li>
	</ul>

	<h3>Biome keys</h3>
	<p>
		<code>/locate biome</code> takes the key of a biome, such as <code>/locate biome jungle</code>.
		A biome added by a mod is written with its namespace, such as <code>mymod:name</code>; the base
		game's can also be written <code>rustcraft:jungle</code>.
	</p>
	<div class="wide">
		<table>
			<thead><tr><th>Biome</th><th>Key</th></tr></thead>
			<tbody>
				{#each data.biomes as b (b.key)}
					<tr><td><a href={b.href}>{b.name}</a></td><td><code>{b.key}</code></td></tr>
				{/each}
			</tbody>
		</table>
	</div>

	<h2>/list</h2>
	<p>
		<code>/list</code> answers with the players online, in alphabetical order: "2 players online:
		Alex, Steve". Anyone can run it.
	</p>

	<h2>Operator commands</h2>
	<p>These manage who may play on a dedicated server.</p>
	<ul>
		<li><code>/op Alex</code>, <code>/deop Alex</code>: make Alex an operator, or no longer one.</li>
		<li><code>/kick Alex too loud</code>: disconnects Alex, who sees "You were kicked from the server: too loud".</li>
		<li>
			<code>/ban Alex griefing</code>: disconnects Alex and refuses them from then on, with the reason.
			<code>/pardon Alex</code> lifts the ban.
		</li>
		<li>
			<code>/whitelist on</code> and <code>/whitelist off</code>: when on, only the players on the
			whitelist and the operators can join. <code>/whitelist add Alex</code>,
			<code>/whitelist remove Alex</code> and <code>/whitelist list</code> manage it.
		</li>
	</ul>
	<p>
		Names are compared without their case. Except for <code>/kick</code>, the player need not be online. A name that
		holds spaces goes between quotes: <code>/ban "Bad Guy"</code>. On a hosted world, members are
		managed from the Hosted worlds tab instead, and only <code>/kick</code> works.
	</p>
</Page>
