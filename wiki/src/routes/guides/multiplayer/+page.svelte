<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';

	const ERRORS: [string, string][] = [
		[
			'Cannot reach "…"',
			'The server did not answer within 10 seconds. Check that it is running and that both its UDP ports are open.'
		],
		['"…" runs another version', 'Join with the same version of Rustcraft as the server.'],
		['"…" is full', 'The server has no room for another player. Try again later.'],
		[
			'"…" refused you',
			'Most often, a player with your name is already on the server. Change your name and try again.'
		],
		[
			'Your name is not accepted',
			'A name has 1 to 32 characters and no control characters.'
		],
		['Connection to "…" lost, "…" stopped', 'The server stopped answering, or shut down.'],
		[
			'Disconnected from "…"',
			'The server dismissed you, and says why: you were kicked or banned, or you are not on its whitelist.'
		]
	];

	const OPTIONS: [string, string, string][] = [
		['--port <port>', '8000', 'The UDP port of the game. The server list asks for the status on port + 1.'],
		['--world <name>', 'default', 'The world to load from saves/<name>, created if it does not exist.'],
		['--seed <number>', 'random', 'The seed of a new world.'],
		[
			'--world-type <type>',
			'default',
			'For a new world: default, large-biomes, archipelago or single-biome.'
		],
		['--biome <key>', '', 'The biome of a single-biome world, such as plains.'],
		['--game-mode <mode>', 'survival', 'The game mode of the players of a new world: survival or creative.'],
		['--difficulty <level>', 'normal', 'For a new world: peaceful, easy, normal or hard.'],
		['--max-players <count>', '64', 'The most players at once, from 1 to 64.'],
		[
			'--max-render-distance <chunks>',
			'64',
			'The largest render distance a player may ask for. High values cost the server memory.'
		],
		['--op <name>', '', 'Makes this player an operator before the server starts. May be repeated.'],
		[
			'--auth <mode>',
			'offline',
			'offline takes anyone under the name they give; hub takes only players logged in to a hub account.'
		],
		[
			'--public-address <host[:port]>',
			'',
			'The address players type to join. Needed with --auth hub.'
		],
		['--game-folder-path <path>', 'see below', 'Where the server keeps its saves and lists.']
	];
</script>

<Page
	title="Multiplayer"
	description="How to play Rustcraft with others: joining a server from the server list, hosted worlds, running your own dedicated server or Docker container, operators, bans and the whitelist."
