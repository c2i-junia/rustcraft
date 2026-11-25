<script lang="ts">
	const repo = 'https://github.com/c2i-junia/rustcraft';
	// Link previews (Discord, Mastodon…) need absolute URLs.
	const site = 'https://rustcraft.dev';
	const description =
		'Rustcraft is an open-source block sandbox game written in Rust with Bevy. Generated worlds, survival, crafting and multiplayer.';

	const features = [
		{
			title: 'Generated worlds',
			text: 'Plains, forests, deserts, jungles, ice spikes and a dozen more biomes, with rivers, oceans, caves and villages. Four world types, from large biomes to archipelagos.'
		},
		{
			title: 'Survival and creative',
			text: 'Ten hearts, hunger, falls, fire and lava, and seven hostile creatures that come out in the dark. Or fly around in creative and build.'
		},
		{
			title: 'Crafting',
			text: 'Tools from wood to diamond, a furnace that keeps smelting while you are away, and a recipe book that fills the crafting grid for you.'
		},
		{
			title: 'Multiplayer',
			text: 'Run a dedicated server and play with friends. The server list shows who is online and how far away each server is.'
		},
		{
			title: 'Weather and time',
			text: 'Days and nights, rain and thunderstorms, the same for every player and kept in the save.'
		},
		{
			title: 'Open source',
			text: 'Written in Rust with the Bevy engine. Read the code, build it yourself, write server mods in Luau, or send a pull request.'
		}
	];
</script>

<svelte:head>
	<title>Rustcraft</title>
	<meta name="description" content={description} />
	<link rel="canonical" href="{site}/" />

	<meta property="og:type" content="website" />
	<meta property="og:site_name" content="Rustcraft" />
	<meta property="og:title" content="Rustcraft" />
	<meta property="og:description" content={description} />
	<meta property="og:url" content="{site}/" />
	<meta property="og:image" content="{site}/og.jpg" />
	<meta property="og:image:width" content="1200" />
	<meta property="og:image:height" content="630" />
	<meta property="og:image:alt" content="A forest and a desert in Rustcraft" />
	<meta name="twitter:card" content="summary_large_image" />
</svelte:head>

<header class="bar">
	<div class="inner">
		<a class="brand" href="/">Rustcraft</a>
		<nav aria-label="Main">
			<a href="#download">Download</a>
			<a href={repo}>GitHub</a>
		</nav>
	</div>
</header>

<main>
	<section class="hero">
		<img
			src="/images/world.webp"
			alt="A forest and a desert in Rustcraft, with a small wooden house in the foreground"
			width="1600"
			height="621"
		/>
		<div class="hero-text inner">
			<h1>Rustcraft</h1>
			<p>
				An open-source sandbox made of blocks. Explore generated worlds, get through the night, and
				build alone or with friends.
			</p>
			<div class="actions">
				<a class="button primary" href="{repo}/releases">Download for Windows and Linux</a>
				<a class="button" href={repo}>View the source</a>
			</div>
		</div>
	</section>

	<section class="inner features" aria-labelledby="features-title">
		<h2 id="features-title">What's in the game</h2>
		<ul>
			{#each features as feature (feature.title)}
				<li>
					<h3>{feature.title}</h3>
					<p>{feature.text}</p>
				</li>
			{/each}
		</ul>
	</section>

	<section class="inner download" id="download" aria-labelledby="download-title">
		<h2 id="download-title">Download</h2>
		<div class="ways">
			<div>
				<h3>Windows and Linux</h3>
				<p>
					Get the archive for your system from the <a href="{repo}/releases">releases page</a>,
					extract it, and run the game from its <code>bin</code> folder.
				</p>
			</div>
			<div>
				<h3>Arch Linux</h3>
				<p>From the AUR, built from source or prebuilt:</p>
				<pre><code>paru -S rustcraft
paru -S rustcraft-bin</code></pre>
			</div>
			<div>
				<h3>From source</h3>
				<p>
					You need Rust, <code>just</code> and a few system libraries. The
					<a href="{repo}#how-to-build">build guide</a> lists them for Windows, Arch, Debian and Fedora.
				</p>
			</div>
		</div>
	</section>
</main>

<footer class="inner">
	<p>
		Rustcraft is free and open source. Textures from the
		<a href="https://github.com/NovaWostra/Pixel-Perfection-Chorus-Eddit">Pixel Perfection</a> resource
		pack by XSSheep and Nova_Wostra.
	</p>
	<p>Not affiliated with Mojang or Microsoft.</p>
</footer>

<style>
	.inner {
		max-width: 1080px;
		margin: 0 auto;
		padding-left: max(20px, env(safe-area-inset-left, 0px));
		padding-right: max(20px, env(safe-area-inset-right, 0px));
		box-sizing: border-box;
	}

	.bar {
		position: absolute;
		inset: 0 0 auto;
		z-index: 1;
	}
	.bar .inner {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding-block: 18px;
	}
	.brand {
		font-family: var(--display);
		font-weight: 700;
		font-size: 1.3rem;
		color: var(--fg);
		text-decoration: none;
	}
	nav {
		display: flex;
		gap: 22px;
	}
	nav a {
		color: var(--fg);
		text-decoration: none;
		font-weight: 500;
	}
	nav a:hover {
		color: var(--rust);
	}

	.hero {
		position: relative;
		display: grid;
		min-height: min(78vh, 720px);
		align-items: end;
		overflow: hidden;
	}
	.hero img {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		object-fit: cover;
		image-rendering: pixelated;
	}
	.hero::after {
		content: '';
		position: absolute;
		inset: 0;
		background:
			linear-gradient(to bottom, rgb(22 19 15 / 0.55), transparent 22%),
			linear-gradient(to top, var(--bg) 4%, rgb(22 19 15 / 0.75) 38%, transparent 72%);
	}
	.hero-text {
		position: relative;
		z-index: 1;
		width: 100%;
		padding-block: 120px 56px;
		display: flex;
		flex-direction: column;
		gap: 18px;
	}
	h1 {
		font-family: var(--display);
		font-weight: 700;
		font-size: clamp(3.2rem, 11vw, 6.5rem);
		line-height: 0.95;
		margin: 0;
		letter-spacing: 0.01em;
		text-shadow: 0 4px 0 var(--rust-deep);
	}
	.hero p {
		margin: 0;
		max-width: 34rem;
		font-size: 1.2rem;
		text-wrap: balance;
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 12px;
		margin-top: 6px;
	}
	.button {
		font-family: var(--display);
		font-weight: 500;
		font-size: 1.05rem;
		text-decoration: none;
		color: var(--fg);
		padding: 12px 20px;
		border: 2px solid var(--fg);
		background: rgb(22 19 15 / 0.6);
		box-shadow: 4px 4px 0 rgb(0 0 0 / 0.5);
		transition: transform 80ms;
	}
	.button.primary {
		background: var(--rust);
		border-color: var(--rust);
		color: #1a0e06;
	}
	.button:hover {
		transform: translate(-2px, -2px);
	}
	.button:active {
		transform: translate(2px, 2px);
		box-shadow: 1px 1px 0 rgb(0 0 0 / 0.5);
	}

	h2 {
		font-family: var(--display);
		font-weight: 700;
		font-size: 2rem;
		margin: 0 0 24px;
	}
	h3 {
		font-family: var(--display);
		font-weight: 500;
		font-size: 1.25rem;
		margin: 0 0 6px;
		color: var(--rust);
	}

	.features {
		padding-block: 48px;
	}
	.features ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
		gap: 32px 40px;
	}
	.features p,
	.download p {
		margin: 0;
		color: var(--muted);
	}

	.download {
		padding-block: 48px 64px;
	}
	.ways {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
		gap: 16px;
	}
	.ways > div {
		background: var(--surface);
		border: 1px solid var(--line);
		padding: 20px 22px;
		display: flex;
		flex-direction: column;
		gap: 10px;
		min-width: 0;
	}
	pre {
		margin: 0;
		background: var(--bg);
		border: 1px solid var(--line);
		padding: 12px 14px;
		overflow-x: auto;
		font-size: 0.9rem;
	}
	code {
		font-family: var(--mono);
		font-size: 0.92em;
	}

	footer {
		border-top: 1px solid var(--line);
		padding-block: 28px 40px;
		color: var(--muted);
		font-size: 0.92rem;
	}
	footer p {
		margin: 0 0 4px;
	}

	@media (prefers-reduced-motion: reduce) {
		.button {
			transition: none;
		}
	}
</style>