>
	{#snippet icon()}<Slot id="Compass" big link={false} />{/snippet}

	<p>
		Rustcraft is played with others on a <strong>dedicated server</strong> that someone runs, or on a
		<strong>hosted world</strong> that the hub runs for you. Both are reached from
		<strong>Multiplayer</strong> on the main menu, which has two tabs: <em>Servers</em> and
		<em>Hosted worlds</em>.
	</p>

	<h2>Your name and skin</h2>
	<p>
		The game asks for your name on the first launch. The button under the version on the main menu
		changes it. A name has 1 to 32 characters. Other players see it, and every world keeps your
		inventory and position under it: a new name starts afresh.
	</p>
	<p>
		The <strong>Skin</strong> button of the main menu chooses one of the 12 skins that come with the
		game. To wear your own, drop a 64×64 PNG on the window while the Skin screen is open. The other
		players see your skin, an imported one included.
	</p>

	<h2>Joining a server</h2>
	<ol>
		<li>Open <strong>Multiplayer</strong>, then the <em>Servers</em> tab.</li>
		<li>
			Press <strong>Add server</strong> (<kbd>Ctrl</kbd>+<kbd>N</kbd>), give it a name and its address:
			a host name or an IPv4 address, with or without a port, such as <code>play.example.com</code>
			or <code>10.0.0.2:8000</code>. Without a port, 8000 is used.
		</li>
		<li>Select the server and press <strong>Join</strong> (<kbd>Enter</kbd>).</li>
	</ol>
	<p>
		Each server shows its status: <em>Online</em> with its players (such as 3 / 20) and its latency,
		<em>Offline</em>, <em>Unknown host</em>, <em>Bad address</em> or <em>Incompatible</em> (another
		version of the game). A server that shows Offline may still be joinable: it may only be hiding its
		status port. Use <kbd>↑</kbd><kbd>↓</kbd> to pick a server, <kbd>Del</kbd> to remove it,
		<kbd>F5</kbd> to refresh the list and <kbd>Esc</kbd> to go back.
	</p>
	<p>Servers are reached over IPv4 only.</p>

	<h3>When joining fails</h3>
	<table>
		<thead><tr><th>Message</th><th>What to do</th></tr></thead>
		<tbody>
			{#each ERRORS as [message, fix] (message)}
				<tr><td>{message}</td><td>{fix}</td></tr>
			{/each}
		</tbody>
	</table>

	<h2>Accounts</h2>
	<p>
		You do not need an account to play alone or on a server that takes anyone. An account on the
		hub reserves your name on the servers that ask for accounts, and is needed for hosted worlds.
		Open it from the name button of the main menu:
	</p>
	<ul>
		<li>
			<strong>Create account</strong>: a username of 3 to 20 characters and a password of at least 10.
			The hub then shows a <strong>recovery code, once</strong>. Write it down: it is the only way back
			into the account if you forget the password.
		</li>
		<li><strong>Log in</strong>: a session lasts 90 days after its last use.</li>
		<li><strong>Recover account</strong>: your username, the recovery code and a new password.</li>
		<li><strong>Play offline as...</strong> changes the name you play under without an account.</li>
	</ul>
	<p>
		On a server that takes anyone, you are a different player logged in and logged out, each with
		its own inventory and position.
	</p>

	<h2>Hosted worlds</h2>
	<p>
		The <em>Hosted worlds</em> tab, once you are logged in, lists the worlds of the hub you own or
		are a member of. You need neither a server nor an open port.
	</p>
	<ul>
		<li>
			<strong>Join</strong> starts a stopped world, waits up to a minute for it, and joins. Any member
			can start a world.
		</li>
		<li>
			Only the owner, the operators and the members can join. The owner adds players by username
			(<strong>Members</strong>), makes them operators or members again, and removes them.
		</li>
		<li>
			<strong>Create a world</strong> (name, optional seed, game mode, difficulty, world type) is only for
			accounts given the host role by whoever runs the hub.
		</li>
		<li>
			<strong>Stop</strong> saves and stops the world. <strong>Delete</strong> takes it off your list;
			the hub keeps its files for 30 days.
		</li>
		<li>
			A world nobody plays for 10 minutes saves and stops by itself. The official hub runs 3 worlds at
			a time: when they are all taken, try again later.
		</li>
	</ul>

	<h2>Running your own server</h2>
	<p>
		The release archive for Windows and Linux holds the game and the dedicated server side by side:
		<code>bin/rustcraft-server</code> (<code>bin\rustcraft-server.exe</code> on Windows). Keep the
		<code>data</code> folder next to <code>bin</code>. Start it from a terminal:
	</p>
	<pre><code>rustcraft-server --world survival --op YourName</code></pre>
	<p>
		<code>--op</code> makes you the first operator. Then open <strong>two UDP ports</strong> in the
		firewall and forward them on the router: the game port (8000 by default) and the next one (8001),
		which answers the server list. Players join at your address and game port.
	</p>
	<p>
		Stop the server with <kbd>Ctrl</kbd>+<kbd>C</kbd>: it saves the world and the players, then
		exits. The world is also saved every 60 seconds and when a player leaves.
	</p>

	<h3>Server options</h3>
	<p>
		<code>rustcraft-server --help</code> lists them all. The seed, world type, game mode and
		difficulty only apply when the world is created: an existing world keeps its own.
	</p>
	<div class="wide">
		<table>
			<thead><tr><th>Option</th><th>Default</th><th>What it does</th></tr></thead>
			<tbody>
				{#each OPTIONS as [option, def, what] (option)}
					<tr><td><code>{option}</code></td><td>{def}</td><td>{what}</td></tr>
				{/each}
			</tbody>
		</table>
	</div>
	<p>
		The game folder holds the saves and the lists. By default it is <code>~/.local/share/rustcraft</code>
		on Linux, <code>%APPDATA%\rustcraft</code> on Windows and
		<code>~/Library/Application Support/rustcraft</code> on macOS. Each world is a folder under
		<code>saves/</code>.
	</p>

	<h3>With Docker</h3>
	<p>Each release comes with an image of the server. The world goes to the volume:</p>
	<pre><code
			>docker run -d --name rustcraft --read-only \
    -v rustcraft-data:/data -p 8000:8000/udp -p 8001:8001/udp \
    ghcr.io/c2i-junia/rustcraft/rustcraft-server</code
		></pre>
	<p>
		Arguments after the image name replace the default <code>--world world --port 8000</code>, for
		example <code>--world survival --seed 42 --max-players 8 --op Alice</code>. The logs go to
		<code>docker logs</code>, and <code>docker stop</code> saves the world before exiting.
	</p>

	<h2>Operators and permissions</h2>
	<p>
		On a dedicated server, every command but <code>/list</code> is for operators, the commands of mods
		included. Anyone else is answered "You must be an operator to use /…". In singleplayer you are
		always an operator. The commands are on the <a href="/guides/commands">commands page</a>.
	</p>
	<ul>
		<li>
			<code>/op &lt;player&gt;</code> and <code>/deop &lt;player&gt;</code> make a player an operator,
			or no longer one.
		</li>
		<li>
			<code>/kick &lt;player&gt; [reason]</code> disconnects a player, who sees the reason.
			<code>/ban &lt;player&gt; [reason]</code> also refuses them until <code>/pardon &lt;player&gt;</code>.
		</li>
		<li>
			<code>/whitelist on</code> lets only the players on the whitelist, and the operators, join.
			<code>/whitelist add</code>, <code>remove</code> and <code>list</code> manage it;
			<code>/whitelist off</code> lets anyone in again.
		</li>
	</ul>
	<p>
		Names are compared without their case. A name with spaces goes between quotes:
		<code>/kick "Bad Guy"</code>. The server keeps the lists in its game folder as
		<code>ops.ron</code>, <code>banned.ron</code> and <code>whitelist.ron</code>. You can also make
		an operator by writing <code>["name"]</code> in <code>ops.ron</code> while the server is stopped.
	</p>
	<p>
		On a server started with <code>--auth hub</code>, the lists hold accounts rather than names: an
		operator stays one after changing their name. On a <strong>hosted world</strong> the owner
		manages members and operators from the Hosted worlds tab; there, <code>/op</code>,
		<code>/deop</code>, <code>/ban</code>, <code>/pardon</code> and <code>/whitelist</code> only
		answer so. <code>/kick</code> still works.
	</p>

	<h2>What players share</h2>
	<ul>
		<li>
			<strong>The time and the weather</strong> are the same for everyone. <code>/time</code> and
			<code>/weather</code> change them for the whole server.
		</li>
		<li><strong>The difficulty</strong> belongs to the world.</li>
		<li>
			<strong>The game mode</strong> belongs to each player: <code>/gamemode</code> changes only your own.
			New players start in the world's game mode.
		</li>
		<li>
			<strong>Sleeping</strong>: a night is skipped only once every player online has used a
			<ItemLink id="Bedroll" /> that night. Each player has their own respawn point.
		</li>
		<li>
			<strong>Inventory, position, experience and achievements</strong> are each player's own, kept
			by the world under their name (or their account on a hub server).
		</li>
		<li>
			<strong>Fighting other players</strong>: arrows, crossbow bolts and thrown axes hurt other
			players; melee blows only reach creatures.
		</li>
		<li>
			<strong>Chat</strong>: press <kbd>T</kbd>, type, and press <kbd>Enter</kbd>. Every player
			online reads it; the answer to a command goes only to whoever typed it.
		</li>
	</ul>
	<p>
		Each player picks a render distance, from 1 to 64 chunks, in Settings &gt; Video or with
		<kbd>O</kbd> and <kbd>P</kbd>. A server may grant less, up to its <code>--max-render-distance</code>,
		and the Video tab says so.
	</p>
</Page>
